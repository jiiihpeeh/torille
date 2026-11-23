//! Per-unit reuse for the grammar rule pass.
//!
//! A warm re-check of a page is dominated by the rules over every sentence,
//! even when the user changed one sentence. The cache finds each grammar unit
//! through a hash of its exact text and keeps the diagnostics with spans
//! relative to the unit. A hash is a lookup key, not an identity: a hit still
//! compares the unit text, so two different sentences that collide do not
//! share diagnostics. An unchanged unit is a hash probe, a string compare and
//! a shift; a changed one re-runs the rules for that slice only.
//!
//! A unit is a grammar sentence, not a typography line. `sentence::spans`
//! splits at every newline for the case/spelling layers, which truncates a
//! wrapped clause and drops the word before a continued line. The cache
//! therefore joins line fragments (a newline, comma or dialogue dash alone does
//! not end a unit) and analyses the previous word together with the unit, so
//! `previous_word`/`boundary_between` see the same context the whole-document
//! pass does.
//!
//! The unbalanced-quote warning is document-global: it reads the quote depth at
//! the end of the whole text. The per-unit pass therefore skips it and the
//! cache adds it once from the full token stream.

use std::collections::HashMap;
use std::ops::Range;

use crate::disambiguation::Disambiguation;
use crate::morphology::{MorphologyAnalysis, MorphologyCache};
use crate::sentence::{SentenceSpan, TokenContext};
use crate::tokenise::{Token, tokenise};

use super::diagnostic::SyntaxDiagnostic;
use super::rank;

/// Cap on cached units. A long book can exceed this; clearing the table costs
/// one full re-check of the units still on screen.
const SENTENCE_CACHE_LIMIT: usize = 2_000;

/// Counters for the performance harness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SentenceCacheStats {
    /// Grammar units currently cached.
    pub entries: usize,
    /// Units reused from the cache since the last clear.
    pub hits: u64,
    /// Units analysed and inserted since the last clear.
    pub misses: u64,
}

/// One cache unit: a grammar sentence with its left context.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GrammarUnit {
    /// Absolute range of the sentence(s) the unit owns.
    core: Range<usize>,
    /// Absolute range that is tokenised and analysed: the previous word, its
    /// separators and the core.
    slice: Range<usize>,
    /// Quote depth at `slice.start`.
    quote_depth: u16,
    /// Whether this is the last unit, which owns an end-of-text insertion.
    last: bool,
}

impl GrammarUnit {
    /// Whether a diagnostic, with spans relative to `slice.start`, belongs to
    /// this unit.
    ///
    /// The leading context must not re-report a finding that belongs to the
    /// previous unit. A zero-width edit exactly on the boundary is owned by
    /// the unit that starts there, so the previous unit keeps only spans that
    /// start before its core end; the last unit also keeps an end-of-text
    /// insertion.
    fn owns(&self, diagnostic: &SyntaxDiagnostic) -> bool {
        let start = self.core.start - self.slice.start;
        let end = self.core.end - self.slice.start;
        if diagnostic.span.start < start {
            return false;
        }
        if self.last {
            diagnostic.span.end <= end
        } else {
            diagnostic.span.start < end
        }
    }
}

#[derive(Debug, Clone)]
struct CachedUnit {
    /// The exact slice this entry was built from.
    ///
    /// The map key is a 64-bit hash, and two different texts of the same
    /// length can collide; reusing the entry's diagnostics would then report
    /// the wrong sentence. The text is kept (bounded by
    /// [`SENTENCE_CACHE_LIMIT`] entries) and compared after a hash hit.
    text: Box<str>,
    /// Core range relative to the slice, so identical slice texts with a
    /// different ownership do not share diagnostics.
    core: Range<usize>,
    /// Quotation depth at the slice start; the rules that read quote depth
    /// must not reuse a slice cached under a different depth.
    quote_depth: u16,
    /// Disambiguation policy key: model/schema/activation state.
    disambiguation: u64,
    /// Diagnostics with spans and edits relative to the slice start.
    diagnostics: Vec<SyntaxDiagnostic>,
}

