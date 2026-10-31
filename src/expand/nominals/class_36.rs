//! Class 36 (the lähin type) inflection, written by hand as a logical engine.
//!
//! Class 36 is the `-in` superlative family (lähin, sisin, vanhin) and, with a
//! different partitive, class 37's `vasen`. From the lemma five stems grow:
//! the strong `lähimpä`, its consonant `lähimp`, the weak `lähimmä`, the
//! weak-`i` `lähimmi`, and the lemma itself. The genitive/locals sit on the
//! weak, the essive/illative on the strong, the partitive on the lemma (or the
//! strong for `vasen`); the plural glides the translative/locals onto the
//! weak-`i` while the genitive/partitive/essive/illative stay on the strong's
//! consonant, and the `-ten` genitive plural sits on the lemma.
//!
//! [`superl_inflect`] is the shared engine and class 37 is its other user.
//!
//! The `-in` genitive plural is a rare bare form. The translative's
//! possessives stay on the weak stem (`vanhimmakseen`); the weak-`i` spelling
//! (`vanhimmikseen`) is not part of the paradigm.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, Ending, cell, nominal_tags, plain, rare};

pub const CLASS: &str = "36";

/// The superlative stems: strong, weak, weak-`i` from `lähin`.
fn stems(lemma: &str) -> (String, String) {
    let front = super::utility::front(lemma);
    let fv = if front { 'ä' } else { 'a' };
    let base = &lemma[..lemma.len() - 1]; // drop the final n
    let strong = format!("{base}mp{fv}"); // lähimpä
    let weak = strong.replace("mp", "mm"); // lähimmä
    (strong, weak)
}

