//! Grammar and punctuation diagnostics on top of the syntax analysis.
//!
//! This layer is separate from the spell checker. `check` runs
//! [`crate::syntax::analyze`], then a small rule engine over the clauses and
//! token readings, and returns diagnostics with byte spans and concrete
//! corrections. It is on-demand and must not run in the keystroke path.
//!
//! The first rule is subject–predicate agreement. The other categories in
//! [`DiagnosticKind`] are reserved for the later phases in
//! `../../misc_md/PLAN_SYNTAX_CHECKER.md`.

mod adposition;
mod agreement;
mod analysis;
mod arguments;
mod connegative;
mod correction;
mod existential;
mod numeral;
mod object_case;
mod possessive;
mod punctuation;
mod rank;
mod rection;
mod sentence_cache;
mod structure;

pub mod diagnostic;

pub use diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticConfidence, DiagnosticKind, Edit,
    Explanation, Finding, Relation, RuleId, Severity, SyntaxDiagnostic,
};
pub use sentence_cache::{SentenceCache, SentenceCacheStats};

use crate::disambiguation::{Disambiguation, disambiguate};
use crate::morphology::analyze_morphology;
use crate::syntax::{AnalysisCache, SyntaxAnalysis, analyze_syntax_with_decisions};
use crate::tokenise::Token;

/// The analysis plus the diagnostics found in it.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxCheck {
    pub analysis: SyntaxAnalysis,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}

impl SyntaxCheck {
    /// Drop diagnostics whose stable rule id the caller has ignored.
    ///
    /// Ignoring is by rule, not by span: the same `subject_predicate_agreement`
    /// stays hidden after the sentence is edited. The span-based `Edit`s and
    /// the analysis are untouched.
    pub fn without_rules(mut self, ignored: &[RuleId]) -> Self {
        self.diagnostics
            .retain(|diagnostic| !ignored.contains(&diagnostic.rule_id));
        self
    }

    /// In-place form of [`without_rules`](Self::without_rules).
    pub fn ignore_rules(&mut self, ignored: &[RuleId]) {
        self.diagnostics
            .retain(|diagnostic| !ignored.contains(&diagnostic.rule_id));
    }
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
///
/// The morphology pass and the disambiguation sidecar are shared with the
/// combined grammar/style path. Without the `neural-m0` feature the sidecar is
/// disabled and the output is identical to the pre-sidecar path.
pub fn check_with_cache(text: &str, cache: &mut AnalysisCache) -> SyntaxCheck {
    let tokens = crate::tokenise::tokenise(text);
    let morphology = analyze_morphology(&tokens, cache);
    let decisions = disambiguate(&tokens, &morphology);
    check_with_morphology_and_decisions(&tokens, &morphology, &decisions)
}

fn check_with_morphology_and_decisions(
    tokens: &[Token],
    morphology: &crate::morphology::MorphologyAnalysis,
    decisions: &Disambiguation,
) -> SyntaxCheck {
    let analysis = analyze_syntax_with_decisions(tokens, morphology, decisions);
    let diagnostics = check_analysis(tokens, &analysis);
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
    let contexts = crate::sentence::analyze(tokens);
    let mut diagnostics = check_analysis_without_quote(tokens, analysis, &contexts);
    diagnostics.extend(punctuation::quote(tokens, &contexts));
    rank::resolve(diagnostics)
}

/// The sentence-local rules. The unbalanced-quote warning is document-global
/// and is added by [`check_analysis`] or by the sentence cache.
///
/// `contexts` is the sentence pass the caller already computed; the sentence
/// cache seeds it with the quote depth at the sentence start.
pub(crate) fn check_analysis_without_quote(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    contexts: &[crate::sentence::TokenContext],
) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = agreement::check(tokens, analysis);
    diagnostics.extend(connegative::check(tokens, analysis));
    diagnostics.extend(numeral::check(tokens, analysis));
    diagnostics.extend(adposition::check(tokens, analysis));
    diagnostics.extend(existential::check(tokens, analysis));
    diagnostics.extend(structure::check(tokens, analysis));
    diagnostics.extend(punctuation::check_without_quote_with_contexts(
        tokens, analysis, contexts,
    ));
    diagnostics.extend(rection::check(tokens, analysis));
    diagnostics.extend(object_case::check(tokens, analysis));
    diagnostics.extend(possessive::check(tokens, analysis));
    diagnostics.extend(arguments::check(tokens, analysis));
    rank::resolve(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The correction-form cache must not change a single diagnostic. The
    /// first pass builds each paradigm; the second reads it back.
    #[test]
    fn a_warm_rule_pass_matches_the_first_pass() {
        let text = "Minä olet väsynyt. He tulee.";
        let tokens = crate::tokenise::tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        let first = check_analysis(&tokens, &analysis);
        assert!(!first.is_empty(), "the sample must exercise a correction");
        let second = check_analysis(&tokens, &analysis);
        assert_eq!(first, second);
    }
}
