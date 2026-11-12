//! The opt-in style layer.
//!
//! This is deliberately separate from the grammar checker. It answers an
//! optional question — “is this wording repetitive, wordy or clichéd?” — and
//! never emits `Error` or `Warning`. It is on demand and must not run in the
//! keystroke path. The design is in `PLAN_STYLE.md`.
//!
//! Phase 1 is the skeleton: the public types and an empty rule pass. Phrase
//! tables, repetition and the long-sentence metric land in later phases.

mod diagnostic;

pub use diagnostic::{StyleDiagnostic, StyleKind};

use crate::tokenise::Token;

/// The diagnostics found in one text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleAnalysis {
    pub diagnostics: Vec<StyleDiagnostic>,
}

/// Analyze a text for optional style observations.
///
/// The pass is linear and on demand. It reuses the tokeniser and, when a rule
/// needs morphology, the shared syntax cache.
pub fn check(text: &str) -> StyleAnalysis {
    let tokens = crate::tokenise::tokenise(text);
    check_tokens(&tokens)
}

/// Run the style rules over an existing token stream.
///
/// This is the testable core and the phase 2 entry point for the table and
/// metric rules. Until then it returns an empty analysis.
pub fn check_tokens(_tokens: &[Token]) -> StyleAnalysis {
    StyleAnalysis {
        diagnostics: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax_check::Severity;

    #[test]
    fn an_empty_layer_returns_no_diagnostics() {
        let analysis = check("Talo on kaunis. Kissa istuu.");
        assert!(analysis.diagnostics.is_empty());
    }

    #[test]
    fn the_style_contract_is_suggestion_only() {
        // This holds trivially while the layer is empty, but it pins the
        // contract for every rule added later.
        let analysis = check("Tämä on hyvä asia. Asia on hyvä.");
        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity == Severity::Suggestion)
        );
    }

    #[test]
    fn check_and_check_tokens_agree() {
        let text = "Talo on kaunis. Kissa istuu.";
        let tokens = crate::tokenise::tokenise(text);
        assert_eq!(check(text), check_tokens(&tokens));
    }
}
