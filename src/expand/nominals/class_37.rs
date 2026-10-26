//! Class 37 (the vasen type) inflection, written by hand.
//!
//! `vasen` (left) is a superlative-shaped word: `vasen` -> strong `vasempa`,
//! weak `vasemma`. Unlike class 36's `-in` superlatives its partitive sits on
//! the strong stem (`vasempaa`) rather than on the lemma, so it reuses the
//! shared superlative engine with `part_on_strong` set.
#![allow(dead_code)]

use crate::tags::Form;

pub const CLASS: &str = "37";

/// Inflect a class-37 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_36::superl_inflect(lemma, possessives, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn vasen() {
        let w = words("vasen");
        assert!(w.contains(&"vasemman".to_string()));
        assert!(w.contains(&"vasempaa".to_string()));
        assert!(w.contains(&"vasempaan".to_string()));
        assert!(w.contains(&"vasempien".to_string()));
        assert!(w.contains(&"vasempia".to_string()));
    }
}