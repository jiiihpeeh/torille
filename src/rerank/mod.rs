//! Optional neural / contextual reranker for spell-correction candidates.
//!
//! This module is the integration point for the small neural reranker the
//! design document describes. Slice 0 of that plan only adds the wiring; the
//! actual reranker (heuristic or trained MLP) lands in later slices.
//!
//! # Where this sits in the spell-check pipeline
//!
//! ```text
//! input word
//!     ↓
//! existing FormBuilder + find_bases_with  (deterministic, unchanged)
//!     ↓
//! Vec<Suggestion>              ← what suggest_detailed returns today
//!     ↓
//! Reranker::rerank              ← no-op unless a real one is plugged in
//!     ↓
//! final ranking by `final_score`
//!     ↓
//! top-N back out as Vec<String>
//! ```
//!
//! The deterministic engine still decides what constitutes a *plausible*
//! Finnish correction. The reranker, when present, decides which of several
//! plausible corrections is most likely intended in this context.
//!
//! # Slice 0 contract
//!
//! The contract this slice guarantees, regardless of which reranker is
//! installed:
//!
//! * When no real reranker is loaded, the [`NoOpReranker`] is used. Behaviour
//!   is identical to pre-Slice-0 `torille::suggest::suggest`: same ranking by
//!   Levenshtein, same return shape, same tests pass.
//! * When a real reranker is loaded but fails (broken file, inedits crash,
//!   etc.), the call falls back to the deterministic ranking. A broken
//!   optional component must never make Torille unusable.
//! * When [`NeuralConfig::enabled`] is `false`, no model is loaded, no memory
//!   is allocated for one, and `default_reranker()` returns the no-op.
//!
//! Later slices will add [`HeuristicReranker`] (slice 1) and a trained
//! [`MlpReranker`] (slice 3+). Neither is implemented yet.

pub mod mlp;

pub use mlp::{Candidate, ModelA, QueryFeatures, Scratch};

use crate::expand::FormBuilder;
use crate::spec::Catalogues;
use crate::tags::{Confidence, Tags};

/// A ranked spell-correction candidate, with everything a contextual reranker
/// might want to look at.
///
/// Built by [`crate::suggest::suggest_detailed`]. The fields fall into three
/// groups:
///
/// * **Identity** — `word`, `lemma`, `source`: what the candidate is.
/// * **Spelling evidence** — `edit_distance`, `existing_score`: what the
///   deterministic engine already knows.
/// * **Morphology** — `readings`, `confidence`: what Finnish grammar says.
///
/// [`original_rank`] records the candidate's 0-based position in the
/// deterministic ranking, so a future "did the neural pass change anything?"
/// report can diff the two orderings.
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
    /// How confident the engine is in this form. Guessed forms should not be
    /// suggested; this field is exposed so a future reranker can down-weight
    /// them.
    pub confidence: Confidence,
    /// Position in the deterministic ranking, 0-based, before any reranking.
    pub original_rank: usize,
    /// A deterministic score in roughly `[0, 1]`. Today it is
    /// `1 / (1 + edit_distance)`, so an exact match scores `1.0` and a two-edit
    /// miss scores `0.33`. A future "morphology bonus" or weighted edit
    /// distance can replace the value without breaking the contract.
    pub existing_score: f32,
}

/// Where a [`Suggestion`] came from in the pipeline.
///
/// The variant matters to the reranker: a [`DirectLemma`](Self::DirectLemma)
/// hit has the strongest evidence (the input is already in the sanalista),
/// a [`NeighbourLemma`](Self::NeighbourLemma) hit only shares a head or tail
/// with the input, a [`GeneratedForm`](Self::GeneratedForm) is one of
/// possibly hundreds of forms emitted by `inflect_all`, and a
/// [`MorphologyHypothesis`](Self::MorphologyHypothesis) was produced by the
/// neural morphology hypothesizer feeding the deterministic
/// `lemma + features → surface form` step.
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
    /// The candidate was realized from a neural morphology hypothesis:
    /// `hypothesizer` predicted `lemma + features`, then deterministic
    /// inflection produced the surface form. This is the path the design
    /// document calls "neural morphology hypothesis" — the only path that
    /// can recover Finnish forms when the head/tail/prefix walks find no
    /// plausible neighbours.
    MorphologyHypothesis {
        /// The lemma the hypothesizer predicted.
        lemma: String,
        /// The morphological features the hypothesizer predicted. Boxed
        /// because [`Tags`] is large relative to the enum tag.
        features: Box<crate::tags::Tags>,
    },
}

