//! Class 1F (the ehto type) -- class 1 with `t -> d` gradation.
//!
//! The final `t` voices to `d`: `ehto` -> `ehdon`, `hyöty` -> `hyödyn`. A word
//! of the class can also decline to gradate at all -- `kyty` keeps its `t`, so
//! its weak stem is itself -- so the class carries the list of lexemes that do
//! not and leaves those at full grade.
//!
//! Which cells take which grade is [`super::gradated`], shared by all eleven of
//! these classes.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::Form;

use super::gradated;

pub const CLASS: &str = "1*F";

/// The gradation this class carries, from the spec in `kotus`: F = type 6, `t -> d` (`ehto` -> `ehdon`).
const GRADATION: Gradation = Gradation(6);

/// Lexemes in the gradation-capable class that do not, in fact, gradate: the
/// class permits `t -> d`, but a particular word can be non-gradating (kyty :
/// kytyn, not kydyn). The weak stem of such a word is the lemma itself.
const NON_GRADATING: &[&str] = &["kyty"];

/// The weak stem: the weak grade read from the kotus gradation spec, `t -> d` (`ehto` -> `ehdon`).
/// The strong stem keeps the strong consonant.
fn weak_stem(lemma: &str) -> String {
    if NON_GRADATING.contains(&lemma) {
        return lemma.to_string();
    }
    GRADATION.apply(lemma, Grade::Weak)
}

/// Inflect a class-1*F noun.
///
/// The grid is [`super::gradated`]: every cell of the paradigm, in the order
/// this class emits them. What is left here is the one thing that tells
/// `1*F` from its ten siblings.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    gradated::inflect(lemma, possessives, &weak_stem(lemma))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// The class gradates in exactly the way its code says, which is the whole
    /// of what this module adds to [`super::gradated`]: `aironveto` is `aironvedon` in the
    /// genitive but `aironvetoni` for the first person, so the possessive sits on the
    /// strong stem while the bare form does not.
    ///
    /// All three spellings are what the kotus reference records for `aironveto`, so
    /// this fails if the gradation code is read wrongly as well as if the grid
    /// is.
    #[test]
    fn the_crossover_holds() {
        let w = words("aironveto");
        assert!(
            w.contains(&"aironvedon".to_string()),
            "genitive bare is weak"
        );
        assert!(
            w.contains(&"aironvetoni".to_string()),
            "genitive -ni is strong"
        );
    }
}
