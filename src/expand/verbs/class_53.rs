//! Class 53 (the `muistaa` type) inflection, written by hand.
//!
//! Class 53 is type 1 with a long theme vowel: `muistaa`, `aakkostaa`,
//! `edistää`, `arkipäiväistää`. Against class 52 there is one difference and no
//! others.
//!
//! Many of them gradate, as `53*C` says: `helpottaa` has `tt` for `t` in its
//! third person and its participles (`helpottaa`, `helpottava`, `helpottanut`)
//! but not elsewhere (`helpotan`, `helpotettu`).
//!
//! The infinitive still drops its final `a`/`ä` for the stem, so the stem is
//! the one that itself ends in the theme vowel: `aakkosta` for `aakkostaa`,
//! `edistä` for `edistää`. That is what puts the doubled `aa` in the
//! third-person present (`aakkostaa` = `aakkosta` + `a`) without any special
//! case -- it is class 52's rule that a stem ending in a vowel takes that same
//! vowel again.
//!
//! The difference is the passive. Where a class-52 verb keeps its stem in the
//! passive (`sanoa` -> `sanotaan`), here the stem's final `a`/`ä` becomes `e`,
//! because in this theme that vowel *is* the passive's: `aakkostetaan` beside
//! `sanotaan`, `arkipäiväistetään` beside `kysytään`. Everything downstream
//! follows the same endings as class 52 -- `aakkostet` + `aan`, `aakkostett` +
//! `iin`, `aakkostetta` + `isiin`/`neen`/`koon`/`essa`/`mään` -- so only the
//! stems are computed here.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "53";

/// Inflect a class-53 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    // The past drops the theme vowel that the present keeps: `aakkostaa`
    // inflects `aakkostaa` in the present and `aakkosti` in the past, so the
    // past runs on `aakkost`.
    let past = Stems::past_stem_of(stem);
    paradigm(&Stems::with_past(lemma, stem, &past, gradation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    /// The words of a lemma that gradates, the code given in the sanalista's
    /// own letters: `C` for `53*C`, which is `tt <-> t`.
    fn graded(lemma: &str, code: char) -> Vec<String> {
        inflect(lemma, Gradation::from_letter(code))
            .into_iter()
            .map(|f| f.word)
            .collect()
    }

    fn has(lemma: &str, form: &str) -> bool {
        words(lemma).iter().any(|w| w == form)
    }

    fn has_g(lemma: &str, code: char, form: &str) -> bool {
        graded(lemma, code).iter().any(|w| w == form)
    }

    #[test]
    fn gradation_splits_the_cells_like_class_52() {
        // `53*C` = `helpottaa`: `tt` <-> `t`. The third person and the
        // participles keep the strong `helpott`, the rest take the weak
        // `helpot`, and the passive is built on the weak grade.
        for want in [
            "helpottaa",
            "helpottivat",
            "helpottaisin",
            "helpottanen",
            "helpottakoon",
            "helpottanut",
            "helpottava",
            "helpotettu",
        ] {
            assert!(
                has_g("helpottaa", 'C', want),
                "helpottaa should have {want}"
            );
        }
        for want in [
            "helpotan",
            "helpotat",
            "helpotamme",
            "helpotimme",
            "helpota",
            "helpotetaan",
            "helpotettiin",
            "helpotettu",
            "helpotettava",
        ] {
            assert!(
                has_g("helpottaa", 'C', want),
                "helpottaa should have {want}"
            );
        }
        assert!(!has_g("helpottaa", 'C', "helpottan"));
    }

    #[test]
    fn the_past_shortens_the_theme_as_well() {
        // `helpottaa` -> `helpotin`, so the past stem is `helpot` less its
        // theme vowel.
        assert!(has_g("helpottaa", 'C', "helpotin"));
        assert!(has_g("helpottaa", 'C', "helpottivat"));
    }

    #[test]
    fn finite_active() {
        for want in [
            "aakkostan",
            "aakkostat",
            "aakkostaa",
            "aakkostamme",
            "aakkostatte",
            "aakkostavat",
            "aakkostin",
            "aakkostit",
            "aakkosti",
            "aakkostivat",
            "aakkostaisin",
            "aakkostaisi",
            "aakkostanen",
            "aakkostanee",
            "aakkostanevat",
            "aakkosta",
            "aakkostakoon",
            "aakkostakaa",
            "aakkostakaamme",
        ] {
            assert!(has("aakkostaa", want), "aakkostaa should have {want}");
        }
    }

    #[test]
    fn the_passive_theme_is_e() {
        // `aakkosta` -> `aakkoste` -> `aakkostet`, so `aakkostetaan`, not
        // `aakkostataan`.
        for want in [
            "aakkostetaan",
            "aakkostettiin",
            "aakkostettaisiin",
            "aakkostettaneen",
            "aakkostettakoon",
            "aakkosteta",
            "aakkostettu",
        ] {
            assert!(has("aakkostaa", want), "aakkostaa should have {want}");
        }
        assert!(!has("aakkostaa", "aakkostataan"));
    }

    #[test]
    fn front_harmony_reaches_the_passive_too() {
        // `edistä` -> `ediste` -> `edistet`, and the mood stem takes `ä`.
        for want in [
            "edistän",
            "edistää",
            "edistämme",
            "edistävät",
            "edistetään",
            "edistettiin",
            "edistettäisiin",
            "edistettäneen",
            "edistettäköön",
            "edistetty",
            "edistettävä",
        ] {
            assert!(has("edistää", want), "edistää should have {want}");
        }
    }

    #[test]
    fn non_forms() {
        // Both spellings of the 2nd infinitive: the `-e` stem with possessives,
        // the passive stem bare.
        for want in [
            "aakkostaessa",
            "aakkostaessani",
            "aakkostaessaan",
            "aakkostettaessa",
            "aakkostaen",
            "aakkostamaan",
            "aakkostamassa",
            "aakkostamasta",
            "aakkostamalla",
            "aakkostamatta",
            "aakkostaman",
            "aakkostettaman",
            "aakkostaminen",
            "aakkostamiseni",
        ] {
            assert!(has("aakkostaa", want), "aakkostaa should have {want}");
        }
    }

    #[test]
    fn participles() {
        for want in [
            "aakkostava",
            "aakkostavan",
            "aakkostavat",
            "aakkostavani",
            "aakkostama",
            "aakkostaman",
            "aakkostamat",
            "aakkostanut",
            "aakkostaneen",
            "aakkostaneita",
            "aakkostaneeni",
            "aakkostettu",
            "aakkostetun",
            "aakkostettua",
            "aakkostettuna",
            "aakkostettava",
            "aakkostamaton",
            "aakkostamattoman",
            "aakkostamattomat",
        ] {
            assert!(has("aakkostaa", want), "aakkostaa should have {want}");
        }
    }
}
