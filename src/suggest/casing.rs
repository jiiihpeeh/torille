//! Surface-case policy shared by the checker and the suggestion popup.
//!
//! The lexicon is stored lowercase, so matching is case-insensitive. That
//! is the right thing for finding the lemma, but not for deciding whether
//! the *written* form is acceptable:
//!
//!   * `Talo` mid-sentence is a capitalization error;
//!   * `Talo` at the start of a sentence is normal;
//!   * `Helsinki` is a proper name and must keep its capital;
//!   * `turkulainen` is a common adjective derived from `Turku` and is
//!     correctly lowercase.
//!
//! [`case_is_valid`] encodes that policy. It assumes the caller has already
//! decided that the lowercase form is a known word; it only judges the
//! capitalization. [`match_case`] performs the other half: it restores the
//! input's capitalization on a suggestion, so `Tlao` suggests `Talo`
//! instead of `talo`.

/// Restore the input's capitalization pattern on `candidate`.
///
/// An all-caps input (`TALO`) keeps all caps; a title-case input (`Tlao`)
/// capitalizes the candidate's first character; anything else is left
/// alone. Mixed-case inputs such as `iPhone` are also left alone -- the
/// engine has no way to reconstruct an arbitrary internal capitalization.
pub fn match_case(input: &str, candidate: &str) -> String {
    let Some(first) = input.chars().next() else {
        return candidate.to_string();
    };
    if !first.is_uppercase() {
        return candidate.to_string();
    }
    let rest: Vec<char> = input.chars().skip(1).collect();
    if !rest.is_empty() && rest.iter().all(|c| !c.is_lowercase()) {
        return candidate.to_uppercase();
    }
    let mut chars = candidate.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::with_capacity(candidate.len());
    out.extend(first.to_uppercase());
    out.push_str(chars.as_str());
    out
}

/// Capitalize the first character of `word`, leaving the rest alone.
pub(crate) fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::with_capacity(word.len());
    out.extend(first.to_uppercase());
    out.push_str(chars.as_str());
    out
}

/// Is the capitalization of `word` acceptable?
///
/// `sentence_initial` says whether the token starts a sentence.
/// `proper` says whether the word is a known proper name (the embedded
/// name set or a name learned from the text). The caller decides whether
/// the lowercase form is a known common word.
///
/// Lowercase and mixed-case words are accepted here; the lexicon decides
/// whether they exist. A title-case word is accepted only at a sentence
/// start or as a proper name -- that is what makes `Talo` mid-sentence a
/// mistake while `Talo` at the start of a sentence is fine. All-caps and
/// internally mixed-case words (`TALO`, `McDonald`) are left to the
/// lexicon: they are typographic choices, not ordinary title case.
pub fn case_is_valid(word: &str, sentence_initial: bool, proper: bool) -> bool {
    let Some(first) = word.chars().next() else {
        return true;
    };
    if !first.is_uppercase() {
        return true;
    }
    let rest: Vec<char> = word.chars().skip(1).collect();
    if rest.iter().all(|c| !c.is_lowercase()) {
        // All caps, including a one-letter word.
        return true;
    }
    if rest.iter().any(|c| c.is_uppercase()) {
        // Internal capitals (`McDonald`, `iPhone`, `DNAase`).
        return true;
    }
    proper || sentence_initial
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_follow_the_input_case() {
        assert_eq!(match_case("Tlao", "talo"), "Talo");
        assert_eq!(match_case("TALO", "talo"), "TALO");
        assert_eq!(match_case("Helsinxi", "helsinki"), "Helsinki");
        assert_eq!(match_case("tlao", "talo"), "talo");
    }

    #[test]
    fn title_case_is_a_sentence_start_or_a_proper_name() {
        assert!(case_is_valid("Talo", true, false));
        assert!(!case_is_valid("Talo", false, false));
        assert!(case_is_valid("Helsinki", false, true));
        assert!(case_is_valid("TALO", false, false));
    }

    #[test]
    fn a_lowercase_derived_adjective_is_fine() {
        // `Turku` is a name, but the derived adjective `turkulainen` is
        // not; capitalizing it mid-sentence is the mistake.
        assert!(case_is_valid("turkulainen", false, false));
        assert!(!case_is_valid("Turkulainen", false, false));
    }
}
