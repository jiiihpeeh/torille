//! Class 27 (the käsi type) inflection, written by hand.
//!
//! Class 27 is the graded `-si` family whose weak grade is `s -> d`
//! (`käsi : käden`). It is the same four-stem shape as class 28, but with the
//! weak stem built by `s -> d` instead of `ns -> nn`:
//!   strong S = käsi, weak W = käde, partitive P = kät, essive stem E = käte.
//! It therefore reuses the shared class-28 engine with its own stems.
#![allow(dead_code)]

use crate::tags::Form;

use super::class_28J::{Stems, inflect_base};

pub const CLASS: &str = "27";

/// Inflect a class-27 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut base = lemma.to_string();
    base.pop(); // käsi -> käs
    let weak = format!("{}e", super::class_28J::rep_last_s(&base, "d")); // käde
    let ptp = super::class_28J::rep_last_s(&base, "t"); // kät
    let e = format!("{ptp}e"); // käte
    let st = Stems {
        strong: lemma.to_string(),
        weak,
        ptp,
        e,
    };
    // The `-kesi` compounds (marraskesi, orvaskesi) are front-harmonic on their
    // front head, despite a back prefix.
    let front = if lemma.ends_with("kesi") {
        Some(true)
    } else {
        None
    };
    inflect_base(lemma, possessives, &st, front)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn s_to_d() {
        let w = words("käsi");
        assert!(w.contains(&"käden".to_string()));
        assert!(w.contains(&"kättä".to_string()));
        assert!(w.contains(&"käteen".to_string()));
        assert!(w.contains(&"käsien".to_string()));
        assert!(w.contains(&"kätten".to_string()));
    }

    /// `tosi`/`täysi` confirm the `-ten` genitive plural (`totten`,
    /// `täytten`) like every other class-27 word, so it is a plain form, not a
    /// restored guess -- the `ten_rare` demotion was wrong and is gone.
    #[test]
    fn the_ten_genitive_plural_is_confirmed_for_tosi_and_taysi() {
        for (lemma, form) in [("tosi", "totten"), ("täysi", "täytten")] {
            let forms = inflect(lemma, true);
            let plain: Vec<&str> = forms
                .iter()
                .filter(|f| f.confidence == crate::tags::Confidence::Plain)
                .map(|f| f.word.as_str())
                .collect();
            assert!(plain.contains(&form), "{form} not plain in {plain:?}");
        }
    }
}
