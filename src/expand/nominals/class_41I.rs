//! Class 41*I (a graded 41 subtype) inflection, written by hand.
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
/// Inflect a class 41*I noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let short = super::class_41::grad_stem(lemma, "ll", "lt");
    let long = super::class_41::long_stem(&short);
    super::class_41::a_stem_inflect(lemma, possessives, &long, &short, true, Some(Gradation(9)))
}
