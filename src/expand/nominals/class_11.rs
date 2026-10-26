//! Class 11 (the apaja/ahava type) inflection, written by hand as a logical
//! engine.
//!
//! Class 11 is a `-a`/`-ä` family (apaja, ahava, algebra) whose plural offers
//! *both* plural stems at once: the `i`-stem (final `a`/`ä` written `i`,
//! `ahavi`) and the `o`-stem (final `a`/`ä` written `o`, `ahavo`). The `i`-stem
//! takes the truncated endings (`ahavien`, `ahavia`, `ahavissa`, like class 10);
//! the `o`-stem takes the `i`-plural (`ahavoiden`/`ahavoitten`, `ahavoita`,
//! `ahavoihin`), and for a front word its vowel-plural too (`käkkäröjen`). A
//! native word gives both stems; the singular is the lemma as its stem.
//!
//! What is worth naming is that the two stems are one `Decl` rather than two
//! passes: the class emits the `i`-stem set, then the `o`-stem set, then the
//! comitative, and a word confirmed on the `i`-stem alone (`rucola`) drops the
//! `o`-stem set and the comitative with it.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain, rare};

pub const CLASS: &str = "11";

/// Class-11 words confirmed with the i-stem plural only (`rucolien`, `rucolia`),
/// no o-stem at all: the o-stem forms are rare alternants.
///
/// The reference has no word confirmed with the o-stem *only*: `algebra`, which
/// used to be the exception here, now has `algebrien` alongside
/// `algebroiden`/`algebroitten`, so every word of the class gives both stems and
/// the i-stem needs no list of its own.
const I_STEM_ONLY: &[&str] = &["rucola"];

fn i_stem(stem: &str) -> String {
    let mut s = stem.to_string();
    let last = s.pop().unwrap();
    match last {
        'a' | 'ä' => s.push('i'),
        _ => s.push(last),
    }
    s
}
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

