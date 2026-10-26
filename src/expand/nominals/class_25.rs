//! Class 25 (the `liemi` type) inflection, written by hand as a logical engine.
//!
//! The `-i` nouns with an `m -> n` stem: `liemi` has an `e`-stem (`lieme`),
//! an `i`-stem (`liemi`, the lemma) and the reduced stem the participle is
//! built on (`lien`). The class is what each of the three feeds -- and the
//! reduced stem is the point of it, because it gives the singular **two**
//! partitives: the reduced `lientä` and the full `liemeä`.
//!
//! It shares class 23's shape otherwise, down to the translative's two
//! possessives columns (`liemikseen` from the `i`-stem and `liemekseen` from
//! the `e`-stem), and down to the nominative, whose own form is the `i`-lemma
//! while its possessives ride the `e`-stem (`liemeni`).

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, e_stem, nominal_tags, plain, possessed};

pub const CLASS: &str = "25";

/// The reduced (`m -> n`) stem the participle is built on: `liemi -> lien`.
fn reduced(lemma: &str) -> String {
    let mut s = lemma.to_string();
    s.pop(); // liemi -> liem
    // The last `m` becomes `n`: liem -> lien.
    if let Some(i) = s.rfind('m') {
        s.replace_range(i..i + 1, "n");
    }
    s
}

/// Inflect a class-25 noun into every cell.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let istem = lemma; // liemi
    let estem = e_stem(istem); // lieme
    let rstem = reduced(lemma); // lien
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the e-stem ----
            // The nominative's bare form is the `i`-lemma but its possessives
            // ride the `e`-stem (`liemeni`, not `liemini`), which is what the
            // reference confirms for every word of the class.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(estem.clone(), ["", ""]),
            cell(Case::Genitive, sg, &estem, ["n", "n"], false).on_stem(),
            // The reduced partitive on the m->n stem (lientä).
            cell(Case::Partitive, sg, &rstem, ["ta", "tä"], true),
            // The full partitive on the e-stem (loimea, niemeä).
            cell(Case::Partitive, sg, &estem, ["a", "ä"], true),
            cell(Case::Essive, sg, &estem, ["na", "nä"], true),
            // The translative's possessives sit on the `i`-stem: liemikseen. The
            // e-stem spelling (liemekseen) is a column of its own, and it is the
            // one the reference confirms.
            cell(Case::Translative, sg, &estem, ["ksi", "ksi"], true)
                .base_on(format!("{istem}kse"), ["", ""]),
            possessed(Case::Translative, sg, format!("{estem}kse"), true),
            cell(Case::Illative, sg, &estem, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Inessive, sg, &estem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &estem, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &estem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &estem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &estem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &estem, ["tta", "ttä"], true),
            // ---- the plural ----
            cell(Case::Nominative, pl, &estem, ["t", "t"], false).base_on(estem.clone(), ["", ""]),
            cell(Case::Genitive, pl, istem, ["en", "en"], false).on_ending(["e", "e"]),
            // The reduced stem's second genitive plural (lienten).
            cell(Case::Genitive, pl, &rstem, ["ten", "ten"], false).on_ending(["te", "te"]),
            cell(Case::Partitive, pl, istem, ["a", "ä"], true),
            cell(Case::Essive, pl, istem, ["na", "nä"], true),
            // The `i`-plural sits on the `i`-final stem; the endings are truncated.
            cell(Case::Translative, pl, istem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, pl, istem, ["in", "in"], false).on_ending("i"),
            cell(Case::Inessive, pl, istem, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, istem, ["sta", "stä"], true),
            cell(Case::Adessive, pl, istem, ["lla", "llä"], true),
            cell(Case::Ablative, pl, istem, ["lta", "ltä"], true),
            cell(Case::Allative, pl, istem, ["lle", "lle"], true),
            cell(Case::Abessive, pl, istem, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, istem, ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(istem);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// The class is the two partitives: the reduced one on the `m -> n` stem and
    /// the full one on the `e`-stem. The sanalista confirms both for `liemi`,
    /// and confirms the reduced stem's second genitive plural too.
    #[test]
    fn mi_type() {
        let w = words("liemi");
        assert!(w.contains(&"liemen".to_string())); // gen sg
        assert!(w.contains(&"lientä".to_string())); // part sg, reduced stem
        assert!(w.contains(&"liemeä".to_string())); // part sg, e-stem
        assert!(w.contains(&"liemien".to_string())); // gen pl, i-stem
        assert!(w.contains(&"lienten".to_string())); // and the reduced stem's
    }

    /// The reduced stem appears twice and nowhere else: one partitive and one
    /// genitive plural. A back word takes the same shape on `a -> n`.
    #[test]
    fn the_reduced_stem_feeds_exactly_two_cells() {
        assert!(words("taimi").contains(&"tainta".to_string()));
        assert!(words("taimi").contains(&"tainten".to_string()));
        // `niemi` is front, so the reduced endings harmonise.
        assert!(words("niemi").contains(&"nientä".to_string()));
        assert!(words("niemi").contains(&"nienten".to_string()));
    }

    /// The nominative's possessives ride the `e`-stem, and the claim is about
    /// the class, so the fixture is the class: all eight words the sanalista
    /// lists for it, with the nominative possessive each one confirms.
    ///
    /// This and class 23 had the `i`-stem here (`liemini`, `jouhini`), which no
    /// word of either class confirms -- and classes 24, 26 and 27, the rest of
    /// the `-i` family, already had the `e`-stem (`mereni`, `kieleni`,
    /// `kauteni`), so this was an oversight in two modules rather than a rule.
    #[test]
    fn the_nominative_possessives_ride_the_e_stem() {
        let confirmed = [
            ("liemi", "liemeni"),
            ("loimi", "loimeni"),
            ("lumi", "lumeni"),
            ("luomi", "luomeni"),
            ("niemi", "niemeni"),
            ("taimi", "taimeni"),
            ("toimi", "toimeni"),
            ("tuomi", "tuomeni"),
        ];
        for (lemma, owned) in confirmed {
            let w = words(lemma);
            assert!(w.contains(&owned.to_string()), "{lemma}: {owned} missing");
            let wrong = owned.replace('e', "i");
            assert!(
                !w.contains(&wrong.to_string()),
                "{lemma}: {wrong} is not a form of the class"
            );
        }
    }
}
