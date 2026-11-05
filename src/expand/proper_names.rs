//! Finnish proper names from Wikidata + Omorfi.
//!
//! `tools/build/build_proper_names.py` fetches the lemmas from Wikidata (Finnish
//! municipalities, cities, towns, villages, rivers, and the given/family
//! names of Finnish citizens). `tools/oracle/build_proper_name_forms.py` expands each
//! lemma to its singular cases with Omorfi.
//!
//! The result is a lowercase set: the typing path only needs to know that a
//! surface form is a name. It is front-coded and Huffman-coded (`HST1`), see
//! [`crate::expand::huffman_set`]. Names are **accepted, never suggested** --
//! they are not in the lemma set, so the suggestion ranking cannot offer them
//! as corrections.

use crate::expand::huffman_set::HuffmanSet;
use crate::expand::static_table::StemMap;

/// Lowercase name forms, built by `build.rs` from
/// `data/proper_name_forms.csv`.
pub static PROPER_NAMES: HuffmanSet = HuffmanSet::new(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/maps/proper_names.bin",
)));

/// `form -> name` for the same source, so a caller can name the lemma behind
/// an inflected form (`mainen` -> `Maine`, `helsingin` -> `Helsinki`).
static PROPER_NAME_LEMMAS: StemMap = StemMap::new(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/maps/proper_name_lemmas.bin",
)));

/// The proper-name lemma a form belongs to, preserving the source's
/// capitalisation (`Mainen` -> `Maine`).
///
/// The form is matched case-insensitively: the typing path lowercases, while a
/// sentence-initial caller passes the capitalised spelling. A form that maps
/// to several names (`Maine` is both the US state and a given name) returns
/// the first in sorted order; the engine does not model name senses.
pub fn lemma(word: &str) -> Option<&'static str> {
    lemmas(word).into_iter().next()
}

/// Every proper-name lemma a form belongs to, preserving capitalisation.
///
/// A surface can be several names (`Maine` is both a state and a given name),
/// and it is also often a common word (`maine` = reputation, `mainen` =
/// adjective). The caller merges this list with the common-word readings; the
/// engine does not pick one meaning for it.
pub fn lemmas(word: &str) -> Vec<&'static str> {
    let direct = surface_lemmas(word);
    if !direct.is_empty() {
        return direct;
    }
    // A clitic or possessive on a name (`Annakin`, `Annani`) still names the
    // same lemma; the proper-name table holds the bare cases only.
    let mut current = word.to_string();
    while let Some(host) = next_host(&current) {
        let found = surface_lemmas(&host);
        if !found.is_empty() {
            return found;
        }
        current = host;
    }
    Vec::new()
}

/// The lemmas of a surface with no clitic/possessive stripping.
fn surface_lemmas(word: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = PROPER_NAME_LEMMAS.bases(word).collect();
    let lower = word.to_lowercase();
    if lower != word {
        for name in PROPER_NAME_LEMMAS.bases(&lower) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// One post-inflectional suffix off `word`, or `None`.
fn next_host(word: &str) -> Option<String> {
    if let Some((host, _clitic)) = crate::expand::clitic::strip_clitic(word) {
        return Some(host);
    }
    crate::expand::check_normal::POSSESSIVE_SUFFIXES
        .iter()
        .find_map(|suffix| {
            word.strip_suffix(suffix)
                .filter(|host| !host.is_empty())
                .map(str::to_string)
        })
}

fn surface_contains(word: &str) -> bool {
    if PROPER_NAMES.contains(word) || PROPER_NAME_LEMMAS.contains(word) {
        return true;
    }
    let lower = word.to_lowercase();
    lower != word && (PROPER_NAMES.contains(&lower) || PROPER_NAME_LEMMAS.contains(&lower))
}

/// True if `word` is a known Finnish proper-name form, including a clitic or
/// possessive on a known name (`Annakin`, `Tolpankin`, `Annani`).
///
/// The callers on the hot path already lowercase, so the first attempt is
/// allocation-free; a mixed-case caller pays one `to_lowercase`.
pub fn contains(word: &str) -> bool {
    if surface_contains(word) {
        return true;
    }
    let mut current = word.to_string();
    while let Some(host) = next_host(&current) {
        if surface_contains(&host) {
            return true;
        }
        current = host;
    }
    false
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
    fn religious_names_and_their_cases_resolve() {
        for word in [
            "Jumala",
            "Jumalan",
            "Jumalalla",
            "Jeesus",
            "Jeesuksen",
            "Jeesukselle",
        ] {
            assert!(contains_capitalised(word), "{word} should be a name");
        }
    }

    #[test]
    fn a_house_name_that_collides_with_a_common_noun_resolves() {
        // `tolppa` is the common noun "pole"; `Tolppa` is the house name in
        // the book (`Tolpan Anna`). The curated forms keep the capital.
        for word in ["Tolppa", "Tolpan", "Tolppaa", "Tolpassa"] {
            assert!(contains_capitalised(word), "{word} should be a name");
        }
    }

    #[test]
    fn clitics_and_possessives_on_names_resolve() {
        for word in ["Annakin", "Annani", "Annansa", "Tolpankin", "Matin"] {
            assert!(contains_capitalised(word), "{word} should be a name");
        }
    }

    #[test]
    fn a_region_that_collides_with_a_common_noun_resolves() {
        // `pohjanmaa` is also a common noun; `Pohjanmaa` is the region.
        for word in ["Pohjanmaa", "Pohjanmaan", "Pohjanmaassa", "Pohjanmaalle"] {
            assert!(contains_capitalised(word), "{word} should be a name");
        }
    }

    #[test]
    fn finnish_regions_resolve() {
        for word in [
            "Ahvenanmaa",
            "Ahvenanmaalla",
            "Uusimaa",
            "Uudenmaan",
            "Pirkanmaa",
            "Pirkanmaan",
            "Varsinais-Suomi",
            "Varsinais-Suomen",
            "Kanta-Häme",
            "Kanta-Hämeen",
            "Kainuu",
            "Kainuun",
            "Itä-Uusimaa",
        ] {
            assert!(contains_capitalised(word), "{word} should be a name");
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
