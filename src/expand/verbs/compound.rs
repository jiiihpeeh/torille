//! The compound verbs a type-1 verb forms: `olla` plus a participle, and the
//! negative auxiliary.
//!
//! A Finnish verb has two layers. The simple layer is everything
//! [`utility::paradigm`] builds -- one word, the stem and an ending. The
//! compound layer adds a word in front: `ei` for the negative, `olla` for the
//! perfect. So a verb's full paradigm is the simple forms plus these, and the
//! parts they need are the ones the simple layer already worked out.
//!
//! The parts of `olla` that a perfect needs are fixed -- the lexicon confirms
//! `olen`, `olet`, `on`, `olemme`, `olette`, `ovat`, their past and conditional
//! twins, the potential `lienen`...`lienevät`, and the imperative `olkoon` /
//! `olkoot` -- so only the verb's own side varies, and it varies with the type:
//! a `-nut` participle for the active perfect and a `-ttu` one for the passive.
//!
//! The negative is built in every mood the reference now lists. The *active*
//! negative was once indicative-only (`en puhu`, `en puhunut`) because the
//! build that materialised the lexicon only assembled those; Omorfi does
//! generate the other connegatives (`[MOOD=COND][NEG=CON]` is `puhuisi`,
//! `[MOOD=IMPV][NEG=CON]` is `puhuko`), so `en puhuisi`, `en puhune` and
//! `älä puhu` / `älköön puhuko` are emitted here alongside the passive ones
//! (`ei puhuttaisi`, `ei puhuttane`, `älköön puhuttako`). See
//! `tools/build_verb_lexicon.py`, which puts them in the lexicon.

use crate::tags::{Mood, Number, Person, Polarity, Tense, Voice};

use super::utility::{VerbBuilder, verb_tags};

/// The pieces of a verb that the compound forms are built from.
pub struct Parts {
    /// The weak present stem, which is also the negative present: the negative
    /// drops the person's ending and leaves the stem (`en puhu`, `en aakkosta`,
    /// and `en hauko` beside `haukoo`, because the negative is not one of the
    /// third-person cells that keeps the strong grade).
    pub stem: String,
    /// The passive present stem (`puhut`). The negative present passive is this
    /// with the harmony vowel alone: `ei puhuta`, `ei kysytä`.
    pub pass_present: String,
    /// The passive connegative stem (`puhutta`), which the negative imperative
    /// takes its `-ko` from: `älköön puhuttako`, `älköön kysyttäkö`.
    pub pass_mood: String,
    /// The past active participle, singular (`puhunut`) and plural
    /// (`puhuneet`). A person of the auxiliary governs which: a singular person
    /// takes the singular participle, a plural person the plural one.
    pub done_sg: String,
    pub done_pl: String,
    /// The alternant spellings of the two above, for the type that spells its
    /// past active participle twice (`tiennyt` beside `tietänyt`). The auxiliary
    /// governs which number it takes, exactly as it does for [`Self::done_sg`]
    /// and [`Self::done_pl`], and the reference has both spellings in the
    /// phrases: `on tiennyt` beside `on tietänyt`, `eivät ole tienneet` beside
    /// `eivät ole tietäneet`.
    pub done_sg_alt: Option<String>,
    pub done_pl_alt: Option<String>,
    /// The past passive participle (`puhuttu`). It does not change by person,
    /// because the passive takes no person at all.
    pub pass_done: String,
    /// The active conditional connegative (`puhuisi`), which negates the mood:
    /// `en puhuisi`. It is the same word as the affirmative third person.
    pub cond_conneg: String,
    /// The alternant conditional connegative of a type that spells its
    /// conditional twice (`katkeisi`), which the negative auxiliary takes
    /// exactly as it takes [`Self::cond_conneg`]: `en katkeaisi` beside
    /// `en katkeisi`.
    pub cond_conneg_alt: Option<String>,
    /// The active potential connegative (`puhune`): `en puhune`.
    pub pot_conneg: String,
    /// The alternant of that, for the type that spells its potential twice
    /// (`tietäne` beside `tienne`), which the negative auxiliary takes in the
    /// same two words: `en tienne` beside `en tietäne`.
    pub pot_conneg_alt: Option<String>,
    /// The active imperative connegative (`puhuko`), which the negated
    /// imperative takes after `älä`'s plural and third persons
    /// (`älköön puhuko`). Its second person singular takes the bare stem
    /// instead (`älä puhu`).
    pub imper_conneg: String,
}

