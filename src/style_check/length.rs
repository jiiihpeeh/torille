//! The long-sentence readability nudge.
//!
//! A sentence over 35 word tokens is reported as a suggestion. The rule skips
//! headings and list fragments (no finite predicate) and a sentence that
//! already has a word-level finding, so one sentence never carries a pile of
//! overlapping advice.

use std::ops::Range;

use crate::sentence;
use crate::syntax::{SyntaxAnalysis, is_finite_predicate};
use crate::syntax_check::DiagnosticConfidence;
use crate::tokenise::Token;

use super::{StyleDiagnostic, StyleKind};

/// Sentences at or below this many word tokens are quiet.
pub(super) const LONG_SENTENCE_WORDS: usize = 35;

pub(super) fn check(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    existing: &[StyleDiagnostic],
) -> Vec<StyleDiagnostic> {
    let mut diagnostics = Vec::new();
    for span in sentence::spans(tokens) {
        let word_count = tokens
            .iter()
            .filter(|token| token.is_word() && span.start <= token.start && token.end <= span.end)
            .count();
        if word_count <= LONG_SENTENCE_WORDS {
            continue;
        }
        if !has_finite_predicate(tokens, analysis, span.start..span.end) {
            continue;
        }
        if existing
            .iter()
            .any(|diagnostic| range_contains(span.start..span.end, &diagnostic.span))
        {
            continue;
        }
        let mut diagnostic = StyleDiagnostic::new(
            StyleKind::LongSentence,
            DiagnosticConfidence::Medium,
            span.start..span.end,
            StyleKind::LongSentence.name(),
        );
        diagnostic.expected = Some(format!("enintään {LONG_SENTENCE_WORDS} sanaa"));
        diagnostics.push(diagnostic);
    }
    diagnostics
}

fn has_finite_predicate(tokens: &[Token], analysis: &SyntaxAnalysis, span: Range<usize>) -> bool {
    tokens.iter().enumerate().any(|(index, token)| {
        span.start <= token.start
            && token.end <= span.end
            && analysis.tokens[index]
                .readings
                .iter()
                .any(is_finite_predicate)
    })
}

fn range_contains(outer: Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<StyleDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        check(&tokens, &analysis, &[])
    }

    fn sentence_with(words: usize) -> String {
        // `Hän on` gives a finite predicate; the `ja on` tail pads the count
        // without repeating a content word.
        let mut text = String::from("Hän on");
        let mut count = 2;
        while count + 2 <= words {
            text.push_str(" ja on");
            count += 2;
        }
        if count < words {
            text.push_str(" nyt");
        }
        text.push('.');
        assert_eq!(
            tokenise(&text)
                .iter()
                .filter(|token| token.is_word())
                .count(),
            words
        );
        text
    }

    #[test]
    fn a_thirty_four_word_sentence_is_quiet() {
        assert!(diagnostics(&sentence_with(34)).is_empty());
    }

    #[test]
    fn a_thirty_five_word_sentence_is_quiet() {
        assert!(diagnostics(&sentence_with(35)).is_empty());
    }

    #[test]
    fn a_thirty_six_word_sentence_is_a_long_sentence() {
        let found = diagnostics(&sentence_with(36));
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, StyleKind::LongSentence);
        assert_eq!(found[0].expected.as_deref(), Some("enintään 35 sanaa"));
        assert!(found[0].corrections.is_empty());
    }

    #[test]
    fn a_fragment_without_a_finite_predicate_is_quiet() {
        let text = "sana ".repeat(LONG_SENTENCE_WORDS + 1) + ".";
        assert!(diagnostics(&text).is_empty());
    }

    #[test]
    fn a_sentence_with_a_word_level_finding_is_suppressed() {
        let text = sentence_with(36);
        let tokens = tokenise(&text);
        let analysis = crate::syntax::analyze(&tokens);
        let covered = StyleDiagnostic::new(
            StyleKind::Wordy,
            DiagnosticConfidence::High,
            0..1,
            StyleKind::Wordy.name(),
        );
        assert!(check(&tokens, &analysis, &[covered]).is_empty());
    }

    #[test]
    fn the_span_is_the_whole_sentence() {
        let text = sentence_with(36);
        let tokens = tokenise(&text);
        let analysis = crate::syntax::analyze(&tokens);
        let found = check(&tokens, &analysis, &[]);
        assert_eq!(found[0].span, 0..text.len());
    }
}
