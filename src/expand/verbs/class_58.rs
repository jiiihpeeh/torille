//! Class 58 (the `laskea` type) inflection, written by hand.
//!
//! Class 58 is the plain type-1 verb, the one Finnish grammars teach first:
//! `laskea`, `kylpeä`, `hakea`, `kutea`. The stem is the infinitive less its
//! theme vowel (`laske`, `kylpe`, `hake`, `kute`) and the third person repeats
//! that stem's own `e` rather than doubling the infinitive's, so `laskee` sits
//! beside `lasken` where class 53 has `muistaa` beside `muistan`. The passive
//! runs on the weak grade with an `e` of its own (`lasketaan`, `kylvetään`,
//! `haetaan`), which is class 53's rule unchanged -- nothing in the passive
//! distinguishes this class from that one.
//!
//! What differs is the past, and only because the stem's theme vowel is a
//! consonant's neighbour rather than the stem's own last letter. Class 53 drops
//! the infinitive's theme vowel, which for it *is* the stem's last letter
//! (`aakkosta` -> `aakkost`); class 58 drops the stem's own `e` (`laske` ->
//! `lask`, `kylpe` -> `kylp`), so `laskea` inflects `laskee` in the present and
//! `laski` in the past. [`Stems::with_e_past`] is what says so.
//!
//! That difference in *where* the drop happens is visible only where the class
//! gradates, because only then is the past's weak grade a different string from
//! its strong one -- and then the two operations are no longer interchangeable.
//! `hakea` (`58*D`) drops its `e` first (`hak`) and gradates `k` away after
//! (`ha`), giving `hain` and `hait` beside the strong `haki` and `hakivat`. Had
//! it gradated first, the weak grade would have been `hage` -> `hag`, and
//! `hagin` is not a word. The present keeps its `e` throughout, which is why its
//! weak grade is `hae` and the imperative's second person singular is `hae`
//! beside `hakekoon`.
//!
//! Gradation, where the class names it, is the usual run: `58*D` (`hakea`) drops
//! the `k` (`hagen`), `58*E` (`kylpeä`) weakens `p` to `v` (`kylven`), `58*F`
//! (`kutea`) `t` to `d` (`kuden`), `58*G` (`tunkea`) `nk` to `ng` (`tungen`)
//! and `58*L` (`kulkea`) `k` to `j` (`kuljen`). The thirteen plain `58` members
//! gradate nothing.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "58";

