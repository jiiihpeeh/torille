//! Class 41*G (a graded 41 subtype) inflection, written by hand.
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 41*G noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let short = super::class_41::grad_stem(lemma, "ng", "nk");
    let long = super::class_41::long_stem(&short);
    super::class_41::a_stem_inflect(lemma, possessives, &long, &short, true)
}