/// A morphological hypothesis produced by the neural model (or a future
/// rule-based stand-in).
///
/// The hypothesis is *not* a surface form: it is a lemma + a compact
/// morphological feature bundle. The deterministic Finnish inflection
/// machinery turns it into one. This indirection is the design document's
/// central rule — the neural component proposes the linguistic intent,
/// `torille` produces the spelling.
///
/// Top-`N` hypotheses are kept (slice 0 has `N = 5` by default). The
/// realisation step validates each one against the deterministic engine
/// and refuses anything that does not actually inflect to a Finnish word.
#[derive(Clone, Debug, PartialEq)]
pub struct Hypothesis {
    /// The lemma the writer is plausibly trying to spell.
    pub lemma: String,
    /// The grammatical reading the lemma should appear in: POS, case,
    /// number, person, tense, mood, voice. Stored as the same [`Tags`] the
    /// rest of the engine uses, so `FormBuilder::forms_of(lemma, &features,
    // cat)` can produce the surface form without any adapter layer.
    pub features: crate::tags::Tags,
    /// The hypothesizer's own confidence in this `(lemma, features)` pair.
    /// `0.0` is "guessing", `1.0` is "certain". Used for ranking within the
    /// hypothesis list and for selective inference in later slices.
    pub confidence: f32,
    /// Where the hypothesis came from. `test / future` slices add a
    /// `RuleBased` variant for hand-coded last-ditch hypotheses.
    pub source: HypothesisSource,
}

/// Where a [`Hypothesis`] was produced.
///
/// Today the only producer is the (still no-op) neural morphology
/// hypothesizer; the variant exists so future rule-based fallbacks
/// (slice 1+) and training-data tooling can tag their own outputs
/// without breaking the contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypothesisSource {
    /// Produced by the neural morphology hypothesizer.
    Neural,
    /// Produced by a future rule-based fallback (slice 1+). Carries no
    /// payload today because the variant is unused at slice 0; the design
    /// is documented here so adding it later is non-breaking.
    RuleBased,
}

/// A pluggable candidate re-ranker.
///
/// Implementations may reorder, drop, or augment `candidates`. The trait
/// contract is intentionally narrow so a heuristic reranker (slice 1) and a
/// trained MLP reranker (slice 3+) can both satisfy it.
///
/// `Send + Sync` because [`crate::suggest::suggest_with_reranker`] will be
/// called from worker threads once the WebAssembly / Firefox port starts
/// using the reranker.
pub trait Reranker: Send + Sync {
    /// Re-rank `candidates` for the given `original` word. May use
    /// `context_tokens` (left and right of `original` in the user's text) as
    /// a feature; the slice-0 callers pass `&[]`.
    ///
    /// Returns a `Vec` with the same elements as the input (modulo any
    /// deliberate drops), re-ordered by the reranker's preferred score. The
    /// caller applies the final Levenshtein-merge to surface a top-N list.
    fn rerank(
        &self,
        original: &str,
        context_tokens: &[&str],
        candidates: Vec<Suggestion>,
    ) -> Vec<Suggestion>;

    /// Whether this reranker is enabled. A disabled reranker is a strong
    /// signal to callers to skip the call entirely; an enabled reranker
    /// whose `rerank` is a no-op (the default [`NoOpReranker`]) still has to
    /// be called so its overhead is visible in profiles.
    fn enabled(&self) -> bool;

    /// A short label for logs and bug reports. Used by the future benchmark
    /// harness to record which reranker produced a given ranking.
    fn name(&self) -> &'static str;
}

