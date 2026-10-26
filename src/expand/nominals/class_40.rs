//! Class 40 (the aatelisuus type) inflection, written by hand.
//!
//! Class 40 is the `-uus`/`-yys` abstract family (aatelisuus, terveellisyys),
//! which gradate the `-us` to `-ud(e)`/`-ut(e)`/`-uks`:
//! `aakkosellisuus` -> genitive `aakkosellisuuden` and the locals on `-uude`,
//! essive/illative/partitive on `-uute`/`-uutta`, and the plural on `-uuks`.
//! It is exactly class 39's `-ude` gradation, so it shares that engine.
#![allow(dead_code)]

use crate::tags::Form;

use super::class_33::inflect_graded_nom_base;
use super::class_39::estem as class39_estem;

pub const CLASS: &str = "40";

/// Inflect a class-40 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // `siivous` is classed 40 but inflects as the class-39 `-us` type, not the
    // `-uus` `-ude` gradation: the DB confirms the `siivoukse*` oblique
    // (`siivouksen`, `siivoukset`, `siivouksien`/`siivousten`), so it rides the
    // oblique stem rather than `siivoude`/`siivoute`.
    if lemma == "siivous" {
        let e = class39_estem(lemma);
        return inflect_graded_nom_base(lemma, possessives, &e, &e, &e);
    }
    super::class_39::use_ude(lemma, possessives)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn uus_type() {
        let w = words("aakkosellisuus");
        assert!(w.contains(&"aakkosellisuuden".to_string()));
        assert!(w.contains(&"aakkosellisuutta".to_string()));
        assert!(w.contains(&"aakkosellisuutena".to_string()));
        assert!(w.contains(&"aakkosellisuuksien".to_string()));
        assert!(w.contains(&"aakkosellisuuksia".to_string()));
    }
}