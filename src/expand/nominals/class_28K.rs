//! Class 28*K (the varsi type), written by hand.
//!
//! The graded `-rsi` family (`varsi`) is the same shape as 28*J (`rs -> rr`,
//! partitive `s -> t`), so it delegates to the shared class-28 engine.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "28*K";
/// Inflect a class 28*K noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_28J::inflect(lemma, possessives)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rsi() {
        let w = inflect("varsi", true).into_iter().map(|f| f.word).collect::<Vec<_>>();
        assert!(w.contains(&"varren".to_string()));
        assert!(w.contains(&"vartta".to_string()));
        assert!(w.contains(&"vartten".to_string()));
    }
}