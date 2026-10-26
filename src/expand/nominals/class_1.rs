//! Class 1 (the valo type) inflection, written by hand as a logical engine.
//!
//! A class-1 noun is its whole word as its stem, with no consonant gradation.
//! Every case is `stem + ending`; the ending's a/ä or o/ö is chosen by the
//! word's harmony, and the illative doubles the stem's final vowel. Because the
//! stem is the lemma, all of class 1 falls out of this shape: `talo` (back) and
//! `äly` (front) differ only in which harmony the endings take.
//!
//! The paradigm is stated as a [`Decl`] -- twenty-four cells, each naming the
//! stem it is built on, the ending that joins it and what its possessives hang
//! off -- and [`Builder::decline`] drives the grid, which is what puts the
//! possessive column on every cell. The possessive bases are the exceptions
//! worth naming: the genitive's sit on the stem (`taloni`, never `*talonani`)
//! and the illative's on the doubled stem, while the translative's and the
//! genitive plural's sit on the oblique stem (`talokse`, `taloje`).

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, doubled_stem, nominal_tags, plain, rare};

pub const CLASS: &str = "1";

/// Inflect a class-1 noun (stem = the lemma, no gradation) into every cell.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // A handful of class-1 lemmas are plural-only: `housut`, `aivot`,
    // `kasvot`, `sakset`-like party words (`bailut`, `etkot`). The sanalista
    // lists their nominative plural as the lemma and gives behind it the same
    // class as an ordinary singular `kala`, so the singular cells below would
    // invent `housu`, `housun`, `housua` and the plural ends up on the wrong
    // stem (`housutja`). Drop the final `-t` for the stem and emit only the
    // plural cells; the nominative is then `housu + t`, which is the lemma.
    let plural_only = lemma.ends_with('t');
    let stem = if plural_only {
        lemma.strip_suffix('t').unwrap_or(lemma)
    } else {
        lemma
    };

    let mut cells = Vec::new();
    if !plural_only {
        cells.extend([
            // ---- the singular ----
            cell(Case::Nominative, sg, stem, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, stem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, stem, ["a", "ä"], true),
            cell(Case::Essive, sg, stem, ["na", "nä"], true),
            cell(Case::Translative, sg, stem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Inessive, sg, stem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, stem, ["sta", "stä"], true),
            // The illative doubles the stem's final vowel and adds `n`:
            // talo -> taloon, and its possessives hang off the doubled stem
            // (talooni), so the cell is stated on the doubled stem.
            cell(Case::Illative, sg, &doubled_stem(stem), ["n", "n"], false).on_stem(),
            cell(Case::Adessive, sg, stem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, stem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, stem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, stem, ["tta", "ttä"], true),
        ]);
    }
    cells.extend([
        // ---- the plural ----
        cell(Case::Nominative, pl, stem, ["t", "t"], false).on_stem(),
        // The plural genitive is `-jen` (talojen) for every class-1 word;
        // that is the only spelling the oracle confirms. The loanword
        // alternates `-iden`/`-itten` (piano -> pianoiden/pianoitten) and
        // the `-ita` partitive plural are real Finnish but confirmed only
        // for a handful of `-i` loanwords (piano), so they are offered at a
        // lower confidence -- still suggested, never as a plain correction
        // the reference does not vouch for.
        cell(Case::Genitive, pl, stem, ["jen", "jen"], false).on_ending(["je", "je"]),
        rare(Case::Genitive, pl, stem, ["iden", "iden"]),
        rare(Case::Genitive, pl, stem, ["itten", "itten"]),
        cell(Case::Partitive, pl, stem, ["ja", "jä"], true),
        rare(Case::Partitive, pl, stem, ["ita", "itä"]),
        cell(Case::Essive, pl, stem, ["ina", "inä"], true),
        cell(Case::Translative, pl, stem, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
        cell(Case::Inessive, pl, stem, ["issa", "issä"], true),
        cell(Case::Elative, pl, stem, ["ista", "istä"], true),
        cell(Case::Illative, pl, stem, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
        cell(Case::Adessive, pl, stem, ["illa", "illä"], true),
        cell(Case::Ablative, pl, stem, ["ilta", "iltä"], true),
        cell(Case::Allative, pl, stem, ["ille", "ille"], true),
        cell(Case::Abessive, pl, stem, ["itta", "ittä"], true),
        // The instructive belongs to no case's shape -- `tloin` is
        // `stem + in` and nothing else ends that way -- so it is the one
        // plain form the grid does not account for.
        plain(Case::Instructive, pl, stem, ["in", "in"]),
    ]);

    b.decline(&Decl::new(cells), |case, number, poss| {
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

    /// A class-1 lemma ending in `-t` is a plural-only noun. `housut` must
    /// yield `housuja`, not the old `housutja`/`housutn`, and must not invent
    /// a singular `housu`.
    #[test]
    fn a_plural_only_lemma_is_declined_from_the_dropped_stem() {
        let forms = words("housut");
        for want in ["housut", "housuja", "housujen", "housuissa", "housuihin"] {
            assert!(
                forms.iter().any(|f| f == want),
                "missing housut form {want}"
            );
        }
        for bad in ["housu", "housun", "housua", "housutja", "housutn"] {
            assert!(
                !forms.iter().any(|f| f == bad),
                "invented housut form {bad}"
            );
        }
    }

    #[test]
    fn other_plural_only_class_one_lemmas_follow_the_same_rule() {
        let forms = words("aivot");
        assert!(forms.iter().any(|f| f == "aivoja"));
        assert!(forms.iter().any(|f| f == "aivojen"));
        assert!(!forms.iter().any(|f| f == "aivotja"));
    }
}
