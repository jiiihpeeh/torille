//! Deterministic ordering and conflict resolution for diagnostics.
//!
//! The UI and the golden tests both depend on a stable order: severity first,
//! then confidence, then source position. Two rules that report the same kind
//! at the same span collapse to the higher-ranked one.

use super::diagnostic::{Confidence, Severity, SyntaxDiagnostic};

pub(super) fn resolve(mut diagnostics: Vec<SyntaxDiagnostic>) -> Vec<SyntaxDiagnostic> {
    // Ambiguous findings are suppressed before they reach a caller: competing
    // readings are not evidence.
    diagnostics.retain(|diagnostic| diagnostic.confidence.is_actionable());
    // A rule whose preconditions are only `Likely` may warn or suggest, never
    // claim an error. Enforce the contract centrally so a new rule cannot
    // silently bypass it.
    for diagnostic in &mut diagnostics {
        if diagnostic.severity == Severity::Error
            && diagnostic.confidence != Confidence::Certain
        {
            diagnostic.severity = Severity::Warning;
        }
        diagnostic.corrections.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| a.note.cmp(b.note))
        });
    }
    diagnostics.sort_by(|a, b| {
        // Error before Warning before Suggestion.
        a.severity
            .cmp(&b.severity)
            .then_with(|| confidence_rank(a.confidence).cmp(&confidence_rank(b.confidence)))
            .then_with(|| a.span.start.cmp(&b.span.start))
            .then_with(|| a.span.end.cmp(&b.span.end))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    // One diagnostic per span: a rection finding wins over the more general
    // negative-object case when both describe the same token.
    diagnostics.dedup_by(|a, b| a.span == b.span);
    diagnostics
}

fn confidence_rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::Certain => 0,
        Confidence::Likely => 1,
        Confidence::Ambiguous => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax_check::diagnostic::{DiagnosticKind, Severity};

    #[test]
    fn errors_come_first_and_duplicates_collapse() {
        let low = SyntaxDiagnostic::new(
            DiagnosticKind::CommaMissing,
            Severity::Suggestion,
            Confidence::Likely,
            10..12,
            "comma_missing",
        );
        let error = SyntaxDiagnostic::new(
            DiagnosticKind::SubjectPredicateAgreement,
            Severity::Error,
            Confidence::Certain,
            4..9,
            "subject_predicate_agreement",
        );
        let duplicate = error.clone();
        let resolved = resolve(vec![low, error, duplicate]);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].severity, Severity::Error);
        assert_eq!(resolved[1].severity, Severity::Suggestion);
    }

    #[test]
    fn an_ambiguous_finding_is_suppressed() {
        let ambiguous = SyntaxDiagnostic::new(
            DiagnosticKind::SubjectPredicateAgreement,
            Severity::Warning,
            Confidence::Ambiguous,
            0..3,
            "subject_predicate_agreement",
        );
        assert!(resolve(vec![ambiguous]).is_empty());
    }

    #[test]
    fn a_likely_finding_is_never_an_error() {
        let likely = SyntaxDiagnostic::new(
            DiagnosticKind::SubjectPredicateAgreement,
            Severity::Error,
            Confidence::Likely,
            0..3,
            "subject_predicate_agreement",
        );
        let resolved = resolve(vec![likely]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].severity, Severity::Warning);
    }
}
