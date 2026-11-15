// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ud_m0_eval` — Phase 3 product smoke test for the M0 model.
//!
//! Build with `--features neural-m0` to run the embedded model. The report
//! separates candidate top-1 from UD role accuracy; it is not a claim that
//! the model improves diagnostics.

use std::collections::BTreeMap;
use std::process::ExitCode;

use torille::disambiguation::{LemmaProvenance, disambiguate};
use torille::morphology::analyze_morphology;
use torille::syntax::{analyze_syntax, analyze_syntax_with_decisions};
use torille_tools::ud::coverage::{matching_views, synthetic_tokens};
use torille_tools::ud::mapping::{MappingStatus, map_token};
use torille_tools::ud::reader::{UdToken, read_file};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("ud_m0_eval: {message}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" => file = Some(args.next().ok_or("--file needs a path")?),
            "--help" | "-h" => {
                println!("usage: ud_m0_eval --file corpus.conllu");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let file = file.ok_or("missing --file corpus.conllu")?;
    let sentences = read_file(&file).map_err(|error| error.to_string())?;
    let cat = torille::spec::Catalogues::embedded();
    let index = torille::expand::LexiconIndex::new(torille::kotus::lemmas());

    let mut ambiguous = 0usize;
    let mut labelled = 0usize;
    let mut no_decision = 0usize;
    let mut model_correct = 0usize;
    let mut first_correct = 0usize;
    let mut role_total = 0usize;
    let mut role_base_correct = 0usize;
    let mut role_model_correct = 0usize;
    let mut role_changed = 0usize;
    let mut role_improved = 0usize;
    let mut role_worsened = 0usize;
    let mut decided_role_total = 0usize;
    let mut decided_role_base_correct = 0usize;
    let mut decided_role_model_correct = 0usize;
    let mut by_role: BTreeMap<&'static str, (usize, usize, usize)> = BTreeMap::new();

    for sentence in &sentences {
        let (tokens, aligned) = synthetic_tokens(sentence);
        if tokens.is_empty() {
            continue;
        }
        let mut cache = torille::morphology::MorphologyCache::new();
        let morphology = analyze_morphology(&tokens, &mut cache);
        let decisions = disambiguate(&tokens, &morphology);
        let baseline = analyze_syntax(&tokens, &morphology);
        let model = analyze_syntax_with_decisions(&tokens, &morphology, &decisions);
        for (ud_index, token_index) in aligned {
            let Some(token_index) = token_index else {
                continue;
            };
            let ud_token = &sentence.tokens[ud_index];
            if let Some(role) = gold_role(ud_token) {
                let base_role = baseline.tokens[token_index].role;
                let model_role = model.tokens[token_index].role;
                let base_correct = base_role.name() == role;
                let model_correct = model_role.name() == role;
                role_total += 1;
                if base_correct {
                    role_base_correct += 1;
                }
                if model_correct {
                    role_model_correct += 1;
                }
                if decisions.decision(token_index).is_some() {
                    decided_role_total += 1;
                    if base_correct {
                        decided_role_base_correct += 1;
                    }
                    if model_correct {
                        decided_role_model_correct += 1;
                    }
                }
                if base_role != model_role {
                    role_changed += 1;
                    match (base_correct, model_correct) {
                        (false, true) => role_improved += 1,
                        (true, false) => role_worsened += 1,
                        _ => {}
                    }
                }
                let entry = by_role.entry(role).or_insert((0, 0, 0));
                entry.0 += 1;
                if base_role.name() == role {
                    entry.1 += 1;
                }
                if model_role.name() == role {
                    entry.2 += 1;
                }
            }
            let gold = map_token(ud_token);
            if matches!(gold.status, MappingStatus::Exact | MappingStatus::Partial)
                && baseline.tokens[token_index].readings.len() >= 2
            {
                ambiguous += 1;
                if let Some(label) = gold_candidate_label(
                    &baseline.tokens[token_index],
                    token_index,
                    &gold,
                    cat,
                    &index,
                ) {
                    labelled += 1;
                    if label == 0 {
                        first_correct += 1;
                    }
                    match decisions.decision(token_index) {
                        Some(decision) if !decision.abstain => {
                            if decision.preferred == label {
                                model_correct += 1;
                            }
                        }
                        _ => no_decision += 1,
                    }
                }
            }
        }
    }

    println!("M0 product smoke test");
    println!("file: {file}");
    println!("ambiguous tokens: {ambiguous}");
    println!("labelled tokens: {labelled}");
    println!("model decisions: {}", labelled.saturating_sub(no_decision));
    println!("no decision: {no_decision}");
    println!(
        "candidate top-1: {:.4} (first candidate {:.4})",
        ratio(model_correct, labelled),
        ratio(first_correct, labelled)
    );
    println!("role tokens: {role_total}");
    println!(
        "role accuracy baseline={:.4} model={:.4}",
        ratio(role_base_correct, role_total),
        ratio(role_model_correct, role_total)
    );
    println!(
        "decided role tokens: {decided_role_total} baseline={:.4} model={:.4}",
        ratio(decided_role_base_correct, decided_role_total),
        ratio(decided_role_model_correct, decided_role_total)
    );
    println!("role changes: {role_changed} improved={role_improved} worsened={role_worsened}");
    println!("role\tgold\tbaseline\tmodel");
    for (role, (total, base, model)) in by_role {
        println!("{role}\t{total}\t{base}\t{model}");
    }
    Ok(())
}

fn gold_candidate_label(
    syntax: &torille::syntax::TokenSyntax,
    token_index: usize,
    gold: &torille_tools::ud::mapping::GoldReading,
    cat: &torille::spec::Catalogues,
    index: &torille::expand::LexiconIndex<'_>,
) -> Option<usize> {
    let (matches, provenance_unavailable) = matching_views(syntax, token_index, gold, cat, index);
    if provenance_unavailable || matches.len() != 1 {
        return None;
    }
    let gold_lemma = gold.lemma.as_deref()?;
    match &matches[0].lemma_provenance {
        LemmaProvenance::Known(lemmas) if lemmas.contains(&gold_lemma) => {
            Some(matches[0].reading_index)
        }
        _ => None,
    }
}

fn gold_role(token: &UdToken) -> Option<&'static str> {
    let deprel = token.deprel.as_str();
    match deprel {
        "nsubj" | "nsubj:cop" | "csubj" | "csubj:cop" => Some("subject"),
        "obj" => Some("object"),
        "root" | "advcl" | "ccomp" | "xcomp" | "acl" | "acl:relcl"
            if matches!(token.upos.as_str(), "VERB" | "AUX") =>
        {
            Some("predicate")
        }
        "aux" | "cop" => Some("auxiliary"),
        "obl" | "advmod" => Some("adverbial"),
        _ => None,
    }
}

fn ratio(correct: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        correct as f64 / total as f64
    }
}
