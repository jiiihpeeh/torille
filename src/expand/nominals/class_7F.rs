//! Class 7*F (the lahti type) \u2014 native `-i` with t -> d.
//! The graded class-7 engine in `class_7` does the work; this module
//! only fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "7*F";

/// Inflect a class 7*F noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_7::inflect_graded(lemma, possessives, Gradation(6))
}