/// A bounded cache of unit diagnostics.
#[derive(Debug, Default, Clone)]
pub struct SentenceCache {
    entries: HashMap<u64, CachedUnit>,
    hits: u64,
    misses: u64,
}

impl CachedUnit {
    /// Whether this entry is the cache unit `slice`.
    fn matches(&self, slice: &str, core: &Range<usize>, quote_depth: u16, disambiguation: u64) -> bool {
        self.text.as_ref() == slice
            && self.core == *core
            && self.quote_depth == quote_depth
            && self.disambiguation == disambiguation
    }
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
        self.check_with_decisions(
            text,
            tokens,
            morphology,
            cache,
            &Disambiguation::disabled(),
        )
    }

    /// The same check with an explicit disambiguation policy.
    ///
    /// The policy key (model version, feature schema and activation state) is
    /// part of the sentence key. A disabled policy is key 0 and preserves the
    /// pre-sidecar behaviour. With a policy enabled, an unchanged sentence
    /// still hits the cache; a policy change is a miss.
    pub fn check_with_decisions(
        &mut self,
        text: &str,
        tokens: &[Token],
        morphology: &MorphologyAnalysis,
        cache: &mut MorphologyCache,
        decisions: &Disambiguation,
    ) -> Vec<SyntaxDiagnostic> {
        // One document pass gives the quote depth at each unit start and the
        // document-global unbalanced-quote warning.
        let contexts = crate::sentence::analyze(tokens);
        let units = grammar_units(tokens, &morphology.sentences, &contexts);
        let decision_key = decisions.cache_key();
        let mut diagnostics = Vec::new();
        for unit in &units {
            let Some(slice) = text.get(unit.slice.clone()) else {
                continue;
            };
            let quote_depth = unit.quote_depth;
            let core = (unit.core.start - unit.slice.start)..(unit.core.end - unit.slice.start);
            let hash = unit_hash(slice, quote_depth, decision_key);
            let cached = self
                .entries
                .get(&hash)
                .filter(|entry| entry.matches(slice, &core, quote_depth, decision_key))
                .map(|entry| entry.diagnostics.clone());
            let relative = match cached {
                Some(diagnostics) => {
                    self.hits += 1;
                    diagnostics
                }
                None => {
                    self.misses += 1;
                    let unit_tokens = tokenise(slice);
                    let unit_contexts =
                        crate::sentence::analyze_with_quote_depth(&unit_tokens, quote_depth);
                    let mut unit_morphology =
                        crate::morphology::analyze_morphology(&unit_tokens, cache);
                    unit_morphology.sentences =
                        crate::sentence::spans_from_contexts(&unit_tokens, &unit_contexts);
                    let unit_decisions =
                        crate::disambiguation::disambiguate(&unit_tokens, &unit_morphology);
                    let analysis = crate::syntax::analyze_syntax_with_contexts_and_decisions(
                        &unit_tokens,
                        &unit_morphology,
                        &unit_contexts,
                        Some(&unit_decisions),
                    );
                    let found: Vec<SyntaxDiagnostic> = super::check_analysis_without_quote(
                        &unit_tokens,
                        &analysis,
                        &unit_contexts,
                    )
                    .into_iter()
                    .filter(|diagnostic| unit.owns(diagnostic))
                    .collect();
                    if self.entries.len() >= SENTENCE_CACHE_LIMIT {
                        self.entries.clear();
                    }
                    self.entries.insert(
                        hash,
                        CachedUnit {
                            text: slice.into(),
                            core,
                            quote_depth,
                            disambiguation: decision_key,
                            diagnostics: found.clone(),
                        },
                    );
                    found
                }
            };
            for mut diagnostic in relative {
                shift(&mut diagnostic, unit.slice.start);
                diagnostics.push(diagnostic);
            }
        }

        diagnostics.extend(super::punctuation::quote(tokens, &contexts));
        rank::resolve(diagnostics)
    }
}

