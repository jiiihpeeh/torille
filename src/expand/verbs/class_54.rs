//! Class 54 (the `huutaa` type) inflection, written by hand.
//!
//! Class 54 is type 1 with a long theme vowel, like class 53, and it shares
//! that class's present: the stem is the infinitive without its final `a`/`ä`
//! (`huuta`, `lypsä`, `rakenta`), the third person doubles the theme vowel
//! (`huutaa`, `rakentaa`) and the passive rethemes the weak grade to `e`
//! (`huudetaan`, `rakennetaan`). What separates it is the *past*.
//!
//! Where class 53's past runs on the present stem less its theme vowel
//! (`aakkosta` -> `aakkosti`), class 54's runs on a stem of its own: the
//! present stem less its theme vowel, with that final consonant replaced by
//! `s`. `huuta` gives `huus` (`huusi`, `huusin`), `rakenta` gives `rakens`
//! (`rakensi`), `askelta` gives `askels` (`askelsi`), and a stem that already
//! ends in `s` keeps it (`lypsää` -> `lypsi`). That stem is the same in every
//! past cell -- the class's gradation shows in the present and the passive
//! only -- so it is passed as the past *and* its own weak grade
//! ([`Stems::with_single_past`]).
//!
//! Gradation, where the class names it, is otherwise class 52/53's: `54*F`
//! (`huutaa`) weakens `t` to `d` (`huudan`, `huudetaan`), `54*J` (`rakentaa`)
//! `nt` to `nn` (`rakennan`), `54*I` (`askeltaa`) `lt` to `ll` (`askellan`)
//! and `54*K` (`kiertää`) `rt` to `rr` (`kierrän`). The code is the whole of
//! what says which consonant it is, and it sits before the theme vowel that
//! class 52's does not have.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "54";

/// Inflect a class-54 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    // The theme vowel is the stem's last character and the past drops it --
    // `huutaa` runs on `huuta` in the present and `huus` in the past. The
    // consonant before it becomes `s`, which `s_past_stem_of` does.
    let past = Stems::s_past_stem_of(stem);
    paradigm(&Stems::with_single_past(
        lemma,
        stem,
        &past,
        gradation.or_else(|| missing_code(lemma)),
    ))
}

/// The gradation a class-54 lemma has but its class string does not record.
///
/// The type is modelled on `huutaa`, which gradates `t` to `d` (`huudan`,
/// `huudettiin`) and whose own sanalista record is `54*F`; the catalogue names
/// the type after it under the plain `54` as well, and nothing in the word's
/// shape says which consonant weakens. The plain class's real members
/// (`lypsää`, `pieksää`) do not gradate, so only the model word is named here.
/// This is the same table as class 52's `täytyä`.
fn missing_code(lemma: &str) -> Option<Gradation> {
    match lemma {
        "huutaa" => Some(Gradation(6)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    /// The words of a lemma that gradates, the code given in the sanalista's
    /// own letters: `F` for `54*F`, which is `t <-> d`.
    fn graded(lemma: &str, code: char) -> Vec<String> {
        inflect(lemma, Gradation::from_letter(code))
            .into_iter()
            .map(|f| f.word)
            .collect()
    }

    fn has(lemma: &str, form: &str) -> bool {
        words(lemma).iter().any(|w| w == form)
    }

    fn has_g(lemma: &str, code: char, form: &str) -> bool {
        graded(lemma, code).iter().any(|w| w == form)
    }

    #[test]
    fn the_present_doubles_the_theme_vowel() {
        for want in [
            "huutaa",
            "huutavat",
            "huutaisin",
            "huutanen",
            "huutakoon",
            "huutanut",
            "huutava",
        ] {
            assert!(has_g("huutaa", 'F', want), "huutaa should have {want}");
        }
    }

    #[test]
    fn the_past_runs_on_its_own_s_stem() {
        // `huuta` -> `huus`: every past cell, strong or weak, is the one stem.
        for want in [
            "huusin", "huusit", "huusi", "huusimme", "huusitte", "huusivat",
        ] {
            assert!(has_g("huutaa", 'F', want), "huutaa should have {want}");
        }
        // And the weak grade still shows in the present and the passive.
        for want in ["huudan", "huudat", "huudamme", "huudetaan", "huudettiin"] {
            assert!(has_g("huutaa", 'F', want), "huutaa should have {want}");
        }
        assert!(!has_g("huutaa", 'F', "huudin"));
    }

    #[test]
    fn nt_gradation_runs_through_the_present() {
        // `54*J` = `rakentaa`: `nt` <-> `nn`, and the past is `rakens`.
        for want in [
            "rakentaa",
            "rakentavat",
            "rakennan",
            "rakennat",
            "rakenna",
            "rakennetaan",
            "rakennettiin",
            "rakensin",
            "rakensivat",
            "rakentanut",
            "rakentava",
        ] {
            assert!(has_g("rakentaa", 'J', want), "rakentaa should have {want}");
        }
        assert!(!has_g("rakentaa", 'J', "rakennut"));
    }

    #[test]
    fn lt_and_rt_gradation() {
        // `54*I` = `askeltaa`: `lt` <-> `ll`, past `askels`.
        for want in [
            "askeltaa",
            "askellan",
            "askella",
            "askellamme",
            "askelletaan",
        ] {
            assert!(has_g("askeltaa", 'I', want), "askeltaa should have {want}");
        }
        assert!(has_g("askeltaa", 'I', "askelsin"));
        // `54*K` = `kiertää`: `rt` <-> `rr`, past `kiers`.
        for want in ["kiertää", "kierrän", "kierrä", "kierretään"] {
            assert!(has_g("kiertää", 'K', want), "kiertää should have {want}");
        }
        assert!(has_g("kiertää", 'K', "kiersin"));
    }

    #[test]
    fn the_plain_class_keeps_its_stem() {
        // `54` with no code gradates nothing: `lypsää` has `lypsän` and
        // `lypsi`, on the one stem less the theme vowel, which is already `s`.
        for want in [
            "lypsää",
            "lypsän",
            "lypsävät",
            "lypsi",
            "lypsin",
            "lypsivät",
            "lypsetään",
            "lypsettiin",
            "lypsänyt",
            "lypsävä",
        ] {
            assert!(has("lypsää", want), "lypsää should have {want}");
        }
    }
}
