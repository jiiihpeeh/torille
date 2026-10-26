//! Class 66 (the `rohkaista` type) inflection, written by hand.
//!
//! Class 66 is the `-ista` type, and it is the first type whose stems are not
//! type 1's. Four things part it from every class above it, and each of them is
//! visible in one line of the sanalista's type row:
//!
//! ```text
//! rohkaista / rohkaisen / rohkaisi / rohkaisisi / rohkaissee / rohkaiskoon
//!            / rohkaissut / rohkaistiin
//! ```
//!
//! * **The infinitive is consonant-final, and the present keeps its `e`.** The
//!   lemma is `-ista`, so the stem proper is `rohkais` and the present stem is
//!   that with the theme: `rohkaise` + `n` is `rohkaisen`, and the third person
//!   adds another `e` (`rohkaisee`), which is the one letter `pres3sg` repeats.
//!   The past drops the theme and runs on `rohkais` itself: `rohkaisi`.
//!
//! * **The potential and the imperative's plurals run on the past stem, not the
//!   present one.** `rohkaissee` is `rohkais` + `s` + `ee` and `rohkaiskaa` is
//!   `rohkais` + `kaa`, where the present stem would give `rohkaisekaa`. This is
//!   why [`Stems`] carries an [`Alt`] for the potential and a separate
//!   `imp_stem`: both are the class's own stem, and both differ from type 1's.
//!   The imperative's *second person singular* stays on the present, `rohkaise`,
//!   so the imperative alone straddles the two stems.
//!
//! * **The `-s` of the infinitive is written into those cells.** The potential
//!   takes `-see` where type 1 takes `-nee`, and the past active participle is
//!   `-ssut` where type 1 has `-nut`. One fact, two cells, and one value: the
//!   class's own ending brings its consonant with it, and the same consonant
//!   fronts both. That is [`Alt`]'s `lead`.
//!
//! * **The passive writes one consonant each way.** `rohkaistaan` in the present
//!   and `rohkaistiin` in the past are the ending's own `t` and no gemination,
//!   and the `-tu` participle is not deranked either: `rohkaistu` has the
//!   oblique `rohkaistun`. That is [`PassiveCons::SingleT`], which is neither of
//!   the two shapes classes 62 to 65 have between them.
//!
//! The gradation runs the other way from type 1's, which is the fourth thing
//! and the reason the module cannot simply hand [`Stems`] a code. Type 1's
//! sanalista lemma shows the *strong* grade and the alternation shows in the
//! other cells; class 66's lemma shows the *weak* one (`häväistä`, `rangaista`)
//! and the finite present and past take the strong grade throughout
//! (`häpäinen`... `häpäisen`, `häpäisi`, `rä`—`rankaisen`, `rankaisi`), while the
//! potential, the imperative's plurals, the whole passive, the present passive
//! participle, the past active participle and the second infinitive take the
//! weak grade the lemma shows (`häväissee`, `häväiskää`, `häväistään`,
//! `häväistava`, `häväissyt`, `häväistessä`). So the module reads the
//! alternation *backwards*, with [`Gradation::strongen`] on the lemma's stem,
//! and hands [`Stems`] no code at all: `with_past(..., None)` leaves
//! [`Stems::weak`] equal to the stem, which is what puts the present's four
//! `e`-less persons, the imperative's second person and the negated present all
//! on the strong grade where this type wants them. The cells that want the weak
//! grade are then named one by one -- the potential, the past participle, the
//! imperative, the passive theme and the second infinitive -- and the rest of
//! the paradigm is class 1's unchanged.
//!
//! The present active participle (`häpäisevä`), the third and fourth
//! infinitives (`häpäisemään`, `häpäiseminen`), the agent and the negative
//! participle (`häpäisemätön`) take the strong grade, and the first infinitive
//! is the lemma as written.
//!
//! The audit is clean on 271 of the class's 272 lemmas. The exception is
//! `vavista`, whose agent Kotus spells `vapisma` -- on the *past* stem, where
//! every other lemma of the class has it on the present one (`rohkaisema`,
//! `vapisema`) -- while the same verb's other entry, `vapista`, spells it
//! `vapisema` like the rest. Kotus marks all 162 of `vavista`'s agent rows
//! `omorfi`-only where `vapista`'s 162 are `both`, so the reference itself
//! declines to vouch for the spelling; this module follows the entry that is
//! vouched for and the class rule. The cost is that one lemma's agent
//! declension (597 forms) counts as invented against Kotus's `union`.

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "66";

