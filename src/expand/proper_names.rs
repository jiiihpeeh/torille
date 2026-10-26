//! Finnish proper names from Wikidata + Omorfi.
//!
//! `tools/build_proper_names.py` fetches the lemmas from Wikidata (Finnish
//! municipalities, cities, towns, villages, rivers, and the given/family
//! names of Finnish citizens). `tools/build_proper_name_forms.py` expands each
//! lemma to its singular cases with Omorfi.
//!
//! The result is a lowercase set: the typing path only needs to know that a
//! surface form is a name. It is front-coded and Huffman-coded (`HST1`), see
//! [`crate::expand::huffman_set`]. Names are **accepted, never suggested** --
//! they are not in the lemma set, so the suggestion ranking cannot offer them
//! as corrections.

use crate::expand::huffman_set::HuffmanSet;

/// Lowercase name forms, built by `build.rs` from
/// `data/proper_name_forms.csv`.
pub static PROPER_NAMES: HuffmanSet = HuffmanSet::new(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/precomputed/proper_names.bin",
)));

/// True if `word` is a known Finnish proper-name form.
///
/// The callers on the hot path already lowercase, so the first attempt is
/// allocation-free; a mixed-case caller pays one `to_lowercase`.
pub fn contains(word: &str) -> bool {
    if PROPER_NAMES.contains(word) {
        return true;
    }
    let lower = word.to_lowercase();
    lower != word && PROPER_NAMES.contains(&lower)
}

/// True if `word` is a known name form **and starts with a capital**.
///
/// This is the gate the checker uses. Foreign place names include words like
/// `Nice`, `Split` and `Reading`; accepting their lowercase spellings would
/// re-open the English false negatives this checker exists to avoid.
pub fn contains_capitalised(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase) && contains(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_names_and_cases_resolve() {
        for word in [
            "helsinki",
            "helsingin",
            "helsingissä",
            "matti",
            "matin",
            "korhonen",
            "korhosen",
        ] {
            assert!(contains(word), "{word} should be a name");
        }
    }

    #[test]
    fn moomin_and_kanteletar_names_resolve() {
        for word in [
            "Kanteletar",
            "Kantelettaren",
            "Muumi",
            "Muumin",
            "Muumipeikko",
            "Muumipeikon",
            "Muumimamma",
            "Muumimamman",
            "Muumipappa",
            "Muumipapan",
            "Niiskuneiti",
            "Niiskuneidin",
            "Niiskuneitiä",
            "Niisku",
            "Niiskun",
            "Nipsu",
            "Nipsun",
            "Haisuli",
            "Haisulin",
            "Nuuskamuikkunen",
            "Nuuskamuikkusen",
            "Tiuhti",
            "Tiuhtin",
            "Viuhti",
            "Viuhtin",
            "Hemuli",
            "Hemulin",
            "Mörkö",
            "Mörön",
            "Vilijonkka",
            "Vilijonkan",
            "Muumilaakso",
            "Muumilaakson",
            "Muumitalo",
            "Muumitalon",
            "Mymmeli",
            "Mymmelin",
            "Mymmeliä",
        ] {
            assert!(contains(word), "{word} should be a known name form");
        }
    }

    #[test]
    fn english_words_are_not_names() {
        for word in ["this", "the", "xyzzy"] {
            assert!(!contains(word), "{word} should not be a name");
        }
    }

    #[test]
    fn lowercase_names_are_not_accepted() {
        // `Mark` is a place and `Bill` a given name in the list; the
        // lowercase English words must stay misspellings.
        assert!(contains_capitalised("Helsinki"));
        assert!(!contains_capitalised("helsinki"));
        assert!(contains_capitalised("Mark"));
        assert!(!contains_capitalised("mark"));
    }
}
