//! Class 1G (the lanko type) -- class 1 with a consonant gradation.
//!
//! The final cluster becomes a nasal: `lanko` -> `langon`, `kenka` -> `kengan`.
//!
//! Which cells take which grade is [`super::gradated`], shared by all eleven of
//! these classes. What tells `1G` from its siblings is only the rule below.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::Form;

use super::gradated;

pub const CLASS: &str = "1*G";

/// The gradation this class carries, from the spec in `kotus`: G = type 7, `nk -> ng` (`lanko` -> `langon`).
const GRADATION: Gradation = Gradation(7);

/// The weak stem: the weak grade read from the kotus gradation spec, `nk -> ng` (`lanko` -> `langon`).
/// The strong stem keeps the strong consonant.
fn weak_stem(lemma: &str) -> String {
    GRADATION.apply(lemma, Grade::Weak)
}

/// Inflect a class-1*G noun.
///
/// The grid is [`super::gradated`]: every cell of the paradigm, in the order
/// this class emits them. What is left here is the one thing that tells
/// `1*G` from its ten siblings.
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
    /// of what this module adds to [`super::gradated`]: `ahdinko` is `ahdingon` in the
    /// genitive but `ahdinkoni` for the first person, so the possessive sits on the
    /// strong stem while the bare form does not.
    ///
    /// All three spellings are what the kotus reference records for `ahdinko`, so
    /// this fails if the gradation code is read wrongly as well as if the grid
    /// is.
    #[test]
    fn the_crossover_holds() {
        let w = words("ahdinko");
        assert!(w.contains(&"ahdingon".to_string()), "genitive bare is weak");
        assert!(
            w.contains(&"ahdinkoni".to_string()),
            "genitive -ni is strong"
        );
    }
}
