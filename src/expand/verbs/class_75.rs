//! Class 75 (the `selvitä` type) inflection, written by hand.
//!
//! **The type is class 74's without the parenthesised conditional alternant.**
//! Both are end-gradating `-tä` types with a lemma-less base, both put the
//! infinitive's theme vowel on the present stem and repeat it in the third
//! person, both run the potential and the past active participle on the weak
//! base with a doubled `n`, and both write the passive with one `t` in the
//! present and `tt` everywhere else. Cell for cell the two types agree
//! everywhere except the conditional:
//!
//! ```text
//! 74  katketa / katkean / katkesi / katkeaisi (katkeisi) / katkennee / katketkoon / katkennut / katkettiin
//! 75  selvitä / selviän / selvisi / selviäisi / selvinnee / selvitköön / selvinnyt / selvittiin
//! ```
//!
//! Class 74's type row parenthesises a second spelling of all six conditional
//! persons (`katkeisi` beside `katkeaisi`), which this module's neighbour
//! carries as [`Stems::cond_alt`]. Class 75's row carries no parenthetical, so
//! its conditional has the one spelling: `selviäisi`, `kerkiäisi`, `bingoaisi`,
//! and never `kerkiisi` or `bingoiisi`. The negated conditional takes no
//! second spelling either — `en selviäisi` alone, where class 74 also has
//! `en kerkeisi`.
//!
//! ## The base, weak and strong
//!
//! As in classes 72 to 74, the sanalista writes every member in the weak grade,
//! so the weak base is the lemma less its `-ta/-tä` — `selvi`, `keri`, `bingo`,
//! `lasso` — and the strong base is [`strong_base_of`]'s work where the class
//! names gradation. `D` is the interesting one here, because `k` is the weak
//! grade's loss rather than its own: `keritä` and `siitä` are written without
//! the `k` their strong grades carry, so `strong_base_of` puts it back in front
//! of the base's last vowel and the finite mood runs on `kerki` and `siki`
//! (`kerkiän`, `sikiän`, `siinnyt`). All the other codes work the ordinary way,
//! and `peitota`'s `C` is worth naming because the site it lands on is the
//! infinitive's own `t`: `peittoan`, `peitottu`, `peitotkaamme`.
//!
//! The whole finite mood runs on the strong grade, for the reason class 74's
//! module gives: the gradation sits at the *end* of the base, so the weak grade
//! is not a variant of anything that could carry a person on its own. That is
//! what `gradation: None` in [`Stems::with_past`] says here — the two grades are
//! already worked out by hand, so `Stems::weak` has nothing left to weaken.
//!
//! The present stem is the strong base with the theme on it (`selviä`, `kerkiä`,
//! `bingoa`), the past is that base with the ending's `s` (`selvisi`, `kerkisi`,
//! `bingosi`), and the conditional is the present stem with `-isi` (`selviäisi`,
//! `kerkiäisi`, `bingoaisi`).
//!
//! Everything else runs on the weak base, with the type's own `t` in front of
//! it: the imperative's plurals (`selvitkäämme`, `keritköön`) and the second
//! infinitive (`selvitessä`, `keritessä`) take `t` or `te`, the potential and
//! the past active participle take the doubled `n` (`selvinnee`, `selvinnyt`),
//! and the passive theme is the base itself. The imperative's second person
//! singular and the negated present are the present stem (`selviä`, `älä
//! selviä`), and so are the third and fourth infinitives, the present
//! participle, the agent and the negative participle (`selviämän`,
//! `selviäminen`, `selviävä`, `selviämä`, `selviämätön`).
//!
//! ## The passive
//!
//! One `t` in the present and `tt` in the past and the moods, which is
//! [`PassiveCons::Geminated`] and every member that has a passive present at
//! all writes (`selvitään` beside `selvittiin`, `selvittäisiin`,
//! `selvittäneen`, `selvittäköön`, `selvittävä`, `selvitty`). The type row's
//! `selvittiin` is where the split shows; the present is the same one `t` that
//! classes 72 to 74 write (`vanhetaan`, `salataan`, `katketaan`).
//!
//! The present's third person is the one place where class 75 differs from
//! class 73, and it is a difference of *spelling*, not of stem: both leave
//! `Stems::pres3sg` at its default, the present stem's own last letter, which is
//! the theme. So `selviä` + `ä` is `selviää` where class 73's already-doubled
//! `salaa` + nothing is `salaa`.
//!
//! ## What the reference leaves
//!
//! 46 of the class's 48 lemmas are clean, and all 36 806 of their `both` rows
//! are generated with nothing invented — up from 5 clean and 32 964 missing
//! before this module. The two that are not are both reference defects, and
//! neither is repairable without hard-coding a word list:
//!
//! * `keritä` files **two** homonym rows, tagged `75*D` and `69`, and they are
//!   byte-identical — 1255 forms each, all 1255 shared, none differing. So the
//!   reference tabulates one paradigm for this lemma and labels the second copy
//!   with a type it does not use: there is not one `keritse`-shaped row for
//!   `keritä` in either source. The module answers the `75*D` reading, and the
//!   `69` reading's 663 forms are what the audit counts as invented. This is the
//!   same defect as class 72's `isota` and `tyvetä`, whose two homonym rows are
//!   byte-identical too; there the tag was class 72's on a class-74-shaped
//!   entry, here it is class 69's on a class-75-shaped one.
//! * `ryöpytä` is the class's only `B` member and its reference entry is thin
//!   (500 `both` rows against the class median of 982): Kotus-Verbit holds the
//!   bare forms of the non-finite nominals and omorfi holds their possessives.
//!   The six bare forms the two sources agree on all carry the theme —
//!   `ryöppyämän`, `ryöppyämällä`, `ryöppyämässä` — and omorfi then drops that
//!   theme from the very same series' possessive forms, writing `ryöppymien`
//!   and `ryöppymälläni` where the theme gives `ryöppyämien` and
//!   `ryöppyämälläni`. All 625 invented forms are that one disagreement, and
//!   omorfi is inconsistent with itself: its own unpossessed rows for this
//!   lemma keep the theme.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Alt, PassiveCons, Stems, front, paradigm, strong_base_of};

