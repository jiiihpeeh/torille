//! Class 33*K (the kerroin type) inflection, written by hand.
//!
//! A graded class-33 subtype whose oblique stem weakens the geminate before the
//! final diphthong (`kerroin -> kertoime`). It delegates to the shared engine.
#![allow(dead_code)]
use crate::tags::Form;
fn estem(lemma: &str) -> String {
    if lemma.contains("rroin") {
        lemma.replace("rroin", "rtoime")
    } else {
        super::class_33::pair_estem(lemma, "rr", "rt")
    }
}
/// Inflect a class 33*K noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_33::inflect_graded(lemma, possessives, &estem(lemma))
}
