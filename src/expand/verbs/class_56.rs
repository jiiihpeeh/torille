//! Class 56 (the `kaivaa` type) inflection, written by hand.
//!
//! Class 56 is type 1 with a long theme vowel, like class 53, and it has that
//! class's present: the stem is the infinitive without its final `a`/`ä`
//! (`kaiva`, `auta`, `anta`), the third person doubles the theme vowel
//! (`kaivaa`, `auttaa`, `antaa`) and the passive rethemes the weak grade to `e`
//! (`kaivetaan`, `autetaan`, `annetaan`). Its past is where it differs.
//!
//! Where class 53's past drops the theme vowel (`muista` -> `muist`,
//! `muistin`), class 56's keeps it as `o`/`ö`: the stem's final `a`/`ä` is read
//! as `o`/`ö` (`kaiva` -> `kaivo`, `auta` -> `auto`, `anta` -> `anto`). As in
//! class 53 the third person takes the strong grade and the other persons the
//! weak one, so `auttaa` has `auttoi` and `auttoivat` beside `autoin` and
//! `auttoimme`, and `antaa` has `antoi` beside `annoin`. [`Stems::with_o_past`]
//! is what says so.
//!
//! Gradation, where the class names it, is class 53's pattern of cells: `56*A`
//! (`virkkaa`) `kk` -> `k`, `56*B` (`appaa`) `pp` -> `p`, `56*C` (`auttaa`)
//! `tt` -> `t`, `56*D` (`alkaa`) `lk` -> `l`, `56*F` (`ahtaa`) `ht` -> `hd`
//! and `56*J` (`antaa`) `nt` -> `nn`. The weak grade is the present's other
//! persons, the imperative's second person singular, the whole passive and the
//! past's other persons; the strong grade is the third persons, the
//! conditional, the potential, the imperative's other cells and the
//! participles.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "56";

/// Inflect a class-56 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    paradigm(&Stems::with_o_past(lemma, stem, gradation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    /// The words of a lemma that gradates, the code given in the sanalista's
    /// own letters: `C` for `56*C`, which is `tt <-> t`.
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
    fn the_present_doubles_the_theme_vowel() {
        for want in [
            "kaivaa",
            "kaivavat",
            "kaivaisin",
            "kaivanen",
            "kaivakoon",
            "kaivanut",
            "kaivava",
        ] {
            assert!(has("kaivaa", want), "kaivaa should have {want}");
        }
    }

    #[test]
    fn the_past_reads_the_theme_as_o() {
        // `kaiva` -> `kaivo`: the whole past, strong and weak together.
        for want in [
            "kaivoin",
            "kaivoit",
            "kaivoi",
            "kaivoimme",
            "kaivoitte",
            "kaivoivat",
        ] {
            assert!(has("kaivaa", want), "kaivaa should have {want}");
        }
    }

    #[test]
    fn tt_gradation_runs_through_present_and_past() {
        // `56*C` = `auttaa`: `tt` <-> `t`, and the past is weak `auto` beside
        // strong `autto`.
        for want in [
            "auttaa",
            "auttavat",
            "auttaisin",
            "auttanen",
            "auttakoon",
            "autan",
            "autat",
            "auta",
            "autetaan",
            "autettiin",
            "auttanut",
            "auttava",
            "autettu",
        ] {
            assert!(has_g("auttaa", 'C', want), "auttaa should have {want}");
        }
        // The past: weak `auto` for the other persons, strong `autto` for the
        // third person.
        assert!(has_g("auttaa", 'C', "autoin"));
        assert!(has_g("auttaa", 'C', "autoit"));
        assert!(has_g("auttaa", 'C', "autoimme"));
        assert!(has_g("auttaa", 'C', "auttoi"));
        assert!(has_g("auttaa", 'C', "auttoivat"));
        assert!(!has_g("auttaa", 'C', "auttoin"));
    }

    #[test]
    fn nt_gradation_makes_the_third_person_strong() {
        // `56*J` = `antaa`: `nt` <-> `nn`, `annoin` beside `antoi`.
        for want in [
            "antaa",
            "antavat",
            "antaisin",
            "antanen",
            "antakoon",
            "annan",
            "annat",
            "annamme",
            "annatte",
            "anna",
            "annetaan",
            "annettiin",
            "antanut",
            "antava",
            "annettu",
            "annoin",
            "annoit",
            "antoi",
            "antoivat",
        ] {
            assert!(has_g("antaa", 'J', want), "antaa should have {want}");
        }
    }

    #[test]
    fn ht_gradation() {
        // `56*F` = `ahtaa`: `ht` <-> `hd`, `ahdoin` beside `ahtoi`.
        for want in [
            "ahtaa",
            "ahtavat",
            "ahtaisin",
            "ahtanen",
            "ahtakoon",
            "ahdan",
            "ahdat",
            "ahda",
            "ahdetaan",
            "ahdettiin",
            "ahtanut",
            "ahtava",
            "ahdettu",
            "ahdoin",
            "ahdoit",
            "ahtoi",
            "ahtoivat",
        ] {
            assert!(has_g("ahtaa", 'F', want), "ahtaa should have {want}");
        }
    }

    #[test]
    fn kk_pp_lk_gradation() {
        // `56*A` = `virkkaa`: `kk` <-> `k`.
        for want in ["virkkaa", "virkan", "virka", "virkoin", "virkkoi"] {
            assert!(has_g("virkkaa", 'A', want), "virkkaa should have {want}");
        }
        // `56*B` = `appaa`: `pp` <-> `p`.
        for want in ["appaa", "apan", "apa", "apoin", "appoi"] {
            assert!(has_g("appaa", 'B', want), "appaa should have {want}");
        }
        // `56*D` = `alkaa`: `lk` <-> `l`.
        for want in ["alkaa", "alan", "ala", "aloin", "alkoi"] {
            assert!(has_g("alkaa", 'D', want), "alkaa should have {want}");
        }
    }

    #[test]
    fn front_harmony_reads_the_theme_as_o_with_two_dots() {
        // No real class-56 lemma is front-harmony, but the rule should still
        // hold: `ä` -> `ö`, as in a made-up `elää`-shaped stem `pöllää`...
        // exercised through the shared stem rule rather than a lemma here.
        assert_eq!(Stems::o_past_stem_of("pöllä"), "pöllö");
        assert_eq!(Stems::o_past_stem_of("kaiva"), "kaivo");
    }
}
