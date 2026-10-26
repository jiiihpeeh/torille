//! Class 76 (the `taitaa` type) inflection, written by hand.
//!
//! The type is the fifth of the end-gradating `-ta`/`-tä` family and the odd one
//! out among them in three ways: the sanalista writes both its members in the
//! **strong** grade where classes 72 to 75 are written in the weak one, its theme
//! is the ending's consonant as well as its vowel, and its ending is one letter
//! longer. `tietää` is `tie` + `tä` + `a` and `taitaa` is `tai` + `ta` + `a`,
//! where classes 72 to 75 take the same `ta`/`tä` off the lemma and put back
//! only the vowel (`selvi` + `tä` = `selviä`).
//!
//! Its type row is the only one in the family with two alternants on it, and they
//! are two different cells rather than two readings of one, written
//! space-separated where class 74's single alternant is parenthesised:
//!
//! ```text
//! 76
//! taitaa                  the infinitive
//! taidan                  the present's first person
//! taisi                   the past's third person
//! taitaisi                the conditional's third person
//! taitanee tainnee        the potential's third person, two spellings
//! taitakoon               the imperative's third person
//! tainnut taitanut        the past active participle, two spellings
//! taidettiin              the passive imperfect
//! ```
//!
//! against class 74's, whose single alternant is on the conditional and
//! parenthesised:
//!
//! ```text
//! 74  ... katkeaisi (katkeisi) ...
//! ```
//!
//! ## The two stems, and which cells take which
//!
//! The base is the lemma less its `taa`/`tää` (`tie`, `tai`) — see [`base_of`],
//! which is *not* the shared `ta_base` for the reason given there — and the theme
//! is the `ta`/`tä` inside that, so the present stem is the two together: `tietä`.
//! Because the sanalista wrote the lemma in the strong grade, that stem already
//! carries the `F` pair's strong member (`t` against `d`) and `Stems::weak`
//! weakens it to `tiedä` on its own. The one thing that is unusual about this
//! type is thus the one thing the shared machinery already does, and the module
//! needs no `strong_base_of` at all.
//!
//! Which cells take which of the two grades is worth setting out, because
//! `paradigm` hands them out in the opposite order to every other type:
//!
//! ```text
//! third persons   tietä   tietä + ä  = tietää    tietä + vät = tietävät
//! everyone else   tiedä   tiedä + n   = tiedän     tiedä + t   = tiedät
//! ```
//!
//! So the third person repeats the theme stem's own last letter, which is what
//! `paradigm`'s `pres3sg` default gives and needs no field, while the first and
//! second persons take the weak grade — the ordinary rule, in the ordinary
//! direction. The negative present and the imperative's second person singular
//! read that same weak stem (`en tiedä`, `tiedä!`), the conditional and the
//! imperative's other three cells read the theme stem (`tietäisi`,
//! `tietäköön`), and the third and fourth infinitives, the present participle,
//! the agent and the negative participle are on the theme stem too
//! (`tietämän`, `tietäminen`, `tietävä`, `tietämä`, `tietämätön`).
//!
//! ## The past
//!
//! The past is the bare base with the ending's `s`, and it does *not* gradate:
//! `tie` + `s` is `ties` in all six cells (`tiesin`, `tiesi`, `tiesivät`).
//! That is the one stem the module has to hand over by hand, because
//! `with_past` would otherwise take the weak grade of the present stem less its
//! theme vowel — `tiedä` less `ä` is `tied`, which is not a Finnish stem. Every
//! other field is a default, and the passive's theme is one of them, which is
//! worth a line of its own below.
//!
//! ## The two alternants
//!
//! The potential and the past active participle are each spelled twice, once on
//! the bare base and once on the theme stem: `tiennee` beside `tietänee`,
//! `tiennyt` beside `tietänyt`. One [`Stems::alt`] carries the alternant of both,
//! and `paradigm` runs each cell once per stem — the same trick class 74's
//! conditional needed [`Stems::cond_alt`] for, one field lower.
//!
//! The spellings run all the way down. The participle's plural is `tienneet`
//! *and* `tietäneet`, its whole declension is built twice, and so is every
//! compound phrase that quotes it (`on tiennyt` beside `on tietänyt`,
//! `eivät ole tienneet` beside `eivät ole tietäneet`, `lienee tiennyt` beside
//! `lienee tietänyt`), which is why `compound::Parts` grew a matching alternant
//! for the participle and for the potential's connegative (`en tienne` beside
//! `en tietäne`). The imperative's connegative has one spelling in every type,
//! so it did not.
//!
//! One more thing about the potential is its own: it takes an `nn` in front of
//! the `-e` where every other type takes a single `n`. That is `tiennen` and not
//! `tienen`, and it is the difference between this type's `Alt::lead` and the
//! `n` its own participle takes.
//!
//! ## The passive
//!
//! The passive theme is the weak grade with its theme vowel read as `e`, which is
//! `Stems::passive_theme_of` and the default here: `tiedä` → `tiede`. From there
//! the passive is `PassiveCons::Geminated` like almost every other type — one `t`
//! in the present, `tt` in the past and the moods — and that `t` lands where the
//! type's own `d` has just been (`tiedetään` beside `tiedettiin`,
//! `tiedettäisiin`, `tiedettäneen`, `tiedettäköön`, `tiedettävä`, `tiedetty`).
//!
//! ## What the reference leaves
//!
//! Both members audit clean against `both`: 1211 reference rows each, 2422 in all,
//! nothing invented and nothing missing, where before this module the class
//! generated nothing at all. That is the first class in this family to audit clean
//! end to end — class 72 invents 12 261 rows, class 74 both invents 4110 and
//! misses 538, class 75 invents 1294 — and the reason is the type's size: two
//! lemmas cannot disagree with the reference the way `ryöpytä` and `keritä` did in
//! class 75, and there is no code in the class but `F`, whose site `Stems::weak`
//! finds inside the theme without help.
//!
//! The one row per lemma still uncovered is the same one every clean lemma in the
//! project leaves, `tietämätöntään` (and `taitamatontaan`): a `-tu` participle's
//! possessive that Kotus-Verbit spells with the theme and omorfi without it, which
//! is a disagreement between the two sources rather than anything this class does.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Alt, Stems, front, paradigm};

