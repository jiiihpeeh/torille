//! Class 48 (the aave type) inflection, written by hand as a logical engine.
//!
//! Class 48 is the graded `-e` family (aave, aihe, aine, aate, ahde, helle,
//! aarre): the final `-e` reads a long `-ee` a-stem (`aave -> aavee`,
//! `aate -> aattee`), whose `ee` carries the singular obliques and nominative
//! plural, whose short `e` carries the plural obliques, and whose bare lemma
//! keeps the partitive (`aavetta`, `aatetta`). The graded subtypes 48\* weaken
//! the consonant before the `-e`. [`e_stem_inflect`] is the shared engine, so
//! porting it ports the family.
//!
//! What is worth naming: the two genitive alternants and two illatives the
//! plural carries, and the nominative possessives riding the long `ee` stem
//! (`aaveeni`, not the short-stem `aaveni`).

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::harmony::loanword_harmony;
use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "48";

/// The graded short stem (the `e`-stem) from a pair applied to the consonant
/// before the final `-e`: aate + t->tt -> aatte, helle + ll->lt -> helte.
pub fn e_grad(lemma: &str, from: &str, to: &str) -> String {
    let mut base = lemma.to_string();
    base.pop(); // drop the final e
    let short = if base.ends_with(from) {
        format!("{}{}e", &base[..base.len() - from.len()], to)
    } else {
        format!("{base}e")
    };
    short
}

/// The shared long-`-ee` engine given the short `e`-stem.
pub fn e_stem_inflect(lemma: &str, possessives: bool, short: &str) -> Vec<Form> {
    let long = format!("{short}{}", short.chars().last().unwrap());
    let front = loanword_harmony(lemma)
        // The compound `-e` heads (ajanviete, toimenpide, joulukiire) set front.
        || ["viete", "pide", "kiire", "viive"]
            .iter()
            .any(|h| lemma.ends_with(h));
    let mut b = Builder::new_harmony(lemma, possessives, front);
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the long ee-stem ----
            // The nominative possessives sit on the strong `ee` stem (aave + ni
            // -> aaveeni), not on the weak/short `e` stem.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(long.clone(), ["", ""]),
            cell(Case::Genitive, sg, &long, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, lemma, ["tta", "ttä"], true),
            cell(Case::Essive, sg, &long, ["na", "nä"], true),
            cell(Case::Translative, sg, &long, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, &long, ["seen", "seen"], false).on_ending(["see", "see"]),
            cell(Case::Inessive, sg, &long, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &long, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &long, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &long, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &long, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &long, ["tta", "ttä"], true),
            // ---- the plural, on the short e-stem ----
            cell(Case::Nominative, pl, &long, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, short, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, short, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, short, ["ita", "itä"], true),
            cell(Case::Essive, pl, short, ["ina", "inä"], true),
            cell(Case::Translative, pl, short, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, short, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Illative, pl, short, ["isiin", "isiin"], false).on_ending(["isii", "isii"]),
            cell(Case::Inessive, pl, short, ["issa", "issä"], true),
            cell(Case::Elative, pl, short, ["ista", "istä"], true),
            cell(Case::Adessive, pl, short, ["illa", "illä"], true),
            cell(Case::Ablative, pl, short, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, short, ["ille", "ille"], true),
            cell(Case::Abessive, pl, short, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, short, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(short);

    b.into_forms()
}

/// Inflect a plain class-48 noun (no gradation): the short stem is the lemma.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Class 48 also carries plural-only lemmas whose citation form is the
    // nominative plural (`tamineet`). Drop the `-t`, use the remaining long
    // `-ee` stem (`taminee`) and keep only the plural cells:
    // `tamineita`, not `tamineettta`.
    if let Some(long) = lemma.strip_suffix('t') {
        let short = match long.chars().last() {
            Some(c) if "aeiouyäö".contains(c) => &long[..long.len() - c.len_utf8()],
            _ => long,
        };
        let mut forms = e_stem_inflect(lemma, possessives, short);
        forms.retain(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        });
        return forms;
    }
    // `viive` (class 48*(E), the *viive* type) gradates v -> p in its oblique
    // and plural cells (`viipeen`, `viipeiksi`) while keeping the lemma in the
    // nominative singular and partitive singular (`viive`, `viivettä`) -- the
    // very shape this engine's `e_stem_inflect` produces from the graded short
    // stem. Its class sibling `häive` however does not gradate at all
    // (`häiveen`, `häiveeksi`); the reference confirms the two behave
    // differently despite sharing class 48*(E), which the sanalista parser
    // collapses to plain 48. So `viive` alone takes the graded stem.
    if lemma == "viive" {
        let short = e_grad(lemma, "v", "p");
        return e_stem_inflect(lemma, possessives, &short);
    }
    let short = &lemma[..lemma.len() - 1]; // drop the final e -> aav
    let short = format!("{short}e"); // aave
    e_stem_inflect(lemma, possessives, &short)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn a_plural_only_lemma_declines_from_the_dropped_stem() {
        let w = words("tamineet");
        for want in ["tamineet", "tamineiden", "tamineita", "tamineissa"] {
            assert!(w.contains(&want.to_string()), "missing {want}");
        }
        for bad in ["tamineettta", "tamineetn"] {
            assert!(!w.contains(&bad.to_string()), "invented {bad}");
        }
    }

    #[test]
    fn aave() {
        let w = words("aave");
        assert!(w.contains(&"aaveen".to_string()));
        assert!(w.contains(&"aavetta".to_string()));
        assert!(w.contains(&"aaveena".to_string()));
        assert!(w.contains(&"aaveet".to_string()));
        assert!(w.contains(&"aaveiden".to_string()));
        assert!(w.contains(&"aaveita".to_string()));
    }

    /// The nominative possessives ride the long `ee` stem, and the plural
    /// carries both genitive alternants and both illatives.
    #[test]
    fn the_long_stem_possessive_and_the_plural_alternants() {
        let w = words("aave");
        assert!(
            w.contains(&"aaveeni".to_string()),
            "long-stem possessive missing"
        );
        assert!(
            !w.contains(&"aaveni".to_string()),
            "short-stem possessive emitted"
        );
        assert!(w.contains(&"aaveitten".to_string()));
        assert!(w.contains(&"aaveisiin".to_string()));
    }
}
