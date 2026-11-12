//! Data-driven expectations for the opt-in style layer.
//!
//! `tests/fixtures/style_diagnostics.json` holds the exact kind, span,
//! expected phrase and correction for each rule case, plus a clean corpus
//! that must stay quiet. Every finding is a `Suggestion`.

use serde_json::Value;
use torille::style_check::{self, StyleKind};
use torille::syntax_check::Severity;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/style_diagnostics.json"))
        .expect("tests/fixtures/style_diagnostics.json is not valid JSON")
}

fn case_by_id<'a>(root: &'a Value, id: &str) -> &'a Value {
    root["cases"]
        .as_array()
        .expect("cases must be an array")
        .iter()
        .find(|case| case["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("no case {id:?} in style_diagnostics.json"))
}

fn kind(name: &str) -> StyleKind {
    [
        StyleKind::Repetition,
        StyleKind::LongSentence,
        StyleKind::Cliche,
        StyleKind::Filler,
        StyleKind::Wordy,
        StyleKind::Colloquial,
    ]
    .into_iter()
    .find(|kind| kind.name() == name)
    .unwrap_or_else(|| panic!("unknown style kind {name:?}"))
}

fn severity(name: &str) -> Severity {
    match name {
        "suggestion" => Severity::Suggestion,
        other => panic!("style severity must be `suggestion`, got {other:?}"),
    }
}

fn run_case(id: &str) {
    let root = fixture();
    let case = case_by_id(&root, id);
    let text = case["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{id}: text must be a string"));
    let check = style_check::check(text);
    let expected = case["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{id}: diagnostics must be an array"));

    let actual = &check.diagnostics;
    assert_eq!(
        actual.len(),
        expected.len(),
        "case {id:?} ({text:?}) diagnostic count: {actual:#?}"
    );
    for (index, expected) in expected.iter().enumerate() {
        let actual = &actual[index];
        let label = format!("case {id:?} diagnostic {index}");
        assert_eq!(
            actual.kind,
            kind(expected["kind"].as_str().unwrap()),
            "{label}: kind"
        );
        assert_eq!(
            actual.severity,
            severity(expected["severity"].as_str().unwrap()),
            "{label}: severity"
        );
        assert_eq!(
            actual.span.start,
            expected["start"].as_u64().unwrap() as usize,
            "{label}: start"
        );
        assert_eq!(
            actual.span.end,
            expected["end"].as_u64().unwrap() as usize,
            "{label}: end"
        );
        assert_eq!(
            actual.message,
            expected["message"].as_str().unwrap(),
            "{label}: message"
        );
        assert_eq!(
            actual.expected.as_deref(),
            expected["expected"].as_str(),
            "{label}: expected"
        );
        let wanted: Vec<String> = expected["corrections"]
            .as_array()
            .expect("corrections must be an array")
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect();
        let got: Vec<String> = actual
            .corrections
            .iter()
            .map(|correction| {
                correction
                    .edits
                    .first()
                    .map(|edit| edit.replacement.clone())
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(got, wanted, "{label}: corrections");
    }
}

#[test]
fn the_clean_corpus_has_no_diagnostics() {
    let root = fixture();
    let clean = root["clean"].as_array().expect("clean must be an array");
    for text in clean {
        let text = text.as_str().expect("clean entries must be strings");
        let check = style_check::check(text);
        assert!(
            check.diagnostics.is_empty(),
            "clean sentence {text:?} produced diagnostics: {:#?}",
            check.diagnostics
        );
    }
}

#[test]
fn every_fixture_diagnostic_is_a_suggestion() {
    let root = fixture();
    for case in root["cases"].as_array().expect("cases must be an array") {
        let text = case["text"].as_str().unwrap();
        for diagnostic in style_check::check(text).diagnostics {
            assert_eq!(diagnostic.severity, Severity::Suggestion);
        }
    }
}

macro_rules! diagnostic_tests {
    ($($test:ident),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                run_case(stringify!($test));
            }
        )*
    };
}

diagnostic_tests!(
    phrase_wordy,
    phrase_cliche_without_replacement,
    word_colloquial,
    word_filler,
    word_wordy,
    a_word_does_not_match_inside_a_longer_word,
    a_phrase_does_not_cross_a_sentence_boundary,
    a_phrase_is_reported_once,
);
