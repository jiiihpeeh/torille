//! Class 1M (the kyky type) -- class 1 with a consonant gradation.
//!
//! The final `k` voices to `v`: `kyky` -> `kyvyn`, `suku` -> `suvun`. This is
//! the consonant `1*D` also ends in, but there it is deleted and here it voices;
//! the two classes are told apart by their words rather than by their shape.
//!
//! Which cells take which grade is [`super::gradated`], shared by all eleven of
//! these classes. What tells `1M` from its siblings is only the rule below.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::Form;

use super::gradated;

pub const CLASS: &str = "1*M";

/// The gradation this class carries, from the spec in `kotus`: M = type 13, `k -> v` (`kyky` -> `kyvyn`).
const GRADATION: Gradation = Gradation(13);

/// The weak stem: the weak grade read from the kotus gradation spec, `k -> v` (`kyky` -> `kyvyn`).
/// The strong stem keeps the strong consonant.
fn weak_stem(lemma: &str) -> String {
    GRADATION.apply(lemma, Grade::Weak)
}

/// Inflect a class-1*M noun.
///
/// The grid is [`super::gradated`]: every cell of the paradigm, in the order
/// this class emits them. What is left here is the one thing that tells
/// `1*M` from its ten siblings.
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
    /// of what this module adds to [`super::gradated`]: `kyky` is `kyvyn` in the
    /// genitive but `kykyni` for the first person, so the possessive sits on the
    /// strong stem while the bare form does not.
    ///
    /// All three spellings are what the kotus reference records for `kyky`, so
    /// this fails if the gradation code is read wrongly as well as if the grid
    /// is.
    #[test]
    fn the_crossover_holds() {
        let w = words("kyky");
        assert!(w.contains(&"kyvyn".to_string()), "genitive bare is weak");
        assert!(w.contains(&"kykyni".to_string()), "genitive -ni is strong");
    }
}
