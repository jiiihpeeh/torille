//! Text normalisation shared by the suggestion, morphology and base-finding
//! paths.
//!
//! The sanalista is lower case, and Finnish has three letters outside ASCII
//! (`ä`, `ö`, `å`) with a capital form for each. A fold that only maps ASCII
//! therefore splits a capitalised word from its own lexicon entry: `Äitj`
//! missed `äiti`, and `Ääkeihinsä` missed `ääki`. Everything that compares a
//! surface against a lemma must use this helper rather than
//! `str::to_ascii_lowercase`.

use std::borrow::Cow;

/// `surface` folded to lower case, borrowing when it has no upper-case
/// character.
///
/// The borrowing check is a character scan, so a lower-case word costs no
/// allocation. `to_lowercase` is Unicode-aware and can change the character
/// count (`İ` is `i` plus a combining dot), which is why the result is a
/// `Cow` rather than a slice.
pub(crate) fn lowercase(surface: &str) -> Cow<'_, str> {
    if surface.chars().any(char::is_uppercase) {
        Cow::Owned(surface.to_lowercase())
    } else {
        Cow::Borrowed(surface)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lowercase_word_is_borrowed() {
        assert!(matches!(lowercase("äiti"), Cow::Borrowed("äiti")));
        assert!(matches!(lowercase(""), Cow::Borrowed("")));
    }

    #[test]
    fn the_finnish_capitals_are_folded() {
        assert_eq!(lowercase("Äiti"), "äiti");
        assert_eq!(lowercase("ÖLJY"), "öljy");
        assert_eq!(lowercase("Åbo"), "åbo");
        assert_eq!(lowercase("STRAẞE"), "straße");
    }
}
