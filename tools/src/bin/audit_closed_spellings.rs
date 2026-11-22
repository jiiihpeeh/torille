// SPDX-License-Identifier: MIT OR Apache-2.0
//! `audit_closed_spellings` — the historical `CLOSED` rows against the engine.
//!
//! Reads `tests/fixtures/closed_spellings.json` and prints one line per row
//! with the current evidence. Exits 1 when an invariant breaks: a `closed` row
//! that no longer resolves (or resolves with the table disabled), a `derived`
//! row that stops deriving, a `nonword` row that production accepts, or a
//! full/fast disagreement.
//!
//! Usage:
//!   audit_closed_spellings [--file PATH]

use std::path::PathBuf;

use torille::expand::check_normal::{NormalizedOutcome, check_normal_with_policy};
use torille::expand::closed_spellings::Closed;
use torille::expand::find_bases_with;
use torille::kotus::{lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::PartOfSpeech;

const DEFAULT_FIXTURE: &str = "tests/fixtures/closed_spellings.json";

fn outcome_tag(outcome: &NormalizedOutcome, surface: &str) -> String {
    match outcome {
        NormalizedOutcome::Lemma => format!("lemma:{surface}"),
        NormalizedOutcome::Inflected { base, .. } => format!("inflected:{base}"),
        NormalizedOutcome::Unknown => "unknown".to_string(),
    }
}

fn resolved(outcome: &NormalizedOutcome) -> bool {
    !matches!(outcome, NormalizedOutcome::Unknown)
}

fn main() {
    let mut path = PathBuf::from(DEFAULT_FIXTURE);
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--file" => path = PathBuf::from(args.next().expect("--file needs a path")),
            other => panic!("unknown argument: {other}"),
        }
    }
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let root: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not valid JSON: {error}", path.display()));
    let rows = root["rows"].as_array().expect("rows must be an array");
    let catalogues = Catalogues::embedded();
    let lexicon = lemmas();

    println!(
        "surface\tbase\tkind\tbase_in_sanalista\tengine_emits\tfind_bases_base\tenabled_full\tenabled_fast\tdisabled_full\tdisabled_fast\tproper_name"
    );
    let mut failures = 0;
    for row in rows {
        let surface = row["surface"].as_str().expect("surface");
        let base = row["base"].as_str().expect("base");
        let kind = row["kind"].as_str().expect("kind");
        let base_row = sanalista().by_word(base).next();
        let base_in_sanalista = base_row.is_some();
        let engine_emits = base_row.is_some_and(|row| {
            let pos = PartOfSpeech::inflecting_reading(row.part_of_speech());
            row.inflections().any(|inflection| {
                torille::expand::inflect(base, inflection, catalogues, pos, true)
                    .iter()
                    .any(|form| form.word == surface)
            })
        });
        let report = find_bases_with(lexicon, catalogues, surface);
        let find_bases_base = report.bases.iter().any(|lemma| lemma.word == base)
            || report.direct.iter().any(|lemma| lemma.word() == base)
            || engine_emits;
        let enabled_full = check_normal_with_policy(surface, true, Closed::Enabled);
        let enabled_fast = check_normal_with_policy(surface, false, Closed::Enabled);
        let disabled_full = check_normal_with_policy(surface, true, Closed::Disabled);
        let disabled_fast = check_normal_with_policy(surface, false, Closed::Disabled);
        let proper_name = torille::expand::proper_names::contains_capitalised(surface);
        println!(
            "{surface}\t{base}\t{kind}\t{base_in_sanalista}\t{engine_emits}\t{find_bases_base}\t{}\t{}\t{}\t{}\t{proper_name}",
            outcome_tag(&enabled_full, surface),
            outcome_tag(&enabled_fast, surface),
            outcome_tag(&disabled_full, surface),
            outcome_tag(&disabled_fast, surface),
        );

        if resolved(&enabled_full) != resolved(&enabled_fast)
            || outcome_tag(&enabled_full, surface) != outcome_tag(&enabled_fast, surface)
            || resolved(&disabled_full) != resolved(&disabled_fast)
            || outcome_tag(&disabled_full, surface) != outcome_tag(&disabled_fast, surface)
        {
            eprintln!("FAIL {surface}: full and fast paths disagree");
            failures += 1;
        }
        match kind {
            "closed" => {
                if !resolved(&enabled_full) {
                    eprintln!("FAIL {surface}: the live closed row no longer resolves");
                    failures += 1;
                }
                if resolved(&disabled_full) {
                    eprintln!("FAIL {surface}: resolves without the closed table (stale row)");
                    failures += 1;
                }
            }
            "derived" => {
                if !resolved(&disabled_full) {
                    eprintln!("FAIL {surface}: no longer derives without the closed table");
                    failures += 1;
                }
            }
            "nonword" => {
                if resolved(&enabled_full) {
                    eprintln!("FAIL {surface}: production accepts it");
                    failures += 1;
                }
            }
            other => panic!("{surface}: unknown kind {other:?}"),
        }
    }
    if failures > 0 {
        eprintln!("FAIL {failures} invariant(s)");
        std::process::exit(1);
    }
    println!("# {} rows, all invariants hold", rows.len());
}
