//! Grammar and punctuation diagnostics on top of the syntax analysis.
//!
//! This layer is separate from the spell checker. `check` runs
//! [`crate::syntax::analyze`], then a small rule engine over the clauses and
//! token readings, and returns diagnostics with byte spans and concrete
//! corrections. It is on-demand and must not run in the keystroke path.
//!
//! The first rule is subject–predicate agreement. The other categories in
//! [`DiagnosticKind`] are reserved for the later phases in
//! `PLAN_SYNTAX_CHECKER.md`.

mod agreement;
mod analysis;
mod arguments;
mod correction;
mod object_case;
mod punctuation;
mod rank;
mod rection;

pub mod diagnostic;

pub use diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

use crate::syntax::{AnalysisCache, SyntaxAnalysis};
use crate::tokenise::Token;

/// The analysis plus the diagnostics found in it.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxCheck {
    pub analysis: SyntaxAnalysis,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}

/// Analyse and check a text with the embedded lexicon.
pub fn check(text: &str) -> SyntaxCheck {
    let tokens = crate::tokenise::tokenise(text);
    let analysis = crate::syntax::analyze(&tokens);
    let diagnostics = check_analysis(&tokens, &analysis);
    SyntaxCheck {
        analysis,
        diagnostics,
    }
}

/// The same check through a caller-owned word cache.
pub fn check_with_cache(text: &str, cache: &mut AnalysisCache) -> SyntaxCheck {
    let tokens = crate::tokenise::tokenise(text);
    let analysis = crate::syntax::analyze_with_cache(&tokens, cache);
    let diagnostics = check_analysis(&tokens, &analysis);
    SyntaxCheck {
        analysis,
        diagnostics,
    }
}

/// Run the rules over an existing analysis.
///
/// This is the testable core: a caller that already has tokens and an analysis
/// does not pay for a second parse.
pub fn check_analysis(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = agreement::check(tokens, analysis);
    diagnostics.extend(punctuation::check(tokens, analysis));
    diagnostics.extend(rection::check(tokens, analysis));
    diagnostics.extend(object_case::check(tokens, analysis));
    diagnostics.extend(arguments::check(tokens, analysis));
    rank::resolve(diagnostics)
}
