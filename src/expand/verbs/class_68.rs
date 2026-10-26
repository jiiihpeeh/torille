//! Class 68 (the `tupakoida` type) inflection, written by hand.
//!
//! **This type is class 62's, cell for cell.** The two type rows are the same
//! eight cells spelled on two lemmas --
//!
//! ```text
//! voida     / voin     / voi     / voisi     / voicee     / voikoon     / voinut     / voitiin
//! tupakoida / tupakoin / tupakoi / tupakoisi / tupakoinee / tupakoikoon / tupakoinut / tupakoitiin
//! ```
//!
//! — and `tupakoi` is the same shape of word beside `voi` that `tupakoitin`
//! would be beside `voida`. So the whole of the first paradigm is
//! [`Stems::ida`], which is class 62's stems and nothing more, and this module
//! has nothing to say about it. The two classes exist because Kotus reads a
//! lexical difference the rule does not see: class 62 is `voida`, `naida` and
//! their long `-elöidä`/`-intöidä` kin, class 68 is the `-oida`/`-oidä` type,
//! and between them they hold every `-ida`/`-idä` verb in the language. The
//! split shows up in the sanalista, not in the rule: `otsikoida` and
//! `aateloida` are class 62 and `tupakoida` and `haravoida` are class 68, and
//! all four inflect alike.
//!
//! What the type row gives class 68 and does not give class 62 is the second
//! set of forms in its parentheses, and that is the whole of this module.
//!
//! ## The `-tse` alternant
//!
//! `tupakoin (tupakoitsen)`, `tupakoi (tupakoitsi)`, `tupakoisi (tupakoitsisi)`:
//! an alternant paradigm beside the type's own, and the only Finnish verb
//! paradigm the sanalista asks for twice. It is the conjunctive present, the
//! one that stands in a subordinate clause (`sen tiedon, että hän
//! tupakoitsee`), and it has its own past and conditional: `tupakoitsi` and
//! `tupakoitsisi` are the same three letters the type's `-ita` verbs
//! (`valitsi`, `valitsisi`) put on their `valit-` base, and the class's
//! alternant is `tupakoit-` with `se`, `si` and `sisi`.
//!
//! Four things about it are worth saying, because none of them is the ordinary
//! type's shape:
//!
//! * **Two of the three stems are `i`-final, and one is not.** The main
//!   paradigm's cells all turn on `paradigm`'s `absorb_i`, which drops an
//!   ending's `i` in front of a stem that ends in `i` and is what makes the main
//!   present's third person the bare `tupakoi`. `tupakoitse` ends in `e`, so
//!   the alternant's present is the plain type-1 set on it -- `tupakoitsen`,
//!   `tupakoitsee`, `tupakoitsevat` -- while `tupakoitsi` and `tupakoitsisi` are
//!   `i`-final and absorb, giving `tupakoitsin` and `tupakoitsisin` where
//!   `tupakoitse` + `isin` would not be a word.
//!
//! * **It is a derivation, not a paradigm, so `paradigm` is the wrong tool for
//!   it.** `paradigm` builds every cell a type has: the potential, the whole
//!   passive, the imperative's plurals, the participles, the second and fourth
//!   infinitives. The reference has no row for any of those on the `-tse` stem,
//!   and generating them would be inventing a dozen words per lemma that neither
//!   Kotus nor Omorfi has. So the alternant is written out here, cell by cell,
//!   and is what it is: the present and its negative and its imperative's second
//!   person, the past, the conditional, the third infinitive's six cases and the
//!   fifth infinitive with its possessives. Thirty-one words, and every one of
//!   them a row in the reference for every member that has one.
//!
//! * **The pronominal negatives are not here, as they are nowhere.** The
//!   reference has `en tupakoitse` beside `et`, `ei`, `emme`, `ette` and
//!   `eivät`, but the engine writes a bare negative stem and lets the pronoun be
//!   supplied from outside (`en puhu`, not a per-lemma `en puhu`-pair), so it
//!   does so here too: `tupakoitse` is pushed once as the negative present and
//!   once as the imperative's second person, which is what the reference's two
//!   rows for that word are.
//!
//! * **It is `-oid-` morphology, not a lexical exception.** There is nothing to
//!   distinguish a member that has the alternant from one that does not, and
//!   nothing in the sanalista to read it off: all 52 of the class's members are
//!   plain `68`, with no gradation code. The class always writes it, which is
//!   the cost of the two members the reference has no `ts` row for -- see below.
//!
//! ## What the reference says
//!
//! 51 lemmas, **0 invented and 0 missing** against `both`: the main paradigm is
//! [`Stems::ida`]'s and the alternant's thirty-one words are exact. Against
//! `union` the class leaves 345 rows uncovered and 62 words over, all of it in
//! three pieces that are not this module's to fix:
//!
//! * **294 rows are the pronominal negatives** above -- six per member, for the
//!   49 members that have the alternant at all.
//!
//! * **51 rows are `...imatontaan`** (`tupakoimatontaan`,
//!   `ahkeroimatontaan`, `emännöimätöntään`), one per member. Every one is
//!   `omorfi`-only, and every one is a mis-tagged row: `polarity` is
//!   `affirmative` under `participle = negative`, `possessive` is `third person`
//!   on a partitive, and the word itself is not Finnish -- it is the `-maton`
//!   participle with the genitive's `t` in it, where `tupakoimattomaan` is the
//!   word. The same category as class 62's `ui`.
//!
//! * **62 words are the alternant for `pakkeloida` and `vatuloida`**, the two
//!   members with no `ts` row anywhere in the reference. There is nothing in the
//!   sanalista to tell them from the other 49 -- all three are plain `68` -- so
//!   suppressing the alternant for them would mean a hard-coded list of two
//!   words, which is the thing this engine does not do. (Class 62's 670 `-oid-`
//!   members have the same fact against them: exactly one of them has a `ts`
//!   row, and it is not emitted there either.) Emitting it for all of them
//!   trades 62 words for the 1 862 rows it does cover, and for the three
//!   alternants `expand`'s own catalogue test requires of the class.
//!
//! `änkyröidä`, the one member the sanalista has and the reference does not,
//! inflects like the rest: it is an `-oidä` word and the type has nothing else
//! to say about it.

