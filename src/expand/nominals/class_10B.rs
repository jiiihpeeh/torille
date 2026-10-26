//! Class 10*B (the dippa type): class 10's `-a` family with pp -> p.
//! The graded class-10 engine in `class_10` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "10*B";

/// Inflect a class 10*B noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_10::inflect_graded(lemma, possessives, Gradation(2))
}
