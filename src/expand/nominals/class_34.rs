//! Class 34 (the alaston type) inflection, written by hand as a logical engine.
//!
//! Class 34 is the privative `-ton`/`-tön` family (alaston, onneton): the
//! lemma carries the partitive and the `-ten` genitive plural (`alastonta`,
//! `alastonten`), and the oblique stem is the lemma with its `-ton` tail
//! rewritten. The shared engine is in [`inflect_ton`], parameterised by that
//! oblique stem.
//!
//! The base class does *not* geminate: `alaston` gives `alastoman`, `alastomia`
//! and `alastomaan`, with the single `t` of the lemma. The `-tt-` belongs to
//! the gradated `34*C` subtype (`onneton` -> `onnettoman`, `rahaton` ->
//! `rahattoman`), which supplies its own stem through
//! [`geminating_oblique`]. Treating base 34 as geminating invented
//! `alasttoman` and lost every plural `alastomia`.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "34";

const _: () = ();

/// Inflect a class-34 noun (`-ton` -> `-ttoma`: onneton -> onnettoman).
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_ton(lemma, possessives, &oblique(lemma))
}

/// The non-geminating `-ton`/`-tön` oblique stem: alaston -> alastoma.
pub fn oblique(lemma: &str) -> String {
    lemma.replace("ton", "toma").replace("tön", "tömä")
}

/// The geminating oblique stem of the `34*C` subtype: onneton -> onnettoma,
/// rahaton -> rahattoma.
pub fn geminating_oblique(lemma: &str) -> String {
    lemma.replace("ton", "ttoma").replace("tön", "ttömä")
}

/// Inflect a `-ton` noun with the given oblique (weak) stem replacing the
/// `-ton` tail (`alasttoma`, `aatelittoma`).
pub fn inflect_ton(lemma: &str, possessives: bool, weak: &str) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let graded = weak[..weak.len() - weak.chars().last().unwrap().len_utf8()].to_string();
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular ----
            // The bare nominative and its possessives ride the lemma.
            cell(Case::Nominative, sg, lemma, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, weak, ["n", "n"], false).on_stem(),
            // The partitive stays on the lemma: alaston -> alastonta.
            cell(Case::Partitive, sg, lemma, ["ta", "tä"], true),
            cell(Case::Essive, sg, weak, ["na", "nä"], true),
            cell(Case::Translative, sg, weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, weak, ["an", "än"], false).on_ending(["a", "ä"]),
            cell(Case::Inessive, sg, weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, weak, ["tta", "ttä"], true),
            // ---- the plural ----
            cell(Case::Nominative, pl, weak, ["t", "t"], false).on_stem(),
            cell(Case::Genitive, pl, &graded, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            // The `-ten` genitive plural goes back to the lemma: alastonten.
            cell(Case::Genitive, pl, lemma, ["ten", "ten"], false).on_ending(["te", "te"]),
            cell(Case::Partitive, pl, &graded, ["ia", "iä"], true),
            cell(Case::Essive, pl, &graded, ["ina", "inä"], true),
            cell(Case::Translative, pl, &graded, ["iksi", "iksi"], true)
                .on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, &graded, ["iin", "iin"], false).on_ending(["ii", "ii"]),
            cell(Case::Inessive, pl, &graded, ["issa", "issä"], true),
            cell(Case::Elative, pl, &graded, ["ista", "istä"], true),
            cell(Case::Adessive, pl, &graded, ["illa", "illä"], true),
            cell(Case::Ablative, pl, &graded, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, &graded, ["ille", "ille"], true),
            cell(Case::Abessive, pl, &graded, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, &graded, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&graded);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn ton() {
        let w = words("alaston");
        assert!(w.contains(&"alastonta".to_string()));
        assert!(w.contains(&"alastonten".to_string()));
    }

    /// Base 34 keeps the single `t` of the lemma (`alastoman`), and the
    /// plural partitive `alastomia` must be there rather than the invented
    /// `alasttomia`.
    #[test]
    fn the_base_oblique_does_not_geminate() {
        let w = words("alaston");
        assert!(w.contains(&"alastoman".to_string()), "oblique missing");
        assert!(w.contains(&"alastomien".to_string()));
        assert!(w.contains(&"alastomiin".to_string()));
        assert!(w.contains(&"alastomia".to_string()));
        assert!(
            !w.contains(&"alasttoman".to_string()),
            "geminated oblique emitted"
        );
    }

    /// The gradated subtype is the one that doubles the `t`.
    #[test]
    fn the_gradated_oblique_doubles_the_t() {
        let w: Vec<String> = super::super::class_34C::inflect("rahaton", true)
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(w.contains(&"rahattoman".to_string()));
        assert!(w.contains(&"rahattomia".to_string()));
    }
}
