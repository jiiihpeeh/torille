//! Class 8*(C): the `-tte` loanwords (ringette) whose tt grade does not apply;
//! they inflect through the plain non-graded class 8.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "8*(C)";
/// Inflect a class 8*(C) noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_8::inflect(lemma, possessives)
}
