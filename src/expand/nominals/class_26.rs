//! Class 26 (the kuori type) inflection, written by hand.
//!
//! Class 26 is the largest consonant-stem `-ri` family (kuori, mieli, hiiri).
//! Its shape is exactly class 24's: the `e`-stem carries the singular
//! obliques, the consonant-stem the partitive (`kuorta`) and the `-ten`
//! genitive plural (`kuorten`), and the lemma itself the `-ien` genitive
//! plural and the partitive plural. It therefore delegates to class 24.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "26";
/// Inflect a class-26 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_24::inflect(lemma, possessives)
}