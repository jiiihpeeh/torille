//! Audit harness for the programmatic Finnish semicolon specification.
//!
//! `tests/fixtures/semicolon_spec.json` records the FI-SEM-001 … FI-SEM-015
//! matrix, the phase that owns each row, and whether it is covered or still a
//! gap. Covered cases assert either a rule/span, the absence of a rule (the
//! Phase 1 boundary fix), or complete quiet. Gap cases are skipped so the
//! suite reports progress without pinning unimplemented behaviour.

use serde_json::Value;
use torille::syntax_check;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/semicolon_spec.json"))
        .expect("tests/fixtures/semicolon_spec.json is not valid JSON")
}

#[test]
fn covered_semicolon_cases_match_the_checker() {
    let root = fixture();
    let mut covered = 0;
    let mut gaps = 0;
    for case in root["cases"].as_array().expect("cases must be an array") {
        let id = case["id"].as_str().expect("case id");
        let text = case["text"].as_str().expect("case text");
        match case["status"].as_str().expect("case status") {
            "gap" => {
                gaps += 1;
                continue;
            }
            "covered" => covered += 1,
            other => panic!("case {id:?} has unknown status {other:?}"),
        }

        let check = syntax_check::check(text);
        let expect = &case["expect"];

        if let Some(rule) = expect["rule"].as_str() {
            let found = check
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.rule_id.as_str() == rule)
                .unwrap_or_else(|| {
                    panic!(
                        "case {id:?} ({text:?}) did not emit {rule:?}: {:#?}",
                        check.diagnostics
                    )
                });
            if let Some(start) = expect["start"].as_u64() {
                assert_eq!(
                    found.span.start, start as usize,
                    "case {id:?} ({text:?}) span start for {rule:?}"
                );
            }
        } else if let Some(absent) = expect["absent_rule"].as_str() {
            assert!(
                !check
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule_id.as_str() == absent),
                "case {id:?} ({text:?}) must not emit {absent:?}: {:#?}",
                check.diagnostics
            );
        } else {
            assert!(
                check.diagnostics.is_empty(),
                "case {id:?} ({text:?}) must stay quiet: {:#?}",
                check.diagnostics
            );
        }
    }

    // Phase 1 covers the valid main-clause rows and the boundary defect. Raise
    // this floor when Phase 2/3/4 land; the gap count is the remaining work.
    assert!(
        covered >= 13,
        "only {covered} covered semicolon-spec cases (gaps: {gaps})"
    );
}
