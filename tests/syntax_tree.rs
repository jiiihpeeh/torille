//! The surface tree over every syntax fixture.
//!
//! Phase 0 is structural: every fixture sentence must build a tree whose
//! document, sentence, clause and word-leaf skeleton satisfies the invariants
//! in `src/syntax/ast/validate.rs`. The test is data-driven for the same reason
//! the checker fixtures are: a new construction belongs in the fixture, not in
//! a hand-written list of strings.

use serde_json::Value;
use torille::syntax::ast::{self, ClauseKind, NodeKind};
use torille::tokenise::tokenise;

fn fixtures() -> Vec<(&'static str, Value)> {
    vec![
        (
            "syntax_cases",
            serde_json::from_str(include_str!("fixtures/syntax_cases.json"))
                .expect("syntax_cases.json is valid JSON"),
        ),
        (
            "syntax_ambiguity",
            serde_json::from_str(include_str!("fixtures/syntax_ambiguity.json"))
                .expect("syntax_ambiguity.json is valid JSON"),
        ),
        (
            "word_order_cases",
            serde_json::from_str(include_str!("fixtures/word_order_cases.json"))
                .expect("word_order_cases.json is valid JSON"),
        ),
    ]
}

fn texts(value: &Value) -> Vec<String> {
    value["cases"]
        .as_array()
        .expect("cases must be an array")
        .iter()
        .filter_map(|case| {
            case.get("sentence")
                .or_else(|| case.get("text"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

#[test]
fn every_fixture_sentence_builds_a_valid_tree() {
    let mut checked = 0;
    for (source, fixture) in fixtures() {
        for text in texts(&fixture) {
            let tokens = tokenise(&text);
            let tree = ast::analyze(&tokens);
            if let Err(violations) = ast::validate(&tokens, &tree.analysis, &tree.ast) {
                panic!("{source} / {text:?}: {violations:#?}");
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 100,
        "only {checked} fixture sentences were checked"
    );
}

#[test]
fn a_relative_clause_is_typed_and_nested() {
    let tokens = tokenise("Mies, joka tuli, söi.");
    let tree = ast::analyze(&tokens);
    let relative = tree
        .ast
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Clause(ClauseKind::Relative))
        .count();
    assert_eq!(relative, 1);
    let host = tree
        .ast
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::Clause(_)) && node.segments.len() == 2)
        .count();
    assert_eq!(host, 1, "the host should have two segments");
    ast::validate(&tokens, &tree.analysis, &tree.ast).expect("relative tree is valid");
}

#[test]
fn coordinated_clauses_keep_their_linear_shape() {
    let tokens = tokenise("Sami söi puuron ja joi maidon.");
    let tree = ast::analyze(&tokens);
    let clauses = tree
        .ast
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::Clause(_)))
        .count();
    assert_eq!(clauses, tree.analysis.clauses.len());
    assert_eq!(clauses, 2);
    ast::validate(&tokens, &tree.analysis, &tree.ast).expect("coordinated tree is valid");
}
