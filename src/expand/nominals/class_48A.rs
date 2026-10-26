//! Class 48*A (the aluke type) inflection, written by hand.
//! The -ke diminutives gradate k -> kk (aluke -> alukke).
#![allow(dead_code)]
use crate::tags::Form;
/// Inflect a class 48*A noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let s = super::class_48::e_grad(lemma, "k", "kk");
    super::class_48::e_stem_inflect(lemma, possessives, &s)
}
