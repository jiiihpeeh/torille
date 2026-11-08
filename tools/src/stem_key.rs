// SPDX-License-Identifier: MIT OR Apache-2.0
//! Recover the stem key a nominal form was built on.
//!
//! This is the inverse of the forward lookup in
//! `src/expand/check_normal.rs`: the generator has an inflected form plus its
//! reading and needs the `(stem, base)` key the stem map will be searched by.
//! The case/number table below is the generator side of the same morphology;
//! the runtime's flat `ENDINGS` list is the search side.

use torille::tags::{Case, Number, Possessive};

/// Endings by `(case, number)`, longest first so the canonical split wins.
fn endings(case: Case, number: Number) -> Option<&'static [&'static str]> {
    Some(match (case, number) {
        (Case::Nominative, Number::Singular) => &[""],
        (Case::Nominative, Number::Plural) => &["t"],
        (Case::Genitive, Number::Singular) => &["n"],
        (Case::Genitive, Number::Plural) => &["en", "jen", "ten", "den"],
        (Case::Partitive, Number::Singular) => &["tta", "ttä", "ta", "tä", "a", "ä"],
        (Case::Partitive, Number::Plural) => &["ja", "jä", "a", "ä"],
        (Case::Illative, Number::Singular) => &["seen", "hon", "hön", "en", "an", "än", "in"],
        (Case::Illative, Number::Plural) => &["ihin", "siin", "in"],
        (Case::Inessive, Number::Singular) => &["ssä", "ssa"],
        (Case::Inessive, Number::Plural) => &["issä", "issa"],
        (Case::Elative, Number::Singular) => &["stä", "sta"],
        (Case::Elative, Number::Plural) => &["istä", "ista"],
        (Case::Adessive, Number::Singular) => &["llä", "lla"],
        (Case::Adessive, Number::Plural) => &["illä", "illa"],
        (Case::Ablative, Number::Singular) => &["ltä", "lta"],
        (Case::Ablative, Number::Plural) => &["iltä", "ilta"],
        (Case::Abessive, Number::Singular) => &["tta", "ttä"],
        (Case::Abessive, Number::Plural) => &["itta", "ittä"],
        (Case::Allative, Number::Singular) => &["lle"],
        (Case::Allative, Number::Plural) => &["ille"],
        (Case::Essive, Number::Singular) => &["nä", "na"],
        (Case::Essive, Number::Plural) => &["inä", "ina"],
        (Case::Translative, Number::Singular) => &["ksi"],
        (Case::Translative, Number::Plural) => &["iksi"],
        (Case::Instructive, Number::Plural) => &["in"],
        _ => return None,
    })
}

/// The possessive suffix spellings to try, third person with both harmonies.
fn possessive_suffixes(possessive: Possessive) -> &'static [&'static str] {
    match possessive {
        Possessive::FirstSingular => &["ni"],
        Possessive::SecondSingular => &["si"],
        Possessive::Third => &["nsa", "nsä"],
        Possessive::FirstPlural => &["mme"],
        Possessive::SecondPlural => &["nne"],
    }
}

/// Recover the stem from a form and its reading.
///
/// Returns `None` when no ending matches; such a row is dropped from the map.
pub fn nominal_stem_key(
    form: &str,
    case: Case,
    number: Number,
    possessive: Option<Possessive>,
) -> Option<String> {
    let mut working = form;
    if let Some(possessive) = possessive {
        let suffix = possessive_suffixes(possessive)
            .iter()
            .find(|suffix| working.len() > suffix.len() && working.ends_with(*suffix))?;
        working = &working[..working.len() - suffix.len()];
    }
    let endings = endings(case, number)?;
    for ending in endings {
        if ending.is_empty() {
            return Some(working.to_string());
        }
        if working.len() > ending.len() && working.ends_with(ending) {
            return Some(working[..working.len() - ending.len()].to_string());
        }
    }
    // A nominative plural without `-t`: suppletive pronouns and `-toista`
    // numerals. Nothing strips, so the whole form is the key.
    if case == Case::Nominative && possessive.is_none() {
        return Some(working.to_string());
    }
    None
}
