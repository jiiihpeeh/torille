//! Class 70 (the `juosta` type) inflection, written by hand.
//!
//! **The type is the `-sta` shape with a `k` that the infinitive does not
//! show.** The lemma is consonant-final once its `ta`/`tä` is off -- `juos`,
//! `pies`, `syös` -- and that is the *weak* grade. The strong grade is the
//! same stem with a `k` in front of its final `s`, `juoks`, and the type row
//! is that grade in four of its eight cells:
//!
//! ```text
//! juosta / juoksen / juoksi / juoksisi / juossee / juoskoon / juossut / juostiin
//! ```
//!
//! so the present, the past and the conditional are the strong stem with the
//! theme vowel on it (`juokse`, `juoksi`, `juoks` + `isi`), and the potential,
//! the imperative's plurals, the passive and the past active participle are the
//! weak one (`juossee`, `juoskaa`, `juostaan`, `juossut`). The remaining cells
//! -- the third and fourth infinitives, the present active participle, the
//! agent and the negative participle -- are the strong stem with a different
//! ending (`juoksema`, `juokseminen`, `juokseva`, `juoksematon`), and the
//! imperative alone straddles the two: its second person singular is the
//! present stem whole (`juokse`) and its plurals are the weak one
//! (`juoskaa`), exactly as class 69's does.
//!
//! That is [`Stems::sta`](super::utility::Stems::sta), which classes 66 and 70
//! share. Class 66's `-ista` verbs are the same shape with the alternation
//! read the other way; what this class adds is only how the strong grade is
//! reached. It is not a gradation code: no member of the class carries one, and
//! the `k` is the type's own, the consonant the infinitive's own `st` and the
//! `-tse`-style theme both point back to. `strong_of` writes it, and the whole
//! module is one line of [`paradigm`](super::utility::paradigm).
//!
//! ## What the reference says
//!
//! The class has three lemmas -- `juosta`, `piestä`, `syöstä` -- and all three
//! are plain `70` with no gradation code, all three end in `sta`/`stä`, and all
//! three inflect alike, front harmony being the only difference (`syökseen`,
//! `syössyt`, `syösty` beside `juoksee`, `juossut`, `juostu`).
//!
//! **All three audit at 0 missing against `both`**, and each leaves one `union`
//! row uncovered, the mis-tagged `...imatontaan` that every verb class in this
//! project has (`juoksematontaan`, `pieksemätöntään`, `syöksemätöntään`).
//!
//! `juosta` is exact apart from that row. The other two are not, and the gap is
//! omorfi's alone: both carry a second, `t`-less `-tu` participle in the plural
//! oblique and its possessives -- `piesyissä` beside `piestyissä`, `syösyiksi`
//! beside `syöstyiksi` -- where the type's own participle has the `t`
//! (`piesty`, `piestyn`, `piestyä`, all of them `both` rows) and every plural
//! cell that *is* a `both` row keeps it (`piestyihin`). The `t`-less variant is
//! `omorfi`-only, no `both` row has it, and it is not a Finnish word, so the
//! module does not write it: 101 rows per member are that variant (the other
//! 101 are the forms the module does write, which the reference lacks). The
//! trade is 3 114 rows covered against 202 words over.

#![allow(dead_code)]

use crate::tags::Form;

use super::utility::{Stems, paradigm};

pub const CLASS: &str = "70";

/// The stem proper of a class-70 verb: the lemma less its `-ta`/`-tä`.
///
/// `juos-ta` gives `juos`, which is the weak grade every one of the type's own
/// cells runs on.
fn base_of(lemma: &str) -> &str {
    lemma
        .strip_suffix("ta")
        .or_else(|| lemma.strip_suffix("tä"))
        .unwrap_or(lemma)
}

/// The strong grade of a class-70 verb: [`base_of`] with a `k` in front of its
/// final `s`.
///
/// `juos` gives `juoks`, `pies` gives `pieks`, `syös` gives `syöks`. It is the
/// type's own consonant and not a gradation the sanalista records, which is why
/// the module cannot hand the shape a code.
fn strong_of(lemma: &str) -> String {
    let base = base_of(lemma);
    format!("{}ks", base.strip_suffix('s').unwrap_or(base))
}

