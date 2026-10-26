//! Class 18 (the ADHD/abbreviation type) inflection, written by hand.
//!
//! Class 18 is two things in one. A word with no lowercase letter is an
//! abbreviation and takes `:` before every ending (`ADHD:tä`); a word that has
//! one is an ordinary short word and is class 17's, so this module hands those
//! straight over.
//!
//! The abbreviation half is mostly the short-vowel grid, and what it adds is
//! restraint about the illative. Each abbreviation confirms exactly one of
//! `hen`/`han`/`hun` (`ADHD:hen`, `DNA:han`, `EU:hun`), so only that one is
//! offered plainly and the other two are rare alternants -- and an abbreviation
//! that confirms none of them keeps all three rare, rather than inventing a
//! plain form to fill the cell. The plural genitive is the same story: `-iden`
//! is confirmed, `-itten` is rare.
//!
//! That restraint is why the [`Decl`] is built rather than written: a cell is
//! sometimes plain and sometimes rare depending on what the word confirms, and
//! the `Entry::as_rare` flag is what says which.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, front, nominal_tags, plain};

#[cfg(test)]
use crate::tags::Confidence;

pub const CLASS: &str = "18";

/// Abbreviations the reference treats as front-harmonic even though a back
/// letter hides in the spelling (`ADHD`, `ALV`).
const FRONT_ABBREV: &[&str] = &["ADHD", "ALV"];

/// The single confirmed illative vowel for each full abbreviation (the third
/// person only takes the `-nsa`/`-nsä` form, so the doubled `-Vn` does not
/// apply). Each abbreviation confirms exactly one, and only that one is offered
/// plainly; the others are rare alternants.
///
/// The confirmed vowel is not always one of the three the alternants are built
/// from: `AMK` is `hon` and `DJ` is `hin`. That is fine here because the table
/// records whatever the reference confirms and the alternant list is only a
/// fallback for a word the table says nothing about.
const ILLATIVE: &[(&str, &str)] = &[
    ("ADHD", "hen"),
    ("ALV", "hen"),
    ("AMK", "hon"),
    ("DJ", "hin"),
    ("DNA", "han"),
    ("EKG", "hen"),
    ("EU", "hun"),
    ("LSD", "hen"),
    ("RNA", "han"),
    ("UV", "hen"),
];

