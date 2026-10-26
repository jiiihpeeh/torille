//! Class 14*A (the allakka type): class 14's `-a` family, the same graded
//! `o`-stem engine as class 9's.
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
pub const CLASS: &str = "14*A";
/// Inflect a class 14*A noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_9::inflect_graded_both(lemma, possessives, Gradation(1))
}