pub const CLASS: &str = "76";

/// The base of a class-76 verb: the lemma less its `taa`/`tää` (`tie` for
/// `tietää`, `tai` for `taitaa`).
///
/// This is not [`super::utility::ta_base`], which strips `ta`/`tä`. The
/// difference is the length of the type's own ending: classes 72 to 75 end in a
/// single `a`/`ä` (`vanheta` is `vanhe` + `ta`), and class 76 ends in `aa`/`ää`
/// (`tietää` is `tie` + `tä` + `a`), so its `ta`/`tä` is not the last two
/// characters of the lemma and `ta_base` hands the whole word back instead.
fn base_of(lemma: &str) -> &str {
    lemma
        .strip_suffix("taa")
        .or_else(|| lemma.strip_suffix("tää"))
        .unwrap_or(lemma)
}

/// Inflect a class-76 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    // The base is the lemma less its `taa`/`tää` and the theme is the `ta`/`tä`
    // inside that, so the present stem is the lemma less its own `aa`/`ää`:
    // `tie` + `tä` is `tietä`. Unlike classes 72 to 75 the theme is the whole
    // `ta`/`tä` and not just its vowel.
    let base = base_of(lemma);
    let theme = if front(lemma) { "tä" } else { "ta" };
    let present = format!("{base}{theme}");
    // The past is the bare base with the ending's `s`, and it is the one stem
    // here that does not gradate: `tiesin`, `tiesi`, `tiesivät`.
    let past = format!("{base}s");
    let mut s = Stems::with_past(lemma, &present, &past, gradation);
    // `with_past` would take the weak grade of the present stem less its theme
    // vowel, which is `tied` and not a Finnish stem, so the past's six cells are
    // handed over as they stand.
    s.past_weak = past;
    // The potential and the past active participle are both on the base, and the
    // type's row parenthesises a second spelling of each: the theme stem beside
    // them (`tiennee` beside `tietänee`, `tiennyt` beside `tietänyt`).
    //
    // The potential takes an `nn` in front of its `-e` where every other type
    // takes a single `n`, which is what makes `tiennen` rather than `tienen`;
    // the participle takes the single `n` that gives `tiennyt`, with the first of
    // the pair folded into the stem so the participle's declension and the
    // compound phrases agree on one string.
    s.pot = Alt {
        stem: base.to_string(),
        lead: "nn",
    };
    s.past_part = Alt {
        stem: format!("{base}n"),
        lead: "n",
    };
    s.alt = Some(Alt {
        stem: present,
        lead: "n",
    });
    paradigm(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotus::Gradation as Code;
    use crate::tags::{Mood, Polarity};

    /// The code every member of the type carries, `76*F`.
    const F: Option<Code> = Some(Code(6));

    fn forms(lemma: &str, code: Option<Code>) -> Vec<String> {
        inflect(lemma, code).into_iter().map(|f| f.word).collect()
    }

    fn has(lemma: &str, code: Option<Code>, word: &str) -> bool {
        forms(lemma, code).iter().any(|w| w == word)
    }

    #[test]
    fn the_base_is_the_lemma_less_its_taa() {
        // Three characters, not two: `tietää` is `tie` + `tä` + `a`, so its
        // `tä` is *not* the last two letters of the word and the shared
        // `ta_base` leaves the lemma untouched.
        assert_eq!(base_of("tietää"), "tie");
        assert_eq!(base_of("taitaa"), "tai");
        // A lemma with neither ending is handed straight back.
        assert_eq!(base_of("taita"), "taita");
    }

    #[test]
    fn the_third_person_is_on_the_theme_stem_and_the_rest_on_the_weak_grade() {
        // The theme stem repeats its own last letter for the third person,
        // which is what `paradigm`'s `pres3sg` default gives unasked...
        for (lemma, word) in [
            ("tietää", "tietää"),
            ("tietää", "tietävät"),
            ("taitaa", "taitaa"),
            ("taitaa", "taitavat"),
        ] {
            assert!(has(lemma, F, word), "missing {word}");
        }
        // ...while the first and second persons take the weak grade, `tietä`
        // with the `F` pair read the other way.
        for (lemma, word) in [
            ("tietää", "tiedän"),
            ("tietää", "tiedät"),
            ("tietää", "tiedämme"),
            ("tietää", "tiedätte"),
            ("taitaa", "taidan"),
            ("taitaa", "taidat"),
            ("taitaa", "taidamme"),
            ("taitaa", "taidatte"),
        ] {
            assert!(has(lemma, F, word), "missing {word}");
        }
    }

    #[test]
    fn the_imperative_and_the_negative_present_are_the_weak_grade() {
        assert!(has("tietää", F, "tiedä"));
        assert!(!has("tietää", F, "tietä"));
        let mut got: Vec<(Option<Mood>, Polarity)> = Vec::new();
        for f in inflect("tietää", F) {
            if f.word == "tiedä" {
                got.extend(f.readings.iter().map(|t| (t.mood, t.polarity)));
            }
        }
        assert!(
            got.contains(&(Some(Mood::Imperative), Polarity::Affirmative)),
            "the imperative's second person is the weak grade: {got:?}"
        );
        assert!(
            got.contains(&(Some(Mood::Indicative), Polarity::Negative)),
            "the negative present is the weak grade: {got:?}"
        );
    }

    #[test]
    fn the_past_does_not_gradate() {
        for word in [
            "tiesin",
            "tiesit",
            "tiesi",
            "tiesimme",
            "tiesitte",
            "tiesivät",
        ] {
            assert!(has("tietää", F, word), "missing {word}");
        }
        // Not `tied` + the past endings, which is what the shared default would
        // have given.
        assert!(!has("tietää", F, "tiedsi"));
        assert!(!has("tietää", F, "tiedin"));
    }

    #[test]
    fn the_potential_and_the_participle_are_each_spelled_twice() {
        for word in [
            "tiennee",
            "tiennevät",
            "tietänee",
            "tietänevät",
            "tiennyt",
            "tiennyttä",
            "tienneen",
            "tienneet",
            "tietänyt",
            "tietänyttä",
            "tietäneet",
        ] {
            assert!(has("tietää", F, word), "missing {word}");
        }
        // The connegatives take both spellings too, and they are phrases in
        // both cases: `en tienne` beside `en tietäne`, never a bare `tienne`.
        for word in [
            "en tienne",
            "en tietäne",
            "eivät tienne",
            "eivät tietäne",
            "eivät tienneet",
            "eivät tietäneet",
        ] {
            assert!(has("tietää", F, word), "missing {word}");
        }
        assert!(!has("tietää", F, "tienne"));
    }

    #[test]
    fn the_conditional_has_the_one_spelling_the_type_row_gives() {
        for word in ["tietäisi", "tietäisin", "tietäisimme", "tietäisivät"] {
            assert!(has("tietää", F, word), "missing {word}");
        }
        // Class 74's parenthesised alternant, which this type does not have.
        assert!(!has("tietää", F, "tietäsi"));
    }

    #[test]
    fn the_passive_is_geminated_off_the_d() {
        assert!(has("tietää", F, "tiedetään"));
        assert!(has("tietää", F, "tiedettiin"));
        assert!(has("tietää", F, "tiedettäisiin"));
        assert!(has("tietää", F, "tiedettäneen"));
        assert!(has("tietää", F, "tiedettäköön"));
        assert!(has("tietää", F, "tiedettävä"));
        assert!(has("tietää", F, "tiedetty"));
        assert!(has("tietää", F, "tiedetä"));
        // The second infinitive's passive inessive is `tiede` + `tt` + `äessä`.
        assert!(has("tietää", F, "tiedettäessä"));
    }
}
