//! Class 1*D (the jako type) -- class 1 with a deleted final consonant.
//!
//! The consonant goes rather than weakening: `reika` -> `reian`, `jako` -> `jao`.
//! Not every word of the class actually loses it -- `haiku` and `urku` keep the
//! `k` -- so the class carries the list of lexemes whose final `k` is
//! phonologically stable and leaves those at full grade.
//!
//! Which cells take which grade is [`super::gradated`], shared by all eleven of
//! these classes.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::Form;

use super::gradated;

pub const CLASS: &str = "1*D";

/// The gradation this class carries, from the spec in `kotus`: D = type 4, `k` deleted (`reika` -> `reian`).
const GRADATION: Gradation = Gradation(4);

/// Class 1*D words whose stem does not actually weaken: loanwords (`haiku`)
/// and a handful of others (`urku`) where the final `k` is phonologically
/// stable rather than silently deletable. The reference confirms their
/// genitive as `haikun`/`urkun` (k kept), so the weak stem is the lemma.
const NO_GRADATION: &[&str] = &["haiku", "urku"];

/// The weak stem: the weak grade read from the kotus gradation spec, `k` deleted (`reika` -> `reian`).
/// The strong stem keeps the strong consonant.
fn weak_stem(lemma: &str) -> String {
    if NO_GRADATION.contains(&lemma) {
        return lemma.to_string();
    }
    GRADATION.apply(lemma, Grade::Weak)
}

/// Inflect a class-1*D noun.
///
/// The grid is [`super::gradated`]: every cell of the paradigm, in the order
/// this class emits them. What is left here is the one thing that tells
/// `1*D` from its ten siblings.
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
    /// of what this module adds to [`super::gradated`]: `alku` is `alun` in the
    /// genitive but `alkuni` for the first person, so the possessive sits on the
    /// strong stem while the bare form does not.
    ///
    /// All three spellings are what the kotus reference records for `alku`, so
    /// this fails if the gradation code is read wrongly as well as if the grid
    /// is.
    #[test]
    fn the_crossover_holds() {
        let w = words("alku");
        assert!(w.contains(&"alun".to_string()), "genitive bare is weak");
        assert!(w.contains(&"alkuni".to_string()), "genitive -ni is strong");
    }
}
