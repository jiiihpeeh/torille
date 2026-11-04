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

include!("class_6_i_plural.rs");

/// The per-lemma `-iden`/`-itten`/`-ita` confirmation.
fn i_plural(lemma: &str) -> IPlural {
    match I_PLURAL.binary_search_by_key(&lemma, |(word, _)| *word) {
        Ok(at) => IPlural::from_mask(I_PLURAL[at].1),
        Err(_) => IPlural::NONE,
    }
}

/// Inflect a base-class-6 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Class 6 also carries plural-only `-i` nouns such as `kekkerit`,
    // `bikinit`, `farkut`-family party words. The lemma is the plural
    // nominative, so build the paradigm from the dropped-`t` base and keep
    // only the plural cells: `kekkereitä`, not `kekkeri` or `kekkeritn`.
    if let Some(base) = lemma.strip_suffix('t') {
        let mut forms = build_i_loans(
            base,
            possessives,
            loanword_harmony(base),
            i_plural(lemma),
        );
        forms.retain(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        });
        return forms;
    }
    build_i_loans(lemma, possessives, loanword_harmony(lemma), i_plural(lemma))
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
    /// `vietnam` is a class-6 loanword but the reference confirms only the
    /// i-stem plural (`vietnamien`, `vietnameja`), not `vietnameiden`/
    /// `vietnameitten`/`vietnameita`. Those alternants stay accepted but not
    /// plain.
    #[test]
    fn vietnam_keeps_the_i_plural_plain_and_the_e_plural_rare() {
        let forms = inflect("vietnam", true);
        let find = |word: &str| {
            forms
                .iter()
                .find(|form| form.word == word)
                .unwrap_or_else(|| panic!("{word} missing"))
        };
        assert!(find("vietnamien").plain());
        assert!(find("vietnameja").plain());
        assert!(!find("vietnameiden").plain());
        assert!(!find("vietnameitten").plain());
        assert!(!find("vietnameita").plain());
    }

    /// A partial mask marks only the alternants the reference confirms.
    /// `appenzeller` has `-iden` and `-ita` but not `-itten`.
    #[test]
    fn a_partial_i_plural_marks_only_the_confirmed_alternants() {
        let forms = inflect("appenzeller", true);
        let find = |word: &str| {
            forms
                .iter()
                .find(|form| form.word == word)
                .unwrap_or_else(|| panic!("{word} missing"))
        };
        assert!(find("appenzellereiden").plain());
        assert!(!find("appenzellereitten").plain());
        assert!(find("appenzellereita").plain());
    }

}
