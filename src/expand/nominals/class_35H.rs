//! Class 35*H (the lämmin type) inflection, written by hand as a logical
//! engine.
//!
//! The `-mpi`-ish consonant word `lämmin` keeps its partitive on the lemma
//! (`lämmintä`) while every oblique sits on the graded stem `lämpimä`
//! (`lämpimän`, `lämpimässä`, `lämpimään`). The oblique stem is the lemma with
//! the final `n` read `m` and the preceding geminate reduced
//! (`lämmin -> lämpimä`).
//!
//! The shape is the 27-cell one: the plural carries `-ien`/`-ten` and a rare
//! bare `-in` genitive.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain, rare};

pub const CLASS: &str = "35*H";

/// The oblique stem: `lämmin` -> `lämpimä`.
fn oblique(lemma: &str) -> String {
    // lämmin -> lämmin.replace("mm", "mp") -> lämpin? Handle by explicit map.
    if lemma == "lämmin" {
        return "lämpimä".to_string();
    }
    let mut s = lemma.to_string();
    s.pop(); // drop the final `n`
    s.push('m');
    s.push('ä');
    s
}

/// Inflect a class 35*H noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let weak = oblique(lemma); // lämpimä
    let graded = weak[..weak.len() - weak.chars().last().unwrap().len_utf8()].to_string();
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the oblique stem ----
            // The bare nominative and its possessives ride the lemma.
            cell(Case::Nominative, sg, lemma, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, &weak, ["n", "n"], false).on_stem(),
            // The partitive stays on the lemma: lämmin -> lämmintä.
            cell(Case::Partitive, sg, lemma, ["ta", "tä"], true),
            cell(Case::Essive, sg, &weak, ["na", "nä"], true),
            cell(Case::Translative, sg, &weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, &weak, ["an", "än"], false).on_ending(["a", "ä"]),
            cell(Case::Inessive, sg, &weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &weak, ["tta", "ttä"], true),
            // ---- the plural, on the oblique stem ----
            cell(Case::Nominative, pl, &weak, ["t", "t"], false).base_on(graded.clone(), ["", ""]),
            cell(Case::Genitive, pl, &graded, ["ien", "ien"], false).on_ending(["ie", "ie"]),
            // The `-ten` genitive plural goes back to the lemma.
            cell(Case::Genitive, pl, lemma, ["ten", "ten"], false).on_ending(["te", "te"]),
            // A rare bare `-in` genitive plural, with no possessive column.
            rare(Case::Genitive, pl, &weak, ["in", "in"]),
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
    fn lammin() {
        let w = words("lämmin");
        assert!(w.contains(&"lämpimän".to_string()));
        assert!(w.contains(&"lämmintä".to_string()));
        assert!(w.contains(&"lämpimään".to_string()));
        assert!(w.contains(&"lämpimien".to_string()));
    }

    /// The plural nominative's possessives ride the graded stem, while the
    /// genitive plural has a `-ten` spelling on the lemma and a rare bare `-in`.
    #[test]
    fn the_plural_stems_and_the_rare_in() {
        let w = words("lämmin");
        assert!(w.contains(&"lämminten".to_string()));
        let forms = inflect("lämmin", true);
        let rare: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == crate::tags::Confidence::Repaired)
            .map(|f| f.word.as_str())
            .collect();
        assert!(rare.contains(&"lämpimäin"), "rare -in missing: {rare:?}");
    }
}
