//! Class 77 (the `kumajaa` type) inflection, written by hand.
//!
//! The type row is the shortest in `kotus-tyypit.txt` and the only one whose
//! cells are not a whole paradigm:
//!
//! ```text
//! 77 (vaillinaisesti taipuvia verbejä; vain joitakin kolmannen persoonan muotoja)
//! kumajaa
//! kumaji
//! kumajaisi
//! ```
//!
//! Three forms, and the note says why: *partially inflected verbs; only some
//! third-person forms*. This is a real fact about Finnish rather than a gap in
//! the reference. `kumajaa` "to thrum", `heläjää` "to ring" and `vipajaa`
//! "to whiz" are intransitive sound-verbs that name a single event, and Finnish
//! inflects them in the third person only: there is no `kumajan` or `kumajitte`
//! in the language, and the sanalista does not have a type for them because it
//! would have nothing else to put in it.
//!
//! **So this module does not call `utility::paradigm`,** which is the one thing
//! every other module here does. `paradigm` emits the whole type-1 paradigm, and
//! for this type almost every cell of it would be a word the engine invented:
//! the four missing present persons, the four missing past persons, the
//! imperative, the potential, the passive, the second infinitive and the `-nut`
//! and `-ttu` and `-ttava` participles, and the whole compound layer but the one
//! negative phrase the type has. What the type *does* have is a conditional in
//! all six persons (`kumajaisi`...`kumajaisivat`), which is more than the note's
//! "only some third-person forms" suggests, so the type row's three cells are a
//! sample and the reference is the inventory. The lemma list is the check:
//! three members, all `-aa`, none gradating.
//!
//! Everything the reference has is one of five things, and each has a helper in
//! [`utility`] already:
//!
//! - the present's and the past's two third persons, which are one letter each
//!   on the present stem (`kumajaa`, `kumajavat`, `kumaji`, `kumajivat`);
//! - the conditional's six persons, and the six negative phrases that take it;
//! - the third infinitive's six cases and the fifth's possessives;
//! - three participle nouns, the agent `kumajama`, the present active
//!   `kumajava` and the negative `kumajamaton`;
//! - the fourth infinitive `kumajaminen`.
//!
//! The present stem is the lemma less its own last letter, because the theme is
//! the ending's `a` and the third person repeats it: `kumajaa` is `kumaja` +
//! `a`, where `aakkostaa` is `aakkosta` + `a` as well. The past drops the theme
//! vowel -- `kumaji` beside `kumajaa` -- which is the shared
//! [`utility::Stems::past_stem`] rule, the same one that gives `aakkostin`
//! beside `aakkostaa`. Nothing here ends in `i`, so the `absorb_i` the shared
//! mood loops apply never fires, and the endings are plain concatenations.

use crate::tags::{Case, Form, Mood, Number, Person, Polarity, Tense, Voice};

use super::compound;
use super::utility::{
    ThirdPossessive, VerbBuilder, front, maton_participle, minen_infinitive, participle_tags,
    va_participle, verb_tags,
};

/// The present stem of an `aa` type-77 verb: the lemma less its own last letter.
///
/// `kumajaa` gives `kumaja`, which is the stem that takes the theme in the
/// third person (`kumaja` + `a`) and the bare `i` in the past (`kumaji`).
///
/// Public because class 78's tests need it to reach class 78's members' stems,
/// which are the same rule -- both types are `-aa`/`-ää`.
pub fn stem_of(lemma: &str) -> String {
    let n = lemma.chars().count();
    lemma.chars().take(n - 1).collect()
}

