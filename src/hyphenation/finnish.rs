//! Finnish typographic policy helpers.
//!
//! The pattern set and the morphology layer produce *candidates*. This
//! module holds the purely orthographic questions the policy asks about
//! a candidate: is it between two vowels, does the next fragment begin
//! with a vowel, and how many characters are on each side.

use crate::is_vowel;

use super::offsets::char_count;

/// Whether the boundary at `byte_offset` sits between two vowels.
pub(crate) fn vowel_vowel(word: &str, byte_offset: usize) -> bool {
    if byte_offset == 0 || byte_offset >= word.len() || !word.is_char_boundary(byte_offset) {
        return false;
    }
    let left = word[..byte_offset].chars().next_back();
    let right = word[byte_offset..].chars().next();
    matches!((left, right), (Some(a), Some(b)) if is_vowel(a) && is_vowel(b))
}

/// Whether the fragment after `byte_offset` starts with a vowel.
pub(crate) fn next_fragment_starts_with_vowel(word: &str, byte_offset: usize) -> bool {
    word.get(byte_offset..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(is_vowel)
}

/// How many characters are on each side of a byte offset.
pub(crate) fn fragment_lengths(word: &str, byte_offset: usize) -> Option<(usize, usize)> {
    if byte_offset > word.len() || !word.is_char_boundary(byte_offset) {
        return None;
    }
    Some((
        char_count(&word[..byte_offset]),
        char_count(&word[byte_offset..]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vowel_boundaries_are_detected() {
        assert!(!vowel_vowel("asia", 1)); // a|s is not vowel-vowel
        assert!(!vowel_vowel("asia", 2)); // s|i is not vowel-vowel
        assert!(vowel_vowel("asia", 3)); // i|a is
        assert!(vowel_vowel("kofeiini", 4)); // e|i
        assert!(!vowel_vowel("matka", 3)); // t|k
    }

    #[test]
    fn fragment_lengths_are_in_characters() {
        assert_eq!(fragment_lengths("Äänekoski", 4), Some((2, 7)));
    }
}
