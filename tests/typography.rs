//! Data-driven expectations for the separate typography pass.
//!
//! `tests/fixtures/typography.json` holds the semicolon spacing edits, their
//! whole-text result and the protected contexts that must stay untouched.
//! Typography never enters `syntax_check`; a UI calls it behind its own toggle.

use serde_json::Value;
use torille::typography;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/typography.json"))
        .expect("tests/fixtures/typography.json is not valid JSON")
}

#[test]
fn typography_cases_match_their_expected_edits() {
    let root = fixture();
    for case in root["cases"].as_array().expect("cases must be an array") {
        let id = case["id"].as_str().expect("case id");
        let text = case["text"].as_str().expect("case text");
        let fixed = case["fixed"].as_str().expect("case fixed text");
        let expected = case["diagnostics"]
            .as_array()
            .expect("diagnostics must be an array");

        let diagnostics = typography::check(text);
        assert_eq!(
            diagnostics.len(),
            expected.len(),
            "case {id:?}: diagnostics: {diagnostics:#?}"
        );
        for (index, expected) in expected.iter().enumerate() {
            let actual = &diagnostics[index];
            let label = format!("case {id:?} diagnostic {index}");
            assert_eq!(
                actual.rule.name(),
                expected["rule"].as_str().expect("rule"),
                "{label}: rule"
            );
            assert_eq!(
                actual.span.start,
                expected["start"].as_u64().expect("start") as usize,
                "{label}: start"
            );
            assert_eq!(
                actual.span.end,
                expected["end"].as_u64().expect("end") as usize,
                "{label}: end"
            );
            assert_eq!(
                actual.replacement,
                expected["replacement"].as_str().expect("replacement"),
                "{label}: replacement"
            );
        }
        assert_eq!(typography::apply(text, &diagnostics), fixed, "case {id:?}");
    }
}

#[test]
fn numeric_runs_are_quiet() {
    let root = fixture();
    for text in root["clean"].as_array().expect("clean must be an array") {
        let text = text.as_str().expect("clean entries must be strings");
        assert!(
            typography::check(text).is_empty(),
            "clean input {text:?} produced diagnostics: {:#?}",
            typography::check(text)
        );
    }
}

#[test]
fn typography_never_reaches_the_grammar_checker() {
    let text = "sana ; toinen";
    assert!(torille::syntax_check::check(text).diagnostics.is_empty());
    assert!(!typography::check(text).is_empty());
}
