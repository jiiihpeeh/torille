//! Class 12 (the aaria type) inflection, written by hand as a logical engine.
//!
//! Class 12 is a `-a`/`-ä` family (aaria, afasia) whose plural is the `o`-stem
//! (final `a`/`ä` written `o`/`ö`, `aario`) carrying the `i`-plural only --
//! `aarioiden`/`aarioitten`, `aarioita`, `aarioihin` -- with none of the
//! `i`-stem alternant of class 11. The singular is the lemma as its stem, and
//! the nominative plural keeps the plain stem (`aariat`).
//!
//! The plural alternants are settled per word, and the two lists are kept from
//! the live engine rather than widened here. Measured against the reference
//! over the 1190 single-class-12 lemmas: 1155 confirm the `i`-plural alone
//! (the `aaria` type), `chihuahua` confirms the vowel-plural alone, and 21 --
//! `autokefalia`, `ebola`, `gemena`, `granola`, `idea`, `igumenia`,
//! `intarsia`, `junttura`, `kanalja`, `kikkara`, `kippura`, `klovneria`,
//! `lappologia`, `luusua`, `nuolija`, `onomasiologia`, `pikkula`,
//! `polyamoria`, `sampanja`, `sosiopatia`, `vasara` -- confirm both. The live
//! lists name only `chihuahua`, `gemena` and `granola`, so the other 18 that
//! confirm the vowel-plural offer the `i`-plural alone. That is the same defect
//! shape the class-13 lists had, but it was examined earlier and left as a
//! curation; this port preserves that decision and records the count rather
//! than reopening it unilaterally.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain, rare};

pub const CLASS: &str = "12";

fn o_stem(stem: &str) -> String {
    let mut s = stem.to_string();
    let last = s.pop().unwrap();
    match last {
        'a' => s.push('o'),
        'ä' => s.push('ö'),
        _ => s.push(last),
    }
    s
}

/// Inflect a class-12 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    let ostem = o_stem(stem);
    let last = stem.chars().last().unwrap();
    let (sg, pl) = (Number::Singular, Number::Plural);

    // Both raised-plural alternants are settled per word by the reference:
    // the asia-type takes only the `i`-plural on the o-stem, `chihuahua` only
    // the vowel-plural, and `gemena`/`granola` both.
    let vowel_plural = lemma == "chihuahua" || lemma == "gemena" || lemma == "granola";
    let i_plural = lemma != "chihuahua";

    let mut entries = vec![
        // ---- the singular, on the stem ----
        cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
        cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
        // The partitive singular carries no doubled short 3rd-person form: the
        // reference lexicon confirms only `asiaansa` (the `-nsa` spelling), not
        // the `asiaaan` double, so `double3` is off here.
        cell(Case::Partitive, sg, stem, ["a", "ä"], false),
        cell(Case::Essive, sg, stem, ["na", "nä"], true),
        cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, stem, ["sta", "stä"], true),
        // The illative doubles the stem's last letter: aaria -> aariaan.
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
        // ---- the plural, on the o-stem ----
        cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
        // The short genitive plural is rare: generated, never offered.
        rare(Case::Genitive, pl, stem, ["in", "in"]),
    ];
    if vowel_plural {
        entries
            .push(cell(Case::Genitive, pl, &ostem, ["jen", "jen"], false).on_ending(["je", "je"]));
        entries.push(cell(Case::Partitive, pl, &ostem, ["ja", "jä"], true));
    }
    if i_plural {
        entries.push(
            cell(Case::Genitive, pl, &ostem, ["iden", "iden"], false).on_ending(["ide", "ide"]),
        );
        entries.push(
            cell(Case::Genitive, pl, &ostem, ["itten", "itten"], false).on_ending(["itte", "itte"]),
        );
        entries.push(cell(Case::Partitive, pl, &ostem, ["ita", "itä"], true));
    }
    entries.extend(vec![
        cell(Case::Essive, pl, &ostem, ["ina", "inä"], true),
        cell(Case::Translative, pl, &ostem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
        cell(Case::Illative, pl, &ostem, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
        cell(Case::Inessive, pl, &ostem, ["issa", "issä"], true),
        cell(Case::Elative, pl, &ostem, ["ista", "istä"], true),
        cell(Case::Adessive, pl, &ostem, ["illa", "illä"], true),
        cell(Case::Ablative, pl, &ostem, ["ilta", "iltä"], true),
        cell(Case::Allative, pl, &ostem, ["ille", "ille"], true),
        cell(Case::Abessive, pl, &ostem, ["itta", "ittä"], true),
        plain(Case::Instructive, pl, &ostem, ["in", "in"]),
    ]);

    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(&ostem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn o_stem_i_plural() {
        let w = words("aaria");
        assert!(w.contains(&"aarian".to_string()));
        assert!(w.contains(&"aarioiden".to_string()));
        assert!(w.contains(&"aarioita".to_string()));
        assert!(w.contains(&"aarioihin".to_string()));
    }

    #[test]
    fn chihuahua_takes_the_vowel_plural_only() {
        let w = words("chihuahua");
        assert!(w.contains(&"chihuahuojen".to_string()));
        assert!(!w.contains(&"chihuahuoiden".to_string()));
        assert!(!w.contains(&"chihuahuoita".to_string()));
    }
}
