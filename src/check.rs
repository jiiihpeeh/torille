//! The combined grammar + style pass.
//!
//! Both layers consume one morphology pass. This facade is the entry point a
//! caller uses when it wants both streams: the per-textarea wasm state, the
//! combined worker request, or a CLI. It never runs in the keystroke path.

use crate::diagnostic::Finding;
use crate::disambiguation::{Disambiguation, disambiguate};
use crate::morphology::{MorphologyAnalysis, MorphologyCache, analyze_morphology};
use crate::style_check::{StyleAnalysis, check_tokens_with_morphology};
use crate::syntax::analyze_syntax_with_decisions;
use crate::syntax_check::{SyntaxCheck, check_analysis};
use crate::tokenise::{Token, tokenise};

/// The grammar and style diagnostics for one text.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckAll {
    pub syntax: SyntaxCheck,
    pub style: StyleAnalysis,
}

impl CheckAll {
    /// Both streams through the shared [`Finding`] contract.
    ///
    /// A panel or an ignore list can treat grammar and style alike without
    /// matching on the concrete diagnostic type.
    pub fn findings(&self) -> Vec<&dyn Finding> {
        self.syntax
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic as &dyn Finding)
            .chain(
                self.style
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic as &dyn Finding),
            )
            .collect()
    }
}

/// Check a text with the embedded lexicon and a thread-local cache.
pub fn check_all(text: &str) -> CheckAll {
    thread_local! {
        static CACHE: std::cell::RefCell<MorphologyCache> =
            std::cell::RefCell::new(MorphologyCache::new());
    }
    CACHE.with_borrow_mut(|cache| check_all_with_cache(text, cache))
}

/// Check a text through a caller-owned morphology cache.
///
/// The cache is the same one the syntax layer uses; a grammar re-check and a
/// style re-check of the same document therefore share every resolved word.
pub fn check_all_with_cache(text: &str, cache: &mut MorphologyCache) -> CheckAll {
    let tokens = tokenise(text);
    let morphology = analyze_morphology(&tokens, cache);
    let decisions = disambiguate(&tokens, &morphology);
    check_all_tokens_with_decisions(&tokens, &morphology, &decisions)
}

/// Check an existing token stream and morphology pass.
pub fn check_all_tokens(tokens: &[Token], morphology: &MorphologyAnalysis) -> CheckAll {
    check_all_tokens_with_decisions(tokens, morphology, &Disambiguation::disabled())
}

/// Check from an existing morphology pass and an explicit disambiguation
/// sidecar.
///
/// Without the `neural-m0` feature the caller passes
/// [`Disambiguation::disabled`] and the path is byte-for-byte the same as
/// [`check_all_tokens`]. The sidecar is not allowed to change style.
pub fn check_all_tokens_with_decisions(
    tokens: &[Token],
    morphology: &MorphologyAnalysis,
    decisions: &Disambiguation,
) -> CheckAll {
    let analysis = analyze_syntax_with_decisions(tokens, morphology, decisions);
    let diagnostics = check_analysis(tokens, &analysis);
    let style = check_tokens_with_morphology(tokens, morphology);
    CheckAll {
        syntax: SyntaxCheck {
            analysis,
            diagnostics,
        },
        style,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_cache_serves_both_layers() {
        let text = "Koira nukkuvat. Tällä hetkellä tilanne on rauhallinen.";
        let mut cache = MorphologyCache::new();
        let all = check_all_with_cache(text, &mut cache);
        let syntax = crate::syntax_check::check_with_cache(text, &mut cache);
        let style = crate::style_check::check_with_cache(text, &mut cache);
        assert_eq!(all.syntax, syntax);
        assert_eq!(all.style, style);
    }

    #[test]
    fn the_combined_pass_uses_one_morphology_result() {
        let text = "Koira nukkuvat. Tällä hetkellä tilanne on rauhallinen.";
        let tokens = tokenise(text);
        let mut cache = MorphologyCache::new();
        let morphology = analyze_morphology(&tokens, &mut cache);
        let all = check_all_tokens(&tokens, &morphology);
        assert_eq!(all.syntax.diagnostics.len(), 1);
        assert_eq!(all.style.diagnostics.len(), 1);
    }

    #[test]
    fn both_layers_share_the_finding_contract() {
        use crate::diagnostic::Severity;
        let all = check_all("Koira nukkuvat. Tällä hetkellä tilanne on rauhallinen.");
        let findings = all.findings();
        assert_eq!(findings.len(), 2);
        assert_eq!(
            findings[0].rule_id().as_str(),
            "subject_predicate_agreement"
        );
        assert!(findings[1].rule_id().as_str().starts_with("style_"));
        assert_eq!(findings[1].severity(), Severity::Suggestion);
        assert!(findings.iter().all(|finding| finding.explanation().is_some()));
    }
}
