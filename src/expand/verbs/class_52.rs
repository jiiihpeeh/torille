//! Class 52 (the `sanoa` type) inflection, written by hand.
//!
//! Class 52 is the plain type-1 verb with a short theme vowel: `sanoa`,
//! `puhua`, `kysyä`. The infinitive drops its final `a`/`ä` for the stem
//! (`sano`, `puhu`, `kysy`), and every other form is a shared ending on that
//! stem.
//!
//! Most of the class does not gradate. Those that do carry the gradation code
//! after the class number in the sanalista -- `52*A` is `haukkoa`, where `kk`
//! gives `k` -- and the code is all that is needed, because type 1 gradates in
//! the same fixed pattern of cells whichever consonant it is (see
//! [`utility::Stems::weak`]).
//!
//! The passive theme is the stem unchanged -- `sanotaan`, `puhutaan`,
//! `kysytään` -- because the theme vowel is not the `a` of the passive
//! `-t-` (see [`class_53`](super::class_53) for the verbs where it is). Where
//! the class gradates, it is the weak grade that the passive is built on:
//! `haukottaa`.
//!
//! The paradigm shape -- the finite endings, the five infinitives, the
//! declension of the participles and their possessives -- is
//! [`utility::paradigm`]; this module is only the stems the type gives.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "52";

/// Inflect a class-52 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let stem = lemma
        .strip_suffix('a')
        .or_else(|| lemma.strip_suffix('ä'))
        .unwrap_or(lemma);
    paradigm(&Stems::new(
        lemma,
        stem,
        gradation.or_else(|| missing_code(lemma)),
    ))
}

