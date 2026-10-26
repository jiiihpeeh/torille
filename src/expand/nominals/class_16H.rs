//! Class 16*H (the vanhempi type) inflection, written by hand.
//!
//! The comparative of the `-mp`/`-mm` nouns: `vanhempi` gradates the way its
//! positive does (`Gradation(8)`, `mp -> mm`) and additionally carries the
//! `a`-stem that the `-i` comparatives are built on. The `a`/`ä` of that stem
//! reads the word's own harmony, so `aiempi` gives `aiempani` and `idempi`
//! gives `idempä`.
//!
//! The nominative is the cell to watch. Its bare form is the `-i` lemma
//! (`vanhempi`) but its possessives ride the `a`-stem (`vanhempani`,
//! `vanhempansa`), never the lemma (`vanhempini`, `vanhepinsa`). The same is
//! true of the genitive and the plural nominative, so three cells cross over
//! from the `i`-stem onto the `a`-stem, and the class would not be a
//! comparative at all without it.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, doubled_stem, nominal_tags, plain, rare};

pub const CLASS: &str = "16*H";

/// Inflect a class 16*H comparative.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let strong = lemma;
    let grad = Gradation(8); // mp -> mm
    let wmi = grad.apply(strong, Grade::Weak); // vanhempi -> vanhemmi
    let mut astrong = strong.to_string();
    astrong.pop(); // vanhempi -> vanhemp
    // The `-i` reads `a`/`ä` by the word's own harmony: `aiempi` (back) ->
    // `aiempa`, `idempi` (front) -> `idempä`.
    astrong.push(if b.is_front() { 'ä' } else { 'a' });
    let wma = grad.apply(&astrong, Grade::Weak); // -> vanhemma
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- singular ----
            // The nominative's possessives ride the `a`-stem, not the `i` lemma:
            // `aiempi` is `aiempani`/`aiempansa`, never `aiempini`/`aiempinsa`.
            cell(Case::Nominative, sg, strong, ["", ""], false).base_on(astrong.clone(), ["", ""]),
            cell(Case::Genitive, sg, &wma, ["n", "n"], false).base_on(astrong.clone(), ["", ""]),
            cell(Case::Partitive, sg, &astrong, ["a", "ä"], true),
            cell(Case::Essive, sg, &astrong, ["na", "nä"], true),
            cell(Case::Translative, sg, &wma, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The illative doubles the `a`-stem's `a`, as every `-i` noun does:
            // vanhempa -> vanhempaan.
            cell(
                Case::Illative,
                sg,
                &doubled_stem(&astrong),
                ["n", "n"],
                false,
            )
            .on_stem(),
            cell(Case::Inessive, sg, &wma, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &wma, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &wma, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &wma, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &wma, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &wma, ["tta", "ttä"], true),
            // ---- plural ----
            cell(Case::Nominative, pl, &wma, ["t", "t"], false).base_on(astrong.clone(), ["", ""]),
            cell(Case::Genitive, pl, strong, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Partitive, pl, strong, ["a", "ä"], true),
            cell(Case::Essive, pl, strong, ["na", "nä"], true),
            cell(Case::Translative, pl, &wmi, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, pl, strong, ["in", "in"], false).on_ending("i"),
            cell(Case::Inessive, pl, &wmi, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, &wmi, ["sta", "stä"], true),
            cell(Case::Adessive, pl, &wmi, ["lla", "llä"], true),
            cell(Case::Ablative, pl, &wmi, ["lta", "ltä"], true),
            cell(Case::Allative, pl, &wmi, ["lle", "lle"], true),
            cell(Case::Abessive, pl, &wmi, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, &wmi, ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&wmi);

    // The short genitive plural is rare: generated, never offered as a plain
    // correction (vanhempain). It comes after the comitative in the live engine's
    // output and the order is observable, so it is a second pass rather than a
    // twelfth entry that would move the comitative after it.
    b.decline(
        &Decl::new(vec![rare(Case::Genitive, pl, &astrong, ["in", "in"])]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// The crossover is the class: `vanhempi` bare, `vanhempani` possessed.
    #[test]
    fn the_nominative_crosses_from_the_i_stem_to_the_a_stem() {
        let w = words("vanhempi");
        assert!(
            w.contains(&"vanhempi".to_string()),
            "nominative is the lemma"
        );
        assert!(
            w.contains(&"vanhempani".to_string()),
            "and its -ni is on the a-stem"
        );
        assert!(!w.contains(&"vanhempini".to_string()), "not on the lemma");
    }

    #[test]
    fn comparative() {
        let w = words("vanhempi");
        assert!(w.contains(&"vanhemman".to_string()));
        assert!(w.contains(&"vanhemmaksi".to_string()));
        assert!(w.contains(&"vanhemmat".to_string()));
        assert!(w.contains(&"vanhemmiksi".to_string()));
    }

    /// The genitive crosses over the same way, and it is the cell the reference
    /// confirms most often: `idemmän` bare, `idempäni` possessed. The `ä` shows
    /// that the `a`-stem reads the word's own harmony rather than a fixed vowel.
    #[test]
    fn a_front_word_reads_ä_on_the_a_stem() {
        let w = words("idempi");
        assert!(
            w.contains(&"idemmän".to_string()),
            "genitive is on the weak grade"
        );
        assert!(
            w.contains(&"idempäni".to_string()),
            "and its -ni is on the a-stem"
        );
    }
}