/// The default reranker: pass-through, no reordering, no model loaded.
///
/// Slice-0 behaviour. `suggest_detailed` followed by `NoOpReranker.rerank`
/// must produce a list byte-identical to the pre-Slice-0 `suggest` ranking.
pub struct NoOpReranker;

impl Reranker for NoOpReranker {
    fn rerank(
        &self,
        _original: &str,
        _context_tokens: &[&str],
        candidates: Vec<Suggestion>,
    ) -> Vec<Suggestion> {
        candidates
    }

    fn enabled(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "noop"
    }
}

/// A pluggable morphology hypothesizer: a component that, when ordinary edit
/// distance cannot find a plausible candidate, predicts `(lemma, features)`
/// pairs the writer might have intended.
///
/// The trait is the second half of the design document's two-role neural
/// architecture. While the [`Reranker`] reorders an existing candidate list,
/// the [`MorphologyHypothesizer`] is consulted *only* when the deterministic
/// path has produced too few candidates to be useful — i.e. when the input
/// is far enough from any neighbour that head/tail/prefix walks miss it.
///
/// The output is intentionally *not* a surface form: it is a lemma + a
/// [`Tags`] feature bundle. The deterministic engine
/// ([`FormBuilder::forms_of`](crate::expand::FormBuilder::forms_of)) turns
/// that into the actual Finnish word. That indirection is the rule from
/// design-document §7: do not let the neural network relearn Finnish
/// inflection.
///
/// `Send + Sync` because the hypothesiser, like the reranker, will be
/// called from worker threads in the WASM/Firefox port.
pub trait MorphologyHypothesizer: Send + Sync {
    /// Produce top-`N` `(lemma, features)` hypotheses for `original`. The
    /// slice-0 default `N` lives on [`PipelineConfig::max_hypotheses`].
    /// `context_tokens` is the surrounding text; slice 0 passes `&[]`.
    fn hypothesize(
        &self,
        original: &str,
        context_tokens: &[&str],
    ) -> Vec<Hypothesis>;

    /// Whether this hypothesizer is enabled. A disabled hypothesizer should
    /// never be called — the caller is expected to check before invoking.
    fn enabled(&self) -> bool;

    /// Short label for logs and bug reports.
    fn name(&self) -> &'static str;
}

/// The default hypothesizer: produces zero hypotheses, never active.
///
/// Slice-0 behaviour. Combined with [`NoOpReranker`] in a
/// [`Pipeline::disabled`], it gives byte-identical pre-Slice-0 output for
/// the existing `torille::suggest::suggest` calls.
pub struct NoOpMorphologyHypothesizer;

impl MorphologyHypothesizer for NoOpMorphologyHypothesizer {
    fn hypothesize(
        &self,
        _original: &str,
        _context_tokens: &[&str],
    ) -> Vec<Hypothesis> {
        Vec::new()
    }

    fn enabled(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "noop-morphology"
    }
}

/// Configuration for the optional neural component.
///
/// Slice 0 exposes two independent flags so callers can opt into one role
/// without the other:
///
/// * `reranking_enabled = true` → enable the candidate reranker.
/// * `morphology_enabled = true` → enable the morphology hypothesizer.
///
/// The defaults are both off. A `None` `model_path` is also the default; later
/// slices will load a single model file shared by both roles (the design
/// document's preferred outcome when one model performs comparably to two
/// specialized ones, design-document §14).
#[derive(Clone, Debug)]
pub struct NeuralConfig {
    /// Whether the candidate reranker is enabled. Defaults to `false`.
    pub reranking_enabled: bool,

    /// Whether the morphology hypothesizer is enabled. Defaults to `false`.
    pub morphology_enabled: bool,

    /// Optional path to the model file. Slice 0 ignores it; later slices
    /// load it and share it between the reranker and the hypothesizer
    /// (design-document §14: prefer one model when accuracy is comparable).
    pub model_path: Option<std::path::PathBuf>,

