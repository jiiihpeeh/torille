//! The detailed suggestion record returned by
//! [`suggest_detailed`](super::suggest_detailed).
//!
//! [`Suggestion`] carries the spelling evidence (edit distance, source) and
//! the morphological evidence (readings, confidence) for one correction
//! candidate. `suggest` only needs the surface word; tools and callers that
//! want to explain or rank candidates use this richer shape.

use crate::tags::{Confidence, Tags};

/// A ranked spell-correction candidate.
///
/// Built by [`crate::suggest::suggest_detailed`]. The fields fall into three
/// groups:
///
/// * **Identity** — `word`, `lemma`, `source`: what the candidate is.
/// * **Spelling evidence** — `edit_distance`, `existing_score`: what the
///   deterministic engine already knows.
/// * **Morphology** — `readings`, `confidence`: what Finnish grammar says.
#[derive(Clone, Debug)]
pub struct Suggestion {
    /// The candidate surface form.
    pub word: String,
    /// Levenshtein distance from the original (misspicked) input. `0` means
    /// the candidate equals the input character-for-character.
    pub edit_distance: usize,
    /// The lemma this candidate traces to. For a direct lemma hit it is the
    /// candidate's own word; for a generated inflected form it is the lemma
    /// whose paradigm produced it. `None` is reserved for candidates whose
    /// lemma could not be recovered — that should not happen today.
    pub lemma: Option<String>,
    /// How the candidate was produced by the deterministic engine.
    pub source: SuggestionSource,
    /// All morphological readings the engine produced for this surface form.
    /// Empty when the candidate is not in the index (a neighbour hit only).
    pub readings: Vec<Tags>,
    /// How confident the engine is in this form. Repaired and guessed forms
    /// are accepted as words but must not reach the suggestion set; callers
    /// that build their own list use this to filter.
    pub confidence: Confidence,
    /// Position in the deterministic ranking, 0-based.
    pub original_rank: usize,
    /// A deterministic score in roughly `[0, 1]`. Today it is
    /// `1 / (1 + edit_distance)`, so an exact match scores `1.0` and a two-edit
    /// miss scores `0.33`.
    pub existing_score: f32,
}

/// Where a [`Suggestion`] came from in the pipeline.
///
/// The variant matters to ranking: a [`DirectLemma`](Self::DirectLemma) hit
/// has the strongest evidence (the input is already in the sanalista), a
/// [`NeighbourLemma`](Self::NeighbourLemma) hit only shares a head or tail
/// with the input, and a [`GeneratedForm`](Self::GeneratedForm) is one of
/// possibly hundreds of forms emitted by `inflect_all`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuggestionSource {
    /// The candidate is a lemma that directly hits the input word in the
    /// sanalista. (`report.direct`.)
    DirectLemma,
    /// The candidate is a lemma that does not match the input but shares a
    /// head, tail, or short prefix with it — the "edit-distance candidates"
    /// the engine compares against. (`report.neighbours`.)
    NeighbourLemma,
    /// The candidate is an inflected form produced by `inflect_all` on a
    /// lemma whose head or tail matched the input. Carries the source lemma.
    /// (`report.forms`.)
    GeneratedForm { lemma: String },
}
