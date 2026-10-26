//! Class 1*A (the akku type) -- class 1 with `kk -> k` consonant gradation.
//!
//! Same shape as class 1 (stem + ending, harmony by last non-neutral vowel,
//! illative doubling, third-person -nsa/doubled), but the word has two grades:
//! the strong stem is the lemma (`akku`) and the weak stem weakens its `kk`
//! (`aku`). Strong feeds the nominative, partitive, essive, illative and
//! genitive-plural; weak feeds the genitive, the local cases and the
//! nominative-plural: `akku` is `akun`, `akussa`, `akulla`, `akut` on the weak
//! stem but `akkua`, `akkuun`, `akkujen` on the strong. The grade belongs to a
//! compound's head, so the weak stem reads the *last* `kk` once.
//!
//! The grade is stated per cell rather than by running the grid twice, because
//! the cells are interleaved: the singular's strong cells come first, then its
//! weak ones, then the plural's strong, then its weak. That order is what the
//! engine emits and it is observable, so the [`Decl`] is a list in the class's
//! own order rather than an array in case order.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, doubled_stem, nominal_tags, plain};

pub const CLASS: &str = "1*A";

/// The weak stem: the head's `kk` read once (akku -> aku; in `etikkakurkku`
/// the head `kurkku`, not the modifier `etikka`).
fn weak_stem(lemma: &str) -> String {
    if let Some(i) = lemma.rfind("kk") {
        format!("{}{}", &lemma[..i + 1], &lemma[i + 2..])
    } else {
        lemma.to_string()
    }
}

/// Inflect a class-1A noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let strong = lemma;
    let weak = weak_stem(lemma);
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- singular, strong grade ----
            cell(Case::Nominative, sg, strong, ["", ""], false).on_stem(),
            cell(Case::Partitive, sg, strong, ["a", "ä"], true),
            cell(Case::Essive, sg, strong, ["na", "nä"], true),
            cell(Case::Illative, sg, &doubled_stem(strong), ["n", "n"], false).on_stem(),
            // ---- singular, weak grade ----
            // The genitive's bare is weak (akun) but its possessives sit on the
            // strong stem (akkuni), the strong-genitive rule. This is the cell
            // the whole gradation exists for, and it is why `Base` names a stem
            // rather than only an ending.
            cell(Case::Genitive, sg, &weak, ["n", "n"], false)
                .base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, sg, &weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, &weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &weak, ["tta", "ttä"], true),
            // ---- plural, strong grade ----
            cell(Case::Genitive, pl, strong, ["jen", "jen"], false).on_ending(["je", "je"]),
            cell(Case::Partitive, pl, strong, ["ja", "jä"], true),
            cell(Case::Essive, pl, strong, ["ina", "inä"], true),
            cell(Case::Illative, pl, strong, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            // ---- plural, weak grade ----
            // The nominative's bare is weak (akut) but its possessives, like the
            // genitive's, sit on the strong stem (akkumme).
            cell(Case::Nominative, pl, &weak, ["t", "t"], false)
                .base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, pl, &weak, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Inessive, pl, &weak, ["issa", "issä"], true),
            cell(Case::Elative, pl, &weak, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &weak, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &weak, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &weak, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &weak, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &weak, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(strong);

    b.into_forms()
}
