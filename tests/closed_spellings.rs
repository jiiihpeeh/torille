//! The closed-spelling residue and the full/fast parity of its rows.
//!
//! `tests/fixtures/closed_spellings.json` is the historical 32-row table. The
//! invariants are:
//!
//! - `closed` rows resolve in production and must **not** resolve with the
//!   table disabled (a row that does is stale and should be deleted);
//! - `derived` rows resolve with the table disabled (the engine or its data
//!   carries them);
//! - `nonword` rows are rejected in production;
//! - the full and fast paths always agree.

use serde_json::Value;
use torille::expand::check_normal::{NormalizedOutcome, check_normal_with_policy};
use torille::expand::closed_spellings::Closed;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/closed_spellings.json"))
        .expect("tests/fixtures/closed_spellings.json is not valid JSON")
}

fn outcome(word: &str, engine_gate: bool, closed: Closed) -> NormalizedOutcome {
    check_normal_with_policy(word, engine_gate, closed)
}

fn resolved(outcome: &NormalizedOutcome) -> bool {
    !matches!(outcome, NormalizedOutcome::Unknown)
}

fn resolved_base(outcome: &NormalizedOutcome, surface: &str) -> Option<String> {
    match outcome {
        NormalizedOutcome::Lemma => Some(surface.to_string()),
        NormalizedOutcome::Inflected { base, .. } => Some(base.clone()),
        NormalizedOutcome::Unknown => None,
    }
}

#[test]
fn every_row_agrees_between_the_full_and_fast_paths() {
    let root = fixture();
    let rows = root["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 32, "the historical table has 32 rows");
    for row in rows {
        let surface = row["surface"].as_str().expect("surface");
        for closed in [Closed::Enabled, Closed::Disabled] {
            let full = outcome(surface, true, closed);
            let fast = outcome(surface, false, closed);
            assert_eq!(
                resolved_base(&full, surface),
                resolved_base(&fast, surface),
                "{surface} ({closed:?}): full and fast disagree"
            );
        }
    }
}

#[test]
fn closed_rows_are_lexical_and_stale_when_derived() {
    let root = fixture();
    let rows = root["rows"].as_array().expect("rows");
    let mut count = 0;
    for row in rows {
        if row["kind"].as_str() != Some("closed") {
            continue;
        }
        count += 1;
        let surface = row["surface"].as_str().expect("surface");
        let base = row["base"].as_str().expect("base");
        let enabled = outcome(surface, true, Closed::Enabled);
        assert_eq!(
            resolved_base(&enabled, surface).as_deref(),
            Some(base),
            "{surface}: the closed row does not resolve to {base}"
        );
        let disabled = outcome(surface, true, Closed::Disabled);
        assert!(
            !resolved(&disabled),
            "{surface} resolves without the closed table; the row is stale"
        );
    }
    assert_eq!(count, 6, "the kuulten family is the live residue");
}

#[test]
fn derived_rows_resolve_without_the_closed_table() {
    let root = fixture();
    for row in root["rows"].as_array().expect("rows") {
        if row["kind"].as_str() != Some("derived") {
            continue;
        }
        let surface = row["surface"].as_str().expect("surface");
        let disabled = outcome(surface, true, Closed::Disabled);
        assert!(
            resolved(&disabled),
            "{surface} no longer derives without the closed table: {disabled:?}"
        );
    }
}

#[test]
fn nonword_rows_are_rejected_in_production() {
    let root = fixture();
    for row in root["rows"].as_array().expect("rows") {
        if row["kind"].as_str() != Some("nonword") {
            continue;
        }
        let surface = row["surface"].as_str().expect("surface");
        let enabled = outcome(surface, true, Closed::Enabled);
        assert!(
            !resolved(&enabled),
            "{surface} is accepted in production: {enabled:?}"
        );
    }
}
