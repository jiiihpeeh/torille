//! Per-sentence reuse for the grammar rule pass.
//!
//! A warm re-check of a page is dominated by the rules over every sentence,
//! even when the user changed one sentence. The cache keys each sentence by a
//! hash of its exact text (and length) and keeps the diagnostics with spans
//! relative to the sentence. An unchanged sentence is a hash probe plus a
//! shift; a changed one re-runs the sentence-local rules for that slice only.
//!
//! The unbalanced-quote warning is document-global: it reads the quote depth at
//! the end of the whole text. The per-sentence pass therefore skips it and the
//! cache adds it once from the full token stream.

use std::collections::HashMap;

use crate::morphology::{MorphologyAnalysis, MorphologyCache};
use crate::tokenise::{Token, tokenise};

use super::diagnostic::SyntaxDiagnostic;
use super::rank;

/// Cap on cached sentences. A long book can exceed this; clearing the table
/// costs one full re-check of the sentences still on screen.
const SENTENCE_CACHE_LIMIT: usize = 2_000;

/// Counters for the performance harness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SentenceCacheStats {
    /// Sentences currently cached.
    pub entries: usize,
    /// Sentences reused from the cache since the last clear.
    pub hits: u64,
    /// Sentences analysed and inserted since the last clear.
    pub misses: u64,
}

#[derive(Debug, Clone)]
struct CachedSentence {
    /// Byte length, so a hash collision cannot reuse a different sentence.
    len: usize,
    /// Quotation depth at the sentence start; the rules that read quote depth
    /// must not reuse a sentence cached under a different depth.
    quote_depth: u16,
    /// Diagnostics with spans and edits relative to the sentence start.
    diagnostics: Vec<SyntaxDiagnostic>,
}

/// A bounded cache of sentence diagnostics.
#[derive(Debug, Default, Clone)]
pub struct SentenceCache {
    entries: HashMap<u64, CachedSentence>,
    hits: u64,
    misses: u64,
}

impl SentenceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop every cached sentence and reset the counters.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> SentenceCacheStats {
        SentenceCacheStats {
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
        }
    }

    /// Grammar diagnostics for `text` through the sentence cache.
    ///
    /// `morphology` is the whole-text pass; its sentence spans define the
    /// units. `cache` is the per-textarea morphology cache, shared with the
    /// whole-text pass so a changed sentence's words are already resolved.
    pub fn check(
        &mut self,
        text: &str,
        tokens: &[Token],
        morphology: &MorphologyAnalysis,
        cache: &mut MorphologyCache,
    ) -> Vec<SyntaxDiagnostic> {
        // One document pass gives the quote depth at each sentence start and
        // the document-global unbalanced-quote warning.
        let contexts = crate::sentence::analyze(tokens);
        let mut token_cursor = 0usize;
        let mut diagnostics = Vec::new();
        for span in &morphology.sentences {
            while token_cursor < tokens.len()
                && (!tokens[token_cursor].is_word() || tokens[token_cursor].start < span.start)
            {
                token_cursor += 1;
            }
            let quote_depth = contexts
                .get(token_cursor)
                .map_or(0, |context| context.quote_depth);
            let Some(slice) = text.get(span.start..span.end) else {
                continue;
            };
            let hash = sentence_hash(slice, quote_depth);
            let cached = self
                .entries
                .get(&hash)
                .filter(|entry| entry.len == slice.len() && entry.quote_depth == quote_depth)
                .cloned();
            let relative = match cached {
                Some(entry) => {
                    self.hits += 1;
                    entry.diagnostics
                }
                None => {
                    self.misses += 1;
                    let sentence_tokens = tokenise(slice);
                    let sentence_contexts =
                        crate::sentence::analyze_with_quote_depth(&sentence_tokens, quote_depth);
                    let mut sentence_morphology =
                        crate::morphology::analyze_morphology(&sentence_tokens, cache);
                    sentence_morphology.sentences =
                        crate::sentence::spans_from_contexts(&sentence_tokens, &sentence_contexts);
                    let analysis = crate::syntax::analyze_syntax_with_contexts(
                        &sentence_tokens,
                        &sentence_morphology,
                        &sentence_contexts,
                    );
                    let found = super::check_analysis_without_quote(
                        &sentence_tokens,
                        &analysis,
                        &sentence_contexts,
                    );
                    if self.entries.len() >= SENTENCE_CACHE_LIMIT {
                        self.entries.clear();
                    }
                    self.entries.insert(
                        hash,
                        CachedSentence {
                            len: slice.len(),
                            quote_depth,
                            diagnostics: found.clone(),
                        },
                    );
                    found
                }
            };
            for mut diagnostic in relative {
                shift(&mut diagnostic, span.start);
                diagnostics.push(diagnostic);
            }
        }

        diagnostics.extend(super::punctuation::quote(tokens, &contexts));
        rank::resolve(diagnostics)
    }
}

