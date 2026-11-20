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

#[test]
fn a_niin_kauan_kuin_comparison_is_not_a_missing_comma() {
    let text = "Sanoi täällä entinen rouvani, kun kuuntelin kerta oven takana, \
                että suku on rakas niin kauan kuin kaikki ovat yhtä köyhiä.";
    let check = syntax_check::check(text);
    assert!(
        !check
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::CommaMissing),
        "{:?}",
        check.diagnostics
    );
}

fn kind(name: &str) -> DiagnosticKind {
    [
        DiagnosticKind::SubjectPredicateAgreement,
        DiagnosticKind::Connegative,
        DiagnosticKind::AttributeAgreement,
        DiagnosticKind::PredicativeAgreement,
        DiagnosticKind::PossessiveAgreement,
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
        DiagnosticKind::SemicolonMisuse,
        DiagnosticKind::QuestionMarkMissing,
        DiagnosticKind::UnbalancedQuote,
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

/// `koko päivä` is a valid subject (`Koko päivä meni hukkaan.`), not a
/// frequency adverbial like `joka päivä`. A first-person predicate with a
/// nominal subject is a deliberate false negative, so the wrong 3sg `juoksi`
/// must not be offered.
#[test]
fn koko_temporal_noun_does_not_offer_a_third_person_verb() {
    let check = syntax_check::check("Koko päivä juoksin.");
    assert!(check.diagnostics.is_empty(), "{:#?}", check.diagnostics);
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
    nested_list_missing_separator,
    nested_list_duplicate_separator,
    subordinate_comma_after_main,
    subordinate_comma_initial,
    fused_negative_subordinator_comma,
    adposition_case_postposition,
    adposition_case_preposition,
    existential_subject_genitive,
    existential_subject_plural_genitive,
    participle_plural_subject,
    participle_singular_subject,
    participle_third_singular,
    participle_negative,
    bare_nominative_object,
    missing_predicate,
    incomplete_coordination,
    possessive_wrong_suffix,
    possessive_third_person,
    possessive_missing,
    extra_comma_subject,
    attribute_degree_number,
    predicative_noun_plural_subject,
    predicative_noun_singular_subject,
    participle_vasynyt,
    relative_comma_pair,
    rection_elative,
    rection_illative,
    rection_partitive,
    rection_weak_verb,
    object_case_negative,
    object_case_negative_subject,
    negative_rection_wins,
    missing_argument,
    extra_argument,
    aspect_progressive_suggestion,
    aspect_completive_suggestion,
    aspect_completive_plural,
    kuten_comma_pair,
    question_mark_missing,
    question_mark_ko,
    question_mark_quoted,
    unbalanced_quote,
    parenthetical_comma,
    semicolon_before_initial_subordinate,
    semicolon_before_subordinate_etta,
    semicolon_before_causal_subordinate,
    semicolon_relative_clause,
    semicolon_coordination,
    semicolon_phrase_object,
    semicolon_phrase_complement,
    semicolon_subject_predicate,
    semicolon_terminal,
    semicolon_between_main_clauses,
    semicolon_elliptical_clause_is_quiet,
    semicolon_afterthought_is_quiet,
    semicolon_inside_parenthetical,
    semicolon_simple_list,
    semicolon_two_item_list_is_quiet,
    semicolon_colon_simple_list,
    semicolon_complex_list_is_quiet,
    semicolon_complex_list_of_names_is_quiet,
    semicolon_complex_enumeration_is_quiet,
    predicative_comma_list_still_checked,
);
