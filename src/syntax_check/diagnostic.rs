//! The diagnostic and correction types for the syntax checker.
//!
//! Detection and correction are separate: a rule emits a [`SyntaxDiagnostic`]
//! with a stable kind and byte span, and [`Correction`]s carry the exact edits
//! that fix it. A correction can contain more than one edit (a comma pair, a
//! token replacement plus an auxiliary change), so callers apply the edit list
//! rather than replacing the whole sentence.
//!
//! The primitives (`Severity`, `Confidence`, `RuleId`, `Explanation`,
//! `Correction`) live in [`crate::diagnostic`], shared with the style layer.
//! This module adds the grammar-specific [`DiagnosticKind`] and the concrete
//! [`SyntaxDiagnostic`].

use std::ops::Range;

pub use crate::diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticConfidence, Edit, Explanation, Finding,
    Relation, RuleId, Severity,
};

/// Stable diagnostic categories. Add a new variant rather than reusing one
/// for a different rule; consumers switch on these names.
///
/// `#[non_exhaustive]`: the set grows with the grammar rules, so a downstream
/// match must have a fallback rather than fail to build on every new kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
    SubjectPredicateAgreement,
    Connegative,
    AttributeAgreement,
    PredicativeAgreement,
    PossessiveAgreement,
    VerbRection,
    CaseSelection,
    ObjectCase,
    MissingArgument,
    ExtraArgument,
    ClauseStructure,
    /// Reserved: Finnish word order is information structure, not a fixed
    /// rule (`Söi Sami puuron` is grammatical). No rule emits this kind; the
    /// construction-aware specification is `docs/word_order.md`, and when a
    /// rule does land the kind stays suggestion-only.
    WordOrder,
    CommaMissing,
    CommaExtra,
    CommaPairUnbalanced,
    SemicolonMisuse,
    SemicolonMissing,
    QuestionMarkMissing,
    UnbalancedQuote,
    ListSeparatorMissing,
    ListSeparatorDuplicate,
    ListCommaBeforeConjunction,
    ListIntroducerComma,
}

impl DiagnosticKind {
    pub fn name(self) -> &'static str {
        match self {
            DiagnosticKind::SubjectPredicateAgreement => "subject_predicate_agreement",
            DiagnosticKind::Connegative => "connegative",
            DiagnosticKind::AttributeAgreement => "attribute_agreement",
            DiagnosticKind::PredicativeAgreement => "predicative_agreement",
            DiagnosticKind::PossessiveAgreement => "possessive_agreement",
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
            DiagnosticKind::SemicolonMisuse => "semicolon_misuse",
            DiagnosticKind::SemicolonMissing => "semicolon_missing",
            DiagnosticKind::QuestionMarkMissing => "question_mark_missing",
            DiagnosticKind::UnbalancedQuote => "unbalanced_quote",
            DiagnosticKind::ListSeparatorMissing => "list_separator_missing",
            DiagnosticKind::ListSeparatorDuplicate => "list_separator_duplicate",
            DiagnosticKind::ListCommaBeforeConjunction => "list_comma_before_conjunction",
            DiagnosticKind::ListIntroducerComma => "list_introducer_comma",
        }
    }

    /// The grammatical relation this kind reports.
    fn relation(self, message: &str) -> Relation {
        match self {
            DiagnosticKind::SubjectPredicateAgreement | DiagnosticKind::Connegative => {
                Relation::SubjectPredicate
            }
            DiagnosticKind::AttributeAgreement => Relation::AttributeHead,
            DiagnosticKind::PredicativeAgreement => Relation::PredicativeComplement,
            DiagnosticKind::PossessiveAgreement => Relation::Possessive,
            DiagnosticKind::VerbRection => Relation::VerbFrame,
            DiagnosticKind::CaseSelection if message == "numeral_case" => {
                Relation::NumeralComplement
            }
            DiagnosticKind::CaseSelection if message == "adposition_case" => {
                Relation::AdpositionComplement
            }
            DiagnosticKind::CaseSelection if message == "existential_subject_case" => {
                Relation::SubjectCase
            }
            DiagnosticKind::CaseSelection => Relation::Other,
            DiagnosticKind::ObjectCase => Relation::ObjectCase,
            DiagnosticKind::MissingArgument | DiagnosticKind::ExtraArgument => Relation::Argument,
            DiagnosticKind::ClauseStructure | DiagnosticKind::WordOrder => {
                Relation::ClauseStructure
            }
            DiagnosticKind::CommaMissing
            | DiagnosticKind::CommaExtra
            | DiagnosticKind::CommaPairUnbalanced
            | DiagnosticKind::SemicolonMisuse
            | DiagnosticKind::SemicolonMissing
            | DiagnosticKind::QuestionMarkMissing
            | DiagnosticKind::UnbalancedQuote
            | DiagnosticKind::ListSeparatorMissing
            | DiagnosticKind::ListSeparatorDuplicate
            | DiagnosticKind::ListCommaBeforeConjunction
            | DiagnosticKind::ListIntroducerComma => Relation::Punctuation,
        }
    }
}

/// A detected grammar or punctuation problem and the corrections that fix it.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxDiagnostic {
    pub kind: DiagnosticKind,
    pub severity: Severity,
    pub confidence: Confidence,
    /// The smallest source span the UI should underline.
    pub span: Range<usize>,
    /// Stable message key; the caller localizes it.
    pub message: &'static str,
    /// What the rule expected, for diagnostics/debugging.
    pub expected: Option<String>,
    /// Stable relation/key explanation, independent of any correction.
    pub explanation: Option<Explanation>,
    pub corrections: Vec<Correction>,
}

impl SyntaxDiagnostic {
    pub fn new(
        kind: DiagnosticKind,
        severity: Severity,
        confidence: Confidence,
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
            explanation: Some(Explanation::new(kind.relation(message), message)),
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

impl Finding for SyntaxDiagnostic {
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
    fn a_diagnostic_carries_a_stable_rule_and_explanation() {
        let diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::CaseSelection,
            Severity::Error,
            Confidence::Certain,
            0..3,
            "adposition_case",
        );
        assert_eq!(diagnostic.rule_id().as_str(), "adposition_case");
        let explanation = diagnostic.explanation.expect("an explanation is set");
        assert_eq!(explanation.relation, Relation::AdpositionComplement);
        assert_eq!(explanation.key, "adposition_case");
    }

    #[test]
    fn the_relation_distinguishes_the_two_case_selection_rules() {
        let numeral = SyntaxDiagnostic::new(
            DiagnosticKind::CaseSelection,
            Severity::Error,
            Confidence::Certain,
            0..3,
            "numeral_case",
        );
        let adposition = SyntaxDiagnostic::new(
            DiagnosticKind::CaseSelection,
            Severity::Error,
            Confidence::Certain,
            0..3,
            "adposition_case",
        );
        assert_eq!(
            numeral.explanation.expect("explanation").relation,
            Relation::NumeralComplement
        );
        assert_eq!(
            adposition.explanation.expect("explanation").relation,
            Relation::AdpositionComplement
        );
    }
}
