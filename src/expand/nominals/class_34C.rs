//! Class 34*C (the aateliton type) inflection, written by hand as a logical
//! engine.
//!
//! The privative `-ton` family geminates the `t` in its oblique stem
//! (`aateliton` -> `aatelittoma`), unlike the plain base class 34
//! (`alaston` -> `alastoma`), so this reuses `class_34::inflect_ton` with
//! `class_34::geminating_oblique` and supplies nothing but its class constant.

#![allow(dead_code)]

use crate::tags::Form;

pub const CLASS: &str = "34*C";

/// Inflect a class 34*C noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_34::inflect_ton(
        lemma,
        possessives,
        &super::class_34::geminating_oblique(lemma),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn tton() {
        let w = words("aateliton");
        assert!(w.contains(&"aateliton".to_string()));
        assert!(w.contains(&"aatelitonta".to_string()));
        assert!(w.contains(&"aatelittoman".to_string()));
        assert!(w.contains(&"aatelitonten".to_string()));
        assert!(w.contains(&"aatelittomien".to_string()));
    }
}