pub const CLASS: &str = "75";

/// The weak base of a class-75 verb: the lemma less its `ta`/`tä` (`selvi`,
/// `keri`, `bingo`, `lasso`).
/// The stripping itself is [`ta_base`], which classes 72 to 76 all use, so
/// what each of them has to say is which grade the sanalista wrote it in.
use super::utility::ta_base as base_of;

/// Inflect a class-75 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let base = base_of(lemma);
    let strong = strong_base_of(base, gradation);
    // The infinitive's own theme vowel, which is also the type's: `-itä` gives
    // `selviä` and `kerkiä`, `-ota` gives `bingoa` and `lassoa`.
    let theme = if front(lemma) { 'ä' } else { 'a' };
    // The present is the strong base with the theme on it, and every cell of the
    // finite mood runs on that strong base — see the module header for why the
    // weak grade cannot carry a person here.
    let present = format!("{strong}{theme}");
    // The past is that base with the ending's `s` in front of its own `i`, so
    // that the third person's bare `i` gives `selvisi`.
    let past = format!("{strong}s");
    let mut s = Stems::with_past(lemma, &present, &past, None);
    // The conditional follows the *present* stem -- `selviä` + `isi` is
    // `selviäisi` -- and unlike class 74 the type row parenthesises no second
    // spelling, so `cond_alt` stays `None` and the negated conditional has the
    // one spelling too.
    s.cond_stem = present.clone();
    // The present's third person is `paradigm`'s ordinary rule: the present
    // stem's own last letter, which is the theme, so `selviä` repeats it into
    // `selviää`. Class 73 is the one of the family that empties the field.
    s.pres3sg = Stems::pres3sg_of(&present);
    // The potential and the past active participle run on the weak base with the
    // class's doubled `n` in front of the ending: `selvinnee`, `selvinnyt`.
    let plain_n = format!("{base}n");
    s.pot = Alt {
        stem: plain_n.clone(),
        lead: "n",
    };
    s.past_part = Alt {
        stem: plain_n,
        lead: "n",
    };
    // The imperative's plurals and the second infinitive take the type's own `t`
    // and `te` on the weak base, and the passive takes the bare base with one
    // `t` in its present and `tt` everywhere else.
    s.imp_stem = format!("{base}t");
    s.inf2_stem = format!("{base}te");
    s.p_theme = base.to_string();
    s.p_cons = PassiveCons::Geminated;
    paradigm(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Mood, Polarity};

    fn forms(lemma: &str, code: Option<Gradation>) -> Vec<String> {
        inflect(lemma, code).into_iter().map(|f| f.word).collect()
    }

    fn has(lemma: &str, code: Option<Gradation>, word: &str) -> bool {
        forms(lemma, code).iter().any(|w| w == word)
    }

    #[test]
    fn base_is_the_lemma_less_its_ta() {
        assert_eq!(base_of("selvitä"), "selvi");
        assert_eq!(base_of("keritä"), "keri");
        assert_eq!(base_of("bingota"), "bingo");
        assert_eq!(base_of("lassota"), "lasso");
    }

    #[test]
    fn the_third_person_repeats_the_theme() {
        // Class 73 empties this field because its stem has already written the
        // theme twice; here the theme is written once and repeated.
        assert!(has("selvitä", None, "selviä"));
        assert!(has("selvitä", None, "selviää"));
        assert!(has("eritä", None, "eriää"));
        assert!(has("bingota", None, "bingoaa"));
    }

    #[test]
    fn the_finite_mood_runs_on_the_strong_base_throughout() {
        assert!(has("keritä", Some(Gradation(4)), "kerkiän"));
        assert!(has("keritä", Some(Gradation(4)), "kerkiää"));
        assert!(has("keritä", Some(Gradation(4)), "kerkisi"));
        assert!(has("keritä", Some(Gradation(4)), "kerkiäisi"));
        // ... and the weak cells on the weak base, which for `D` means the `k`
        // the sanalista left out of the lemma.
        assert!(has("keritä", Some(Gradation(4)), "kerinnee"));
        assert!(has("keritä", Some(Gradation(4)), "kerinnyt"));
        assert!(has("keritä", Some(Gradation(4)), "keritköön"));
        // The negated present is the *present* stem, so it keeps the `k` the
        // lemma lost: the reference has `en kerkiä` beside `en kerinnee`.
        assert!(has("keritä", Some(Gradation(4)), "kerkiä"));
        assert!(!has("keritä", Some(Gradation(4)), "kerkinee"));
        assert!(!has("keritä", Some(Gradation(4)), "kerkinnyt"));
    }

    #[test]
    fn the_c_code_lands_on_the_infinitives_own_t() {
        // `peito` + `t` gemmates: the strong grade is `peitto`, the weak cells
        // put the `t` back on the bare base.
        assert!(has("peitota", Some(Gradation(3)), "peittoan"));
        assert!(has("peitota", Some(Gradation(3)), "peitottu"));
        assert!(has("peitota", Some(Gradation(3)), "peitotkaamme"));
        assert!(has("peitota", Some(Gradation(3)), "peitotessa"));
    }

    #[test]
    fn the_conditional_has_the_one_spelling_the_type_row_gives() {
        for word in [
            "selviäisi",
            "selviäisin",
            "selviäisit",
            "selviäisimme",
            "selviäisitte",
            "selviäisivät",
        ] {
            assert!(has("selvitä", None, word), "missing {word}");
        }
        // Class 74's parenthesised alternant, which this type does not have.
        assert!(!has("selvitä", None, "selviisi"));
        assert!(!has("keritä", Some(Gradation(4)), "kerkiisi"));
    }

    #[test]
    fn the_passive_splits_its_t() {
        assert!(has("selvitä", None, "selvitään"));
        assert!(has("selvitä", None, "selvittiin"));
        assert!(has("selvitä", None, "selvittäisiin"));
        assert!(has("selvitä", None, "selvittäneen"));
        assert!(has("selvitä", None, "selvittäköön"));
        assert!(has("selvitä", None, "selvittävä"));
        assert!(has("selvitä", None, "selvitty"));
    }

    #[test]
    fn the_imperative_and_the_negated_present_are_the_present_stem() {
        let mut got: Vec<(Option<Mood>, Polarity)> = Vec::new();
        for f in inflect("selvitä", None) {
            if f.word == "selviä" {
                got.extend(f.readings.iter().map(|t| (t.mood, t.polarity)));
            }
        }
        assert!(
            got.contains(&(Some(Mood::Imperative), Polarity::Affirmative)),
            "the imperative's second person is the present stem: {got:?}"
        );
        assert!(
            got.contains(&(Some(Mood::Indicative), Polarity::Negative)),
            "the negated present is the present stem: {got:?}"
        );
    }
}