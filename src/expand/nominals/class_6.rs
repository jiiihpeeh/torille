//! Class 6 (the aateli/agitaattori type) inflection, written by hand.
//!
//! Class 6 is the loanword `-i`/consonant family (`aateli`, `agitaattori`,
//! `appenzeller`). It shares the non-gradating loanword `-i` paradigm in
//! `utility::build_i_loans` -- an `i`-stem singular (the illative doubling the
//! `i`) and a full `e`-stem plural (`-ien`/`-eiden`/`-eitten` genitive,
//! `-ja`/`-ita` partitive) -- and the loanword harmony in `harmony`.

#![allow(dead_code)]

use super::harmony::loanword_harmony;
use super::utility::{IPlural, build_i_loans};
use crate::tags::{Form, Number};

pub const CLASS: &str = "6";

/// Inflect a base-class-6 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Class 6 also carries plural-only `-i` nouns such as `kekkerit`,
    // `bikinit`, `farkut`-family party words. The lemma is the plural
    // nominative, so build the paradigm from the dropped-`t` base and keep
    // only the plural cells: `kekkereitä`, not `kekkeri` or `kekkeritn`.
    if let Some(base) = lemma.strip_suffix('t') {
        let mut forms = build_i_loans(base, possessives, loanword_harmony(base), IPlural::All);
        forms.retain(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        });
        return forms;
    }
    build_i_loans(lemma, possessives, loanword_harmony(lemma), IPlural::All)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn singular_i_stem() {
        let w = words("aateli");
        assert!(w.contains(&"aatelin".to_string()));
        assert!(w.contains(&"aatelia".to_string()));
        assert!(w.contains(&"aateliin".to_string()));
        assert!(w.contains(&"aatelissa".to_string()));
    }

    #[test]
    fn full_plural_variants() {
        let w = words("aateli");
        assert!(w.contains(&"aatelien".to_string())); // gen pl, i-stem + -en
        assert!(w.contains(&"aateleiden".to_string())); // gen pl, e-stem + -iden
        assert!(w.contains(&"aateleitten".to_string()));
        assert!(w.contains(&"aateleja".to_string())); // part pl, vowel-plural
        assert!(w.contains(&"aateleita".to_string())); // part pl, i-plural
    }

    #[test]
    fn consonant_final_lemma() {
        let w = words("appenzeller");
        assert!(w.contains(&"appenzellerin".to_string()));
        assert!(w.contains(&"appenzelleria".to_string()));
        assert!(w.contains(&"appenzellereita".to_string()));
    }

    #[test]
    fn plural_only_party_words_use_the_dropped_stem() {
        let w = words("kekkerit");
        assert!(w.contains(&"kekkerit".to_string()));
        assert!(w.contains(&"kekkereitä".to_string()));
        assert!(w.contains(&"kekkereissä".to_string()));
        assert!(!w.contains(&"kekkeri".to_string()));
        assert!(!w.contains(&"kekkeritn".to_string()));
    }
}
