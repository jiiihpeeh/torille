//! Class 15 (the apea/hopea type) inflection, written by hand as a logical
//! engine.
//!
//! Class 15 is the `-ea`/`-eä` family (apea, hopea, aukea). The singular is the
//! lemma as its stem; the plural drops the stem's final `a`/`ä` (`apea -> ape`)
//! and carries the `i`-plural -- `apeiden`/`apeitten`, `apeita`, `apeissa`.
//! The nominative plural keeps the plain lemma (`apeat`), and the illative
//! offers both `apeihin` and `apeisiin`.
//!
//! The short genitive plural (`korkeain`) is a rare bare form and the engine
//! emits it *after* the comitative, so it is a second `decline` pass rather
//! than an entry of the one `Decl` -- the class_9 precedent.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain, rare};

pub const CLASS: &str = "15";

include!("class_15_long_partitive.rs");

/// Inflect a class-15 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    let mut pstem = stem.to_string();
    pstem.pop(); // drop the final a/ä: apea -> ape
    let last = stem.chars().last().unwrap();
    let (sg, pl) = (Number::Singular, Number::Plural);
    let long_partitive = cell(Case::Partitive, sg, stem, ["ta", "tä"], true);
    let long_partitive = if LONG_PARTITIVE.binary_search(&lemma).is_ok() {
        long_partitive
    } else {
        long_partitive.as_rare()
    };

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the stem ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, stem, ["a", "ä"], true),
            // The long partitive -ta is plain only for the lemmas the
            // reference confirms (korkeata); the rest keep it rare.
            long_partitive,
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
            // The illative doubles the stem's last letter: apea -> apeaan.
            cell(
                Case::Illative,
                sg,
                stem,
                Ending::Given(format!("{last}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{last}"))),
            cell(Case::Adessive, sg, stem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, stem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, stem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, stem, ["tta", "ttä"], true),
            // ---- the plural, on the dropped stem with the i-plural ----
            cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, &pstem, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, &pstem, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, &pstem, ["ita", "itä"], true),
            cell(Case::Essive, pl, &pstem, ["ina", "inä"], true),
            cell(Case::Translative, pl, &pstem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &pstem, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Illative, pl, &pstem, ["isiin", "isiin"], false).on_ending(["isii", "isii"]),
            cell(Case::Inessive, pl, &pstem, ["issa", "issä"], true),
            cell(Case::Elative, pl, &pstem, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &pstem, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &pstem, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &pstem, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &pstem, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &pstem, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&pstem);
    // The short genitive plural is rare: generated, never offered (korkeain).
    // It comes after the comitative, so it is a second pass.
    b.decline(
        &Decl::new(vec![rare(Case::Genitive, pl, stem, ["in", "in"])]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn i_plural_on_dropped_stem() {
        let w = words("apea");
        assert!(w.contains(&"apean".to_string()));
        assert!(w.contains(&"apeaa".to_string()));
        assert!(w.contains(&"apeat".to_string())); // nom pl keeps lemma
        assert!(w.contains(&"apeiden".to_string()));
        assert!(w.contains(&"apeita".to_string()));
        assert!(w.contains(&"apeissa".to_string()));
    }

    /// The short genitive plural is rare and comes after the comitative.
    #[test]
    fn the_short_genitive_plural_comes_after_the_comitative() {
        let forms = inflect("apea", true);
        let short = forms
            .iter()
            .position(|f| f.word == "apeain")
            .expect("apeain");
        let comitative = forms
            .iter()
            .position(|f| f.readings.iter().any(|t| t.case == Some(Case::Comitative)))
            .expect("a comitative");
        assert!(
            short > comitative,
            "short at {short}, comitative at {comitative}"
        );
    }
    /// `ankea` confirms the long partitive, `vihreä` does not; the latter
    /// must keep `vihreätä` accepted but repaired.
    #[test]
    fn the_long_partitive_is_plain_only_where_confirmed() {
        let ankea = inflect("ankea", true);
        assert!(ankea.iter().any(|f| f.word == "ankeata" && f.plain()));
        let vihrea = inflect("vihreä", true);
        assert!(!vihrea.iter().any(|f| f.word == "vihreätä" && f.plain()));
    }

}
