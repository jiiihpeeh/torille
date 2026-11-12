//! Data-driven expectations for the syntax correction checker.
//!
//! `tests/fixtures/syntax_diagnostics.json` holds the wrong sentences with the
//! exact diagnostic span and correction, plus a clean corpus that must stay
//! quiet. Each case is its own test so the morphology runs in parallel.

use serde_json::Value;
use torille::syntax_check::{self, DiagnosticKind, Severity};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/syntax_diagnostics.json"))
        .expect("tests/fixtures/syntax_diagnostics.json is not valid JSON")
}

fn case_by_id<'a>(root: &'a Value, id: &str) -> &'a Value {
    root["cases"]
        .as_array()
        .expect("cases must be an array")
        .iter()
        .find(|case| case["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("no case {id:?} in syntax_diagnostics.json"))
}

fn kind(name: &str) -> DiagnosticKind {
    [
        DiagnosticKind::SubjectPredicateAgreement,
        DiagnosticKind::AttributeAgreement,
        DiagnosticKind::VerbRection,
        DiagnosticKind::CaseSelection,
        DiagnosticKind::ObjectCase,
        DiagnosticKind::MissingArgument,
        DiagnosticKind::ExtraArgument,
        DiagnosticKind::ClauseStructure,
        DiagnosticKind::WordOrder,
        DiagnosticKind::CommaMissing,
        DiagnosticKind::CommaExtra,
        DiagnosticKind::CommaPairUnbalanced,
        DiagnosticKind::ListSeparatorMissing,
        DiagnosticKind::ListSeparatorDuplicate,
        DiagnosticKind::ListCommaBeforeConjunction,
        DiagnosticKind::ListIntroducerComma,
    ]
    .into_iter()
    .find(|kind| kind.name() == name)
    .unwrap_or_else(|| panic!("unknown diagnostic kind {name:?}"))
}

fn severity(name: &str) -> Severity {
    match name {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "suggestion" => Severity::Suggestion,
        other => panic!("unknown severity {other:?}"),
    }
}

fn run_case(id: &str) {
    let root = fixture();
    let case = case_by_id(&root, id);
    let text = case["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{id}: text must be a string"));
    let check = syntax_check::check(text);
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
fn the_cache_path_agrees_with_the_stateless_path() {
    let mut cache = torille::syntax::AnalysisCache::new();
    let text = "Koira nukkuvat. Minä menee kotiin.";
    let stateless = syntax_check::check(text);
    let cached = syntax_check::check_with_cache(text, &mut cache);
    assert_eq!(stateless.diagnostics, cached.diagnostics);
    assert_eq!(stateless.analysis, cached.analysis);
}

#[test]
fn the_clean_corpus_has_no_diagnostics() {
    let root = fixture();
    let clean = root["clean"].as_array().expect("clean must be an array");
    for text in clean {
        let text = text.as_str().expect("clean entries must be strings");
        let check = syntax_check::check(text);
        assert!(
            check.diagnostics.is_empty(),
            "clean sentence {text:?} produced diagnostics: {:#?}",
            check.diagnostics
        );
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
    sv_agreement_plural_verb,
    sv_agreement_first_singular,
    sv_agreement_third_plural,
    sv_agreement_plural_subject,
    sv_agreement_first_plural,
    sv_agreement_compound_auxiliary,
    sv_agreement_negative_auxiliary,
    sv_agreement_coordinated_subject,
    attribute_plural_head,
    attribute_singular_head,
    attribute_number_mismatch,
    attribute_demonstrative_plural,
    attribute_demonstrative_singular,
    attribute_case_mismatch,
    list_missing_separator,
    list_duplicate_separator,
    list_comma_before_conjunction,
    list_introducer_comma,
    subordinate_comma_after_main,
    subordinate_comma_initial,
    relative_comma_pair,
);
