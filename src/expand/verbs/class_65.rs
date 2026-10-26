//! Class 65 (the `käydä` type) inflection, written by hand.
//!
//! Class 65 is the `-ydä` type, and it is class 64's `juoda` with the connecting
//! vowel gradating rather than simply standing: `käydä` is `kä` + `y` + `dä`
//! where `juoda` is `jo` + `u` + `o` + `da`, and the past reads that `y` as `v`
//! -- `kävin`, `kävi`, `kävivät` beside the present's `käyn`, `käy`, `käyvät`.
//! So the present stem is the lemma less its `dä` (`käy`) and the past stem is
//! that with the stem-final `y` read as `v` (`käv`).
//!
//! The alternation is the class's own, and it has to be: every
//! [`Gradation`](crate::kotus::Gradation) code is a pair of *consonants* --
//! `kk <-> k`, `nt <-> nn`, `k <-> v` and so on -- and this is a vowel giving
//! way to a consonant, which no code in [`Gradation::pair`] describes. The
//! sanalista also writes class 65 plain, with no code after it, so there is
//! nothing to read the alternation off and no `missing_code` table. It is
//! written out here instead, which is why the class needs its own
//! `past_stem_of` even though the result is the same shape as class 64's.
//!
//! Everything else about the type is class 63's and class 64's cell for cell,
//! and the reference agrees:
//!
//! * **The passive deranks its `t` to `d`,** taking the `d` in the present and
//!   writing a single `t` everywhere else: `käydään` and `käytiin`,
//!   `käytävä` and `käyty` with its oblique `käydyn`. That is
//!   [`PassiveCons::Deranked`], and it brings the past passive participle's
//!   oblique deranking with it as it does for classes 62, 63 and 64.
//!
//! * **The passive theme is the present stem as it stands.** Class 53 reads a
//!   stem-final `a`/`ä` as the passive's `e` (`muistetaan` beside `sanoa`'s
//!   `sanotaan`), and that is a rule this type's `y` is not touched by, so
//!   `p_theme` is left as `with_past` set it -- the same position class 63 has
//!   to override explicitly and class 64 reaches by accident.
//!
//! * **The third person present is the bare stem,** `käy`, so `pres3sg` is
//!   empty as it is for classes 62, 63 and 64: the stem ends in a vowel and
//!   there is no theme left to write again. Note that this is the *strong*
//!   grade -- the third person is one of the cells a gradating type-1 class
//!   gradates in the *weak* grade, so a class that gradates would say
//!   otherwise, and this one does not.
//!
//! * **The second infinitive runs on the `d`:** `käydessä` and `käyden` are
//!   `käyd` + `essä` and `käyd` + `en`, where class 53 has `muisteessa` and
//!   `muisteen`. So `inf2_stem` is the stem plus `de`, as in classes 62, 63
//!   and 64. The `d` is the ending's own, and it is the same `d` the passive
//!   shows in `käydään`, which is a coincidence of the two rather than a
//!   shared fact -- the second infinitive is active, and it takes the `d`
//!   whichever way the passive writes its consonant.
//!
//! * **The conditional follows the past stem,** so `kävisi` is `käv` + `isi`
//!   where class 53's `muistaisi` is `muist` + `isi` off the present stem, and
//!   the same `kävisi` the type's own past stem predicts. Hence `cond_stem` is
//!   the past stem, as in classes 63 and 64. The potential and the imperative
//!   keep the present stem (`käynee`, `käyköön`), so those two do not part
//!   company, as they do in classes 58 and 59.
//!
//! The class has one member, `käydä`, and it is plain `65`. Every rule above is
//! therefore read off the reference for a single word, which is worth saying
//! plainly: the class is a type of its own because the sanalista lists it, not
//! because the data shows a second member of it.

#![allow(dead_code)]

use crate::tags::Form;

use super::utility::{PassiveCons, Stems, paradigm};

pub const CLASS: &str = "65";

/// The past stem of a class-65 verb: the present stem with its final `y` read
/// as `v`, so `käy` gives `käv`.
///
/// The alternation is vowel-to-consonant, which no
/// [`Gradation`](crate::kotus::Gradation) code carries -- they are all pairs of
/// consonants -- so it is written out here rather than threaded through a code.
fn past_stem_of(stem: &str) -> String {
    let mut chars: Vec<char> = stem.chars().collect();
    if chars.last() == Some(&'y') {
        chars.pop();
        chars.push('v');
    }
    chars.into_iter().collect()
}