/// Shift a diagnostic and every correction edit by the sentence's start.
fn shift(diagnostic: &mut SyntaxDiagnostic, offset: usize) {
    if offset == 0 {
        return;
    }
    diagnostic.span.start += offset;
    diagnostic.span.end += offset;
    for correction in &mut diagnostic.corrections {
        for edit in &mut correction.edits {
            edit.start += offset;
            edit.end += offset;
        }
    }
}

/// A hash of the exact sentence text, its byte length and the starting quote
/// depth.
fn sentence_hash(text: &str, quote_depth: u16) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    text.len().hash(&mut hasher);
    quote_depth.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentence_cache_check(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let mut morphology_cache = MorphologyCache::new();
        let morphology = crate::morphology::analyze_morphology(&tokens, &mut morphology_cache);
        let mut sentences = SentenceCache::new();
        sentences.check(text, &tokens, &morphology, &mut morphology_cache)
    }

    fn whole_check(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        super::super::check_analysis(&tokens, &analysis)
    }

    #[test]
    fn the_sentence_cache_matches_the_whole_text_pass() {
        for text in [
            "Koira nukkuvat. Tällä hetkellä tilanne on rauhallinen.",
            "Hän tuli tupaan, jossa roihusi tuli.",
            "Sami ja Liisa söivät ja joivat.",
            "Minä olen lukenut. He ovat lukeneet.",
            "Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.",
            "Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.",
            "Teksti (tämä) jatkuu. Toinen virke.",
            "Hän sanoi: »Elä tule.",
            "Hän sanoi: »Tämä on hyvä. Omena esimerkiksi on hyvä.»",
            "Tyttö, joka laulaa, on iloinen.",
        ] {
            assert_eq!(sentence_cache_check(text), whole_check(text), "{text}");
        }
    }

    #[test]
    fn an_unchanged_sentence_is_reused_and_a_changed_one_is_not() {
        let text = "Koira nukkuvat. Kissa istuu.";
        let tokens = tokenise(text);
        let mut morphology_cache = MorphologyCache::new();
        let morphology = crate::morphology::analyze_morphology(&tokens, &mut morphology_cache);
        let mut sentences = SentenceCache::new();

        let first = sentences.check(text, &tokens, &morphology, &mut morphology_cache);
        assert_eq!(sentences.stats().misses, 2);
        assert_eq!(sentences.stats().hits, 0);

        let second = sentences.check(text, &tokens, &morphology, &mut morphology_cache);
        assert_eq!(first, second);
        assert_eq!(sentences.stats().hits, 2);
        assert_eq!(sentences.stats().misses, 2);

        // The second sentence changes; the first is still a hit.
        let changed = "Koira nukkuvat. Kissa nukkuu.";
        let changed_tokens = tokenise(changed);
        let changed_morphology =
            crate::morphology::analyze_morphology(&changed_tokens, &mut morphology_cache);
        let third = sentences.check(
            changed,
            &changed_tokens,
            &changed_morphology,
            &mut morphology_cache,
        );
        assert_eq!(sentences.stats().hits, 3);
        assert_eq!(sentences.stats().misses, 3);
        assert_eq!(third, whole_check(changed));
    }

    #[test]
    fn the_cache_is_cleared_with_the_state() {
        let text = "Koira nukkuvat.";
        let tokens = tokenise(text);
        let mut morphology_cache = MorphologyCache::new();
        let morphology = crate::morphology::analyze_morphology(&tokens, &mut morphology_cache);
        let mut sentences = SentenceCache::new();
        let _ = sentences.check(text, &tokens, &morphology, &mut morphology_cache);
        assert!(!sentences.is_empty());
        sentences.clear();
        assert!(sentences.is_empty());
        assert_eq!(sentences.stats(), SentenceCacheStats::default());
    }
}
