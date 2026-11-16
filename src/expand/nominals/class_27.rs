//! Class 27 (the käsi type) inflection, written by hand.
//!
//! Class 27 is the graded `-si` family whose weak grade is `s -> d`
//! (`käsi : käden`). It is the same four-stem shape as class 28, but with the
//! weak stem built by `s -> d` instead of `ns -> nn`:
//!   strong S = käsi, weak W = käde, partitive P = kät, essive stem E = käte.
//! It therefore reuses the shared class-28 engine with its own stems.
#![allow(dead_code)]

use crate::tags::{Case, Confidence, Form, Number};

use super::class_28J::{Stems, inflect_base};

pub const CLASS: &str = "27";

/// Inflect a class-27 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut base = lemma.to_string();
    base.pop(); // käsi -> käs
    let weak = format!("{}e", super::class_28J::rep_last_s(&base, "d")); // käde
    let ptp = super::class_28J::rep_last_s(&base, "t"); // kät
    let ten_prefix = matches!(lemma, "tosi" | "täysi").then(|| format!("{ptp}t"));
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
    let mut forms = inflect_base(lemma, possessives, &st, front);
    // Wiktionary's käsi table gives `tosien`/`täysien` as the genitive plural
    // and marks the `-ten` spelling (`totten`, `täytten`) rare. The shared
    // class-28 engine emits `-ten` as plain for the family, so demote it here
    // for these two words only; `käsi`/`kynsi` keep it.
    if let Some(prefix) = ten_prefix {
        for form in &mut forms {
            let genitive_plural = form.readings.iter().any(|reading| {
                reading.case == Some(Case::Genitive)
                    && reading.number == Some(Number::Plural)
            });
            if genitive_plural && form.word.starts_with(&prefix) {
                form.confidence = Confidence::Repaired;
            }
        }
    }
    forms
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

    /// Wiktionary's käsi table gives `tosien`/`täysien` as the genitive plural
    /// and marks the `-ten` spelling (`totten`, `täytten`) rare. The shared
    /// class-28 engine emits `-ten` plainly for the family, so class 27 demotes
    /// it for these two words.
    #[test]
    fn the_ten_genitive_plural_is_rare_for_tosi_and_taysi() {
        for (lemma, sien, rare) in [
            ("tosi", "tosien", ["totten", "tottemme", "tottesi"]),
            ("täysi", "täysien", ["täytten", "täyttemme", "täyttesi"]),
        ] {
            let forms = inflect(lemma, true);
            let plain: Vec<&str> = forms
                .iter()
                .filter(|f| f.confidence == Confidence::Plain)
                .map(|f| f.word.as_str())
                .collect();
            assert!(plain.contains(&sien), "{sien} not plain in {plain:?}");
            for form in rare {
                assert!(!plain.contains(&form), "{form} is still plain");
                assert!(
                    forms
                        .iter()
                        .any(|f| f.word == form && f.confidence == Confidence::Repaired),
                    "{form} is not a repaired rare form"
                );
            }
        }
    }
}
