//! Class 10*E (the koypa type): class 10's `-a` family with p -> v.
//! The graded class-10 engine in `class_10` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "10*E";

/// Inflect a class 10*E noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_10::inflect_graded(lemma, possessives, Gradation(5))
}
