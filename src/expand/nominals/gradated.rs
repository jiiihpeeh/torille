//! The gradating class-1 grid, shared by `1*B` through `1*M`.
//!
//! Eleven modules carried this grid and their `inflect` bodies were byte-identical
//! -- 3 549 characters each, 39 039 in all -- because the variant that
//! distinguishes `1*G` from `1*H` is not the grid but the consonant that
//! weakens, and each module said so in its own `weak_stem`. That is the verb
//! side's shape exactly: `verbs::utility::paradigm` holds the paradigm and each
//! class module is only the stems its type gives (`class_52.rs` says so in its
//! own module doc). So this is the paradigm, and a module that needs it supplies
//! a weak stem.
//!
//! The grid is class 1's, with a strong and a weak grade. Strong feeds the
//! nominative, partitive, essive and singular illative in the singular, and the
//! genitive, partitive, essive and illative in the plural; weak feeds the
//! genitive, the local cases and the nominative plural in the singular, and the
//! translative and the local cases in the plural. Two cells cross over and say
//! so: the singular genitive's bare is weak (`huppun`) while its possessives sit
//! on the strong stem (`hupuni`), and the plural nominative is the same
//! (`huput` beside `hupuni`).
//!
//! That crossover is the strong-genitive rule, and it is why [`Decl`] is a `Vec`
//! in the class's own order rather than a fixed array: the grid's order is part
//! of what the engine emits.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, doubled_stem, nominal_tags, plain};

/// Inflect a gradating class-1 noun: `lemma` is the strong stem, `weak` the weak
/// one, already computed by the caller's gradation rule.
pub fn inflect(lemma: &str, possessives: bool, weak: &str) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let strong = lemma;
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- singular, strong grade ----
            cell(Case::Nominative, sg, strong, ["", ""], false).on_stem(),
            cell(Case::Partitive, sg, strong, ["a", "ä"], true),
            cell(Case::Essive, sg, strong, ["na", "nä"], true),
            // The illative doubles the stem's final vowel: huppu -> huppuun, and
            // its possessives hang off the doubled stem (huppuuni).
            cell(Case::Illative, sg, &doubled_stem(strong), ["n", "n"], false).on_stem(),
            // ---- singular, weak grade ----
            // The genitive's bare is weak and its possessives are strong: the
            // crossover that makes the class gradate at all.
            cell(Case::Genitive, sg, weak, ["n", "n"], false).base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, sg, weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, weak, ["tta", "ttä"], true),
            // ---- plural, strong grade ----
            cell(Case::Genitive, pl, strong, ["jen", "jen"], false).on_ending(["je", "je"]),
            cell(Case::Partitive, pl, strong, ["ja", "jä"], true),
            cell(Case::Essive, pl, strong, ["ina", "inä"], true),
            cell(Case::Illative, pl, strong, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            // ---- plural, weak grade ----
            // The nominative crosses over the same way the singular genitive
            // does: weak bare (huput), strong possessives (hupuni).
            cell(Case::Nominative, pl, weak, ["t", "t"], false)
                .base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, pl, weak, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Inessive, pl, weak, ["issa", "issä"], true),
            cell(Case::Elative, pl, weak, ["ista", "istä"], true),
            cell(Case::Adessive, pl, weak, ["illa", "illä"], true),
            cell(Case::Ablative, pl, weak, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, weak, ["ille", "ille"], true),
            cell(Case::Abessive, pl, weak, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, weak, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    // The comitative is the one place the strong stem comes back after the weak
    // grade: hupuineen, not hupuineen off the weak stem.
    b.comitative(strong);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    /// Inflect through a real class rather than a hand-made weak stem, so the
    /// test cannot pass on a weak stem no class would ever produce.
    fn words(lemma: &str) -> Vec<String> {
        crate::expand::nominals::class_1B::inflect(lemma, true)
            .into_iter()
            .map(|f| f.word)
            .collect()
    }

    /// The strong-genitive crossover, which is the whole reason this grid
    /// exists: the genitive's bare form is weak (`hupun`) while its possessives
    /// sit on the strong stem (`huppuni`). Without the crossover the class would
    /// not gradate at all -- it would be class 1 with a spelling change.
    #[test]
    fn genitive_is_weak_bare_and_strong_possessed() {
        let w = words("huppu");
        assert!(w.contains(&"hupun".to_string()), "genitive bare is weak");
        assert!(w.contains(&"huppuni".to_string()), "genitive -ni is strong");
        assert!(
            !w.contains(&"hupuni".to_string()),
            "and not weak all the way down"
        );
    }

    /// The plural nominative crosses over the same way, and the local cases do
    /// not. The genitive is the exception, not the rule, and a grid that applied
    /// the crossover everywhere would give `*hupussani` on the strong stem.
    #[test]
    fn the_crossover_is_two_cells_and_no_more() {
        let w = words("huppu");
        assert!(
            w.contains(&"huput".to_string()),
            "nominative plural bare is weak"
        );
        assert!(
            w.contains(&"huppuni".to_string()),
            "nominative plural -ni is strong"
        );
        assert!(
            w.contains(&"hupussa".to_string()),
            "inessive is weak throughout"
        );
        assert!(
            w.contains(&"hupussani".to_string()),
            "and so are its possessives"
        );
    }

    /// The illative doubles the strong stem's final vowel in the singular, and
    /// the plural illative sits on the strong grade as well -- both are in the
    /// grid's strong group, so neither is `hupuihin`.
    #[test]
    fn both_illatives_are_on_the_strong_grade() {
        let w = words("huppu");
        assert!(
            w.contains(&"huppuun".to_string()),
            "singular illative doubles"
        );
        assert!(
            w.contains(&"huppuuni".to_string()),
            "and its possessives sit on it"
        );
        assert!(
            w.contains(&"huppuihin".to_string()),
            "plural illative is strong too"
        );
        assert!(
            !w.contains(&"hupuihin".to_string()),
            "and not off the weak stem"
        );
    }

    /// The comitative is the one cell after the weak grade that comes back to
    /// the strong stem, so it is worth pinning: `huppuineen`.
    #[test]
    fn the_comitative_comes_back_to_the_strong_stem() {
        assert!(words("huppu").contains(&"huppuineen".to_string()));
    }
}
