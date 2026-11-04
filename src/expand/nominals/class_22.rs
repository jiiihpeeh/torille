//! Class 22 (the buffet/parfait type) inflection, written by hand as a logical
//! engine.
//!
//! Class 22 is the consonant-final French-borrowed family (buffet, gourmet,
//! nougat, parfait). It is a non-gradating `-i` loanword (the singular obliques
//! sit on the `i`-stem: `buffetin`, `buffetissa`), and Finnish traditionally
//! breaks it with an apostrophe (`parfait'n`, `parfait'ssa`). Both the plain
//! `i`-stem paradigm and the apostrophe-cut paradigm are emitted: the plain
//! forms match the parser's database, the apostrophe forms the classic
//! spelling the Wiktionary catalogue uses.
//!
//! The plain half is class 5/6's shared [`build_i_loans`](super::utility::build_i_loans)
//! grid. The apostrophe half is a full 27-cell `Decl` built with
//! `Builder::new_rare`, so every form it makes is repaired. Two external
//! illatives (`nougathan`, `nougathan`) are plain and come last.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, IPlural, build_i_loans, cell, front, nominal_tags, plain};

pub const CLASS: &str = "22";

/// Inflect a class-22 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut out = build_i_loans(lemma, possessives, front(lemma), IPlural::NONE);
    out.extend(apostrophe(lemma, possessives));
    // The plain external illatives (`nougathen`, `nougathan`) beside the
    // forms above.
    let sg = Number::Singular;
    let mut b = Builder::new(lemma, possessives);
    b.decline(
        &Decl::new(vec![
            cell(Case::Illative, sg, lemma, ["hen", "hen"], false).on_ending(["he", "he"]),
            cell(Case::Illative, sg, lemma, ["han", "han"], false).on_ending(["ha", "ha"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    out.extend(b.into_forms());
    out
}

/// The apostrophe-cut paradigm: every ending is attached after an apostrophe
/// on the lemma (`parfait'n`, `parfait'ssa`, `parfait'iden`).
fn apostrophe(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = super::utility::Builder::new_rare(lemma, possessives);
    let s = lemma;
    let st = format!("{s}'");
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular ----
            // The bare nominative is the lemma; its possessives ride the
            // apostrophe stem (buffet'ni).
            cell(Case::Nominative, sg, s, ["", ""], false).base_on(st.clone(), ["", ""]),
            cell(Case::Genitive, sg, &st, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, &st, ["ta", "tä"], true),
            cell(Case::Essive, sg, &st, ["na", "nä"], true),
            cell(Case::Translative, sg, &st, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The illative's harmony is lexical, not derivable from the
            // spelling: Wiktionary gives `parfait'hen` (front) but
            // `nougat'han` (back). Both apostrophe spellings are emitted, as
            // the external illatives below are.
            cell(Case::Illative, sg, &st, ["hen", "hen"], false).on_ending(["he", "he"]),
            cell(Case::Illative, sg, &st, ["han", "han"], false).on_ending(["ha", "ha"]),
            cell(Case::Inessive, sg, &st, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &st, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &st, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &st, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &st, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &st, ["tta", "ttä"], true),
            // ---- the plural ----
            cell(Case::Nominative, pl, &st, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, &st, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, &st, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, &st, ["ita", "itä"], true),
            cell(Case::Essive, pl, &st, ["ina", "inä"], true),
            cell(Case::Translative, pl, &st, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &st, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Inessive, pl, &st, ["issa", "issä"], true),
            cell(Case::Elative, pl, &st, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &st, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &st, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &st, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &st, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &st, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&st);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn consonant_final() {
        let w = words("buffet");
        assert!(w.contains(&"buffetin".to_string()));
        assert!(w.contains(&"buffetissa".to_string()));
        assert!(w.contains(&"buffet'n".to_string()));
        assert!(w.contains(&"buffet'ssa".to_string()));
    }

    /// The plain and apostrophe halves are both present; the external
    /// illatives come last.
    #[test]
    fn both_paradigms_and_the_external_illatives() {
        let w = words("nougat");
        assert!(w.contains(&"nougatin".to_string()));
        assert!(w.contains(&"nougat'n".to_string()));
        assert!(w.contains(&"nougathen".to_string()));
        assert!(w.contains(&"nougathan".to_string()));
    }
}
