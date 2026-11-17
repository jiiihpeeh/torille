//! The shared diagnostic contract for grammar and style.
//!
//! Both layers emit the same primitives: a stable [`RuleId`], an independent
//! [`Severity`] and [`Confidence`], a byte [`Range`], a stable message key, an
//! optional [`Explanation`], and optional [`Correction`]s. A finding is never
//! required to carry a rewrite: detection and correction are separate.
//!
//! The syntax layer keeps its [`crate::syntax_check::DiagnosticKind`] and the
//! style layer its [`crate::style_check::StyleKind`]; the [`Finding`] trait is
//! the common surface a panel or an ignore list consumes.

use std::ops::Range;

/// How serious a finding is.
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
///
/// The states are structural, not numerical:
///
/// * `Certain` — one clear reading; an `Error` is allowed.
/// * `Likely` — one preferred reading with a known competitor; use `Warning`
///   or `Suggestion`, never `Error`.
/// * `Ambiguous` — competing readings; the finding is suppressed before it
///   reaches a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    Certain,
    Likely,
    Ambiguous,
}

impl Confidence {
    pub fn name(self) -> &'static str {
        match self {
            Confidence::Certain => "certain",
            Confidence::Likely => "likely",
            Confidence::Ambiguous => "ambiguous",
        }
    }

    /// Whether the finding may be shown at all.
    pub fn is_actionable(self) -> bool {
        self != Confidence::Ambiguous
    }

    /// The structural state implied by a role reading.
    pub fn from_role(role: crate::syntax::RoleConfidence) -> Self {
        match role {
            crate::syntax::RoleConfidence::Likely => Confidence::Certain,
            crate::syntax::RoleConfidence::Weak => Confidence::Likely,
        }
    }
}

/// Compatibility alias for the old confidence type name.
pub type DiagnosticConfidence = Confidence;

#[allow(non_upper_case_globals)]
impl Confidence {
    /// Old name for [`Confidence::Certain`].
    pub const High: Self = Self::Certain;
    /// Old name for [`Confidence::Likely`].
    pub const Medium: Self = Self::Likely;
    /// Old name for [`Confidence::Ambiguous`].
    pub const Low: Self = Self::Ambiguous;
}

/// A stable rule identifier, suitable for ignore/dismiss lists.
///
/// The identifier is the rule's message key, not a span: ignoring
/// `subject_predicate_agreement` must survive edits to the sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(&'static str);

impl RuleId {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// The grammatical relation that triggered a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    SubjectPredicate,
    AttributeHead,
    PredicativeComplement,
    VerbFrame,
    AdpositionComplement,
    NumeralComplement,
    ObjectCase,
    Argument,
    ClauseStructure,
    Punctuation,
    Repetition,
    Length,
    Phrase,
    WordChoice,
    Other,
}

impl Relation {
    pub fn name(self) -> &'static str {
        match self {
            Relation::SubjectPredicate => "subject_predicate",
            Relation::AttributeHead => "attribute_head",
            Relation::PredicativeComplement => "predicative_complement",
            Relation::VerbFrame => "verb_frame",
            Relation::AdpositionComplement => "adposition_complement",
            Relation::NumeralComplement => "numeral_complement",
            Relation::ObjectCase => "object_case",
            Relation::Argument => "argument",
            Relation::ClauseStructure => "clause_structure",
            Relation::Punctuation => "punctuation",
            Relation::Repetition => "repetition",
            Relation::Length => "length",
            Relation::Phrase => "phrase",
            Relation::WordChoice => "word_choice",
            Relation::Other => "other",
        }
    }
}

/// The stable, localisable explanation of why a finding exists.
///
/// `key` is separate from the message so a UI can show a sentence-local
/// explanation without depending on a correction. The expected category stays
/// on the diagnostic (`expected`) because it is also useful to the debug view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Explanation {
    /// The grammatical relation that triggered the finding.
    pub relation: Relation,
    /// Stable explanation key, normally the diagnostic's message.
    pub key: &'static str,
}

impl Explanation {
    pub const fn new(relation: Relation, key: &'static str) -> Self {
        Self { relation, key }
    }
}

/// What licenses a correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CorrectionLicense {
    /// An exact form from the inflection engine (`case_corrections`, ...).
    Inflection,
    /// A curated table row (style phrases, verb frames, adpositions).
    Curated,
    /// A mechanical punctuation edit.
    Punctuation,
    /// No provenance recorded yet.
    Other,
}

impl CorrectionLicense {
    pub fn name(self) -> &'static str {
        match self {
            CorrectionLicense::Inflection => "inflection",
            CorrectionLicense::Curated => "curated",
            CorrectionLicense::Punctuation => "punctuation",
            CorrectionLicense::Other => "other",
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

/// One way to fix a finding. Applying every edit yields a corrected text.
#[derive(Debug, Clone, PartialEq)]
pub struct Correction {
    pub edits: Vec<Edit>,
    /// Lower is better; populated by the ranker.
    pub score: f32,
    /// Stable note key for the UI/debugging.
    pub note: &'static str,
    /// Where the replacement came from.
    pub license: CorrectionLicense,
    /// How safe the replacement is on its own.
    pub confidence: Confidence,
}

impl Correction {
    pub fn new(edits: Vec<Edit>, score: f32, note: &'static str) -> Self {
        Self {
            edits,
            score,
            note,
            license: CorrectionLicense::Other,
            confidence: Confidence::Likely,
        }
    }

    /// A single-token replacement.
    pub fn replacement(start: usize, end: usize, word: impl Into<String>) -> Self {
        Self::new(vec![Edit::new(start, end, word)], 0.0, "replacement")
    }

    /// Record where the correction comes from.
    pub fn licensed_by(mut self, license: CorrectionLicense) -> Self {
        self.license = license;
        self
    }

    /// Record how safe the correction is on its own.
    pub fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }
}

/// The shared surface of a grammar or style finding.
pub trait Finding {
    fn rule_id(&self) -> RuleId;
    fn severity(&self) -> Severity;
    fn confidence(&self) -> Confidence;
    fn span(&self) -> &Range<usize>;
    fn message(&self) -> &'static str;
    fn explanation(&self) -> Option<&Explanation>;
    fn corrections(&self) -> &[Correction];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_names_are_stable() {
        assert_eq!(Confidence::Certain.name(), "certain");
        assert_eq!(Confidence::Likely.name(), "likely");
        assert_eq!(Confidence::Ambiguous.name(), "ambiguous");
        assert!(!Confidence::Ambiguous.is_actionable());
    }

    #[test]
    fn relation_names_are_stable() {
        assert_eq!(Relation::SubjectPredicate.name(), "subject_predicate");
        assert_eq!(Relation::AdpositionComplement.name(), "adposition_complement");
        assert_eq!(Relation::Punctuation.name(), "punctuation");
    }

    #[test]
    fn a_correction_records_its_provenance() {
        let correction = Correction::replacement(0, 3, "talo")
            .licensed_by(CorrectionLicense::Inflection)
            .with_confidence(Confidence::Certain);
        assert_eq!(correction.license, CorrectionLicense::Inflection);
        assert_eq!(correction.confidence, Confidence::Certain);
    }
}
