//! Class 49*J (the kannel type) inflection, written by hand.
//!
//! The `-el`/`-er`/`-ar` family that gradates `nn -> nt` in its obliques:
//! `kannel` is `kantele`/`kanteleen`, `piennar` is `pientare`/`pientareen`,
//! `tanner` is `tantere`/`tantereen`. It is exactly class 49's paradigm on the
//! gradated stem: the nominative and partitive singular keep the lemma
//! (`kannel`, `kannelta`), every oblique takes the gradated base (`kantel`,
//! `kantele`).
#![allow(dead_code)]

use crate::tags::Form;

pub const CLASS: &str = "49*J";

/// The gradated oblique base: the last `nn` reads `nt` (`kannel -> kantel`,
/// `piennar -> pientar`, `tanner -> tanter`).
fn gradated(lemma: &str) -> String {
    match lemma.rfind("nn") {
        Some(at) => {
            let mut s = lemma.to_string();
            s.replace_range(at..at + 2, "nt");
            s
        }
        None => lemma.to_string(),
    }
}

/// Inflect a class 49*J noun.
pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {
    super::class_49::inflect_with(lemma, &gradated(lemma), possessives)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(lemma: &str) -> Vec<String> {
        inflect(lemma, true).into_iter().map(|f| f.word).collect()
    }
    #[test]
    fn kannel() {
        let w = words("kannel");
        assert!(w.contains(&"kannel".to_string()));
        assert!(w.contains(&"kanteleen".to_string()));
        assert!(w.contains(&"kanteleiden".to_string()));
        assert!(w.contains(&"kannelta".to_string()));
    }
    #[test]
    fn piennar() {
        assert_eq!(gradated("piennar"), "pientar");
        assert_eq!(gradated("kannel"), "kantel");
    }
}
