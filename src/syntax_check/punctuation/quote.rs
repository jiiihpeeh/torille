//! Unbalanced-quote warning.

use crate::sentence;
use crate::tokenise::Token;

use super::super::diagnostic::{
    DiagnosticConfidence, DiagnosticKind, Severity, SyntaxDiagnostic,
};

/// One warning for a quote left open at the end of the document.
pub(super) fn quote_diagnostics(
    tokens: &[Token],
    contexts: &[sentence::TokenContext],
) -> Vec<SyntaxDiagnostic> {
    let depth = contexts
        .last()
        .map(|context| context.quote_depth)
        .unwrap_or(0);
    if depth == 0 {
        return Vec::new();
    }
    let at = tokens.last().map(|token| token.end).unwrap_or(0);
    vec![SyntaxDiagnostic::new(
        DiagnosticKind::UnbalancedQuote,
        Severity::Warning,
        DiagnosticConfidence::Medium,
        at..at,
        "unbalanced_quote",
    )]
}
