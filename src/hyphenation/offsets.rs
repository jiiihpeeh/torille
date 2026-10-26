//! UTF-8-safe offset conversion.
//!
//! Hyphenation works in character positions because patterns are
//! character-based, but callers slice Rust strings by byte. Every
//! conversion goes through these helpers so a break can never land
//! inside a multi-byte character.

/// Byte offset of the character at `char_offset`, or `None` when the
/// offset is past the end.
pub(crate) fn char_to_byte(text: &str, char_offset: usize) -> Option<usize> {
    if char_offset == text.chars().count() {
        return Some(text.len());
    }
    text.char_indices().nth(char_offset).map(|(byte, _)| byte)
}

/// Character offset of a byte offset, or `None` when `byte_offset` is
/// not a character boundary.
pub(crate) fn byte_to_char(text: &str, byte_offset: usize) -> Option<usize> {
    if byte_offset > text.len() || !text.is_char_boundary(byte_offset) {
        return None;
    }
    Some(text[..byte_offset].chars().count())
}

/// Character count, kept in one place so the policy and the helpers
/// cannot disagree about what a fragment length means.
pub(crate) fn char_count(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_byte_offsets_round_trip() {
        let word = "Äänekoski";
        for (char_offset, (byte_offset, _)) in word.char_indices().enumerate() {
            assert_eq!(char_to_byte(word, char_offset), Some(byte_offset));
            assert_eq!(byte_to_char(word, byte_offset), Some(char_offset));
        }
        assert_eq!(char_to_byte(word, char_count(word)), Some(word.len()));
    }

    #[test]
    fn an_offset_inside_a_character_is_rejected() {
        let word = "ä";
        assert_eq!(byte_to_char(word, 1), None);
    }
}
