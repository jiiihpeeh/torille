//! Capitalised surfaces must reach the same lexicon neighbourhood as their
//! lower-case spelling.
//!
//! The sanalista is lower case, and Finnish has three letters outside ASCII
//! (`ä`, `ö`, `å`) with a capital for each. The suggestion and base-finding
//! paths used `to_ascii_lowercase`, which leaves `Ä`/`Ö`/`Å` alone: `Äitj`
//! got no suggestions at all while `äitj` suggested `äiti`, and
//! `Ääkeihinsä` was unknown with `check_normal` while `ääkeihinsä` was known.
//!
//! The invariants here are the review's E5 and E6 tables.

use std::collections::HashSet;

use torille::expand::check_normal::{NormalizedOutcome, check_normal};
use torille::suggest::{CheckOutcome, check, check_fast, is_known_word, suggest_detailed};

/// The inflected bases a `CheckOutcome` names, in its order.
fn bases(outcome: CheckOutcome) -> Vec<String> {
    match outcome {
        CheckOutcome::Inflected(bases) => bases.iter().map(|base| base.to_string()).collect(),
        CheckOutcome::Lemma | CheckOutcome::Unknown => Vec::new(),
    }
}

/// The first-character capitalisation `match_case` applies to a
/// sentence-initial query.
fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn suggestions(word: &str) -> Vec<String> {
    suggest_detailed(word, 5, 2)
        .into_iter()
        .map(|suggestion| suggestion.word)
        .collect()
}

/// E5: a capitalised typo gets the lower-case candidate list, in the same
/// order, with the input's own case restored.
#[test]
fn a_capitalised_typo_gets_the_same_suggestions_as_its_lowercase() {
    for (lower, upper) in [
        ("äitj", "Äitj"),
        ("öljy", "Öljy"),
        ("äidiss", "Äidiss"),
        ("åbo", "Åbo"),
    ] {
        let expected: Vec<String> = suggestions(lower)
            .iter()
            .map(|word| capitalized(word))
            .collect();
        assert_eq!(suggestions(upper), expected, "{upper}");
    }
    // The lower-case list is non-empty for the Finnish-capital cases, so the
    // equality above is not two empty lists.
    assert!(!suggestions("äitj").is_empty());
    assert!(!suggestions("öljy").is_empty());
}

/// E6: `check`, `check_fast` and `check_normal` answer a capitalised form
/// exactly as they answer its lower-case spelling.
///
/// `check` is the one exception by design: a capitalised surface can also be a
/// proper name (`Talossa` is the house and the surname `Talo`), and the name
/// readings are appended *beside* the common one. The common bases must
/// therefore be a prefix of the capitalised list, not the whole of it.
#[test]
fn a_capitalised_form_is_known_exactly_where_its_lowercase_is() {
    let proper = HashSet::new();
    for (lower, upper) in [
        ("ääkeihinsä", "Ääkeihinsä"),
        ("talossa", "Talossa"),
        ("kauniissa", "Kauniissa"),
        ("autonkuljettaja", "Autonkuljettaja"),
    ] {
        let lower_outcome = check(lower);
        let upper_outcome = check(upper);
        assert_eq!(upper_outcome.is_known(), lower_outcome.is_known(), "check({upper})");
        assert!(
            bases(upper_outcome).starts_with(&bases(lower_outcome)),
            "check({upper}): the common bases must stay first"
        );
        assert_eq!(
            check_fast(upper, &proper).map(|outcome| format!("{outcome:?}")),
            check_fast(lower, &proper).map(|outcome| format!("{outcome:?}")),
            "check_fast({upper})"
        );
        assert_eq!(
            check_normal(upper, false),
            check_normal(lower, false),
            "check_normal({upper})"
        );
        assert_eq!(
            is_known_word(upper),
            is_known_word(lower),
            "is_known_word({upper})"
        );
    }
    // The word that started the finding stays recognised, so the parity is
    // not two unknowns.
    assert_ne!(check_normal("Ääkeihinsä", false), NormalizedOutcome::Unknown);
}
