//! Class 7 (the kivi/sini type) inflection, written by hand as a logical
//! engine.
//!
//! Class 7 is the native `-i` family (`kivi`, `sini`, `koski`). Its stem
//! *changes*: the singular is built on the `e`-stem, the final `i` written `e`
//! (`kivi -> kive`), so the genitive is `kiven`, the partitive `kiveä` and the
//! illative `kiveen` (the doubled `e`). The plural keeps the `i` (`kiven`,
//! `kiviä`, `kivissä`) with the single exception of the nominative, which
//! reverts to the `e`-stem (`kivet`).
//!
//! Because the genitive stem differs from the lemma, the nominative's
//! possessives sit on the `e`-stem too (`kiveni`, not `kivini`), the strong-
//! genitive rule shared by `joki/jokeni`.
//!
//! The graded types `7*B`..`7*L` are this module too, with consonant gradation
//! on top, and gradation there means *four* stems rather than two: a native
//! `-i` word writes its grade in both the `i`-stem and the `e`-stem (`appi`,
//! `api`, `appe`, `ape`). Both pairs are in one `Decl`, which is the case the
//! `Decl` was made for -- the alternative was a `which grade` flag on every
//! cell of a 24-cell grid.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::{Case, Form, Number, PartOfSpeech};

use super::harmony::loanword_harmony;
use super::utility::{Builder, Decl, Ending, cell, e_stem, nominal_tags, plain};

pub const CLASS: &str = "7";

/// The (i-stem, e-stem) pair. Normally the e-stem is the final `i` written `e`
/// (`kivi -> kive`), but a handful of light words insert `j` (`veli -> velje`,
/// plural stem `velji`).
///
/// `veli` is the only base class-7 word that does: of the class's 74 lemmas it
/// is the only one whose genitive ends in `jen` (`olki`, `sylki` and the rest
/// get their `j` from gradation, which is the graded grid's business, not this
/// list's). So the pair is stated rather than derived, with the measurement
/// behind it.
fn stem_pair(lemma: &str) -> (String, String) {
    if lemma == "veli" {
        return ("velji".to_string(), "velje".to_string());
    }
    let istem = lemma.to_string();
    let estem = e_stem(&istem);
    (istem, estem)
}

