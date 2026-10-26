//! Class 10*D (the poika type): class 10's `-a` family with gradation of a
//! single `k`. Most words are `k -> (zero)` (`ruoka -> ruonan`, `sulka ->
//! sulan`), but `poika` contracts `oi + j` to `oj` (`poika -> poja`) and `ruoka`
//! reads two ways, `ruoka` -> `ruoa`/`ruua`.
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
pub const CLASS: &str = "10*D";

/// Inflect a class 10*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    if lemma == "poika" {
        // poika -> poja, contracting the oi+j.
        return super::class_10::inflect_graded_with(lemma, possessives, "poja");
    }
    if lemma == "ruoka" {
        // ruoka reads both ruoa and ruua (ruuka): offer both weak stems.
        let mut out = super::class_10::inflect_graded_with(lemma, possessives, "ruoa");
        out.extend(super::class_10::inflect_graded_with(lemma, possessives, "ruua"));
        return out;
    }
    super::class_10::inflect_graded(lemma, possessives, Gradation(4))
}