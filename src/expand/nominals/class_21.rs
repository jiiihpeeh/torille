//! Class 21 (the brie type) inflection, written by hand as a logical engine.
//!
//! Class 21 is the single-final-vowel `-e`/`-é`/`-y` foreign family (brie,
//! brasserie, cowboy, jersey). It keeps the lemma whole as its stem. The
//! foreign words harmonize inconsistently (`jersey` front but `cowboy`/`fondue`
//! back), so one harmony is chosen as plain and the other is emitted repaired,
//! and the two are merged with a first-wins dedup.
//!
//! [`paradigm`] builds one harmony's worth of the grid; `inflect` runs it
//! twice. The singular partitive and the three singular illatives (`-hen`,
//! `-hyn`, `-hin`) and the three plural genitives (`-iden`, `-itten`, `-jen`)
//! are the extra cells beyond the base grid.

#![allow(dead_code)]

use crate::tags::{Case, Confidence, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, front, nominal_tags, plain};

pub const CLASS: &str = "21";

/// Foreign words whose spelling ends in `y` but which the reference reads as
/// back-harmonic (`cowboyta`, not `cowboytä`), which the orthographic harmony
/// would get wrong.
const BACK_HARMONY: &[&str] = &[
    "cowboy",
    "jockey",
    "maahockey",
    "playboy",
    "spray",
    "gray",
    "cosplay",
    "speedway",
];

fn paradigm(lemma: &str, possessives: bool, front: bool) -> Vec<Form> {
    let mut b = Builder::new_harmony(lemma, possessives, front);
    let s = lemma;
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the lemma ----
            cell(Case::Nominative, sg, s, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, s, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, s, ["ta", "tä"], true),
            cell(Case::Partitive, sg, s, ["a", "ä"], true),
            cell(Case::Essive, sg, s, ["na", "nä"], true),
            cell(Case::Translative, sg, s, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The three foreign illatives, in the reference's order.
            cell(Case::Illative, sg, s, ["hen", "hen"], false).on_ending(["he", "he"]),
            cell(Case::Illative, sg, s, ["hyn", "hyn"], false).on_ending(["hy", "hy"]),
            cell(Case::Illative, sg, s, ["hin", "hin"], false).on_ending(["hi", "hi"]),
            cell(Case::Inessive, sg, s, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, s, ["sta", "stä"], true),
            cell(Case::Adessive, sg, s, ["lla", "llä"], true),
            cell(Case::Ablative, sg, s, ["lta", "ltä"], true),
            cell(Case::Allative, sg, s, ["lle", "lle"], true),
            cell(Case::Abessive, sg, s, ["tta", "ttä"], true),
            // ---- the plural, the i-plural on the lemma ----
            cell(Case::Nominative, pl, s, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, s, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, s, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            // The `-jen` genitive plural of the foreign words: cowboyjeni.
            cell(Case::Genitive, pl, s, ["jen", "jen"], false).on_ending(["je", "je"]),
            cell(Case::Partitive, pl, s, ["ita", "itä"], true),
            cell(Case::Essive, pl, s, ["ina", "inä"], true),
            cell(Case::Translative, pl, s, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, s, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Inessive, pl, s, ["issa", "issä"], true),
            cell(Case::Elative, pl, s, ["ista", "istä"], true),
            cell(Case::Adessive, pl, s, ["illa", "illä"], true),
            cell(Case::Ablative, pl, s, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, s, ["ille", "ille"], true),
            cell(Case::Abessive, pl, s, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, s, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(s);

    b.into_forms()
}

/// Inflect a class-21 noun: the right harmony plain, the other repaired.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // A foreign word keeps one harmony; emitting both made the wrong one a
    // plain correction the reference rejects (`briea` for `brietä`). The
    // orthographic harmony reads `brie`/`jersey` front and `cowboy`/`fondue`
    // back, which is what the reference confirms.
    let is_front = !BACK_HARMONY.contains(&lemma) && front(lemma);
    let mut out = paradigm(lemma, possessives, is_front);
    // The other harmony is a real spelling for some words; offer it repaired.
    let mut other = paradigm(lemma, possessives, !is_front);
    for f in &mut other {
        f.confidence = Confidence::Repaired;
    }
    out.extend(other);
    // de-duplicate identical spellings across the two runs.
    let mut seen = std::collections::HashSet::new();
    out.retain(|f| seen.insert(f.word.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn single_vowel() {
        let w = words("brie");
        assert!(w.contains(&"brien".to_string()));
        assert!(w.contains(&"brietä".to_string()));
        assert!(w.contains(&"briehen".to_string()));
        assert!(w.contains(&"brieiden".to_string()));
    }

    #[test]
    fn y_words() {
        let w = words("cowboy");
        assert!(w.contains(&"cowboyilla".to_string()));
        assert!(w.contains(&"cowboyta".to_string()));
        assert!(w.contains(&"cowboyhinsä".to_string()) || w.contains(&"cowboyhinsa".to_string()));
    }

    /// `brie`'s front partitive is plain; the back one is repaired.
    #[test]
    fn the_wrong_harmony_is_repaired() {
        let forms = inflect("brie", true);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        assert!(plain.contains(&"brietä"), "{plain:?}");
        assert!(!plain.contains(&"briea"), "{plain:?}");
    }
}
