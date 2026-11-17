//! Schema and stability checks for the word-order specification.
//!
//! The rule engine is not implemented yet. This gate pins the machine-readable
//! contract before a rule starts depending on it: ids are unique, decisions are
//! one of the three allowed values, and every rule carries examples and
//! exceptions. The prose spec must name every rule id.

use std::collections::HashSet;

use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/word_order_rules.json"))
        .expect("tests/fixtures/word_order_rules.json is not valid JSON")
}

#[test]
fn the_spec_has_unique_stable_rule_ids() {
    let root = fixture();
    let rules = root["rules"]
        .as_array()
        .expect("rules must be an array");
    assert!(!rules.is_empty());
    let mut ids = HashSet::new();
    for rule in rules {
        let id = rule["id"].as_str().expect("rule id must be a string");
        assert!(ids.insert(id.to_string()), "duplicate rule id {id}");
        assert!(id.starts_with("FI_"), "unstable id {id}");
    }
}

#[test]
fn every_decision_is_force_prefer_or_allow() {
    let root = fixture();
    let decisions: Vec<&str> = root["decisions"]
        .as_array()
        .expect("decisions must be an array")
        .iter()
        .map(|value| value.as_str().expect("decision must be a string"))
        .collect();
    assert_eq!(decisions, ["FORCE", "PREFER", "ALLOW"]);

    for rule in root["rules"].as_array().expect("rules") {
        let decision = rule["decision"].as_str().expect("decision");
        assert!(
            decisions.contains(&decision),
            "{}: unknown decision {decision}",
            rule["id"]
        );
        assert_eq!(
            rule["hard_constraint"].as_bool(),
            Some(decision == "FORCE"),
            "{}: hard_constraint must be true only for FORCE",
            rule["id"]
        );
        assert!(
            rule["exceptions"].is_array(),
            "{}: exceptions must be an array",
            rule["id"]
        );
        let examples = &rule["examples"];
        let preferred = examples["preferred"].as_array().expect("preferred");
        let allowed = examples["allowed"].as_array().expect("allowed");
        assert!(
            !preferred.is_empty() || !allowed.is_empty(),
            "{}: needs at least one example",
            rule["id"]
        );
    }
}

#[test]
fn the_spec_document_names_every_rule() {
    let root = fixture();
    let doc = include_str!("../docs/word_order.md");
    for rule in root["rules"].as_array().expect("rules") {
        let id = rule["id"].as_str().expect("rule id");
        assert!(doc.contains(id), "docs/word_order.md does not name {id}");
    }
}

#[test]
fn the_allow_table_covers_the_information_structure_cases() {
    let root = fixture();
    let allow = root["rules"]
        .as_array()
        .expect("rules")
        .iter()
        .find(|rule| rule["id"] == "FI_ALLOW_001")
        .expect("FI_ALLOW_001 exists");
    let allowed: Vec<&str> = allow["examples"]["allowed"]
        .as_array()
        .expect("allowed")
        .iter()
        .map(|value| value.as_str().expect("example"))
        .collect();
    for example in [
        "Kirjan luin eilen.",
        "Minä en sitä tiedä.",
        "Eilen kävin kaupassa.",
        "Kotona hän viihtyy hyvin.",
        "JUURI HÄN teki sen.",
        "Asia käsiteltiin eilen.",
        "Pöydällä on kahvikuppi.",
    ] {
        assert!(allowed.contains(&example), "missing ALLOW example: {example}");
    }
}

#[test]
fn neutral_svo_is_a_preference_not_a_force() {
    let root = fixture();
    let order = root["rules"]
        .as_array()
        .expect("rules")
        .iter()
        .find(|rule| rule["id"] == "FI_ORDER_001")
        .expect("FI_ORDER_001 exists");
    assert_eq!(order["decision"], "PREFER");
    assert_eq!(order["hard_constraint"], false);
    assert!(
        order["exceptions"]
            .as_array()
            .expect("exceptions")
            .iter()
            .any(|value| value == "stylistic_inversion"),
        "SVO must allow stylistic inversion"
    );
}
