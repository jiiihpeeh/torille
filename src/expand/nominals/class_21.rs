//! Class 21 (the brie type) inflection, written by hand as a logical engine.
//!
//! Class 21 is the single-final-vowel `-e`/`-é`/`-y` foreign family (brie,
//! brasserie, cowboy, jersey). It keeps the lemma whole as its stem. The
//! foreign words harmonize inconsistently (`jersey`/`reggae` front but
//! `cowboy`/`fondue` back), so one harmony is chosen as plain and the other is
//! emitted repaired, and the two are merged with a first-wins dedup. A word
//! Wiktionary gives in one harmony only (`jersey`, `reggae`) has no other
//! column at all.
//!
//! [`paradigm`] builds one harmony's worth of the grid; `inflect` runs it
//! twice. The singular partitive and the three singular illatives (`-hen`,
//! `-hyn`, `-hin`) and the two plural genitives (`-iden`, `-itten`) are the
//! extra cells beyond the base grid. The `-jen` genitive the reference once
//! listed is not a form of any class-21 word: live Voikko rejects `briejen`,
//! `bébéjen` and every other lemma's, while accepting `-iden`/`-itten`.

#![allow(dead_code)]

use crate::tags::{Case, Confidence, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, front, nominal_tags, plain};

pub const CLASS: &str = "21";

/// Foreign words whose spelling ends in `y` but which the reference reads as
/// back-harmonic (`cowboyta`, not `cowboytä`), which the orthographic harmony
/// would get wrong. `jersey` is not one: it is read `tsöösi`, with the front
/// `ö`, so the orthographic front harmony is the right one.
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

/// Foreign words the orthographic harmony reads back but the reference reads
/// front. `reggae` is pronounced `regee`, and Wiktionary's Finnish entry gives
/// the front column (`reggaetä`, `reggaessä`, `reggaeitä`, `reggaettä`), not
/// the back `reggaeta` the spelling would suggest.
const FRONT_HARMONY: &[&str] = &["reggae"];

/// Words Wiktionary gives one harmony only. The other column is not a spelling
/// of the word, so it is not emitted even as repaired: `jersey` is read
/// `tsöösi`, and `reggae` is read `regee`.
const FRONT_ONLY: &[&str] = &["jersey", "reggae"];

/// The illative spellings live Voikko confirms for a class-21 word when the
/// oracle lists all three. A word outside these lists keeps the oracle's three
/// plain spellings, because the reference has no better answer for it.
const HEN_ONLY: &[&str] = &[
    "brie",
    "bébé",
    "fondue",
    "handsfree",
    "reggae",
    "rosé",
    "selfie",
];
const Y_ALL_THREE: &[&str] = &["cowboy", "playboy", "spray"];
const Y_HYN_ONLY: &[&str] = &["jersey", "jockey", "maahockey"];
const HIN_ONLY: &[&str] = &["pistou"];

fn paradigm(lemma: &str, possessives: bool, front: bool) -> Vec<Form> {
    let mut b = Builder::new_harmony(lemma, possessives, front);
    let s = lemma;
    let (sg, pl) = (Number::Singular, Number::Plural);
    let hen_plain = !Y_ALL_THREE.contains(&lemma)
        && !Y_HYN_ONLY.contains(&lemma)
        && !HIN_ONLY.contains(&lemma);
    let hyn_plain = !HEN_ONLY.contains(&lemma) && !HIN_ONLY.contains(&lemma);
    let hin_plain = !HEN_ONLY.contains(&lemma) && !Y_HYN_ONLY.contains(&lemma);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the lemma ----
            cell(Case::Nominative, sg, s, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, s, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, s, ["ta", "tä"], true),
            cell(Case::Essive, sg, s, ["na", "nä"], true),
            cell(Case::Translative, sg, s, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The three foreign illatives, in the reference's order. Only the
            // one live Voikko confirms for this word is plain; the others stay
            // as repaired alternants so they are never offered as corrections.
            {
                let hen = cell(Case::Illative, sg, s, ["hen", "hen"], false).on_ending(["he", "he"]);
                if hen_plain { hen } else { hen.as_rare() }
            },
            {
                let hyn = cell(Case::Illative, sg, s, ["hyn", "hyn"], false).on_ending(["hy", "hy"]);
                if hyn_plain { hyn } else { hyn.as_rare() }
            },
            {
                let hin = cell(Case::Illative, sg, s, ["hin", "hin"], false).on_ending(["hi", "hi"]);
                if hin_plain { hin } else { hin.as_rare() }
            },
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
    let is_front = FRONT_HARMONY.contains(&lemma)
        || (!BACK_HARMONY.contains(&lemma) && front(lemma));
    let mut out = paradigm(lemma, possessives, is_front);
    // A front-only word has no back column in Wiktionary, and emitting one as
    // repaired only creates forms the oracle mistakes for real ones.
    if FRONT_ONLY.contains(&lemma) {
        return out;
    }
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
        assert!(w.contains(&"brieitten".to_string()));
        assert!(!w.contains(&"briejen".to_string()));
        assert!(!w.contains(&"brieä".to_string()));
    }

    /// `bébé` confirms only `bébéhen`; the oracle's other two illatives are
    /// repaired, never plain.
    #[test]
    fn the_unconfirmed_illatives_are_repaired() {
        let forms = inflect("bébé", true);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        assert!(plain.contains(&"bébéhen"));
        assert!(!plain.contains(&"bébéhyn"));
        assert!(!plain.contains(&"bébéhin"));
    }

    #[test]
    fn y_words() {
        let w = words("cowboy");
        assert!(w.contains(&"cowboyilla".to_string()));
        assert!(w.contains(&"cowboyta".to_string()));
        assert!(w.contains(&"cowboyhinsä".to_string()) || w.contains(&"cowboyhinsa".to_string()));
    }

    /// `jersey` is front-only. Wiktionary records `jerseyiltä`, not the
    /// English-spelling back `jerseyilta`, so the back column is not emitted
    /// even as repaired.
    #[test]
    fn jersey_is_front_only() {
        let forms = inflect("jersey", true);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        assert!(plain.contains(&"jerseyiltä"));
        assert!(plain.contains(&"jerseynä"));
        assert!(!forms.iter().any(|f| f.word == "jerseyilta"));
        assert!(!forms.iter().any(|f| f.word == "jerseynansa"));
    }

    /// `reggae` is read `regee`, so Wiktionary's Finnish entry gives the
    /// front column even though the spelling's last vowel is `a`.
    #[test]
    fn reggae_is_front_harmonic() {
        let forms = inflect("reggae", true);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        for want in [
            "reggaetä",
            "reggaessä",
            "reggaestä",
            "reggaellä",
            "reggaeltä",
            "reggaelle",
            "reggaenä",
            "reggaettä",
            "reggaeitä",
            "reggaeissä",
            "reggaeistä",
            "reggaeillä",
            "reggaeiltä",
            "reggaeille",
            "reggaeinä",
            "reggaeiksi",
            "reggaeittä",
            "reggaein",
        ] {
            assert!(plain.contains(&want), "{want} is not plain: {plain:?}");
        }
        for back in ["reggaeta", "reggaessa", "reggaeitta", "reggaena"] {
            assert!(
                !forms.iter().any(|f| f.word == back),
                "{back} must not be emitted at all"
            );
        }
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
