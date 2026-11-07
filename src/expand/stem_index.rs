//! A reverse-stem index for fast "is this a real inflected form?"
//! lookups when the form set misses.
//!
//! The form set (`all_forms`) is the comprehensive "is this a real
//! Finnish word?" gate: every surface form the engine can produce
//! is in a `HashSet<String>`, O(1) probe. For a real word like
//! `veden` the form set hits and the slow check never fires.
//!
//! The slow check (gate 4 in `check_normal`, the wasm's `check_fast`
//! per-misspell demote) is the fallback for words the form set
//! *missed*: typos, irregular forms, clitic variants the engine
//! doesn't generate. The current implementation walks the prefix
//! space (3097 candidates for a `vet*` prefix) and runs
//! `inflect_all` on each until a match is found -- 5-50 ms per
//! word, 50x slower than the form set.
//!
//! The reverse-stem index is a different answer to the same
//! question. For `veden`, the relevant morphological split is
//! `vede + n` (the weak stem plus the genitive singular ending).
//! The engine already knows the stems and endings because that's
//! how it produces the form; we materialise the stem-side of the
//! mapping as a `HashMap<String, HashSet<usize>>` keyed by stem.
//! At lookup, the slow check enumerates the common Finnish case
//! endings (~30), strips each from the word, and probes the index
//! for the resulting stem. A hit returns a small candidate set;
//! the slow check then `inflect_all`s each candidate to verify
//! the form came from that lemma.
//!
//! Build cost: the same `inflect_all` walk the form set uses, plus
//! the stem-extraction step (try every ending for every form). For
//! each form we may index 3-5 (stem, lemma) pairs, but most of
//! them collapse to the same deduped set per lemma, so the final
//! size is bounded by `lemmas × distinct-stems-per-lemma`,
//! typically 3-5 entries per lemma for the noun partition. With
//! 100 k lemmas that's ~500 k entries -- an order of magnitude
//! smaller than the 13.6 M-entry form set.
//!
//! Lookup cost: O(ENDINGS) hashmap probes (≈ 30), plus 1
//! `inflect_all` per candidate (typically 1). ≈ 1 ms per word on
//! a desktop, vs 5-50 ms for the prefix walk.
//!
//! **No shortcuts in `is_known_word`.** This index is the engine's
//! own data, derived from the same paradigm walk the form set
//! uses. The strip-and-probe loop is "is there *any* Finnish
//! lemma whose paradigm emits this word?" -- the same question
//! `inflect_all` answers, but indexed for the inverse direction.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::expand::inflect_all;
use crate::kotus::parse_sanalista;
use crate::spec::Catalogues;
use crate::tags::{Confidence, PartOfSpeech};

/// Common Finnish case / number / clitic endings, longest first.
///
/// Each entry is a string the engine can produce as a suffix. The
/// list is curated, not class-specific: it covers every productive
/// case ending (the 8 catalogue slots + the 4 cases the catalogue
/// omits), the genitive and partitive suffixes, the plural set,
/// and a handful of clitic forms. New endings can be appended
/// without rebuilding anything but the index (one pass of
/// `inflect_all` + this list).
const ENDINGS: &[&str] = &[
    // 5-char (longest first so a 5-char match wins over a 4-char
    // one for "elative pl" / etc.)
    "isiin",
    // 4-char
    "kaan", "kään", "seen", "hon", "hön", "jaa", "jää", "isi",
    "ihin", "issa", "istä", "illa", "iltä", "ille", "iksi",
    "tta", "ttä",
    // 3-char
    "ssa", "ssä", "sta", "stä", "lla", "llä", "lle", "lta", "ltä",
    "ksi",
    // 2-char
    "han", "hän", "kin", "na", "nä", "ta", "tä", "ja", "jä",
    "ia", "iä",
    "en", "an", "än", "in", "on", "ön", "un", "yn",
    // 1-char
    "n", "t",
    // partitive singular: `-a` / `-ä` (`talo` -> `taloa`,
    // `vesi` -> `vettä` is caught by the longer `ttä` above, but
    // `tuoli` -> `tuolia` / `vesi` -> `vesiä` need the bare vowel)
    "a", "ä",
];

/// A `stem -> {lemma indices}` reverse-stem index.
///
/// Build it once with [`stem_index`] and share via the `OnceLock`.
/// Lookups are O(ENDINGS) hashmap probes plus a verify call per
/// candidate.
pub struct StemIndex {
    by_stem: HashMap<String, HashSet<usize>>,
}

impl StemIndex {
    /// Build the index from the noun partition. The walk is the
    /// same `inflect_all` pass the form set uses; for every form
    /// generated, we try every ending on the ENDINGS list, derive
    /// the stem, and add (stem, lemma_index) to the set.
    pub fn build(lexicon: &[crate::kotus::Lemma]) -> Self {
        let cat = Catalogues::embedded().clone();
        let mut by_stem: HashMap<String, HashSet<usize>> =
            HashMap::with_capacity(LEX_STEM_ESTIMATE);
        for (i, lemma) in lexicon.iter().enumerate() {
            for form in inflect_all(
                lemma.word,
                &lemma.inflections,
                &cat,
                None,
                PartOfSpeech::Noun,
                /* possessives */ false,
            ) {
                let word = form.word.as_str();
                for ending in ENDINGS {
                    if let Some(stem) = word.strip_suffix(ending) {
                        // The stem must be non-empty (an empty
                        // stem means the lemma is a one-letter
                        // word; not a thing in the sanalista).
                        if !stem.is_empty() {
                            by_stem
                                .entry(stem.to_string())
                                .or_default()
                                .insert(i);
                        }
                        // We accept the FIRST matching ending
                        // (longest, by ENDINGS order). The
                        // alternative -- indexing every match --
                        // bloats the index by 3-5x for no real
                        // lookup benefit: the verify step covers
                        // any split that didn't land in the
                        // index. The longest-ending heuristic is
                        // the canonical Finnish split for the
                        // common cases (genitive "n", inessive
                        // "ssa/ssä", etc.).
                        break;
                    }
                }
            }
        }
        Self { by_stem }
    }

