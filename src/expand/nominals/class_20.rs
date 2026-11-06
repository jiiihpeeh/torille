//! Class 20 (the ateljee type) inflection, written by hand as a logical
//! engine.
//!
//! Class 20 is the `-ee`/`-öö` family (ateljee, bebee, bidee), parallel to
//! class 17's `-oo`/`-aa` words: the singular is the lemma as its stem with
//! the long partitive `-ta` (`ateljeeta`), and the illative offers both
//! `-hen` and `-seen` (`ateljeehen`, `ateljeeseen`). The plural drops one of
//! the final vowels (`ateljee -> atelje`) and carries the `i`-plural.
//!
//! What is worth naming is the illative. The reference confirms `-seen` for
//! every word of the class and `-hen` for every word that ends in `ee`
//! (`ateljeehen`), and confirms neither `-hyn` nor `-han` for anybody, so the
//! four cells are emitted as one plain, one plain-or-rare depending on the
//! word, and two rare. That mix is what `Entry::as_rare` is for: a cell that
//! is an alternant for this word rather than a different kind of cell.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain, plural_only};

pub const CLASS: &str = "20";

/// Inflect a class-20 noun into every cell.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // A `-t`-final lemma is a plural-only citation (`bileet`, `häät`):
    // decline the dropped-`t` base and keep only its plural cells.
    if let Some(base) = lemma.strip_suffix('t')
        && !base.is_empty()
    {
        return plural_only(inflect(base, possessives));
    }
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    // The plural drops one of the two final vowels: ateljee -> atelje.
    let mut pstem = stem.to_string();
    pstem.pop();
    let (sg, pl) = (Number::Singular, Number::Plural);
    // `-hen` is confirmed for the `-ee` words, so it is a plain offer there and
    // a rare alternant elsewhere. The ending is already two vowels, so the cell
    // is stated as a stem of its own rather than as `stem` plus an ending.
    let hen = cell(Case::Illative, sg, &format!("{stem}he"), ["n", "n"], false).on_stem();

    b.decline(
        &Decl::new(vec![
            // ---- the singular (on the stem) ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, stem, ["ta", "tä"], true),
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The long illative: `-seen` for every word of the class.
            cell(Case::Illative, sg, &format!("{stem}see"), ["n", "n"], false).on_stem(),
            // ... and `-hen` for the `-ee` ones.
            if stem.ends_with("ee") {
                hen
            } else {
                hen.as_rare()
            },
            // `-hyn` and `-han` are confirmed for nobody in the class.
            cell(Case::Illative, sg, &format!("{stem}hy"), ["n", "n"], false)
                .on_stem()
                .as_rare(),
            cell(Case::Illative, sg, &format!("{stem}ha"), ["n", "n"], false)
                .on_stem()
                .as_rare(),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
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

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Confidence;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// The singular illatives and nothing else -- the bare forms, so a
    /// possessive suffix cannot be mistaken for the ending.
    fn illatives(lemma: &str) -> Vec<(String, Confidence)> {
        inflect(lemma, true)
            .into_iter()
            .filter(|f| {
                f.readings.iter().any(|t| {
                    t.case == Some(Case::Illative)
                        && t.number == Some(Number::Singular)
                        && t.possessive.is_none()
                })
            })
            .map(|f| (f.word, f.confidence))
            .collect()
    }

    /// The long form of the class: the singular is the lemma, the partitive is
    /// the long `-ta`, and the plural has dropped a vowel and taken the
    /// `i`-plural. Every form here is one the sanalista records for `ateljee`.
    #[test]
    fn the_comitative_third_person_follows_the_stem_harmony() {
        // `bileet` is front-harmony, so the full third-person comitative is
        // `bileinensä`. The earlier hard-coded `-inensa` produced the
        // back-harmony spelling for every word.
        let forms = inflect("bileet", true);
        assert!(forms.iter().any(|f| f.word == "bileinensä"));
        assert!(!forms.iter().any(|f| f.word == "bileinensa"));
        // A back-harmony word keeps the back spelling.
        let forms = inflect("ateljee", true);
        assert!(forms.iter().any(|f| f.word == "ateljeinensa"));
    }

    #[test]
    fn ee_words() {
        let w = words("ateljee");
        assert!(w.contains(&"ateljeen".to_string())); // gen sg
        assert!(w.contains(&"ateljeeta".to_string())); // part sg, long
        assert!(w.contains(&"ateljeeseen".to_string())); // ill sg, -seen
        assert!(w.contains(&"ateljeehen".to_string())); // ill sg, -hen
        assert!(w.contains(&"ateljeiden".to_string())); // gen pl
        assert!(w.contains(&"ateljeitten".to_string())); // and its -itten twin
    }

    /// The class's one real decision, stated as an invariant over the words the
    /// reference confirms an illative for: `-seen` is plain for all three,
    /// `-hen` is plain for all three (they all end in `ee`), and `-hyn`/`-han`
    /// are plain for none -- they are guesses, and a guess that a catalogue
    /// model reproduces must not be offered as a correction.
    #[test]
    fn only_the_confirmed_illatives_are_plain() {
        for lemma in ["ateljee", "bebee", "bidee"] {
            let plain: Vec<String> = illatives(lemma)
                .into_iter()
                .filter(|(_, c)| *c == Confidence::Plain)
                .map(|(w, _)| w)
                .collect();
            let plain: Vec<&str> = plain.iter().map(String::as_str).collect();
            assert!(
                plain.contains(&format!("{lemma}seen").as_str()),
                "{lemma}: {plain:?}"
            );
            assert!(
                plain.contains(&format!("{lemma}hen").as_str()),
                "{lemma}: {plain:?}"
            );
            assert!(
                !plain.iter().any(|w| w.ends_with("hyn")),
                "{lemma}: {plain:?}"
            );
            assert!(
                !plain.iter().any(|w| w.ends_with("han")),
                "{lemma}: {plain:?}"
            );
        }
    }
}
