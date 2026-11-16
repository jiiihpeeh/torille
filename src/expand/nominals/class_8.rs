//! Class 8 (the agaave/akne type) inflection, written by hand as a logical
//! engine.
//!
//! Class 8 is the loanword `-e` family (`agaave`, `akne`, `beige`). Like class 1
//! the whole lemma is the stem with no gradation; every case is `stem + ending`,
//! the ending's a/ä chosen by harmony, and the illative doubles the stem's
//! final `e` (`agaave -> agaaveen`). Unlike class 1, the plural is the vowel
//! plural -- `-jen`/`-ja` (`agaavejen`, `agaaveja`) -- with no `-iden`/
//! `-itten`/`-ita` `i`-plural alternant, which is how these `-e` loanwords
//! inflect. (Three lemmas of the class do take an `i`-plural; see the measured
//! gap at the end of this comment.)
//!
//! The graded types `8*A`, `8*B` and `8*C` are this module too, with consonant
//! gradation on top: `nukke -> nuke`, so the class modules are one line each.
//!
//! What is worth naming: the graded grid splits the plural by grade in a way the
//! plain grid cannot be asked for, and the split is not a difference of stem but
//! of which cases exist on which. Strong feeds the genitive and partitive
//! plural (`nukkejen`, `nukkeja`) and weak the nominative (`nuket`), the
//! translative and the locals -- so the plural is one `Decl` with both stems in
//! it, exactly as the singular is.
//!
//! One measured gap, left as a gap. Of the 109 class-8 lemmas, 106 confirm the
//! vowel plural this grid generates (`-jen`/`-ja`). Three do not: they are
//! `birdie`, `söde` and `vintage`, and the reference gives each an `i`-plural
//! (`birdieiden`, `birdieitä`) -- and also doubles the stem's `e` across the
//! paradigm (`vintageen`, `vintagetta`, `vintageeseen`, `södeet`). That is not
//! this class's type but a distinct doubled-`e` irregular, so fixing it is not
//! a matter of adding a name to a list: it would mean modelling a paradigm the
//! grid has no shape for, for three words, two of which (`söde`, `vintage`)
//! the grid spells wrong in the singular as well. Left alone deliberately,
//! with the count, rather than papered over with a word list.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain};

pub const CLASS: &str = "8";