    /// Blend weight applied to the neural score when it is mixed with the
    /// existing deterministic score (`final_score = rule_score + weight *
    /// neural_score`). Slice 0 does not apply it; the value is stored so
    /// downstream code can read a stable, FFI-friendly `f32`.
    pub weight: f32,
}

impl Default for NeuralConfig {
    fn default() -> Self {
        Self {
            reranking_enabled: false,
            morphology_enabled: false,
            model_path: None,
            weight: 0.5,
        }
    }
}

impl NeuralConfig {
    /// `true` if any role is enabled. The default `false` means
    /// [`load_pipeline`] returns the disabled pipeline.
    pub fn any_enabled(&self) -> bool {
        self.reranking_enabled || self.morphology_enabled
    }
}

/// Pipeline-level tuning knobs that the reranker / hypothesiser themselves
/// do not own.
///
/// Kept separate from [`NeuralConfig`] (which is the user-facing knob set)
/// so the same knobs work for a future hand-tuned heuristic reranker as
/// for the trained MLP: neither needs to know about a model path.
#[derive(Clone, Debug)]
pub struct PipelineConfig {
    /// The number of candidates below which the morphology hypothesiser is
    /// invoked. Defaults to [`usize::MAX`], which means "never". Slice 1
    /// will tune this against the benchmark.
    pub morphology_threshold: usize,

    /// The cap on how many hypotheses the hypothesiser may produce for one
    /// input. The design document's design range is 1–5.
    pub max_hypotheses: usize,

    /// The cap on how many [`Suggestion`]s the pipeline surfaces overall.
    /// Independent of the reranker/hypothesiser: this is the final cut.
    pub max_candidates: usize,

    /// Blend weight applied to the neural score when it is mixed with the
    /// existing deterministic score. Slice 0 stores but does not apply it.
    pub neural_weight: f32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            // Effectively disables the hypothesiser path until a slice
            // explicitly tunes the threshold.
            morphology_threshold: usize::MAX,
            max_hypotheses: 5,
            max_candidates: 5,
            neural_weight: 0.5,
        }
    }
}

/// The combined reranker + morphology hypothesizer, plus the
/// [`PipelineConfig`] the inference loop reads.
///
/// Slice 0's contract: [`Pipeline::disabled`] is identical to today's
/// deterministic pipeline in observable behaviour — same candidates, same
/// ranking, same tests pass. Slices 1+ swap the no-op components for real
/// ones; the `Pipeline` wrapper stays.
pub struct Pipeline {
    /// The candidate reranker. `NoOpReranker` when disabled.
    pub reranker: Box<dyn Reranker>,
    /// The morphology hypothesiser. `NoOpMorphologyHypothesizer` when
    /// disabled.
    pub hypothesizer: Box<dyn MorphologyHypothesizer>,
    /// The pipeline-level tuning knobs.
    pub config: PipelineConfig,
}

impl Pipeline {
    /// A pipeline that does nothing neural: no reranking, no hysteresis,
    /// no model loaded. Equivalent to pre-Slice-0 `suggest`.
    pub fn disabled() -> Self {
        Self {
            reranker: Box::new(NoOpReranker),
            hypothesizer: Box::new(NoOpMorphologyHypothesizer),
            config: PipelineConfig::default(),
        }
    }

