//! Class 32*D (the ien type) inflection, written by hand.
//!
//! `ien` is the `k`-deletion gradation of the consonant-word family: the weak
//! nominative keeps `ien`, while the strong consonant stem puts the `k` back
//! before the final vowel (`iken`), and the `e`-stem is `ikene`. The partitive
//! singular and the `-ten` genitive plural stay on the weak stem (`ientä`,
//! `ienten`), which is why this cannot be the plain class-32 grid.
#![allow(dead_code)]

use crate::tags::Form;

pub const CLASS: &str = "32*D";

/// The strong consonant stem: `ien` -> `iken`.
fn strong(lemma: &str) -> String {
    if lemma.contains('k') {
        return lemma.to_string();
    }
    let Some((at, _)) = lemma
        .char_indices()
        .rev()
        .find(|(_, c)| "aeiouyäö".contains(*c))
    else {
        return lemma.to_string();
    };
    format!("{}k{}", &lemma[..at], &lemma[at..])
}

/// Inflect a class 32*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let istem = strong(lemma);
    let estem = format!("{istem}e");
    super::class_32::inflect_with_stems_extra(
        lemma,
        possessives,
        &istem,
        &estem,
        lemma,
        lemma,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ien_keeps_the_weak_partitive_and_ten_genitive() {
        let w: Vec<String> = inflect("ien", true).into_iter().map(|f| f.word).collect();
        for good in ["ien", "ikenen", "ientä", "ikeneen", "ikenissä", "ienten", "ikeniä"] {
            assert!(w.contains(&good.to_string()), "ien is missing {good}");
        }
        for bad in ["ikenta", "ikenten", "ienen", "ienessä"] {
            assert!(!w.contains(&bad.to_string()), "ien must not offer {bad}");
        }
    }
}
