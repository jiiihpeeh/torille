//! Class 5*G (the blandinki type) \u2014 class 5's `-i` family with nk -> ng.
//! The graded class-5 engine in `class_5` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "5*G";

/// Inflect a class 5*G noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_5::inflect_graded(lemma, possessives, Gradation(7))
}
