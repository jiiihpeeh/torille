//! Class 5*F (the ahti type) \u2014 class 5's `-i` family with t -> d.
//! The graded class-5 engine in `class_5` does the work; this module only
//! fixes the gradation type.

#![allow(dead_code)]

use crate::kotus::{Gradation, Grade};
use crate::tags::Form;

use super::utility::plural_only;

pub const CLASS: &str = "5*F";

/// Inflect a class 5*F noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // A `-t`-final lemma is a plural-only citation (`pihdit`, `bileet`).
    // The plural nominative carries the weak grade and the final `-t`
    // (`pihdit`), so drop the `-t`, turn the weak `d` back to strong `t`
    // (`pihdi` -> `pihti`), decline the singular comparative base and keep
    // only the plural cells. Wiktionary 5*F `pihdit`: `pihtien`, `pihtejä`,
    // `pihteihin`, `pihdeissä`, ...; there is no singular cell.
    if let Some(base) = lemma.strip_suffix('t')
        && !base.is_empty()
    {
        let strong = Gradation(6).strongen(base);
        return plural_only(super::class_5::inflect_graded(
            &strong,
            possessives,
            Gradation(6),
        ));
    }
    // `arkkitehti` is genuinely both-harmonic: Wiktionary gives both the
    // `{{fi-decl-risti|arkkiteh|t|d|a}}` and the `ä` declension, so
    // `arkkitehtia`/`arkkitehtiä` and `arkkitehtiensa`/`arkkitehtiensä` are
    // both real. Emit both columns; the shared endings deduplicate below.
    if lemma == "arkkitehti" {
        let mut out = super::class_5::inflect_graded_with_harmony(
            lemma,
            possessives,
            Gradation(6),
            Some(false),
        );
        out.extend(super::class_5::inflect_graded_with_harmony(
            lemma,
            possessives,
            Gradation(6),
            Some(true),
        ));
        let mut seen = std::collections::HashSet::new();
        out.retain(|form| seen.insert(form.word.clone()));
        return out;
    }
    super::class_5::inflect_graded(lemma, possessives, Gradation(6))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// `arkkitehti` is both-harmonic in Wiktionary: the `a` and `ä`
    /// `risti` declensions are both given, so both columns are plain.
    #[test]
    fn arkkitehti_emits_both_harmonies() {
        let w = words("arkkitehti");
        for expected in [
            "arkkitehtia",
            "arkkitehtiä",
            "arkkitehteja",
            "arkkitehtejä",
            "arkkitehdilla",
            "arkkitehdillä",
            "arkkitehtiensa",
            "arkkitehtiensä",
        ] {
            assert!(w.contains(&expected.to_string()), "missing {expected}");
        }
        let mut seen = std::collections::HashSet::new();
        for word in &w {
            assert!(seen.insert(word), "duplicate {word}");
        }
    }

    /// Wiktionary `pihdit` (Kotus 5*F, plural only): no singular cells, and
    /// the partitive is `pihtejä`, not the loanword `pihdejä` and not the
    /// broken `pihdid...` shapes the class used to build from the citation.
    #[test]
    fn the_plural_only_citation_declines_from_the_dropped_t_stem() {
        let w = words("pihdit");
        for expected in [
            "pihdit",
            "pihtien",
            "pihtejä",
            "pihteihin",
            "pihdeissä",
            "pihdeistä",
            "pihdeillä",
            "pihdeiltä",
            "pihdeille",
            "pihteinä",
            "pihdeiksi",
            "pihdeittä",
            "pihdein",
        ] {
            assert!(w.contains(&expected.to_string()), "missing {expected}");
        }
        assert!(
            !w.iter().any(|word| word.starts_with("pihdid")),
            "the citation's weak d leaked into the stem: {w:?}"
        );
        assert!(
            !w.iter().any(|word| word == "pihti"),
            "a plural-only word must not emit a singular nominative"
        );
    }
}
