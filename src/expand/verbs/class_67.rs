//! Class 67 (the `tulla` type) inflection, written by hand.
//!
//! Class 67 is the `-lla`/`-llä` type, and it is the first type whose two stems
//! are the *same* stem in two grades and whose present and past endings both sit
//! directly on it. Five things part it from type 1, and each is visible in one
//! line of the sanalista's type row:
//!
//! ```text
//! tulla / tulen / tulin / tulisi / tullee / tulkaa / tullut / tullaan
//! ```
//!
//! * **The present stem is the past stem with the theme put back.** `tul` +
//!   `e` is `tule`, and that is the whole present stem: `tulen`, `tulet`,
//!   `tulee`, `tulemme`, `tulet­te`, `tulevat`. The theme `e` is the one letter
//!   `pres3sg` repeats, which is why `with_past` needs no override for the third
//!   person. The past drops it again and runs on `tul` itself: `tulin`, `tulit`,
//!   `tuli`, `tulimme`, `tulitte`, `tulivat`.
//!
//! * **The potential and the past active participle front a consonant of the
//!   class's own.** `tullee` is `tul` + `l` + `ee` where type 1 has `-nee`, and
//!   `tullut` is `tul` + `l` + `ut` where type 1 has `sanonut`. That is the
//!   [`Alt`] the class already has, one value for both cells.
//!
//! * **The imperative's plurals, the passive, the second infinitive and the
//!   potential run on the lemma's own grade; the finite present, the past, the
//!   conditional, the imperative's second person and the agent run on the other
//!   one.** This is the reverse of class 66's arrangement and the reason the
//!   module overrides six of [`Stems`]' fields rather than passing a code:
//!   `imp_stem` is the lemma stem (`tulkaa`, not `tulekaa`), `p_theme` is it as
//!   well, and `inf2_stem` is it with the class's own consonant in front of the
//!   connective (`tullessa`, `tullen`, not `tulessa`).
//!
//! * **The passive adds nothing in the present.** Its theme already ends in a
//!   consonant, and the present gives that consonant back where every other type
//!   adds a `t` of its own: `tullaan`, `aaltoillaan`, `leikellään`. Only the past
//!   takes the ordinary `t` (`tultiin`, `aaltoiltiin`). That is
//!   [`PassiveCons::ThemeEnd`], a shape none of classes 62 to 66 has.
//!
//! * **The past passive participle's oblique is the past active one.** Its
//!   nominative is `tulu`'s `-tu` shape with the `t` in front of the final
//!   vowel (`tultu`, `aaltoiltu`, `leikelty`), but its oblique carries the
//!   class's consonant instead and reads `tullu` (`tullun`, `tullussa`, `tullut`
//!   for the nominative plural), the same stem as `tul` + `l` + `ut`. Nothing in
//!   `tultu` points at that `l`, so this is the one type whose participle oblique
//!   cannot be read off its nominative, and the class hands the stem over
//!   through [`Stems::p_tu_oblique`].
//!
//! The negated cells all follow: the present is the imperative's second person
//! (`en tule`), the past quotes the `-llut` participle whole (`en tullut`), and
//! the passive present is the theme with the ending's own vowel (`ei tulla`,
//! `ei leikellä`).
//!
//! **On the consonant.** Four of the cells above take a consonant, and all four
//! read it off the lemma the same way: every member's infinitive ends in
//! `<stem><consonant><a|ä>`, and the consonant is the letter in front of the
//! infinitive's `a`/`ä`. `tulla` gives `tul` and `l`, `panna` gives `pan` and `n`,
//! `purra` gives `pur` and `r`, `pierrä` gives `pier` and `r`. That single letter
//! is the whole of the difference between the class's four shapes, and
//! [`base_of`] is where it is read.
//!
//! **On the gradation.** The alternation never runs in one fixed direction, so
//! the module cannot read it off the lemma either: the sanalista writes some
//! members in the weak grade (`nakella`, `leikellä`, `määritellä`) and others in
//! the strong one (`ommella`, `annella`, `askarrella`), and in every case the
//! finite present, the past, the conditional, the imperative's second person and
//! the agent take the *other* grade from the lemma while the potential, the
//! imperative's plurals, the whole passive, the second infinitive and the
//! participles take the lemma's own. So the module hands [`Stems`] no code at
//! all -- `with_past(..., None)` leaves [`Stems::weak`] equal to the stem, which
//! puts the present, the imperative's second person and the negated present on
//! the other grade -- and computes that grade itself with [`Gradation::strongen`]
//! on the lemma's stem.
//!
//! Two codes need more than that. `D` is the gradation that *drops* the
//! consonant (`reikä` : `reiän`), and here the lemma is in the dropped grade and
//! the other one has to be put back, which is an insertion rather than a
//! substitution with no site left to work from; `jaella` is the class's only
//! member with that code, so [`strong_base_of`] writes its strong grade out. And
//! `C` is coded on 398 members of the class, of which [`gradation_of`] drops it
//! for two: `köpötellä` and `narskutella` carry `67*C` in the sanalista but no
//! `both` row either of them has shows the `tt` the code calls for.
//!
//! **What is left.** 1378 of the class's 1379 members are exact against the
//! reference's `both` rows, and the one left is not this type's — the
//! sanalista's own index is what files it under 67, and `data/kotus-tyypit.txt`
//! has a type row for it. `panna` is a noun there as well (*kaksi pannaa*), so
//! the engine inflects it in both parts of speech: every one of its 1182 verb
//! forms is exact, and the 152 noun forms beside them have no rows in a table
//! that is about verbs, which is the artifact class 52's and 53's counts are
//! made of.
//!
//! `olla` is the irregular, and its two unreachable corners are written out in
//! [`fix_olla`]: `on` beside `olen` and `ovat` beside `olevat`, which no
//! present stem produces at once, and the auxiliary `lie`'s own potential
//! (`lienen`, `lienee`, `lienevät`, `lie`), which the reference files under
//! `olla`. Its potential runs through `lie` (`Alt::n("lie")`), and the cells no
//! stem reaches, with `lie` and its negative `en lie`, are added there. Its 5th
//! infinitive possessives (`olemaisillamme`) and its past active participle's
//! comitative (`olleine`) are gaps in Omorfi's own lexicon, not the engine's,
//! and the reference has no rows for them.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::{Confidence, Form, Mood, Number, Person, Polarity, Tense, Voice};

