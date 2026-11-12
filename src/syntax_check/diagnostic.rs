//! The diagnostic and correction types for the syntax checker.
//!
//! Detection and correction are separate: a rule emits a [`SyntaxDiagnostic`]
//! with a stable kind and byte span, and [`Correction`]s carry the exact edits
//! that fix it. A correction can contain more than one edit (a comma pair, a
//! token replacement plus an auxiliary change), so callers apply the edit list
//! rather than replacing the whole sentence.

use std::ops::Range;

/// How serious a diagnostic is.
///
/// `Error` means standard written Finnish is violated. `Warning` covers a
/// construction that is likely wrong but has variation (coordinated subjects,
/// colloquial number). `Suggestion` is optional punctuation/style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Suggestion,
}

impl Severity {
    pub fn name(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Suggestion => "suggestion",
        }
    }
}

/// How much the checker trusts the diagnosis itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticConfidence {
    High,
    Medium,
    Low,
}

impl DiagnosticConfidence {
    pub fn name(self) -> &'static str {
        match self {
            DiagnosticConfidence::High => "high",
            DiagnosticConfidence::Medium => "medium",
            DiagnosticConfidence::Low => "low",
        }
    }
}

/// Stable diagnostic categories. Add a new variant rather than reusing one
/// for a different rule; consumers switch on these names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticKind {
    SubjectPredicateAgreement,
    AttributeAgreement,
    VerbRection,
    CaseSelection,
    ObjectCase,
    MissingArgument,
    ExtraArgument,
    ClauseStructure,
    WordOrder,
    CommaMissing,
    CommaExtra,
    CommaPairUnbalanced,
    ListSeparatorMissing,
    ListSeparatorDuplicate,
    ListCommaBeforeConjunction,
    ListIntroducerComma,
}

impl DiagnosticKind {
    pub fn name(self) -> &'static str {
        match self {
            DiagnosticKind::SubjectPredicateAgreement => "subject_predicate_agreement",
            DiagnosticKind::AttributeAgreement => "attribute_agreement",
            DiagnosticKind::VerbRection => "verb_rection",
            DiagnosticKind::CaseSelection => "case_selection",
            DiagnosticKind::ObjectCase => "object_case",
            DiagnosticKind::MissingArgument => "missing_argument",
            DiagnosticKind::ExtraArgument => "extra_argument",
            DiagnosticKind::ClauseStructure => "clause_structure",
            DiagnosticKind::WordOrder => "word_order",
            DiagnosticKind::CommaMissing => "comma_missing",
            DiagnosticKind::CommaExtra => "comma_extra",
            DiagnosticKind::CommaPairUnbalanced => "comma_pair_unbalanced",
            DiagnosticKind::ListSeparatorMissing => "list_separator_missing",
            DiagnosticKind::ListSeparatorDuplicate => "list_separator_duplicate",
            DiagnosticKind::ListCommaBeforeConjunction => "list_comma_before_conjunction",
            DiagnosticKind::ListIntroducerComma => "list_introducer_comma",
        }
    }
}

/// One replacement in the source text. `start == end` is an insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

impl Edit {
    pub fn new(start: usize, end: usize, replacement: impl Into<String>) -> Self {
        Self {
            start,
            end,
            replacement: replacement.into(),
        }
    }

    pub fn insertion(at: usize, replacement: impl Into<String>) -> Self {
        Self::new(at, at, replacement)
    }
}

/// One way to fix a diagnostic. Applying every edit yields a corrected text.
#[derive(Debug, Clone, PartialEq)]
pub struct Correction {
    pub edits: Vec<Edit>,
    /// Lower is better; populated by the ranker.
    pub score: f32,
    /// Stable note key for the UI/debugging.
    pub note: &'static str,
}

impl Correction {
    pub fn new(edits: Vec<Edit>, score: f32, note: &'static str) -> Self {
        Self {
            edits,
            score,
            note,
        }
    }

    /// A single-token replacement.
    pub fn replacement(start: usize, end: usize, word: impl Into<String>) -> Self {
        Self::new(
            vec![Edit::new(start, end, word)],
            0.0,
            "replacement",
        )
    }
}

/// A detected grammar or punctuation problem and the corrections that fix it.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxDiagnostic {
    pub kind: DiagnosticKind,
    pub severity: Severity,
    pub confidence: DiagnosticConfidence,
    /// The smallest source span the UI should underline.
    pub span: Range<usize>,
    /// Stable message key; the caller localizes it.
    pub message: &'static str,
    /// What the rule expected, for diagnostics/debugging.
    pub expected: Option<String>,
    pub corrections: Vec<Correction>,
}

impl SyntaxDiagnostic {
    pub fn new(
        kind: DiagnosticKind,
        severity: Severity,
        confidence: DiagnosticConfidence,
        span: Range<usize>,
        message: &'static str,
    ) -> Self {
        Self {
            kind,
            severity,
            confidence,
            span,
            message,
            expected: None,
            corrections: Vec::new(),
        }
    }
}
