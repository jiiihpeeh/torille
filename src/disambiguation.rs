//! The seam for context-dependent morphological reading disambiguation.
//!
//! Phase 2 introduces the sidecar types only. No model is trained or run:
//! [`disambiguate`] returns [`Disambiguation::disabled`] and the existing
//! syntax/checker pipeline is intentionally unchanged.
//!
//! A future model scores the readings that [`crate::morphology::WordInfo`]
//! already produced. It must not remove a reading, assign a syntactic role or
//! change a diagnostic by itself. Decisions are per token, not stored in the
//! shared surface cache.

use std::hash::{Hash, Hasher};

use crate::tags::{Confidence, Tags};

/// One token's ranking over its existing morphological readings.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadingDecision {
    /// Index into `WordInfo.readings` / `TokenSyntax.readings`.
    pub preferred: usize,
    /// One score per reading, in the same order as the readings.
    pub scores: Vec<f32>,
    /// Difference between the best and second-best score.
    pub margin: f32,
    /// Entropy of the score distribution, for abstention.
    pub entropy: f32,
    /// The model declines to choose, for example because the margin is low.
    pub abstain: bool,
}

/// Per-analysis decisions for the tokens that have a context-sensitive choice.
///
/// `decisions` is token-indexed and may be shorter than the token stream when
/// later tokens have no decision. `decision` returns `None` for those.
#[derive(Debug, Clone, PartialEq)]
pub struct Disambiguation {
    pub decisions: Vec<Option<ReadingDecision>>,
    /// Model/artifact version. A change invalidates sentence caches.
    pub model_version: u32,
    /// Candidate-feature schema version. A change invalidates sentence caches.
    pub feature_schema_version: u32,
    /// False in Phase 2 and whenever neural inference is switched off.
    pub enabled: bool,
}

impl Disambiguation {
    /// The no-op decision set used when neural inference is disabled.
    pub fn disabled() -> Self {
        Self {
            decisions: Vec::new(),
            model_version: 0,
            feature_schema_version: 0,
            enabled: false,
        }
    }

    /// The decision for one token, if the model produced one.
    pub fn decision(&self, token_index: usize) -> Option<&ReadingDecision> {
        self.decisions.get(token_index).and_then(Option::as_ref)
    }

    /// A stable key for the sentence cache.
    ///
    /// A disabled model has key 0. When enabled, the model version, feature
    /// schema and activation state participate, so changing any of them makes
    /// an unchanged sentence a cache miss rather than a stale hit.
    pub fn cache_key(&self) -> u64 {
        if !self.enabled {
            return 0;
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.enabled.hash(&mut hasher);
        self.model_version.hash(&mut hasher);
        self.feature_schema_version.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for Disambiguation {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Run the (currently disabled) reading-disambiguation pass.
///
/// Phase 2 has no model. The signature is the seam a later model will use:
/// it receives the token stream and the existing [`crate::morphology::MorphologyAnalysis`],
/// and returns a sidecar decision table. It never takes a mutable morphology
/// cache, so it cannot alter the surface-keyed readings.
pub fn disambiguate(
    _tokens: &[crate::tokenise::Token],
    _morphology: &crate::morphology::MorphologyAnalysis,
) -> Disambiguation {
    Disambiguation::disabled()
}

/// Provenance of a candidate reading's lemma.
///
/// A reading index may map to zero, one or several lemmas. `Known` is not a
/// claim that the list has one element; callers must not assume a unique
/// lemma from a preferred reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LemmaProvenance<'a> {
    Known(Vec<&'a str>),
    Unavailable { reason: &'static str },
}

/// A value that the current engine either provides or cannot provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance<T> {
    Known(T),
    Unavailable { reason: &'static str },
}

/// Adapter from a token reading to the features a candidate scorer consumes.
#[derive(Debug, Clone)]
pub struct CandidateView<'a> {
    pub token_index: usize,
    pub reading_index: usize,
    pub tags: Tags,
    pub lemma_provenance: LemmaProvenance<'a>,
    /// `WordInfo` does not retain `Form::confidence` per reading today.
    pub source_confidence: Provenance<Confidence>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    #[test]
    fn the_noop_pass_is_disabled_and_has_no_decisions() {
        let tokens = tokenise("Koira nukkuu.");
        let mut cache = crate::morphology::MorphologyCache::new();
        let morphology = crate::morphology::analyze_morphology(&tokens, &mut cache);
        let decisions = disambiguate(&tokens, &morphology);
        assert!(!decisions.enabled);
        assert!(decisions.decisions.is_empty());
        assert_eq!(decisions.cache_key(), 0);
        assert!(decisions.decision(0).is_none());
    }

    #[test]
    fn an_enabled_model_changes_the_cache_key() {
        let disabled = Disambiguation::disabled();
        let enabled = Disambiguation {
            enabled: true,
            model_version: 1,
            feature_schema_version: 1,
            ..Disambiguation::disabled()
        };
        assert_ne!(disabled.cache_key(), enabled.cache_key());
        assert_ne!(enabled.cache_key(), 0);
    }
}
