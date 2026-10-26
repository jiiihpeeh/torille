//! Class 39 (the aakkostus type) inflection, written by hand as a logical
//! engine.
//!
//! Class 39 is the `-us`/`-ys`/`-os`/`-is` denominal family (aakkostus, aatos,
//! onnellisuus): the tail `Vs` reads `Vks` (`aakkostus -> aakkostuks`), whose
//! `e`-stem carries the singular obliques (`aakkostuksen`, `aakkostuksessa`)
//! and whose bare `ks` stem the plural (`aakkostuksia`), while the partitive
//! stays on the lemma (`aakkostusta`). It reuses the class-33 graded engine.
//!
//! Two word groups have their own grid: the `-ude` words (`UDE_WORDS`), whose
//! singular rides a `-ude`/`-ute` stem, and `pallas`, whose `ll` reads `lt`
//! (`paltaa`/`palta`). Both are `Decl`s here; the common path is class 33's
//! `inflect_graded_nom_base`.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::class_33::inflect_graded_nom_base;
use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "39";

/// The oblique `e`-stem: aakkostus -> aakkostukse, aatos -> aatokse.
pub fn estem(lemma: &str) -> String {
    if lemma.ends_with('s') {
        format!("{}kse", &lemma[..lemma.len() - 1])
    } else {
        format!("{lemma}s")
    }
}

/// The `-us` words that gradate to `-ude` (vakaus, ahtaus): the genitive and
/// locals on `ude`, the essive/illative/partitive on `ute`/`tta`, and the
/// plural on `uks`.
const UDE_WORDS: &[&str] = &[
    "ahtaus",
    "karvaus",
    "nuntius",
    "pangasius",
    "rosvous",
    "vakaus",
];

/// Inflect a class-39 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    if UDE_WORDS.contains(&lemma) {
        return use_ude(lemma, possessives);
    }
    if lemma == "pallas" {
        return use_pallas(lemma, possessives);
    }
    // For the `-us`/`-os`/`-is` set the nominative singular and nominative
    // plural possessives both ride on the full oblique stem (`aakkostukseni`,
    // `aakkostuksemme`), not on the bare lemma / bare `ks` stem.
    let e = estem(lemma);
    inflect_graded_nom_base(lemma, possessives, &e, &e, &e)
}

/// The single `pallas` (ll -> lt): the singular oblique and nominative-plural
/// stems are `paltaa`, the plural obliques `palta`, the partitive `pallasta`.
fn use_pallas(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let a = "paltaa";
    let c = "palta";
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the long a-stem ----
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(a.to_string(), ["", ""]),
            cell(Case::Genitive, sg, a, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, lemma, ["ta", "tä"], true),
            cell(Case::Essive, sg, a, ["na", "nä"], true),
            cell(Case::Translative, sg, a, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, a, ["seen", "seen"], false).on_ending(["see", "see"]),
            cell(Case::Inessive, sg, a, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, a, ["sta", "stä"], true),
            cell(Case::Adessive, sg, a, ["lla", "llä"], true),
            cell(Case::Ablative, sg, a, ["lta", "ltä"], true),
            cell(Case::Allative, sg, a, ["lle", "lle"], true),
            cell(Case::Abessive, sg, a, ["tta", "ttä"], true),
            // ---- the plural, on the short a-stem ----
            cell(Case::Nominative, pl, a, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, c, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, c, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, c, ["ita", "itä"], true),
            cell(Case::Essive, pl, c, ["ina", "inä"], true),
            cell(Case::Translative, pl, c, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, c, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Illative, pl, c, ["isiin", "isiin"], false).on_ending(["isii", "isii"]),
            cell(Case::Inessive, pl, c, ["issa", "issä"], true),
            cell(Case::Elative, pl, c, ["ista", "istä"], true),
            cell(Case::Adessive, pl, c, ["illa", "illä"], true),
            cell(Case::Ablative, pl, c, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, c, ["ille", "ille"], true),
            cell(Case::Abessive, pl, c, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, c, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(c);

    b.into_forms()
}

/// The mixed `-ude` gradation of a handful of words (also the whole of
/// class 40's `-uus` family).
pub fn use_ude(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let u = &lemma[..lemma.len() - 1]; // vakau
    let ude = format!("{u}de");
    let ute = format!("{u}te");
    let uks = format!("{u}ks");
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular ----
            // The nominative singular possessives ride on the `-ute` stem
            // (`vakauteni`, `aakkosellisuuteni`), not on the `-ude` genitive
            // stem.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(ute.clone(), ["", ""]),
            cell(Case::Genitive, sg, &ude, ["n", "n"], false).base_on(ute.clone(), ["", ""]),
            cell(Case::Partitive, sg, u, ["tta", "ttä"], true),
            cell(Case::Essive, sg, &ute, ["na", "nä"], true),
            cell(Case::Translative, sg, &ude, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, &ute, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Inessive, sg, &ude, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &ude, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &ude, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &ude, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &ude, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &ude, ["tta", "ttä"], true),
            // ---- the plural ----
            // The bare nominative plural is on the `-ude` stem (`vakaudet`),
            // but its possessives ride the `-ute` stem like the singular's.
            cell(Case::Nominative, pl, &ude, ["t", "t"], false).base_on(ute, ["", ""]),
            cell(Case::Genitive, pl, &uks, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            cell(Case::Partitive, pl, &uks, ["ia", "iä"], true),
            cell(Case::Essive, pl, &uks, ["ina", "inä"], true),
            cell(Case::Translative, pl, &uks, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &uks, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, &uks, ["issa", "issä"], true),
            cell(Case::Elative, pl, &uks, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &uks, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &uks, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &uks, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &uks, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &uks, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&uks);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn us_type() {
        let w = words("aakkostus");
        assert!(w.contains(&"aakkostuksen".to_string()));
        assert!(w.contains(&"aakkostusta".to_string()));
        assert!(w.contains(&"aakkostuksena".to_string()));
        assert!(w.contains(&"aakkostuksia".to_string()));
        assert!(w.contains(&"aakkostuksien".to_string()));
    }

    #[test]
    fn os_type() {
        let w = words("aatos");
        assert!(w.contains(&"aatoksen".to_string()));
        assert!(w.contains(&"aatosta".to_string()));
    }

    /// `nuntius`/`pangasius` are `-ius` loans the reference gives the `-ude`
    /// shape (`nuntiuden`, not `nuntiuksen`) like `ahtaus`/`vakaus`.
    #[test]
    fn ius_loans_take_the_ude_shape() {
        for (lemma, ude) in [("nuntius", "nuntiuden"), ("pangasius", "pangasiuden")] {
            let w = words(lemma);
            assert!(w.contains(&ude.to_string()), "{ude} missing for {lemma}");
            assert!(
                !w.contains(&format!("{}ksen", &lemma[..lemma.len() - 2])),
                "{lemma} has the plain -ukse genitive"
            );
        }
    }

    /// The `-ude` singular's possessives ride the `-ute` stem, not the `-ude`
    /// genitive stem.
    #[test]
    fn the_ude_possessives_ride_the_ute_stem() {
        let w = words("vakaus");
        assert!(w.contains(&"vakauteni".to_string()));
        assert!(!w.contains(&"vakaudeni".to_string()));
    }
}