use super::utility::{Alt, PassiveCons, Stems, front, paradigm, verb_tags};

pub const CLASS: &str = "67";

/// The stem proper of a class-67 verb, and the consonant its cells front.
///
/// Every one of the class's lemmas ends in `<stem><consonant><a|ä>`, where the
/// consonant is the one the class's own endings supply: `tul-la` gives `tul` and
/// `l`, `pan-na` and `men-nä` give `pan` and `men` with `n`, `pur-ra` and
/// `pier-rä` give `pur` and `pier` with `r`. Where the stem ends in the
/// consonant already (`tul`, `pur`, `pier`) the infinitive writes it once and the
/// endings have nothing to add; where it does not (`pan`, `men`) the endings
/// supply it instead, which is the whole difference between `pannen` and
/// `panen`.
///
/// The consonant is worth carrying along for that reason: it is the one fact
/// that tells `tulla` from `panna` or `purra`, and it is what
/// [`Alt::lead`], [`PassiveCons::ThemeEnd`], [`Stems::p_tu_oblique`] and
/// [`Stems::inf2_stem`] are all built from.
///
/// A lemma that does not end in `a`/`ä` is one the sanalista has filed here from
/// elsewhere, and it keeps the `-lla` reading of itself whole.
fn base_of(lemma: &str) -> (&str, char) {
    if !lemma.ends_with(['a', 'ä']) {
        return (lemma, 'l');
    }
    let Some((at, _)) = lemma.char_indices().nth_back(1) else {
        return (lemma, 'l');
    };
    match lemma[at..].chars().next() {
        Some(cons) => (&lemma[..at], cons),
        None => (lemma, 'l'),
    }
}

/// The gradation code the module actually applies, which is the sanalista's own
/// for every member but two.
///
/// `köpötellä` and `narskutella` are written `67*C`, and `C` is `tt <-> t`, but
/// every cell the reference records for either of them is on the lemma's own
/// single `t` -- `köpötelkää`, `ei köpötellä`, `ei köpöteltäisi`, `narskutelkoön`
/// -- and not one of the 1294 rows either of them has carries the `tt` the code
/// calls for. They are the only two of the class's 398 `C` members that come out
/// this way, and nothing in the lemma separates them from `määritellä`, whose
/// `t` does geminate. So the code is dropped for these two and the module reads
/// them as ungraduated, which is what the reference does.
fn gradation_of(lemma: &str, gradation: Option<Gradation>) -> Option<Gradation> {
    match lemma {
        "köpötellä" | "narskutella" => None,
        _ => gradation,
    }
}

/// The strong grade of a class-67 stem: the grade the finite present, the past,
/// the conditional, the imperative's second person and the agent run on, and the
/// one the sanalista lemma does not show.
///
/// The lemma's grade is not a fixed one -- `nakella` and `määritellä` write the
/// weak, `ommella` and `askarrella` the strong -- so the alternation is always
/// run from the lemma's own grade to the other, whichever way the sanalista
/// chose. [`Gradation::strongen`] does exactly that, and a lemma with no code is
/// its own strong grade.
fn strong_base_of(base: &str, gradation: Option<Gradation>) -> String {
    match gradation {
        // `D` drops its consonant rather than changing it, so there is no weak
        // site in a stem written in the dropped grade to work from, and putting
        // the `k` back is an insertion rather than a substitution. `jaella` is
        // the class's only member with this code.
        Some(Gradation(4)) if base == "jael" => "jakel".to_string(),
        Some(g) => g.strongen(base),
        None => base.to_string(),
    }
}

