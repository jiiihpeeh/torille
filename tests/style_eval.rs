//! Aggregate evaluation for the opt-in style layer.
//!
//! `tests/fixtures/style_eval.json` is the measurement snapshot: clean rows
//! must have no `Error`/`Warning`, no curated table hit and no more metric
//! findings than the policy allows. Every case must produce the kind it names.
//! The exact-output gate stays in `tests/style_check.rs`.

use std::collections::BTreeMap;

use serde_json::Value;
use torille::style_check::{self, StyleKind};
use torille::syntax_check::Severity;

/// Every kind the style layer currently emits.
const EMITTED_KINDS: &[StyleKind] = &[
    StyleKind::Repetition,
    StyleKind::LongSentence,
    StyleKind::Cliche,
    StyleKind::Filler,
    StyleKind::Wordy,
    StyleKind::Colloquial,
];

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/style_eval.json"))
        .expect("tests/fixtures/style_eval.json is not valid JSON")
}

fn kind(name: &str) -> StyleKind {
    EMITTED_KINDS
        .iter()
        .copied()
        .find(|kind| kind.name() == name)
        .unwrap_or_else(|| panic!("unknown emitted style kind {name:?}"))
}

fn is_table(kind: StyleKind) -> bool {
    matches!(
        kind,
        StyleKind::Cliche | StyleKind::Filler | StyleKind::Wordy | StyleKind::Colloquial
    )
}

#[test]
fn the_evaluation_clean_rows_stay_quiet() {
    let root = fixture();
    let clean = root["clean"].as_array().expect("clean must be an array");
    let mut table_hits = Vec::new();
    let mut metric_hits = Vec::new();
    let mut wrong_severity = Vec::new();

    for text in clean {
        let text = text.as_str().expect("clean entries must be strings");
        for diagnostic in style_check::check(text).diagnostics {
            if diagnostic.severity != Severity::Suggestion {
                wrong_severity.push(format!(
                    "{text:?}: {} is {}",
                    diagnostic.kind.name(),
                    diagnostic.severity.name()
                ));
            }
            if is_table(diagnostic.kind) {
                table_hits.push(format!("{text:?}: {}", diagnostic.kind.name()));
            } else {
                metric_hits.push(format!("{text:?}: {}", diagnostic.kind.name()));
            }
        }
    }

    assert!(
        wrong_severity.is_empty(),
        "clean rows produced a non-suggestion:\n{}",
        wrong_severity.join("\n")
    );
    assert!(
        table_hits.is_empty(),
        "clean rows hit the curated table:\n{}",
        table_hits.join("\n")
    );

    let budget = (clean.len() / 20).max(1);
    for hit in &metric_hits {
        eprintln!("clean metric finding: {hit}");
    }
    assert!(
        metric_hits.len() <= budget,
        "clean rows produced {} metric findings, budget is {budget}:\n{}",
        metric_hits.len(),
        metric_hits.join("\n")
    );
    eprintln!(
        "clean rows: {}  table hits: 0  metric findings: {}  budget: {budget}",
        clean.len(),
        metric_hits.len()
    );
}

#[test]
fn the_evaluation_cases_match_their_expected_kinds() {
    let root = fixture();
    let cases = root["cases"].as_array().expect("cases must be an array");

    let mut expected_counts: BTreeMap<StyleKind, usize> = BTreeMap::new();
    let mut actual_counts: BTreeMap<StyleKind, usize> = BTreeMap::new();
    let mut hits: BTreeMap<StyleKind, usize> = BTreeMap::new();
    let mut failures = Vec::new();

    for case in cases {
        let id = case["id"].as_str().expect("case id must be a string");
        let text = case["text"].as_str().expect("case text must be a string");
        let expected: Vec<StyleKind> = case["kinds"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: kinds must be an array"))
            .iter()
            .map(|value| kind(value.as_str().expect("kind entries must be strings")))
            .collect();
        let actual: Vec<StyleKind> = style_check::check(text)
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.kind)
            .collect();

        for kind in &expected {
            *expected_counts.entry(*kind).or_default() += 1;
        }
        for kind in &actual {
            *actual_counts.entry(*kind).or_default() += 1;
        }
        for kind in &expected {
            if actual.contains(kind) {
                *hits.entry(*kind).or_default() += 1;
            } else {
                failures.push(format!(
                    "{id}: expected {}, got {:?}",
                    kind.name(),
                    actual.iter().map(|kind| kind.name()).collect::<Vec<_>>()
                ));
            }
        }
    }

    for kind in EMITTED_KINDS {
        assert!(
            expected_counts.contains_key(kind),
            "evaluation corpus has no case for {}",
            kind.name()
        );
    }
    assert!(
        failures.is_empty(),
        "missing expected diagnostics:\n{}",
        failures.join("\n")
    );

    eprintln!(
        "{:<24} {:>8} {:>6} {:>4} {:>10} {:>6}",
        "kind", "expected", "actual", "hit", "precision", "recall"
    );
    for kind in EMITTED_KINDS {
        let expected = expected_counts.get(kind).copied().unwrap_or(0);
        let actual = actual_counts.get(kind).copied().unwrap_or(0);
        let hit = hits.get(kind).copied().unwrap_or(0);
        let precision = if actual == 0 {
            1.0
        } else {
            hit as f32 / actual as f32
        };
        let recall = if expected == 0 {
            1.0
        } else {
            hit as f32 / expected as f32
        };
        eprintln!(
            "{:<24} {expected:>8} {actual:>6} {hit:>4} {precision:>10.2} {recall:>6.2}",
            kind.name()
        );
    }
}
