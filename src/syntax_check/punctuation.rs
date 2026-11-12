//! List and clause punctuation diagnostics.
//!
//! The rules consume the list arena and the clause spans produced by the
//! analysis layer. They never guess a boundary from the comma alone: a
//! subordinate clause needs a subordinator, a relative clause needs a relative
//! pronoun, and a list needs the parallel items that `syntax_list` found.

use crate::syntax::{Clause, SyntacticRole, SyntaxAnalysis, TokenSyntax};
use crate::syntax_list::{ListItem, ListKind};
use crate::tags::{Mood, PartOfSpeech, Polarity};
use crate::tokenise::Token;

use super::diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = list_diagnostics(tokens, analysis);
    diagnostics.extend(clause_diagnostics(tokens, analysis));
    diagnostics
}

// ── lists ────────────────────────────────────────────────────────────────

fn list_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for list in analysis.lists.iter().filter(|list| list.kind == ListKind::Nominal) {
        let Some(first_item) = list.items.first() else {
            continue;
        };

        if let Some(colon) = list.introducer {
            let commas = comma_positions(tokens, colon, first_item.tokens.start);
            if let Some(&(start, _)) = commas.first() {
                out.push(remove_comma(
                    DiagnosticKind::ListIntroducerComma,
                    Severity::Error,
                    start,
                    "list_introducer_comma",
                ));
            }
        }

        for pair in list.items.windows(2) {
            let [left, right] = pair else {
                continue;
            };
            let commas = comma_positions(tokens, left.tokens.end, right.tokens.start);
            if commas.len() >= 2 {
                let (start, _) = commas[1];
                out.push(remove_comma(
                    DiagnosticKind::ListSeparatorDuplicate,
                    Severity::Error,
                    start,
                    "list_separator_duplicate",
                ));
            }
        }

        for item in &list.items {
            out.extend(missing_separators(item, tokens, &analysis.tokens));
        }

        if let Some(conjunction) = list.conjunction {
            let previous = list
                .items
                .iter()
                .rev()
                .find(|item| item.tokens.end <= conjunction);
            if let Some(previous) = previous {
                let commas = comma_positions(tokens, previous.tokens.end, conjunction);
                if commas.len() == 1 {
                    let (start, _) = commas[0];
                    out.push(remove_comma(
                        DiagnosticKind::ListCommaBeforeConjunction,
                        Severity::Suggestion,
                        start,
                        "list_comma_before_conjunction",
                    ));
                }
            }
        }
    }
    out
}

fn missing_separators(
    item: &ListItem,
    tokens: &[Token],
    out: &[TokenSyntax],
) -> Vec<SyntaxDiagnostic> {
    let words: Vec<usize> = item
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    let mut diagnostics = Vec::new();
    for pair in words.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        if is_common_noun(*left, tokens, out) && is_common_noun(*right, tokens, out) {
            let boundary = tokens[*left].end;
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::ListSeparatorMissing,
                Severity::Error,
                DiagnosticConfidence::High,
                boundary..boundary,
                "list_separator_missing",
            );
            diagnostic.corrections = vec![Correction::new(
                vec![Edit::insertion(boundary, ",")],
                0.0,
                "list_separator_missing",
            )];
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn is_common_noun(index: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    if tokens[index].is_capitalised() {
        return false;
    }
    let readings = &out[index].readings;
    let noun = readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Noun));
    let competing = readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Adjective
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
                    | PartOfSpeech::Verb
                    | PartOfSpeech::NegativeVerb
            )
        )
    });
    noun && !competing
}

fn remove_comma(
    kind: DiagnosticKind,
    severity: Severity,
    start: usize,
    message: &'static str,
) -> SyntaxDiagnostic {
    let mut diagnostic = SyntaxDiagnostic::new(
        kind,
        severity,
        DiagnosticConfidence::High,
        start..start + 1,
        message,
    );
    diagnostic.corrections = vec![Correction::new(
        vec![Edit::new(start, start + 1, "")],
        0.0,
        message,
    )];
    diagnostic
}

// ── clauses ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClauseKind {
    Main,
    Subordinate,
    Relative,
    Coordinate,
    Infinitive,
}

fn clause_diagnostics(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
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

fn clause_kind(clause: &Clause, tokens: &[Token], analysis: &SyntaxAnalysis) -> ClauseKind {
    let Some(first) = first_word(clause, tokens) else {
        return ClauseKind::Main;
    };
    let lower = tokens[first].text.to_lowercase();
    if crate::syntax::is_subordinator(&lower) {
        return ClauseKind::Subordinate;
    }
    if crate::syntax::is_relative_word(first, tokens, &analysis.tokens) {
        return ClauseKind::Relative;
    }
    if crate::syntax::is_coordinating(&lower) {
        return ClauseKind::Coordinate;
    }
    if is_infinitive(&analysis.tokens[first]) {
        return ClauseKind::Infinitive;
    }
    ClauseKind::Main
}

fn first_word(clause: &Clause, tokens: &[Token]) -> Option<usize> {
    clause
        .tokens
        .clone()
        .find(|&index| tokens[index].is_word())
}

fn last_word(clause: &Clause, tokens: &[Token]) -> Option<usize> {
    clause
        .tokens
        .clone()
        .rev()
        .find(|&index| tokens[index].is_word())
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

fn is_finite(syntax: &TokenSyntax) -> bool {
    syntax.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
        ) && reading.polarity == Polarity::Affirmative
            && matches!(
                reading.mood,
                Some(Mood::Indicative | Mood::Conditional | Mood::Imperative | Mood::Potential)
            )
    })
}

fn is_infinitive(syntax: &TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| reading.mood == Some(Mood::Infinitive))
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

fn comma_between(tokens: &[Token], left: usize, right: usize) -> bool {
    let (start, end) = if left < right {
        (left + 1, right)
    } else {
        (right + 1, left)
    };
    tokens[start..end]
        .iter()
        .filter(|token| !token.is_word())
        .any(|token| token.text.contains(','))
}

fn comma_positions(tokens: &[Token], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for token in tokens.iter().take(end).skip(start).filter(|token| !token.is_word()) {
        for (offset, character) in token.text.char_indices() {
            if character == ',' {
                let start = token.start + offset;
                out.push((start, start + 1));
            }
        }
    }
    out
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
