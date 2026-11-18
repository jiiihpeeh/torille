//! Class 30 (the peitsi type) inflection, written by hand.
//!
//! Class 30 is the `-tsi` family (peitsi, veitsi), whose partitive collapses the
//! trailing `ts` to `s` (`veitsi -> veis`): gen `veitsen`, part `veistä`,
//! genitive plural `veitsien` + `veisten`. It is therefore the same four-stem
//! shape as class 29 (weak = the `i -> e` stem, reduced partitive stem) and
//! reuses the shared class-28 engine.
#![allow(dead_code)]

use crate::tags::Form;

use super::class_28J::{Stems, inflect_base};
use super::utility::e_stem;

pub const CLASS: &str = "30";

/// Inflect a class-30 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // The class-30 word ends in `-tsi`; the partitive collapse is `ts` -> `s`.
    // A word of another shape is not this class (the wizard tries them all).
    let Some(base) = lemma.strip_suffix('i') else {
        return Vec::new();
    };
    if !base.ends_with("ts") {
        return Vec::new();
    }
    let weak = e_stem(lemma); // veitse
    // Collapse the trailing `ts` to `s`: drop the `t` (keeping veits -> veis).
    let ptp = format!("{}s", &base[..base.len() - 2]);
    let st = Stems {
        strong: lemma.to_string(),
        weak: weak.clone(),
        ptp,
        e: weak,
    };
    inflect_base(lemma, possessives, &st, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn tsi() {
        let w = words("veitsi");
        assert!(w.contains(&"veitsen".to_string()));
        assert!(w.contains(&"veistä".to_string()));
        assert!(w.contains(&"veitsien".to_string()));
        assert!(w.contains(&"veisten".to_string()));
    }
}
