//! The curated phrase and word tables.
//!
//! Both are small, sorted front-coded blobs built from
//! `data/style_phrases.csv` and `data/style_words.csv`. A key maps to two
//! values: the style kind and an optional replacement. The note column is
//! curation-only and is not shipped.

use crate::expand::static_table::StemMap;

use super::StyleKind;

static PHRASES: StemMap = StemMap::new(include_bytes!(concat!(
    env!("OUT_DIR"),
    "/style_phrases.bin"
)));
static WORDS: StemMap = StemMap::new(include_bytes!(concat!(env!("OUT_DIR"), "/style_words.bin")));

/// Longest phrase the matcher tries. Kept in step with the build writer, which
/// rejects a longer row.
pub(super) const MAX_PHRASE_WORDS: usize = 4;

/// One matched table row.
#[derive(Debug, Clone, Copy)]
pub(super) struct Entry<'a> {
    pub kind: StyleKind,
    pub replacement: Option<&'a str>,
}

/// Look up a lowercased, whitespace-normalised phrase.
pub(super) fn phrase(key: &str) -> Option<Entry<'static>> {
    lookup(&PHRASES, key)
}

/// Look up a lowercased single word.
pub(super) fn word(key: &str) -> Option<Entry<'static>> {
    lookup(&WORDS, key)
}

fn lookup(map: &'static StemMap, key: &str) -> Option<Entry<'static>> {
    let mut values = map.bases(key);
    let kind = kind_from_name(values.next()?)?;
    let replacement = values.next().filter(|value| !value.is_empty());
    Some(Entry { kind, replacement })
}

fn kind_from_name(name: &str) -> Option<StyleKind> {
    match name {
        "cliche" => Some(StyleKind::Cliche),
        "filler" => Some(StyleKind::Filler),
        "wordy" => Some(StyleKind::Wordy),
        "colloquial" => Some(StyleKind::Colloquial),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curated_phrase_carries_its_kind_and_replacement() {
        let entry = phrase("tällä hetkellä").expect("curated phrase");
        assert_eq!(entry.kind, StyleKind::Wordy);
        assert_eq!(entry.replacement, Some("nyt"));
    }

    #[test]
    fn a_phrase_without_a_replacement_is_label_only() {
        let entry = phrase("viime kädessä").expect("curated phrase");
        assert_eq!(entry.kind, StyleKind::Cliche);
        assert_eq!(entry.replacement, None);
    }

    #[test]
    fn a_curated_word_carries_its_kind_and_replacement() {
        let entry = word("systeemi").expect("curated word");
        assert_eq!(entry.kind, StyleKind::Colloquial);
        assert_eq!(entry.replacement, Some("järjestelmä"));
    }

    #[test]
    fn an_unknown_key_is_not_in_the_table() {
        assert!(phrase("tämä ei ole taulukossa").is_none());
        assert!(word("tavaton").is_none());
    }
}
