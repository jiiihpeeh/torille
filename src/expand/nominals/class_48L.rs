//! Class 48*L (the elje type) inflection, written by hand.
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 48*L noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let s = super::class_48::e_grad(lemma, "j", "k");
    super::class_48::e_stem_inflect(lemma, possessives, &s)
}
