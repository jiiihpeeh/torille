//! Class 41*D (a graded 41 subtype) inflection, written by hand.
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 41*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut base = lemma.to_string();
    base.pop();
    let last = base.chars().last().unwrap();
    let body = &base[..base.len() - base.chars().last().unwrap().len_utf8()];
    let short = format!("{}k{last}", body);
    let long = super::class_41::long_stem(&short);
    super::class_41::a_stem_inflect(lemma, possessives, &long, &short, true)
}