/// Inflect a class-7 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // Class 7 carries plural-only `-kset` lemmas (`sakset`, `päitset`,
    // `happiviikset`). The citation form is the nominative plural; rebuild
    // the `-ksi` singular (`sakset` -> `saksi`), inflect it, and keep only
    // the plural cells. Without this the engine read `sakset` as a singular
    // and invented `sakseea`.
    if let Some(stem) = lemma.strip_suffix('t') {
        let singular = if let Some(body) = stem.strip_suffix('e') {
            format!("{body}i")
        } else {
            stem.to_string()
        };
        let mut forms = inflect(&singular, possessives);
        forms.retain(|form| {
            form.readings
                .iter()
                .any(|reading| reading.number == Some(Number::Plural))
        });
        return forms;
    }
    let mut b = Builder::new_harmony(lemma, possessives, loanword_harmony(lemma));
    let (istem, estem) = stem_pair(lemma);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // Both illatives double their own stem's last letter, which is not known
    // until the stems are in hand: `kiveen` and `kiviin`.
    let eelast = estem.chars().last().unwrap();
    let ilast = istem.chars().last().unwrap();

    b.decline(
        &Decl::new(vec![
            // ---- the singular, on the e-stem ----
            // Bare nominative is the lemma; its possessives sit on the e-stem
            // (kiveni), so the cell is the one place in the class whose bare is
            // not its own stem.
            cell(Case::Nominative, sg, lemma, ["", ""], false).base_on(estem.clone(), ["", ""]),
            cell(Case::Genitive, sg, &estem, ["n", "n"], false).on_stem(),
            cell(Case::Partitive, sg, &estem, ["a", "ä"], true),
            cell(Case::Essive, sg, &estem, ["na", "nä"], true),
            cell(Case::Translative, sg, &estem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The illative doubles the e: kive -> kiveen.
            cell(
                Case::Illative,
                sg,
                &estem,
                Ending::Given(format!("{eelast}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{eelast}"))),
            cell(Case::Inessive, sg, &estem, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &estem, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &estem, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &estem, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &estem, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &estem, ["tta", "ttä"], true),
            // ---- the plural, on the i-stem (except the nominative) ----
            // Because the i-stem already carries the plural `i`, the usual
            // plural endings' leading `i` is absorbed: kivi + ssä = kivissä,
            // kivi + nä = kivinä, kivi + lle = kiville (not kivi-issä, kivi-inä,
            // kivi-ille). Nominative reverts to the e-stem: kive -> kivet. Its
            // possessives sit on the e-stem too (oveni, not ovini) -- the
            // strong-genitive rule.
            cell(Case::Nominative, pl, &estem, ["t", "t"], false).on_stem(),
            // Genitive plural keeps the i: kivi -> kivien.
            cell(Case::Genitive, pl, &istem, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Partitive, pl, &istem, ["a", "ä"], true),
            cell(Case::Essive, pl, &istem, ["na", "nä"], true),
            cell(Case::Translative, pl, &istem, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The illative doubles the i: kivi -> kiviin.
            cell(
                Case::Illative,
                pl,
                &istem,
                Ending::Given(format!("{ilast}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{ilast}"))),
            cell(Case::Inessive, pl, &istem, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, &istem, ["sta", "stä"], true),
            cell(Case::Adessive, pl, &istem, ["lla", "llä"], true),
            cell(Case::Ablative, pl, &istem, ["lta", "ltä"], true),
            cell(Case::Allative, pl, &istem, ["lle", "lle"], true),
            cell(Case::Abessive, pl, &istem, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, &istem, ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&istem);

    b.into_forms()
}

/// The graded class-7 subtypes (7*B..7*L). A graded class-7 noun is a native
/// `-i` word with consonant gradation, so it has a strong stem (the lemma,
/// `appi`), a weak stem (`grad` applied, `api`), and each written `i` and `e`
/// (i-strong appi, i-weak api, e-strong appe, e-weak ape).
///
/// In the singular the `e`-stems carry the cases (e-strong for partitive/
/// essive/illative, e-weak for genitive/translative/locals, as in class 7);
/// the bare genitive is weak (`apen`) but its possessives keep the strong
/// (`appeni`). In the plural the `i`-stems carry them (i-strong for genitive/
/// partitive/essive/illative with the truncated `-en`/`-a`/`-na`/`-in`,
/// i-weak for nominative/translative/locals/instructive).
pub fn inflect_graded(lemma: &str, possessives: bool, grad: Gradation) -> Vec<Form> {
    let mut b = Builder::new_harmony(lemma, possessives, loanword_harmony(lemma));
    let strong = lemma;
    let weak = grad.apply(strong, Grade::Weak);
    let i_strong = strong.to_string();
    let i_weak = weak.clone();
    let e_strong = e_stem(strong);
    let e_weak = e_stem(&weak);
    let (sg, pl) = (Number::Singular, Number::Plural);
    // The two illatives that double their own stem's last letter: appeen and
    // appiin.
    let ee = e_strong.chars().last().unwrap();
    let ii = i_strong.chars().last().unwrap();

    b.decline(
        &Decl::new(vec![
            // ---- singular: the e-stems ----
            // Bare nominative is the lemma; its possessives and the genitive's
            // sit on the strong e-stem (appeni), the strong-genitive rule --
            // the bare genitive is weak (apen) but the possessive keeps the
            // strong.
            cell(Case::Nominative, sg, strong, ["", ""], false).base_on(e_strong.clone(), ["", ""]),
            cell(Case::Genitive, sg, &e_weak, ["n", "n"], false)
                .base_on(e_strong.clone(), ["", ""]),
            cell(Case::Partitive, sg, &e_strong, ["a", "ä"], true),
            cell(Case::Essive, sg, &e_strong, ["na", "nä"], true),
            cell(Case::Translative, sg, &e_weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            // The illative doubles the e of the strong e-stem: appe -> appeen.
            cell(
                Case::Illative,
                sg,
                &e_strong,
                Ending::Given(format!("{ee}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{ee}"))),
            cell(Case::Inessive, sg, &e_weak, ["ssa", "ssä"], true),
            cell(Case::Elative, sg, &e_weak, ["sta", "stä"], true),
            cell(Case::Adessive, sg, &e_weak, ["lla", "llä"], true),
            cell(Case::Ablative, sg, &e_weak, ["lta", "ltä"], true),
            cell(Case::Allative, sg, &e_weak, ["lle", "lle"], true),
            cell(Case::Abessive, sg, &e_weak, ["tta", "ttä"], true),
            // ---- plural: the i-stems ----
            // Nominative is the weak e-stem with the plural -t: het -> apet.
            cell(Case::Nominative, pl, &e_weak, ["t", "t"], false)
                .base_on(e_strong.clone(), ["", ""]),
            // Strong i-stem for genitive/partitive/essive/illative: appi -> appien.
            cell(Case::Genitive, pl, &i_strong, ["en", "en"], false).on_ending(["e", "e"]),
            cell(Case::Partitive, pl, &i_strong, ["a", "ä"], true),
            cell(Case::Essive, pl, &i_strong, ["na", "nä"], true),
            // Weak i-stem for translative and the locals (the i stays, so the
            // plural endings lose their leading i): api -> apiksi, apissa, apilla.
            cell(Case::Translative, pl, &i_weak, ["ksi", "ksi"], true).on_ending(["kse", "kse"]),
            cell(
                Case::Illative,
                pl,
                &i_strong,
                Ending::Given(format!("{ii}n")),
                false,
            )
            .on_ending(Ending::Given(format!("{ii}"))),
            cell(Case::Inessive, pl, &i_weak, ["ssa", "ssä"], true),
            cell(Case::Elative, pl, &i_weak, ["sta", "stä"], true),
            cell(Case::Adessive, pl, &i_weak, ["lla", "llä"], true),
            cell(Case::Ablative, pl, &i_weak, ["lta", "ltä"], true),
            cell(Case::Allative, pl, &i_weak, ["lle", "lle"], true),
            cell(Case::Abessive, pl, &i_weak, ["tta", "ttä"], true),
            plain(Case::Instructive, pl, &i_weak, ["n", "n"]),
        ]),
        |case, number, poss| nominal_tags(PartOfSpeech::Noun, case, number, poss),
    );
    b.comitative(&i_strong);

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// A plural-only `-kset` lemma (`sakset`) declines from the rebuilt
    /// `-ksi` singular and keeps only the plural cells.
    #[test]
    fn a_plural_only_lemma_declines_from_the_rebuilt_singular() {
        let w = words("sakset");
        for want in ["sakset", "saksien", "saksia", "saksilla", "saksille"] {
            assert!(w.contains(&want.to_string()), "missing {want}");
        }
        for bad in ["sakseea", "sakseen"] {
            assert!(!w.contains(&bad.to_string()), "invented {bad}");
        }
    }

    #[test]
    fn e_stem_singular() {
        let w = words("kivi");
        assert!(w.contains(&"kiven".to_string())); // gen
        assert!(w.contains(&"kiveä".to_string())); // part
        assert!(w.contains(&"kiveen".to_string())); // illat, doubled e
        assert!(w.contains(&"kiveni".to_string())); // nominative possessive on e-stem
    }

    #[test]
    fn i_stem_plural() {
        let w = words("kivi");
        assert!(w.contains(&"kivet".to_string())); // nom pl, e-stem
        assert!(w.contains(&"kivien".to_string())); // gen pl, i-stem
        assert!(w.contains(&"kiviä".to_string())); // part pl
        assert!(w.contains(&"kiviin".to_string())); // illat pl
        assert!(w.contains(&"kivissä".to_string())); // inessive pl
        assert!(w.contains(&"kiviksi".to_string())); // transl pl
    }

    /// `veli` is the one word of the class that inserts a `j` rather than
    /// writing the final `i` as `e` -- `velje`/`velji`, not `vele`/`veli` -- so
    /// the pair is stated rather than derived.
    #[test]
    fn veli_inserts_a_j() {
        let (istem, estem) = stem_pair("veli");
        assert_eq!((istem.as_str(), estem.as_str()), ("velji", "velje"));
        let w = words("veli");
        assert!(w.contains(&"veljen".to_string())); // gen, e-stem
        assert!(w.contains(&"veljet".to_string())); // nom pl, e-stem
        assert!(w.contains(&"veljien".to_string())); // gen pl, i-stem
    }

    #[test]
    fn graded_e_and_i_stems() {
        // appi is 7*B (pp -> p): weak e-stem feeds genitive (apen) and the
        // plural locals; strong i-stem feeds the genitive plural (appien).
        let w = inflect_graded("appi", true, Gradation(2));
        let ws: Vec<&str> = w.iter().map(|f| f.word.as_str()).collect();
        assert!(ws.contains(&"apen")); // genitive, weak e
        assert!(ws.contains(&"appeni")); // genitive possessive, strong e
        assert!(ws.contains(&"appea")); // partitive, strong e
        assert!(ws.contains(&"appeen")); // illative, strong e
        assert!(ws.contains(&"apet")); // nom plural, weak e
        assert!(ws.contains(&"appien")); // gen plural, strong i
        assert!(ws.contains(&"appia")); // part plural, strong i
        assert!(ws.contains(&"apissa")); // inessive plural, weak i
    }

    /// Both crossovers of the graded grid are the strong-genitive rule, and both
    /// are tested because they are the class: the genitive's bare is the weak
    /// `e`-stem (`apen`) while its possessives are the strong (`appeni`), and the
    /// nominative plural does the same (`apet` beside `appeni`).
    #[test]
    fn the_graded_grid_crosses_over_twice() {
        let w: Vec<String> = inflect_graded("appi", true, Gradation(2))
            .into_iter()
            .map(|f| f.word)
            .collect();
        assert!(w.contains(&"apen".to_string())); // gen sg bare: weak e
        assert!(w.contains(&"appeni".to_string())); // gen sg 1sg: strong e
        assert!(w.contains(&"apet".to_string())); // nom pl bare: weak e
    }
}
