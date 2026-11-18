//! Class 48*D (a graded 48 subtype) inflection, written by hand.
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 48*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let Some(base) = lemma.strip_suffix('e') else {
        return Vec::new();
    };
    let s = format!("{base}ke");
    super::class_48::e_stem_inflect(lemma, possessives, &s)
}