/// Inflect a class-67 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let (base, cons) = base_of(lemma);
    // `Alt::lead` wants the consonant by name, so that a member with an `n` or
    // an `r` in front of it spells that and not the `l` its `-lla` neighbours use.
    let cons_name: &'static str = match cons {
        'n' => "n",
        'r' => "r",
        _ => "l",
    };
    let strong_base = strong_base_of(base, gradation_of(lemma, gradation));
    // The present keeps the theme `e`, the past drops it: `tule` beside `tul`,
    // `pane` beside `pan`.
    let present = format!("{strong_base}e");
    let mut stems = Stems::with_past(lemma, &present, &strong_base, None);
    // `pres3sg` is already the `e`, being the present stem's own last letter, so
    // the third person is `tulee` with no override.
    //
    // The potential and the past participle are the cells that front the class's
    // own consonant, and the imperative's plurals, the passive theme and the
    // second infinitive are the cells that stay on the lemma's own grade.
    stems.pot = if lemma == "olla" {
        // `olla`'s potential is the auxiliary `lie`'s, not the class's:
        // `lienen`, not `ollee`. `lie` + `n` is the potential's stem and `lie`
        // is its own third person, so `Alt::n("lie")` reaches both the mood
        // (`lienen`...`lienevät`) and the negative's connegative (`liene`).
        Alt::n("lie")
    } else {
        Alt {
            stem: base.to_string(),
            lead: cons_name,
        }
    };
    stems.past_part = Alt {
        stem: base.to_string(),
        lead: cons_name,
    };
    stems.imp_stem = base.to_string();
    stems.p_theme = base.to_string();
    // The second infinitive is the one other cell that fronts the consonant:
    // `tulle`ssa, `panne`ssa and `purre`ssa.
    stems.inf2_stem = format!("{base}{cons}e");
    // The conditional follows the present's grade and drops the theme:
    // `tulisi` beside the present's `tule`, `panisi` beside `pane`.
    stems.cond_stem = strong_base.clone();
    // The passive adds nothing in the present -- its theme already ends in the
    // class's own consonant, which the ending gives back -- and the ordinary `t`
    // in the past.
    stems.p_cons = PassiveCons::ThemeEnd;
    // The past passive participle's oblique is the past active one's: `tul` + `l`
    // + `u` (and `pan` + `n` + `u`), where its nominative has the `t` instead
    // (`tultu`, `pantu`).
    stems.p_tu_oblique = Some(format!(
        "{base}{cons}{}",
        if front(lemma) { 'y' } else { 'u' }
    ));
    let mut forms = paradigm(&stems);
    if lemma == "olla" {
        fix_olla(&mut forms);
    }
    forms
}

