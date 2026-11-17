//! Conservative clause-structure warnings.
//!
//! Two narrow patterns, both `Warning` and both without a correction:
//!
//! * a clause that ends in a coordinating conjunction (`Hän tuli ja.`), where
//!   the coordinated predicate is missing;
//! * a sentence-final subordinate clause with a personal-pronoun subject and a
//!   predicative word but no finite predicate (`Koska minä väsynyt.`).
//!
//! The rule deliberately ignores headings, captions and sentence fragments:
//! the clause must end at a declarative period, not a question mark, an
//! exclamation mark, a comma, a line break or the end of the text.

use crate::syntax::SyntaxAnalysis;
use crate::tags::{Mood, PartOfSpeech};
use crate::tokenise::Token;

use super::analysis::clause_words;
use super::diagnostic::{Confidence, DiagnosticKind, Severity, SyntaxDiagnostic};

const COORDINATORS: &[&str] = &["ja", "tai", "vai", "sekä", "eli"];
const PERSONAL_PRONOUNS: &[&str] = &["minä", "sinä", "hän", "me", "te", "he"];

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        let words = clause_words(clause, tokens);
        let Some(&last) = words.last() else {
            continue;
        };
        if !ends_declarative_sentence(tokens, clause.tokens.end) {
            continue;
        }

        if clause.predicate.is_some()
            && COORDINATORS.contains(&analysis.tokens[last].key.as_str())
        {
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::ClauseStructure,
                Severity::Warning,
                Confidence::Certain,
                tokens[last].start..tokens[last].end,
                "incomplete_coordination",
            );
            diagnostic.expected = Some("coordinated predicate".to_string());
            diagnostics.push(diagnostic);
            continue;
        }

        if clause.predicate.is_some() {
            continue;
        }
        let Some(&first) = words.first() else {
            continue;
        };
        if !crate::syntax::is_subordinator(&analysis.tokens[first].key) {
            continue;
        }
        // The personal pronoun is the strong subject cue; a bare noun fragment
        // (`Koska Liisa.`) is left alone because it can be a heading or an
        // answer.
        let Some(pronoun) = words
            .iter()
            .copied()
            .skip(1)
            .find(|&index| PERSONAL_PRONOUNS.contains(&analysis.tokens[index].key.as_str()))
        else {
            continue;
        };
        if pronoun == last {
            continue;
        }
        let has_predicative = words
            .iter()
            .copied()
            .skip_while(|&index| index <= pronoun)
            .any(|index| {
                analysis.tokens[index].readings.iter().any(|reading| {
                    matches!(
                        reading.part_of_speech,
                        Some(PartOfSpeech::Adjective | PartOfSpeech::Verb)
                    ) && reading.mood != Some(Mood::Infinitive)
                })
            });
        if !has_predicative {
            continue;
        }
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::ClauseStructure,
            Severity::Warning,
            Confidence::Certain,
            tokens[last].start..tokens[last].end,
            "missing_predicate",
        );
        diagnostic.expected = Some("finite predicate".to_string());
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// Whether the next real punctuation after the clause is a declarative period
/// with no new word in between.
fn ends_declarative_sentence(tokens: &[Token], clause_end: usize) -> bool {
    for token in &tokens[clause_end..] {
        if token.is_word() {
            return false;
        }
        if token.text.contains('.') {
            return true;
        }
        if token.text.contains('?') || token.text.contains('!') {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn diagnose(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        check(&tokens, &analysis)
    }

    #[test]
    fn a_missing_predicate_is_a_warning() {
        let diagnostics = diagnose("Koska minä väsynyt.");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::ClauseStructure);
        assert_eq!(diagnostics[0].message, "missing_predicate");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
        assert!(diagnostics[0].corrections.is_empty());
    }

    #[test]
    fn an_incomplete_coordination_is_a_warning() {
        let diagnostics = diagnose("Hän tuli ja.");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "incomplete_coordination");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn fragments_and_complete_clauses_stay_quiet() {
        for text in [
            "Koska minä olen väsynyt.",
            "Hän tuli ja lähti.",
            "Koska minä väsynyt?",
            "Koska minä väsynyt",
            "Hän tuli ja",
        ] {
            assert!(diagnose(text).is_empty(), "{text}: {:?}", diagnose(text));
        }
    }
}
