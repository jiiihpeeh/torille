//! Class 2 (the `aakkosto`/`ehkäisy` type) inflection, written by hand as a
//! logical engine.
//!
//! A class-2 noun is its whole word as its stem, exactly like a class-1 `valo`,
//! but it is not the head of a gradation family: every class-2 word is a plain,
//! non-gradating vowel-final noun (`-sto`, `-ilu`, `-aisu`, loanwords). Every
//! case is `stem + ending`; the ending's a/ä or o/ö is chosen by the word's
//! harmony, and the illative doubles the stem's final vowel.
//!
//! The only cell in which class 2 differs from class 1 is the shape of the
//! `i`-plural. Class 2 freely alternates between the vowel-plural (`-ja`/`-jen`)
//! and the `i`-plural (`-ita`/`-iden`/`-itten`, `aaltoilu → aaltoiluja/aaltoiluita`)
//! for a stem whose final vowel is `a`, `o`, `u`, `y` or `ö`; an `e`-final
//! stem (`anime`) keeps only the vowel-plural, exactly as the reference does.
//!
//! That alternation is why the [`Decl`] is built rather than written: the `i`-
//! plural is three cells and a condition, and a table of fixed entries cannot
//! hold a cell that is sometimes absent. The cells are still declared in the
//! order the class emits them, which is what the order of the `push`es records.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, doubled_stem, nominal_tags, plain, plural_only};

pub const CLASS: &str = "2";

/// Inflect a class-2 noun (stem = the lemma, no gradation) into every cell.
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
    // The `i`-plural ending is available only when the stem does not end in a
    // neutral `e`: `anime -> animeja/animejen` but never `animeita/animeiden`.
    let e_final = stem.chars().last() == Some('e');

    let mut entries = vec![
        // ---- the singular ----
        cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
        cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
        cell(Case::Partitive, sg, stem, ["a", "ä"], true),
        cell(Case::Essive, sg, stem, ["na", "nä"], true),
        cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, stem, ["sta", "stä"], true),
        // The illative doubles the stem's final vowel and adds `n`: aaltoilu ->
        // aaltoiluun, and its possessives hang off the doubled stem (aaltoiluuni).
        cell(Case::Illative, sg, &doubled_stem(stem), ["n", "n"], false).on_stem(),
        cell(Case::Adessive, sg, stem, ["lla", "llä"], true),
        cell(Case::Ablative, sg, stem, ["lta", "ltä"], true),
        cell(Case::Allative, sg, stem, ["lle", "lle"], true),
        cell(Case::Abessive, sg, stem, ["tta", "ttä"], true),
        // ---- the plural ----
        cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
    ];

    // The plural genitive alternates between `-jen` and the `i`-plural
    // `-iden`/`-itten`; the `i`-plural is dropped for an `e`-final stem.
    entries.push(cell(Case::Genitive, pl, stem, ["jen", "jen"], false).on_ending(["je", "je"]));
    if !e_final {
        entries.push(
            cell(Case::Genitive, pl, stem, ["iden", "iden"], false).on_ending(["ide", "ide"]),
        );
        entries.push(
            cell(Case::Genitive, pl, stem, ["itten", "itten"], false).on_ending(["itte", "itte"]),
        );
    }
    entries.push(cell(Case::Partitive, pl, stem, ["ja", "jä"], true));
    if !e_final {
        entries.push(cell(Case::Partitive, pl, stem, ["ita", "itä"], true));
    }
    entries.extend([
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
    ]);

    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
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
        let w = words("aaltoilu");
        assert!(w.contains(&"aaltoilun".to_string())); // gen sg
        assert!(w.contains(&"aaltoilua".to_string())); // part sg
        assert!(w.contains(&"aaltoiluun".to_string())); // illat sg (doubled)
        assert!(w.contains(&"aaltoilut".to_string())); // nom pl
    }

    #[test]
    fn i_plural_for_vowel_stems() {
        let w = words("ehkäisy");
        assert!(w.contains(&"ehkäisyjä".to_string())); // vowel pl, front
        assert!(w.contains(&"ehkäisyitä".to_string())); // i pl, front
        assert!(w.contains(&"ehkäisyiden".to_string())); // gen i pl
    }

    #[test]
    fn e_final_drops_i_plural() {
        // `anime` keeps only the vowel-plural; no `animeita`/`animeiden`.
        let w = words("anime");
        assert!(w.contains(&"animeja".to_string()));
        assert!(w.contains(&"animejen".to_string()));
        assert!(
            !w.iter()
                .any(|f| f.contains("animeita") || f.contains("animeiden"))
        );
    }
}
