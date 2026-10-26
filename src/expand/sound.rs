use crate::is_vowel;

/// Finnish phonotactic constraints used to filter speculative forms.
pub fn is_phonotactic(word: &str) -> bool {
    let mut run = 0usize;
    let mut last: Option<char> = None;
    let mut same = 0usize;
    for c in word.chars() {
        if !c.is_alphabetic() {
            continue;
        }
        if is_vowel(c) {
            run += 1;
            // `valtioon` and `korkeaa` are words, so three in a row is allowed
            // and only a longer run marks a generation gone wrong.
            if run > 3 {
                return false;
            }
        } else {
            run = 0;
        }
        if Some(c) == last {
            same += 1;
            // Three of the same consonant never occurs in a Finnish word.
            if same > 2 {
                return false;
            }
        } else {
            same = 1;
        }
        last = Some(c);
    }
    true
}

/// Whether a byte starts a Finnish vowel.
///
/// The split points are byte offsets rather than character indices, so the
/// caller often only has the first byte of a two-byte `ä` in hand, and a
/// character test would miss it.
///
/// Every two-byte Finnish vowel leads with `0xc3` — `ä`, `ö`, `å` and their
/// capitals alike — so that byte is the whole of the two-byte case, and
/// `0xc4`, `0xc5` and `0xd6` are not Finnish vowels: they lead the Latin
/// Extended-A letters and the combining diacriticals instead. A lone `0xc3`
/// cannot be told apart from the `Ã` of a Portuguese word, which is a
/// limitation of having one byte, and costs nothing because the stems and
/// endings asked about here come out of the catalogue, which is lower case and
/// free of accents beyond `ä` and `ö`.
pub(super) fn is_vowel_byte(c: Option<u8>) -> bool {
    matches!(
        c,
        Some(b'a' | b'e' | b'i' | b'o' | b'u' | b'y' | b'A' | b'E' | b'I' | b'O' | b'U' | b'Y')
    ) || matches!(c, Some(0xc3))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one-byte vowels, and the leading byte of each two-byte one.
    #[test]
    fn a_vowel_byte_is_recognised() {
        for c in ['a', 'e', 'i', 'o', 'u', 'y', 'A', 'E', 'I', 'O', 'U', 'Y'] {
            assert!(is_vowel_byte(Some(c as u8)), "{c} read as a consonant");
        }
        for c in ['ä', 'ö', 'å'] {
            let mut buf = [0u8; 4];
            assert!(
                is_vowel_byte(c.encode_utf8(&mut buf).as_bytes().first().copied()),
                "{c} read as a consonant"
            );
        }
        assert!(!is_vowel_byte(Some(b'n')));
        assert!(!is_vowel_byte(None));
    }

    /// `0xc4` and `0xc5` lead the Latin Extended-A letters, and `0xd6` leads
    /// the combining diacriticals. None of them is a Finnish vowel.
    #[test]
    fn a_non_finnish_leading_byte_is_not_a_vowel() {
        assert!(!is_vowel_byte(Some(0xc4))); // `ā`
        assert!(!is_vowel_byte(Some(0xc5))); // `Ņ`
        assert!(!is_vowel_byte(Some(0xd6)));
        assert!(!is_vowel_byte(Some(0xe3))); // `ã`
    }

    /// A word is kept or dropped on the whole string, so a `å` or a capital
    /// is a vowel here rather than something to reject the word over.
    #[test]
    fn the_widened_vowel_set_survives_the_phonotactic_filter() {
        for c in ['å', 'Å', 'A', 'Ö'] {
            assert!(is_vowel(c), "{c} is not read as a vowel");
        }
        assert!(is_phonotactic("Grå"));
        assert!(is_phonotactic("Åland"));
        // Still a run of four, whichever letters it is made of.
        assert!(!is_phonotactic("ÅÄÖa"));
    }
}