/// Inflect a class-65 verb into its whole paradigm.
pub fn inflect(lemma: &str) -> Vec<Form> {
    let stem = lemma
        .strip_suffix("da")
        .or_else(|| lemma.strip_suffix("dä"))
        .unwrap_or(lemma);
    let past = past_stem_of(stem);
    let mut stems = Stems::with_past(lemma, stem, &past, None);
    // The passive writes one consonant each way rather than geminating, which is
    // the rule for classes 62, 63 and 64 as well.
    stems.p_cons = PassiveCons::Deranked;
    // The theme is already a vowel, so there is nothing to repeat in the third
    // person.
    stems.pres3sg = String::new();
    // `p_theme` is left as `with_past` set it: `retheme` fires on a stem-final
    // `a`/`ä`, and this type's stem ends in `y`, so the weak grade is the
    // passive theme as it stands.
    // The conditional follows the past stem; the second infinitive the `d`.
    stems.cond_stem = past;
    stems.inf2_stem = format!("{stem}de");
    paradigm(&stems)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(lemma: &str, form: &str) -> bool {
        inflect(lemma).iter().any(|f| f.word == form)
    }

    fn hasnt(lemma: &str, form: &str) -> bool {
        !has(lemma, form)
    }

    /// The present stem is the lemma less its `dä` and the past stem reads the
    /// stem-final `y` as `v`, which is the whole difference between the tenses.
    #[test]
    fn the_past_stem_reads_the_final_y_as_a_v() {
        assert_eq!(past_stem_of("käy"), "käv");
        // A stem with no final `y` is its own past stem, so the rule does not
        // have to know the type to be safe on one.
        assert_eq!(past_stem_of("kä"), "kä");
        assert_eq!(past_stem_of("juo"), "juo");
        for want in [
            "käyn",
            "käyt",
            "käy",
            "käymme",
            "käytte",
            "käyvät",
            "kävin",
            "kävit",
            "kävi",
            "kävimme",
            "kävitte",
            "kävivät",
        ] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
        // The present's `y` is the strong grade, so the participle and the
        // imperative keep it too; only the past and the conditional lose it.
        for want in [
            "käyvä",
            "käy",
            "käykää",
            "käyköön",
            "käynyt",
            "käydään",
            "käytiin",
        ] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
    }

    /// The conditional follows the past stem; the potential and the imperative
    /// keep the present one.
    #[test]
    fn the_conditional_follows_the_past_stem_and_the_rest_the_present() {
        for want in [
            "kävisin",
            "kävisit",
            "kävisi",
            "kävisimme",
            "kävisitte",
            "kävisivät",
        ] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
        for want in [
            "käynen",
            "käynet",
            "käynee",
            "käynemme",
            "käynette",
            "käynevät",
        ] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
        // `käyisi` would be the present stem's own conditional, and it is not a
        // word; nor is the potential off the past stem.
        assert!(hasnt("käydä", "käyisi"));
        assert!(hasnt("käydä", "kävisinne"));
        for want in ["käy", "käykää", "käykäämme", "käyköön", "käykööt"] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
    }

    /// The passive takes the `d` in the present and a single `t` everywhere
    /// else, and its theme is the present stem's `y` as it stands -- `käetään`
    /// is not a word, and neither is a `v` from the past stem.
    #[test]
    fn the_passive_deranks_its_t_and_keeps_the_present_y() {
        for want in [
            "käydään",
            "käytiin",
            "käytäisiin",
            "käytäneen",
            "käytäköön",
            "käytävä",
            "käyty",
            "käytäessä",
            "käytäen",
            "käytämän",
        ] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
        for absent in [
            "käettään",
            "kävdään",
            "käyttiin",
            "käyttävä",
            "käytty",
            "käyttäessä",
        ] {
            assert!(hasnt("käydä", absent), "käydä should not have {absent}");
        }
    }

    /// `käyty` is the nominative and `käydy` the oblique stem, so the participle
    /// deranks its `t` to a `d` in the oblique exactly as classes 62, 63 and 64
    /// do -- and the plural carries both spellings, as they all do.
    #[test]
    fn the_past_passive_participle_deranks_in_the_oblique() {
        for want in [
            "käyty",
            "käydyn",
            "käydyssä",
            "käydyttä",
            "käydyiltä",
            "käydyin",
            "käydyt",
            "käytyä",
            "käydyissä",
        ] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
        // The geminated spellings of class 61 are not words here.
        for absent in ["käytty", "käyttyn", "käyttyä"] {
            assert!(hasnt("käydä", absent), "käydä should not have {absent}");
        }
    }

    /// The second infinitive runs on the `d`, where class 53 runs on the `e`.
    #[test]
    fn the_second_infinitive_takes_the_d() {
        for want in ["käydessä", "käydessään", "käydessäni", "käyden"] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
        for absent in ["käyessä", "käyellä", "käyen", "käyvessä"] {
            assert!(hasnt("käydä", absent), "käydä should not have {absent}");
        }
    }

    /// The remaining infinitives and the active participles all run on the
    /// present stem, as in every other type. The fifth infinitive takes no bare
    /// form, only its possessives, as everywhere.
    #[test]
    fn the_infinitives_and_the_active_participles_run_on_the_present_stem() {
        for want in [
            "käydä",
            "käymä",
            "käyminen",
            "käymäisilläni",
            "käyvän",
            "käyvä",
            "käynyt",
            "käymätön",
        ] {
            assert!(has("käydä", want), "käydä should have {want}");
        }
        assert!(hasnt("käydä", "käymäisillä"));
    }
}