impl Parts {
    /// The past active participle of `number`, in every spelling the type has
    /// for it: the one each type writes, and the alternant of the types that
    /// write two (`tiennyt` then `tietänyt`).
    fn dones(&self, number: Number) -> impl Iterator<Item = &str> {
        let (primary, alt): (&str, Option<&str>) = match number {
            Number::Plural => (&self.done_pl, self.done_pl_alt.as_deref()),
            _ => (&self.done_sg, self.done_sg_alt.as_deref()),
        };
        std::iter::once(primary).chain(alt)
    }
}

/// The six persons of the negative auxiliary, with the participle number each
/// one governs.
const NEGATIVE: &[(&str, Person, Number)] = &[
    ("en", Person::First, Number::Singular),
    ("et", Person::Second, Number::Singular),
    ("ei", Person::Third, Number::Singular),
    ("emme", Person::First, Number::Plural),
    ("ette", Person::Second, Number::Plural),
    ("eivät", Person::Third, Number::Plural),
];

/// The persons of the negated imperative, and the auxiliary each takes. The
/// second person singular takes the bare connegative (`älä puhu`); every other
/// person takes the `-ko` one (`älkää puhuko`, `älköön puhuko`, `älkööt
/// puhuko`). There is no first person singular imperative.
const IMPERATIVE_NEG: &[(&str, Person, Number)] = &[
    ("älä", Person::Second, Number::Singular),
    ("älköön", Person::Third, Number::Singular),
    ("älkäämme", Person::First, Number::Plural),
    ("älkää", Person::Second, Number::Plural),
    ("älkööt", Person::Third, Number::Plural),
];

/// Emit the six conditional connegatives, `en puhuisi` beside `eivät puhuisi`,
/// taking the mood's own connegative and, where the type spells the mood twice,
/// the alternant's beside it: `en katkeaisi` and `en katkeisi`.
///
/// [`emit`] calls this inside the rest of the negative, but a type whose
/// reference lists *only* the conditional connegative calls it on its own.
/// Class 77 is that type: `kumajaa` has no other negative form in the lexicon,
/// because the type is only inflected in the third person and in the
/// conditional.
pub fn conditional_connegatives(
    b: &mut VerbBuilder,
    cond_conneg: &str,
    cond_conneg_alt: Option<&str>,
) {
    for (neg, person, number) in NEGATIVE {
        for conneg in std::iter::once(cond_conneg).chain(cond_conneg_alt) {
            push2(
                b,
                neg,
                conneg,
                tags(
                    Some(Tense::Present),
                    Mood::Conditional,
                    Some(*person),
                    Some(*number),
                    Voice::Active,
                    Polarity::Negative,
                ),
            );
        }
    }
}

