//! Class 13 (the aivina/aluna type) inflection, written by hand as a logical
//! engine.
//!
//! Class 13 is an `-a`/`-ä` family (aivina, aluna, areena) whose plural is the
//! `o`-stem (final `a`/`ä` written `o`/`ö`, `aivino`) carrying the `i`-plural
//! -- `aivinoiden`/`aivinoitten`, `aivinoita`, `aivinoihin` -- with the
//! vowel-plural (`aivinojen`, `aivinoja`) beside it. The singular is the lemma
//! as its stem, and the nominative plural keeps the plain stem (`aivinat`).
//!
//! What is worth naming is the two lists. Which of the two plural families a
//! word offers plainly is the reference's to say, and for all 168 words of the
//! class it says one thing: 153 give both, 3 (`asana`, `kaldera`, `latuska`)
//! give the `i`-plural alone, 10 give the vowel-plural alone. So each family is
//! one cell with two confidences rather than two kinds of cell, which is what
//! `Entry::as_rare` is for.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain, rare};

pub const CLASS: &str = "13";

/// Class-13 words whose confirmed plural is the `i`-plural only (`-oiden`,
/// `-oitten`, `-oita`) and not the vowel-plural (`-ojen`/`-oja`). For them the
/// vowel-plural `-ojen`/`-oja` is a rare alternant, never offered plainly.
///
/// Read off the reference, which for every word of the class attests one whole
/// plural family or the other and never mixes the two across all four forms
/// (`-ojen`, `-oja`, `-oiden`/`-oitten`, `-oita`): 153 words give both, 3 give
/// this one and 10 give the other.
const I_PLURAL_ONLY: &[&str] = &["asana", "kaldera", "latuska"];

/// Class-13 words whose confirmed plural is the vowel-plural only (`-ojen`,
/// `-oja`) and not the `i`-plural. For them the `-oiden`/`-oitten`/`-oita`
/// forms are rare, never offered plainly. These are the `-a`-ending loans whose
/// reference entry lists only `-ojen`/`-oja`.
const VOWEL_PLURAL_ONLY: &[&str] = &[
    "baabuska", "babuška", "barista", "gratista", "kulitsa", "maniska", "namiska", "perilla",
    "pitaija", "retina",
];

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

