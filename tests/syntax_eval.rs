//! Aggregate evaluation for the syntax checker.
//!
//! `tests/fixtures/syntax_eval.json` is the measurement snapshot: clean rows
//! must produce no `Error`, and every case must produce the kinds it names.
//! The exact-output gate stays in `tests/syntax_check.rs`; this test reports
//! per-kind precision/recall so a new rule cannot hide behind the small
//! per-case fixture.

use std::collections::BTreeMap;

use serde_json::Value;
use torille::syntax_check::{self, DiagnosticKind, Severity};

/// Every kind the checker currently emits. The reserved kinds are deliberately
/// absent: presence here is a coverage requirement for the evaluation corpus.
const EMITTED_KINDS: &[DiagnosticKind] = &[
    DiagnosticKind::SubjectPredicateAgreement,
    DiagnosticKind::AttributeAgreement,
    DiagnosticKind::VerbRection,
    DiagnosticKind::ObjectCase,
    DiagnosticKind::MissingArgument,
    DiagnosticKind::ExtraArgument,
    DiagnosticKind::CommaMissing,
    DiagnosticKind::CommaPairUnbalanced,
    DiagnosticKind::ListSeparatorMissing,
    DiagnosticKind::ListSeparatorDuplicate,
    DiagnosticKind::ListCommaBeforeConjunction,
    DiagnosticKind::ListIntroducerComma,
];

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/syntax_eval.json"))
        .expect("tests/fixtures/syntax_eval.json is not valid JSON")
}

fn kind(name: &str) -> DiagnosticKind {
    EMITTED_KINDS
        .iter()
        .copied()
        .find(|kind| kind.name() == name)
        .unwrap_or_else(|| panic!("unknown emitted diagnostic kind {name:?}"))
}

#[test]
fn the_evaluation_clean_rows_have_no_errors() {
    let root = fixture();
    let clean = root["clean"].as_array().expect("clean must be an array");
    let mut errors = Vec::new();
    let mut warnings = 0usize;
    let mut suggestions = 0usize;
    for text in clean {
        let text = text.as_str().expect("clean entries must be strings");
        let check = syntax_check::check(text);
        for diagnostic in &check.diagnostics {
            match diagnostic.severity {
                Severity::Error => errors.push(format!(
                    "{text:?}: {} at {}..{}",
                    diagnostic.kind.name(),
                    diagnostic.span.start,
                    diagnostic.span.end
                )),
                Severity::Warning => warnings += 1,
                Severity::Suggestion => suggestions += 1,
            }
        }
    }
    assert!(
        errors.is_empty(),
        "clean evaluation rows produced errors:\n{}",
        errors.join("\n")
    );
    eprintln!(
        "clean rows: {}  warnings: {warnings}  suggestions: {suggestions}",
        clean.len()
    );
}

#[test]
fn the_evaluation_cases_match_their_expected_kinds() {
    let root = fixture();
    let cases = root["cases"].as_array().expect("cases must be an array");

    let mut expected_counts: BTreeMap<DiagnosticKind, usize> = BTreeMap::new();
    let mut actual_counts: BTreeMap<DiagnosticKind, usize> = BTreeMap::new();
    let mut hits: BTreeMap<DiagnosticKind, usize> = BTreeMap::new();
    let mut failures = Vec::new();

    for case in cases {
        let id = case["id"].as_str().expect("case id must be a string");
        let text = case["text"].as_str().expect("case text must be a string");
        let expected: Vec<DiagnosticKind> = case["kinds"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: kinds must be an array"))
            .iter()
            .map(|value| kind(value.as_str().expect("kind entries must be strings")))
            .collect();
        let actual: Vec<DiagnosticKind> = syntax_check::check(text)
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
        "{:<28} {:>8} {:>6} {:>4} {:>10} {:>6}",
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
            "{:<28} {expected:>8} {actual:>6} {hit:>4} {precision:>10.2} {recall:>6.2}",
            kind.name()
        );
    }
}
