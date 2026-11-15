// SPDX-License-Identifier: MIT OR Apache-2.0
//! `m0_prose_eval` — Phase 3 diagnostic comparison on adjudicated prose.
//!
//! Runs the same grammar pass twice: once with the baseline role classifier
//! and once with the enabled M0 reading preferences. Style is not evaluated
//! because the sidecar may not change it. Requires `--features neural-m0`;
//! without that feature both sides are the baseline.
//!
//! ```text
//! m0_prose_eval --file tests/fixtures/prose/adjudicated.json
//! ```

use std::collections::BTreeMap;
use std::process::ExitCode;

use serde_json::Value;
use torille::disambiguation::disambiguate;
use torille::morphology::{MorphologyCache, analyze_morphology};
use torille::syntax::{analyze_syntax, analyze_syntax_with_decisions};
use torille::syntax_check::{SyntaxDiagnostic, check_analysis};
use torille::tokenise::tokenise;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("m0_prose_eval: {message}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut file = None;
    let mut text_file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" => file = Some(args.next().ok_or("--file needs a path")?),
            "--text" => text_file = Some(args.next().ok_or("--text needs a path")?),
            "--help" | "-h" => {
                println!("usage: m0_prose_eval --file adjudicated.json");
                println!("       m0_prose_eval --text prose.txt");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if let Some(text_file) = text_file {
        let text =
            std::fs::read_to_string(&text_file).map_err(|error| format!("{text_file}: {error}"))?;
        return run_text(&text_file, &text);
    }
    let file = file.ok_or("missing --file adjudicated.json or --text prose.txt")?;
    let text = std::fs::read_to_string(&file).map_err(|error| format!("{file}: {error}"))?;
    let root: Value = serde_json::from_str(&text).map_err(|error| format!("{file}: {error}"))?;

    let mut summary = Summary::default();
    for (row_index, row) in rows(&root, "clean")?.iter().enumerate() {
        let kinds = compare(row)?;
        summary.clean_rows += 1;
        summary.clean_words += word_count(row);
        summary.clean_baseline += kinds.base.len();
        summary.clean_model += kinds.model.len();
        report_change("clean", row_index, row, &kinds)?;
    }
    for (row_index, row) in rows(&root, "errors")?.iter().enumerate() {
        let kinds = compare(row)?;
        let expected = expected_kinds(row)?;
        summary.error_rows += 1;
        if kinds.base == expected {
            summary.error_baseline += 1;
        }
        if kinds.model == expected {
            summary.error_model += 1;
        }
        report_change("error", row_index, row, &kinds)?;
    }
    for (row_index, row) in rows(&root, "known_false_positives")?.iter().enumerate() {
        let kinds = compare(row)?;
        let expected = expected_kinds(row)?;
        summary.known_rows += 1;
        summary.known_words += word_count(row);
        if kinds.base != expected {
            println!(
                "known row {row_index} baseline {:?} expected {:?}: {}",
                kinds.base,
                expected,
                row_text(row)?
            );
        }
        if kinds.base == expected {
            summary.known_baseline += 1;
        }
        if kinds.model == expected {
            summary.known_model += 1;
        }
        report_change("known", row_index, row, &kinds)?;
    }

    println!("M0 adjudicated-prose grammar comparison");
    println!("file: {file}");
    println!(
        "clean: {} rows / {} words; diagnostics baseline={} model={}",
        summary.clean_rows, summary.clean_words, summary.clean_baseline, summary.clean_model
    );
    println!(
        "true positives: {} rows; exact kind match baseline={} model={}",
        summary.error_rows, summary.error_baseline, summary.error_model
    );
    println!(
        "known false positives: {} rows / {} words; unchanged baseline={} model={}",
        summary.known_rows, summary.known_words, summary.known_baseline, summary.known_model
    );
    Ok(())
}

#[derive(Default)]
struct Summary {
    clean_rows: usize,
    clean_words: usize,
    clean_baseline: usize,
    clean_model: usize,
    error_rows: usize,
    error_baseline: usize,
    error_model: usize,
    known_rows: usize,
    known_words: usize,
    known_baseline: usize,
    known_model: usize,
}

type Detail = (String, String);

struct Comparison {
    base: Vec<String>,
    model: Vec<String>,
    base_details: Vec<Detail>,
    model_details: Vec<Detail>,
    roles: Vec<(usize, String, String, String)>,
    base_clauses: Vec<String>,
    model_clauses: Vec<String>,
}

fn compare(row: &Value) -> Result<Comparison, String> {
    compare_text(row_text(row)?)
}

fn compare_text(text: &str) -> Result<Comparison, String> {
    let tokens = tokenise(text);
    let mut cache = MorphologyCache::new();
    let morphology = analyze_morphology(&tokens, &mut cache);
    let decisions = disambiguate(&tokens, &morphology);
    let baseline = analyze_syntax(&tokens, &morphology);
    let model = analyze_syntax_with_decisions(&tokens, &morphology, &decisions);
    let base_diagnostics = check_analysis(&tokens, &baseline);
    let model_diagnostics = check_analysis(&tokens, &model);
    let roles = tokens
        .iter()
        .enumerate()
        .filter(|(index, _)| baseline.tokens[*index].role != model.tokens[*index].role)
        .map(|(index, token)| {
            let preferred = decisions
                .decision(index)
                .and_then(|decision| baseline.tokens[index].readings.get(decision.preferred))
                .map(|tags| format!("{tags:?}"))
                .unwrap_or_else(|| "none".to_string());
            (
                index,
                format!("{}:{}", token.text, baseline.tokens[index].role.name()),
                model.tokens[index].role.name().to_string(),
                preferred,
            )
        })
        .collect();
    Ok(Comparison {
        base: kinds(&base_diagnostics),
        model: kinds(&model_diagnostics),
        base_details: details(&base_diagnostics, text),
        model_details: details(&model_diagnostics, text),
        roles,
        base_clauses: clauses(&baseline, &tokens),
        model_clauses: clauses(&model, &tokens),
    })
}

fn report_change(
    group: &str,
    row_index: usize,
    row: &Value,
    comparison: &Comparison,
) -> Result<(), String> {
    if comparison.base == comparison.model {
        return Ok(());
    }
    let text = row_text(row)?;
    println!("changed {group} row {row_index}: {text}");
    println!("  baseline: {:?}", comparison.base);
    println!("  model:    {:?}", comparison.model);
    for (kind, message) in &comparison.model_details {
        if comparison.base.contains(kind) {
            continue;
        }
        println!("  added {kind}: {message}");
    }
    for (kind, message) in &comparison.base_details {
        if comparison.model.contains(kind) {
            continue;
        }
        println!("  removed {kind}: {message}");
    }
    for (index, before, after, preferred) in &comparison.roles {
        println!("  role token {index}: {before} -> {after} (preferred {preferred})");
    }
    for (before, after) in comparison
        .base_clauses
        .iter()
        .zip(&comparison.model_clauses)
    {
        if before != after {
            println!("  clause: {before} -> {after}");
        }
    }
    Ok(())
}

fn run_text(path: &str, text: &str) -> Result<(), String> {
    let comparison = compare_text(text)?;
    println!("M0 prose grammar comparison");
    println!("file: {path}");
    println!(
        "diagnostics baseline={} model={}",
        comparison.base.len(),
        comparison.model.len()
    );
    for (kind, message) in &comparison.model_details {
        if comparison.base.contains(kind) {
            continue;
        }
        println!("added {kind}: {message}");
    }
    for (kind, message) in &comparison.base_details {
        if comparison.model.contains(kind) {
            continue;
        }
        println!("removed {kind}: {message}");
    }
    let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for kind in &comparison.base {
        counts.entry(kind.clone()).or_default().0 += 1;
    }
    for kind in &comparison.model {
        counts.entry(kind.clone()).or_default().1 += 1;
    }
    println!("kind\tbaseline\tmodel");
    for (kind, (base, model)) in &counts {
        if base != model {
            println!("{kind}\t{base}\t{model}");
        }
    }
    let (added, removed) = detail_difference(&comparison.base_details, &comparison.model_details);
    for (kind, message) in added {
        println!("added {kind}: {message}");
    }
    for (kind, message) in removed {
        println!("removed {kind}: {message}");
    }
    println!("role changes: {}", comparison.roles.len());
    for (index, before, after, preferred) in &comparison.roles {
        println!("  role token {index}: {before} -> {after} (preferred {preferred})");
    }
    Ok(())
}

fn clauses(
    analysis: &torille::syntax::SyntaxAnalysis,
    tokens: &[torille::tokenise::Token],
) -> Vec<String> {
    analysis
        .clauses
        .iter()
        .map(|clause| {
            let word = |index: Option<usize>| {
                index
                    .map(|index| tokens[index].text.as_str())
                    .unwrap_or("-")
                    .to_string()
            };
            format!(
                "pred={} subj={} obj={} neg={}",
                word(clause.predicate),
                word(clause.subject),
                word(clause.object),
                clause.negative
            )
        })
        .collect()
}

fn details(diagnostics: &[SyntaxDiagnostic], text: &str) -> Vec<Detail> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let span = text
                .get(diagnostic.span.clone())
                .unwrap_or_default()
                .to_string();
            (
                diagnostic.kind.name().to_string(),
                format!("{} [{span:?}]", diagnostic.message),
            )
        })
        .collect()
}

