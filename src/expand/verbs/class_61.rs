//! Class 61 (the `sallia` type) inflection, written by hand.
//!
//! Class 61 is the `-i` type, and one thing about it is enough to make it a type
//! of its own: the theme vowel is *kept*. In classes 52 and 53 the infinitive's
//! ending `aa` is a doubled theme, so the stem ends in the theme and the past
//! drops it (`aakkosta` -> `aakkost`), and the passive rethemes it to `e`
//! (`aakkostetaan`). Here the infinitive's ending is a single `ia`, the stem is
//! `salli` and it *is* the past stem too -- `salli` + `i` gives `salli`, because
//! the ending's `i` merges with the one the stem already ends in. That is
//! `paradigm`'s `absorb_i`, and it is the whole difference: `Stems::with_past`
//! is handed the stem twice.
//!
//! Two of the six present and past cells therefore come out as the same word in
//! both tenses, because they are the same letters: `sallia` has `sallin` and
//! `sallin`, `sallit` and `sallit`, `sallivat` and `sallivat`, and the
//! reference gives no other present or past cell for this type either. The one
//! place the two tenses part company is the third person singular, `sallii`
//! against `salli`, and it is the third person present that is the one place a
//! `-i` verb repeats its stem's last letter.
//!
//! The passive has no `e` of its own to retheme into, because the stem's `i` is
//! already the passive's: `sallitaan` is `salli` + `t` + `aan` where class 53
//! gives `aakkostetaan` and class 58 `lasketaan`. `retheme` finds no `a`/`ä` to
//! change and leaves the weak grade standing, which is why the passive is the
//! one place the gradation shows twice over -- `sovitaan` for `sopia`, `lehditään`
//! for `lehtiä`.
//!
//! The conditional and the second infinitive stay on the theme, as in class 52,
//! because a stem ending in `i` can take their endings: `sallisi` is `salli` +
//! `isi` with the `i`s merged, and `salliessa` and `sallien` are `sallie` + `ssa`
//! and `sallie` + `n`. Only classes 58 and 59, whose theme is the stem's own
//! final `e`, have to move those two onto the `i`-stem instead.
//!
//! Gradation is the usual type-1 run and reaches the same cells as everywhere:
//! `61*A` (`kukkia`) `kk` -> `k` (`kukin`), `61*B` (`oppia`) `pp` -> `p`
//! (`opin`), `61*C` (`miettiä`) `tt` -> `t` (`mietin`), `61*D` (`poikia`) the
//! `k` deleted (`poiin`), `61*E` (`sopia`) `p` -> `v` (`sovin`), `61*F`
//! (`lehtiä`) `t` -> `d` (`lehdin`), `61*G` (`hönkiä`) `nk` -> `ng` (`höngin`),
//! `61*H` (`empiä`) `mp` -> `mm` (`emmin`), `61*J` (`sontia`) `nt` -> `nn`
//! (`sonnin`) and `61*L` (`hylkiä`) `k` -> `j` (`hyljin`).

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "61";

/// Inflect a class-61 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    // The stem is the past stem: this type does not drop its theme vowel, so the
    // `i` of the past endings merges with the stem's own instead.
    paradigm(&Stems::with_past(lemma, stem, stem, gradation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    /// The words of a lemma that gradates, the code given in the sanalista's own
    /// letters: `A` for `61*A`, which is `kk <-> k`.
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
    fn the_stem_keeps_its_i_and_the_third_person_repeats_it() {
        // `salli` + `i` = `sallii`, where class 52's doubled theme would have
        // repeated an `a`.
        for want in [
            "sallii",
            "sallisi",
            "sallisin",
            "sallisimme",
            "sallinee",
            "sallikoon",
            "sallikaa",
            "sallikaamme",
            "sallikoot",
            "sallinut",
            "sallineet",
            "salliva",
            "sallima",
        ] {
            assert!(has("sallia", want), "sallia should have {want}");
        }
    }

    #[test]
    fn the_past_merges_its_i_with_the_stems() {
        // `salli` + `i` = `salli`, not `sallii`, so the third person past is the
        // bare stem. Every other cell is the present's word over again, because
        // the past stem *is* the present stem.
        for want in [
            "salli", "sallin", "sallimme", "sallit", "sallitte", "sallivat",
        ] {
            assert!(has("sallia", want), "sallia should have {want}");
        }
        // The one cell where the two tenses part company: the same stem and the
        // same ending, giving two words.
        assert!(has("sallia", "sallii"));
    }

    #[test]
    fn the_passive_has_no_e_of_its_own() {
        // The stem's `i` is the passive's, so the theme is the weak grade as it
        // stands: `salli` + `t` + `aan`, beside class 53's `aakkostetaan`.
        for want in [
            "sallitaan",
            "sallittiin",
            "sallittaisiin",
            "sallittaneen",
            "sallittakoon",
            "sallittava",
            "sallittu",
        ] {
            assert!(has("sallia", want), "sallia should have {want}");
        }
        // And the passive second infinitive runs on it too.
        for want in ["salliessa", "sallien", "sallittaessa", "sallittaen"] {
            assert!(has("sallia", want), "sallia should have {want}");
        }
    }

    /// One lemma for each gradation code the class uses, checked in the four
    /// cells that show it: the present's weak grade, the third person strong, the
    /// imperative's second person singular and the passive.
    #[test]
    fn every_gradation_code_of_the_class() {
        for (lemma, code, weak, strong, passive) in [
            ("kukkia", 'A', "kukin", "kukkii", "kukitaan"), // kk -> k
            ("oppia", 'B', "opin", "oppii", "opitaan"),     // pp -> p
            ("miettiä", 'C', "mietin", "miettii", "mietitään"), // tt -> t
            ("poikia", 'D', "poiin", "poikii", "poiitaan"), // k deleted
            ("sopia", 'E', "sovin", "sopii", "sovitaan"),   // p -> v
            ("lehtiä", 'F', "lehdin", "lehtii", "lehditään"), // t -> d
            ("hönkiä", 'G', "höngin", "hönkii", "höngitään"), // nk -> ng
            ("empiä", 'H', "emmin", "empii", "emmitään"),   // mp -> mm
            ("sontia", 'J', "sonnin", "sontii", "sonnitaan"), // nt -> nn
            ("hylkiä", 'L', "hyljin", "hylkii", "hyljitään"), // k -> j
        ] {
            assert!(
                has_g(lemma, code, weak),
                "{lemma} ({code}) should have {weak}"
            );
            assert!(
                has_g(lemma, code, strong),
                "{lemma} ({code}) should have {strong}"
            );
            assert!(
                has_g(lemma, code, passive),
                "{lemma} ({code}) should have {passive}"
            );
            // The imperative's second person singular is the weak grade bare.
            let bare = weak.strip_suffix('n').unwrap_or(weak);
            assert!(
                has_g(lemma, code, bare),
                "{lemma} ({code}) should have the bare weak grade {bare}"
            );
        }
    }

    #[test]
    fn the_past_weak_grade_is_the_presents() {
        // `lehtiä` has `lehdin` for both tenses' first person singular, where
        // class 58's `hakea` had two different words (`hain`, `haen`).
        for want in [
            "lehdin",
            "lehdit",
            "lehdimme",
            "lehditte",
            "lehti",
            "lehtivät",
        ] {
            assert!(has_g("lehtiä", 'F', want), "lehtiä should have {want}");
        }
        // The third person keeps the strong grade in both tenses.
        assert!(has_g("lehtiä", 'F', "lehtii"));
        assert!(has_g("lehtiä", 'F', "lehtivät"));
    }
}
