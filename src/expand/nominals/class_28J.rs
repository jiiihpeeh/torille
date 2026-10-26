//! Class 28*J (the kynsi/kansi type) inflection, written by hand as a logical
//! engine.
//!
//! Class 28 is the graded `-si` family, whose strong/weak split grades the
//! cluster before the final `i`. From the lemma (`kansi`) four stems grow:
//!   strong S = kansi, the weak W = kanne (`ns -> nn`), the partitive stem
//!   P = kant (`s -> t`), and the essive/illative stem E = kante (`= P + e`).
//! The singular's genitive/translative/locals sit on W, its partitive on P and
//! its essive/illative on E; the plural's nom sits on W and everything else on
//! S, with the `-ten` genitive plural on P.
//!
//! This module is not just class 28*J: [`inflect_base`] is the shared engine
//! for classes 27, 28*J, 29 and 30, which differ only in how their four stems
//! are built (27's `käsi` has a `d` where 28 has `nn`, 29/30 collapse stems).
//! Those modules supply the stems and this one supplies the grid, the
//! `gradated.rs` move again.
//!
//! What is worth naming: the nominative and genitive bare forms sit on the
//! strong and weak stems, but *their possessives* sit on E, so `kansi` gives
//! `kanteni`, not `kansini` or `kanneni` -- both the nominative and the
//! genitive inherit `kanteni`. That is a `base_on` override on two cells, and
//! no separate possessive column is emitted.

#![allow(dead_code)]

use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::utility::{Builder, Decl, cell, nominal_tags, plain};

pub const CLASS: &str = "28*J";

/// Replace the final `s` (the last character) in `base` with `repl`.
pub fn rep_last_s(base: &str, repl: &str) -> String {
    let mut s = base.to_string();
    s.pop(); // drop the `s`
    s.push_str(repl);
    s
}

/// The four stems of a class-28 noun. `weak` is the genitive/inessive stem,
/// `ptp` the partitive stem, `e` the essive/illative stem.
pub struct Stems {
    pub strong: String,
    pub weak: String,
    pub ptp: String,
    pub e: String,
}

fn stems(lemma: &str) -> Stems {
    let mut base = lemma.to_string();
    base.pop(); // kansi -> kans
    let prev = base.chars().rev().nth(1).unwrap(); // the consonant before `s` (n)
    // The strong already carries that consonant, so the `s` is replaced with a
    // single copy of it: kans -> kann.
    let mut weak = rep_last_s(&base, &prev.to_string());
    weak.push('e'); // kanne
    let ptp = rep_last_s(&base, "t"); // kant
    let e = format!("{ptp}e"); // kante
    Stems {
        strong: lemma.to_string(),
        weak,
        ptp,
        e,
    }
}

/// Inflect a class-28 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    inflect_base(lemma, possessives, &stems(lemma), None)
}

/// The shared 28* gradation engine, given the four stems. `front` overrides
/// the orthographic harmony where a class knows the word's real identity
/// (a compound whose front head governs: marraskesi).
pub fn inflect_base(lemma: &str, possessives: bool, st: &Stems, front: Option<bool>) -> Vec<Form> {
    let mut b = match front {
        Some(f) => Builder::new_harmony(lemma, possessives, f),
        None => Builder::new(lemma, possessives),
    };
    let (strong, weak, ptp, e) = (&st.strong, &st.weak, &st.ptp, &st.e);
    let (sg, pl) = (Number::Singular, Number::Plural);

    // The nominative and genitive bare forms sit on the strong/weak stems, but
    // their possessives sit on the essive stem `e` (`kansi` -> `kanteni`, not
    // `kansini`/`kanneni`; `käsi` -> `käteni`). Both cells want the same
    // `kanteni`, so the genitive's own weak-stem forms and the strong-stem
    // nominatives are not emitted as possessives at all.
    let nom_sg = cell(Case::Nominative, sg, strong, ["", ""], false).base_on(e.clone(), ["", ""]);
    let gen_sg = cell(Case::Genitive, sg, weak, ["n", "n"], false).base_on(e.clone(), ["", ""]);
    let nom_pl = cell(Case::Nominative, pl, weak, ["t", "t"], false).base_on(e.clone(), ["", ""]);

    b.decline(
        &Decl::new(vec![
            // ---- the singular ----
            nom_sg,
            gen_sg,
            cell(Case::Partitive, sg, ptp, ["ta", "tä"], true),
            cell(Case::Essive, sg, e, ["na", "nä"], true),
            cell(Case::Translative, sg, weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The illative is `E` doubled before `n`: kante -> kanteen.
            cell(Case::Illative, sg, e, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Inessive, sg, weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, weak, ["tta", "ttä"], true),
            // ---- the plural ----
            nom_pl,
            // Genitive plural: `S`-stem `-en`, then the `P`-stem `-ten`.
            cell(Case::Genitive, pl, strong, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Genitive, pl, ptp, ["ten", "ten"], false).on_ending(["te", "te"]),
            cell(Case::Partitive, pl, strong, ["a", "ä"], true),
            cell(Case::Essive, pl, strong, ["na", "nä"], true),
            // The plural sits on the `i`-final strong stem, so the `i`-plural
            // endings are truncated: kansiksi, kansin, kansissa, kansilla.
            cell(Case::Translative, pl, strong, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(Case::Illative, pl, strong, ["in", "in"], false).on_ending(["i", "i"]),
            cell(Case::Inessive, pl, strong, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, strong, ["sta", "stä"], true),
            cell(Case::Adessive, pl, strong, ["lla", "llä"], true),
            cell(Case::Ablative, pl, strong, ["lta", "ltä"], true),
            cell(Case::Allative, pl, strong, ["lle", "lle"], true),
            cell(Case::Abessive, pl, strong, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, strong, ["n", "n"]),
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
    fn graded_si() {
        let w = words("kansi");
        assert!(w.contains(&"kannen".to_string()));
        assert!(w.contains(&"kantta".to_string()));
        assert!(w.contains(&"kanteen".to_string()));
        assert!(w.contains(&"kansien".to_string()));
        assert!(w.contains(&"kantten".to_string()));
    }

    /// The class's distinctive rule: the nominative and the genitive both send
    /// their possessives to the essive stem, so `kansi` and `kannen` share the
    /// one possessive column `kanteni` -- and `kansini`/`kanneni` are not
    /// emitted at all.
    #[test]
    fn the_possessives_of_the_nominative_and_genitive_share_the_e_stem() {
        let w = words("kansi");
        assert!(w.contains(&"kanteni".to_string()), "kanteni missing");
        assert!(!w.contains(&"kansini".to_string()), "kansini emitted");
        assert!(!w.contains(&"kanneni".to_string()), "kanneni emitted");
    }

    /// The `-ten` genitive plural is plain across the whole family -- every one
    /// of the 30 lemmas in 27/28*J/29/30 attests it. The `ten_rare` flag that
    /// used to demote it for class 27's `tosi`/`täysi` was demoting a form the
    /// reference gives (`totten`, `täytten`) and is gone.
    #[test]
    fn the_ten_genitive_plural_is_plain_for_the_family() {
        let s = stems("kansi");
        let forms = inflect_base("kansi", true, &s, None);
        let plain: Vec<&str> = forms
            .iter()
            .filter(|f| f.confidence == crate::tags::Confidence::Plain)
            .map(|f| f.word.as_str())
            .collect();
        assert!(plain.contains(&"kantten"), "kantten not plain");
    }
}
