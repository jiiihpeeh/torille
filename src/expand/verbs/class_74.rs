//! Class 74 (the `katketa` type) inflection, written by hand.
//!
//! **The type is class 73's with the theme vowel kept in the third person and
//! carried into the conditional.** Both are `-ta/-tä` types whose gradation
//! sits at the very end of the base, both put the infinitive's own `t` in front
//! of everything on the weak base, and both run the potential and the past
//! active participle on that weak base with a doubled `n` (`katkennee`,
//! `katkennut` beside `salannut`). What separates them is what the finite mood
//! does with the theme, and it is only two cells wide:
//!
//! ```text
//! katketa / katkean / katkesi / katkeaisi (katkeisi) / katkennee / katketkoon / katkennut / katkettiin
//! salata / salaan / salasi / salaisi          / salannee / salatkoon / salannut / salattiin
//! ```
//!
//! ## The present repeats the theme; class 73 does not
//!
//! The present stem is the strong base with the infinitive's theme vowel on it,
//! and it is the same stem in both classes (`katkea`, `salaa`, `pakkaa`,
//! `hikoa`). The third person differs and only the third person: class 73's is
//! that stem unchanged (`salaa`), class 74's repeats the theme (`katkeaa`),
//! which is what `paradigm`'s ordinary `pres3sg` rule -- the stem's own last
//! letter -- gives without being asked. Every other person hangs its ending on
//! the same stem in both (`katkean`, `katkeat`, `katkeamme`, `katkeatte`,
//! `katkeavat`), so class 74 needs **no** field where class 73 empties one.
//!
//! ## The conditional follows the present, and has a second spelling
//!
//! The conditional is the *present stem* with the ordinary `-isi`
//! (`katkeaisi`, `hikoaisi`, `kerkeäisi`), where class 73's is the strong base
//! bare (`salaisi`). That is the only other difference, and it is why the type
//! row carries the parenthetical that classes 60, 68 and 74 between them are
//! the only ones of `kotus-tyypit.txt` to carry:
//!
//! ```text
//! katkeaisi (katkeisi)
//! ```
//!
//! **The parentheses are a second set of all six persons, not a spare form**,
//! and every member that has a finite conditional at all has both: 99 of the
//! class's 180 lemmas have the alternant in all six persons as a `both` row, and
//! 43 more have it as omorfi's alone, so 142 of 180 carry it against the 38
//! that have no finite conditional in the reference at all. The alternant is the
//! **strong base** bare -- `katkeisi`, `hikoisi`, `kokoisi`, `herkeisi`,
//! `häpeisi` -- one letter short of the primary, and it is [`Stems::cond_alt`]
//! that carries it. It reaches the negative auxiliary too, which takes it
//! beside the primary in all six phrases (`en katkeaisi` and `en katkeisi`).
//!
//! The alternant stops there. The passive conditional (`katkettaisiin`) and
//! every other conditional-shaped cell have one spelling each, and are built
//! from stems `cond_alt` does not touch.
//!
//! ## The weak base, and where the two grades divide
//!
//! As in class 73 the sanalista writes every member in the weak grade (`hiota`
//! has no `k`, `koota` its single `o`, `keretä` its single `e`), so the weak
//! base is the lemma's own stem and the strong one is [`strong_base_of`]'s
//! work -- and `D` (`k` against nothing) is the commonest code in the class
//! for the reason it is in classes 72 and 73.
//!
//! **The whole finite mood runs on the strong base**, for the same reason class
//! 73's does: the gradation sits at the *end* of the base, so the weak form is
//! not a lighter variant of anything (`hi` + `k` + `oan` is a word, `hioan` is
//! not) and the paradigm has no weak present. So this module hands `Stems` a
//! stem that already carries the strong grade (`gradation: None`, both grades
//! worked out by hand) and the weak grade appears only in the fields that name
//! it as a string: the imperative's plurals (`katketkaa`, `katketkoon`), the
//! second infinitive (`katketessa`, `katketen`), the passive theme
//! (`katketaan` beside `katkettiin`), and the potential and the past active
//! participle on the [`Alt`] shape classes 72 and 73 share (`katke` + `n` +
//! `n` + `ee` = `katkennee`, `katke` + `n` + `n` + `ut` = `katkennut`).
//!
//! The imperative's second person singular and the negated present are the
//! present stem (`katkea`: `katkea!`, `en katkea`), and the third and fourth
//! infinitives, the present participle, the agent and the negative participle
//! take it too (`katkeama`, `katkeaminen`, `katkeava`, `katkeamaton`).

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Alt, PassiveCons, Stems, front, paradigm, strong_base_of};

pub const CLASS: &str = "74";

