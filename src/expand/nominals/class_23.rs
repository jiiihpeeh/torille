//! Class 23 (the `jouhi` type) inflection, written by hand as a logical engine.
//!
//! The `-i` nouns that keep a consonant stem: `jouhi` has an `e`-stem
//! (`jouhe`) for its cases, an `i`-stem (`jouhi`, the lemma) for its plural, and
//! a consonant stem (`jouh`) for the singular partitive (`jouhta`).
//!
//! Three stems and the class is which cells take which, which is what the
//! [`Decl`] says. Two of its cells are the reason the machinery has
//! `Entry::Owned`: the translative's bare is `jouhe` + `-ksi` while its
//! possessives come off the `i`-stem (`jouhikseen`), and the `e`-stem spelling
//! is offered beside it (`jouhekseen`). Both columns exist and the reference
//! confirms the second (`merekseen`, `liemekseen`, `niemekseen`).

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, e_stem, nominal_tags, plain, possessed};

pub const CLASS: &str = "23";

/// The consonant stem: the lemma with its final `i` dropped (`jouhi -> jouh`),
/// which the singular partitive is built on (`jouhta`).
fn c_stem(lemma: &str) -> String {
    let mut s = lemma.to_string();
    s.pop(); // drop the final -i
    s
}

/// Inflect a class-23 noun into every cell.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let istem = lemma; // jouhi
    let estem = e_stem(istem); // jouhe
    let cstem = c_stem(lemma); // jouh
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the e-stem ----
            // The nominative's bare form is the `i`-lemma; its possessives ride
            // the `e`-stem (`jouheni`), which is what the reference confirms for
            // every word of the class.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(estem.clone(), ["", ""]),
            cell(Case::Genitive, sg, &estem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, &cstem, ["ta", "tä"], true),
            cell(Case::Essive, sg, &estem, ["na", "nä"], true),
            // The translative's possessives sit on the `i`-stem: jouhikseen.
            cell(Case::Translative, sg, &estem, ["ksi", "ksi"], true)
                .base_on(format!("{istem}kse"), ["", ""]),
            // The e-stem spelling (jouhekseen) is a column of its own: it has no
            // bare form, only the five owners.
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
            // Genitive plural: only the -ien form (no -ten) for this family.
            cell(Case::Genitive, pl, istem, ["en", "en"], false).on_ending(["e", "e"]),
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

    /// Three stems, three different cells: the singular's cases on the `e`-stem,
    /// the partitive on the consonant stem, the plural on the `i`-stem. The
    /// sanalista confirms all of them for `jouhi`.
    #[test]
    fn hi_type() {
        let w = words("jouhi");
        assert!(w.contains(&"jouhen".to_string())); // gen sg, e-stem
        assert!(w.contains(&"jouhta".to_string())); // part sg, consonant stem
        assert!(w.contains(&"jouheen".to_string())); // ill sg
        assert!(w.contains(&"jouhien".to_string())); // gen pl, i-stem
        assert!(w.contains(&"jouhissa".to_string())); // iness pl
    }

    /// The two translative columns, and the one the reference confirms: the
    /// `i`-stem's `jouhikse...` is the cell's own base, while `jouhekseen` is a
    /// column with no bare form of its own.
    #[test]
    fn the_translative_offers_both_stems() {
        let w = words("jouhi");
        assert!(
            w.contains(&"jouheksi".to_string()),
            "the bare form is the e-stem's"
        );
        assert!(
            w.contains(&"jouhikseen".to_string()),
            "the i-stem's third person"
        );
        assert!(w.contains(&"jouhekseen".to_string()), "and the e-stem's");
        assert!(w.contains(&"jouhikseni".to_string()), "the i-stem's -ni");
        assert!(w.contains(&"jouhekseni".to_string()), "and the e-stem's");
    }

    /// The nominative's possessives ride the `e`-stem, and the claim is about
    /// the class, so the fixture is the class: every `-hi` word the sanalista
    /// confirms a nominative possessive for.
    ///
    /// This and class 25 had the `i`-stem here (`jouhini`, `liemini`), which no
    /// word of either class confirms -- and classes 24, 26 and 27, the rest of
    /// the `-i` family, already had the `e`-stem (`käteni`, `kauteni`,
    /// `mereni`), so this was an oversight in two modules rather than a rule.
    /// `moni` is the tenth word of the class and the sanalista records no
    /// nominative possessive for it at all, so it is not in the list.
    #[test]
    fn the_nominative_possessives_ride_the_e_stem() {
        let confirmed = [
            ("jouhi", "jouheni"),
            ("lohi", "loheni"),
            ("riihi", "riiheni"),
            ("syli", "syleni"),
            ("tiili", "tiileni"),
            ("tuli", "tuleni"),
            ("tuohi", "tuoheni"),
            ("uuhi", "uuheni"),
            ("vuohi", "vuoheni"),
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
