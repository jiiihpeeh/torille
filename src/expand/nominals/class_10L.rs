//! Class 10*L (the ylka type): class 10's `-a` family with k -> j.
//! The graded class-10 engine in `class_10` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "10*L";

/// Inflect a class 10*L noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_10::inflect_graded(lemma, possessives, Gradation(12))
}
