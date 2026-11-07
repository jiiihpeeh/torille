//! Class 69 (the `valita` type) inflection, written by hand.
//!
//! **One base carries every finite cell of the class.** The type row says
//!
//! ```text
//! valita / valitsen / valitsi / valitsisi / valinnee / valitkoon / valinnut / valittiin
//! ```
//!
//! and four of those seven cells are the same five letters with a different
//! ending on it. The base is `valit` -- the lemma less its `ta`, with the type's
//! own `t` in front, `vali` + `t` -- and `se` and `si` go on the front of it for
//! the present and the past, with the conditional's own `isi` on a `valits` that
//! the past reaches by taking the ending's `i` (`valits` + `isi` is `valitsisi`
//! and `valits` + `i` is `valitsi`). It is the same base and the same `s` class
//! 68's `-tse` alternant is built on -- there the `i` is on the stem and
//! swallowed, so its conditional is `tupakoitsisi` where this type's is
//! `valitsisi` -- which is the whole of what `tupakoida` and `valita` share; the
//! two classes differ in which of the two paradigms is the main one.
//!
//! So the stems are [`Stems::ita`] and this module has nothing to say about
//! them. There is no gradation code: all 48 members are plain `69`, and all 48
//! end in `ta`/`tä`, so there is no member to tell apart either.
//!
//! Four cells are where the base shows and the present stem does not, and each
//! of them is the type's own.
//!
//! **The second infinitive and the imperative's plurals run on the base, not on
//! the present stem.** `valitessa` and `valiten` stand on `valite` with `ssa` and
//! `n` on it, not on `valitsessa` and `valitsen`; `valitkaa` and `valitkoon` are
//! the base with `kaa` and `koon` where the present stem would give `valitsekaa`
//! and `valitsekoon` -- while the imperative's second person singular is the
//! present stem whole (`valitse`), so one cell of the type is on each of the
//! two. This is why `inf2_stem` and `imp_stem` are set in [`Stems::ita`] rather
//! than left on the present stem.
//!
//! **The passive needs no consonant of its own in the present.** Its theme is
//! the base, and the base already ends in the `t` the passive wants -- the type's
//! own, the same one its potential drops again -- so the present is the theme
//! with `-aan` straight on it (`valitaan`) where every other type adds a
//! consonant (`sanotaan` beside `sallitaan`) and class 67 gives its theme's own
//! back (`tullaan`). Everywhere else the gemination is the ordinary one:
//! `valittiin` beside `sallittiin`, `valittava` beside `sallittava`. That is
//! [`PassiveCons::ThemeOwn`](super::utility::PassiveCons::ThemeOwn).
//!
//! **The potential and the past active participle are the lemma's own stem with
//! the `n` doubled in front of the ending.** `valinnee` and `valinnut` are
//! `valin` + `n` + `nee` and `valin` + `n` + `ut`, and `valin` is `vali` + `n`
//! where `vali` is the lemma less its `ta` -- so this is the one cell a type has
//! on a stem that is neither the base nor the present stem, and the only one that
//! writes its lead consonant twice, which the ordinary
//! [`Alt::n`](super::utility::Alt) already does. Type 1 writes the same `n` once
//! on each side of a vowel-initial stem (`sano` + `n` + `ee`), so the two look
//! alike and are not: `vali` + `n` + `n` + `ee`, not `vali` + `n` + `ee`.
//!
//! **The conditional is the base with `s` on it, and cannot be reached from the
//! present stem.** `absorb_i` swallows the ending's own `i` in front of an
//! `i`-final stem, so a stem with `si` or `sisi` on it would give `valitsisisi`
//! for the third person where the word is `valitsisi`. `valits` is not `i`-final
//! and takes the ordinary `-isi` set whole: `valitsisi`, `valitsisin`,
//! `valitsisimme`, `valitsisivät`. So `cond_stem` is set, as it is for every type
//! whose conditional cannot be reached from the present.
//!
//! ## What the reference says
//!
//! The type is fully regular: all 48 members are plain `69` with no gradation
//! code, all 48 end in `ta`/`tä`, and not one member has a cell the others do
//! not. Front harmony is the only thing that varies (`keritsen`, `keritsimme`,
//! `kerittää`, `kerittämään`, `keritty`).
//!
//! **46 of the 48 members audit at 0 invented and 0 missing against `both`**, and
//! leave one `union` row apiece uncovered: the mis-tagged `...imatontaan` that
//! every verb class in this project has (`valitsematontaan` is `omorfi`-only,
//! `polarity` `affirmative` under `participle = negative`, and not a Finnish word
//! -- the `-maton` participle with the genitive's `t` in it where
//! `valitsemattomaan` is the word).
//!
//! The other two are the class's only two-reading members, and the reference
//! pools both readings' rows into one set that no single class's rule can match.
//! Neither gap is a cell this module gets wrong: on both, every row the
//! reference files under class 69 is generated.
//!
//! * `keritä` is homonym 1 `75*D` and homonym 2 `69`. All 369 of its class-69
//!   `both` rows are generated; the 491 rows on the other reading's `kerki` stem
//!   are class 75*D's, which class 75 does not cover yet. The 723 words over are
//!   the rest of class 69's paradigm, which the reference does not list for this
//!   lexeme because most of its rows went to the other reading: 554 of the
//!   module's 1 277 words are in the reference and 723 are not, where `valita`
//!   has 1 188 words and none of them over.
//! * `kaita` is homonym 1 the adjective `9*F` and homonym 2 `69`. It has no
//!   missing row at all, and none of its 151 words over is on a class-69 stem:
//!   they are the nominal path's own (`kaitaa` and `kaitana` are the verb's 1st
//!   infinitive declined as a noun, `kaitoihin` and `kaitoinen` its 4th
//!   infinitive, and `kaidoin` and `kaidoissa` the adjective's own stem), which
//!   the verb module neither adds to nor can subtract from.

