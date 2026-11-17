//! Intended vs competing readings for ambiguous Finnish surfaces.
//!
//! `tests/fixtures/syntax_ambiguity.json` pins the role that must win and, for
//! the same surface, the role that must not win. A new role rule is not
//! complete until it has both halves: `joka päivä` is a frequency adverbial,
//! but `joka mies` is a subject; `sen` is an object in `Tarvitsen sen`, but an
//! adverb elsewhere.
//!
//! The fixture also carries the expected `syntax_check` kind list, so a clean
//! false-positive case is pinned in the same place as the role reading.

use serde_json::Value;
use torille::syntax::{SyntacticRole, SyntaxAnalysis};
use torille::syntax_check;
use torille::tags::Case;
use torille::tokenise::{tokenise, Token};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/syntax_ambiguity.json"))
        .expect("tests/fixtures/syntax_ambiguity.json is not valid JSON")
}

fn role(name: &str) -> SyntacticRole {
    [
        SyntacticRole::Predicate,
        SyntacticRole::Auxiliary,
        SyntacticRole::Subject,
        SyntacticRole::Object,
        SyntacticRole::Adverbial,
        SyntacticRole::Complement,
        SyntacticRole::Modifier,
        SyntacticRole::Unknown,
    ]
    .into_iter()
    .find(|role| role.name() == name)
    .unwrap_or_else(|| panic!("unknown role {name:?}"))
}

fn case(name: &str) -> Case {
    Case::ALL
        .into_iter()
        .find(|case| case.name() == name)
        .unwrap_or_else(|| panic!("unknown case {name:?}"))
}

fn word_index(tokens: &[Token], word: &str) -> Option<usize> {
    tokens
        .iter()
        .position(|token| token.is_word() && token.text.eq_ignore_ascii_case(word))
}

fn check_role(
    id: &str,
    spec: &Value,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    want_equal: bool,
    failures: &mut Vec<String>,
) {
    let word = spec["word"]
        .as_str()
        .unwrap_or_else(|| panic!("{id}: role spec needs a word"));
    let expected = role(
        spec["role"]
            .as_str()
            .unwrap_or_else(|| panic!("{id}: role spec needs a role")),
    );
    let Some(index) = word_index(tokens, word) else {
        failures.push(format!("{id}: {word:?} is not a word token"));
        return;
    };
    let actual = analysis.tokens[index].role;
    if (actual == expected) != want_equal {
        let relation = if want_equal { "equal to" } else { "different from" };
        failures.push(format!(
            "{id}: {word:?} role is {actual:?}, expected {relation} {expected:?}"
        ));
    }
    if let Some(expected) = spec["case"].as_str().map(case) {
        let actual = analysis.tokens[index].case;
        if (actual == Some(expected)) != want_equal {
            let relation = if want_equal { "equal to" } else { "different from" };
            failures.push(format!(
                "{id}: {word:?} case is {actual:?}, expected {relation} {expected:?}"
            ));
        }
    }
}

fn check_subject(id: &str, expected: &str, check: &syntax_check::SyntaxCheck, tokens: &[Token]) {
    let actual = check.analysis.clauses.first().and_then(|clause| clause.subject);
    if expected == "none" {
        assert!(
            actual.is_none(),
            "{id}: expected no subject, got {:?}",
            actual.map(|index| &tokens[index].text)
        );
        return;
    }
    let index = word_index(tokens, expected)
        .unwrap_or_else(|| panic!("{id}: subject {expected:?} is not a word token"));
    assert_eq!(
        actual,
        Some(index),
        "{id}: expected {expected:?} as the subject"
    );
}

fn check_predicate(id: &str, expected: &str, check: &syntax_check::SyntaxCheck, tokens: &[Token]) {
    let actual = check.analysis.clauses.first().and_then(|clause| clause.predicate);
    let index = word_index(tokens, expected)
        .unwrap_or_else(|| panic!("{id}: predicate {expected:?} is not a word token"));
    assert_eq!(
        actual,
        Some(index),
        "{id}: expected {expected:?} as the predicate"
    );
}

#[test]
fn ambiguous_surfaces_pick_the_intended_reading() {
    let root = fixture();
    let cases = root["cases"].as_array().expect("cases must be an array");
    let mut failures = Vec::new();

    for case_value in cases {
        let id = case_value["id"].as_str().expect("case id must be a string");
        let text = case_value["text"]
            .as_str()
            .expect("case text must be a string");
        let tokens = tokenise(text);
        let check = syntax_check::check(text);
        let analysis = &check.analysis;

        if let Some(expected) = case_value["clauses"].as_u64()
            && analysis.clauses.len() != expected as usize
        {
            failures.push(format!(
                "{id}: expected {expected} clauses, got {}",
                analysis.clauses.len()
            ));
        }
        if let Some(expected) = case_value["subject"].as_str() {
            check_subject(id, expected, &check, &tokens);
        }
        if let Some(expected) = case_value["predicate"].as_str() {
            check_predicate(id, expected, &check, &tokens);
        }
        if let Some(roles) = case_value["roles"].as_array() {
            for spec in roles {
                check_role(id, spec, &tokens, analysis, true, &mut failures);
            }
        }
        if let Some(roles) = case_value["forbidden_roles"].as_array() {
            for spec in roles {
                check_role(id, spec, &tokens, analysis, false, &mut failures);
            }
        }
        if let Some(expected) = case_value["diagnostics"].as_array() {
            let mut expected: Vec<&str> = expected
                .iter()
                .filter_map(|value| value.as_str())
                .collect();
            let mut actual: Vec<&str> = check
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.kind.name())
                .collect();
            expected.sort_unstable();
            actual.sort_unstable();
            if expected != actual {
                failures.push(format!(
                    "{id}: diagnostics expected {expected:?}, got {actual:?}"
                ));
            }
        }
        if let Some(features) = case_value["features"].as_object() {
            let Some(clause) = analysis.clauses.first() else {
                failures.push(format!("{id}: no clause to pin features on"));
                continue;
            };
            for (name, expected) in features {
                let expected = expected.as_str().unwrap_or_default();
                match clause.features.value(name) {
                    Some(actual) if actual == expected => {}
                    Some(actual) => failures.push(format!(
                        "{id}: feature {name} is {actual:?}, expected {expected:?}"
                    )),
                    None => failures.push(format!(
                        "{id}: feature {name} is absent, expected {expected:?}"
                    )),
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "ambiguity regressions:\n{}",
        failures.join("\n")
    );
}
