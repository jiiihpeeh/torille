//! Clause-boundary comma diagnostics.

use crate::syntax::{Clause, SyntacticRole, SyntaxAnalysis};
use crate::tokenise::Token;

use super::super::analysis::{comma_between, first_word, is_finite, last_word};
use super::super::diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};
use super::clause::{ClauseKind, clause_kind};

pub(super) fn clause_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for (index, clause) in analysis.clauses.iter().enumerate() {
        let kind = clause_kind(clause, tokens, analysis);
        if kind == ClauseKind::Subordinate {
            out.extend(subordinate_comma(index, clause, kind, tokens, analysis));
        }
        if kind == ClauseKind::Relative {
            out.extend(relative_comma(clause, tokens, analysis));
        }
    }
    out
}

/// A subordinate clause after a main clause, or a main clause after an initial
/// subordinate clause, needs a comma at the boundary. A merged initial
/// subordinate clause (`Koska olin väsynyt menin kotiin`) is split at its last
/// finite predicate because the clause parser could not see the boundary.
fn subordinate_comma(
    index: usize,
    clause: &Clause,
    _kind: ClauseKind,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    if index > 0 && same_sentence(&analysis.clauses[index - 1], clause, analysis) {
        let previous = &analysis.clauses[index - 1];
        if let (Some(left), Some(right)) = (last_word(previous, tokens), first_word(clause, tokens)) {
            let previous_kind = clause_kind(previous, tokens, analysis);
            let boundary_needs_comma = previous_kind != ClauseKind::Subordinate
                || clause_kind(clause, tokens, analysis) == ClauseKind::Subordinate;
            if boundary_needs_comma && !comma_between(tokens, left, right) {
                out.push(comma_missing(tokens[left].end, DiagnosticKind::CommaMissing));
            }
        }
    }
    if let Some(next) = analysis.clauses.get(index + 1)
        && same_sentence(clause, next, analysis)
        && let (Some(left), Some(right)) = (last_word(clause, tokens), first_word(next, tokens))
        && clause_kind(next, tokens, analysis) == ClauseKind::Main
        && !comma_between(tokens, left, right)
    {
        out.push(comma_missing(tokens[left].end, DiagnosticKind::CommaMissing));
    }

    // The subordinate clause may still be merged with its main clause in one
    // `Clause`: `Koska olin väsynyt menin kotiin`. Use the last finite
    // predicate as the main clause's head and insert the comma before it.
    let finite = finite_words(clause, tokens, analysis);
    if finite.len() >= 2
        && let Some(&main) = finite.last()
        && let Some(before) = previous_word(tokens, main)
        && !comma_between(tokens, before, main)
    {
        out.push(comma_missing(tokens[before].end, DiagnosticKind::CommaMissing));
    }
    out
}

/// A relative clause that already has an opening comma and is followed by its
/// host predicate needs the closing comma. A restrictive relative without an
/// opening comma is left quiet.
fn relative_comma(
    clause: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    let Some(first) = first_word(clause, tokens) else {
        return Vec::new();
    };
    let Some(antecedent) = previous_word(tokens, first) else {
        return Vec::new();
    };
    if !comma_between(tokens, antecedent, first) {
        return Vec::new();
    }
    let finite = finite_predicate_words(clause, tokens, analysis);
    if finite.len() < 2 {
        return Vec::new();
    }
    let first_finite = finite[0];
    let host = *finite.last().unwrap_or(&first_finite);
    if host == first_finite || coordinated_between(tokens, first_finite, host) {
        return Vec::new();
    }
    let Some(before) = previous_word(tokens, host) else {
        return Vec::new();
    };
    if comma_between(tokens, before, host) {
        return Vec::new();
    }
    vec![comma_missing(
        tokens[before].end,
        DiagnosticKind::CommaPairUnbalanced,
    )]
}

fn comma_missing(at: usize, kind: DiagnosticKind) -> SyntaxDiagnostic {
    let message = if kind == DiagnosticKind::CommaPairUnbalanced {
        "comma_pair_unbalanced"
    } else {
        "comma_missing"
    };
    let mut diagnostic = SyntaxDiagnostic::new(
        kind,
        Severity::Error,
        DiagnosticConfidence::High,
        at..at,
        message,
    );
    diagnostic.corrections = vec![Correction::new(
        vec![Edit::insertion(at, ",")],
        0.0,
        "comma_missing",
    )];
    diagnostic
}

fn previous_word(tokens: &[Token], index: usize) -> Option<usize> {
    (0..index).rev().find(|&candidate| tokens[candidate].is_word())
}

fn finite_predicate_words(
    clause: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| {
            tokens[index].is_word()
                && matches!(
                    analysis.tokens[index].role,
                    SyntacticRole::Predicate | SyntacticRole::Auxiliary
                )
                && is_finite(&analysis.tokens[index])
        })
        .collect()
}

fn finite_words(clause: &Clause, tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word() && is_finite(&analysis.tokens[index]))
        .collect()
}

fn coordinated_between(tokens: &[Token], left: usize, right: usize) -> bool {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| token.is_word())
        .any(|token| {
            matches!(
                token.text.to_lowercase().as_str(),
                "ja" | "sekä" | "tai" | "vai"
            )
        })
}

fn same_sentence(left: &Clause, right: &Clause, analysis: &SyntaxAnalysis) -> bool {
    sentence_index(left.start, analysis) == sentence_index(right.start, analysis)
}

fn sentence_index(byte: usize, analysis: &SyntaxAnalysis) -> Option<usize> {
    analysis
        .sentences
        .iter()
        .find(|sentence| byte >= sentence.start && byte < sentence.end)
        .map(|sentence| sentence.index)
}