/// Emit every compound form of the verb whose parts are `p`.
pub fn emit(b: &mut VerbBuilder, p: &Parts) {
    let a = b.h("a", "ä");

    // --- the negative of the simple present ---
    // The negative present drops the person's ending and keeps the bare stem,
    // so all six persons share one verb form: `en puhu`, `et puhu`, `ei puhu`.
    for (neg, person, number) in NEGATIVE {
        push2(
            b,
            neg,
            &p.stem,
            tags(
                Some(Tense::Present),
                Mood::Indicative,
                Some(*person),
                Some(*number),
                Voice::Active,
                Polarity::Negative,
            ),
        );
    }

    // --- the negative of the conditional ---
    // `en puhuisi`: the reference confirms both the connegative (`puhuisi` is
    // also the affirmative third person) and the phrase, so they are plain.
    conditional_connegatives(b, &p.cond_conneg, p.cond_conneg_alt.as_deref());

    // --- the negative of the potential ---
    // `en puhune`, where the connegative is Omorfi's `[MOOD=POTN][NEG=CON]`.
    for (neg, person, number) in NEGATIVE {
        for conneg in std::iter::once(&p.pot_conneg).chain(p.pot_conneg_alt.iter()) {
            push2(
                b,
                neg,
                conneg,
                tags(
                    Some(Tense::Present),
                    Mood::Potential,
                    Some(*person),
                    Some(*number),
                    Voice::Active,
                    Polarity::Negative,
                ),
            );
        }
    }

    // --- the negative imperative ---
    // `älä puhu`, `älköön puhuko`: Omorfi generates the active imperative
    // connegative (`[MOOD=IMPV][NEG=CON]` is `puhuko`) and the reference has
    // the phrase, so these are plain too. The second person singular keeps the
    // bare present connegative where every other person takes the `-ko` one.
    for (neg, person, number) in IMPERATIVE_NEG {
        let conneg = if *person == Person::Second && *number == Number::Singular {
            &p.stem
        } else {
            &p.imper_conneg
        };
        push2(
            b,
            neg,
            conneg,
            tags(
                Some(Tense::Present),
                Mood::Imperative,
                Some(*person),
                Some(*number),
                Voice::Active,
                Polarity::Negative,
            ),
        );
    }

    // --- the negative of the participles, on their own ---
    // `ei puhunut` negates the past active participle in any person, and the
    // plural takes the plural participle (`ei puhuneet`).
    for (neg, person, number) in NEGATIVE {
        for done in p.dones(*number) {
            push2(
                b,
                neg,
                done,
                tags(
                    Some(Tense::Past),
                    Mood::Indicative,
                    Some(*person),
                    Some(*number),
                    Voice::Active,
                    Polarity::Negative,
                ),
            );
        }
    }
    // The passive participle has no person, so only the third person negates it:
    // `ei puhuttu`.
    push2(
        b,
        "ei",
        &p.pass_done,
        tags(
            Some(Tense::Past),
            Mood::Indicative,
            None,
            None,
            Voice::Passive,
            Polarity::Negative,
        ),
    );

    // --- the negative of the passive ---
    // Its present is the passive stem with the bare theme vowel (`puhuta`), and
    // its conditional and potential are the connegative stem's (`puhuttaisi`,
    // `puhuttane`).
    for (word, tense, mood) in [
        (
            format!("{}{a}", p.pass_present),
            Tense::Present,
            Mood::Indicative,
        ),
        (
            format!("{}isi", p.pass_mood),
            Tense::Present,
            Mood::Conditional,
        ),
        (
            format!("{}ne", p.pass_mood),
            Tense::Present,
            Mood::Potential,
        ),
    ] {
        push2(
            b,
            "ei",
            &word,
            tags(
                Some(tense),
                mood,
                None,
                None,
                Voice::Passive,
                Polarity::Negative,
            ),
        );
    }
    // `älköön puhuttako`, `älköön kysyttäkö`: the negative imperative keeps the
    // connegative stem, which already ends in the harmony vowel, so only the
    // `o` harmonises.
    let o = b.h("o", "ö");
    push2(
        b,
        "älköön",
        &format!("{}k{o}", p.pass_mood),
        tags(
            Some(Tense::Present),
            Mood::Imperative,
            None,
            None,
            Voice::Passive,
            Polarity::Negative,
        ),
    );

    // --- the perfect: `olla` plus the past active participle ---
    for (aux, tense, mood) in perfects() {
        for (word, person, number) in aux {
            for done in p.dones(*number) {
                push2(
                    b,
                    word,
                    done,
                    tags(
                        Some(tense),
                        mood,
                        Some(*person),
                        Some(*number),
                        Voice::Active,
                        Polarity::Affirmative,
                    ),
                );
            }
        }
    }

    // --- the perfect of the passive: `olla` plus `-ttu` ---
    // The passive perfect has no person, so each auxiliary appears once, in the
    // third person only. The potential has two spellings, `lienee` and the stem
    // form `lie`; the lexicon lists both.
    for (aux, tense, mood) in [
        ("on", Tense::Perfect, Mood::Indicative),
        ("oli", Tense::Pluperfect, Mood::Indicative),
        ("olisi", Tense::Perfect, Mood::Conditional),
        ("lienee", Tense::Perfect, Mood::Potential),
        ("lie", Tense::Perfect, Mood::Potential),
        ("olkoon", Tense::Perfect, Mood::Imperative),
    ] {
        push2(
            b,
            aux,
            &p.pass_done,
            tags(
                Some(tense),
                mood,
                None,
                None,
                Voice::Passive,
                Polarity::Affirmative,
            ),
        );
    }
    // The third-person singular potential of the active perfect likewise appears
    // with both spellings: `lienee puhunut` and `lie puhunut`.
    for aux in ["lienee", "lie"] {
        for done in p.dones(Number::Singular) {
            push2(
                b,
                aux,
                done,
                tags(
                    Some(Tense::Perfect),
                    Mood::Potential,
                    Some(Person::Third),
                    Some(Number::Singular),
                    Voice::Active,
                    Polarity::Affirmative,
                ),
            );
        }
    }

    // --- the negative of the perfect ---
    // Negating the auxiliary rather than the participle: `en ole puhunut` in
    // the present, `en ollut puhunut` in the past -- where the plural takes
    // `olleet`, the negated `olla`'s own plural.
    for (neg, person, number) in NEGATIVE {
        for done in p.dones(*number) {
            push2(
                b,
                // The negated present of `olla` is `ole` in both halves, unlike
                // the verb's own vowel harmony.
                &format!("{neg} ole"),
                done,
                tags(
                    Some(Tense::Perfect),
                    Mood::Indicative,
                    Some(*person),
                    Some(*number),
                    Voice::Active,
                    Polarity::Negative,
                ),
            );
        }
        let past_aux = match number {
            Number::Plural => "olleet",
            _ => "ollut",
        };
        for done in p.dones(*number) {
            push2(
                b,
                &format!("{neg} {past_aux}"),
                done,
                tags(
                    Some(Tense::Pluperfect),
                    Mood::Indicative,
                    Some(*person),
                    Some(*number),
                    Voice::Active,
                    Polarity::Negative,
                ),
            );
        }
    }
    // The passive perfect's negative is third person only, and the potential
    // has both spellings here too (`ei lie puhuttu`, `ei liene puhuttu`).
    for (aux, tense, mood) in [
        ("ole", Tense::Perfect, Mood::Indicative),
        ("ollut", Tense::Pluperfect, Mood::Indicative),
        ("olisi", Tense::Perfect, Mood::Conditional),
        ("lie", Tense::Perfect, Mood::Potential),
        ("liene", Tense::Perfect, Mood::Potential),
    ] {
        push2(
            b,
            &format!("ei {aux}"),
            &p.pass_done,
            tags(
                Some(tense),
                mood,
                None,
                None,
                Voice::Passive,
                Polarity::Negative,
            ),
        );
    }
    // `älköön olko puhuttu`: the negated imperative's auxiliary is the bare
    // infinitive, as in `älköön olko`.
    push2(
        b,
        "älköön olko",
        &p.pass_done,
        tags(
            Some(Tense::Perfect),
            Mood::Imperative,
            None,
            None,
            Voice::Passive,
            Polarity::Negative,
        ),
    );
}