#![allow(dead_code)]

use crate::tags::{Case, Form, Mood, Number, Person, Polarity, Tense, Voice};

use super::utility::{Stems, ThirdPossessive, VerbBuilder, paradigm, verb_tags};

pub const CLASS: &str = "68";

/// Inflect a class-68 verb into its whole paradigm, the `-tse` alternant
/// included.
pub fn inflect(lemma: &str) -> Vec<Form> {
    let mut out = paradigm(&Stems::ida(lemma));
    out.extend(tse_alternant(lemma));
    out
}

/// The alternant's three stems, one per tense: the present's (`tupakoitse`),
/// the past's (`tupakoitsi`) and the conditional's (`tupakoitsisi`).
///
/// All three are the *same* base with `se`, `si` and `sisi` on it, and the base
/// is the main stem with the type's own `t` in front of the ending -- the `t`
/// the passive writes its `-aa` against (`tupakoitaa`) and the `-ttu` participle
/// its `u` against (`tupakoitu`). So the second paradigm is the first one three
/// letters on (`tupakoi` -> `tupakoit` -> `tupakoitse`), which is how the
/// type's own `-ita` verbs are built too: `valitse`, `valitsi`, `valitsisi` on
/// the base `valit`, where the infinitive is `valita` and the class's own stem
/// is `vali`.
///
/// Two of the three are `i`-final and one is not, and that is the whole of the
/// alternant's grammar: `tupakoitse` ends in a vowel, so the present is the
/// plain type-1 set on it -- `tupakoitsen`, `tupakoitsee`, `tupakoitsevat` --
/// and the `i`-final `tupakoitsi` and `tupakoitsisi` absorb their endings' `i`
/// as the main paradigm's own `absorb_i` does, giving `tupakoitsin` and
/// `tupakoitsisin` where a written-whole `isin` would not be a word.
fn tse_stems(lemma: &str) -> (String, String, String) {
    let stem = lemma
        .strip_suffix("da")
        .or_else(|| lemma.strip_suffix("dä"))
        .unwrap_or(lemma);
    let base = format!("{stem}t");
    (
        format!("{base}se"),
        format!("{base}si"),
        format!("{base}sisi"),
    )
}

