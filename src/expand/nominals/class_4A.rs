//! Class 4*A (the aallokko type) --- class 4's `-kko` family with `kk -> k`
//! consonant gradation and the full plural, as a logical engine.
//!
//! Same shape as class 1*A (akku): the strong stem is the lemma (`aallokko`),
//! the weak stem reads its geminate once (`aalloko`), strong feeds the
//! nominative/partitive/essive/illative/genitive-plural, weak feeds the
//! genitive, the local cases and the nominative-plural.
//!
//! What sets class 4*A apart from 1*A is the plural. Class 4*A offers, beside
//! the strong vowel-plural (`aallokkoja`/`aallokkojen`), the `i`-plural on the
//! weak stem (`aallokoita`, `aallokoiden`/`aallokoitten`), exactly as the
//! reference lists for these `-kko` nouns, and the weak essive/illative plural
//! (`aallokoina`, `aallokoihin`) beside the strong ones. Those extras are why
//! it does not route through the shared `gradated.rs` grid.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain};

pub const CLASS: &str = "4*A";

/// The gradation this class carries, from the spec in `kotus`: A = type 1,
/// `kk -> k`, so aallokko -> aalloko.
const GRADATION: Gradation = Gradation(1);

/// The weak stem: aallokko -> aalloko.
fn weak_stem(lemma: &str) -> String {
    GRADATION.apply(lemma, Grade::Weak)
}

/// Inflect a class-4A noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let strong = lemma;
    let weak = weak_stem(lemma);
    let last = strong.chars().last().unwrap();
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- singular, strong grade ----
            cell(Case::Nominative, sg, strong, ["", ""], false).on_stem(),
            cell(Case::Partitive, sg, strong, ["a", "ä"], true),
            cell(Case::Essive, sg, strong, ["na", "nä"], true),
            // The illative doubles the strong stem's last letter: aallokkoon.
            cell(
                Case::Illative,
                sg,
                strong,
                Ending::Given(format!("{last}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{last}"))),
            // ---- singular, weak grade ----
            // The genitive is weak but its possessives sit on the strong stem.
            cell(Case::Genitive, sg, &weak, ["n", "n"], false)
                .base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, sg, &weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, &weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &weak, ["tta", "ttä"], true),
            // ---- plural, strong grade ----
            cell(Case::Genitive, pl, strong, ["jen", "jen"], false).on_ending(["je", "je"]),
            cell(Case::Partitive, pl, strong, ["ja", "jä"], true),
            cell(Case::Essive, pl, strong, ["ina", "inä"], true),
            cell(Case::Illative, pl, strong, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            // ---- plural, the weak-grade alternants of the essive and illative ----
            cell(Case::Essive, pl, &weak, ["ina", "inä"], true),
            cell(Case::Illative, pl, &weak, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            // ---- plural, the `i`-plural on the weak grade ----
            cell(Case::Genitive, pl, &weak, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, &weak, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, &weak, ["ita", "itä"], true),
            // ---- plural, weak grade ----
            // The nominative is weak but its possessives sit on the strong stem.
            cell(Case::Nominative, pl, &weak, ["t", "t"], false)
                .base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, pl, &weak, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Inessive, pl, &weak, ["issa", "issä"], true),
            cell(Case::Elative, pl, &weak, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &weak, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &weak, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &weak, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &weak, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &weak, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(strong);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn kk_to_k_gradation() {
        let w = words("aallokko");
        assert!(w.contains(&"aallokon".to_string())); // gen, weak
        assert!(w.contains(&"aallokossa".to_string())); // iness, weak
        assert!(w.contains(&"aallokkoa".to_string())); // part, strong
        assert!(w.contains(&"aallokkoon".to_string())); // illat, strong
    }

    #[test]
    fn weak_stem_i_plural() {
        let w = words("aallokko");
        assert!(w.contains(&"aallokkoja".to_string())); // strong vowel-plural
        assert!(w.contains(&"aallokoita".to_string())); // weak i-plural
        assert!(w.contains(&"aallokoiden".to_string()));
        assert!(w.contains(&"aallokoitten".to_string()));
    }

    #[test]
    fn weak_essive_and_illative_plural() {
        let w = words("aallokko");
        assert!(w.contains(&"aallokkoina".to_string())); // strong essive plural
        assert!(w.contains(&"aallokoina".to_string())); // weak essive plural
        assert!(w.contains(&"aallokkoihin".to_string())); // strong illative plural
        assert!(w.contains(&"aallokoihin".to_string())); // weak illative plural
    }
}
