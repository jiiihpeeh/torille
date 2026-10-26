//! Class 33*L (the poljin type) inflection, written by hand.
//!
//! A graded class-33 subtype whose oblique stem reads the consonant before the
//! final vowel as `k` (`poljin -> polkime`, `suljin -> sulkime`). It delegates
//! to the shared class-33 engine.
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 33*L noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let estem = super::class_33::pair_estem(lemma, "j", "k");
    super::class_33::inflect_graded(lemma, possessives, &estem)
}
