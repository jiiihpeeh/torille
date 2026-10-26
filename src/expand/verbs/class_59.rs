//! Class 59 (the `tuntea` type) inflection, written by hand.
//!
//! Class 59 is a type-1 verb whose past is an `s` past, so it is class 54 and
//! class 58 at once: the stem is the infinitive less its theme vowel (`tunte`),
//! the third person repeats the stem's own `e` (`tuntee`) rather than doubling
//! the infinitive's as class 53 does, and the past is the stem less that `e`
//! (`tunt`) with its final consonant turned to `s` (`tuns`).
//!
//! `Stems::e_s_past_stem_of` is that rule, and `Stems::with_single_past` says
//! the past does not gradate, which is class 54's arrangement: every past cell
//! takes `tuns` alike, `tunsin` and `tunsi` beside each other with no weak grade
//! between them.
//!
//! Everything else is class 53's, reached the ordinary way. The conditional and
//! the second infinitive run on the theme (`tuntisi`, `tunteessa`, `tunteen`)
//! where class 58 had to move both onto the `i`-stem, and the passive runs on
//! the weak grade with an `e` of its own (`tunnetaan`, `tunnettiin`).
//!
//! The class has one member, and it is a gradating one: `tuntea` is `59*J`,
//! `nt` weakened to `nn` (`tunnen`, `tunnettiin`). That gradation is what makes
//! the passive theme `tunne` rather than `tunte` -- the weak grade lands on the
//! `e` the theme would have had to supply, so `retheme` finds nothing left to
//! change. `tuntea` is also the type's model word in the catalogue under the
//! plain `59`, and nothing in the word's shape says which consonant weakens, so
//! `missing_code` names it.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "59";

/// Inflect a class-59 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    let gradation = gradation.or_else(|| missing_code(lemma));
    // Class 54's `s` past, on class 58's vowel: `tunte` -> `tunt` -> `tuns`.
    let past = Stems::e_s_past_stem_of(stem);
    paradigm(&Stems::with_e_s_past(lemma, stem, &past, gradation))
}

/// The type is modelled on `tuntea`, whose own sanalista record is `59*J` (`nt`
/// <-> `nn`, `tunnen`), and which the catalogue names the type after under the
/// plain `59` as well; nothing in the word's shape says which consonant weakens.
/// The class has no other member, so this is the whole table -- the same
/// situation as class 54's `huutaa` and class 57's `saartaa`.
fn missing_code(lemma: &str) -> Option<Gradation> {
    match lemma {
        "tuntea" => Some(Gradation(10)), // J: `nt` <-> `nn`
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    fn has(lemma: &str, form: &str) -> bool {
        words(lemma).iter().any(|w| w == form)
    }

    #[test]
    fn the_stem_keeps_its_e_and_the_third_person_repeats_it() {
        // `tunte` + `e`, as in class 58, not class 53's doubled `aa`.
        for want in [
            "tuntee",
            "tuntevat",
            "tuntisin",
            "tuntisivat",
            "tuntenee",
            "tuntenen",
            "tuntenemme",
            "tuntenette",
            "tuntenevat",
            "tuntekoon",
            "tuntekaa",
            "tuntekaamme",
            "tuntekoot",
            "tuntenut",
            "tunteneet",
            "tunteva",
            "tuntema",
        ] {
            assert!(has("tuntea", want), "tuntea should have {want}");
        }
    }

    #[test]
    fn the_past_drops_the_e_and_turns_the_consonant_to_s() {
        // `tunte` -> `tunt` -> `tuns`, and every past cell takes that alike.
        for want in [
            "tunsin", "tunsit", "tunsi", "tunsimme", "tunsitte", "tunsivat",
        ] {
            assert!(has("tuntea", want), "tuntea should have {want}");
        }
        // Class 58's past stops at `lask`; class 54's `tuntes` keeps a vowel that
        // this type does not have, and neither gives the conditional or the
        // second infinitive an `e` either.
        for absent in ["tunsen", "tuntesen", "tunsisi", "tunsissa"] {
            assert!(!has("tuntea", absent), "tuntea should not have {absent}");
        }
    }

    #[test]
    fn nt_to_nn_gradates_the_present_and_the_passive() {
        // `59*J` = `nt` <-> `nn`, so the weak grade is `tunne` and the strong
        // `tunte`, and the imperative's second person singular takes the weak.
        for want in [
            "tunnen",
            "tunnet",
            "tunnemme",
            "tunnette",
            "tunne",
            "tunnetaan",
            "tunnettiin",
            "tunnettaisiin",
            "tunnettaneen",
            "tunnettakoon",
            "tunnettu",
            "tunnettava",
        ] {
            assert!(has("tuntea", want), "tuntea should have {want}");
        }
        // The participles take the strong grade, where `nt` stands.
        for want in ["tuntenut", "tunteva", "tuntema", "tuntevan", "tuntenutta"] {
            assert!(has("tuntea", want), "tuntea should have {want}");
        }
    }

    #[test]
    fn the_second_infinitive_and_the_conditional_go_to_the_i_stem() {
        // `tuntee` + `ssa` would be `tunteessa` and `tuntisi` would be `tunteisi`,
        // neither of which is a word; both move onto the `i`-stem `tunt`, giving
        // `tuntiessa` and `tuntien`.
        for want in ["tuntiessa", "tuntien", "tuntisi"] {
            assert!(has("tuntea", want), "tuntea should have {want}");
        }
        for absent in ["tunteessa", "tunteen", "tunteisi"] {
            assert!(!has("tuntea", absent), "tuntea should not have {absent}");
        }
    }

    #[test]
    fn the_weak_grade_reaches_the_passive_theme() {
        // The passive theme is the weak grade with an `e` of its own. `tunne`
        // already ends in one, so it is `tunnetaan` and not `tuntetaan` -- the
        // gradation is what supplies the `e` that class 53 has to retheme in.
        assert!(has("tuntea", "tunnetaan"));
        assert!(!has("tuntea", "tuntetaan"));
    }
}
