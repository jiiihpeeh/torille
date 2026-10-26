//! Class 42 (the mies type) inflection, written by hand.
//!
//! The irregular `mies`: the final `s` reads `h` in the oblique `e`-stem
//! (`mies -> miehe`), which carries the sg obliques (`miehen`, `miehessä`)
//! and the plural (`miehien`), while the lemma keeps the partitive (`miestä`).
//! It is the class-39 shape, so it delegates to that graded engine.
#![allow(dead_code)]

use crate::tags::Form;

use super::class_33::inflect_graded;

pub const CLASS: &str = "42";

/// Inflect a class-42 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let estem = format!("{}he", &lemma[..lemma.len() - 1]);
    inflect_graded(lemma, possessives, &estem)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn mies() {
        let w = words("mies");
        assert!(w.contains(&"miehen".to_string()));
        assert!(w.contains(&"miestä".to_string()));
        assert!(w.contains(&"mieheen".to_string()));
        assert!(w.contains(&"miehet".to_string()));
        assert!(w.contains(&"miehien".to_string()));
    }
}