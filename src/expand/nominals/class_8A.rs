//! Class 8*A (runtime the nukke type): class 8's `-e` family with `kk -> k`.
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
pub const CLASS: &str = "8*A";
/// Inflect a class 8*A noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_8::inflect_graded(lemma, possessives, Gradation(1))
}