#![allow(dead_code)]

use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "69";

/// Inflect a class-69 verb into its whole paradigm.
pub fn inflect(lemma: &str) -> Vec<Form> {
    paradigm(&Stems::ita(lemma))
}

#[cfg(test)]
mod tests {
    use super::super::utility::front;
    use super::*;

    /// Whether the paradigm of `lemma` has `form`.
    fn has(lemma: &str, form: &str) -> bool {
        inflect(lemma).iter().any(|f| f.word == form)
    }

    /// Assert that a member has every one of `wants`.
    fn has_all(lemma: &str, wants: &[String]) {
        for want in wants {
            assert!(has(lemma, want), "{lemma} should have {want}");
        }
    }

    /// Assert that a member has none of `absents`.
    fn has_none(lemma: &str, absents: &[String]) {
        for absent in absents {
            assert!(!has(lemma, absent), "{lemma} should not have {absent}");
        }
    }

    /// A member's five stems: the base (`valit`), the present and the past that
    /// take a letter in front of it, the conditional that takes the ordinary
    /// `-isi` on a `valits`, and the lemma's own stem plus `n` that the
    /// potential and the past active participle run on. The three vowels are the
    /// word's own harmony, so a front member's expectations are spelled by the
    /// same rule the paradigm uses.
    fn t(lemma: &str) -> (String, String, String, String, String, char, char, char) {
        let (a, o, u) = if front(lemma) {
            ('ä', 'ö', 'y')
        } else {
            ('a', 'o', 'u')
        };
        let s = Stems::ita(lemma);
        (
            s.imp_stem.clone(),
            s.stem.clone(),
            s.past_stem.clone(),
            s.cond_stem.clone(),
            s.pot.stem.clone(),
            a,
            o,
            u,
        )
    }

