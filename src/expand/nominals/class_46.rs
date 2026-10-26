//! Class 46 (the tuhat type) inflection, written by hand.
//!
//! The numeral `tuhat` (thousand) gradate `-at -> -annen` (`tuhat -> tuhannen`)
//! with the plural on `tuhans-`, the same shape as class 45's `kahdeksas`
//! ordinal. It reuses that ordinal engine.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "46";
/// Inflect a class-46 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_45::ordinal(lemma, possessives)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tuhat() {
        let w = inflect("tuhat", true).into_iter().map(|f| f.word).collect::<Vec<_>>();
        assert!(w.contains(&"tuhannen".to_string()));
        assert!(w.contains(&"tuhatta".to_string()));
        assert!(w.contains(&"tuhansien".to_string()));
        assert!(!w.contains(&"tuhant".to_string()));
    }
}