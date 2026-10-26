//! Class 33*C (a graded -in subtype) inflection, written by hand.
//!
//! A graded class-33 `-in` noun whose oblique stem weakens a particular
//! consonant before the final `-in`. It delegates to the shared class-33
//! graded engine with its own oblique stem.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "33*C";
/// Inflect a class 33*C noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let estem = super::class_33::geminate_estem(lemma);
    super::class_33::inflect_graded(lemma, possessives, &estem)
}