/// The superlative engine. `part_on_strong` puts the partitive on the strong
/// stem (vasempaa) instead of the lemma (lähintä).
pub fn superl_inflect(lemma: &str, possessives: bool, part_on_strong: bool) -> Vec<Form> {
    // The caller's `possessives` is the decision here, not a local guess: it is
    // made from the part of speech in the dictionary, and only the dictionary
    // knows that `marto` is an adjective that can still be a noun
    // (`adjektiivi, substantiivi`) while `mieto` is not. A blanket `true` in
    // this class would override exactly the distinction the caller worked to
    // make, so the argument is threaded through to the builder.
    let mut b = Builder::new(lemma, possessives);
    let (strong, weak) = stems(lemma);
    let c = strong[..strong.len() - strong.chars().last().unwrap().len_utf8()].to_string(); // lähimp
    // The weak-i stem: lähimmä -> lähimmi.
    let wi = format!(
        "{}{}",
        &weak[..weak.len() - weak.chars().last().unwrap().len_utf8()],
        'i'
    );
    let e = strong.chars().last().unwrap();
    let (sg, pl) = (Number::Singular, Number::Plural);

    // The partitive is the one place `vasen` differs: it puts `vasempaa` on
    // the strong stem before the lemma's `vasenta`.
    let part_sg = {
        let mut parts = Vec::new();
        if part_on_strong {
            parts.push(cell(Case::Partitive, sg, &strong, ["a", "ä"], true));
        }
        parts.push(cell(Case::Partitive, sg, lemma, ["ta", "tä"], true));
        parts
    };

    let mut entries = vec![
        // ---- the singular ----
        // The nominative's possessives sit on the strong stem: lähimpäni.
        cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(strong.clone(), ["", ""]),
    ];
    entries.extend(part_sg);
    entries.extend(vec![
        // The bare genitive is weak (`vanhimman`) but its possessives sit on
        // the strong stem (`vanhimpansa`), the strong-genitive rule. Leaving
        // them on the weak stem invented `vanhimmansa`, which is not a word.
        cell(Case::Genitive, sg, &weak, ["n", "n"], false).base_on(strong.clone(), ["", ""]),
        cell(Case::Essive, sg, &strong, ["na", "nä"], true),
        // The translative and its possessives are spelled on the weak stem:
        // lähimmäksi / lähimmäkseen.
        cell(Case::Translative, sg, &weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        // The illative doubles the strong stem's last letter: lähimpään.
        cell(
            Case::Illative,
            sg,
            &strong,
            Ending::Given(format!("{e}n")),
            false,
        )
        .on_ending(Ending::Given(format!("{e}"))),
        cell(Case::Inessive, sg, &weak, ["ssa", "ssä"], true),
        cell(Case::Elative, sg, &weak, ["sta", "stä"], true),
        cell(Case::Adessive, sg, &weak, ["lla", "llä"], true),
        cell(Case::Ablative, sg, &weak, ["lta", "ltä"], true),
        cell(Case::Allative, sg, &weak, ["lle", "lle"], true),
        cell(Case::Abessive, sg, &weak, ["tta", "ttä"], true),
        // ---- the plural ----
        // The plural nominative is weak (`sisimmät`), but its possessives use
        // the strong stem (`sisimpänsä`), not `sisimmänsä`.
        cell(Case::Nominative, pl, &weak, ["t", "t"], false).base_on(strong.clone(), ["", ""]),
        cell(Case::Genitive, pl, &c, ["ien", "ien"], false).on_ending(["ie", "ie"]),
        cell(Case::Genitive, pl, lemma, ["ten", "ten"], false).on_ending(["te", "te"]),
        // A rare bare `-in` genitive plural, with no possessive column.
        rare(Case::Genitive, pl, &strong, ["in", "in"]),
        cell(Case::Partitive, pl, &c, ["ia", "iä"], true),
        cell(Case::Essive, pl, &c, ["ina", "inä"], true),
        cell(Case::Illative, pl, &c, ["iin", "iin"], false).on_ending(["ii", "ii"]),
        // The plural translative and locals glide onto the weak-i stem.
        cell(Case::Translative, pl, &wi, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
        cell(Case::Inessive, pl, &wi, ["ssa", "ssä"], true),
        cell(Case::Elative, pl, &wi, ["sta", "stä"], true),
        cell(Case::Adessive, pl, &wi, ["lla", "llä"], true),
        cell(Case::Ablative, pl, &wi, ["lta", "ltä"], true),
        cell(Case::Allative, pl, &wi, ["lle", "lle"], true),
        cell(Case::Abessive, pl, &wi, ["tta", "ttä"], true),
        plain(Case::Instructive, pl, &wi, ["n", "n"]),
    ]);

    b.decline(&Decl::new(entries), |case, number, poss| {
        nominal_tags(PartOfSpeech::Noun, case, number, poss)
    });
    b.comitative(&wi);

    b.into_forms()
}

/// Inflect a class-36 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    superl_inflect(lemma, possessives, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    #[test]
    fn superlative() {
        let w = words("lähin");
        assert!(w.contains(&"lähimmän".to_string()));
        assert!(w.contains(&"lähintä".to_string()));
        assert!(w.contains(&"lähimpänä".to_string()));
        assert!(w.contains(&"lähimpään".to_string()));
        assert!(w.contains(&"lähimpien".to_string()));
        assert!(w.contains(&"lähimpiä".to_string()));
        assert!(w.contains(&"lähimmiksi".to_string()));
        assert!(w.contains(&"lähimmissä".to_string()));
    }

    /// The genitive's possessives use the strong stem even though the bare
    /// genitive is weak: `vanhimpansa`, not `vanhimmansa`.
    #[test]
    fn the_genitive_possessives_use_the_strong_stem() {
        let w = words("vanhin");
        assert!(w.contains(&"vanhimpansa".to_string()));
        assert!(!w.contains(&"vanhimmansa".to_string()));
    }

    /// The plural nominative is weak but its possessives are strong:
    /// `sisimpänsä`, not `sisimmänsä`.
    #[test]
    fn the_plural_nominative_possessives_use_the_strong_stem() {
        let w = words("sisin");
        assert!(w.contains(&"sisimpänsä".to_string()));
        assert!(!w.contains(&"sisimmänsä".to_string()));
    }

    /// The singular translative possessive stays on the weak stem, and the
    /// `-in` genitive plural is a rare bare form. The plural keeps the
    /// weak-`i` stem (`lähimmikseen`).
    #[test]
    fn the_singular_translative_possessive_and_the_rare_in_genitive() {
        let singular: Vec<String> = inflect("lähin", true)
            .into_iter()
            .filter(|f| {
                f.readings.iter().any(|r| {
                    r.case == Some(Case::Translative) && r.number == Some(Number::Singular)
                })
            })
            .map(|f| f.word)
            .collect();
        assert!(singular.contains(&"lähimmäkseen".to_string()));
        assert!(!singular.contains(&"lähimmikseen".to_string()));
        let forms = inflect("lähin", true);
        let rare: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == crate::tags::Confidence::Repaired)
            .map(|f| f.word.as_str())
            .collect();
        assert!(
            rare.iter().any(|x| x.ends_with("in")),
            "rare -in missing: {rare:?}"
        );
    }

    /// `part_on_strong` is what class 37 (`vasen`) sets: the strong partitive
    /// `vasempaa` is offered beside the lemma's `vasenta`.
    #[test]
    fn vasen_takes_both_partitives() {
        let w: Vec<String> = superl_inflect("vasen", true, true)
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(w.contains(&"vasempaa".to_string()));
        assert!(w.contains(&"vasenta".to_string()));
    }
}
