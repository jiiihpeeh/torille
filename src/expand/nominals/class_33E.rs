//! Class 33*E (a graded -in subtype) inflection, written by hand.
//!
//! A graded class-33 `-in` noun whose oblique stem weakens a particular
//! consonant before the final `-in`. It delegates to the shared class-33
//! graded engine with its own oblique stem.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "33*E";
/// Inflect a class 33*E noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let estem = super::class_33::pair_estem(lemma, "v", "p");
    super::class_33::inflect_graded(lemma, possessives, &estem)
}
