//! Class 32*C (the espanjatar type) inflection, written by hand as a logical
//! engine.
//!
//! The `-tar`/`-tär` feminine nouns (espanjatar, haltiatar) geminate the `t`
//! of their tail in the oblique stem (`espanjatar -> espanjattare`). The
//! singular oblique cases sit on that stem and the partitive on the lemma
//! (`espanjatarta`); the plural sits on the bare graded stem
//! (`espanjattarien`, `espanjattaria`), except the `-ten` genitive plural on
//! the lemma (`espanjatarten`).
//!
//! Three stems, not the two of base class 32, which is why this module does
//! not route through `class_32::inflect_with_stems`: the singular partitive and
//! the `-ten` genitive plural take the bare lemma, the plural's `i`-cases take
//! the geminated stem with its final vowel dropped (`espanjattar`), and the
//! rest take the full geminated `e`-stem. The three are written out at the top
//! and every cell names which it rides.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "32*C";

/// Inflect a class 32*C noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let estem = if let Some(stripped) = lemma.strip_suffix("tar") {
        format!("{stripped}ttare")
    } else if let Some(stripped) = lemma.strip_suffix("tär") {
        format!("{stripped}ttäre")
    } else {
        lemma.to_string()
    };
    let graded = estem[..estem.len() - estem.chars().last().unwrap().len_utf8()].to_string();
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular ----
            // Both nominative possessives ride the oblique `e`-stem
            // (`aallottareni`, `aallottarensa`), not the bare lemma.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(estem.clone(), ["", ""]),
            cell(Case::Genitive, sg, &estem, ["n", "n"], false).on_stem(),
            // The partitive stays on the bare lemma: espanjatar -> espanjatarta.
            cell(Case::Partitive, sg, lemma, ["ta", "tä"], true),
            cell(Case::Essive, sg, &estem, ["na", "nä"], true),
            cell(Case::Translative, sg, &estem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, &estem, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Inessive, sg, &estem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &estem, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &estem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &estem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &estem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &estem, ["tta", "ttä"], true),
            // ---- the plural, on the bare graded stem ----
            cell(Case::Nominative, pl, &estem, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, &graded, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            // The `-ten` genitive plural goes back to the bare lemma:
            // espanjatar -> espanjatarten.
            cell(Case::Genitive, pl, lemma, ["ten", "ten"], false).on_ending(["te", "te"]),
            cell(Case::Partitive, pl, &graded, ["ia", "iä"], true),
            cell(Case::Essive, pl, &graded, ["ina", "inä"], true),
            cell(Case::Translative, pl, &graded, ["iksi", "iksi"], true)
                .on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &graded, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, &graded, ["issa", "issä"], true),
            cell(Case::Elative, pl, &graded, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &graded, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &graded, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &graded, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &graded, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &graded, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&graded);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn tar() {
        let w = words("espanjatar");
        assert!(w.contains(&"espanjatar".to_string()));
        assert!(w.contains(&"espanjattaren".to_string()));
        assert!(w.contains(&"espanjatarta".to_string()));
        assert!(w.contains(&"espanjattaret".to_string()));
        assert!(w.contains(&"espanjatarten".to_string()));
        assert!(w.contains(&"espanjattaria".to_string()));
    }

    /// The three stems are the class: the partitive and the `-ten` genitive
    /// plural on the bare lemma, the `i`-plural on the geminated stem without
    /// its final vowel, the singular obliques on the full geminated `e`-stem.
    #[test]
    fn the_three_stems_split_the_cases() {
        let w = words("espanjatar");
        assert!(w.contains(&"espanjatarta".to_string())); // part sg, lemma
        assert!(w.contains(&"espanjatarten".to_string())); // gen pl -ten, lemma
        assert!(w.contains(&"espanjattaria".to_string())); // part pl, graded
        assert!(w.contains(&"espanjattaren".to_string())); // gen sg, e-stem
        assert!(w.contains(&"espanjattareen".to_string())); // illat sg, e-stem
    }
}