    /// Candidate lemma indices for a `(word, ending)` pair. The
    /// caller has already stripped `ending` from `word` to get
    /// `stem`; the index returns the lemmas that have a form
    /// whose canonical split uses that ending.
    pub fn candidates_for_stem(&self, stem: &str) -> Option<&HashSet<usize>> {
        self.by_stem.get(stem)
    }

    /// The set of endings this index tries. Exposed for tests and
    /// for callers that want to enumerate the candidates
    /// themselves.
    pub fn endings() -> &'static [&'static str] {
        ENDINGS
    }
}

static STEM_INDEX: OnceLock<StemIndex> = OnceLock::new();

/// The process-wide stem index, built once on first access. Cheap
/// to build (a single pass over the noun partition's `inflect_all`
/// output) and shared thereafter.
pub fn stem_index() -> &'static StemIndex {
    STEM_INDEX.get_or_init(|| {
        let lexicon = parse_sanalista(crate::SANALISTA);
        StemIndex::build(&lexicon)
    })
}

/// Find any lemma whose paradigm emits `word`, using the
/// reverse-stem index to short-circuit. Returns the first lemma
/// the verify step accepts. This is the slow check's preferred
/// fast lane -- the prefix walks in `find_any_base_with` are
/// the fallback for stems the index doesn't cover (irregular
/// verbs, typos, anything where the spelling doesn't decompose
/// into a stem on the ENDINGS list).
pub fn find_base_via_stem_index(
    word: &str,
    cat: &Catalogues,
    lexicon: &[crate::kotus::Lemma],
) -> Option<String> {
    let idx = stem_index();
    // A repaired form is only a fallback: `vedelle` is the plain allative of
    // `vesi`, but it is also a repaired potential connegative of the verb
    // `vedellä`, and the noun reading must win whichever candidate the hash
    // set visits first.
    let mut repaired: Option<String> = None;
    for ending in ENDINGS {
        let Some(stem) = word.strip_suffix(ending) else { continue };
        if stem.is_empty() { continue }
        let Some(candidates) = idx.candidates_for_stem(stem) else { continue };
        for &lemma_idx in candidates {
            let lemma = &lexicon[lemma_idx];
            // A candidate that *is* the word is the lemma form of the
            // word, not an inflected form of it. That is gate 1's job
            // (the lemma HashSet probe), and returning it here would
            // shadow the real inflected base: `vedellä` is both the
            // verb lemma `vedellä` and the adessive of the noun
            // `vesi`, and the stem index is meant for the latter.
            if lemma.word == word {
                continue;
            }
            let forms = inflect_all(
                lemma.word,
                &lemma.inflections,
                cat,
                None,
                PartOfSpeech::Noun,
                false,
            );
            for form in &forms {
                if form.word != word {
                    continue;
                }
                if form.confidence == Confidence::Plain {
                    return Some(lemma.word.to_string());
                }
                repaired.get_or_insert_with(|| lemma.word.to_string());
            }
        }
    }
    repaired
}

/// Rough upper bound for the (stem, lemma) pair count, used to
/// size the inner HashMap at build time. Class 27 alone (vesi)
/// has 3 distinct stems × 1 lemma = 3 pairs; multiply by the
/// number of lemmas with multi-stem paradigms and the 100 k
/// sanalista gives a few hundred thousand pairs. 1 M is a safe
/// overestimate.
const LEX_STEM_ESTIMATE: usize = 1_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every inflected form of `vesi` (class 27, gradation `*F`)
    /// has at least one valid stem under the ENDINGS list, so the
    /// stem index returns `vesi` for each. The lemma `vesi`
    /// itself is excluded: the stem index is for inflected forms
    /// (forms generated by the engine), and the lemma is a
    /// self-stem. Lemma lookup is gate 1's job
    /// (`is_known_word`'s lemma HashSet probe).
    #[test]
    fn stem_index_finds_vesi_for_its_forms() {
        for w in &[
            "veden", "vettä", "veteen", "vedet", "vesiä",
            "vesien", "vesiin", "vedessä", "vedestä", "vedellä",
            "vedeltä", "vedelle", "vetenä", "vedeksi",
        ] {
            let cat = Catalogues::embedded().clone();
            let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
            let base = find_base_via_stem_index(w, &cat, &lexicon);
            assert_eq!(
                base.as_deref(),
                Some("vesi"),
                "{w}: stem index missed; first base = {base:?}",
            );
        }
    }

    /// The stem index returns `None` for an unknown word. The
    /// fallback (`find_any_base_with`'s prefix walks) catches
    /// these; the stem index is the fast lane, not the only
    /// gate.
    #[test]
    fn stem_index_rejects_garbage() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for w in &["xyzzy", "qwerty", "asdfg"] {
            assert!(
                find_base_via_stem_index(w, &cat, &lexicon).is_none(),
                "{w}: stem index should miss",
            );
        }
    }
}