/// Inflect a class-11 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    let istem = i_stem(stem);
    let ostem = o_stem(stem);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The two illatives that double the stem's own last letter, which is not
    // known until the lemma is in hand: `ahavaan` and `ahaviin`.
    let last = stem.chars().last().unwrap();
    let il = istem.chars().last().unwrap();

    // Every word of the class confirms both stems, so the i-stem variant needs
    // no plain/rare split: it is the truncated ending set of class 10.
    let o_plain = !I_STEM_ONLY.contains(&lemma);
    // The o-stem vowel-plural (`-ojen`/`-oja`) is confirmed only for the front
    // words (käkkäröjen); the back words (ahavojen, ahavoja) confirm the o-stem
    // i-plural only, so the vowel-plural is their rare alternant.
    let o_vowel_plain = o_plain && b.is_front();

    let mut entries = vec![
        // ---- the singular (on the stem) ----
        cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
        cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
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
        // ---- the plural, nominative on the plain stem ----
        cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
        // ---- the i-stem variant (truncated endings) ----
        cell(Case::Genitive, pl, &istem, ["en", "en"], false).on_ending(["e", "e"]),
        cell(Case::Partitive, pl, &istem, ["a", "ä"], true),
        // The short genitive plural is a guess on the plain stem, not the
        // i-stem's: `ahavain`.
        rare(Case::Genitive, pl, stem, ["in", "in"]),
        cell(Case::Essive, pl, &istem, ["na", "nä"], true),
        cell(Case::Translative, pl, &istem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(
            Case::Illative,
            pl,
            &istem,
            Ending::Given(format!("{il}n")),
            false,
        )
        .on_ending(Ending::Given(format!("{il}"))),
        cell(Case::Inessive, pl, &istem, ["ssa", "ssä"], true),
        cell(Case::Elative, pl, &istem, ["sta", "stä"], true),
        cell(Case::Adessive, pl, &istem, ["lla", "llä"], true),
        cell(Case::Ablative, pl, &istem, ["lta", "ltä"], true),
        cell(Case::Allative, pl, &istem, ["lle", "lle"], true),
        cell(Case::Abessive, pl, &istem, ["tta", "ttä"], true),
        plain(Case::Instructive, pl, &istem, ["n", "n"]),
    ];

    // ---- the o-stem variant (vowel-plural and i-plural) ----
    if o_plain {
        // The vowel-plural, plain for the front words and a rare alternant for
        // the rest. One cell and two confidences, which is what `as_rare` is
        // for: the spelling is the same either way.
        let vowel = cell(Case::Genitive, pl, &ostem, ["jen", "jen"], false).on_ending(["je", "je"]);
        let vowel = if o_vowel_plain {
            vowel
        } else {
            vowel.as_rare()
        };
        let vowel_part = cell(Case::Partitive, pl, &ostem, ["ja", "jä"], true);
        let vowel_part = if o_vowel_plain {
            vowel_part
        } else {
            vowel_part.as_rare()
        };
        entries.extend(vec![
            vowel,
            vowel_part,
            cell(Case::Genitive, pl, &ostem, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, &ostem, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, &ostem, ["ita", "itä"], true),
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
    }

    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    // The comitative rides the o-stem, so a word confirmed on the i-stem alone
    // has none.
    if o_plain {
        b.comitative(&ostem);
    }

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Confidence;

    /// Every lemma the sanalista files under class 11, and no others.
    const CLASS_WORDS: &[&str] = &[
        "ahava",
        "algebra",
        "apaja",
        "curuba",
        "hapera",
        "harava",
        "hatara",
        "hattara",
        "hekuma",
        "hutera",
        "itara",
        "jellona",
        "judoka",
        "kihara",
        "kiverä",
        "korona",
        "kumura",
        "käkkärä",
        "käpälä",
        "kärhämä",
        "lattana",
        "leukoija",
        "littana",
        "lättänä",
        "mahatma",
        "mammona",
        "mandala",
        "marihuana",
        "maruna",
        "mimoosa",
        "mitsuba",
        "murena",
        "mäkärä",
        "ohrana",
        "omena",
        "orpana",
        "paatsama",
        "papana",
        "pipana",
        "poppana",
        "probleema",
        "rucola",
        "rukola",
        "samosa",
        "sikkara",
        "sikuna",
        "säkkärä",
        "tomera",
        "täkänä",
        "vanttera",
        "äpärä",
        "ödeema",
    ];

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
    fn both_plural_stems() {
        let w = words("ahava");
        assert!(w.contains(&"ahavien".to_string()));
        assert!(w.contains(&"ahavia".to_string()));
        assert!(w.contains(&"ahavoiden".to_string()));
        assert!(w.contains(&"ahavoihin".to_string()));
    }

    #[test]
    fn front_word_o_stem() {
        let w = words("käkkärä");
        assert!(w.contains(&"käkkäröiden".to_string()));
        assert!(w.contains(&"käkkäröihin".to_string()));
    }

    /// No word of the class is o-stem-only, so the i-stem genitive plural is
    /// Plain for every one of them; `algebra` used to be the exception, which
    /// left its confirmed `algebrien` a Repaired guess while the reference
    /// attests it beside `algebroiden`/`algebroitten`. The claim is about the
    /// class, so the fixture is the class.
    #[test]
    fn every_word_gives_the_i_stem_plural_plainly() {
        for lemma in CLASS_WORDS {
            let stem = i_stem(lemma);
            let plain = plain_words(lemma);
            assert!(
                plain.contains(&format!("{stem}en")),
                "{lemma}: {stem}en is not plain"
            );
        }
    }

    /// `rucola` is the other way round: the reference gives it `rucolien` and
    /// nothing on the o-stem, so the o-stem plural is the rare alternant.
    #[test]
    fn a_word_confirmed_on_the_i_stem_alone_rare_the_o_stem() {
        let forms = inflect("rucola", true);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        assert!(plain.contains(&"rucolien"), "{plain:?}");
        assert!(!plain.contains(&"rucoloiden"), "{plain:?}");
    }

    /// The o-stem's comitative goes with the o-stem: `rucolan` is the i-stem's
    /// own plural set, and there is no comitative for a word the reference
    /// confirms without an o-stem.
    #[test]
    fn the_comitative_rides_the_o_stem() {
        assert!(words("ahava").contains(&"ahavoineen".to_string()));
        assert!(!words("rucola").contains(&"rucoloineen".to_string()));
    }
}
