//! Class 47 (the allekirjoittanut type) inflection, written by hand.
//!
//! The `-nut`/`-nyt` participle substantives: the final `ut`/`yt` reads `ee`
//! (`allekirjoittanut -> allekirjoittanee`, `helpottunut -> helpottunee`), a
//! long `-ee` a-stem carrying the singular obliques and nominative plural and
//! whose short `-e` stem carries the plural obliques; the lemma keeps the
//! partitive (`allekirjoittanutta`). It reuses class 41's a-stem engine.
#![allow(dead_code)]

use crate::tags::Form;

use super::class_41::a_stem_inflect;

pub const CLASS: &str = "47";

/// Inflect a class-47 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    let long = format!("{}ee", &lemma[..lemma.len() - 2]);
    let short = &long[..long.len() - 1];
    a_stem_inflect(lemma, possessives, &long, short, false, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn nut() {
        let w = words("allekirjoittanut");
        assert!(w.contains(&"allekirjoittaneen".to_string()));
        assert!(w.contains(&"allekirjoittanutta".to_string()));
        assert!(w.contains(&"allekirjoittaneita".to_string()));
    }
}
