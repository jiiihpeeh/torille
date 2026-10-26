//! Class 9*G (the hanka type) \u2014 class 9's `-a` family with nk -> ng.
//! The graded class-9 engine in `class_9` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "9*G";

/// Inflect a class 9*G noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_9::inflect_graded(lemma, possessives, Gradation(7))
}