/// The weak base of a class-74 verb: the lemma less its `ta`/`tä` (`katke`,
/// `hio`, `koo`, `kere`).
///
/// The sanalista writes every member in this grade, so the base is the lemma's
/// own stem and the strong grade is the one the module has to work out.
/// The stripping itself is [`ta_base`], which classes 72 to 76 all use, so
/// what each of them has to say is which grade the sanalista wrote it in.
use super::utility::ta_base as base_of;

/// Inflect a class-74 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let base = base_of(lemma);
    let strong = strong_base_of(base, gradation);
    // The infinitive's own theme vowel, which is also the type's: `-ata` gives
    // `katkea` and `hikoa`, `-ätä` gives `kerkeä`.
    let theme = if front(lemma) { 'ä' } else { 'a' };
    // The present is the strong base with the theme on it. Every cell of the
    // mood runs on that strong base, which `gradation: None` below is what
    // says: the two grades are already worked out by hand, so `Stems::weak`
    // has nothing left to weaken.
    let present = format!("{strong}{theme}");
    // The past is that base with the ending's `s` in front of its own `i`, so
    // that the third person's bare `i` gives `katkesi`.
    let past = format!("{strong}s");
    let mut s = Stems::with_past(lemma, &present, &past, None);
    // The conditional follows the *present* stem -- `katkea` + `isi` is
    // `katkeaisi` -- and the type row's parenthesised alternant is the strong
    // base bare, one letter short of it: `katkeisi`.
    s.cond_stem = present.clone();
    s.cond_alt = Some(strong.clone());
    // The potential and the past active participle run on the weak base with the
    // class's doubled `n` in front of the ending: `katkennee`, `katkennut`.
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
    // on the weak base, and the passive's theme is the base itself so that the
    // `t` lands on the stem's own: `katketkaa`, `katketessa`, `katketaan`
    // beside `katkettiin`.
    s.imp_stem = format!("{base}t");
    s.inf2_stem = format!("{base}te");
    s.p_theme = base.to_string();
    // The passive is [`PassiveCons::Geminated`]: one `t` in the present and two
    // everywhere else -- `katketaan`, `keretään`, `kaikotaan` beside
    // `katkettiin`, `katkettaisiin`, `katkettaneen` and `katkettakoon`. That
    // is not a cell where class 74 parts company with class 73, which writes it
    // the same way (`salataan` beside `salattiin`); it is where it parts company
    // with every type whose passive present has no consonant of its own. 144 of
    // the class's 180 members have a passive present at all, and all 144 write
    // the single `t`; the other 36 have no such row.
    s.p_cons = PassiveCons::Geminated;
    paradigm(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Mood, Polarity};

    fn forms(lemma: &str, code: Option<Gradation>) -> Vec<String> {
        inflect(lemma, code)
            .into_iter()
            .map(|f| f.word)
            .collect()
    }

    fn has(lemma: &str, code: Option<Gradation>, word: &str) -> bool {
        forms(lemma, code).iter().any(|w| w == word)
    }

    #[test]
    fn base_is_the_lemma_less_its_ta() {
        assert_eq!(base_of("katketa"), "katke");
        assert_eq!(base_of("keretä"), "kere");
        assert_eq!(base_of("hiota"), "hio");
    }

    #[test]
    fn an_ungradated_member_is_one_paradigm() {
        let got = forms("katketa", None);
        for word in [
            "katketa", "katkean", "katkeat", "katkeaa", "katkeamme", "katkeatte", "katkeavat",
            "katkesin", "katkesit", "katkesi", "katkesimme", "katkesitte", "katkesivat",
            "katkennemme", "katkennen", "katkennet", "katkenneemme", "katkennee", "katkennette",
            "katkennevat",
            "katkennut", "katkenneet", "katketkaa", "katketkaamme", "katketkoon", "katketkoot",
            "katketen", "katketessa", "katkettaessa", "katketaan", "katkettiin", "katkettava",
            "katkettu", "katkettaisiin", "katkettaneen", "katkettakoon", "katkeamaan", "katkettaman",
            "katkeama", "katkeaminen", "katkeava", "katkeamaton",
        ] {
            assert!(got.iter().any(|w| w == word), "missing {word}");
        }
    }

    #[test]
    fn the_third_person_repeats_the_theme() {
        // The one cell where this type parts company with class 73: the same
        // present stem `katkea`, and the theme repeated in the third person
        // rather than left as it stands (`katkeaa`, not `katkea`).
        assert!(has("katketa", None, "katkeaa"));
        assert!(has("erota", None, "eroaa"));
    }

    #[test]
    fn the_conditional_has_both_spellings_on_the_two_grades() {
        // The primary hangs on the present stem ...
        for word in [
            "katkeaisi", "katkeaisin", "katkeaisit", "katkeaisimme", "katkeaisitte", "katkeaisivat",
        ] {
            assert!(has("katketa", None, word), "missing {word}");
        }
        // ... and the type row's parenthesised alternant on the strong base
        // bare, one letter short of it.
        for word in [
            "katkeisi", "katkeisin", "katkeisit", "katkeisimme", "katkeisitte", "katkeisivat",
        ] {
            assert!(has("katketa", None, word), "missing {word}");
        }
        // The alternant does not reach the passive, which has one spelling.
        assert!(has("katketa", None, "katkettaisiin"));
        assert!(!has("katketa", None, "katkettisiin"));
    }

    #[test]
    fn both_spellings_take_the_strong_base() {
        // `hiota` is `74*D`: base `hio`, strong `hiko`, so `hikoa` is the
        // present stem and `hikoisi` the alternant -- never `hioa`/`hioisi`.
        assert!(has("hiota", Some(Gradation(4)), "hikoaa"));
        assert!(has("hiota", Some(Gradation(4)), "hikoaisi"));
        assert!(has("hiota", Some(Gradation(4)), "hikoisi"));
        assert!(!has("hiota", Some(Gradation(4)), "hioaa"));
        assert!(!has("hiota", Some(Gradation(4)), "hioisi"));
        // The front-vowel `D` member of the type row's own family: `kerkeäisi`
        // and `kerkeisi` on the strong base, not `kereäisi`/`kereisi` on the
        // weak one. Omorfi has `kereisi` and libvoikko rejects it.
        assert!(has("keretä", Some(Gradation(4)), "kerkeää"));
        assert!(has("keretä", Some(Gradation(4)), "kerkeäisi"));
        assert!(has("keretä", Some(Gradation(4)), "kerkeisi"));
        assert!(!has("keretä", Some(Gradation(4)), "kereisi"));
        // A front-vowel member whose gradation is not `D`: `herjetä` is `74*L`,
        // strong `herke`, and its reference has both `herkeäisi` and `herkeisi`.
        assert!(has("herjetä", Some(Gradation(12)), "herkeäisi"));
        assert!(has("herjetä", Some(Gradation(12)), "herkeisi"));
    }

    #[test]
    fn the_finite_mood_runs_on_the_strong_base_throughout() {
        // `kaikota` is `74*A`, so its strong base is `kaikko` and no cell of
        // the mood may show the weak `kaiko`.
        for word in [
            "kaikkoan", "kaikkoaa", "kaikkosi", "kaikkoaisi", "kaikkoisi", "kaikkoamme",
        ] {
            assert!(has("kaikota", Some(Gradation(1)), word), "missing {word}");
        }
        // ... while the weak base is what the potential, the participle, the
        // imperative's plurals, the second infinitive and the passive take -- and
        // the passive is the cell that writes a single `t` in its present.
        for word in [
            "kaikonnee", "kaikonnut", "kaikotkaa", "kaikotkoon", "kaikotessa", "kaikoten",
            "kaikottava", "kaikottu", "kaikotaan", "kaikottiin", "kaikottaisiin",
        ] {
            assert!(has("kaikota", Some(Gradation(1)), word), "missing {word}");
        }
    }

    #[test]
    fn the_front_vowel_type_fronts_the_way_the_past_participle_does() {
        // `herjetä` is `74*L`: weak base `herje`, strong `herke`. Every word
        // below is a `both` row of its reference entry, and the two grades fall
        // where this module says they do.
        for word in [
            // the present on the strong base, theme repeated in the third person
            "herkeän", "herkeät", "herkeää", "herkeämme", "herkeätte", "herkeävät",
            // the past on the strong base too, with the ending's `s`
            "herkesi", "herkesin", "herkesit", "herkesimme", "herkesitte", "herkesivät",
            // both conditional spellings, on the strong base
            "herkeäisi", "herkeäisin", "herkeisi", "herkeisin",
            // the potential and the participle on the weak one, `n` doubled
            "herjennee", "herjennemme", "herjennen", "herjennyt", "herjenneet",
            // the imperative's plurals and the second infinitive, weak base + `t`
            "herjetkää", "herjetkäämme", "herjetköön", "herjetkööt", "herjetessä", "herjeten",
            // the passive: one `t` in the present, two everywhere else
            "herjetään", "herjettiin", "herjettävä", "herjetty", "herjettäisiin",
            "herjettäneen", "herjettäköön", "herjettäessä", "herjettämän",
            // the third and fourth infinitives, the present participle, the agent
            // and the negative participle, all on the present stem
            "herkeämä", "herkeämän", "herkeäminen", "herkeävä", "herkeämätön",
        ] {
            assert!(has("herjetä", Some(Gradation(12)), word), "missing {word}");
        }
    }

    #[test]
    fn the_imperative_singular_and_the_negated_present_are_the_present_stem() {
        // The same word twice, as the two cells that share the present stem: the
        // imperative's second person and the bare negative. `paradigm` pushes it
        // once for each, and the reference has both rows.
        let mut got: Vec<(Option<Mood>, Polarity)> = Vec::new();
        for f in inflect("katketa", None) {
            if f.word == "katkea" {
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
