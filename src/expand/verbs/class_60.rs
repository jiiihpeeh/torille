//! Class 60 (the `lähteä` type) inflection, written by hand.
//!
//! Class 60 is class 59's type -- theme vowel `e`, so the conditional and the
//! second infinitive follow the `i`-stem (`lähtisi`, `lähtiessä`, `lähtien`) --
//! with a past that has *two* stems, both correct. Kotus names them on one line
//! (`lähti (läksi)`) and the reference carries all twelve past cells, so the
//! module is the union of the two paradigms, as class 57's is: the class-58
//! `i`-past that gradates (`lähti`, `lähtivät` beside `lähdin`, `lähdit`)
//! together with the class-59 `s` past that does not (`läksi`, `läksivät`,
//! `läksin`, `läksit`). Every other cell the two agree on, so the merge keeps
//! the shared form once and adds only the `s` past's own words.
//!
//! The class has one member, `lähteä`, and it is `60*F`: `t` weakened to `d`.
//! That gradation is what carries the present and the passive (`lähden`,
//! `lähtevä`; `lähdetään`, `lähdettiin`) and it reaches the `i`-past but not the
//! `s` past, whose stem `läks` has no `t` left to weaken -- so `lähdin` sits
//! beside `läksin` with nothing between them. `lähteä` is also the type's model
//! word in the catalogue under the plain `60`, and nothing in the word's shape
//! says which consonant weakens, so `missing_code` names it.
//!
//! The parenthetical in `lähti (läksi)` is not one of the alternant *class*
//! readings that `parse_inflections` drops: the sanalista gives `lähteä` the
//! one class `60*F`, and the two pasts are two stems of that one type, which is
//! the same arrangement as class 57's `saarsi saartoi`.
//!
//! The second of the two is lexical. `lähteä` gives `läks`, which class 59's
//! letter rule cannot reach -- it turns a final consonant to `s` and this stem
//! needs `ht` -> `ks`, with no `k` anywhere in it -- so `s_past_stem` gives the
//! stem per lemma, which is what Finnish grammars do when they call `mennä`
//! `meni`, `tulla` `tuli` and `lähteä` `läksi` irregular.

#![allow(dead_code)]

use std::collections::HashSet;

use crate::kotus::Gradation;
use crate::tags::{Form, Tags};

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "60";

/// Inflect a class-60 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    let gradation = gradation.or_else(|| missing_code(lemma));
    // The class-58 `i` past, which gradates: `lähte` -> `läht` -> `lähd` weak.
    let mut forms = paradigm(&Stems::with_e_past(lemma, stem, gradation));
    // And the class-59 `s` past beside it, which does not gradate: `läks`.
    let s_past = s_past_stem(lemma, stem);
    let mut seen: HashSet<(String, Vec<Tags>)> = forms
        .iter()
        .map(|f| (f.word.clone(), f.readings.clone()))
        .collect();
    for form in paradigm(&Stems::with_e_s_past(lemma, stem, &s_past, gradation)) {
        if seen.insert((form.word.clone(), form.readings.clone())) {
            forms.push(form);
        }
    }
    forms
}

/// The second past stem, where the type spells it and the stem's shape will not.
///
/// Class 59's rule turns the stem's final consonant to `s`, which gives `tunte`
/// -> `tunt` -> `tuns`. A `mennä`-type verb's second past is not derivable that
/// way: `lähteä` gives `läks`, which needs `ht` -> `ks`, and the `k` has no
/// source anywhere in the stem. These are the verbs Finnish grammars list as
/// irregular -- `mennä` `meni`, `tulla` `tuli`, `lähteä` `läksi` -- so the stem
/// is lexical and is given here per lemma. The fallback is class 59's rule, for
/// a member of the class that turns out to spell its `s` past regularly.
fn s_past_stem(lemma: &str, stem: &str) -> String {
    match lemma {
        "lähteä" => "läks".to_string(),
        _ => Stems::e_s_past_stem_of(stem),
    }
}

/// The gradation a class-60 lemma has but its class string does not record.
///
/// The type is modelled on `lähteä`, whose own sanalista record is `60*F`, and
/// the catalogue names the type after it under the plain `60` as well. The class
/// has no other member, so this is the whole table, as for class 59's `tuntea`.
fn missing_code(lemma: &str) -> Option<Gradation> {
    match lemma {
        "lähteä" => Some(Gradation(6)), // F: `t` <-> `d`
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    fn has(lemma: &str, form: &str) -> bool {
        words(lemma).iter().any(|w| w == form)
    }

    fn hasnt(lemma: &str, form: &str) -> bool {
        !has(lemma, form)
    }

    #[test]
    fn both_pasts_are_there_and_each_is_complete() {
        // The class-58 `i` past, strong in the third persons and weak in the
        // rest, which is where the `60*F` gradation shows.
        for want in [
            "lähdin",
            "lähdit",
            "lähdimme",
            "lähditte",
            "lähti",
            "lähtivät",
        ] {
            assert!(has("lähteä", want), "lähteä should have {want}");
        }
        // And the class-59 `s` past, which has no weak grade of its own.
        for want in [
            "läksin",
            "läksit",
            "läksimme",
            "läksitte",
            "läksi",
            "läksivät",
        ] {
            assert!(has("lähteä", want), "lähteä should have {want}");
        }
        // The `s` past's stem has no `t` left, so it does not gradate either.
        for absent in ["läksdin", "läksdit", "läksdimme", "läksditti"] {
            assert!(hasnt("lähteä", absent), "lähteä should not have {absent}");
        }
    }

    #[test]
    fn t_to_d_gradates_the_present_and_the_passive() {
        // `60*F` = `t` <-> `d`: weak `lähde` beside strong `lähte`.
        for want in [
            "lähden",
            "lähdet",
            "lähdemme",
            "lähdette",
            "lähde",
            "lähtee",
            "lähtevät",
            "lähtenee",
            "lähteköön",
            "lähtekää",
            "lähtekäämme",
            "lähtekööt",
            "lähdetään",
            "lähdettiin",
            "lähdettäisiin",
            "lähdettäneen",
            "lähdettäköön",
            "lähteneet",
            "lähtemin",
            "lähtemä",
            "lähdetty",
            "lähtevä",
            "lähdettävä",
        ] {
            assert!(has("lähteä", want), "lähteä should have {want}");
        }
    }

    #[test]
    fn the_conditional_and_the_second_infinitive_follow_the_i_stem() {
        // Class 58's rule: `läht` + `isi`, `lähtie` + `ssä` and `lähtie` + `n`.
        for want in [
            "lähtisi",
            "lähtisin",
            "lähtisimme",
            "lähtisivät",
            "lähtiessä",
            "lähtien",
        ] {
            assert!(has("lähteä", want), "lähteä should have {want}");
        }
        // Only once each, even though both pasts carry them.
        assert_eq!(
            words("lähteä").iter().filter(|w| *w == "lähtisi").count(),
            1,
            "lähtisi should be offered once"
        );
        for absent in ["lähteen", "lähteessä", "lähteisi"] {
            assert!(hasnt("lähteä", absent), "lähteä should not have {absent}");
        }
    }

    #[test]
    fn the_participle_takes_the_strong_grade_and_the_front_twin() {
        // `lähteneet` and `lähtemin` are strong; `lähtenyt` is the front `-nyt`.
        for want in ["lähtenyt", "lähteneet", "lähtemä", "lähtemät", "lähtemätön"] {
            assert!(has("lähteä", want), "lähteä should have {want}");
        }
    }
}
