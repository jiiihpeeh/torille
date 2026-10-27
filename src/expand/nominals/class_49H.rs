//! Class 49*H (the ommel type) inflection, written by hand.
//!
//! The `-el` family that grades `mm -> mp` in its obliques: `ommel` is
//! `ompele`/`ompeleen`, `vemmel` is `vempele`/`vempeleen`. It is exactly class
//! 49's paradigm on the gradated stem: the nominative and partitive singular
//! keep the lemma (`ommel`, `ommelta`), every oblique takes the gradated base
//! (`ompel`, `ompele`).
//!
//! Class 49's engine is the right home for this rather than a generated rule
//! because the word has **two spellings of the same cell**, not one: the
//! single-`e` half (`ompelen`, `ompelessa`) and the double-`e` half
//! (`ompeleen`, `ompeleessa`). [`class_49::inflect_with`] emits both, and the
//! nominative `ompele` beside `ommel`. A generated one-form-per-cell rule can
//! only offer one of them, which is why `ommel` used to miss half its paradigm
//! (and the heuristic filled the gap with the non-word `ommele`).
#![allow(dead_code)]

use crate::tags::Form;

pub const CLASS: &str = "49*H";

/// The gradated oblique base: the last `mm` reads `mp` (`ommel -> ompel`,
/// `vemmel -> vempel`).
fn gradated(lemma: &str) -> String {
    match lemma.rfind("mm") {
        Some(at) => {
            let mut s = lemma.to_string();
            s.replace_range(at..at + 2, "mp");
            s
        }
        None => lemma.to_string(),
    }
}

/// Inflect a class 49*H noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_49::inflect_with(lemma, &gradated(lemma), possessives)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    fn has(w: &[String], word: &str) -> bool {
        w.iter().any(|x| x == word)
    }

    /// The two ways of spelling `ommel` are both real, and both must be
    /// offered: the short-e type (`ompelen`, `ompelessa`) and the long-e type
    /// (`ompeleen`, `ompeleessa`). Wiktionary lists them side by side and live
    /// Voikko accepts every one of them.
    #[test]
    fn ommel_keeps_both_spellings() {
        let w = words("ommel");
        for word in [
            "ommel",
            "ompele",
            "ompelen",
            "ompeleen",
            "ommelta",
            "ompeletta",
            "ompelessa",
            "ompeleessa",
            "ompelesta",
            "ompeleesta",
            "ompelissa",
            "ompeleissa",
            "ompelin",
            "ompelein",
        ] {
            assert!(has(&w, word), "ommel is missing {word}");
        }
        // The heuristic's wrong stem is not a word and must never be offered.
        assert!(!has(&w, "ommele"), "ommel must not offer *ommele");
    }

    /// The same alternation on the other member, so this is the class's rule
    /// and not a hand-spelled exception for `ommel`.
    #[test]
    fn vemmel_keeps_both_spellings() {
        let w = words("vemmel");
        for word in [
            "vemmel",
            "vempele",
            "vempelen",
            "vempeleen",
            "vemmeltä",
            "vempelettä",
            "vempelessä",
            "vempeleessä",
        ] {
            assert!(has(&w, word), "vemmel is missing {word}");
        }
        assert!(!has(&w, "vemmele"), "vemmel must not offer *vemmele");
    }

    #[test]
    fn gradated() {
        assert_eq!(super::gradated("ommel"), "ompel");
        assert_eq!(super::gradated("vemmel"), "vempel");
    }
}
