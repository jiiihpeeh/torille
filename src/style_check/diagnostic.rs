//! Types for the opt-in style layer.
//!
//! Style diagnostics reuse the grammar checker's small diagnostic primitives
//! (`Severity`, `DiagnosticConfidence`, `Correction`) but keep their own kind
//! enum. A style finding is always [`Severity::Suggestion`]; the constructor
//! enforces that, so a rule cannot accidentally turn a preference into an
//! error.

use std::ops::Range;

use crate::syntax_check::{Correction, DiagnosticConfidence, Severity};

/// Stable style categories. Keep the names distinct from the grammar kinds;
/// a consumer must be able to tell the two streams apart even when a panel
/// merges them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StyleKind {
    /// The same content lemma repeats close enough to read as an echo.
    Repetition,
    /// A sentence is longer than the readability threshold.
    LongSentence,
    /// A curated, established phrase reads as a cliché or stock phrase.
    Cliche,
    /// A word that usually adds nothing to the sentence.
    Filler,
    /// A multi-word expression that can be shortened.
    Wordy,
    /// A clearly informal word in an otherwise neutral text.
    Colloquial,
}

impl StyleKind {
    pub fn name(self) -> &'static str {
        match self {
            StyleKind::Repetition => "style_repetition",
            StyleKind::LongSentence => "style_long_sentence",
            StyleKind::Cliche => "style_cliche",
            StyleKind::Filler => "style_filler",
            StyleKind::Wordy => "style_wordy",
            StyleKind::Colloquial => "style_colloquial",
        }
    }
}

/// One optional style observation.
///
/// `severity` is always `Suggestion`; it is stored rather than computed so the
/// diagnostic can be serialized through the same wire shape as a grammar
/// diagnostic. `expected` carries the matched phrase or lemma. `corrections`
/// is empty unless a curated table row names a safe replacement.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleDiagnostic {
    pub kind: StyleKind,
    pub severity: Severity,
    pub confidence: DiagnosticConfidence,
    /// The smallest source span the UI should highlight.
    pub span: Range<usize>,
    /// Stable message key, normally [`StyleKind::name`].
    pub message: &'static str,
    /// The matched phrase or lemma, for the panel and debugging.
    pub expected: Option<String>,
    pub corrections: Vec<Correction>,
}

impl StyleDiagnostic {
    /// A suggestion-only diagnostic.
    pub fn new(
        kind: StyleKind,
        confidence: DiagnosticConfidence,
        span: Range<usize>,
        message: &'static str,
    ) -> Self {
        Self {
            kind,
            severity: Severity::Suggestion,
            confidence,
            span,
            message,
            expected: None,
            corrections: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_kind_has_a_stable_name() {
        for (kind, name) in [
            (StyleKind::Repetition, "style_repetition"),
            (StyleKind::LongSentence, "style_long_sentence"),
            (StyleKind::Cliche, "style_cliche"),
            (StyleKind::Filler, "style_filler"),
            (StyleKind::Wordy, "style_wordy"),
            (StyleKind::Colloquial, "style_colloquial"),
        ] {
            assert_eq!(kind.name(), name);
        }
    }

    #[test]
    fn a_style_diagnostic_is_always_a_suggestion() {
        let diagnostic = StyleDiagnostic::new(
            StyleKind::LongSentence,
            DiagnosticConfidence::Medium,
            0..4,
            StyleKind::LongSentence.name(),
        );
        assert_eq!(diagnostic.severity, Severity::Suggestion);
    }
}