/// The `kolme` numeral: singular on `kolme`, plural on `kolmi` except the
/// nominative.
fn kolme_grid(possessives: bool) -> Vec<Form> {
    let mut b = Builder::new("kolme", possessives);
    let (sg, pl) = (Number::Singular, Number::Plural);
    b.decline(
        &Decl::new(vec![
            cell(Case::Nominative, sg, "kolme", ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, "kolme", ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, "kolme", ["a", "ä"], true),
            cell(Case::Essive, sg, "kolme", ["na", "nä"], true),
            cell(Case::Translative, sg, "kolme", ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, "kolme", ["ssa", "ssä"], true),
            cell(Case::Elative, sg, "kolme", ["sta", "stä"], true),
            cell(Case::Illative, sg, "kolme", ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Adessive, sg, "kolme", ["lla", "llä"], true),
            cell(Case::Ablative, sg, "kolme", ["lta", "ltä"], true),
            cell(Case::Allative, sg, "kolme", ["lle", "lle"], true),
            cell(Case::Abessive, sg, "kolme", ["tta", "ttä"], true),
            cell(Case::Nominative, pl, "kolme", ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, "kolmi", ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Partitive, pl, "kolmi", ["a", "ä"], true),
            cell(Case::Essive, pl, "kolmi", ["na", "nä"], true),
            cell(Case::Translative, pl, "kolmi", ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, pl, "kolmi", ["ssa", "ssä"], true),
            cell(Case::Elative, pl, "kolmi", ["sta", "stä"], true),
            cell(Case::Illative, pl, "kolmi", ["in", "in"], false).on_ending(["i", "i"]),
            cell(Case::Adessive, pl, "kolmi", ["lla", "llä"], true),
            cell(Case::Ablative, pl, "kolmi", ["lta", "ltä"], true),
            cell(Case::Allative, pl, "kolmi", ["lle", "lle"], true),
            cell(Case::Abessive, pl, "kolmi", ["tta", "ttä"], true),
            plain(Case::Instructive, pl, "kolmi", ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Numeral, case, number, poss),
    );
    b.comitative("kolmi");
    b.into_forms()
}

/// Inflect a class-8 noun (stem = the lemma, no gradation).
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // `kolme` is the class-8 *numeral*, not an `-e` loanword: its plural stem
    // is `kolmi` (kolmien, kolmia, kolmiin), not the loanword `kolmejen`.
    // Wiktionary gives the ovi/7-adjacent numeral table for it.
    if lemma == "kolme" {
        return kolme_grid(possessives);
    }
    let mut b = Builder::new(lemma, possessives);
    let stem = lemma;
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The singular illative doubles the stem's own last letter, which is not
    // known until the lemma is in hand: agaave -> agaaveen.
    let elast = stem.chars().last().unwrap();

    b.decline(
        &Decl::new(vec![
            // ---- the singular (on the stem) ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, stem, ["a", "ä"], true),
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
            cell(
                Case::Illative,
                sg,
                stem,
                Ending::Given(format!("{elast}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{elast}"))),
            cell(Case::Adessive, sg, stem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, stem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, stem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, stem, ["tta", "ttä"], true),
            // ---- the plural, the vowel-plural only ----
            cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, stem, ["jen", "jen"], false).on_ending(["je", "je"]),
            cell(Case::Partitive, pl, stem, ["ja", "jä"], true),
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
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(stem);

    b.into_forms()
}

/// The graded class-8 subtypes (8*A, 8*B, 8*C). A graded class-8 noun is an `-e`
/// loanword with consonant gradation (strong = the lemma, weak = `grad`
/// applied, `nukke -> nuke`). As in class 1*A, strong feeds the
/// nominative/partitive/essive/illative and the genitive plural; weak feeds the
/// genitive, translative, the local cases and the nominative plural. The
/// genitive plural keeps `-jen` and the partitive `-ja`, the vowel-plural, like
/// base class 8.
pub fn inflect_graded(lemma: &str, possessives: bool, grad: Gradation) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let strong = lemma;
    let weak = &grad.apply(strong, Grade::Weak);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The singular illative doubles the strong stem's own last letter: nukkeen.
    let elast = strong.chars().last().unwrap();

    b.decline(
        &Decl::new(vec![
            // ---- singular, strong grade ----
            cell(Case::Nominative, sg, strong, ["", ""], false).on_stem(),
            cell(Case::Partitive, sg, strong, ["a", "ä"], true),
            cell(Case::Essive, sg, strong, ["na", "nä"], true),
            cell(
                Case::Illative,
                sg,
                strong,
                Ending::Given(format!("{elast}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{elast}"))),
            // ---- singular, weak grade ----
            // Genitive bare is weak (nuken) but its possessives sit on the
            // strong stem (nukkeni).
            cell(Case::Genitive, sg, weak, ["n", "n"], false).base_on(strong.to_string(), ["", ""]),
            cell(Case::Translative, sg, weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, weak, ["tta", "ttä"], true),
            // ---- plural, strong grade ----
            // The genitive plural keeps the strong stem + -jen: nukkejen.
            cell(Case::Genitive, pl, strong, ["jen", "jen"], false).on_ending(["je", "je"]),
            cell(Case::Partitive, pl, strong, ["ja", "jä"], true),
            cell(Case::Essive, pl, strong, ["ina", "inä"], true),
            cell(Case::Illative, pl, strong, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            // ---- plural, weak grade ----
            // Nominative bare is weak (nuket), possessives strong (nukkeni).
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
    b.comitative(strong);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn e_stem_lemma() {
        let w = words("agaave");
        assert!(w.contains(&"agaaven".to_string())); // gen
        assert!(w.contains(&"agaavea".to_string())); // part
        assert!(w.contains(&"agaaveen".to_string())); // illat, doubled e
        assert!(w.contains(&"agaavet".to_string())); // nom pl
        assert!(w.contains(&"agaavejen".to_string())); // gen pl, -jen
    }

    #[test]
    fn vowel_plural_only() {
        let w = words("akne");
        assert!(w.contains(&"aknejen".to_string()));
        assert!(w.contains(&"akneja".to_string()));
        assert!(
            !w.iter()
                .any(|f| f.contains("akneita") || f.contains("akneiden"))
        );
    }

    /// The graded grid crosses over twice, and both crossovers are the strong
    /// genitive rule: `nukke`'s genitive is `nuken` but `nukkeni`, and its
    /// nominative plural is `nuket` but `nukkeni`. The plural's genitive stays on
    /// the strong stem (`nukkejen`) while its locals are weak (`nukessa`).
    #[test]
    fn the_graded_grid_crosses_over_twice() {
        let w: Vec<String> = inflect_graded("nukke", true, Gradation(1))
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(w.contains(&"nuken".to_string())); // gen sg bare: weak
        assert!(w.contains(&"nukkeni".to_string())); // gen sg 1sg: strong
        assert!(w.contains(&"nuket".to_string())); // nom pl bare: weak
        assert!(w.contains(&"nukkejen".to_string())); // gen pl: strong
        assert!(w.contains(&"nukessa".to_string())); // iness pl: weak
    }
}
