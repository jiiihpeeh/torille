//! Class 71 (the `nähdä` type) inflection, written by hand.
//!
//! **The lemma's `hd` is three different consonants, and no one stem carries the
//! whole paradigm.** `nähdä` and `tehdä` are the class, and each splits into a
//! present stem that has lost the `h` and grown a `k`, a past stem on that same
//! `k`, and a third stem, the lemma's own `näh`/`teh`, for everything else:
//!
//! ```text
//! nähdä / näen / näki / näkisi / nähnee / nähköön / nähnyt / nähtiin
//! ```
//!
//! * the present is strong `näke` and weak `näe`, and the split is a gradation:
//!   the first and second persons take the weak grade (`näen`, `näet`,
//!   `näemme`, `näette`) and the third the strong (`näkee`, `näkevät`). It is
//!   `k` <-> `∅`, the sanalista's gradation `D`, and the type writes it into
//!   itself rather than into the lemmas' rows;
//! * the past is the strong stem with the past's `i`: `näki` in the third
//!   persons and, with the `k` dropped, `näin`/`näit`/`näimme`/`näitte` in the
//!   others. The conditional is the strong stem bare with the ordinary `-isi`
//!   (`näk` + `isi` is `näkisi`), so it keeps the `k` in every person where the
//!   past keeps it in only two;
//! * everything else is the lemma's own `näh`/`teh`: the potential and the past
//!   active participle (`näh` + `n` + `ee`, `näh` + `n` + `yt`), the imperative
//!   (`näh` + `kää`, `näh` + `köön`) and the passive (`näh` + `d` + `ään`,
//!   `näh` + `t` + `iin`). The passive is [`PassiveCons::Deranked`], the same
//!   shape class 62's `-ida` has: a `d` in the present, a single `t` in the past
//!   and the moods, and a `-tu` participle that deranks again in the
//!   oblique (`nähty` beside `nähdyn`).
//!
//! The second infinitive is the `näh` stem with the type's own `de`
//! (`nähden`, `nähdessä`), and the third and fourth infinitives, the present
//! participle, the agent and the negative participle take the strong present
//! stem (`näkemä`, `näkeminen`, `näkevä`, `näkemätön`). The imperative is the
//! only cell on both stems, its second person singular being the weak present
//! (`näe`) and its plurals the `näh` one (`nähkää`).
//!
//! ## What the reference says
//!
//! Two lemmas, and they are the same shape word for word, `nä-` against `te-`.
//! Both audit at **0 invented and 0 missing against `both`** over all 926 of
//! their reference forms, and both leave one `union` row uncovered, the
//! mis-tagged `...imatontaan` that every verb class in this project has
//! (`näkemätöntään`, `tekemättömään`).

#![allow(dead_code)]

use crate::kotus::Gradation;
use crate::tags::Form;

use super::utility::{Alt, PassiveCons, Stems, paradigm};

pub const CLASS: &str = "71";

/// The lemma's own stem: `nähdä` less its `dä`, the `näh`/`teh` every cell but
/// the present and the past runs on.
fn h_stem_of(lemma: &str) -> &str {
    lemma
        .strip_suffix("dä")
        .or_else(|| lemma.strip_suffix("da"))
        .unwrap_or(lemma)
}

/// The strong present and past stem: the `h` stem's own `h` read as `k`
/// (`näh` -> `näk`, `teh` -> `tek`).
///
/// The `h` becomes a `k` and the `d` of the infinitive falls away, which is
/// what puts `näkee` beside `nähdä` and `näki` beside `nähnee`.
fn k_stem_of(lemma: &str) -> String {
    let h = h_stem_of(lemma);
    format!("{}k", h.strip_suffix('h').unwrap_or(h))
}

