//! A pre-bucketed prefix → lemma index for fast first-pass inflection
//! lookups.
//!
//! [`find_bases_with`] does the same work at runtime: scan the
//! sanalista for lemmas whose first 3-4 chars match the query word's,
//! inflect each candidate, and check if the form matches. The cost is
//! dominated by the `inflect_all` calls (a few ms each), not the
//! scan. Pre-bucketing the lemmas by prefix turns the scan into a
//! `HashMap` lookup, but the `inflect_all` cost remains.
//!
//! Build: one pass over the sanalista bucketing each lemma by its
//! first [`PREFIX_LEN`] characters. No inflection happens at build
//! time -- the bucket is just `Vec<u32>` of lemma indices -- so the
//! build is a few hundred milliseconds for the noun partition.
//!
//! Lookup: for a query word, take the first [`PREFIX_LEN`] chars as
//! the bucket key. For each candidate lemma, run [`inflect_all`] and
//! check whether any generated form equals the query. The bucket size
//! for a 3-char prefix averages ~10 lemmas; `inflect_all` is sub-ms,
//! so the per-word cost is single-digit ms.
//!
//! This is a *filter*, not an answer. The bucket key is the lemma's
//! first 3 chars, which catches `talo` + `-ssa`, `talo` + `-ja`, ...
//! but misses gradated forms like `veteen` (lemma `vesi`, prefix
//! `ves` ≠ `vet`). The existing `find_bases_with` clitic phase
//! plus the per-textbox slow check catch the misses; this index is
//! the fast common-case lane the typing path uses.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::expand::inflect_all;
use crate::kotus::{parse_sanalista, Lemma};
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

const PREFIX_LEN: usize = 3;

/// A pre-bucketed prefix → lemma index.
///
/// The bucket key is the first [`PREFIX_LEN`] characters of the
/// lemma's `word` field (the sanalista spelling). Both 3- and 4-char
/// buckets were tried; 3 is the smaller candidate set without
/// sacrificing coverage for the common case.
pub struct PrefixIndex {
    by_prefix: HashMap<String, Vec<u32>>,
}

impl PrefixIndex {
    /// Build the index. One pass over the lexicon, bucketing each
    /// lemma by its first [`PREFIX_LEN`] characters. The bucket
    /// key is the lowercased form; the sanalista is already
    /// lowercased, so this is a slice.
    pub fn build(lexicon: &[Lemma]) -> Self {
        let mut by_prefix: HashMap<String, Vec<u32>> =
            HashMap::with_capacity(26_usize.pow(PREFIX_LEN as u32));
        for (i, lemma) in lexicon.iter().enumerate() {
            if lemma.word.is_empty() {
                continue;
            }
            let key: String = lemma.word.chars().take(PREFIX_LEN).collect();
            by_prefix.entry(key).or_default().push(i as u32);
        }
        Self { by_prefix }
    }

    /// The candidate lemma indices for `word`. The first [`PREFIX_LEN`]
    /// characters of `word` are the bucket key; if `word` is
    /// shorter, the empty bucket is returned.
    pub fn candidates_for(&self, word: &str) -> &[u32] {
        if word.chars().count() < PREFIX_LEN {
            return &[];
        }
        let key: String = word.chars().take(PREFIX_LEN).collect();
        self.by_prefix.get(&key).map(Vec::as_slice).unwrap_or(&[])
    }
}

static PREFIX_INDEX: OnceLock<PrefixIndex> = OnceLock::new();

/// The pre-bucketed prefix index, built once on first access. Cheap
/// to build (a few hundred ms for the noun partition) and shared
/// thereafter. The wasm's first pass reads this to find candidate
/// lemmas, then runs `inflect_all` per candidate.
pub fn prefix_index() -> &'static PrefixIndex {
    PREFIX_INDEX.get_or_init(|| {
        let lexicon = parse_sanalista(crate::SANALISTA);
        PrefixIndex::build(&lexicon)
    })
}

/// The candidates the index returns for `word`, with the
/// [`inflect_all`] check. Returns `Some(form)` if any candidate
/// inflects to `word`; `None` otherwise. This is the per-word cost
/// the first pass pays for words the lemma + noun-case stripper
/// missed.
///
/// Cost: O(bucket_size) `inflect_all` calls, ~1-3 ms each. With
/// 3-char buckets averaging ~10 lemmas for the noun partition, a
/// lookup is ~10-30 ms -- well under the slow per-word check
/// (`find_bases_with` itself, 50-200 ms) and the right shape for the
/// typing path.
pub fn inflection_candidate_for(
    word: &str,
    pos: PartOfSpeech,
    possessives: bool,
) -> Option<String> {
    let idx = prefix_index();
    let candidates = idx.candidates_for(word);
    if candidates.is_empty() {
        return None;
    }
    let cat = Catalogues::embedded().clone();
    let lexicon = parse_sanalista(crate::SANALISTA);
    for &row in candidates {
        let lemma = &lexicon[row as usize];
        for form in inflect_all(lemma.word, &lemma.inflections, &cat, None, pos, possessives) {
            if form.word == word {
                return Some(lemma.word.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::PartOfSpeech as Pos;

    /// The prefix index is a fast lane for the common case: a word
    /// whose first 3 chars match a lemma's first 3 chars. `talo` and
    /// its productive cases land in the `tal` bucket; the lookup
    /// inflects each candidate and accepts on match.
    #[test]
    fn prefix_index_finds_talo_inflections() {
        for w in &["talo", "taloa", "talossa", "taloon", "taloja"] {
            let lemma = inflection_candidate_for(w, Pos::Noun, false);
            assert_eq!(
                lemma.as_deref(),
                Some("talo"),
                "{w}: should map to lemma talo via prefix index, got {lemma:?}",
            );
        }
    }

    /// `veden` is the genitive of `vesi`. The strong stem prefix is
    /// `ves`, the weak is `ved`; only the lemma's first 3 chars
    /// (`ves`) sit in the index bucket. The `ved` lookup misses the
    /// bucket, which is the documented gap -- the prefix index is a
    /// fast common-case lane, not a complete classifier.
    #[test]
    fn prefix_index_misses_gradated_forms() {
        let lemma = inflection_candidate_for("veden", Pos::Noun, false);
        assert!(
            lemma.is_none(),
            "veden: prefix index should miss (vesi bucketed under 'ves', not 'ved')"
        );
    }
}