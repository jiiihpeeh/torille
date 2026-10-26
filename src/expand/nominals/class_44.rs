//! Class 44 (the kevät type) inflection, written by hand.
//!
//! The single `kevät` (spring): the `-ät` tail reads a long `-ää` a-stem
//! (`kevät -> kevää`/`kevä`), exactly class 41's shape, with the partitive on
//! the lemma (`kevättä`). It delegates to class 41.
#![allow(dead_code)]
use crate::tags::Form;
pub const CLASS: &str = "44";
/// Inflect a class-44 noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_41::inflect_singular(lemma, possessives)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kevat() {
        let w = inflect("kevät", true)
            .into_iter()
            .map(|f| f.word)
            .collect::<Vec<_>>();
        assert!(w.contains(&"kevään".to_string()));
        assert!(w.contains(&"kevättä".to_string()));
    }
}
