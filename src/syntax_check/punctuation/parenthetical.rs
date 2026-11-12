//! `esimerkiksi` and `kuten` parenthetical comma pairs.

use crate::sentence;
use crate::syntax::{SyntacticRole, SyntaxAnalysis};
use crate::tokenise::Token;

use super::super::analysis::{clause_words, comma_between, inside_brackets};
use super::super::diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

/// Cues whose parenthetical can be delimited locally.
///
/// `esimerkiksi` is parenthetical on its own. `kuten` is parenthetical over
/// the following phrase (`Omena, kuten päärynä, on hedelmä`), so its extent
/// ends at the next predicate.
const EXAMPLE_CUES: &[&str] = &["esimerkiksi"];

pub(super) fn parenthetical_diagnostics(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    contexts: &[sentence::TokenContext],
) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for clause in &analysis.clauses {
        let words = clause_words(clause, tokens);
        for (position, &index) in words.iter().enumerate() {
            let lower = tokens[index].text.to_lowercase();
            let example = EXAMPLE_CUES.contains(&lower.as_str());
            let kuten = lower == "kuten";
            if !example && !kuten {
                continue;
            }
            if position == 0 {
                continue;
            }
            if inside_brackets(tokens, clause.tokens.start, index)
                || contexts
                    .get(index)
                    .is_some_and(|context| context.quote_depth > 0)
            {
                continue;
            }
            let left = words[position - 1];
            if comma_between(tokens, left, index) {
                continue;
            }
            let (last, next) = if example {
                let Some(&next) = words.get(position + 1) else {
                    continue;
                };
                (index, next)
            } else {
                let Some((last, predicate)) = kuten_phrase(&words, position, analysis) else {
                    continue;
                };
                (last, predicate)
            };
            if comma_between(tokens, index, last) || comma_between(tokens, last, next) {
                continue;
            }
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::CommaMissing,
                Severity::Suggestion,
                DiagnosticConfidence::Medium,
                tokens[index].start..tokens[index].end,
                "parenthetical_comma",
            );
            diagnostic.corrections = vec![Correction::new(
                vec![
                    Edit::insertion(tokens[left].end, ","),
                    Edit::insertion(tokens[last].end, ","),
                ],
                0.0,
                "parenthetical_comma",
            )];
            out.push(diagnostic);
        }
    }
    out
}

/// The last word of a `kuten` phrase and the predicate that closes it.
///
/// `Omena kuten päärynä on hedelmä`: the phrase is `kuten päärynä`, and `on`
/// is the predicate. With no predicate after the cue (`Hän puhui kuten
/// asiantuntija`) there is no phrase boundary, so the rule stays quiet.
fn kuten_phrase(
    words: &[usize],
    position: usize,
    analysis: &SyntaxAnalysis,
) -> Option<(usize, usize)> {
    let end_position = words
        .iter()
        .enumerate()
        .skip(position + 1)
        .find(|&(_, &index)| analysis.tokens[index].role == SyntacticRole::Predicate)
        .map(|(position, _)| position)?;
    if end_position <= position + 1 {
        return None;
    }
    Some((words[end_position - 1], words[end_position]))
}
