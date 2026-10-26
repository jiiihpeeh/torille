//! Class 7*D (the henki_joki type) \u2014 native `-i` with k -> (zero).
//! The graded class-7 engine in `class_7` does the work; this module
//! only fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

pub const CLASS: &str = "7*D";

/// Inflect a class 7*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_7::inflect_graded(lemma, possessives, Gradation(4))
}
