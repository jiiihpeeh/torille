//! Personal pronouns and their genitive possessor forms.
//!
//! The personal pronouns are a closed paradigm, not derivable by the ordinary
//! noun rules: `minä` has the genitive `minun`, not `minän`. These genitive
//! forms are the ordinary way of making the possessor explicit, and they pair
//! with a possessive suffix on the head: `minun taloni` is "my house".
//!
//! The genitive and the suffix both carry the same person, so the possessor is
//! redundant with the suffix (`minun` already says first person singular and so
//! does `taloni`), but Finnish keeps both.

use crate::tags::Possessive;

/// The genitive form of each personal pronoun, with the possessive suffix it
/// pairs with.
///
/// The third person covers both singular and plural, matching the third-person
/// possessive suffix.
pub const PRONOUNS: &[(Possessive, &str, &str)] = &[
    (Possessive::FirstSingular, "minä", "minun"),
    (Possessive::SecondSingular, "sinä", "sinun"),
    (Possessive::Third, "hän", "hänen"),
    (Possessive::FirstPlural, "me", "meidän"),
    (Possessive::SecondPlural, "te", "teidän"),
    (Possessive::Third, "he", "heidän"),
];

/// The genitive possessor of `poss`, if it is a personal pronoun.
pub fn genitive(poss: Possessive) -> &'static str {
    PRONOUNS
        .iter()
        .find(|(p, _, _)| *p == poss)
        .map(|r| r.2)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pronoun_genitives_pair_with_the_suffixes() {
        // The third person is one form for both `hän` and `he`.
        assert_eq!(genitive(Possessive::FirstSingular), "minun");
        assert_eq!(genitive(Possessive::SecondSingular), "sinun");
        assert_eq!(genitive(Possessive::Third), "hänen");
        assert_eq!(genitive(Possessive::FirstPlural), "meidän");
        assert_eq!(genitive(Possessive::SecondPlural), "teidän");
    }

    #[test]
    fn the_explicit_possessor_precedes_the_head() {
        // `minun taloni` is the explicit possessor plus the suffixed head.
        let head = "taloni";
        assert_eq!(
            format!("{} {}", genitive(Possessive::FirstSingular), head),
            "minun taloni"
        );
    }
}
