//! Deterministic ordering and conflict resolution for diagnostics.
//!
//! The UI and the golden tests both depend on a stable order: severity first,
//! then confidence, then source position. Two rules that report the same kind
//! at the same span collapse to the higher-ranked one.

use super::diagnostic::{DiagnosticConfidence, SyntaxDiagnostic};

pub(super) fn resolve(mut diagnostics: Vec<SyntaxDiagnostic>) -> Vec<SyntaxDiagnostic> {
    for diagnostic in &mut diagnostics {
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
    diagnostics.dedup_by(|a, b| a.kind == b.kind && a.span == b.span);
    diagnostics
}

fn confidence_rank(confidence: DiagnosticConfidence) -> u8 {
    match confidence {
        DiagnosticConfidence::High => 0,
        DiagnosticConfidence::Medium => 1,
        DiagnosticConfidence::Low => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax_check::diagnostic::{DiagnosticKind, Severity};

    #[test]
    fn errors_come_first_and_duplicates_collapse() {
        let low = SyntaxDiagnostic::new(
            DiagnosticKind::WordOrder,
            Severity::Suggestion,
            DiagnosticConfidence::Low,
            10..12,
            "word_order",
        );
        let error = SyntaxDiagnostic::new(
            DiagnosticKind::SubjectPredicateAgreement,
            Severity::Error,
            DiagnosticConfidence::High,
            4..9,
            "subject_predicate_agreement",
        );
        let duplicate = error.clone();
        let resolved = resolve(vec![low, error, duplicate]);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].severity, Severity::Error);
        assert_eq!(resolved[1].severity, Severity::Suggestion);
    }
}
