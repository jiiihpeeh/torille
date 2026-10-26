//! Class 8*B (the jeppe type): class 8's `-e` family with `pp -> p`.
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
pub const CLASS: &str = "8*B";
/// Inflect a class 8*B noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_8::inflect_graded(lemma, possessives, Gradation(2))
}
