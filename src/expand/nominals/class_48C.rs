//! Class 48*C (a graded 48 subtype) inflection, written by hand.
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 48*C noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let s = super::class_48::e_grad(lemma, "t", "tt");
    super::class_48::e_stem_inflect(lemma, possessives, &s)
}
