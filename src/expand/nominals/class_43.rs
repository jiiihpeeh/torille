//! Class 43 (the airut type) inflection, written by hand as a logical engine.
//!
//! Class 43 is the `-ut`/`-yt` family (airut, immyt): the final `t` reads `e`
//! (`airut -> airue`), whose `e`-stem carries the singular obliques
//! (`airuen`, `airuessa`) and the plural (`airuiden`), while the lemma keeps
//! the partitive (`airutta`).
//!
//! The shape is the familiar one: `e`-stem singular, bare consonant stem
//! plural, two genitive alternants (`-iden`/`-itten`) and two illatives
//! (`-ihin`/`-isiin`).

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "43";

/// Inflect a class-43 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let estem = format!("{}e", &lemma[..lemma.len() - 1]); // airue
    let base = &estem[..estem.len() - 1]; // airu
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the e-stem ----
            // The bare nominative is the lemma; its possessives ride the e-stem.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(estem.clone(), ["", ""]),
            cell(Case::Genitive, sg, &estem, ["n", "n"], false).on_stem(),
            // The partitive is the bare consonant stem with `-tta`.
            cell(Case::Partitive, sg, base, ["tta", "ttä"], true),
            cell(Case::Essive, sg, &estem, ["na", "nä"], true),
            cell(Case::Translative, sg, &estem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, sg, &estem, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Inessive, sg, &estem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &estem, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &estem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &estem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &estem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &estem, ["tta", "ttä"], true),
            // ---- the plural, on the bare consonant stem with the i-plural ----
            // The plural nominative's possessives ride the e-stem
            // (`neitsyeni`, `neitsyensä`), not the bare consonant stem
            // (`*neitsyni`, `*neitsynsä`).
            cell(Case::Nominative, pl, &estem, ["t", "t"], false)
                .base_on(estem.clone(), ["", ""]),
            cell(Case::Genitive, pl, base, ["iden", "iden"], false).on_ending(["ide", "ide"]),
            cell(Case::Genitive, pl, base, ["itten", "itten"], false).on_ending(["itte", "itte"]),
            cell(Case::Partitive, pl, base, ["ita", "itä"], true),
            cell(Case::Essive, pl, base, ["ina", "inä"], true),
            cell(Case::Translative, pl, base, ["iksi", "iksi"], true).on_ending(["ikse", "ikse"]),
            cell(Case::Illative, pl, base, ["ihin", "ihin"], false).on_ending(["ihi", "ihi"]),
            cell(Case::Illative, pl, base, ["isiin", "isiin"], false).on_ending(["isii", "isii"]),
            cell(Case::Inessive, pl, base, ["issa", "issä"], true),
            cell(Case::Elative, pl, base, ["ista", "istä"], true),
            cell(Case::Adessive, pl, base, ["illa", "illä"], true),
            cell(Case::Ablative, pl, base, ["ilta", "iltä"], true),
            cell(Case::Allative, pl, base, ["ille", "ille"], true),
            cell(Case::Abessive, pl, base, ["itta", "ittä"], true),
            plain(Case::Instructive, pl, base, ["in", "in"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(base);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn airut() {
        let w = words("airut");
        assert!(w.contains(&"airuen".to_string()));
        assert!(w.contains(&"airutta".to_string()));
        assert!(w.contains(&"airuiden".to_string()));
        assert!(w.contains(&"airuita".to_string()));
    }

    /// The singular rides the `e`-stem, the plural the bare consonant stem.
    #[test]
    fn the_e_stem_and_the_bare_stem_split() {
        let w = words("airut");
        assert!(w.contains(&"airuessa".to_string()));
        assert!(w.contains(&"airuissa".to_string()));
        assert!(w.contains(&"airuitten".to_string()));
    }

    /// The plural nominative's possessive column is on the e-stem:
    /// `neitsyeni`, not `*neitsyni`.
    #[test]
    fn the_plural_nominative_possessives_use_the_e_stem() {
        let w = words("neitsyt");
        for good in ["neitsyeni", "neitsyesi", "neitsyensä"] {
            assert!(w.contains(&good.to_string()), "neitsyt is missing {good}");
        }
        for bad in ["neitsyni", "neitsysi", "neitsynsä"] {
            assert!(!w.contains(&bad.to_string()), "neitsyt must not offer {bad}");
        }
    }
}