/// The alternant paradigm, on the three stems [`tse_stems`] names.
fn tse_alternant(lemma: &str) -> Vec<Form> {
    let (ts, si, sisi) = tse_stems(lemma);
    let mut b = VerbBuilder::new(lemma);
    let front = b.front();
    // `VerbBuilder::h` would be the same function, but it borrows `b` and the
    // pushes below need it mutably; the flag is already read out.
    let h = |back: &'static str, front_twin: &'static str| -> String {
        if front {
            front_twin.to_string()
        } else {
            back.to_string()
        }
    };

    // --- the finite present, as an ordinary present on a vowel-final stem ---
    for (e, person, number) in [
        ("n", Person::First, Number::Singular),
        ("t", Person::Second, Number::Singular),
        ("e", Person::Third, Number::Singular),
        ("mme", Person::First, Number::Plural),
        ("tte", Person::Second, Number::Plural),
    ] {
        b.push(
            format!("{ts}{e}"),
            verb_tags(
                Some(Tense::Present),
                Some(Mood::Indicative),
                Some(person),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        );
    }
    b.push(
        format!("{ts}{}", h("vat", "vät")),
        verb_tags(
            Some(Tense::Present),
            Some(Mood::Indicative),
            Some(Person::Third),
            Some(Number::Plural),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    );
    // The bare stem is the negated present, as it is in every paradigm here,
    // and the same word is the imperative's second person -- which is what the
    // two rows the reference gives `tupakoitse` are.
    b.push(
        ts.clone(),
        verb_tags(
            Some(Tense::Present),
            Some(Mood::Indicative),
            None,
            None,
            Some(Voice::Active),
            None,
            Polarity::Negative,
        ),
    );
    b.push(
        ts.clone(),
        verb_tags(
            Some(Tense::Present),
            Some(Mood::Imperative),
            Some(Person::Second),
            Some(Number::Singular),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    );

    // --- the past, on `si`, which is `i`-final, so `absorb_i` fires on it
    // exactly as it does in the main past: the type's own `in`/`it`/`i`/
    // `imme`/`itte`/`vat` with their `i` swallowed. ---
    for (e, person, number) in [
        ("n", Person::First, Number::Singular),
        ("t", Person::Second, Number::Singular),
        ("", Person::Third, Number::Singular),
        ("mme", Person::First, Number::Plural),
        ("tte", Person::Second, Number::Plural),
    ] {
        b.push(
            format!("{si}{e}"),
            verb_tags(
                Some(Tense::Past),
                Some(Mood::Indicative),
                Some(person),
                Some(number),
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            ),
        );
    }
    b.push(
        format!("{si}{}", h("vat", "vät")),
        verb_tags(
            Some(Tense::Past),
            Some(Mood::Indicative),
            Some(Person::Third),
            Some(Number::Plural),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    );

    // --- the conditional, on `sisi`, which is `i`-final as the past's stem is
    // and absorbs the same way: `tupakoitse` + `isi` is not a word, and
    // `tupakoitsi` + `si` = `tupakoitsisi` is. ---
    for (e, person, number) in [
        ("n", Person::First, Number::Singular),
        ("t", Person::Second, Number::Singular),
        ("", Person::Third, Number::Singular),
        ("mme", Person::First, Number::Plural),
        ("tte", Person::Second, Number::Plural),
    ] {
        b.push(
            format!("{sisi}{e}"),
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
    b.push(
        format!("{sisi}{}", h("vat", "vät")),
        verb_tags(
            Some(Tense::Present),
            Some(Mood::Conditional),
            Some(Person::Third),
            Some(Number::Plural),
            Some(Voice::Active),
            None,
            Polarity::Affirmative,
        ),
    );

    // --- the third infinitive's six cases, and no `-tama` illative ---
    let ma = format!("{ts}m{}", h("a", "ä"));
    for (e, case) in [
        (format!("{ma}{}", h("n", "n")), Case::Instructive),
        (format!("{ma}{}", h("ssa", "ssä")), Case::Inessive),
        (
            format!("{ma}{}{}", h("a", "ä"), h("n", "n")),
            Case::Illative,
        ),
        (format!("{ma}{}", h("sta", "stä")), Case::Elative),
        (format!("{ma}{}", h("lla", "llä")), Case::Adessive),
        (format!("{ma}{}", h("tta", "ttä")), Case::Abessive),
    ] {
        b.push(
            e,
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
    // The fifth infinitive, with its six possessives -- and, as in `paradigm`,
    // no bare form: the reference has no row for `tupakoitsemaisilla` itself,
    // only for the six possessives of it. There is no fourth infinitive either,
    // since `-minen` is not a row the reference has on this stem.
    b.possess(
        format!("{ts}m{}isill{}", h("a", "ä"), h("a", "ä")),
        ThirdPossessive::LongAndShort,
        |poss| {
            let mut tags = verb_tags(
                None,
                Some(Mood::Infinitive),
                None,
                None,
                Some(Voice::Active),
                None,
                Polarity::Affirmative,
            );
            tags.possessive = Some(poss);
            tags
        },
    );
    b.into_forms()
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

    /// A member's four stems: the main one and the alternant's three, which
    /// are the main one with `tse`, `tsi` and `tsisi` on it. The three letters
    /// are the word's own harmony, so a front member's expectations are spelled
    /// by the same rule the paradigm uses.
    fn t(lemma: &str) -> (String, String, String, String, char, char, char) {
        let (a, o, u) = if front(lemma) {
            ('ä', 'ö', 'y')
        } else {
            ('a', 'o', 'u')
        };
        let (ts, si, sisi) = tse_stems(lemma);
        (Stems::ida(lemma).stem, ts, si, sisi, a, o, u)
    }

    /// The stem less its own final `i`, which is what the main conditional's
    /// `absorb_i` amounts to: `tupako` + `isi` is the same word as `tupakoi` +
    /// `si`, and the first is the one the ending is written against.
    fn c(b: &str) -> &str {
        &b[..b.len() - 1]
    }

    /// The second infinitive's inessive, whose `-ssa` follows the word's
    /// harmony while the `de` in front of it is a plain `e` in both.
    ///
    /// (The third person's own `s` in a possessive suffix harmonises the same
    /// way -- `emännöitsemäisillänsä` -- so [`nsa`](nsa) is here for the same
    /// reason.)
    fn ess(lemma: &str) -> &'static str {
        if front(lemma) { "ssä" } else { "ssa" }
    }

    /// The third person's long possessive suffix, harmonised.
    fn nsa(lemma: &str) -> &'static str {
        if front(lemma) { "nsä" } else { "nsa" }
    }

    /// The main paradigm is the `-ida` type's, cell for cell with class 62. The
    /// stem is pinned down by the bare third person present -- `tupakoi`, not
    /// `tupakoii` or `tupakoia` -- and the three fields the type sets are
    /// pinned down by what is *not* there: the geminated passive, the rethemed
    /// second infinitive, and a repeated third-person vowel.
    #[test]
    fn the_main_paradigm_is_the_ida_types() {
        for lemma in [
            "tupakoida",
            "haravoida",
            "ahkeroida",
            "aprikoida",
            "emännöidä",
            "heilimöidä",
            "kyynelöidä",
            "vihannoida",
        ] {
            let (b, _, _, _, a, _, _) = t(lemma);
            assert!(has(lemma, &b), "{lemma} should have the bare stem {b}");
            has_all(
                lemma,
                &[
                    format!("{b}n"),
                    format!("{b}t"),
                    format!("{b}mme"),
                    format!("{b}tte"),
                    format!("{b}v{a}t"),
                    // The conditional, on the stem less its own `i`.
                    format!("{}isi", c(&b)),
                    format!("{}isin", c(&b)),
                    format!("{}isimme", c(&b)),
                    format!("{}isiv{}t", c(&b), a),
                    // The second infinitive runs on the ending's `d`.
                    format!("{b}de{}", ess(lemma)),
                    format!("{b}den"),
                ],
            );
            has_none(
                lemma,
                &[
                    // A repeated third-person vowel (class 61's shape).
                    format!("{b}ii"),
                    // A rethemed third person, and class 61's geminated passive.
                    format!("{b}{a}"),
                    format!("{b}{a}n"),
                    format!("{b}ttii"),
                    format!("{b}tt{}n", a),
                    // A second infinitive on the rethemed stem, as class 61 has.
                    format!("{b}e{}", ess(lemma)),
                    // And the conditional on the stem whole, `tupako` + `is`.
                    format!("{b}isi"),
                ],
            );
        }
    }

    /// The rest of the type, on a front-harmony member where every `u`-ending
    /// is its `y` twin -- the `-nut` participle, the `i`-stem the passive
    /// deranks, and the imperative's `-kö-` -- while the endings that carry no
    /// vowel of their own (`-minen`, `-tama`, `-den`) stay plain.
    #[test]
    fn a_front_member_harmourizes_the_vowel_endings_only() {
        let (b, _, _, _, a, o, u) = t("emännöidä");
        has_all(
            "emännöidä",
            &[
                b.clone(),
                format!("{b}n"),
                format!("{b}t"),
                format!("{b}v{a}t"),
                format!("{}isi", c(&b)),
                format!("{b}k{a}{a}"),
                format!("{b}k{o}{o}n"),
                format!("{b}k{a}{a}mme"),
                format!("{b}n{u}t"),
                format!("{b}minen"),
                format!("{b}neet"),
                format!("{b}v{a}"),
                format!("{b}m{a}"),
                format!("{b}m{a}t{o}n"),
                format!("{b}m{a}isill{a}mme"),
                format!("{b}t{u}"),
                format!("{b}ty"),
                format!("{b}dyt"),
                format!("{b}dyn"),
                format!("{b}d{a}{a}n"),
                format!("{b}t{a}isiin"),
                format!("{b}t{a}m{a}n"),
            ],
        );
        // The `-i-` of `-minen` and the `-a-` of `-tama` do not harmonise, and
        // the type's own back-harmony spellings are not words here.
        has_none(
            "emännöidä",
            &[
                format!("{b}tam{a}n"),
                format!("{b}nut"),
                format!("{b}vat"),
                format!("{b}m{a}ton"),
                format!("{b}d{}n", o),
            ],
        );
    }

    /// The alternant's stem is the main stem with `tse` on it -- the ending's
    /// `d` written `ts` -- and because it ends in a vowel nothing is absorbed:
    /// the present is the plain type-1 set on it. The other two stems are
    /// `i`-final, and they take their endings the way the main past does.
    #[test]
    fn the_alternants_stem_is_vowel_final_so_nothing_is_absorbed() {
        let (b, ts, si, sisi, a, _, _) = t("tupakoida");
        assert_eq!(ts, format!("{b}tse"));
        assert_eq!(si, format!("{b}tsi"));
        assert_eq!(sisi, format!("{b}tsisi"));
        assert!(ts.ends_with('e'), "{ts} should be vowel-final");
        has_all(
            "tupakoida",
            &[
                format!("{ts}n"),
                format!("{ts}t"),
                format!("{ts}e"),
                format!("{ts}mme"),
                format!("{ts}tte"),
                format!("{ts}v{a}t"),
                format!("{sisi}n"),
                format!("{sisi}t"),
                format!("{sisi}"),
                format!("{sisi}mme"),
                format!("{sisi}tte"),
                format!("{sisi}v{a}t"),
            ],
        );
        // The third person present is the stem with an `e`, where the main
        // present's is the bare stem; and the conditional's `isi` is written
        // against its own stem, not the present's.
        has_none(
            "tupakoida",
            &[
                format!("{ts}ee"),
                format!("{ts}isii"),
                // `tupakoitse` + `isi`, which no reference row has.
                format!("{ts}isi"),
                format!("{ts}isin"),
                // The other harmony's spellings are not words here.
                format!("{ts}isiv{}t", 'v'),
                format!("{sisi}isiv{}t", 'v'),
            ],
        );
        // The main conditional is `tupakoisi`, and that is the same word spelled
        // against a different stem -- which is the whole difference between the
        // type row's cell and the one in its parentheses.
        assert_eq!(format!("{}isi", c(&b)), format!("{b}si"));
        // The same on a front member.
        let (_, fts, _, fsisi, fa, _, _) = t("emännöidä");
        has_all(
            "emännöidä",
            &[
                format!("{fts}n"),
                format!("{fts}e"),
                format!("{fts}v{fa}t"),
                format!("{fsisi}v{fa}t"),
            ],
        );
        has_none(
            "emännöidä",
            &[format!("{fts}v{}t", 'a'), format!("{fsisi}v{}t", 'a')],
        );
    }

    /// The alternant's past is one of its two `i`-final stems, so `absorb_i`
    /// fires on it exactly as in the main past and gives the same six endings
    /// off a different stem -- which is why the type row needs its parentheses
    /// on the past as well as on the present.
    #[test]
    fn the_alternants_past_absorbs_as_the_main_past_does() {
        let (b, ts, si, _, a, _, _) = t("tupakoida");
        assert!(si.ends_with('i'), "{si} should be i-final");
        has_all(
            "tupakoida",
            &[
                si.clone(),
                format!("{si}n"),
                format!("{si}t"),
                format!("{si}mme"),
                format!("{si}tte"),
                format!("{si}v{a}t"),
            ],
        );
        // The endings written whole are the main past's shape on the main
        // stem, and are not words here.
        has_none(
            "tupakoida",
            &[
                format!("{ts}in"),
                format!("{ts}it"),
                format!("{ts}i"),
                format!("{ts}imme"),
                format!("{ts}ivat"),
            ],
        );
        // Nor is the main past's own `i`-stem, which is what the parenthesis
        // in the type row stands beside.
        has_none(
            "tupakoida",
            &[format!("{b}in"), format!("{b}it"), format!("{b}imme")],
        );
        assert!(has("tupakoida", &format!("{b}n")));
    }

    /// The alternant's two infinitives: the `-ma` one in its six cases and the
    /// `-maisilla` one with its possessives, harmonised throughout. There is no
    /// `-taman` illative, no `-minen`, no passive, and no potential beside
    /// them -- the reference has no row for any of those on this stem, and
    /// `paradigm` would have written a dozen words per lemma out of them.
    #[test]
    fn the_alternant_has_two_infinitives_and_no_more() {
        let (_, ts, _, _, a, _, _) = t("tupakoida");
        let ma = format!("{ts}m{a}");
        has_all(
            "tupakoida",
            &[
                format!("{ma}n"),
                format!("{ma}{}", ess("tupakoida")),
                format!("{ma}{a}n"),
                format!("{ma}st{}", a),
                format!("{ma}ll{}", a),
                format!("{ma}tt{}", a),
                format!("{ts}m{a}isill{a}mme"),
                format!("{ts}m{a}isill{a}ni"),
                format!("{ts}m{a}isill{a}nsa"),
            ],
        );
        has_none(
            "tupakoida",
            &[
                format!("{ts}minen"),
                format!("{ts}tam{a}n"),
                format!("{ts}t{a}{a}n"),
                format!("{ts}tiin"),
                format!("{ts}ne"),
                format!("{ts}nut"),
            ],
        );
        // And the front member's own spellings.
        let (_, fts, _, _, fa, _, _) = t("emännöidä");
        has_all(
            "emännöidä",
            &[
                format!("{fts}m{fa}n"),
                format!("{fts}m{fa}{fa}n"),
                format!("{fts}m{fa}st{fa}"),
                format!("{fts}m{fa}isill{fa}ni"),
                format!("{fts}m{fa}isill{fa}{}", nsa("emännöidä")),
            ],
        );
    }
}
