//! Class 29 (the lapsi type) inflection, written by hand.
//!
//! Class 29 is the ungradated `-si` family whose weak grade is plain `i -> e`
//! (`lapsi : lapsen`) and whose partitive collapses `ps -> s` (`lasta`). Its
//! four stems: strong S = lapsi, weak W = lapse (= essive stem E), and the
//! partitive stem P = las. It reuses the shared class-28 engine.
#![allow(dead_code)]

use crate::tags::Form;

use super::class_28J::{Stems, inflect_base};
use super::utility::e_stem;

pub const CLASS: &str = "29";

/// Inflect a class-29 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let weak = e_stem(lemma); // lapse
    let mut base = lemma.to_string();
    base.pop(); // laps
    // Collapse the trailing `ps` to `s`: remove the `p` (keeping laps -> las).
    let b = base.clone();
    let ptp = format!("{}{}", &b[..b.len() - 2], &b[b.len() - 1..]);
    let st = Stems {
        strong: lemma.to_string(),
        weak: weak.clone(),
        ptp,
        e: weak,
    };
    inflect_base(lemma, possessives, &st, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn ps_to_s() {
        let w = words("lapsi");
        assert!(w.contains(&"lapsen".to_string()));
        assert!(w.contains(&"lasta".to_string()));
        assert!(w.contains(&"lapsien".to_string()));
        assert!(w.contains(&"lasten".to_string()));
    }
}