/// Inflect a class-58 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    paradigm(&Stems::with_e_past(lemma, stem, gradation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    /// The words of a lemma that gradates, the code given in the sanalista's
    /// own letters: `D` for `58*D`, which is `k <-> ∅`.
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
    fn the_third_person_repeats_the_stems_own_e() {
        // `laske` + `e`, not the doubled `aa` of class 53.
        for want in [
            "laskee",
            "laskevat",
            "laskisi",
            "laskisin",
            "laskenee",
            "laskenen",
            "laskekoon",
            "laskekaamme",
            "laskekaa",
            "laskekoot",
            "laskenut",
            "laskeneet",
            "laskeva",
            "laskema",
            "laskettu",
            "laskettava",
        ] {
            assert!(has("laskea", want), "laskea should have {want}");
        }
    }

    #[test]
    fn the_past_drops_the_stems_e() {
        // `laske` -> `lask`, and the stem keeps its `e` everywhere else.
        for want in [
            "lasken",
            "lasket",
            "laskemme",
            "laskette",
            "laske",
            "lasketaan",
            "laskettiin",
            "laskettaisiin",
            "laskettaneen",
            "laskettakoon",
        ] {
            assert!(has("laskea", want), "laskea should have {want}");
        }
        for want in [
            "laskin", "laskit", "laski", "laskimme", "laskitte", "laskivat",
        ] {
            assert!(has("laskea", want), "laskea should have {want}");
        }
    }

    #[test]
    fn a_deleted_k_does_not_keep_the_weak_grade_s_e() {
        // `58*D` = `hakea`: the `e` goes before the `k` does, so the past's weak
        // grade is `ha` and not `hag`.
        for want in [
            "haen", "haet", "haemme", "haette", "hae", "hakee", "hakevat", "hakekoon", "hakisi",
            "hakenee", "haetaan", "haettiin", "hakenut", "hakeva", "haettu",
        ] {
            assert!(has_g("hakea", 'D', want), "hakea should have {want}");
        }
        for want in ["hain", "hait", "haimme", "haitte", "haki", "hakivat"] {
            assert!(has_g("hakea", 'D', want), "hakea should have {want}");
        }
        // The order the other way round would give `hagin`, which is not a word.
        assert!(!has_g("hakea", 'D', "hagin"));
    }

    #[test]
    fn p_to_v_and_front_harmony() {
        // `58*E` = `kylpeä`: `p <-> v`, and every ending takes the front twin.
        for want in [
            "kylpee",
            "kylpevät",
            "kylpisi",
            "kylpisin",
            "kylpisivät",
            "kylpenee",
            "kylpenen",
            "kylpenevät",
            "kylpeköön",
            "kylpekää",
            "kylpekäämme",
            "kylpekööt",
            "kylven",
            "kylvet",
            "kylvemme",
            "kylvette",
            "kylve",
            "kylvetään",
            "kylvettiin",
            "kylvettäisiin",
            "kylvettäneen",
            "kylvettäköön",
            "kylpenyt",
            "kylpeneet",
            "kylpevä",
            "kylpemä",
            "kylvetty",
            "kylvettävä",
            "kylvin",
            "kylvit",
            "kylvemme",
            "kylvitte",
            "kylpi",
            "kylpivät",
        ] {
            assert!(has_g("kylpeä", 'E', want), "kylpeä should have {want}");
        }
    }

    #[test]
    fn t_to_d_and_nk_to_ng() {
        // `58*F` = `kutea`: `t <-> d`, past weak `kud` beside strong `kut`.
        for want in [
            "kutee",
            "kutevat",
            "kutisi",
            "kutenee",
            "kutekoon",
            "kuden",
            "kudet",
            "kudemme",
            "kudette",
            "kude",
            "kudetaan",
            "kudettiin",
            "kutenut",
            "kuteva",
            "kudettu",
            "kudin",
            "kudit",
            "kudimme",
            "kuditte",
            "kuti",
            "kutivat",
        ] {
            assert!(has_g("kutea", 'F', want), "kutea should have {want}");
        }
        // `58*G` = `tunkea`: `nk <-> ng`, past weak `tung` beside strong `tunk`.
        for want in [
            "tunkee",
            "tunkevat",
            "tunkisi",
            "tunkenee",
            "tunkekoon",
            "tungen",
            "tunget",
            "tungette",
            "tunge",
            "tungetaan",
            "tungettiin",
            "tunkenut",
            "tunkeva",
            "tungettu",
            "tungin",
            "tungit",
            "tungimme",
            "tungitte",
            "tunki",
            "tunkivat",
        ] {
            assert!(has_g("tunkea", 'G', want), "tunkea should have {want}");
        }
    }

    #[test]
    fn k_to_j() {
        // `58*L` = `kulkea`: `k <-> j`, and here both grades keep the `e`, so the
        // ordering `hakea` exposes makes no difference: past weak `kulj`, past
        // strong `kulk`.
        for want in [
            "kulkee",
            "kulkevat",
            "kulkisi",
            "kulkenee",
            "kulkekoon",
            "kuljen",
            "kuljet",
            "kuljemme",
            "kuljette",
            "kulje",
            "kuljetaan",
            "kuljettiin",
            "kulkenut",
            "kulkeva",
            "kuljettu",
            "kuljin",
            "kuljit",
            "kuljimme",
            "kuljitte",
            "kulki",
            "kulkivat",
        ] {
            assert!(has_g("kulkea", 'L', want), "kulkea should have {want}");
        }
    }
}