    /// The second infinitive's inessive ending, whose own `a`/`ä` harmonises
    /// while the `e` in front of it does not.
    fn ss(lemma: &str) -> &'static str {
        if front(lemma) { "ssä" } else { "ssa" }
    }
    /// The three tense cells, and how the letters reach them. The present and
    /// the past take a letter on the front of the base -- `se` and `si` -- and
    /// the conditional takes none but ends in the base's own `s`, so the
    /// ordinary `-isi` set goes on it whole. The past is the one `i`-final stem
    /// of the three and so is the only one that swallows its endings' `i`; the
    /// present is vowel-final and does not, and its third person is an `e` where
    /// the type-1 one repeats the theme.
    #[test]
    fn the_three_tense_stems_are_one_base_with_a_letter_in_front() {
        for lemma in [
            "valita",
            "tarvita",
            "mainita",
            "vaaita",
            "kestita",
            "keritä",
            "häiritä",
            "tilkitä",
            "suudita",
        ] {
            let (base, stem, past, cond, _, a, _, _) = t(lemma);
            assert_eq!(stem, format!("{base}se"));
            assert_eq!(past, format!("{base}si"));
            assert_eq!(cond, format!("{base}s"));
            has_all(
                lemma,
                &[
                    // The present: six endings on a vowel-final stem, so the
                    // third person is the stem with an `e` of its own.
                    stem.clone(),
                    format!("{stem}n"),
                    format!("{stem}t"),
                    format!("{stem}e"),
                    format!("{stem}mme"),
                    format!("{stem}tte"),
                    format!("{stem}v{a}t"),
                    // The past: the same six on an `i`-final stem, which is
                    // where it and the type-1 cells differ. The bare stem is
                    // the third person, and there is no word for it alone.
                    format!("{past}n"),
                    format!("{past}t"),
                    format!("{past}mme"),
                    format!("{past}tte"),
                    format!("{past}v{a}t"),
                    // The conditional: not `i`-final, so nothing is swallowed
                    // and the whole `-isi` set stands.
                    format!("{cond}isi"),
                    format!("{cond}isin"),
                    format!("{cond}isit"),
                    format!("{cond}isimme"),
                    format!("{cond}isitte"),
                    format!("{cond}isiv{a}t"),
                ],
            );
            has_none(
                lemma,
                &[
                    // Type 1's repeated theme vowel, which no cell here has.
                    format!("{base}{a}i"),
                    // The `i`-final past with its endings written whole.
                    format!("{past}in"),
                    format!("{past}it"),
                    format!("{past}i"),
                    format!("{past}imme"),
                    format!("{past}itte"),
                    format!("{past}ivat"),
                    // The conditional read off the present stem, and off the
                    // past: the two mistakes `cond_stem` exists to stop.
                    format!("{stem}isi"),
                    format!("{stem}isin"),
                    format!("{past}isi"),
                    format!("{past}isin"),
                    // And the `sisi` stem, which is class 68's conditional and
                    // not this type's: `i`-final, so `absorb_i` would swallow
                    // the ending's `i` and give a fourth `i` in a row.
                    format!("{base}sisisi"),
                    format!("{base}sisisin"),
                ],
            );
        }
    }

    /// The two cells that run on the base rather than on the present stem,
    /// which is the one thing about this type no other has. The imperative is
    /// both of them in a single cell: the second person singular is the stem
    /// and every other person is the base.
    #[test]
    fn the_second_infinitive_and_the_imperative_run_on_the_base() {
        for lemma in ["valita", "tarvita", "keritä", "kestitä"] {
            let (base, stem, _, _, _, a, o, _) = t(lemma);
            has_all(
                lemma,
                &[
                    // The second infinitive's inessive and instructive, both off
                    // the base with the connective `e` in front of the ending.
                    format!("{base}e{}", ss(lemma)),
                    format!("{base}en"),
                    // The imperative: the stem for the second person and the
                    // base for the rest.
                    stem.clone(),
                    format!("{base}k{o}{o}n"),
                    format!("{base}k{a}{a}mme"),
                    format!("{base}k{a}{a}"),
                    format!("{base}k{o}{o}t"),
                ],
            );
            // The same cells read off the present stem are not words: both
            // spellings of the second infinitive, and the whole imperative.
            has_none(
                lemma,
                &[
                    format!("{stem}{}", ss(lemma)),
                    format!("{stem}e{}", ss(lemma)),
                    format!("{stem}en"),
                    format!("{stem}k{o}{o}n"),
                    format!("{stem}k{a}{a}"),
                    format!("{stem}k{o}{o}t"),
                ],
            );
        }
    }

    /// The passive takes no consonant of its own in the present, because the
    /// theme's own `t` is the one the passive wants. Everywhere else it
    /// gemmates as every type does, and the `-tu` participle is the ordinary
    /// one that drops its doubled `t` in the oblique.
    #[test]
    fn the_passive_adds_no_consonant_in_the_present() {
        for lemma in ["valita", "tarvita", "keritä", "tilkitä"] {
            let (base, _, _, _, _, a, o, u) = t(lemma);
            // The geminated theme, and the mood theme on top of it: the base
            // already ends in `t`, so the gemination is one more of them.
            let tt = format!("{base}t");
            let pm = format!("{tt}{a}");
            has_all(
                lemma,
                &[
                    // The present, with nothing in front of the `-aan`.
                    format!("{base}{a}{a}n"),
                    // The past and the moods, with the gemination as usual.
                    format!("{base}tiin"),
                    format!("{pm}v{a}"),
                    format!("{pm}isiin"),
                    format!("{pm}neen"),
                    format!("{pm}k{o}{o}n"),
                    format!("{pm}{}", if front(lemma) { "essä" } else { "essa" }),
                    format!("{pm}m{a}n"),
                    // The negated main-verb words: the theme alone, and the
                    // geminated theme plus the participle's own vowel.
                    format!("{base}{a}"),
                    format!("{tt}{u}"),
                    format!("{pm}isi"),
                    format!("{pm}ne"),
                    format!("{pm}{}", if front(lemma) { "kö" } else { "ko" }),
                    // The `-tu` participle: geminated in the nominative and the
                    // cases that keep it, with a single `t` in the oblique.
                    format!("{tt}{u}{a}"),
                    format!("{tt}{u}{u}n"),
                    format!("{base}{u}n"),
                    format!("{base}{u}t"),
                    format!("{base}{u}{}", ss(lemma)),
                ],
            );
            has_none(
                lemma,
                &[
                    // A `t` in front of the present's `-aan`, which is what
                    // every other type has and what this one has not.
                    format!("{base}t{a}{a}n"),
                    // A third `t` where the gemination is already two.
                    format!("{base}tttiiin"),
                    format!("{base}tko"),
                ],
            );
        }
    }

    /// The potential and the past active participle are the lemma's own stem
    /// with the `n` doubled in front of the ending: `valin` + `n` + `nee` and
    /// `valin` + `n` + `ut`, where type 1 writes `sano` + `n` + `ee` and so
    /// spells its `n` only once. This is the one cell a type has on a stem that
    /// is neither the base nor the present stem.
    #[test]
    fn the_potential_and_the_past_participle_double_the_lemmas_own_n() {
        for lemma in ["valita", "tarvita", "mainita", "keritä", "kestitä"] {
            let (base, _, _, _, short, a, _, u) = t(lemma);
            // The base less the type's own `t`, plus the `n` of the lead.
            let bare = &base[..base.len() - 1];
            assert_eq!(short, format!("{bare}n"));
            has_all(
                lemma,
                &[
                    // The potential, the lead `n` and the ending's `e` in a row.
                    format!("{short}nen"),
                    format!("{short}net"),
                    format!("{short}nee"),
                    format!("{short}neemme"),
                    format!("{short}nette"),
                    format!("{short}nev{a}t"),
                    // The past active participle on the same stem: the `n` and
                    // the `-ut` in a row, which is what makes `valinnut` rather
                    // than the `valinut` a single `n` would give.
                    format!("{short}n{u}t"),
                    format!("{short}n{u}tt{a}"),
                    // And its plural, the ordinary `-nee-` shape.
                    format!("{short}neen"),
                    format!("{short}neit{a}"),
                ],
            );
            // Type 1's own `n`-lead on the base is not a word in either cell,
            // and neither is a single `n` where the type writes two.
            has_none(
                lemma,
                &[
                    format!("{base}nen"),
                    format!("{base}nee"),
                    format!("{base}n{u}t"),
                    format!("{base}n{u}tt{a}"),
                    format!("{base}neita"),
                    format!("{short}{u}t"),
                    format!("{short}{u}tt{a}"),
                ],
            );
        }
    }
}
