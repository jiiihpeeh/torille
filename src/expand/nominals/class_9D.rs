//! Class 9*D (the aika/vika type): class 9's `-a` family with gradation of a
//! single `k`. The weak grade is `k -> j` for the `-aika` (time) compounds,
//! which contract `ai + j` to `aj` (`aika -> aja`, `adventtiaika ->
//! adventtiaja`); it is `k -> (zero)` for the other single-`k` words (`vika ->
//! vian`, `laki -> lain`, `jalka -> jalan`). `taika` alone ends in `-aika` yet
//! is `taian` (k -> zero).
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
pub const CLASS: &str = "9*D";

/// Inflect a class 9*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    if lemma.ends_with("aika") && lemma != "taika" {
        // aika -> aja: drop the final `aika` and write `aja`.
        let head = &lemma[..lemma.len() - 4];
        let weak = format!("{head}aja");
        super::class_9::inflect_graded_with(lemma, possessives, &weak)
    } else {
        super::class_9::inflect_graded(lemma, possessives, Gradation(4))
    }
}