/// Build the grammar units for `spans`.
///
/// Line fragments are merged while the previous span's trailing separator has
/// no terminal mark, colon or closing quote/bracket: `sentence::spans` ends a
/// sentence at every newline, which is right for the case/spelling layers but
/// truncates a wrapped clause for the grammar rules. A unit's slice starts at
/// the previous unit's last word so `previous_word` and `boundary_between`
/// see the same context as the whole-document pass.
fn grammar_units(
    tokens: &[Token],
    spans: &[SentenceSpan],
    contexts: &[TokenContext],
) -> Vec<GrammarUnit> {
    let mut runs: Vec<Range<usize>> = Vec::new();
    for span in spans {
        match runs.last_mut() {
            Some(last) if last.end == span.start && !hard_boundary(tokens, last.end) => {
                last.end = span.end;
            }
            _ => runs.push(span.start..span.end),
        }
    }
    let count = runs.len();
    runs.into_iter()
        .enumerate()
        .map(|(index, core)| {
            let slice_start = if index == 0 {
                0
            } else {
                tokens
                    .iter()
                    .rposition(|token| token.is_word() && token.end <= core.start)
                    .map_or(core.start, |word| tokens[word].start)
            };
            let quote_depth = tokens
                .iter()
                .position(|token| token.start >= slice_start)
                .and_then(|index| contexts.get(index))
                .map_or(0, |context| context.quote_depth);
            GrammarUnit {
                slice: slice_start..core.end,
                core,
                quote_depth,
                last: index + 1 == count,
            }
        })
        .collect()
}

/// Whether the separator run before `boundary` ends a grammar unit.
///
/// The boundary text lives at the end of the previous span (`span.end` is the
/// next span's start), after its last word. A newline, comma or dialogue dash
/// alone does not end a unit; a terminal mark, colon or closing quote/bracket
/// does.
fn hard_boundary(tokens: &[Token], boundary: usize) -> bool {
    let Some(last) = tokens
        .iter()
        .rposition(|token| token.is_word() && token.end <= boundary)
    else {
        return false;
    };
    for token in tokens.iter().skip(last + 1) {
        if token.start >= boundary {
            break;
        }
        if token.is_word() {
            continue;
        }
        let trimmed = token.text.trim();
        let core = trimmed.trim_end_matches(|character: char| {
            character.is_whitespace() || matches!(character, '-' | '–' | '—')
        });
        if core.is_empty() {
            continue;
        }
        if matches!(core.chars().last(), Some('.' | '!' | '?' | ':')) {
            return true;
        }
        if matches!(
            core.chars().last(),
            Some('»' | '«' | '"' | '“' | '”' | '[' | ']' | '(' | ')' | '{' | '}')
        ) {
            return true;
        }
    }
    false
}

/// Shift a diagnostic and every correction edit by the slice's start.
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

