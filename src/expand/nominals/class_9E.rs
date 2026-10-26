//! Class 9*E (the aapa type) \u2014 class 9's `-a` family with p -> v.
//! The graded class-9 engine in `class_9` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "9*E";

/// Inflect a class 9*E noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_9::inflect_graded(lemma, possessives, Gradation(5))
}
