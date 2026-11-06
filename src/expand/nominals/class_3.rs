//! Class 3 (the `absorptio`/`trikoo` type) inflection, written by hand.
//!
//! A class-3 noun is its whole word as its stem, with no consonant gradation --
//! the same shape as class 1 and class 2: `-io`, `-oo` and `-öö` loanwords such
//! as `absorptio`, `akvaario`, `trikoo`. Every singular case is `stem + ending`,
//! the ending's a/ä chosen by harmony, and the illative doubles the final vowel.
//!
//! Class 3 is distinguished from class 2 only in the shape of the plural.
//! Where class 2 freely alternates the vowel-plural and the `i`-plural, class 3
//! uses *only* the `i`-plural: the plural genitive `-iden`/`-itten`
//! (`absorptioiden`/`absorptioitten`) and the plural partitive `-ita`
//! (`absorptioita`), with no `-jen`/`-ja` alternant.
//!
//! What class 3 adds of its own is the long partitive. A stem ending in a vowel
//! sequence takes `-ta`/`-tä` (`absorptio -> absorptiota`) where a single-vowel
//! stem takes `-a`/`-ä`, so the ending is chosen from the stem rather than from
//! the harmony and the cell says which one it got.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, doubled_stem, nominal_tags, plain, plural_only};

pub const CLASS: &str = "3";

/// Inflect a class-3 noun (stem = the lemma, no gradation) into every cell.
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
    let (sg, pl) = (Number::Singular, Number::Plural);
    // A class-3 stem ends in a vowel sequence (`-io`, `-oo`, `-öö`, `-oe`), so
    // its partitive singular is the long `-ta`/`-tä` (`absorptio -> absorptiota`)
    // rather than the short `-a`/`-ä` of a single-vowel ending word.
    let vowels = ['a', 'e', 'i', 'o', 'u', 'y', 'ä', 'ö'];
    let mut chars = stem.chars();
    let last = chars.next_back().unwrap();
    let penult = chars.next_back();
    let vf = vowels.contains(&last) && penult.is_some_and(|c| vowels.contains(&c));
    let part = if vf { ["ta", "tä"] } else { ["a", "ä"] };

    b.decline(
        &Decl::new(vec![
            // ---- the singular ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, stem, part, true),
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
            // The illative doubles the stem's final vowel and adds `n`:
            // absorptio -> absorptioon.
            cell(Case::Illative, sg, &doubled_stem(stem), ["n", "n"], false).on_stem(),
            cell(Case::Adessive, sg, stem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, stem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, stem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, stem, ["tta", "ttä"], true),
            // ---- the plural ----
            cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
            // Class 3 keeps only the `i`-plural: -iden/-itten for the genitive
            // and -ita for the partitive; there is no -jen or -ja vowel-plural
            // alternant, which is the whole difference from class 2.
            cell(Case::Genitive, pl, stem, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, stem, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, stem, ["ita", "itä"], true),
            cell(Case::Essive, pl, stem, ["ina", "inä"], true),
            cell(Case::Translative, pl, stem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Inessive, pl, stem, ["issa", "issä"], true),
            cell(Case::Elative, pl, stem, ["ista", "istä"], true),
            cell(Case::Illative, pl, stem, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Adessive, pl, stem, ["illa", "illä"], true),
            cell(Case::Ablative, pl, stem, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, stem, ["ille", "ille"], true),
            cell(Case::Abessive, pl, stem, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, stem, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(stem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn stem_is_the_lemma() {
        let w = words("absorptio");
        assert!(w.contains(&"absorption".to_string()));
        assert!(w.contains(&"absorptiota".to_string()));
        assert!(w.contains(&"absorptioon".to_string()));
        assert!(w.contains(&"absorptiot".to_string()));
    }

    #[test]
    fn i_plural_only() {
        // No vowel-plural: absorptio takes -ita/-iden, never -ja/-jen.
        let w = words("absorptio");
        assert!(w.contains(&"absorptioita".to_string()));
        assert!(w.contains(&"absorptioiden".to_string()));
        assert!(w.contains(&"absorptioitten".to_string()));
        assert!(
            !w.iter()
                .any(|f| f.contains("absorptioja") || f.contains("absorptiojen"))
        );
    }

    #[test]
    fn vowel_sequence_partitive() {
        // A stem ending in a vowel sequence takes -ta/-tä, not -a/-ä.
        let w = words("absorptio");
        assert!(w.contains(&"absorptiota".to_string()));
        assert!(!w.contains(&"absorptioa".to_string()));
    }
}
