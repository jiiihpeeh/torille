//! Class 19 (the suo type) inflection, written by hand as a logical engine.
//!
//! Class 19 is the short-vowel `-uo`/`-ie`/`-yö` family (suo, tie, työ, vyö,
//! yö): the singular (and nominative plural) sit on the lemma (`suon`, `suot`),
//! while the plural obliques drop the first vowel of the final diphthong
//! (`suo -> so`) and carry the `i`-plural (`soiden`, `soita`, `soissa`).
//!
//! So there are two stems and the class is which cells take which: the lemma
//! for the singular and the two nominatives, the shortened stem for the plural
//! obliques. The singular possessives hang off the lemma itself (`tie` ->
//! `tieni`, `suo` -> `suoni`); the plural nominative takes no possessive at
//! all, so it is emitted as a bare cell.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "19";

/// The stem with the diphthong's first element dropped: `suo -> so`, `tie ->
/// te`, `työ -> tö`, `vyö -> vö`. It is the stem every plural oblique and the
/// comitative are built on, and it is what the nominatives hang their
/// possessives off.
fn short(lemma: &str) -> String {
    let mut chars: Vec<char> = lemma.chars().collect();
    // Drop the penultimate character, which in this class is the diphthong's
    // first element.
    if chars.len() >= 2 {
        chars.remove(chars.len() - 2);
    }
    chars.into_iter().collect()
}

/// Inflect a class-19 noun into every cell.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let s = lemma;
    let w = short(lemma);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The illative inserts the word's own last vowel before the `n`
    // (`suo -> suohon`, `tie -> tiehen`), so its base is that vowel again
    // without the `n` and its possessives hang off it (`suohoni`).
    let lastv = s.chars().last().unwrap().to_string();
    let ill = format!("{s}h{lastv}");

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the lemma ----
            // The singular possessive hangs off the lemma (`tie` ->
            // `tieni`, `suo` -> `suoni`), not the short plural stem.
            cell(Case::Nominative, sg, s, ["", ""], false),
            // The genitive possessive hangs off the lemma (`tie` ->
            // `tieni`), not the bare `tien` (which would give `tienni`).
            cell(Case::Genitive, sg, s, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, s, ["ta", "tä"], true),
            cell(Case::Essive, sg, s, ["na", "nä"], true),
            cell(Case::Translative, sg, s, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, &ill, ["n", "n"], false).on_stem(),
            cell(Case::Inessive, sg, s, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, s, ["sta", "stä"], true),
            cell(Case::Adessive, sg, s, ["lla", "llä"], true),
            cell(Case::Ablative, sg, s, ["lta", "ltä"], true),
            cell(Case::Allative, sg, s, ["lle", "lle"], true),
            cell(Case::Abessive, sg, s, ["tta", "ttä"], true),
            // ---- the plural: the nominative on the lemma, the obliques on the
            // short stem ----
            plain(Case::Nominative, pl, s, ["t", "t"]),
            cell(Case::Genitive, pl, &w, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, &w, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, &w, ["ita", "itä"], true),
            cell(Case::Essive, pl, &w, ["ina", "inä"], true),
            cell(Case::Translative, pl, &w, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &w, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Inessive, pl, &w, ["issa", "issä"], true),
            cell(Case::Elative, pl, &w, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &w, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &w, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &w, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &w, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &w, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&w);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    fn words(lemma: &str) -> Vec<String> {
        super::inflect(lemma, true)
            .into_iter()
            .map(|f| f.word)
            .collect()
    }

    /// The whole class in one lemma: the singular on the word, the plural
    /// obliques on the short stem. The sanalista confirms both halves.
    #[test]
    fn the_singular_is_the_word_and_the_plural_is_the_short_stem() {
        let w = words("suo");
        assert!(w.contains(&"suon".to_string())); // gen sg, on the lemma
        assert!(w.contains(&"suota".to_string())); // part sg, long
        assert!(w.contains(&"suohon".to_string())); // ill sg, `h` + the word's `o`
        assert!(w.contains(&"suot".to_string())); // nom pl, on the lemma
        assert!(w.contains(&"soiden".to_string())); // gen pl, on the short stem
        assert!(w.contains(&"soita".to_string())); // part pl, the `i`-plural
        assert!(w.contains(&"soissa".to_string())); // iness pl, short stem
    }

    /// The short stem is the diphthong's first element dropped, and the three
    /// spellings of it that matter are the reference's own.
    #[test]
    fn the_diphthong_loses_its_first_element() {
        for (lemma, gen_pl) in [
            ("suo", "soiden"),
            ("tie", "teiden"),
            ("työ", "töiden"),
            ("vyö", "vöiden"),
        ] {
            assert!(
                words(lemma).contains(&gen_pl.to_string()),
                "{lemma}: {gen_pl}"
            );
        }
    }

    /// Both nominatives hand their possessives to the short stem, which is why
    /// neither is `Base::Bare`: `suot` bare but `soni` owned. The reference
    /// records no nominative possessive for `suo`, so this is the class's rule
    /// rather than a form it confirms -- but it is the same short stem the
    /// plural obliques use, and the same one both nominatives agree on.
    #[test]
    fn the_singular_possessives_ride_the_lemma() {
        let w = words("suo");
        assert!(w.contains(&"suo".to_string()));
        assert!(w.contains(&"suot".to_string()));
        assert!(w.contains(&"suoni".to_string()), "on the lemma");
        assert!(!w.contains(&"soni".to_string()), "not on the short stem");
    }
}