/// The third person's two cells in one mood: the singular, which takes the
/// ending given, and the plural, which takes its own `i`-initial ending.
///
/// The two moods of this type differ in which stem they run on, so both the
/// stem and the plural ending are the caller's: the present repeats the theme
/// (`kumajaa` beside `kumajavat`) while the past drops it (`kumaji` beside
/// `kumajivat`).
fn third_persons(b: &mut VerbBuilder, stem: &str, singular: &str, plural: &str, tense: Tense) {
    for (ending, number) in [(singular, Number::Singular), (plural, Number::Plural)] {
        b.push(
            format!("{stem}{ending}"),
            verb_tags(
                Some(tense),
                Some(Mood::Indicative),
                Some(Person::Third),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        );
    }
}

pub fn inflect(lemma: &str) -> Vec<Form> {
    forms(lemma, true)
}

/// The same type without its past, which is all of class 78. See
/// [`class_78`](super::class_78) for the evidence that the two types differ by
/// exactly those two cells and nothing else.
pub fn inflect_without_past(lemma: &str) -> Vec<Form> {
    forms(lemma, false)
}

/// The type's cells, with `past` saying whether its past is among them.
///
/// The two callers are the two types: class 77 has the past's two third
/// persons (`kumaji`, `kumajivat`) and class 78 has none. Everything else the
/// reference gives either type is the same cell on the same stem, which is why
/// the difference is a flag rather than a second module.
fn forms(lemma: &str, past: bool) -> Vec<Form> {
    let f = front(lemma);
    let a = if f { 'ä' } else { 'a' };
    let mut b = VerbBuilder::new(lemma);
    let e_h = b.h("ssa", "ssä").to_string();
    let st = b.h("sta", "stä").to_string();
    let ll = b.h("lla", "llä").to_string();
    let tt = b.h("tta", "ttä").to_string();
    let stem = stem_of(lemma);
    let vat = b.h("vat", "vät").to_string();

    // --- the present's two third persons ---
    // `kumajaa` is the theme repeated and `kumajavat` is the plural's own
    // ending; there is no first or second person in this type.
    third_persons(&mut b, &stem, &a.to_string(), &vat, Tense::Present);
    // The past's two, on the stem less its theme vowel -- `kumaj` beside
    // `kumaja` -- which is the shared `Stems::past_stem` rule and not a new one:
    // the past ending's own `i` would otherwise double the theme.
    if past {
        let ivat = b.h("ivat", "ivät").to_string();
        third_persons(&mut b, &stem_of(&stem), "i", &ivat, Tense::Past);
    }

    // --- the conditional, which the type has in all six persons ---
    // `-isi` on the present stem, with no absorption: the stem ends in `a`.
    for (ending, person, number) in [
        ("isin".to_string(), Person::First, Number::Singular),
        ("isit".to_string(), Person::Second, Number::Singular),
        ("isi".to_string(), Person::Third, Number::Singular),
        ("isimme".to_string(), Person::First, Number::Plural),
        ("isitte".to_string(), Person::Second, Number::Plural),
        (b.h("isivat", "isivät"), Person::Third, Number::Plural),
    ] {
        b.push(
            format!("{stem}{ending}"),
            verb_tags(
                Some(Tense::Present),
                Some(Mood::Conditional),
                Some(person),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        );
    }
    // The conditional is the only mood this type has a negative of, so it is the
    // only compound phrase it has at all: `en kumajaisi`.
    compound::conditional_connegatives(&mut b, &format!("{stem}isi"), None);

    // --- the third infinitive, `-ma` in six cases and no possessives ---
    // The `-ma` stem is the same string as the agent participle's nominative, so
    // the third infinitive and the agent share it.
    let ma = format!("{stem}m{a}");
    for (word, case) in [
        (format!("{ma}{a}n"), Case::Illative),
        (format!("{ma}{e_h}"), Case::Inessive),
        (format!("{ma}{st}"), Case::Elative),
        (format!("{ma}{ll}"), Case::Adessive),
        (format!("{ma}{tt}"), Case::Abessive),
        (format!("{ma}n"), Case::Instructive),
    ] {
        b.push(
            word,
            verb_tags(
                None,
                Some(Mood::Infinitive),
                None,
                Some(Number::Singular),
                Some(Voice::Active),
                Some(case),
                Polarity::Affirmative,
            ),
        );
    }
    // The fourth: the `-minen` nominal, declined with its own possessives.
    b.decline(&minen_infinitive(&stem, f), |case, number, poss| {
        let mut t = verb_tags(
            None,
            Some(Mood::Infinitive),
            None,
            Some(number),
            Some(Voice::Active),
            Some(case),
            Polarity::Affirmative,
        );
        t.possessive = poss;
        t
    });
    // The fifth: `-maisillä`/`-maisilla`, which harmonises throughout and takes
    // only possessives -- the bare form is not a word of the type.
    b.possess(
        format!("{stem}m{a}isill{a}"),
        ThirdPossessive::LongAndShort,
        |poss| {
            let mut t = verb_tags(
                None,
                Some(Mood::Infinitive),
                None,
                None,
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            );
            t.possessive = Some(poss);
            t
        },
    );

    // --- the three participle nouns ---
    // Present active `-va`: the plural stem in `i`, no possessive on the
    // singular partitive.
    b.decline(
        &va_participle(&format!("{stem}v{a}"), &stem, "v", true),
        |case, number, poss| {
            participle_tags(Some(Tense::Present), Voice::Active, case, number, poss)
        },
    );
    // Agent `-ma`: the `-va` shape on the third infinitive's own stem.
    b.decline(
        &va_participle(&ma, &stem, "m", true),
        |case, number, poss| participle_tags(None, Voice::Active, case, number, poss),
    );
    // Negative `-maton`, which does take the singular partitive's possessive
    // (`kumajamatontani`), the other two do not.
    let o = if f { 'ö' } else { 'o' };
    let maton = format!("{stem}m{a}t{o}n");
    b.decline(
        &maton_participle(&stem, a, f, &maton),
        |case, number, poss| participle_tags(None, Voice::Active, case, number, poss),
    );

    b.into_forms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Possessive;

    fn forms(lemma: &str) -> Vec<String> {
        inflect(lemma).into_iter().map(|f| f.word).collect()
    }

    fn has(lemma: &str, word: &str) -> bool {
        forms(lemma).iter().any(|w| w == word)
    }

    #[test]
    fn without_its_past_the_type_is_class_78s_and_differs_in_exactly_two_forms() {
        // The claim class 78's module rests on: the two types share every cell
        // but the past's two third persons. Checked here on the type's own
        // member rather than left to the class-78 tests, so that a change to
        // either module breaks the other one.
        let full: Vec<String> = inflect("kumajaa").into_iter().map(|f| f.word).collect();
        let without: Vec<String> = inflect_without_past("kumajaa")
            .into_iter()
            .map(|f| f.word)
            .collect();
        let dropped: Vec<&String> = full.iter().filter(|w| !without.contains(w)).collect();
        assert_eq!(dropped, vec!["kumaji", "kumajivat"]);
        // And nothing the shorter variant adds in their place.
        assert_eq!(without.len() + 2, full.len());
        // The present's two are in both, so the flag is on the past alone.
        assert!(without.contains(&"kumajaa".to_string()));
        assert!(without.contains(&"kumajavat".to_string()));
        // The conditional, which both types have in full.
        for word in ["kumajaisi", "kumajaisin", "kumajaisivat"] {
            assert!(without.contains(&word.to_string()), "missing {word}");
        }
    }

    #[test]
    fn the_stem_is_the_lemma_less_its_last_letter() {
        assert_eq!(stem_of("kumajaa"), "kumaja");
        assert_eq!(stem_of("heläjää"), "heläjä");
        assert_eq!(stem_of("vipajaa"), "vipaja");
        // And the past drops the theme vowel off that.
        assert_eq!(stem_of(&stem_of("kumajaa")), "kumaj");
    }

    #[test]
    fn only_the_third_persons_are_inflected() {
        for word in ["kumajaa", "kumajavat", "kumaji", "kumajivat"] {
            assert!(has("kumajaa", word), "missing {word}");
        }
        // The four persons each of the present and the past has no form of, and
        // every one of them would be invented rather than Finnish.
        for word in [
            "kumajan",
            "kumajat",
            "kumajamme",
            "kumajatte",
            "kumajain",
            "kumajait",
            "kumajaimme",
            "kumajaitte",
        ] {
            assert!(!has("kumajaa", word), "invented {word}");
        }
    }

    #[test]
    fn the_moods_the_type_does_not_have_are_absent() {
        for word in [
            // imperative
            "kumaja",
            "kumajaka",
            "kumajat",
            "kumajamme",
            "kumajakaa",
            "kumajatka",
            // potential
            "kumajane",
            "kumajanet",
            "kumajanee",
            "kumajanemme",
            "kumajanette",
            "kumajaneet",
            // passive
            "kumajetaan",
            "kumajettiin",
            "kumajettaisiin",
            // the second infinitive and the other participles
            "kumajaminen2",
            "kumajanut",
            "kumajaneet2",
            "kumajettu",
            "kumajettava",
            // and the passive instructive
            "kumajetttaman",
        ] {
            assert!(!has("kumajaa", word), "invented {word}");
        }
    }

    #[test]
    fn the_conditional_has_all_six_persons_and_is_the_only_negative() {
        for word in [
            "kumajaisi",
            "kumajaisin",
            "kumajaisit",
            "kumajaisimme",
            "kumajaisitte",
            "kumajaisivat",
        ] {
            assert!(has("kumajaa", word), "missing {word}");
        }
        // The six phrases, and nothing else in the compound layer.
        for word in [
            "en kumajaisi",
            "et kumajaisi",
            "ei kumajaisi",
            "emme kumajaisi",
            "ette kumajaisi",
            "eivät kumajaisi",
        ] {
            assert!(has("kumajaa", word), "missing {word}");
        }
        for word in [
            "en kumajaa",
            "en kumajane",
            "ei kumajettaisi",
            "on kumajanut",
            "ei kumajettu",
        ] {
            assert!(!has("kumajaa", word), "invented {word}");
        }
    }

    #[test]
    fn the_infinitives_are_the_third_the_fourth_and_the_fifth() {
        for word in [
            "kumajamaan",
            "kumajamassa",
            "kumajamasta",
            "kumajamatta",
            "kumajamalla",
            "kumajaman",
        ] {
            assert!(has("kumajaa", word), "missing {word}");
        }
        assert!(has("kumajaa", "kumajaminen"));
        assert!(has("kumajaa", "kumajamistani"));
        // The fifth takes only possessives: the bare form is not in the type.
        assert!(has("kumajaa", "kumajamaisillani"));
        assert!(has("kumajaa", "kumajamaisillansa"));
        assert!(has("kumajaa", "kumajamaisillaan"));
        assert!(!has("kumajaa", "kumajamaisilla"));
    }

    #[test]
    fn the_three_participle_nouns_are_declined() {
        // Agent, present active, negative: nominative and the oblique stems.
        for word in [
            "kumajama",
            "kumajaman",
            "kumajamassa",
            "kumajamista",
            "kumajamaa",
            "kumajamaani",
            "kumajava",
            "kumajavan",
            "kumajavassa",
            "kumajavia",
            "kumajaviani",
            "kumajamaton",
            "kumajamattoman",
            "kumajamattomassa",
            "kumajamattomia",
        ] {
            assert!(has("kumajaa", word), "missing {word}");
        }
        // The singular partitive's possessive is the negative's alone.
        assert!(has("kumajaa", "kumajamatontani"));
        assert!(!has("kumajaa", "kumajavaantani"));
        assert!(!has("kumajaa", "kumajamaantani"));
    }

    #[test]
    fn the_front_vowel_member_harmonises_throughout() {
        for word in [
            "heläjää",
            "heläjävät",
            "heläjäisi",
            "heläjäisivät",
            "heläjämään",
            "heläjämässä",
            "heläjämistä",
            "heläjämättä",
            "heläjämällä",
            "heläjämän",
            "heläjämäisilläni",
            "heläjäminen",
            "heläjävä",
            "heläjämätön",
        ] {
            assert!(has("heläjää", word), "missing {word}");
        }
        assert!(has("heläjää", "en heläjäisi"));
        assert!(has("vipajaa", "en vipajaisi"));
        assert!(has("vipajaa", "vipajamasta"));
        // The three participle nouns of the front member: the `a`/`ä` pair read
        // the other way in each of their endings, and the stem's own `ä`
        // untouched, which is the whole of what the type's harmony amounts to.
        for word in [
            "heläjämä",
            "heläjämää",
            "heläjämät",
            "heläjämien",
            "heläjävä",
            "heläjävää",
            "heläjävät",
            "heläjämätön",
            "heläjämättömät",
            "heläjämättömien",
        ] {
            assert!(has("heläjää", word), "missing {word}");
        }
    }

    #[test]
    fn the_plural_third_person_harmonises_and_the_possessive_is_third_person_long_and_short() {
        assert!(has("heläjää", "heläjävät"));
        assert!(has("vipajaa", "vipajavat"));
        // The fifth's third person doubles the theme vowel for the short
        // spelling: `heläjämäisillään` beside `heläjämäisillänsä`.
        assert!(has("heläjää", "heläjämäisillään"));
        assert!(has("heläjää", "heläjämäisillänsä"));
    }

    #[test]
    fn every_participle_carrying_reading_is_tagged_active_and_unmooded() {
        let mut saw_va = false;
        for form in inflect("kumajaa") {
            for t in &form.readings {
                if form.word == "kumajava" {
                    assert_eq!(t.voice, Some(Voice::Active));
                    assert_eq!(t.mood, None, "a participle takes no mood");
                    assert_eq!(t.tense, Some(Tense::Present));
                    saw_va = true;
                }
                if form.word == "kumajama" {
                    assert_eq!(t.mood, None, "the agent takes no tense either");
                }
            }
        }
        assert!(saw_va, "the present active participle was not emitted");
        // The possessive on a participle reading is the one field
        // `participle_tags` adds over `verb_tags`.
        let found = inflect("kumajaa")
            .into_iter()
            .filter(|f| f.word == "kumajavaani")
            .flat_map(|f| f.readings)
            .any(|t| t.possessive == Some(Possessive::FirstSingular));
        assert!(found, "kumajavaani carries no possessive reading");
    }
}
