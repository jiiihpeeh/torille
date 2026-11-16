//! Class 9*D (the aika/vika type): class 9's `-a` family with gradation of a
//! single `k`. The weak grade is `k -> j` for the `-aika` (time) compounds,
//! which contract `ai + j` to `aj` (`aika -> aja`, `adventtiaika ->
//! adventtiaja`); it is `k -> (zero)` for the other single-`k` words (`vika ->
//! vian`, `laki -> lain`, `jalka -> jalan`). `taika` alone ends in `-aika` yet
//! is `taian` (k -> zero).
#![allow(dead_code)]
use crate::kotus::Gradation;
use crate::tags::Form;
pub const CLASS: &str = "9*D";

/// Inflect a class 9*D noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    // `nahka` is `9*(D)`: Wiktionary gives both the graded table (`nahan`,
    // `nahoissa`, `nahoitta`) and the ungraded one (`nahkan`, `nahkoissa`,
    // `nahkoitta`). The sanalista override keeps the graded table as the
    // standard one, so add the ungraded table beside it rather than choosing
    // between two real paradigms.
    if lemma == "nahka" {
        let mut out = super::class_9::inflect(lemma, possessives);
        out.extend(super::class_9::inflect_graded(
            lemma,
            possessives,
            Gradation(4),
        ));
        let mut seen = std::collections::HashSet::new();
        out.retain(|form| seen.insert(form.word.clone()));
        return out;
    }
    if lemma.ends_with("aika") && lemma != "taika" {
        // aika -> aja: drop the final `aika` and write `aja`.
        let head = &lemma[..lemma.len() - 4];
        let weak = format!("{head}aja");
        super::class_9::inflect_graded_with(lemma, possessives, &weak)
    } else {
        super::class_9::inflect_graded(lemma, possessives, Gradation(4))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }

    /// Wiktionary gives both the graded and the ungraded `nahka` table.
    #[test]
    fn nahka_keeps_both_tables() {
        let w = words("nahka");
        for want in ["nahan", "nahoissa", "nahoitta", "nahkan", "nahkoissa", "nahkoitta"] {
            assert!(w.contains(&want.to_string()), "missing {want}");
        }
    }
}