/// One spelling of `olla` and the person and number it serves.
type Aux = (&'static str, Person, Number);

/// One tense's perfects: the `olla` forms that go with them and the mood the
/// whole set is in.
type Auxiliaries = (&'static [Aux], Tense, Mood);

/// The auxiliary forms a perfect takes: each tense's own `olla`, with the
/// person and number each spelling governs. Only the singular and plural
/// participles are distinguished, so the auxiliary is listed once per person.
fn perfects() -> [Auxiliaries; 5] {
    use Number::{Plural as Pl, Singular as Sg};
    use Person::{First as F1, Second as S2, Third as T3};
    [
        (
            &[
                ("olen", F1, Sg),
                ("olet", S2, Sg),
                ("on", T3, Sg),
                ("olemme", F1, Pl),
                ("olette", S2, Pl),
                ("ovat", T3, Pl),
            ],
            Tense::Perfect,
            Mood::Indicative,
        ),
        (
            &[
                ("olin", F1, Sg),
                ("olit", S2, Sg),
                ("oli", T3, Sg),
                ("olimme", F1, Pl),
                ("olitte", S2, Pl),
                ("olivat", T3, Pl),
            ],
            Tense::Pluperfect,
            Mood::Indicative,
        ),
        (
            &[
                ("olisin", F1, Sg),
                ("olisit", S2, Sg),
                ("olisi", T3, Sg),
                ("olisimme", F1, Pl),
                ("olisitte", S2, Pl),
                ("olisivat", T3, Pl),
            ],
            Tense::Perfect,
            Mood::Conditional,
        ),
        (
            &[
                ("lienen", F1, Sg),
                ("lienet", S2, Sg),
                ("lienee", T3, Sg),
                ("lienemme", F1, Pl),
                ("lienette", S2, Pl),
                ("lienevät", T3, Pl),
            ],
            Tense::Perfect,
            Mood::Potential,
        ),
        (
            &[("olkoon", T3, Sg), ("olkoot", T3, Pl)],
            Tense::Perfect,
            Mood::Imperative,
        ),
    ]
}

fn tags(
    tense: Option<Tense>,
    mood: Mood,
    person: Option<Person>,
    number: Option<Number>,
    voice: Voice,
    polarity: Polarity,
) -> crate::tags::Tags {
    verb_tags(
        tense,
        Some(mood),
        person,
        number,
        Some(voice),
        None,
        polarity,
    )
}

/// Push a compound form: the auxiliary, a space, and the verb's own word.
fn push2(b: &mut VerbBuilder, head: &str, tail: &str, tags: crate::tags::Tags) {
    b.push(format!("{head} {tail}"), tags);
}
