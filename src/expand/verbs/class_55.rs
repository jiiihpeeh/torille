//! Class 55 (the `soutaa` type) inflection, written by hand.
//!
//! Class 55 is type 1 with a long theme vowel, like class 53, and it has that
//! class's present: the stem is the infinitive without its final `a`/`ä`
//! (`souta`, `kiitä`, `yltä`), the third person doubles the theme vowel
//! (`soutaa`, `kiitää`, `yltää`) and the passive rethemes the weak grade to `e`
//! (`soudetaan`, `kiidetään`, `ylletään`). Its past is where it differs, and it
//! is the only class with *two* past stems, both of them correct.
//!
//! One past is class 53's: the present stem less its theme vowel, strong in the
//! third person and weak in the other persons. `souta` gives `sout`, so `souti`
//! and `soutivat` beside `soudin` and `soudimme`. The other is class 54's `s`
//! stem: the same base with that final consonant replaced by `s`, one stem for
//! every past cell. `souta` gives `sous`, so `sousi` and `sousin` beside
//! `souti` and `soudin`. Kotus names both (`souti sousi`) and the reference
//! carries both, so this module emits the union of the two past paradigms.
//!
//! Gradation, where the class names it, is class 54's: `55*F` (`soutaa`)
//! weakens `t` to `d` (`soudan`, `soudetaan`), `55*I` (`yltää`) `lt` to `ll`
//! (`yllän`) and `55*J` (`entää`) `nt` to `nn` (`ennän`). It shows in the
//! present and the passive only, the past running on its own stems.

#![allow(dead_code)]

use std::collections::HashSet;

use crate::kotus::Gradation;
use crate::tags::{Form, Tags};

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "55";

/// Inflect a class-55 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    let gradation = gradation.or_else(|| missing_code(lemma));
    // The class-53 past: the present stem less its theme vowel, strong in the
    // third person and the weak grade elsewhere.
    let mut forms = paradigm(&Stems::with_past(
        lemma,
        stem,
        &Stems::past_stem_of(stem),
        gradation,
    ));
    // And the class-54 `s` past beside it. The two paradigms agree on every
    // cell but the past, so the merge keeps the shared form once and adds only
    // the `s` past's own words.
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

/// The gradation a class-55 lemma has but its class string does not record.
///
/// The type is modelled on `soutaa`, which gradates `t` to `d` (`soudan`,
/// `soudettiin`) and whose own sanalista record is `55*F`; the catalogue names
/// the type after it under the plain `55` as well, and nothing in the word's
/// shape says which consonant weakens. Every real member of the class carries a
/// code (`55*F`, `55*I`, `55*J`), so only the model word is named here. This is
/// the same table as class 54's `huutaa`.
fn missing_code(lemma: &str) -> Option<Gradation> {
    match lemma {
        "soutaa" => Some(Gradation(6)),
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
    /// own letters: `F` for `55*F`, which is `t <-> d`.
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
            "soutaa",
            "soutavat",
            "soutaisin",
            "soutanen",
            "soutakoon",
            "soutanut",
            "soutava",
        ] {
            assert!(has_g("soutaa", 'F', want), "soutaa should have {want}");
        }
    }

    #[test]
    fn the_past_has_both_stems() {
        // The weak-grade past beside the `s` past: `soudin` and `sousin`, the
        // third person strong as `souti` beside `sousi`.
        for want in [
            "soudin", "soudit", "souti", "soudimme", "souditte", "soutivat",
        ] {
            assert!(has_g("soutaa", 'F', want), "soutaa should have {want}");
        }
        for want in [
            "sousin", "sousit", "sousi", "sousimme", "sousitte", "sousivat",
        ] {
            assert!(has_g("soutaa", 'F', want), "soutaa should have {want}");
        }
    }

    #[test]
    fn the_weak_grade_stays_in_the_present_and_passive() {
        for want in [
            "soudan",
            "soudat",
            "soudamme",
            "soudatte",
            "soudetaan",
            "soudettiin",
        ] {
            assert!(has_g("soutaa", 'F', want), "soutaa should have {want}");
        }
    }

    #[test]
    fn lt_gradation_and_front_harmony() {
        // `55*I` = `yltää`: `lt` <-> `ll`, pasts `yllin`/`ylsin`.
        for want in [
            "yltää",
            "yllän",
            "yllät",
            "yllämme",
            "yllätte",
            "yltävät",
            "ylletään",
            "yllettiin",
            "yllin",
            "yllit",
            "yllimme",
            "yllitte",
            "ylti",
            "yltivät",
            "ylsin",
            "ylsit",
            "ylsimme",
            "ylsitte",
            "ylsi",
            "ylsivät",
        ] {
            assert!(has_g("yltää", 'I', want), "yltää should have {want}");
        }
    }

    #[test]
    fn nt_gradation() {
        // `55*J` = `entää`: `nt` <-> `nn`, pasts `ennin`/`ensin`.
        for want in [
            "entää",
            "ennän",
            "ennät",
            "ennämme",
            "ennätte",
            "entävät",
            "ennetään",
            "ennettiin",
            "ennin",
            "ennit",
            "ennimme",
            "ennitte",
            "enti",
            "entivät",
            "ensin",
            "ensit",
            "ensimme",
            "ensitte",
            "ensi",
            "ensivät",
        ] {
            assert!(has_g("entää", 'J', want), "entää should have {want}");
        }
    }

    #[test]
    fn the_model_word_carries_its_own_gradation() {
        // `55` with no code still gradates `soutaa`, whose sanalista record is
        // `55*F`.
        assert!(has("soutaa", "soudan"));
        assert!(has("soutaa", "sousin"));
    }
}
