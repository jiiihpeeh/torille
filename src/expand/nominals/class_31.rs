//! Class 31 (the haaksi type) inflection, written by hand.
//!
//! The `-ksi` word haaksi turns the tail to `hde`/`ht`/`hte`: from the lemma
//! (`haaksi`) the oblique stem `haahde` carries the genitive/locals, the
//! essive/illative stem `haahte` and partitive stem `haaht` (part `haahta`);
//! the plural sits on the lemma (`haaksien`, `haaksia`).
//!
//! This is the first port that needs a possessive column with no bare form of
//! its own. The singular translative's bare is `haahdeksi` while its possessives
//! hang off the *lemma* stem (`haaksikseen`), which is neither the cell's stem
//! nor its base, so [`possessed`] says the column on its own rather than
//! smuggling a bare form in beside it.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain, possessed};

pub const CLASS: &str = "31";

/// The partitive/essive stems from the lemma: `haaksi` -> `haahd`? (haa+hde).
fn stems(lemma: &str) -> (String, String, String, String) {
    let base = lemma.strip_suffix("ksi").unwrap_or(lemma); // haa
    (
        lemma.to_string(),
        format!("{base}hde"),
        format!("{base}ht"),
        format!("{base}hte"),
    )
}

/// Inflect a class-31 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let mut b = Builder::new(lemma, possessives);
    let (s, w, p, e) = stems(lemma);
    let (sg, pl) = (Number::Singular, Number::Plural);

    b.decline(
        &Decl::new(vec![
            // ---- singular ----
            cell(Case::Nominative, sg, &s, ["", ""], false).on_stem(),
            cell(Case::Genitive, sg, &w, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, &p, ["a", "ä"], true),
            cell(Case::Essive, sg, &e, ["na", "nä"], true),
            cell(Case::Translative, sg, &w, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The translative's possessives sit on the lemma stem: haaksikseen.
            possessed(Case::Translative, sg, format!("{s}kse"), true),
            cell(Case::Illative, sg, &e, ["en", "en"], false).on_ending("e"),
            cell(Case::Inessive, sg, &w, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &w, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &w, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &w, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &w, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &w, ["tta", "ttä"], true),
            // ---- plural, on the lemma ----
            // The nominative's bare is on the `hde` stem (`haahdet`) but its
            // possessives are on the lemma: `haaksini`, not `haahdeni`.
            cell(Case::Nominative, pl, &w, ["t", "t"], false).base_on(s.clone(), ["", ""]),
            cell(Case::Genitive, pl, &s, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Partitive, pl, &s, ["a", "ä"], true),
            cell(Case::Essive, pl, &s, ["na", "nä"], true),
            cell(Case::Translative, pl, &s, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, pl, &s, ["in", "in"], false).on_ending("i"),
            cell(Case::Inessive, pl, &s, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, &s, ["sta", "stä"], true),
            cell(Case::Adessive, pl, &s, ["lla", "llä"], true),
            cell(Case::Ablative, pl, &s, ["lta", "ltä"], true),
            cell(Case::Allative, pl, &s, ["lle", "lle"], true),
            cell(Case::Abessive, pl, &s, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, &s, ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&s);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn haaksi() {
        let w = words("haaksi");
        assert!(w.contains(&"haahden".to_string()));
        assert!(w.contains(&"haahta".to_string()));
        assert!(w.contains(&"haaksien".to_string()));
        assert!(w.contains(&"haaksia".to_string()));
        assert!(!w.contains(&"haahtten".to_string()));
    }
}
