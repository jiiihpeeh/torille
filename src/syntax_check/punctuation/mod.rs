//! List and clause punctuation diagnostics.
//!
//! The rules consume the list arena and the clause spans produced by the
//! analysis layer. They never guess a boundary from the comma alone: a
//! subordinate clause needs a subordinator, a relative clause needs a relative
//! pronoun, and a list needs the parallel items that `syntax_list` found.

mod clause;
mod comma;
mod lists;
mod parenthetical;
mod question;
mod quote;

use crate::sentence;
use crate::syntax::SyntaxAnalysis;
use crate::tokenise::Token;

use super::diagnostic::SyntaxDiagnostic;

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let contexts = sentence::analyze(tokens);
    let mut diagnostics = lists::list_diagnostics(tokens, analysis);
    diagnostics.extend(comma::clause_diagnostics(tokens, analysis));
    diagnostics.extend(question::question_diagnostics(tokens, analysis));
    diagnostics.extend(quote::quote_diagnostics(tokens, &contexts));
    diagnostics.extend(parenthetical::parenthetical_diagnostics(tokens, analysis, &contexts));
    diagnostics
}

// ── lists ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax_check::{DiagnosticKind, Severity};
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        check(&tokens, &analysis)
    }

    fn kind(text: &str, wanted: DiagnosticKind) -> Vec<SyntaxDiagnostic> {
        diagnostics(text)
            .into_iter()
            .filter(|diagnostic| diagnostic.kind == wanted)
            .collect()
    }

    #[test]
    fn a_missing_question_mark_is_an_error() {
        let found = kind("Missä sinä asut.", DiagnosticKind::QuestionMarkMissing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Error);
        assert_eq!(found[0].corrections[0].edits[0].replacement, "?");
    }

    #[test]
    fn a_ko_question_without_a_mark_is_an_error() {
        let found = kind(
            "Onko sinulla kirjaa.",
            DiagnosticKind::QuestionMarkMissing,
        );
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn indirect_questions_and_existing_marks_stay_quiet() {
        for text in [
            "Kysyin, missä hän asuu.",
            "Hän sanoi: »Missä sinä asut?»",
            "Missä sinä asut?",
        ] {
            assert!(
                kind(text, DiagnosticKind::QuestionMarkMissing).is_empty(),
                "{text}"
            );
        }
    }

    #[test]
    fn a_quoted_question_gets_the_mark_inside_the_quote() {
        let found = kind(
            "»Missä sinä asut.»",
            DiagnosticKind::QuestionMarkMissing,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].corrections[0].edits[0].start, 19);
        assert_eq!(found[0].corrections[0].edits[0].replacement, "?");
    }

    #[test]
    fn an_indefinite_pronoun_is_not_a_question() {
        assert!(
            kind("Kuka tahansa voi tulla.", DiagnosticKind::QuestionMarkMissing).is_empty()
        );
    }

    #[test]
    fn a_semicolon_is_not_a_punctuation_error() {
        assert!(diagnostics("Hän tuli; minä lähdin.").is_empty());
    }

    #[test]
    fn an_unbalanced_quote_is_a_warning() {
        let found = kind("Hän sanoi: »Elä tule.", DiagnosticKind::UnbalancedQuote);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn balanced_quotes_are_quiet() {
        assert!(kind("Hän sanoi: »Elä tule.»", DiagnosticKind::UnbalancedQuote).is_empty());
    }

    #[test]
    fn an_unmarked_parenthetical_adverb_gets_a_pair() {
        let found = kind("Omena esimerkiksi on hyvä.", DiagnosticKind::CommaMissing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Suggestion);
        assert_eq!(found[0].corrections[0].edits.len(), 2);
        assert_eq!(found[0].corrections[0].edits[0].replacement, ",");
        assert_eq!(found[0].corrections[0].edits[1].replacement, ",");
    }

    #[test]
    fn a_marked_parenthetical_adverb_is_quiet() {
        assert!(diagnostics("Omena, esimerkiksi, on hyvä.").is_empty());
        assert!(diagnostics("Esimerkiksi omena on hyvä.").is_empty());
    }

    #[test]
    fn a_kuten_phrase_gets_a_pair_around_the_phrase() {
        let found = kind("Omena kuten päärynä on hedelmä.", DiagnosticKind::CommaMissing);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Suggestion);
        assert_eq!(found[0].corrections[0].edits.len(), 2);
        assert_eq!(found[0].corrections[0].edits[0].start, 5);
        assert_eq!(found[0].corrections[0].edits[1].start, 22);
    }

    #[test]
    fn a_marked_kuten_phrase_and_a_nonparenthetical_kuten_are_quiet() {
        for text in [
            "Omena, kuten päärynä, on hedelmä.",
            "Omena kuten päärynä, on hedelmä.",
            "Kuten tiedät, omena on hyvä.",
            "Hän puhui kuten asiantuntija.",
        ] {
            assert!(diagnostics(text).is_empty(), "{text}: {:#?}", diagnostics(text));
        }
    }
}
