//! Finnish spell checker.
//!
//! A dictionary-driven checker: the Kotus sanalista provides the lemmas, the
//! Kotus paradigm catalogue turns each lemma into its inflected forms, and
//! anything still unknown is checked by compound analysis.

#![forbid(unsafe_code)]

pub(crate) mod case;
pub mod compound;
pub mod derivation;
pub mod expand;
pub(crate) mod gradation;
pub mod grammar;
pub(crate) mod harmony;
pub mod hyphenation;
pub mod kotus;
mod map_reader;
pub mod pronoun;
pub mod rerank;
pub mod spec;
pub mod suggest;
pub mod tags;
pub mod tokenise;
pub(crate) mod wordset;

pub use kotus::EmbeddedLemma;
pub use kotus::Inflection;
pub use kotus::Lemma;
pub use kotus::Sanalista;
pub use kotus::parse_sanalista;
pub use kotus::read_sanalista;
pub use kotus::sanalista;
pub use rerank::Hypothesis;
pub use rerank::HypothesisSource;
pub use rerank::MorphologyHypothesizer;
pub use rerank::NeuralConfig;
pub use rerank::NoOpMorphologyHypothesizer;
pub use rerank::NoOpReranker;
pub use rerank::Pipeline;
pub use rerank::PipelineConfig;
pub use rerank::Reranker;
pub use rerank::Suggestion;
pub use rerank::SuggestionSource;
pub use rerank::{default_reranker, load_pipeline, realize_hypothesis};
pub use suggest::{suggest, EditOp, levenshtein_with_ops};

/// Whether a character is one of the Finnish vowels.
///
/// `å` is a Finnish vowel in the Swedish loanwords that keep it, and the
/// capital letters are the same vowels in the same words, so both are counted.
/// The inflection rules only ever read the sanalista's own lower-case lemmas
/// and endings, where neither occurs; the phonotactic filter does read whatever
/// it is handed, and would reject a word it should merely pass.
pub fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e'
            | 'i'
            | 'o'
            | 'u'
            | 'y'
            | 'ä'
            | 'ö'
            | 'å'
            | 'A'
            | 'E'
            | 'I'
            | 'O'
            | 'U'
            | 'Y'
            | 'Ä'
            | 'Ö'
            | 'Å'
    )
}

/// The noun lexicon, embedded as its source text so the crate needs no data
/// file at runtime.
///
/// [`kotus::sanalista`] parses this once and caches the result; use that rather
/// than reading the text, unless you want the raw rows.
pub const SANALISTA: &str = include_str!("../data/nykysuomensanalista.csv");