/// Inflect a class-70 verb into its whole paradigm.
pub fn inflect(lemma: &str) -> Vec<Form> {
    paradigm(&Stems::sta(lemma, &strong_of(lemma)))
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

    /// A member's two stems and its harmony, spelled by the same rule the
    /// paradigm uses: the weak base (`juos`) and the strong one (`juoks`), the
    /// two theme vowels the endings harmonise with, and the participle's `u`.
    fn t(lemma: &str) -> (String, String, char, char, char) {
        let (a, o) = if front(lemma) {
            ('ä', 'ö')
        } else {
            ('a', 'o')
        };
        (
            base_of(lemma).to_string(),
            strong_of(lemma),
            a,
            o,
            if front(lemma) { 'y' } else { 'u' },
        )
    }

    /// The strong grade is the base with a `k` in front of its `s`, and it is
    /// what the present, the past and the conditional run on -- the cells where
    /// the sanalista lemma does not show the consonant.
    #[test]
    fn the_strong_grade_inserts_a_k_before_the_s() {
        for lemma in ["juosta", "piestä", "syöstä"] {
            let (base, strong, a, _, _) = t(lemma);
            assert_eq!(strong, format!("{}ks", &base[..base.len() - 1]));
            let present = format!("{strong}e");
            has_all(
                lemma,
                &[
                    format!("{present}n"),
                    format!("{present}t"),
                    format!("{present}e"),
                    format!("{present}mme"),
                    format!("{present}tte"),
                    format!("{present}v{a}t"),
                    format!("{strong}in"),
                    format!("{strong}it"),
                    format!("{strong}i"),
                    format!("{strong}imme"),
                    format!("{strong}itte"),
                    format!("{strong}iv{a}t"),
                    format!("{strong}isi"),
                    format!("{strong}isin"),
                    format!("{strong}isimme"),
                    format!("{strong}isiv{a}t"),
                    // The strong grade is the third and fourth infinitives'
                    // and the present active participle's too.
                    format!("{strong}em{a}"),
                    format!("{present}v{a}"),
                ],
            );
            has_none(
                lemma,
                &[
                    // The weak base's own present, and the `k` in the cells
                    // that do not take it.
                    format!("{base}n"),
                    format!("{base}e"),
                    format!("{base}isi"),
                    format!("{base}ssee"),
                    format!("{base}ssut"),
                ],
            );
        }
    }

    /// The potential and the past active participle are the weak base with the
    /// type's own `s` in front of the ending: `juos` + `s` + `ee`, `juos` +
    /// `s` + `ut`. It is the same `s` the infinitive ends in and the same shape
    /// class 66's `-ssut` is built on.
    #[test]
    fn the_potential_and_the_past_participle_are_splus_the_ending() {
        for lemma in ["juosta", "piestä", "syöstä"] {
            let (base, _, a, _, u) = t(lemma);
            let ss = format!("{base}s");
            has_all(
                lemma,
                &[
                    format!("{ss}en"),
                    format!("{ss}et"),
                    format!("{ss}ee"),
                    format!("{ss}eemme"),
                    format!("{ss}ette"),
                    format!("{ss}ev{a}t"),
                    format!("{ss}{u}t"),
                    format!("{ss}{u}tt{a}"),
                    // The participle's plural, on the same `s`.
                    format!("{ss}eet"),
                    format!("{ss}eiden"),
                ],
            );
            // The strong stem's `k` is not in either cell, and neither is the
            // bare weak base's present participle.
            has_none(
                lemma,
                &[
                    format!("{base}ksee"),
                    format!("{base}ks{u}t"),
                    format!("{base}nee"),
                    format!("{base}n{u}t"),
                ],
            );
        }
    }

    /// The passive writes one `t` each way, present and past alike, and its
    /// `-tu` participle is the plain one that is not deranked: `juostu` keeps
    /// its `t` in the oblique (`juostun`, `juostussa`).
    #[test]
    fn the_passive_writes_one_t() {
        for lemma in ["juosta", "piestä", "syöstä"] {
            let (base, _, a, o, u) = t(lemma);
            let t = format!("{base}t");
            // The passive mood stem is the theme with the mood's own harmony
            // vowel on it: `juosta`, `piestä`.
            let pm = format!("{t}{a}");
            has_all(
                lemma,
                &[
                    format!("{t}{a}{a}n"),
                    format!("{t}iin"),
                    format!("{pm}v{a}"),
                    format!("{pm}isiin"),
                    format!("{pm}neen"),
                    format!("{pm}k{o}{o}n"),
                    format!("{pm}ess{a}"),
                    format!("{pm}m{a}n"),
                    // The negated main-verb words and the `-tu` participle.
                    format!("{t}{a}"),
                    format!("{t}{u}"),
                    format!("{pm}isi"),
                    format!("{pm}ne"),
                    format!(
                        "{pm}k{}",
                        if a == 'ä' { "ö" } else { "o" }
                    ),
                    format!("{t}{u}{a}"),
                    format!("{t}{u}{u}n"),
                    format!("{t}{u}n"),
                    format!("{t}{u}t"),
                    format!("{t}{u}ss{a}"),
                ],
            );
            has_none(
                lemma,
                &[
                    // The gemination every other type would write.
                    format!("{base}tt{a}{a}n"),
                    format!("{base}ttii"),
                ],
            );
        }
    }

    /// The imperative's second person is the strong present stem and its plurals
    /// are the weak base, and the second infinitive's connective is on the weak
    /// base too, so the two straddle the grades in opposite directions.
    #[test]
    fn the_imperative_straddles_the_grades() {
        for lemma in ["juosta", "piestä", "syöstä"] {
            let (base, strong, a, o, _) = t(lemma);
            has_all(
                lemma,
                &[
                    format!("{strong}e"),
                    format!("{base}k{o}{o}n"),
                    format!("{base}k{a}{a}mme"),
                    format!("{base}k{a}{a}"),
                    format!("{base}k{o}{o}t"),
                    format!("{base}te{}", if front(lemma) { "ssä" } else { "ssa" }),
                    format!("{base}ten"),
                ],
            );
            has_none(
                lemma,
                &[
                    format!("{strong}k{o}{o}n"),
                    format!("{strong}k{a}{a}"),
                    format!("{strong}te"),
                ],
            );
        }
    }
}