/// The two cells of `olla` no stem reaches, and the auxiliary `lie`.
///
/// The class's regular type gives `olen`/`olet` and the whole past correctly,
/// but its third persons are `olee`/`olevat` where `olla` has `on`/`ovat` --
/// no choice of present stem produces both the `olen` of the first person and
/// the `on` of the third -- so those two forms are dropped and written here.
/// The potential is reached through the `lie` stem (see [`inflect`]), and the
/// auxiliary `lie` itself, with its negative `en lie`, is filed under `olla`
/// by the reference, so it is added too.
fn fix_olla(forms: &mut Vec<Form>) {
    let plain = |word: String, tags| Form::new(word, tags).with(Confidence::Plain);
    // The regular present third persons are `olee` and `olevat`; the passive
    // present (`ollaan`) and the `olevat` reading of the present active
    // participle have no active third person and are kept.
    forms.retain(|f| {
        !f.readings.iter().any(|t| {
            t.mood == Some(Mood::Indicative)
                && t.tense == Some(Tense::Present)
                && t.person == Some(Person::Third)
                && t.voice == Some(Voice::Active)
                && t.polarity == Polarity::Affirmative
        })
    });
    for (word, number) in [("on", Number::Singular), ("ovat", Number::Plural)] {
        forms.push(plain(
            word.to_string(),
            verb_tags(
                Some(Tense::Present),
                Some(Mood::Indicative),
                Some(Person::Third),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        ));
    }
    forms.push(plain(
        "lie".to_string(),
        verb_tags(
            None,
            Some(Mood::Potential),
            Some(Person::Third),
            Some(Number::Singular),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    ));
    // The potential's plural third person harmonises with `lie` and not with
    // `olla`: `lienevät`, not `lienevat`. Only the simple mood is affected --
    // the perfect compounds carry a `Perfect` tense.
    forms.retain(|f| {
        !f.readings.iter().any(|t| {
            t.mood == Some(Mood::Potential)
                && t.tense == Some(Tense::Present)
                && t.person == Some(Person::Third)
                && t.number == Some(Number::Plural)
                && t.polarity == Polarity::Affirmative
        })
    });
    forms.push(plain(
        "lienevät".to_string(),
        verb_tags(
            None,
            Some(Mood::Potential),
            Some(Person::Third),
            Some(Number::Plural),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    ));
    // `liene` is the potential's connegative, and it is also used on its
    // own (`liene parasta` = "it is probably best"), so it is a word in
    // its own right and not only part of `ei liene`.
    forms.push(plain(
        "liene".to_string(),
        verb_tags(
            None,
            Some(Mood::Potential),
            None,
            None,
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    ));
    // The negative potential has both connegatives, `liene` (the mood's own)
    // and `lie`; the `lie` one is added here.
    for (neg, person, number) in [
        ("en", Person::First, Number::Singular),
        ("et", Person::Second, Number::Singular),
        ("ei", Person::Third, Number::Singular),
        ("emme", Person::First, Number::Plural),
        ("ette", Person::Second, Number::Plural),
        ("eivät", Person::Third, Number::Plural),
    ] {
        forms.push(plain(
            format!("{neg} lie"),
            verb_tags(
                Some(Tense::Present),
                Some(Mood::Potential),
                Some(person),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Negative,
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether the paradigm of `lemma` (inflected with `gradation`) has `form`.
    fn has(lemma: &str, gradation: Option<Gradation>, form: &str) -> bool {
        inflect(lemma, gradation).iter().any(|f| f.word == form)
    }

    /// Assert that a member has every one of `wants`.
    fn has_all(lemma: &str, gradation: Option<Gradation>, wants: &[String]) {
        for want in wants {
            assert!(has(lemma, gradation, want), "{lemma} should have {want}");
        }
    }

    /// Assert that a member has none of `absents`.
    fn has_none(lemma: &str, gradation: Option<Gradation>, absents: &[String]) {
        for absent in absents {
            assert!(
                !has(lemma, gradation, absent),
                "{lemma} should not have {absent}"
            );
        }
    }

    /// The three stems a member's expectations are built from: `b` is the grade
    /// the lemma shows, which the imperative's plurals, the passive and the
    /// second infinitive run on; `sb` is the other grade, which the finite
    /// present, the past and the conditional run on; `sp` is that with the
    /// type's theme `e` put back.
    fn stems(lemma: &str, gradation: Option<Gradation>) -> (String, String, String) {
        let (b, _) = base_of(lemma);
        let sb = strong_base_of(b, gradation);
        (b.to_string(), format!("{sb}e"), sb)
    }

    /// The harmony of the word, so that a front member's expectations are spelled
    /// by the same rule the paradigm uses.
    fn aou(lemma: &str) -> (char, char, char) {
        if front(lemma) {
            ('ä', 'ö', 'y')
        } else {
            ('a', 'o', 'u')
        }
    }

    /// The inessive's own `-ssa`, which follows the word's harmony even though the
    /// second infinitive's connective before it is a plain `e` in both (`tullessa`
    /// beside `elellessä`).
    fn ess(lemma: &str) -> &'static str {
        if front(lemma) { "ssä" } else { "ssa" }
    }

    /// The present stem carries the theme and the past stem does not, so the two
    /// are one letter apart and the endings sit straight on them.
    #[test]
    fn the_present_keeps_the_theme_and_the_past_drops_it() {
        let (b, sp, sb) = stems("tulla", None);
        let (a, _, _) = aou("tulla");
        assert_eq!(b, "tul");
        assert_eq!(sb, "tul");
        assert_eq!(sp, "tule");
        assert_eq!(base_of("elellä"), ("elel", 'l'));
        assert_eq!(base_of("panna"), ("pan", 'n'));
        assert_eq!(base_of("mennä"), ("men", 'n'));
        assert_eq!(base_of("purra"), ("pur", 'r'));
        assert_eq!(base_of("pierrä"), ("pier", 'r'));
        has_all(
            "tulla",
            None,
            &[
                format!("{sp}n"),
                format!("{sp}t"),
                format!("{sp}e"),
                format!("{sp}mme"),
                format!("{sp}tte"),
                format!("{sp}v{a}t"),
                format!("{sb}in"),
                format!("{sb}it"),
                format!("{sb}i"),
                format!("{sb}imme"),
                format!("{sb}itte"),
                format!("{sb}iv{a}t"),
            ],
        );
        // The theme is written once, not twice, and the past does not keep it.
        has_none(
            "tulla",
            None,
            &[
                format!("{sp}ee"),
                format!("{sp}vi"),
                format!("{sb}ii"),
                format!("{sb}iii"),
            ],
        );
        let (_, hsp, hsb) = stems("elellä", None);
        let (ha, _, _) = aou("elellä");
        has_all(
            "elellä",
            None,
            &[
                format!("{hsp}n"),
                format!("{hsp}e"),
                format!("{hsp}v{ha}t"),
                format!("{hsb}i"),
                format!("{hsb}iv{ha}t"),
            ],
        );
        has_none(
            "elellä",
            None,
            &[format!("{hsp}ee"), format!("{hsp}v{ha}{ha}t")],
        );
    }

    /// The potential and the past participle front the infinitive's own `l`,
    /// which is what makes the class's `-llut` and `-llen` where type 1 has
    /// `-nut` and `-nen`.
    #[test]
    fn the_potential_and_the_past_participle_front_the_l() {
        let (b, _, _) = stems("tulla", None);
        let (a, _, _) = aou("tulla");
        has_all(
            "tulla",
            None,
            &[
                format!("{b}len"),
                format!("{b}let"),
                format!("{b}lee"),
                format!("{b}lemme"),
                format!("{b}lette"),
                format!("{b}lev{a}t"),
                format!("{b}lut"),
                format!("{b}leet"),
                format!("{b}leen"),
                format!("{b}leiden"),
                format!("{b}leessa"),
                format!("{b}leeseen"),
            ],
        );
        // Type 1's `-nen` potential and its `-nut` participle are not words here,
        // and neither is the present stem with a `l` in front of it.
        has_none(
            "tulla",
            None,
            &[
                format!("{b}nen"),
                format!("{b}nut"),
                format!("{b}neet"),
                format!("tule{b}nee"),
            ],
        );
        let (hb, _, _) = stems("elellä", None);
        let (ha, _, _) = aou("elellä");
        has_all(
            "elellä",
            None,
            &[
                format!("{hb}len"),
                format!("{hb}lev{ha}t"),
                format!("{hb}lyt"),
                format!("{hb}leet"),
                format!("{hb}len"),
            ],
        );
        has_none("elellä", None, &[format!("{hb}nen"), format!("{hb}nyt")]);
    }

    /// The passive theme is the lemma's grade and the present adds no consonant
    /// of its own to it, repeating the `l` the theme already ends in. The past
    /// takes the ordinary `t`, and the past passive participle's oblique is the
    /// past active one's rather than its own nominative's.
    #[test]
    fn the_passive_repeats_the_stems_l_in_the_present() {
        let (b, _, _) = stems("tulla", None);
        let (a, o, u) = aou("tulla");
        has_all(
            "tulla",
            None,
            &[
                // Present: the theme's own `l`, and no `t`.
                format!("{b}l{a}{a}n"),
                format!("ei {b}l{a}"),
                // Past and the moods: the ordinary `t`.
                format!("{b}tiin"),
                format!("{b}t{a}isiin"),
                format!("{b}t{a}neen"),
                format!("{b}t{a}k{o}{o}n"),
                format!("{b}t{a}v{a}"),
                format!("{b}t{a}en"),
                format!("{b}t{a}essa"),
                // The participle's nominative has the `t`; every other cell that
                // is not on the nominative has the class's `l`.
                format!("{b}t{u}"),
                format!("{b}t{u}a"),
                format!("{b}t{u}un"),
                format!("{b}t{u}na"),
                format!("{b}t{u}jen"),
                format!("{b}l{u}n"),
                format!("{b}l{u}t"),
                format!("{b}l{u}ssa"),
                format!("{b}l{u}lla"),
                format!("{b}l{u}lle"),
                format!("{b}l{u}lta"),
                format!("{b}l{u}sta"),
                format!("{b}l{u}ksi"),
                format!("{b}l{u}in"),
                format!("{b}l{u}itta"),
            ],
        );
        // Neither the geminated present of every other type nor a deranked
        // participle is a word here, and the oblique is not read off the
        // nominative either.
        has_none(
            "tulla",
            None,
            &[
                format!("{b}t{a}{a}n"),
                format!("{b}ttiin"),
                format!("{b}tt{a}v{a}"),
                format!("{b}tt{u}"),
                format!("{b}t{u}nen"),
                format!("{b}t{u}ssa"),
                format!("{b}t{u}lla"),
                format!("{b}ldun"),
            ],
        );
        let (hb, _, _) = stems("leikellä", None);
        let (ha, _, hu) = aou("leikellä");
        has_all(
            "leikellä",
            None,
            &[
                format!("{hb}l{ha}{ha}n"),
                format!("{hb}tiin"),
                format!("{hb}t{ha}v{ha}"),
                format!("{hb}t{hu}"),
                format!("{hb}l{hu}n"),
                format!("{hb}l{hu}t"),
                format!("{hb}l{hu}ss{ha}"),
                format!("{hb}l{hu}ll{ha}"),
            ],
        );
        has_none(
            "leikellä",
            None,
            &[
                format!("{hb}t{ha}{ha}n"),
                format!("{hb}ttiin"),
                format!("{hb}t{hu}ss{ha}"),
            ],
        );
    }

    /// The imperative's plurals, the second infinitive and the negated present
    /// split the two stems: the plurals and the second infinitive stay on the
    /// lemma's grade while the imperative's second person and the negated present
    /// are the present stem, theme and all.
    #[test]
    fn the_imperative_plurals_and_the_second_infinitive_stay_on_the_lemma() {
        let (b, sp, _) = stems("tulla", None);
        let (a, o, _) = aou("tulla");
        has_all(
            "tulla",
            None,
            &[
                format!("{b}k{a}{a}"),
                format!("{b}k{a}{a}mme"),
                format!("{b}k{o}{o}n"),
                format!("{b}k{o}{o}t"),
                format!("{b}lessa"),
                format!("{b}len"),
                // The imperative's second person singular is the present stem,
                // and the negated present quotes it.
                sp.clone(),
                format!("en {sp}"),
                // The conditional follows the past's grade, theme dropped.
                format!("{b}isi"),
                format!("{b}isimme"),
                format!("{b}isivat"),
            ],
        );
        // The present stem is not a word before the imperative's `k`, nor is a
        // single `e` the second infinitive's connective.
        has_none(
            "tulla",
            None,
            &[
                format!("{sp}k{a}{a}"),
                format!("{sp}k{o}{o}n"),
                format!("{sp}essa"),
                format!("{b}essa"),
                format!("{b}isen"),
            ],
        );
        let (hb, hsp, _) = stems("elellä", None);
        let (ha, ho, _) = aou("elellä");
        has_all(
            "elellä",
            None,
            &[
                format!("{hb}k{ha}{ha}"),
                format!("{hb}k{ho}{ho}n"),
                format!("{hb}less{ha}"),
                format!("{hb}len"),
                hsp.clone(),
                format!("en {hsp}"),
                format!("{hb}isi"),
            ],
        );
        has_none(
            "elellä",
            None,
            &[format!("{hsp}k{ha}{ha}"), format!("{hb}ess{ha}")],
        );
    }

    /// The class's four shapes -- the `-lla` type, whose stem already ends in the
    /// consonant the cells front, and the `-nna` and `-rra` types, whose stem
    /// does not -- differ in nothing else, and the consonant is the only fact a
    /// cell needs that the lemma does not already show.
    #[test]
    fn the_consonant_the_stem_does_not_end_in_is_the_one_the_cells_front() {
        for (lemma, b, cons, a, o, u, hs) in [
            ("tulla", "tul", 'l', 'a', 'o', 'u', "ssa"),
            ("panna", "pan", 'n', 'a', 'o', 'u', "ssa"),
            ("purra", "pur", 'r', 'a', 'o', 'u', "ssa"),
            ("pierrä", "pier", 'r', 'ä', 'ö', 'y', "ssä"),
        ] {
            let sp = format!("{b}e");
            assert_eq!(base_of(lemma), (b, cons), "{lemma}'s stem");
            // The present and the past run on the bare stem, and the present is
            // the bare stem with the type's theme.
            has_all(
                lemma,
                None,
                &[
                    format!("{sp}n"),
                    format!("{sp}t"),
                    format!("{sp}e"),
                    format!("{sp}mme"),
                    format!("{sp}tte"),
                    format!("{sp}v{a}t"),
                    format!("{b}in"),
                    format!("{b}it"),
                    format!("{b}i"),
                    format!("{b}imme"),
                    format!("{b}itte"),
                    format!("{b}iv{a}t"),
                ],
            );
            // The cells that front the consonant are the potential, the past
            // active participle, the whole passive, the second infinitive and
            // the past passive participle's oblique -- and the imperative's
            // plurals, which do not.
            has_all(
                lemma,
                None,
                &[
                    format!("{b}{cons}en"),
                    format!("{b}{cons}et"),
                    format!("{b}{cons}ee"),
                    format!("{b}{cons}emme"),
                    format!("{b}{cons}ette"),
                    format!("{b}{cons}ev{a}t"),
                    format!("{b}{cons}{u}t"),
                    format!("{b}{cons}eet"),
                    format!("{b}{cons}{u}n"),
                    format!("{b}{cons}{u}ss{a}"),
                    format!("{b}{cons}{a}{a}n"),
                    format!("ei {b}{cons}{a}"),
                    format!("{b}tiin"),
                    format!("{b}t{a}isiin"),
                    format!("{b}t{a}neen"),
                    format!("{b}t{a}k{o}{o}n"),
                    format!("{b}t{a}v{a}"),
                    format!("{b}t{u}"),
                    format!("{b}t{u}jen"),
                    format!("{b}{cons}e{hs}"),
                    format!("{b}{cons}en"),
                    format!("{b}k{a}{a}"),
                    format!("{b}k{o}{o}n"),
                    format!("en {sp}"),
                ],
            );
            // And the consonant the member does not have is in none of them.
            let wrong = if cons == 'l' { 'n' } else { 'l' };
            for absent in [
                format!("{b}{wrong}{u}t"),
                format!("{b}{wrong}eet"),
                format!("{b}{wrong}{u}n"),
                format!("{b}{wrong}{a}{a}n"),
                format!("{b}{wrong}{a}"),
                format!("{b}{wrong}t{u}"),
                format!("{b}{wrong}e{hs}"),
            ] {
                assert!(
                    !has(lemma, None, &absent),
                    "{lemma} should not have {absent}"
                );
            }
        }
    }

    /// The finite present, the past, the conditional, the imperative's second
    /// person and the agent take the grade the lemma does not show, while the
    /// potential, the imperative's plurals, the whole passive, the second
    /// infinitive and the participles take the one it does. The sanalista writes
    /// some members in the weak grade and others in the strong one, and both are
    /// here.
    #[test]
    fn the_gradation_runs_from_the_lemma_grade_to_the_other() {
        // `A` is `kk <-> k` and `leikellä` shows the weak one.
        let a_code = Gradation::from_letter('A');
        assert_eq!(strong_base_of("leikel", a_code), "leikkel");
        // `C` is `tt <-> t` and `määritellä` shows the weak one too.
        let c = Gradation::from_letter('C');
        assert_eq!(strong_base_of("määritel", c), "määrittel");
        // `H` is `mp <-> mm` and `ommella` shows the *strong* one, so here the
        // alternation runs the other way and the lemma's grade is the `mm`.
        let h = Gradation::from_letter('H');
        assert_eq!(strong_base_of("ommel", h), "ompel");
        // `K` is `rt <-> rr`, again written in the strong grade.
        let k = Gradation::from_letter('K');
        assert_eq!(strong_base_of("longerrel", k), "longertel");
        // `D` deletes the consonant and is the one code the alternation cannot
        // reach, so its only member's strong grade is written out.
        let d = Gradation::from_letter('D');
        assert_eq!(strong_base_of("jael", d), "jakel");
        assert_eq!(strong_base_of("jael", None), "jael");

        // A member written in the weak grade, and one written in the strong one:
        // the split between the cells is the same either way round.
        for (lem, g, elsewhere, shows) in [
            ("leikellä", a_code, "leikkel", "leikel"),
            ("ommella", h, "ompel", "ommel"),
        ] {
            let (b, sp, _) = stems(lem, g);
            let (a, o, u) = aou(lem);
            let hs = ess(lem);
            assert_eq!(b, shows);
            assert_eq!(stems(lem, g).1, format!("{elsewhere}e"));
            // The finite present, the past, the conditional, the imperative's
            // second person, the agent and the third and fourth infinitives take
            // the grade the lemma does not show.
            has_all(
                lem,
                g,
                &[
                    format!("{sp}n"),
                    format!("{sp}e"),
                    format!("{sp}m{a}{a}n"),
                    format!("{sp}minen"),
                    format!("{sp}m{a}"),
                    format!("{sp}v{a}"),
                    format!("{sp}m{a}t{o}n"),
                    format!("{elsewhere}i"),
                    format!("{elsewhere}isi"),
                    sp.clone(),
                    format!("en {sp}"),
                ],
            );
            // The potential, the imperative's plurals, the passive, the second
            // infinitive and both participles take the grade it does show.
            has_all(
                lem,
                g,
                &[
                    format!("{b}lee"),
                    format!("{b}lev{a}t"),
                    format!("{b}k{a}{a}"),
                    format!("{b}k{o}{o}n"),
                    format!("{b}l{a}{a}n"),
                    format!("{b}tiin"),
                    format!("{b}t{a}v{a}"),
                    format!("{b}t{u}"),
                    format!("{b}l{u}n"),
                    format!("{b}l{u}t"),
                    format!("{b}le{hs}"),
                    format!("en {b}l{u}t"),
                ],
            );
            // Neither grade leaks into the other's cells: the other grade is
            // nowhere in the finite present, the past or the conditional, and
            // the lemma's own grade is nowhere in the potential, the imperative's
            // plurals, the passive or the participle's oblique.
            for absent in [
                // The grade the lemma shows is nowhere in the finite present, the
                // past, the conditional or the imperative's second person.
                format!("{b}{a}n"),
                format!("{b}n"),
                format!("{b}e"),
                format!("{b}i"),
                format!("{b}isi"),
                // And the other grade is nowhere in the potential, the
                // imperative's plurals, the passive, the second infinitive or
                // the past active participle.
                format!("{elsewhere}see"),
                format!("{elsewhere}k{a}{a}"),
                format!("{elsewhere}l{a}{a}n"),
                format!("{elsewhere}t{a}{a}n"),
                format!("{elsewhere}l{u}t"),
                format!("{elsewhere}le{hs}"),
            ] {
                assert!(!has(lem, g, &absent), "{lem} should not have {absent}");
            }
        }
    }

    /// Two members carry a gradation code that the reference's own rows do not
    /// bear out, and the code is dropped for them. Nothing in the lemma tells
    /// them apart from a member that does gradate -- `määritellä` is written the
    /// same way, with the same code, and its `t` does geminate -- so the two
    /// names are the whole of the exception.
    #[test]
    fn two_members_are_read_without_the_gradation_the_sanalista_codes() {
        let c = Gradation::from_letter('C');
        // Every other member keeps the code it is given, whatever its grade.
        assert_eq!(gradation_of("määritellä", c), c);
        assert_eq!(gradation_of("leikellä", c), c);
        assert_eq!(gradation_of("tulla", None), None);
        for lem in ["köpötellä", "narskutella"] {
            assert_eq!(gradation_of(lem, c), None, "{lem}'s code");
            let (b, cons) = base_of(lem);
            let (a, _, u) = aou(lem);
            let hs = ess(lem);
            let sp = format!("{b}e");
            // The geminated grade is what the code asks for and what the
            // reference has not a single row of.
            let gem = strong_base_of(b, c);
            assert_eq!(gem.len(), b.len() + 1, "{lem}'s geminated grade");
            // So every cell runs on the lemma's own single consonant, and both
            // grades the lemma shows agree.
            has_all(
                lem,
                c,
                &[
                    format!("{sp}n"),
                    format!("{sp}e"),
                    format!("{sp}m{a}"),
                    format!("{sp}m{a}n"),
                    format!("{b}i"),
                    format!("{b}isi"),
                    format!("{b}{cons}ee"),
                    format!("{b}{cons}{u}t"),
                    format!("{b}{cons}{u}n"),
                    format!("{b}{cons}{a}{a}n"),
                    format!("{b}t{u}"),
                    format!("{b}tiin"),
                    format!("{b}k{a}{a}"),
                    format!("{b}{cons}e{hs}"),
                ],
            );
            has_none(
                lem,
                c,
                &[
                    format!("{gem}e"),
                    format!("{gem}en"),
                    format!("{gem}i"),
                    format!("{gem}isi"),
                    format!("{gem}lee"),
                    format!("{gem}l{u}t"),
                    format!("{gem}l{a}{a}n"),
                    format!("{gem}k{a}{a}"),
                ],
            );
        }
    }

    /// `olla` is the class's irregular: the regular type reaches `olen` and the
    /// whole past, but no present stem gives both the `olen` of the first person
    /// and the `on` of the third. Those two third persons are written by
    /// [`fix_olla`], the potential runs through the auxiliary `lie`, and `lie`
    /// itself with its negative is filed under `olla` by the reference.
    #[test]
    fn the_irregular_olla_takes_the_on_ovat_and_lie() {
        has_all(
            "olla",
            None,
            &[
                // The two present third persons the regular type cannot reach.
                "on".to_string(),
                "ovat".to_string(),
                // The auxiliary `lie` carries the whole potential...
                "lie".to_string(),
                "lienen".to_string(),
                "lienee".to_string(),
                "lienevät".to_string(),
                // ...and the negative's own connegative.
                "en lie".to_string(),
                "ei lie".to_string(),
            ],
        );
        // The regular type's own third person is gone, and the potential's
        // plural harmonises with `lie` (`lienevät`) and not with `olla`.
        has_none("olla", None, &["olee".to_string(), "lienevat".to_string()]);
        // `olevat` is still a word -- it is the plural of the present active
        // participle -- but no reading of it is the indicative third person.
        let forms = inflect("olla", None);
        assert!(
            !forms.iter().any(|f| {
                (f.word == "olee" || f.word == "olevat")
                    && f.readings.iter().any(|t| {
                        t.mood == Some(Mood::Indicative)
                            && t.tense == Some(Tense::Present)
                            && t.person == Some(Person::Third)
                            && t.voice == Some(Voice::Active)
                            && t.polarity == Polarity::Affirmative
                    })
            }),
            "olla should not read `olee`/`olevat` as a present third person"
        );
    }

}
