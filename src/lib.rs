//! Finnish spell checking, morphology and grammar/style analysis.
//!
//! The engine is dictionary-driven. The Kotus *sanalista* supplies the lemmas,
//! a committed paradigm catalogue turns each lemma into its inflected forms,
//! and words the lexicon does not know fall through to compound analysis. The
//! data is embedded in the crate, so there is no file to find and no model to
//! download at run time.
//!
//! # Checking and correcting
//!
//! ```
//! use torille::{CheckOutcome, check, suggest};
//!
//! assert!(check("talossa").is_known());
//! match check("talossa") {
//!     CheckOutcome::Inflected(bases) => assert_eq!(bases, vec!["talo"]),
//!     other => panic!("expected an inflected form, got {other:?}"),
//! }
//!
//! let corrections = suggest("koirra", 3, 2);
//! assert!(corrections.iter().any(|word| word.as_str() == "koira"));
//! ```
//!
//! # Morphology
//!
//! [`expand::inflect_word`] inflects a lemma the lexicon knows and returns
//! every generated form with its [`tags::Tags`] reading and
//! [`tags::Confidence`]:
//!
//! ```
//! use torille::expand::inflect_word;
//! use torille::tags::Case;
//!
//! let forms = inflect_word("talo");
//! let inessive = forms.iter().find(|form| form.word == "talossa").unwrap();
//! assert_eq!(inessive.union().case, Some(Case::Inessive));
//! ```
//!
//! For a word outside the lexicon, pass an explicit [`kotus::Inflection`] to
//! [`expand::inflect_all_in`].
//!
//! # Grammar, style and typography
//!
//! [`syntax_check`] adds grammar diagnostics over a sentence, [`style_check`]
//! is a separate opt-in style layer, [`typography`] fixes punctuation
//! spacing, and [`hyphenation`] finds Finnish line-break points. [`check_all`]
//! runs the grammar and style layers together.
//!
//! # Features
//!
//! The default features are the hand-written per-class engines
//! (`generated-nominals`, `generated-verbs`; the names are historical). The
//! remaining features are opt-in: `probe-stats` adds cold-morphology counters,
//! and `neural-m0` / `neural-m0-stack` embed an optional reading reranker
//! model. Do not enable the two neural features together.
//!
//! # Licence
//!
//! The engine is `MIT OR Apache-2.0`. The dictionary data has its own terms;
//! see the `data/LICENSE` file in the repository.

#![forbid(unsafe_code)]

pub(crate) mod case;
pub mod class_wizard;
pub mod compound;
pub mod derivation;
pub mod diagnostic;
pub mod disambiguation;
pub mod expand;
mod fast_hash;
pub(crate) mod gradation;
pub mod grammar;
pub mod check;
pub(crate) mod harmony;
pub mod hyphenation;
pub mod kotus;
mod plural_only;
mod map_reader;
pub mod morphology;
pub mod pronoun;
#[cfg(feature = "probe-stats")]
pub mod probe_stats;
#[cfg(not(feature = "probe-stats"))]
pub(crate) mod probe_stats;
mod rejected;
mod sanalista_overrides;
pub mod sentence;
pub mod spec;
pub mod style_check;
pub mod suggest;
pub mod syntax;
pub mod syntax_check;
pub mod syntax_list;
pub mod tags;
pub(crate) mod text;
pub mod tokenise;
pub mod typography;
pub(crate) mod wordset;

pub use kotus::EmbeddedLemma;
pub use kotus::Inflection;
pub use kotus::Lemma;
pub use kotus::Sanalista;
pub use kotus::parse_sanalista;
pub use kotus::read_sanalista;
pub use kotus::sanalista;
pub use check::{CheckAll, check_all, check_all_with_cache};
pub use suggest::{
    CheckOutcome, EditOp, Suggestion, SuggestionSource, check, is_known_word, levenshtein_with_ops,
    suggest, suggest_detailed,
};

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

/// The noun lexicon as its committed source text.
///
/// The runtime does not parse this. [`kotus::sanalista`] and
/// [`kotus::lemmas`] read the packed blob `build.rs` emits; use
/// [`kotus::lemmas`] when you need the rows. This const exists for the public
/// text parser ([`kotus::parse_sanalista`]) and for tooling that needs the raw
/// text. It is a `const`, so its bytes are emitted only where a reachable use
/// remains: a release binary or wasm that never parses the text does not
/// carry it (measured on the shipped wasm and the CLI; see
/// `misc_md/REVIEW_CORRECT.md` C4.4).
pub const SANALISTA: &str = include_str!("../data/nykysuomensanalista.csv");