/// Inflect a class-13 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    let ostem = o_stem(stem);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The singular illative doubles the stem's own last letter, which is not
    // known until the lemma is in hand: `aivinaan`.
    let last = stem.chars().last().unwrap();

    // Which of the two plural families is the word's confirmed one. Both
    // families are always offered; the list only says which is offered plainly.
    let vowel_plain = !I_PLURAL_ONLY.contains(&lemma);
    let i_plain = !VOWEL_PLURAL_ONLY.contains(&lemma);

    // Four cells, two spellings, one decision each: the genitive and partitive
    // plural of each family, plain for the family the reference confirms and a
    // rare alternant for the other. Stating them apart keeps the emission order
    // the class's own -- the vowel-plural, then the short genitive plural on the
    // plain stem, then the `i`-plural -- rather than sorting them into case
    // order, which would change what the engine emits.
    let or = |ending: [&'static str; 2]| cell(Case::Genitive, pl, &ostem, ending, false);
    let vowel_gen = or(["jen", "jen"]).on_ending(["je", "je"]);
    let vowel_gen = if vowel_plain {
        vowel_gen
    } else {
        vowel_gen.as_rare()
    };
    let vowel_part = cell(Case::Partitive, pl, &ostem, ["ja", "jä"], true);
    let vowel_part = if vowel_plain {
        vowel_part
    } else {
        vowel_part.as_rare()
    };
    let i_gen = or(["iden", "iden"]).on_ending(["ide", "ide"]);
    let i_gen = if i_plain { i_gen } else { i_gen.as_rare() };
    let i_itten = or(["itten", "itten"]).on_ending(["itte", "itte"]);
    let i_itten = if i_plain { i_itten } else { i_itten.as_rare() };
    let i_part = cell(Case::Partitive, pl, &ostem, ["ita", "itä"], true);
    let i_part = if i_plain { i_part } else { i_part.as_rare() };

    b.decline(
        &Decl::new(vec![
            // ---- the singular (on the stem) ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            // The partitive singular takes only the `-nsa` third person: the
            // doubled `ainaa+n` (`alunaaan`) is not confirmed for the class.
            cell(Case::Partitive, sg, stem, ["a", "ä"], false),
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
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
            // ---- the plural, on the o-stem i-plural ----
            cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
            // The genitive and partitive plural each carry both an `i`-stem
            // vowel-plural (`-ojen`/`-oja`) and an `i`-plural (`-oiden`/
            // `-oitten`/`-oita`); the short genitive plural between them is a
            // guess on the plain stem either way.
            vowel_gen,
            vowel_part,
            rare(Case::Genitive, pl, stem, ["in", "in"]),
            i_gen,
            i_itten,
            i_part,
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
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&ostem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Confidence;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    fn plain_words(lemma: &str) -> Vec<String> {
        inflect(lemma, true)
            .into_iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word)
            .collect()
    }

    #[test]
    fn o_stem_i_plural_only() {
        let w = words("aivina");
        assert!(w.contains(&"aivinan".to_string()));
        assert!(w.contains(&"aivinoiden".to_string()));
        assert!(w.contains(&"aivinoita".to_string()));
        assert!(w.contains(&"aivinoja".to_string())); // both plural variants
        assert!(w.contains(&"aivinoihin".to_string()));
    }

    /// Which plural family is the plain one is decided by the reference, and
    /// both lists used to say otherwise: `I_PLURAL_ONLY` held the `-ina`
    /// sound-word family, which the reference confirms *both* families for, so
    /// their confirmed `-ojen`/`-oja` was a Repaired guess; `VOWEL_PLURAL_ONLY`
    /// held `suurima`, which the reference also confirms both families for, so
    /// its confirmed `-oiden`/`-oitten`/`-oita` was demoted for no reason. The
    /// words the reference does separate are the ones now listed.
    #[test]
    fn the_plain_plural_family_is_the_one_the_reference_confirms() {
        for lemma in ["aivina", "suurima", "kahina", "mölinä", "media", "reppana"] {
            let plain = plain_words(lemma);
            let stem = o_stem(lemma);
            for confirmed in [format!("{stem}jen"), format!("{stem}iden")] {
                assert!(
                    plain.contains(&confirmed),
                    "{lemma}: {confirmed} is not plain"
                );
            }
        }
    }

    /// The ten `-a`-ending loans the reference gives `-ojen`/`-oja` and nothing
    /// else: the vowel-plural plainly, the `i`-plural only as a rare guess.
    #[test]
    fn a_word_confirmed_on_the_vowel_plural_alone_rare_the_i_plural() {
        for lemma in VOWEL_PLURAL_ONLY {
            let plain = plain_words(lemma);
            let stem = o_stem(lemma);
            assert!(
                plain.contains(&format!("{stem}jen")),
                "{lemma}: {stem}jen is not plain"
            );
            for unconfirmed in [
                format!("{stem}iden"),
                format!("{stem}itten"),
                format!("{stem}ita"),
            ] {
                assert!(
                    !plain.contains(&unconfirmed),
                    "{lemma}: {unconfirmed} is plain"
                );
            }
        }
    }

    /// And the three the other way round: `asana` has `asanoiden` and no
    /// `asanojen`, so the vowel-plural is its rare alternant.
    #[test]
    fn a_word_confirmed_on_the_i_plural_alone_rare_the_vowel_plural() {
        for lemma in I_PLURAL_ONLY {
            let plain = plain_words(lemma);
            let stem = o_stem(lemma);
            assert!(
                plain.contains(&format!("{stem}iden")),
                "{lemma}: {stem}iden is not plain"
            );
            for unconfirmed in [format!("{stem}jen"), format!("{stem}ja")] {
                assert!(
                    !plain.contains(&unconfirmed),
                    "{lemma}: {unconfirmed} is plain"
                );
            }
        }
    }
}