    /// `true` if any neural component in this pipeline is enabled.
    pub fn any_neural_enabled(&self) -> bool {
        self.reranker.enabled() || self.hypothesizer.enabled()
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Build the reranker the configuration asks for.
///
/// Slice 0 returns [`NoOpReranker`] regardless of the config: the
/// architecture is the deliverable, not the model. Slice 1 replaces the
/// `reranking_enabled` branch with a hand-tuned heuristic reranker; slice
/// 3+ loads a real model from `cfg.model_path`.
///
/// Future behaviour on a failed model load: return the no-op rather than
/// propagating the error, so a broken optional component cannot make
/// Torille unusable (design-doc rule §29).
pub fn default_reranker() -> Box<dyn Reranker> {
    Box::new(NoOpReranker)
}

/// Realize a [`Hypothesis`] into a [`Suggestion`] via the deterministic
/// Finnish inflection engine.
///
/// `h` is `(lemma, features)` produced by the morphology hypothesiser.
/// `builder` is the `torille::expand::FormBuilder` (built once per process —
/// building it takes ~30 s). `cat` is the paradigm catalogue.
///
/// Returns `None` if the engine cannot inflect `h.lemma` to a form that
/// matches `h.features`. The design document's §32 rule: "The neural
/// component proposes. The deterministic system validates." A hypothesis
/// the engine refuses is exactly the case where the validation fails.
///
/// Slice 0 callers never reach this — the no-op hypothesiser produces no
/// hypotheses, and the `Pipeline::disabled` entry in
/// [`crate::suggest::suggest_with_pipeline`] skips the call. Slices 1+
/// build a `FormBuilder` once and route real hypotheses through here.
pub fn realize_hypothesis(
    h: &Hypothesis,
    builder: &FormBuilder,
    cat: &Catalogues,
) -> Option<Suggestion> {
    let forms = builder.forms_of(&h.lemma, &h.features, cat);
    let surface = forms.into_iter().next()?;
    Some(Suggestion {
        word: surface,
        // edit_distance: caller computes against the user's input.
        edit_distance: 0,
        // original_rank: caller decides; the realised suggestion is
        // typically appended after the deterministic candidates.
        original_rank: usize::MAX,
        lemma: Some(h.lemma.clone()),
        source: SuggestionSource::MorphologyHypothesis {
            lemma: h.lemma.clone(),
            features: Box::new(h.features),
        },
        // The surface form is in a single reading by virtue of how
        // `forms_of` filters — the engine emitted one form per matching
        // `(base, want)` pair.
        readings: vec![h.features],
        // Realised forms have no human-shaped confidence yet; the future
        // slice-3 reranker / selection pass will combine the engine's
        // `Confidence` with the hypothesiser's `confidence`.
        confidence: Confidence::Guessed,
        // Final scoring: caller writes this after computing edit
        // distance against the user's input.
        existing_score: 0.0,
    })
}

/// Load a [`Pipeline`] from a [`NeuralConfig`].
/// Slice 0 only knows two outcomes:
///
/// * `cfg.any_enabled() == false` (the default) → [`Pipeline::disabled`].
///   No file is opened, no model is loaded, no memory is held.
/// * `cfg.any_enabled() == true` → still [`Pipeline::disabled`] today.
///   Slice 1 wires a real reranker; slice 2+ wires a real hypothesiser;
///   slice 3+ reads `cfg.model_path`.
///
/// On any future failure (broken file, inference crash) this returns
/// [`Pipeline::disabled`] rather than propagating the error — the design
/// document's rule that a broken optional component must never make
/// Torille unusable (§29).
pub fn load_pipeline(cfg: &NeuralConfig) -> Pipeline {
    if !cfg.any_enabled() {
        return Pipeline::disabled();
    }
    // Slice 0 deliberately does not load anything: the architecture is the
    // deliverable, not the model. Slices 1+ read `cfg.model_path`.
    let _ = cfg; // silence "field never read" until slice 1
    Pipeline::disabled()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_suggestion(word: &str, rank: usize) -> Suggestion {
        Suggestion {
            word: word.to_string(),
            edit_distance: rank,
            lemma: Some(word.to_string()),
            source: SuggestionSource::DirectLemma,
            readings: Vec::new(),
            confidence: Confidence::Plain,
            original_rank: rank,
            existing_score: 1.0 / (1.0 + rank as f32),
        }
    }

    /// The no-op reranker must preserve the input order — the deterministic
    /// ranking is what gets surfaced.
    #[test]
    fn noop_reranker_preserves_order() {
        let r = NoOpReranker;
        let candidates = vec![
            dummy_suggestion("talo", 0),
            dummy_suggestion("taloa", 1),
            dummy_suggestion("taloissa", 2),
        ];
        let out = r.rerank("input", &[], candidates.clone());
        let words_in: Vec<&str> = candidates.iter().map(|s| s.word.as_str()).collect();
        let words_out: Vec<&str> = out.iter().map(|s| s.word.as_str()).collect();
        assert_eq!(words_in, words_out);
    }

    /// The no-op reranker is disabled. Callers may skip the call entirely
    /// based on this; the future `selective_inference` pass relies on it.
    #[test]
    fn noop_reranker_is_disabled() {
        let r = NoOpReranker;
        assert!(!r.enabled());
        assert_eq!(r.name(), "noop");
    }

    /// `NeuralConfig::default()` is the off-path: both roles disabled, no
    /// model, weight 0.5 (slice 3+ reads the weight).
    #[test]
    fn neural_config_default_is_off() {
        let cfg = NeuralConfig::default();
        assert!(!cfg.reranking_enabled);
        assert!(!cfg.morphology_enabled);
        assert!(cfg.model_path.is_none());
        assert!((cfg.weight - 0.5).abs() < f32::EPSILON);
        assert!(!cfg.any_enabled());
    }

    /// `NeuralConfig::any_enabled` flips on for either role. The default
    /// config is fully off.
    #[test]
    fn neural_config_any_enabled() {
        let mut cfg = NeuralConfig::default();
        assert!(!cfg.any_enabled());
        cfg.reranking_enabled = true;
        assert!(cfg.any_enabled());
        cfg.reranking_enabled = false;
        cfg.morphology_enabled = true;
        assert!(cfg.any_enabled());
    }

    /// `default_reranker()` returns a no-op that does nothing.
    #[test]
    fn default_reranker_is_noop() {
        let r = default_reranker();
        assert!(!r.enabled());
        assert_eq!(r.name(), "noop");
    }

    /// `load_pipeline` with `any_enabled() == false` returns the disabled
    /// pipeline, even with a model path set. Slice 0 returns the disabled
    /// pipeline regardless.
    #[test]
    fn load_pipeline_off_returns_disabled() {
        let cfg = NeuralConfig {
            reranking_enabled: false,
            morphology_enabled: false,
            model_path: Some(std::path::PathBuf::from("/nonexistent/model.bin")),
            weight: 0.5,
        };
        let p = load_pipeline(&cfg);
        assert!(!p.reranker.enabled());
        assert!(!p.hypothesizer.enabled());
        assert!(!p.any_neural_enabled());
    }

    /// The no-op morphology hypothesizer is disabled and produces zero
    /// hypotheses for any input. Slice 0 behaviour.
    #[test]
    fn noop_morphology_hypothesizer_is_disabled_and_empty() {
        let h = NoOpMorphologyHypothesizer;
        assert!(!h.enabled());
        assert_eq!(h.name(), "noop-morphology");
        assert!(h.hypothesize("jooksee", &[]).is_empty());
        assert!(h.hypothesize("anything", &["left", "right"]).is_empty());
    }

    /// `Pipeline::disabled()` is the no-op-on-both-axes pipeline. The
    /// default pipeline is the disabled one, so `Default::default()`
    /// matches `disabled()`.
    #[test]
    fn pipeline_disabled_is_default_and_neutral() {
        let d = Pipeline::default();
        let p = Pipeline::disabled();
        assert!(!d.any_neural_enabled());
        assert!(!p.any_neural_enabled());
        assert!(!d.reranker.enabled());
        assert!(!d.hypothesizer.enabled());
    }

    /// `PipelineConfig::default()` does not invoke the hypothesizer
    /// (`morphology_threshold = usize::MAX`). Slice 1 tunes this against
    /// the benchmark; slice 0 keeps the deterministic path authoritative.
    #[test]
    fn pipeline_config_default_disables_hypothesizer() {
        let cfg = PipelineConfig::default();
        assert_eq!(cfg.morphology_threshold, usize::MAX);
        assert_eq!(cfg.max_hypotheses, 5);
        assert_eq!(cfg.max_candidates, 5);
    }
}