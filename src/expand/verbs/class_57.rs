//! Class 57 (the `saartaa` type) inflection, written by hand.
//!
//! Class 57 is type 1 with a long theme vowel, like class 53, and it has that
//! class's present: the stem is the infinitive without its final `a`/`ä`
//! (`saarta`, `kaata`), the third person doubles the theme vowel (`saartaa`,
//! `kaataa`) and the passive rethemes the weak grade to `e` (`saarretaan`,
//! `kaadetaan`). Its past is where it differs, and like class 55 it has *two*
//! past stems, both of them correct.
//!
//! One past is class 56's: the stem's theme vowel read as `o`/`ö`, strong in the
//! third person and weak in the others. `saarta` gives `saarto` and `saarro`,
//! so `saartoi` beside `saarroin`. The other is class 54's `s` stem: the stem
//! less its theme vowel with that final consonant replaced by `s`, one stem for
//! every past cell. `saarta` gives `saars`, so `saarsi` and `saarsin` beside
//! `saartoi` and `saarroin`. Kotus names both (`saarsi saartoi`) and the
//! reference carries both, so this module emits the union of the two past
//! paradigms — the same move as class 55's, over class 56's past rather than
//! class 53's.
//!
//! Gradation, where the class names it, is class 54's: `57*F` (`kaataa`) weakens
//! `t` to `d` (`kaadan`, `kaadetaan`) and `57*K` (`saartaa`) `rt` to `rr`
//! (`saarran`). It shows in the present and the passive only, each past running
//! on its own stems. Both pasts still read the weak grade off the present, so
//! `saartaa` has `saarsin` (`saars` from the strong `saart`, `saars` again from
//! the weak `saarr`) beside `saarroin`.

#![allow(dead_code)]

use std::collections::HashSet;

use crate::kotus::Gradation;
use crate::tags::{Form, Tags};

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "57";

/// Inflect a class-57 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    let gradation = gradation.or_else(|| missing_code(lemma));
    // The class-56 past: the stem's theme vowel read as `o`/`ö`, strong in the
    // third person and the weak grade elsewhere.
    let mut forms = paradigm(&Stems::with_o_past(lemma, stem, gradation));
    // And the class-54 `s` past beside it. The two paradigms agree on every cell
    // but the past, so the merge keeps the shared form once and adds only the
    // `s` past's own words.
    let s_past = Stems::s_past_stem_of(stem);
    let mut seen: HashSet<(String, Vec<Tags>)> = forms
        .iter()
        .map(|f| (f.word.clone(), f.readings.clone()))
        .collect();
    for form in paradigm(&Stems::with_single_past(lemma, stem, &s_past, gradation)) {
        if seen.insert((form.word.clone(), form.readings.clone())) {
            forms.push(form);
        }
    }
    forms
}

/// The gradation a class-57 lemma has but its class string does not record.
///
/// The type is modelled on `saartaa`, which gradates `rt` to `rr` (`saarran`,
/// `saarrettiin`) and whose own sanalista record is `57*K`; the catalogue names
/// the type after it under the plain `57` as well, and nothing in the word's
/// shape says which consonant weakens. Every real member of the class carries a
/// code (`57*F`, `57*K`), so only the model word is named here. This is the same
/// table as class 54's `huutaa` and class 55's `soutaa`.
fn missing_code(lemma: &str) -> Option<Gradation> {
    match lemma {
        "saartaa" => Some(Gradation(11)), // K: `rt` <-> `rr`
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    /// The words of a lemma that gradates, the code given in the sanalista's
    /// own letters: `K` for `57*K`, which is `rt <-> rr`.
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
            "saartaa",
            "saartavat",
            "saartaisin",
            "saartanen",
            "saartakoon",
            "saartanut",
            "saartava",
        ] {
            assert!(has_g("saartaa", 'K', want), "saartaa should have {want}");
        }
    }

    #[test]
    fn the_past_has_both_stems() {
        // The class-56 `o` past: `saarto` in the third person, `saarro` in the
        // other persons.
        for want in [
            "saarroin",
            "saarroit",
            "saartoi",
            "saarroimme",
            "saarroitte",
            "saartoivat",
        ] {
            assert!(has_g("saartaa", 'K', want), "saartaa should have {want}");
        }
        // And the class-54 `s` past: `saars` in every past cell.
        for want in [
            "saarsin",
            "saarsit",
            "saarsi",
            "saarsimme",
            "saarsitte",
            "saarsivat",
        ] {
            assert!(has_g("saartaa", 'K', want), "saartaa should have {want}");
        }
    }

    #[test]
    fn the_weak_grade_stays_in_the_present_and_passive() {
        for want in [
            "saarran",
            "saarrat",
            "saarramme",
            "saarratte",
            "saarra",
            "saarretaan",
            "saarrettiin",
            "saarrettaisiin",
            "saarrettaneen",
            "saarrettakoon",
        ] {
            assert!(has_g("saartaa", 'K', want), "saartaa should have {want}");
        }
        // The strong grade is the third persons, the conditional, the potential,
        // the imperative's other cells and the participles.
        for want in [
            "saartakaa",
            "saartakoot",
            "saartanut",
            "saartaneet",
            "saartama",
            "saarrettu",
            "saarrettava",
        ] {
            assert!(has_g("saartaa", 'K', want), "saartaa should have {want}");
        }
    }

    #[test]
    fn t_gradation_gives_both_pasts() {
        // `57*F` = `kaataa`: `t` <-> `d`. The `o` past is `kaado`/`kaato`, the
        // `s` past `kaas` in every cell -- both grades reach `kaas`, since
        // `kaat` and `kaad` end in the same consonant.
        for want in [
            "kaataa",
            "kaatavat",
            "kaataisin",
            "kaatanen",
            "kaatakoon",
            "kaadan",
            "kaadat",
            "kaada",
            "kaadetaan",
            "kaadettiin",
            "kaatanut",
            "kaatava",
            "kaadettu",
            "kaadoin",
            "kaadoit",
            "kaatoi",
            "kaadoimme",
            "kaadoitte",
            "kaatoivat",
            "kaasin",
            "kaasit",
            "kaasi",
            "kaasimme",
            "kaasitte",
            "kaasivat",
        ] {
            assert!(has_g("kaataa", 'F', want), "kaataa should have {want}");
        }
    }

    #[test]
    fn the_model_word_carries_its_own_gradation() {
        // `57` with no code still gradates `saartaa`, whose sanalista record is
        // `57*K`.
        for want in ["saarran", "saarrettiin", "saarsin", "saarroin", "saartoi"] {
            assert!(has("saartaa", want), "saartaa should have {want}");
        }
    }
}
