//! Class 10*A (the aikaluokka type): class 10's `-a` family with kk -> k.
//! The graded class-10 engine in `class_10` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "10*A";

/// Inflect a class 10*A noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_10::inflect_graded(lemma, possessives, Gradation(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wiktionary gives `rukka` as `fi-decl-koira|ru|kk|k|a`: the weak grade
    /// is `ruka` (`rukan`, `rukassa`, `rukitta`), not the strong `rukka` the
    /// old `NON_GRADATING` entry forced.
    #[test]
    fn rukka_gradates_kk_to_k() {
        let words: Vec<String> = inflect("rukka", true)
            .into_iter()
            .map(|form| form.word)
            .collect();
        for want in ["rukka", "rukan", "rukkaa", "rukassa", "rukitta"] {
            assert!(words.contains(&want.to_string()), "missing {want}");
        }
        for wrong in ["rukkan", "rukkassa", "rukkitta"] {
            assert!(!words.contains(&wrong.to_string()), "gradated {wrong}");
        }
    }
}