/// Inflect a class-18 word.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // An abbreviation (all uppercase) takes `:` before every ending; a regular
    // short word (maa, aamunkoi, hai) inflects like class 17's long-vowel nouns.
    let is_abbrev = lemma.chars().all(|c| !c.is_lowercase());
    if !is_abbrev {
        return super::class_17::inflect(lemma, possessives);
    }
    let fi = if FRONT_ABBREV.contains(&lemma) {
        true
    } else {
        front(lemma)
    };
    let mut b = Builder::new_harmony(lemma, possessives, fi);
    let s = lemma;
    let stem = format!("{s}:");
    let (sg, pl) = (Number::Singular, Number::Plural);

    let mut entries = vec![
        // ---- the singular ----
        cell(Case::Nominative, sg, s, ["", ""], false).base_on(stem.clone(), ["", ""]),
        cell(Case::Genitive, sg, &stem, ["n", "n"], false).base_on(stem.clone(), ["", ""]),
        cell(Case::Partitive, sg, &stem, ["ta", "tä"], true),
        cell(Case::Essive, sg, &stem, ["na", "nä"], true),
        cell(Case::Translative, sg, &stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
    ];

    // Only the single confirmed illative is offered plainly; the other two
    // spellings are rare alternants (the abbreviations that confirm neither keep
    // all three rare, so nothing is fabricated).
    let ill = ILLATIVE.iter().find(|(w, _)| *w == lemma).map(|(_, v)| *v);
    match ill {
        Some(ill) => {
            let ill_base = &ill[..ill.len() - 1]; // `hen` -> `he`, for possessives
            entries.push(
                cell(Case::Illative, sg, &stem, [ill, ill], false)
                    .base_on(format!("{stem}{ill_base}"), ["", ""]),
            );
        }
        None => {
            for (bare, base) in [("hen", "he"), ("han", "ha"), ("hun", "hu")] {
                entries.push(
                    cell(Case::Illative, sg, &stem, [bare, bare], false)
                        .base_on(format!("{stem}{base}"), ["", ""])
                        .as_rare(),
                );
            }
        }
    }

    entries.extend([
        cell(Case::Inessive, sg, &stem, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, &stem, ["sta", "stä"], true),
        cell(Case::Adessive, sg, &stem, ["lla", "llä"], true),
        cell(Case::Ablative, sg, &stem, ["lta", "ltä"], true),
        cell(Case::Allative, sg, &stem, ["lle", "lle"], true),
        cell(Case::Abessive, sg, &stem, ["tta", "ttä"], true),
        // ---- the plural, the i-plural on the lemma ----
        cell(Case::Nominative, pl, &stem, ["t", "t"], false).base_on(stem.clone(), ["", ""]),
        // Only the `-iden` genitive is confirmed; `-itten` is a rare alternant.
        cell(Case::Genitive, pl, &stem, ["iden", "iden"], false).on_ending(["ide", "ide"]),
        cell(Case::Genitive, pl, &stem, ["itten", "itten"], false)
            .on_ending(["itte", "itte"])
            .as_rare(),
        cell(Case::Partitive, pl, &stem, ["ita", "itä"], true),
        cell(Case::Essive, pl, &stem, ["ina", "inä"], true),
        cell(Case::Translative, pl, &stem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
        cell(Case::Illative, pl, &stem, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
        cell(Case::Inessive, pl, &stem, ["issa", "issä"], true),
        cell(Case::Elative, pl, &stem, ["ista", "istä"], true),
        cell(Case::Adessive, pl, &stem, ["illa", "illä"], true),
        cell(Case::Ablative, pl, &stem, ["ilta", "iltä"], true),
        cell(Case::Allative, pl, &stem, ["ille", "ille"], true),
        cell(Case::Abessive, pl, &stem, ["itta", "ittä"], true),
        plain(Case::Instructive, pl, &stem, ["in", "in"]),
    ]);

    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(&stem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// The colon is what makes this class: it goes on every ending but the
    /// nominative's base, which is the bare abbreviation.
    #[test]
    fn abbreviation() {
        let w = words("ADHD");
        assert!(w.contains(&"ADHD:n".to_string()));
        assert!(w.contains(&"ADHD:tä".to_string()));
        assert!(w.contains(&"ADHD:ni".to_string()));
        assert!(w.contains(&"ADHD:iden".to_string()));
    }

    /// The restraint this class is for: an abbreviation confirms exactly one
    /// illative, that one is offered plainly, and the two the engine could
    /// generate but the reference does not confirm are *not* offered at all.
    /// An abbreviation in the table gets nothing invented; one absent from it
    /// gets all three as rare guesses.
    #[test]
    fn the_confirmed_illative_is_the_only_one_offered_plainly() {
        let forms = inflect("ADHD", true);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .filter(|w| w.starts_with("ADHD:") && (w.ends_with('n') || w.ends_with("nsä")))
            .collect();
        assert!(plain.contains(&"ADHD:hen"), "{plain:?}");
        assert!(!plain.contains(&"ADHD:han"), "{plain:?}");
        assert!(!plain.contains(&"ADHD:hun"), "{plain:?}");
    }

    /// A confirmed vowel outside the three the alternants are built from is still
    /// offered plainly, which is what the table is for: `AMK` is `hon`.
    #[test]
    fn a_confirmed_vowel_outside_the_alternants_is_offered_plainly() {
        let forms = inflect("AMK", true);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        assert!(plain.contains(&"AMK:hon"), "{plain:?}");
    }

    /// A word the table says nothing about gets three rare illatives and no plain
    /// one, rather than a plain form invented to fill the cell.
    #[test]
    fn an_unlisted_abbreviation_gets_rares_and_no_plain_illative() {
        let forms = inflect("ZZTOP", true);
        let illatives: Vec<(&str, Confidence)> = forms
            .iter()
            .filter(|f| f.word.starts_with("ZZTOP:h"))
            .filter(|f| f.word.len() == "ZZTOP:hen".len())
            .map(|f| (f.word.as_str(), f.confidence))
            .collect();
        assert_eq!(
            illatives,
            vec![
                ("ZZTOP:hen", Confidence::Repaired),
                ("ZZTOP:han", Confidence::Repaired),
                ("ZZTOP:hun", Confidence::Repaired),
            ],
            "no plain illative is invented"
        );
    }

    /// A word with a lowercase letter is not an abbreviation and is class 17's.
    #[test]
    fn a_lower_case_letter_is_class_17() {
        let w = words("maa");
        assert!(w.contains(&"maa".to_string()));
        assert!(!w.iter().any(|f| f.contains("maa:")));
    }
}