/// Multiset difference over `(kind, message)` details, where the message
/// already carries the source span.
fn detail_difference(base: &[Detail], model: &[Detail]) -> (Vec<Detail>, Vec<Detail>) {
    let mut remaining: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (kind, message) in base {
        *remaining.entry((kind, message)).or_default() += 1;
    }
    let mut added = Vec::new();
    for (kind, message) in model {
        let key = (kind.as_str(), message.as_str());
        match remaining.get_mut(&key) {
            Some(count) if *count > 0 => *count -= 1,
            _ => added.push((kind.clone(), message.clone())),
        }
    }
    let mut removed = Vec::new();
    for (kind, message) in base {
        let key = (kind.as_str(), message.as_str());
        if let Some(count) = remaining.get_mut(&key)
            && *count > 0
        {
            *count -= 1;
            removed.push((kind.clone(), message.clone()));
        }
    }
    (added, removed)
}

fn kinds(diagnostics: &[SyntaxDiagnostic]) -> Vec<String> {
    let mut kinds: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.kind.name().to_string())
        .collect();
    kinds.sort_unstable();
    kinds
}

fn expected_kinds(row: &Value) -> Result<Vec<String>, String> {
    let mut kinds: Vec<String> = row["kinds"]
        .as_array()
        .ok_or("row kinds must be an array")?
        .iter()
        .map(|kind| {
            kind.as_str()
                .map(str::to_string)
                .ok_or("row kind must be a string")
        })
        .collect::<Result<_, _>>()?;
    kinds.sort_unstable();
    Ok(kinds)
}

fn rows<'a>(root: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    root[key]
        .as_array()
        .ok_or_else(|| format!("{key} must be an array"))
}

fn row_text(row: &Value) -> Result<&str, String> {
    row.as_str()
        .or_else(|| row["text"].as_str())
        .ok_or_else(|| "row must be a string or an object with a text string".to_string())
}

fn word_count(row: &Value) -> usize {
    row_text(row)
        .map(|text| {
            tokenise(text)
                .iter()
                .filter(|token| token.is_word())
                .count()
        })
        .unwrap_or(0)
}
