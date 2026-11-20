//! Audit harness for the programmatic Finnish punctuation specification.
//!
//! `tests/fixtures/punctuation_spec.json` records every rule group the
//! specification names, the phase that owns it, and whether it is covered or
//! still a gap. Covered cases assert the rule and insertion point; gap cases
//! are skipped so the suite reports progress without pinning an unimplemented
//! behaviour. The exact-output regression net stays
//! `tests/fixtures/syntax_diagnostics.json`.

use serde_json::Value;
use torille::syntax_check;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/punctuation_spec.json"))
        .expect("tests/fixtures/punctuation_spec.json is not valid JSON")
}

#[test]
fn covered_spec_cases_match_the_checker() {
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
        match case["expect"]["rule"].as_str() {
            Some(rule) => {
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
                if let Some(start) = case["expect"]["start"].as_u64() {
                    assert_eq!(
                        found.span.start, start as usize,
                        "case {id:?} ({text:?}) span start for {rule:?}"
                    );
                }
            }
            None => assert!(
                check.diagnostics.is_empty(),
                "case {id:?} ({text:?}) must stay quiet: {:#?}",
                check.diagnostics
            ),
        }
    }

    // The two implemented phases are the audit floor. Raise these numbers when
    // a later phase lands; the gap count is the remaining specification work.
    assert!(
        covered >= 22,
        "only {covered} covered punctuation-spec cases (gaps: {gaps})"
    );
}
