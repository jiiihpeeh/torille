//! Class 32 (the ahven type) inflection, written by hand as a logical engine.
//!
//! Class 32 is the consonant-final family (ahven, haiven, jäsen): the lemma is
//! a bare consonant word that grows an `-e` stem (`ahvene`) carrying the
//! singular obliques (`ahvenen`, `ahvenessa`); the partitive and the `-ten`
//! genitive plural stay on the bare consonant stem (`ahventa`, `ahventen`),
//! and the plural carries the full `i`-plural (`ahvenien`, `ahvenia`).
//!
//! This module is not just class 32: [`inflect_with_stems`] is the shared
//! consonant-word engine, and class 38 (`-nen` -> `-s`) is its other user,
//! supplying the two stems it computes. The grid is one `Decl`; the two stems
//! are the only thing a class has to say.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "32";

/// Inflect a class-32 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_with_estem(lemma, possessives, &format!("{lemma}e"))
}

/// The class-32 consonant word, given its oblique `e`-stem (ahvene,
/// espanjattare) and its bare consonant stem (defaulting to the lemma).
pub fn inflect_with_estem(lemma: &str, possessives: bool, estem: &str) -> Vec<Form> {
    inflect_with_stems(lemma, possessives, lemma, estem)
}

/// The consonant-word engine with explicit `istem` (the bare consonant stem;
/// `ahven`, `aakkos`) and `estem` (`ahvene`, `aakkose`).
pub fn inflect_with_stems(lemma: &str, possessives: bool, istem: &str, estem: &str) -> Vec<Form> {
    consonant_word(lemma, istem, estem, Builder::new(lemma, possessives))
}

/// As [`inflect_with_stems`], for a class that knows the word's front/back
/// identity better than the orthographic heuristic can read off its spelling.
/// The grid is untouched; only the builder that picks the endings differs.
pub fn inflect_with_stems_harmony(
    lemma: &str,
    possessives: bool,
    istem: &str,
    estem: &str,
    front: bool,
) -> Vec<Form> {
    consonant_word(
        lemma,
        istem,
        estem,
        Builder::new_harmony(lemma, possessives, front),
    )
}

/// The consonant-word grid, for a caller that has already decided the word's
/// harmony. Split out so the two entry points differ in nothing else, which is
/// what keeps a harmony exception from being able to change a stem.
fn consonant_word(lemma: &str, istem: &str, estem: &str, mut b: Builder) -> Vec<Form> {
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the e-stem ----
            // The bare nominative is the lemma (the bare consonant stem), but
            // its possessives ride the e-stem: ahven -> ahveneni.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(estem.to_string(), ["", ""]),
            cell(Case::Genitive, sg, estem, ["n", "n"], false).on_stem(),
            // The partitive stays on the bare consonant stem: ahven + ta.
            cell(Case::Partitive, sg, istem, ["ta", "tä"], true),
            cell(Case::Essive, sg, estem, ["na", "nä"], true),
            cell(Case::Translative, sg, estem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, estem, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Inessive, sg, estem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, estem, ["sta", "stä"], true),
            cell(Case::Adessive, sg, estem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, estem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, estem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, estem, ["tta", "ttä"], true),
            // ---- the plural, the i-plural on the bare consonant stem ----
            // The nominative's possessives ride the e-stem, as in the singular.
            cell(Case::Nominative, pl, estem, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, istem, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            cell(Case::Genitive, pl, istem, ["ten", "ten"], false).on_ending(["te", "te"]),
            cell(Case::Partitive, pl, istem, ["ia", "iä"], true),
            cell(Case::Essive, pl, istem, ["ina", "inä"], true),
            cell(Case::Translative, pl, istem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, istem, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, istem, ["issa", "issä"], true),
            cell(Case::Elative, pl, istem, ["ista", "istä"], true),
            cell(Case::Adessive, pl, istem, ["illa", "illä"], true),
            cell(Case::Ablative, pl, istem, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, istem, ["ille", "ille"], true),
            cell(Case::Abessive, pl, istem, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, istem, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(istem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn ahven() {
        let w = words("ahven");
        assert!(w.contains(&"ahvenen".to_string()));
        assert!(w.contains(&"ahventa".to_string()));
        assert!(w.contains(&"ahvenien".to_string()));
        assert!(w.contains(&"ahventen".to_string()));
        assert!(w.contains(&"ahvenia".to_string()));
    }

    /// The two stems do two different jobs: the partitive and the whole plural
    /// sit on the bare consonant stem, the singular obliques on the `e`-stem.
    /// `ahven` splits them; the test names both.
    #[test]
    fn the_bare_and_e_stems_split_the_cases() {
        let w = words("ahven");
        assert!(w.contains(&"ahvenessa".to_string())); // inessive, e-stem
        assert!(w.contains(&"ahventa".to_string())); // partitive, bare stem
        assert!(w.contains(&"ahvenissa".to_string())); // inessive pl, bare stem
        assert!(
            !w.contains(&"ahveneta".to_string()),
            "e-stem partitive emitted"
        );
    }
}
