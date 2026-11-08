// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_adj_degrees` — write `data/adj_degrees.csv` from the engine.
//!
//! For every adjective (and every pronoun, since `usea` behaves like one),
//! ask the engine for its nominative-singular forms and take the `-mpi`
//! comparative and the `-in` superlative. The table is embedded by
//! `build.rs`; `src/expand/derived_forms` also has productive fallbacks, so
//! a word with no table row is not necessarily degree-less.
//!
//! Run after a class/derivation change:
//!
//!     cargo run --release -p torille-tools --bin gen_adj_degrees
//!     cargo build -p torille

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use torille::expand::LexiconIndex;
use torille::kotus::{Lemma, lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::{Case, Number};

fn main() {
    let mut out = PathBuf::from("data/adj_degrees.csv");
    let mut limit: Option<usize> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                out = PathBuf::from(args.next().unwrap_or_else(|| usage()));
            }
            "--limit" => {
                limit = Some(
                    args.next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or_else(|| usage()),
                );
            }
            _ => usage(),
        }
    }

    let catalogue = Catalogues::embedded();
    let lexicon = sanalista().to_lemmas();
    let index = LexiconIndex::new(&lexicon);

    let mut candidates: Vec<&'static Lemma<'static>> = Vec::new();
    for lemma in lemmas() {
        if is_classed(lemma) && has_pos(lemma, "adjektiivi") {
            candidates.push(lemma);
        }
    }
    for lemma in lemmas() {
        if is_classed(lemma) && has_pos(lemma, "pronomini") {
            candidates.push(lemma);
        }
    }
    if let Some(max) = limit {
        candidates.truncate(max);
    }

    let mut seen = HashSet::new();
    let mut rows: Vec<(String, String, String)> = Vec::new();
    for lemma in candidates {
        if !seen.insert(lemma.word) {
            continue;
        }
        let forms = torille_tools::inflect_lemma(lemma.word, catalogue, &index);
        let mut nominatives: Vec<&str> = Vec::new();
        for form in &forms {
            let tags = form.union();
            if tags.case == Some(Case::Nominative) && tags.number == Some(Number::Singular)
                && !nominatives.contains(&form.word.as_str())
            {
                nominatives.push(&form.word);
            }
        }
        let comparative = nominatives
            .iter()
            .find(|w| **w != lemma.word && w.ends_with("mpi"))
            .copied()
            .unwrap_or("");
        let superlative = nominatives
            .iter()
            .find(|w| **w != lemma.word && **w != comparative && w.ends_with("in"))
            .copied()
            .unwrap_or("");
        if !comparative.is_empty() || !superlative.is_empty() {
            rows.push((
                lemma.word.to_string(),
                comparative.to_string(),
                superlative.to_string(),
            ));
        }
    }

    let mut text = String::from("positive\tcomparative\tsuperlative\n");
    // Curated rows that the engine cannot infer from the positive itself
    // (`loitto` -> `loitompi`). They live in a separate source file so
    // regenerating this table does not drop them.
    let extra = Path::new("data/adj_degrees_extra.csv");
    let mut extra_rows: Vec<(String, String, String)> = Vec::new();
    if let Ok(text) = fs::read_to_string(extra) {
        for line in text.lines().skip(1) {
            let mut cols = line.split('\t');
            let Some(positive) = cols.next().map(str::trim).filter(|w| !w.is_empty()) else {
                continue;
            };
            let comparative = cols.next().unwrap_or("").trim();
            let superlative = cols.next().unwrap_or("").trim();
            extra_rows.push((
                positive.to_string(),
                comparative.to_string(),
                superlative.to_string(),
            ));
        }
    }
    let engine_positive: HashSet<String> = rows.iter().map(|(p, _, _)| p.clone()).collect();
    for row in &extra_rows {
        if !engine_positive.contains(&row.0) {
            rows.push(row.clone());
        }
    }
    for (positive, comparative, superlative) in &rows {
        text.push_str(positive);
        text.push('\t');
        text.push_str(comparative);
        text.push('\t');
        text.push_str(superlative);
        text.push('\n');
    }
    fs::write(&out, text).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    let both = rows
        .iter()
        .filter(|(_, c, s)| !c.is_empty() && !s.is_empty())
        .count();
    eprintln!(
        "{} adjectives with a degree form ({both} with both), of {}",
        rows.len(),
        seen.len()
    );
    eprintln!("wrote {}", out.display());
}

fn usage() -> ! {
    eprintln!("usage: gen_adj_degrees [--out PATH] [--limit N]");
    std::process::exit(2);
}

fn has_pos(lemma: &Lemma<'_>, pos: &str) -> bool {
    lemma
        .part_of_speech
        .split([',', '+'])
        .any(|item| item.trim() == pos)
}

fn is_classed(lemma: &Lemma<'_>) -> bool {
    !lemma.inflections.is_empty()
}
