//! Class 1I (the aalto type) -- class 1 with a consonant gradation.
//!
//! The final `lt` reads as `ll`: `aalto` -> `aallon`.
//!
//! Which cells take which grade is [`super::gradated`], shared by all eleven of
//! these classes. What tells `1I` from its siblings is only the rule below.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::Form;

use super::gradated;

pub const CLASS: &str = "1*I";

/// The gradation this class carries, from the spec in `kotus`: I = type 9, `lt -> ll` (`aalto` -> `aallon`).
const GRADATION: Gradation = Gradation(9);

/// The weak stem: the weak grade read from the kotus gradation spec, `lt -> ll` (`aalto` -> `aallon`).
/// The strong stem keeps the strong consonant.
fn weak_stem(lemma: &str) -> String {
    GRADATION.apply(lemma, Grade::Weak)
}

/// Inflect a class-1*I noun.
///
/// The grid is [`super::gradated`]: every cell of the paradigm, in the order
/// this class emits them. What is left here is the one thing that tells
/// `1*I` from its ten siblings.
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
    /// of what this module adds to [`super::gradated`]: `aalto` is `aallon` in the
    /// genitive but `aaltoni` for the first person, so the possessive sits on the
    /// strong stem while the bare form does not.
    ///
    /// All three spellings are what the kotus reference records for `aalto`, so
    /// this fails if the gradation code is read wrongly as well as if the grid
    /// is.
    #[test]
    fn the_crossover_holds() {
        let w = words("aalto");
        assert!(w.contains(&"aallon".to_string()), "genitive bare is weak");
        assert!(w.contains(&"aaltoni".to_string()), "genitive -ni is strong");
    }
}
