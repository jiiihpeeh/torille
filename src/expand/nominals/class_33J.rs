//! Class 33*J (the hienonnin type) inflection, written by hand.
//!
//! A graded class-33 subtype whose oblique stem reads the geminate `nn` as `nt`
//! (`hienonnin -> hienontime`). It delegates to the shared class-33 engine.
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 33*J noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let estem = super::class_33::pair_estem(lemma, "nn", "nt");
    super::class_33::inflect_graded(lemma, possessives, &estem)
}
