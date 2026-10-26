//! Class 28*I (the jälsi type), written by hand.
//!
//! The graded `-lsi` family (`jälsi`) is the same shape as 28*J (`ls -> ll`,
//! partitive `s -> t`), so it delegates to the shared class-28 engine.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "28*I";
/// Inflect a class 28*I noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_28J::inflect(lemma, possessives)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lsi() {
        let w = inflect("jälsi", true).into_iter().map(|f| f.word).collect::<Vec<_>>();
        assert!(w.contains(&"jällen".to_string()));
        assert!(w.contains(&"jälttä".to_string()));
    }
}