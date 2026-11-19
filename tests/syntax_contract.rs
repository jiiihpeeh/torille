//! The public syntax and checker contract.
//!
//! These tests pin the distinctions the feature reference relies on:
//! morphological case vs the syntactic object feature, all-token indices,
//! half-open byte spans, and deterministic diagnostic ordering. They are
//! intentionally small; behavioural coverage is in `syntax_cases.json`,
//! `syntax_diagnostics.json` and the prose gate.

use torille::syntax::{self, ObjectCase, SyntacticRole};
use torille::syntax_check;
use torille::tags::Case;
use torille::tokenise::tokenise;

#[test]
fn token_indices_cover_the_whole_token_stream() {
    let tokens = tokenise("Talo on kaunis.");
    let analysis = syntax::analyze(&tokens);
    assert_eq!(analysis.tokens.len(), tokens.len());
    for (index, token) in tokens.iter().enumerate() {
        let syntax = &analysis.tokens[index];
        if token.is_word() {
            assert_eq!(syntax.key, token.text.to_lowercase());
        } else {
            assert!(syntax.key.is_empty(), "separator {index} has a key");
        }
        assert!(token.start <= token.end);
        assert!(token.end <= "Talo on kaunis.".len());
    }
}

#[test]
fn clause_ranges_and_spans_are_half_open_word_envelopes() {
    let tokens = tokenise("Talo on kaunis. Koira nukkuu.");
    let analysis = syntax::analyze(&tokens);
    for clause in &analysis.clauses {
        assert!(clause.start <= clause.end);
        assert!(clause.tokens.start <= clause.tokens.end);
        for index in clause.tokens.clone() {
            let token = &tokens[index];
            assert!(token.start >= clause.start);
            assert!(token.end <= clause.end);
        }
        let Some(&first) = clause
            .tokens
            .clone()
            .find(|&index| tokens[index].is_word())
            .as_ref()
        else {
            continue;
        };
        assert!(tokens[first].start >= clause.start);
    }
}

#[test]
fn morphological_case_and_object_case_are_distinct_levels() {
    let tokens = tokenise("Näen poikaa.");
    let analysis = syntax::analyze(&tokens);
    let object = tokens
        .iter()
        .position(|token| token.text == "poikaa")
        .expect("poikaa");
    assert_eq!(analysis.tokens[object].role, SyntacticRole::Object);
    assert_eq!(analysis.tokens[object].case, Some(Case::Partitive));
    assert_eq!(
        analysis.clauses[0].features.object_case,
        Some(ObjectCase::Partitive)
    );
}

#[test]
fn a_total_object_keeps_its_morphological_reading() {
    let tokens = tokenise("Näen pojan.");
    let analysis = syntax::analyze(&tokens);
    let object = tokens
        .iter()
        .position(|token| token.text == "pojan")
        .expect("pojan");
    assert_eq!(analysis.tokens[object].role, SyntacticRole::Object);
    assert!(
        matches!(
            analysis.tokens[object].case,
            Some(Case::Genitive | Case::Accusative)
        ),
        "total object must keep a genitive/accusative reading: {:?}",
        analysis.tokens[object].case
    );
    assert!(
        matches!(
            analysis.clauses[0].features.object_case,
            Some(ObjectCase::Genitive | ObjectCase::Accusative)
        ),
        "the object feature follows the resolved reading"
    );
}

#[test]
fn an_existential_and_a_necessive_have_distinct_subject_kinds() {
    let tokens = tokenise("Pöydällä on kirja.");
    let analysis = syntax::analyze(&tokens);
    assert_eq!(
        analysis.clauses[0].features.construction,
        syntax::Construction::Existential
    );
    assert_eq!(
        analysis.clauses[0].features.subject_kind,
        syntax::SubjectKind::Existential
    );

    let tokens = tokenise("Minun täytyy mennä.");
    let analysis = syntax::analyze(&tokens);
    assert_eq!(
        analysis.clauses[0].features.construction,
        syntax::Construction::Necessive
    );
    assert_eq!(
        analysis.clauses[0].features.subject_kind,
        syntax::SubjectKind::GenitiveNecessive
    );
}

#[test]
fn diagnostics_are_deterministic() {
    let text = "Koira nukkuvat. Minä, menen kotiin.";
    let first = syntax_check::check(text);
    let second = syntax_check::check(text);
    let key = |diagnostics: &[syntax_check::SyntaxDiagnostic]| {
        diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.kind,
                    diagnostic.rule_id.as_str().to_string(),
                    diagnostic.severity,
                    diagnostic.confidence,
                    diagnostic.span.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(key(&first.diagnostics), key(&second.diagnostics));
}

/// Every diagnostic emitted by the fixtures is listed in the public
/// DiagnosticKind -> RuleId contract, and every fixture-backed rule is
/// represented.
#[test]
fn every_fixture_diagnostic_has_a_contract_entry() {
    const CONTRACT: &[(&str, &str)] = &[
        ("subject_predicate_agreement", "subject_predicate_agreement"),
        ("subject_predicate_agreement", "participle_agreement"),
        ("connegative", "connegative"),
        ("attribute_agreement", "attribute_agreement"),
        ("predicative_agreement", "predicative_agreement"),
        ("predicative_agreement", "predicative_noun_agreement"),
        ("possessive_agreement", "possessive_agreement"),
        ("possessive_agreement", "possessive_missing"),
        ("verb_rection", "verb_rection"),
        ("case_selection", "numeral_case"),
        ("case_selection", "adposition_case"),
        ("case_selection", "existential_subject_case"),
        ("object_case", "object_case"),
        ("object_case", "bare_nominative_object"),
        ("missing_argument", "missing_argument"),
        ("extra_argument", "extra_argument"),
        ("clause_structure", "incomplete_coordination"),
        ("clause_structure", "missing_predicate"),
        ("comma_missing", "comma_missing"),
        ("comma_missing", "parenthetical_comma"),
        ("comma_extra", "comma_extra"),
        ("comma_pair_unbalanced", "comma_pair_unbalanced"),
        ("question_mark_missing", "question_mark_missing"),
        ("unbalanced_quote", "unbalanced_quote"),
        ("list_separator_missing", "list_separator_missing"),
        ("list_separator_duplicate", "list_separator_duplicate"),
        (
            "list_comma_before_conjunction",
            "list_comma_before_conjunction",
        ),
        ("list_introducer_comma", "list_introducer_comma"),
    ];

    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/syntax_diagnostics.json"))
            .expect("syntax_diagnostics.json");
    let mut emitted = std::collections::BTreeSet::new();
    for case in fixture["cases"].as_array().expect("cases") {
        let text = case["text"].as_str().expect("case text");
        for diagnostic in syntax_check::check(text).diagnostics {
            let pair = (
                diagnostic.kind.name().to_string(),
                diagnostic.rule_id.as_str().to_string(),
            );
            assert!(
                CONTRACT.contains(&(pair.0.as_str(), pair.1.as_str())),
                "unmapped diagnostic: {pair:?}"
            );
            emitted.insert(pair);
        }
    }
    for (kind, rule) in CONTRACT {
        assert!(
            emitted.contains(&(kind.to_string(), rule.to_string())),
            "fixture does not exercise {kind} / {rule}"
        );
    }
}
