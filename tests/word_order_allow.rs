//! The word-order `ALLOW` regression net.
//!
//! [`docs/word_order.md`](../docs/word_order.md) says `PREFER` is a generation
//! preference and `ALLOW` is silence. Before any rule is allowed to look at
//! order, the sentences that are *correct* must be proven clean: object,
//! topic, temporal and locative fronting, contrastive focus, passives and
//! existentials. The same fixture pins the clause features the analysis must
//! produce for each one, so a change to the feature model is caught here rather
//! than by a later rule producing a false positive.
//!
//! `tests/fixtures/word_order_cases.json` carries the examples;
//! `tests/fixtures/word_order_rules.json` is the rule list they belong to.

use std::collections::BTreeSet;

use serde_json::Value;
use torille::syntax_check::{self, DiagnosticKind};

fn cases() -> Value {
    serde_json::from_str(include_str!("fixtures/word_order_cases.json"))
        .expect("tests/fixtures/word_order_cases.json is not valid JSON")
}

fn rules() -> Value {
    serde_json::from_str(include_str!("fixtures/word_order_rules.json"))
        .expect("tests/fixtures/word_order_rules.json is not valid JSON")
}

/// Every rule id in the machine-readable specification.
fn rule_ids() -> BTreeSet<String> {
    rules()["rules"]
        .as_array()
        .expect("rules must be an array")
        .iter()
        .map(|rule| rule["id"].as_str().expect("rule id").to_string())
        .collect()
}

/// Every sentence the specification lists, preferred and allowed.
fn spec_examples() -> BTreeSet<String> {
    let mut examples = BTreeSet::new();
    for rule in rules()["rules"].as_array().expect("rules") {
        for side in ["preferred", "allowed"] {
            for example in rule["examples"][side].as_array().expect("examples") {
                examples.insert(example.as_str().expect("example").to_string());
            }
        }
    }
    examples
}

#[test]
fn the_net_covers_every_example_in_the_specification() {
    let covered: BTreeSet<String> = cases()["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| case["text"].as_str().expect("text").to_string())
        .collect();
    let examples = spec_examples();
    let missing: Vec<&String> = examples
        .iter()
        .filter(|example| !covered.contains(*example))
        .collect();
    assert!(
        missing.is_empty(),
        "tests/fixtures/word_order_cases.json does not cover: {missing:?}"
    );
}

#[test]
fn every_case_names_a_rule_and_the_side_of_its_examples() {
    let ids = rule_ids();
    let specification = rules();
    for case in cases()["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("case id");
        let rule = case["rule"]
            .as_str()
            .unwrap_or_else(|| panic!("{id}: rule"));
        assert!(ids.contains(rule), "{id}: unknown rule {rule}");
        let outcome = case["outcome"]
            .as_str()
            .unwrap_or_else(|| panic!("{id}: outcome"));
        assert!(
            matches!(outcome, "preferred" | "allowed"),
            "{id}: unknown outcome {outcome}"
        );

        // The case must be an example of the rule it names, on the side it
        // claims. This is what keeps the net tied to the specification rather
        // than to a copy of it.
        let entry = specification["rules"]
            .as_array()
            .expect("rules")
            .iter()
            .find(|entry| entry["id"] == rule)
            .expect("the rule exists");
        let listed: Vec<&str> = entry["examples"][outcome]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: {rule} has no {outcome} examples"))
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let text = case["text"].as_str().expect("case text");
        assert!(
            listed.contains(&text),
            "{id}: {text:?} is not a {outcome} example of {rule}"
        );
    }
}

#[test]
fn word_order_is_never_a_diagnostic() {
    for case in cases()["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("case id");
        let text = case["text"].as_str().expect("case text");
        let check = syntax_check::check(text);
        assert!(
            check
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.kind != DiagnosticKind::WordOrder),
            "{id}: word order must stay suggestion-only and unemitted"
        );
    }
}

#[test]
fn allow_and_preferred_examples_are_clean_and_carry_their_features() {
    let mut failures = Vec::new();

    for case in cases()["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("case id");
        let text = case["text"].as_str().expect("case text");
        let check = syntax_check::check(text);

        if let Some(expected) = case["diagnostics"].as_array() {
            let mut expected: Vec<&str> = expected.iter().filter_map(Value::as_str).collect();
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

        let index = case["clause"].as_u64().unwrap_or(0) as usize;
        let Some(clause) = check.analysis.clauses.get(index) else {
            failures.push(format!("{id}: no clause {index}"));
            continue;
        };
        let Some(features) = case["features"].as_object() else {
            failures.push(format!("{id}: features must be an object"));
            continue;
        };
        for (name, expected) in features {
            let expected = expected
                .as_str()
                .unwrap_or_else(|| panic!("{id}: {name} must be a string"));
            match clause.features.value(name) {
                Some(actual) if actual == expected => {}
                Some(actual) => {
                    failures.push(format!("{id}: {name} is {actual:?}, expected {expected:?}"))
                }
                None => failures.push(format!("{id}: {name} is absent, expected {expected:?}")),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "word-order regression net:\n{}",
        failures.join("\n")
    );
}
