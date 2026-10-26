//! Class 1E (the hepo type) -- class 1 with `p -> v` gradation.
//!
//! The final `p` voices to `v`: `hepo` -> `hevon`, `sopu` -> `sovun`. The code
//! carries it and the spec in `kotus` says what the code does, so the module
//! names the code and nothing else.
//!
//! Which cells take which grade is [`super::gradated`], shared by all eleven of
//! these classes.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::Form;

use super::gradated;

pub const CLASS: &str = "1*E";

/// The gradation this class carries, from the spec in `kotus`: E = type 5, `p -> v` (`hepo` -> `hevon`).
const GRADATION: Gradation = Gradation(5);

/// The weak stem: the weak grade read from the kotus gradation spec, `p -> v` (`hepo` -> `hevon`).
/// The strong stem keeps the strong consonant.
fn weak_stem(lemma: &str) -> String {
    GRADATION.apply(lemma, Grade::Weak)
}

/// Inflect a class-1*E noun.
///
/// The grid is [`super::gradated`]: every cell of the paradigm, in the order
/// this class emits them. What is left here is the one thing that tells
/// `1*E` from its ten siblings.
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
    /// of what this module adds to [`super::gradated`]: `hepo` is `hevon` in the
    /// genitive but `heponi` for the first person, so the possessive sits on the
    /// strong stem while the bare form does not.
    ///
    /// All three spellings are what the kotus reference records for `hepo`, so
    /// this fails if the gradation code is read wrongly as well as if the grid
    /// is.
    #[test]
    fn the_crossover_holds() {
        let w = words("hepo");
        assert!(w.contains(&"hevon".to_string()), "genitive bare is weak");
        assert!(w.contains(&"heponi".to_string()), "genitive -ni is strong");
    }
}