/// A hash of the exact slice text, its byte length and the starting quote
/// depth.
fn unit_hash(text: &str, quote_depth: u16, disambiguation: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    text.len().hash(&mut hasher);
    quote_depth.hash(&mut hasher);
    disambiguation.hash(&mut hasher);
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
            "Voitteko Jumalan silmien edessä\ntehdä semmoista?",
            "Siellä oli laulujen\nja kanteleen maa.",
            "hauskuudekseni--kun ajattelin niitä arvoja, mitkä niillä on maailmalle\nja mitkä minulle.",
            "kuule,\nmiten kauheasti puut heiluvat ja miten kovasti kaikki lehdet värisevät.",
        ] {
            assert_eq!(sentence_cache_check(text), whole_check(text), "{text}");
        }
    }

    #[test]
    fn a_line_wrapped_sentence_is_one_unit() {
        let text = "Siellä oli laulujen\nja kanteleen maa.";
        let tokens = tokenise(text);
        let contexts = crate::sentence::analyze(&tokens);
        let spans = crate::sentence::spans(&tokens);
        let units = grammar_units(&tokens, &spans, &contexts);
        assert_eq!(units.len(), 1, "{units:#?}");
        assert_eq!(units[0].core, 0..text.len());
        assert_eq!(sentence_cache_check(text), whole_check(text));
    }

    #[test]
    fn a_period_ends_a_unit_and_the_next_slice_keeps_the_previous_word() {
        let text = "Ensimmäinen virke. Toinen virke.";
        let tokens = tokenise(text);
        let contexts = crate::sentence::analyze(&tokens);
        let spans = crate::sentence::spans(&tokens);
        let units = grammar_units(&tokens, &spans, &contexts);
        assert_eq!(units.len(), 2, "{units:#?}");
        assert!(
            text[units[1].slice.clone()].starts_with("virke. Toinen"),
            "{:?}",
            &text[units[1].slice.clone()]
        );
    }

    #[test]
    fn a_dialogue_dash_does_not_end_a_unit() {
        let text = "hauskuudekseni--kun ajattelin niitä arvoja.";
        let tokens = tokenise(text);
        let contexts = crate::sentence::analyze(&tokens);
        let spans = crate::sentence::spans(&tokens);
        let units = grammar_units(&tokens, &spans, &contexts);
        assert_eq!(units.len(), 1, "{units:#?}");
    }

    #[test]
    fn the_leading_context_finding_belongs_to_the_previous_unit() {
        let text = "Ensimmäinen virke. Toinen virke.";
        let tokens = tokenise(text);
        let contexts = crate::sentence::analyze(&tokens);
        let spans = crate::sentence::spans(&tokens);
        let units = grammar_units(&tokens, &spans, &contexts);
        let diagnostic = SyntaxDiagnostic::new(
            super::super::diagnostic::DiagnosticKind::CommaMissing,
            super::super::diagnostic::Severity::Error,
            super::super::diagnostic::Confidence::Certain,
            0..0,
            "comma_missing",
        );
        assert!(!units[1].owns(&diagnostic), "the previous word is not owned");
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

    /// The key is a 64-bit hash, so a hit must still compare the unit text.
    /// Before the text was stored, an entry whose hash collided with a
    /// different same-length sentence reused its diagnostics.
    #[test]
    fn a_hash_collision_recomputes_instead_of_reusing_diagnostics() {
        let text = "Koira nukkuvat.";
        let tokens = tokenise(text);
        let mut morphology_cache = MorphologyCache::new();
        let morphology = crate::morphology::analyze_morphology(&tokens, &mut morphology_cache);
        let mut sentences = SentenceCache::new();

        let contexts = crate::sentence::analyze(&tokens);
        let spans = crate::sentence::spans(&tokens);
        let unit = &grammar_units(&tokens, &spans, &contexts)[0];
        let core = (unit.core.start - unit.slice.start)..(unit.core.end - unit.slice.start);
        // A fabricated entry under the hash of the real unit, with different
        // text of the same length and the same ownership metadata.
        let slice = &text[unit.slice.clone()];
        let hash = unit_hash(slice, unit.quote_depth, 0);
        sentences.entries.insert(
            hash,
            CachedUnit {
                text: "x".repeat(slice.len()).into(),
                core,
                quote_depth: unit.quote_depth,
                disambiguation: 0,
                diagnostics: Vec::new(),
            },
        );

        let got = sentences.check(text, &tokens, &morphology, &mut morphology_cache);
        assert_eq!(got, whole_check(text));
        assert_eq!(sentences.stats().misses, 1, "the colliding entry is not a hit");
        assert_eq!(sentences.stats().hits, 0);
    }
}