/// Inflect a class-71 verb into its whole paradigm.
pub fn inflect(lemma: &str) -> Vec<Form> {
    let h = h_stem_of(lemma).to_string();
    let k = k_stem_of(lemma);
    // The present is `näke`, the past `näki`, and the gradation `D` gives the
    // weak `näe` the first and second persons take. `with_past` would make the
    // past's weak grade out of that same `näe` and the passive theme out of its
    // `e`, so both are set by hand below.
    let mut s = Stems::with_past(
        lemma,
        &format!("{k}e"),
        &format!("{k}i"),
        Gradation::from_letter('D'),
    );
    // `näin`, not `näein`: the weak past is the `k` stem less its `k`, plus `i`.
    s.past_weak = format!("{}i", k.strip_suffix('k').unwrap_or(&k));
    // The conditional is the `k` stem bare.
    s.cond_stem = k.clone();
    // The potential, the past active participle and the imperative run on the
    // `h` stem, and the passive's theme is the same word.
    s.pot = Alt::n(&h);
    s.past_part = Alt::n(&h);
    s.imp_stem = h.clone();
    s.p_theme = h.clone();
    s.p_cons = PassiveCons::Deranked;
    // The second infinitive is the `h` stem with the type's own `de`: `nähden`,
    // `nähdessä`.
    s.inf2_stem = format!("{h}de");
    paradigm(&s)
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

    /// A member's four stems and its harmony: the lemma's own `näh`, the strong
    /// `näk` the present and past share, the strong present `näke`, the weak
    /// present `näe`, and the three harmony vowels.
    fn t(lemma: &str) -> (String, String, String, String, char, char, char) {
        let (a, o, u) = if front(lemma) {
            ('ä', 'ö', 'y')
        } else {
            ('a', 'o', 'u')
        };
        let h = h_stem_of(lemma).to_string();
        let k = k_stem_of(lemma);
        let weak = format!("{}e", k.strip_suffix('k').unwrap_or(&k));
        let strong = format!("{k}e");
        (h, k, strong, weak, a, o, u)
    }

    /// The present splits by grade: the first and second persons take the weak
    /// `näe` and the third the strong `näke`, which is the type's own `k`<->
    /// nothing alternation. The negative present and the imperative's second
    /// person are the weak stem too.
    #[test]
    fn the_present_splits_by_grade() {
        for lemma in ["nähdä", "tehdä"] {
            let (_, _, strong, weak, a, _, _) = t(lemma);
            has_all(
                lemma,
                &[
                    format!("{weak}n"),
                    format!("{weak}t"),
                    format!("{strong}e"),
                    format!("{weak}mme"),
                    format!("{weak}tte"),
                    format!("{strong}v{a}t"),
                    weak.clone(),
                ],
            );
            has_none(
                lemma,
                &[
                    format!("{strong}n"),
                    format!("{strong}t"),
                    format!("{weak}e"),
                    format!("{weak}v{a}t"),
                ],
            );
        }
    }

    /// The past keeps the `k` only in the third persons (`näki` beside `näin`),
    /// and the conditional keeps it in all six because it is the `k` stem bare
    /// with the ordinary `-isi`.
    #[test]
    fn the_past_drops_the_k_and_the_conditional_keeps_it() {
        for lemma in ["nähdä", "tehdä"] {
            let (_, k, _, _, a, _, _) = t(lemma);
            let weak_past = format!("{}i", k.strip_suffix('k').unwrap_or(&k));
            has_all(
                lemma,
                &[
                    // The past: `k` in the third persons, dropped elsewhere.
                    format!("{weak_past}n"),
                    format!("{weak_past}t"),
                    format!("{k}i"),
                    format!("{weak_past}mme"),
                    format!("{weak_past}tte"),
                    format!("{k}iv{a}t"),
                    // The conditional: `k` throughout.
                    format!("{k}isi"),
                    format!("{k}isin"),
                    format!("{k}isit"),
                    format!("{k}isimme"),
                    format!("{k}isitte"),
                    format!("{k}isiv{a}t"),
                ],
            );
            has_none(
                lemma,
                &[
                    // The `k` in the past's other persons, and the weak past's
                    // `i` in a cell the conditional owns.
                    format!("{k}in"),
                    format!("{k}imme"),
                    format!("{weak_past}isi"),
                    format!("{weak_past}isin"),
                ],
            );
        }
    }

    /// The potential, the past active participle, the imperative's plurals and
    /// the whole passive run on the lemma's own `näh` stem, not on the
    /// present's `k` one; the passive is deranked as class 62's is.
    #[test]
    fn the_h_stem_carries_the_potential_the_participle_and_the_passive() {
        for lemma in ["nähdä", "tehdä"] {
            let (h, k, _, _, a, o, u) = t(lemma);
            has_all(
                lemma,
                &[
                    // Potential and past active participle, the `n`-lead on `näh`.
                    format!("{h}nen"),
                    format!("{h}nee"),
                    format!("{h}nev{a}t"),
                    format!("{h}n{u}t"),
                    format!("{h}n{u}tt{a}"),
                    // Imperative: 2sg is the weak present, the rest the `h` stem.
                    format!("{h}k{o}{o}n"),
                    format!("{h}k{a}{a}"),
                    format!("{h}k{o}{o}t"),
                    // Passive: `d` in the present, `t` in the past and the moods,
                    // and the `-tu` participle deranking in the oblique.
                    format!("{h}d{a}{a}n"),
                    format!("{h}tiin"),
                    format!("{h}t{a}v{a}"),
                    format!("{h}t{a}isiin"),
                    format!("{h}t{a}neen"),
                    format!("{h}t{a}k{o}{o}n"),
                    format!("{h}t{a}ess{a}"),
                    format!("{h}t{a}m{a}n"),
                    format!("{h}t{u}"),
                    format!("{h}t{u}{a}"),
                    format!("{h}d{u}n"),
                    format!("{h}d{u}ss{a}"),
                ],
            );
            has_none(
                lemma,
                &[
                    // The present's `k` in the potential, the passive past and
                    // the `-tu` participle's nominative.
                    format!("{k}nee"),
                    format!("{k}tiin"),
                    format!("{k}t{u}"),
                    // And the `d` in the past, which is the present's alone.
                    format!("{h}diin"),
                ],
            );
        }
    }

    /// The strong present stem carries the third and fourth infinitives, the
    /// present participle, the agent and the negative participle, while the
    /// second infinitive is the `h` stem with the type's own `de`.
    #[test]
    fn the_strong_present_stem_carries_the_infinitives() {
        for lemma in ["nähdä", "tehdä"] {
            let (h, _, strong, _, _, o, _) = t(lemma);
            has_all(
                lemma,
                &[
                    format!("{strong}m{}", if front(lemma) { "ä" } else { "a" }),
                    format!("{strong}minen"),
                    format!("{strong}v{}", if front(lemma) { "ä" } else { "a" }),
                    format!("{strong}m{}t{o}n", if front(lemma) { "ä" } else { "a" }),
                    // The second infinitive is the `h` stem's `de`.
                    format!("{h}de{}", if front(lemma) { "ssä" } else { "ssa" }),
                    format!("{h}den"),
                ],
            );
            has_none(
                lemma,
                &[
                    format!("{h}m{}", if front(lemma) { "ä" } else { "a" }),
                    format!("{h}v{}", if front(lemma) { "ä" } else { "a" }),
                    format!("{strong}de{}", if front(lemma) { "ssä" } else { "ssa" }),
                ],
            );
        }
    }
}
