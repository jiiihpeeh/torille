//! Adjudicated Finnish prose for the grammar checker.
//!
//! `tests/fixtures/prose/adjudicated.json` separates:
//!
//! - `clean`: reviewed sentences that must stay completely quiet;
//! - `errors`: reviewed true positives whose kinds must match exactly;
//! - `known_false_positives`: reviewed bugs. They pin the current kinds so a
//!   fix fails the suite and the entry can move to `clean` (or to `errors`).
//!
//! The suite prints the false-positive rate over the clean word count. Grow the
//! corpus a paragraph at a time; do not move a known row until it has been
//! reviewed again.

use serde_json::Value;
use std::path::{Path, PathBuf};
use torille::syntax_check;
use torille::tokenise::tokenise;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/prose/adjudicated.json"))
        .expect("tests/fixtures/prose/adjudicated.json is not valid JSON")
}

fn curated_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prose/curated")
}

/// Every `*.txt` under `tests/fixtures/prose/curated/`, sorted for a stable
/// order. A curated file is one paragraph per line; `#` lines are comments.
fn curated_rows() -> Vec<String> {
    let dir = curated_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .collect();
    files.sort();
    let mut rows = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            rows.push(line.to_string());
        }
    }
    rows
}

fn curated_word_count(rows: &[String]) -> usize {
    rows.iter().map(|text| word_count(text)).sum()
}

fn diagnostic_kinds(text: &str) -> Vec<String> {
    syntax_check::check(text)
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.kind.name().to_string())
        .collect()
}

fn word_count(text: &str) -> usize {
    tokenise(text)
        .iter()
        .filter(|token| token.is_word())
        .count()
}

fn expected_kinds(entry: &Value) -> Vec<String> {
    let mut kinds: Vec<String> = entry["kinds"]
        .as_array()
        .expect("entry kinds must be an array")
        .iter()
        .map(|kind| {
            kind.as_str()
                .expect("entry kind must be a string")
                .to_string()
        })
        .collect();
    kinds.sort_unstable();
    kinds
}

fn actual_kinds(kinds: Vec<String>) -> Vec<String> {
    let mut kinds = kinds;
    kinds.sort_unstable();
    kinds
}

fn clean_rows(root: &Value) -> Vec<&str> {
    root["clean"]
        .as_array()
        .expect("clean must be an array")
        .iter()
        .map(|text| text.as_str().expect("clean entries must be strings"))
        .collect()
}

fn clean_word_count(root: &Value) -> usize {
    clean_rows(root).into_iter().map(word_count).sum()
}

fn entry_word_count(entries: &[Value]) -> usize {
    entries
        .iter()
        .map(|entry| word_count(entry["text"].as_str().expect("entry text must be a string")))
        .sum()
}

#[test]
fn all_clean_prose_is_quiet() {
    let root = fixture();
    let clean = clean_rows(&root);
    let curated = curated_rows();
    for text in clean
        .iter()
        .copied()
        .chain(curated.iter().map(String::as_str))
    {
        let check = syntax_check::check(text);
        assert!(
            check.diagnostics.is_empty(),
            "clean prose produced diagnostics: {text:?}\n{:#?}",
            check.diagnostics
        );
    }
    let adjudicated_words = clean_word_count(&root);
    let curated_words = curated_word_count(&curated);
    eprintln!(
        "adjudicated prose: {} rows, {} words",
        clean.len(),
        adjudicated_words
    );
    eprintln!(
        "curated prose: {} rows, {} words",
        curated.len(),
        curated_words
    );
    eprintln!(
        "all clean prose: {} rows, {} words",
        clean.len() + curated.len(),
        adjudicated_words + curated_words
    );
}

#[test]
fn adjudicated_true_positives_match_expected_kinds() {
    let root = fixture();
    let errors = root["errors"]
        .as_array()
        .expect("errors must be an array");
    for entry in errors {
        let text = entry["text"].as_str().expect("error text must be a string");
        let expected = expected_kinds(entry);
        let actual = actual_kinds(diagnostic_kinds(text));
        assert_eq!(
            actual, expected,
            "true-positive row changed: {text:?}\nentry: {entry:#?}"
        );
    }
}

#[test]
fn known_false_positives_match_the_reviewed_baseline() {
    let root = fixture();
    let known = root["known_false_positives"]
        .as_array()
        .expect("known_false_positives must be an array");
    let mut known_errors = 0usize;
    for entry in known {
        let text = entry["text"]
            .as_str()
            .expect("known false positive text must be a string");
        let expected = expected_kinds(entry);
        let actual = actual_kinds(diagnostic_kinds(text));
        known_errors += actual.len();
        assert_eq!(
            actual, expected,
            "known false positive changed; if it is fixed, move the row to `clean` or `errors`: {text:?}\nentry: {entry:#?}"
        );
    }
    let clean_words = clean_word_count(&root);
    let known_words = entry_word_count(known);
    let correct_words = clean_words + known_words;
    let per_1000 = if correct_words == 0 {
        0.0
    } else {
        known_errors as f64 * 1000.0 / correct_words as f64
    };
    eprintln!(
        "adjudicated correct prose: {} clean words + {} known-false-positive words = {} words; {} known errors, {:.1} per 1000 words",
        clean_words, known_words, correct_words, known_errors, per_1000
    );
}
