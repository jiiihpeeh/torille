//! The combined grammar + style pass.
//!
//! Both layers consume one morphology pass. This facade is the entry point a
//! caller uses when it wants both streams: the per-textarea wasm state, the
//! combined worker request, or a CLI. It never runs in the keystroke path.

use crate::morphology::{MorphologyAnalysis, MorphologyCache, analyze_morphology};
use crate::style_check::{StyleAnalysis, check_tokens_with_morphology};
use crate::syntax::analyze_syntax;
use crate::syntax_check::{SyntaxCheck, check_analysis};
use crate::tokenise::{Token, tokenise};

/// The grammar and style diagnostics for one text.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckAll {
    pub syntax: SyntaxCheck,
    pub style: StyleAnalysis,
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
    check_all_tokens(&tokens, &morphology)
}

/// Check an existing token stream and morphology pass.
pub fn check_all_tokens(tokens: &[Token], morphology: &MorphologyAnalysis) -> CheckAll {
    let analysis = analyze_syntax(tokens, morphology);
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
}