/// The gradation a class-52 lemma has but the sanalista does not record.
///
/// `täytyä` is listed as a plain `52`, yet it gradates like any other type-1
/// verb: `täydyn` and `täydytty` beside the third person's `täytyy`. Nothing
/// in the word's shape says which consonant weakens, so the code has to be
/// named; `F` (`t <-> d`) is what `täyty` becomes `täydy` by. This is the
/// same kind of table as `grammar::verb`'s `nähdä`/`tehdä`, for the same
/// reason: the data does not carry the fact.
fn missing_code(lemma: &str) -> Option<Gradation> {
    match lemma {
        "täytyä" => Some(Gradation(6)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Case, Confidence, Number};

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, None).into_iter().map(|f| f.word).collect()
    }

    /// The words of a lemma that gradates, the code given in the sanalista's
    /// own letters: `A` for `52*A`, which is `kk <-> k`.
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
    fn gradated_third_person_keeps_strong_grade() {
        // `52*A` = `haukkoa`: `kk` <-> `k`.
        assert!(has_g("haukkoa", 'A', "haukkoo"));
        assert!(has_g("haukkoa", 'A', "haukkoi"));
        assert!(has_g("haukkoa", 'A', "haukkoisin"));
        assert!(has_g("haukkoa", 'A', "haukkonen"));
        assert!(has_g("haukkoa", 'A', "haukkokaa"));
        assert!(has_g("haukkoa", 'A', "haukkonut"));
        assert!(has_g("haukkoa", 'A', "haukkova"));
    }

    #[test]
    fn gradated_non_third_person_uses_weak_grade() {
        assert!(has_g("haukkoa", 'A', "haukon"));
        assert!(has_g("haukkoa", 'A', "haukot"));
        assert!(has_g("haukkoa", 'A', "haukomme"));
        assert!(has_g("haukkoa", 'A', "haukoimme"));
        assert!(has_g("haukkoa", 'A', "hauko"));
        assert!(has_g("haukkoa", 'A', "haukotaan"));
        assert!(has_g("haukkoa", 'A', "haukottiin"));
    }

    #[test]
    fn gradated_tytyy_has_f_alternation() {
        // The exception forces `täytyä` to gradation `F` (t <-> d).
        assert!(has("täytyä", "täytyi"));
        assert!(has("täytyä", "täytyy"));
        assert!(has("täytyä", "täytyi"));
        assert!(has("täytyä", "täydyn"));
        assert!(has("täytyä", "täydytty"));
    }

    #[test]
    fn finite_active_and_passive() {
        for want in [
            "puhun",
            "puhut",
            "puhuu",
            "puhumme",
            "puhutte",
            "puhuvat",
            "puhuin",
            "puhuit",
            "puhui",
            "puhuimme",
            "puhuitte",
            "puhuivat",
            "puhuisin",
            "puhuisi",
            "puhuisivat",
            "puhunen",
            "puhunee",
            "puhunevat",
            "puhu",
            "puhukoon",
            "puhukaa",
            "puhukaamme",
            "puhukoot",
            "puhutaan",
            "puhuttiin",
            "puhuttaisiin",
            "puhuttaneen",
            "puhuttakoon",
        ] {
            assert!(has("puhua", want), "puhua should have {want}");
        }
    }

    #[test]
    fn participles_decline_with_their_own_possessives() {
        for want in [
            "puhuva",
            "puhuvan",
            "puhuvassa",
            "puhuviin",
            "puhuvia",
            "puhuvassani",
            "puhuvassaan",
            "puhuvassansa",
            "puhuvakseni",
            "puhuvikseni",
            "puhuvakseen",
            "puhuvaksensa",
            "puhunut",
            "puhuneen",
            "puhunutta",
            "puhuneita",
            "puhuneeni",
            "puhuneideni",
            "puhuneittemme",
            "puhuma",
            "puhumaan",
            "puhumaani",
            "puhumieni",
            "puhumaton",
            "puhumattoman",
            "puhumatonta",
            "puhumattomat",
            "puhumattomani",
            "puhumattomiemme",
            "puhumattomaksi",
            "puhuttava",
            "puhuttavan",
            "puhuttavaa",
            "puhuttu",
            "puhutun",
            "puhuttua",
            "puhuttuna",
            "puhuttuun",
            "puhuttuani",
            "puhuttuni",
            "puhuttujemme",
            "puhutussansa",
            "puhuminen",
            "puhumisen",
            "puhumista",
            "puhumiseni",
            "puhumaisillaan",
            "puhumaisillani",
        ] {
            assert!(has("puhua", want), "puhua should have {want}");
        }
    }

    #[test]
    fn infinitives() {
        for want in [
            "puhua",
            "puhuessa",
            "puhuessani",
            "puhuttaessa",
            "puhuen",
            "puhumaan",
            "puhumassa",
            "puhumasta",
            "puhumalla",
            "puhumatta",
            "puhuman",
            "puhuttaman",
            "puhuttaen",
        ] {
            assert!(has("puhua", want), "puhua should have {want}");
        }
    }

    #[test]
    fn compounds() {
        for want in [
            "on puhunut",
            "oli puhunut",
            "olivat puhuneet",
            "olisi puhunut",
            "lienevät puhuneet",
            "olkoon puhunut",
            "olkoot puhuneet",
            "lie puhunut",
            "en ole puhunut",
            "emme ole puhuneet",
            "en ollut puhunut",
            "eivät olleet puhuneet",
            "on puhuttu",
            "olisi puhuttu",
            "ei ollut puhuttu",
            "ei lie puhuttu",
            "älköön olko puhuttu",
            "en puhu",
            "et puhu",
            "ei puhu",
            "emme puhu",
            "eivät puhu",
            "en puhunut",
            "eivät puhuneet",
            "ei puhuttu",
            "ei puhuta",
            "ei puhuttaisi",
            "ei puhuttane",
            "älköön puhuttako",
        ] {
            assert!(has("puhua", want), "puhua should have {want}");
        }
    }

    #[test]
    fn the_old_wrong_spellings_are_gone() {
        // The doubled theme vowel in the 3rd person was a slip (`sanooo`), and
        // the nominative possessive of `-nut` is on the genitive stem.
        assert!(!has("sanoa", "sanooo"));
        assert!(!has("puhua", "puhunutmme"));
    }

    /// The form `word` of `puhua`, with its confidence and its readings.
    fn form(word: &str) -> Form {
        inflect("puhua", None)
            .into_iter()
            .find(|f| f.word == word)
            .unwrap_or_else(|| panic!("puhua should have {word}"))
    }

    /// The negated conditional, potential and imperative are now reference
    /// rows (`build_verb_lexicon.py` puts them in), so they are plain.
    #[test]
    fn the_negative_moods_are_plain() {
        for want in [
            "en puhuisi",
            "et puhuisi",
            "ei puhuisi",
            "emme puhuisi",
            "en puhune",
            "eivät puhune",
            "älä puhu",
            "älköön puhuko",
            "älkäämme puhuko",
            "älkää puhuko",
            "älkööt puhuko",
        ] {
            assert_eq!(form(want).confidence, Confidence::Plain, "{want}");
        }
    }

    /// The comitative is plural-only and reference-confirmed, so it is plain
    /// too, and the `-ttu` participle takes it on the strong stem
    /// (`puhuttuine`), not the weak plural oblique one (`puhutuine`).
    #[test]
    fn the_comitative_is_plain() {
        for want in [
            "puhuvine",
            "puhuvineen",
            "puhuvineni",
            "puhuvinesi",
            "puhuvinemme",
            "puhuvinenne",
            "puhuvinensa",
            "puhumisineen",
            "puhumattomineen",
            "puhuttuine",
            "puhuttuineen",
        ] {
            assert_eq!(form(want).confidence, Confidence::Plain, "{want}");
        }
        // The local plural stem's weak `t` is not the comitative's.
        assert!(!has("puhua", "puhutuine"));
    }

    /// The accusative is syncretic -- its singular is the genitive and its
    /// plural the nominative -- so the paradigm carries it as a second reading
    /// of those two words. The engine's `dedup` merges the two readings later,
    /// and the agreed reading then drops the case, since they disagree about
    /// it; here in the raw module they are two forms.
    #[test]
    fn the_accusative_is_a_second_reading() {
        for (word, number) in [("puhuvan", Number::Singular), ("puhuvat", Number::Plural)] {
            assert!(
                inflect("puhua", None).iter().any(|f| {
                    f.word == word
                        && f.readings
                            .iter()
                            .any(|t| t.case == Some(Case::Accusative) && t.number == Some(number))
                }),
                "{word} should have an accusative reading"
            );
        }
    }
}
