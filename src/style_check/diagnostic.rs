//! Types for the opt-in style layer.
//!
//! Style diagnostics reuse the grammar checker's small diagnostic primitives
//! (`Severity`, `Confidence`, `Correction`) but keep their own kind
//! enum. A style finding is always [`Severity::Suggestion`]; the constructor
//! enforces that, so a rule cannot accidentally turn a preference into an
//! error.

use std::ops::Range;

use crate::diagnostic::{
    Confidence, Correction, Explanation, Finding, Relation, RuleId, Severity,
};

/// Stable style categories. Keep the names distinct from the grammar kinds;
/// a consumer must be able to tell the two streams apart even when a panel
/// merges them.
///
/// `#[non_exhaustive]`: the table grows, so a downstream match must have a
/// fallback rather than fail to build on every new kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum StyleKind {
    /// The same content lemma repeats close enough to read as an echo.
    Repetition,
    /// The same surface is written twice in a row with nothing between.
    DuplicateWord,
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
            StyleKind::DuplicateWord => "style_duplicate_word",
            StyleKind::LongSentence => "style_long_sentence",
            StyleKind::Cliche => "style_cliche",
            StyleKind::Filler => "style_filler",
            StyleKind::Wordy => "style_wordy",
            StyleKind::Colloquial => "style_colloquial",
        }
    }

    fn relation(self) -> Relation {
        match self {
            StyleKind::Repetition | StyleKind::DuplicateWord => Relation::Repetition,
            StyleKind::LongSentence => Relation::Length,
            StyleKind::Cliche | StyleKind::Wordy => Relation::Phrase,
            StyleKind::Filler | StyleKind::Colloquial => Relation::WordChoice,
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
    pub confidence: Confidence,
    /// The smallest source span the UI should highlight.
    pub span: Range<usize>,
    /// Stable message key, normally [`StyleKind::name`].
    pub message: &'static str,
    /// The matched phrase or lemma, for the panel and debugging.
    pub expected: Option<String>,
    /// Stable relation/key explanation, independent of any correction.
    pub explanation: Option<Explanation>,
    pub corrections: Vec<Correction>,
}

impl StyleDiagnostic {
    /// A suggestion-only diagnostic.
    pub fn new(
        kind: StyleKind,
        confidence: Confidence,
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
            explanation: Some(Explanation::new(kind.relation(), message)),
            corrections: Vec::new(),
        }
    }

    /// The stable rule identifier for ignore/dismiss lists.
    ///
    /// Derived from [`Self::message`], which is the rule's key, so the two
    /// cannot disagree. This was a public field; a caller could set it to a
    /// different string than the message it was built from.
    pub fn rule_id(&self) -> RuleId {
        RuleId::new(self.message)
    }
}

impl Finding for StyleDiagnostic {
    fn rule_id(&self) -> RuleId {
        self.rule_id()
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn confidence(&self) -> Confidence {
        self.confidence
    }

    fn span(&self) -> &Range<usize> {
        &self.span
    }

    fn message(&self) -> &'static str {
        self.message
    }

    fn explanation(&self) -> Option<&Explanation> {
        self.explanation.as_ref()
    }

    fn corrections(&self) -> &[Correction] {
        &self.corrections
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_kind_has_a_stable_name() {
        for (kind, name) in [
            (StyleKind::Repetition, "style_repetition"),
            (StyleKind::DuplicateWord, "style_duplicate_word"),
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
            Confidence::Likely,
            0..4,
            StyleKind::LongSentence.name(),
        );
        assert_eq!(diagnostic.severity, Severity::Suggestion);
    }
}