/// The stem proper of a class-66 verb: the lemma less its `-ta`/`-tä`.
///
/// `rohkais-ta` gives `rohkais`, which is already the shape the past runs on;
/// the present stem is this with the type's theme `e` put back.
fn base_of(lemma: &str) -> &str {
    lemma
        .strip_suffix("ta")
        .or_else(|| lemma.strip_suffix("tä"))
        .unwrap_or(lemma)
}

/// The strong grade of the class-66 stem, which is the grade the finite present
/// and past run on and the one the sanalista lemma does *not* show.
///
/// The lemma is the weak grade (`häväistä`, `rangaista`), so the alternation has
/// to be read backwards; [`Gradation::strongen`] is what does that, and a lemma
/// with no code is its own strong grade.
fn strong_base_of(base: &str, gradation: Option<Gradation>) -> String {
    gradation.map_or_else(|| base.to_string(), |g| g.strongen(base))
}

/// Inflect a class-66 verb into its whole paradigm.
pub fn inflect(lemma: &str, gradation: Option<Gradation>) -> Vec<Form> {
    let base = base_of(lemma);
    // The present keeps the theme, the past drops it, and the two are the weak
    // and strong grades of the same stem: `rohkaise` beside `rohkais`, and
    // `häpäise` beside `häpäis` for the `E` member.
    let strong_base = strong_base_of(base, gradation);
    paradigm(&Stems::sta(lemma, &strong_base))
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

    /// The two stems of a plain member, which is what every expectation below is
    /// built from: `b` is the stem the lemma shows and the past runs on, `sp` is
    /// that with the type's theme `e` put back for the present.
    fn stems(lemma: &str) -> (String, String) {
        let b = base_of(lemma).to_string();
        (b.clone(), format!("{b}e"))
    }

    /// The `h` and `o` of the word, so that a front member's expectations are
    /// spelled by the same harmony the paradigm uses.
    fn ho(lemma: &str) -> (char, char) {
        if lemma.contains(['y', 'ä', 'ö']) {
            ('ä', 'ö')
        } else {
            ('a', 'o')
        }
    }

    /// The present keeps the infinitive's theme and the past drops it, so the two
    /// stems are one letter apart and the third person repeats the `e`.
    #[test]
    fn the_present_keeps_the_theme_and_the_past_drops_it() {
        let (b, sp) = stems("rohkaista");
        assert_eq!(b, "rohkais");
        assert_eq!(sp, "rohkaise");
        assert_eq!(base_of("hymistä"), "hymis");
        has_all(
            "rohkaista",
            None,
            &[
                format!("{sp}n"),
                format!("{sp}t"),
                format!("{sp}e"),
                format!("{sp}mme"),
                format!("{sp}tte"),
                format!("{sp}vat"),
                format!("{b}in"),
                format!("{b}it"),
                format!("{b}i"),
                format!("{b}imme"),
                format!("{b}itte"),
                format!("{b}ivat"),
            ],
        );
        // The theme is written once, not twice, and the past does not keep it.
        has_none(
            "rohkaista",
            None,
            &[format!("{sp}ee"), format!("{sp}ii"), format!("{b}ii")],
        );
        let (_, hsp) = stems("hymistä");
        let (hb, a) = (base_of("hymistä").to_string(), 'ä');
        has_all(
            "hymistä",
            None,
            &[
                format!("{hsp}n"),
                format!("{hsp}e"),
                format!("{hsp}v{a}t"),
                format!("{hb}i"),
                format!("{hb}iv{a}t"),
            ],
        );
    }

    /// The potential and the imperative's plurals run on the past stem with the
    /// infinitive's `s` in front, while the imperative's second person stays on
    /// the present stem. This is where the class first parts company with type 1.
    #[test]
    fn the_potential_and_the_imperative_run_on_the_past_stem() {
        let (b, sp) = stems("rohkaista");
        has_all(
            "rohkaista",
            None,
            &[
                format!("{b}sen"),
                format!("{b}set"),
                format!("{b}see"),
                format!("{b}semme"),
                format!("{b}sette"),
                format!("{b}sevat"),
                format!("{b}kaa"),
                format!("{b}kaamme"),
                format!("{b}koon"),
                format!("{b}koot"),
                // The imperative's second person singular is the one cell of the
                // imperative that stays on the present stem.
                sp.clone(),
            ],
        );
        // Type 1's `-nen` potential and its imperative on the present stem are
        // not words here, and neither is an `n` before the participle's `-ut`.
        has_none(
            "rohkaista",
            None,
            &[
                format!("{sp}nen"),
                format!("{sp}nee"),
                format!("{sp}nevät"),
                format!("{sp}net"),
                format!("{sp}kaa"),
                format!("{sp}koon"),
                format!("{sp}nut"),
                format!("{b}nut"),
            ],
        );
        let (hb, hsp) = stems("hymistä");
        let (a, o) = ho("hymistä");
        has_all(
            "hymistä",
            None,
            &[
                format!("{hb}sen"),
                format!("{hb}see"),
                format!("{hb}sevät"),
                format!("{hb}kä{a}"),
                format!("{hb}k{o}{o}n"),
                hsp.clone(),
            ],
        );
        has_none(
            "hymistä",
            None,
            &[format!("{hsp}nen"), format!("{hsp}kä{a}")],
        );
    }

    /// The past active participle is the class's `-ssut`, and the `s` it carries
    /// is the same one the potential shows.
    #[test]
    fn the_past_participle_is_ssut() {
        let (b, sp) = stems("rohkaista");
        has_all(
            "rohkaista",
            None,
            &[
                format!("{b}sut"),
                format!("{b}seet"),
                format!("{b}seen"),
                format!("{b}seiden"),
                format!("{b}seessa"),
                format!("{b}seeseen"),
            ],
        );
        has_none(
            "rohkaista",
            None,
            &[format!("{sp}nut"), format!("{sp}neet")],
        );
        let (hb, _) = stems("hymistä");
        has_all("hymistä", None, &[format!("{hb}syt"), format!("{hb}seet")]);
        has_none("hymistä", None, &[format!("{hb}nyt")]);
    }

    /// The passive writes the ending's own `t` in the present and the same `t`
    /// in the past: no gemination anywhere, and no deranking of the `-tu`, which
    /// is the shape neither class 62's nor class 63's passive has.
    #[test]
    fn the_passive_writes_one_t_each_way() {
        let (b, _) = stems("rohkaista");
        has_all(
            "rohkaista",
            None,
            &[
                format!("{b}taan"),
                format!("{b}tiin"),
                format!("{b}taisiin"),
                format!("{b}taneen"),
                format!("{b}takoon"),
                format!("{b}tava"),
                format!("{b}tu"),
                format!("{b}tun"),
                format!("{b}tussa"),
                format!("{b}tuihin"),
                format!("{b}taman"),
            ],
        );
        has_none(
            "rohkaista",
            None,
            &[
                format!("{b}ttaan"),
                format!("{b}ttiin"),
                format!("{b}ttava"),
                format!("{b}ttu"),
                format!("{b}daan"),
                format!("{b}diin"),
                format!("{b}dun"),
            ],
        );
        let (hb, _) = stems("hymistä");
        let (a, _) = ho("hymistä");
        has_all(
            "hymistä",
            None,
            &[
                format!("{hb}tä{a}n"),
                format!("{hb}tiin"),
                format!("{hb}tävä"),
                format!("{hb}ty"),
                format!("{hb}tyn"),
            ],
        );
        has_none(
            "hymistä",
            None,
            &[format!("{hb}ttä{a}n"), format!("{hb}ttiin")],
        );
    }

    /// The conditional follows the present's own stem, the second infinitive the
    /// past stem's own `t`, and the agent, the third and fourth infinitives and
    /// the negative participle the present stem once more.
    #[test]
    fn the_infinitives_and_the_agent_run_on_the_present_stem() {
        let (b, sp) = stems("rohkaista");
        has_all(
            "rohkaista",
            None,
            &[
                // The first infinitive is the lemma as written.
                "rohkaista".to_string(),
                // The conditional is the past stem's `i` + `si`.
                format!("{b}isin"),
                format!("{b}isi"),
                format!("{b}isimme"),
                format!("{b}isitte"),
                format!("{b}isivat"),
                // The second infinitive is the past stem's own `te`.
                format!("{b}tessa"),
                format!("{b}ten"),
                // The third, fourth, agent and negative participle run on the
                // present stem.
                format!("{sp}maan"),
                format!("{sp}malla"),
                format!("{sp}minen"),
                format!("{sp}maton"),
                format!("{sp}ma"),
                format!("{sp}va"),
                // The negated present is the bare present stem and the negated
                // past quotes the `-ssut` participle whole.
                format!("en {sp}"),
                format!("en {b}sut"),
            ],
        );
        has_none(
            "rohkaista",
            None,
            &[format!("{b}sinen"), format!("{sp}nee"), format!("{sp}ne")],
        );
    }

    /// The gradation runs backwards. Type 1's lemma shows the strong grade and
    /// the other cells the weak one; here the lemma shows the weak grade and the
    /// finite present, the conditional and the agent the strong one, while the
    /// potential, the imperative's plurals, the whole passive, the second
    /// infinitive and the past participle take the weak one.
    #[test]
    fn the_gradation_is_read_backwards_from_the_lemma() {
        // `66*E` is `p <-> v` and `häväistä` shows the weak `v`.
        let e = Gradation::from_letter('E');
        assert_eq!(strong_base_of("häväis", e), "häpäis");
        // `66*G` is `nk <-> ng` and `rangaista` shows the weak `ng`.
        let g = Gradation::from_letter('G');
        assert_eq!(strong_base_of("rangais", g), "rankais");
        // A plain member is its own strong grade.
        assert_eq!(strong_base_of("rohkais", None), "rohkais");

        for (lemma, code, strong) in [
            ("häväistä", e, "häpäis"),
            ("rangaista", g, "rankais"),
            ("vavista", e, "vapis"),
        ] {
            let b = base_of(lemma).to_string();
            let sb = strong.to_string();
            let sp = format!("{sb}e");
            let (a, o) = ho(lemma);
            let y = if a == 'ä' { 'y' } else { 'u' };
            // The finite present, the past, the conditional, the agent and the
            // third and fourth infinitives take the strong grade.
            has_all(
                lemma,
                code,
                &[
                    format!("{sp}n"),
                    format!("{sp}t"),
                    format!("{sp}e"),
                    format!("{sp}mme"),
                    format!("{sp}tte"),
                    format!("{sb}in"),
                    format!("{sb}it"),
                    format!("{sb}i"),
                    format!("{sb}imme"),
                    format!("{sb}isi"),
                    format!("{sp}m{a}{a}n"),
                    format!("{sp}minen"),
                    format!("{sp}m{a}"),
                    format!("{sp}v{a}"),
                    format!("en {sp}"),
                ],
            );
            // The potential, the imperative's plurals, the passive, the second
            // infinitive and the past participle take the weak one.
            has_all(
                lemma,
                code,
                &[
                    format!("{b}sen"),
                    format!("{b}see"),
                    format!("{b}sev{a}t"),
                    format!("{b}k{a}{a}"),
                    format!("{b}k{o}{o}n"),
                    format!("{b}t{a}{a}n"),
                    format!("{b}tiin"),
                    format!("{b}t{a}v{a}"),
                    format!("{b}t{y}"),
                    format!("{b}t{a}en"),
                    format!("{b}s{y}t"),
                ],
            );
            // The imperative's second person singular stays on the present stem,
            // and the negated past quotes the weak `-ssut`.
            has_all(lemma, code, &[sp.clone(), format!("en {b}s{y}t")]);
            // The weak grade is nowhere in the finite present, and the strong
            // grade is nowhere in the potential or the passive.
            for absent in [
                format!("{b}{a}n"),
                format!("{b}ne"),
                format!("{sb}see"),
                format!("{sb}k{a}{a}"),
                format!("{sb}t{a}{a}n"),
                format!("{sb}syt"),
            ] {
                assert!(
                    !has(lemma, code, &absent),
                    "{lemma} should not have {absent}"
                );
            }
        }
    }
}
