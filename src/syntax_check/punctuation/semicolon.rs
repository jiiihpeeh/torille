//! Semicolon misuse diagnostics.
//!
//! A semicolon is valid between two independent clauses. These rules only
//! report the structural cases the specification forbids: a main/subordinate
//! boundary, coordinated or detached sentence elements, and a sentence-final
//! semicolon. Every finding is a `Suggestion`; the semicolon itself is never an
//! `Error`.

use crate::sentence;
use crate::syntax::{Clause, SyntaxAnalysis};
use crate::tags::Case;
use crate::tokenise::Token;

use super::super::analysis::{clause_words, first_word, is_finite, last_word, nominal_case};
use super::super::diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticKind, Edit, Severity, SyntaxDiagnostic,
};
use super::clause::{ClauseKind, clause_kind};

const PERSONAL_PRONOUNS: &[&str] = &["minä", "sinä", "hän", "me", "te", "he"];

pub(super) fn semicolon_diagnostics(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    contexts: &[sentence::TokenContext],
) -> Vec<SyntaxDiagnostic> {
    let mut out = clause_semicolons(tokens, analysis);
    out.extend(terminal_semicolons(tokens, analysis, contexts));
    out
}

/// The semicolons that sit between two adjacent clauses in one sentence.
fn clause_semicolons(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    for pair in analysis.clauses.windows(2) {
        let [previous, next] = pair else {
            continue;
        };
        if !same_sentence(previous, next, analysis) {
            continue;
        }
        let Some((start, end)) = semicolon_between(tokens, previous, next) else {
            continue;
        };

        let previous_kind = clause_kind(previous, tokens, analysis);
        let next_kind = clause_kind(next, tokens, analysis);
        if matches!(
            previous_kind,
            ClauseKind::Subordinate | ClauseKind::Relative
        ) || matches!(next_kind, ClauseKind::Subordinate | ClauseKind::Relative)
        {
            out.push(semicolon_replacement(
                start,
                end,
                ",",
                "semicolon_subordinate",
            ));
            continue;
        }

        if subject_predicate_split(previous, next, tokens, analysis) {
            out.push(semicolon_deletion(tokens, start, end, next));
            continue;
        }

        if coordinated_elements(previous, next, tokens, analysis) {
            out.push(semicolon_replacement(
                start,
                end,
                ",",
                "semicolon_coordination",
            ));
            continue;
        }

        if detached_phrase(previous, next, tokens, analysis) {
            out.push(semicolon_deletion(tokens, start, end, next));
        }
    }
    out
}

/// `Opiskelijat; valmistautuivat kokeeseen.` — the semicolon splits a bare
/// subject from its predicate.
fn subject_predicate_split(
    previous: &Clause,
    next: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    if previous.predicate.is_some() || next.predicate.is_none() || next.subject.is_some() {
        return false;
    }
    let words = clause_words(previous, tokens);
    let [word] = words.as_slice() else {
        return false;
    };
    is_nominal_word(*word, analysis) && !is_finite(&analysis.tokens[*word])
}

/// `Hän on ystävällinen; kohtelias ja avulias.` — the second segment continues
/// the first clause's complement.
///
/// Object/list coordination (`Ostin omenoita; appelsiineja ja banaaneja.`) is
/// left to the semicolon-aware list pass; flagging it here would turn every
/// semicolon list into a single comma suggestion.
fn coordinated_elements(
    previous: &Clause,
    next: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    let Some(role) = previous.complement else {
        return false;
    };
    let Some(first) = bare_phrase(next, tokens, analysis) else {
        return false;
    };
    let Some(previous_case) = nominal_case(&analysis.tokens[role]) else {
        return false;
    };
    let Some(next_case) = nominal_case(&analysis.tokens[first]) else {
        return false;
    };
    previous_case == next_case
}

/// `Ostin; leipää.` / `Hän on; väsynyt.` — the second segment is the object or
/// complement the first predicate is still missing.
fn detached_phrase(
    previous: &Clause,
    next: &Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    if previous.predicate.is_none() || previous.object.is_some() || previous.complement.is_some() {
        return false;
    }
    let Some(first) = bare_phrase(next, tokens, analysis) else {
        return false;
    };
    let Some(case) = nominal_case(&analysis.tokens[first]) else {
        return false;
    };
    matches!(
        case,
        Case::Nominative | Case::Accusative | Case::Genitive | Case::Partitive
    )
}

/// A segment with no predicate and no overt subject whose words are a nominal
/// phrase (possibly coordinated). Returns the first word.
fn bare_phrase(clause: &Clause, tokens: &[Token], analysis: &SyntaxAnalysis) -> Option<usize> {
    if clause.predicate.is_some() || clause.subject.is_some() {
        return None;
    }
    let words = clause_words(clause, tokens);
    let &first = words.first()?;
    if !is_nominal_word(first, analysis) || is_finite(&analysis.tokens[first]) {
        return None;
    }
    if tokens[first].is_capitalised() || is_personal_pronoun(first, tokens) {
        return None;
    }
    if words
        .iter()
        .any(|&word| !is_nominal_word(word, analysis) && !is_coordinator(word, tokens))
    {
        return None;
    }
    Some(first)
}

fn is_nominal_word(index: usize, analysis: &SyntaxAnalysis) -> bool {
    analysis.tokens[index]
        .readings
        .iter()
        .any(|reading| crate::syntax::is_nominal_pos(reading.part_of_speech))
}

fn is_coordinator(index: usize, tokens: &[Token]) -> bool {
    crate::syntax::is_coordinating(&crate::text::lowercase(&tokens[index].text))
}

fn is_personal_pronoun(index: usize, tokens: &[Token]) -> bool {
    PERSONAL_PRONOUNS.contains(&crate::text::lowercase(&tokens[index].text).as_ref())
}

/// `Hän lähti kotiin;` — a semicolon with no word left in its sentence.
fn terminal_semicolons(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    contexts: &[sentence::TokenContext],
) -> Vec<SyntaxDiagnostic> {
    let mut out = Vec::new();
    let mut bracket_depth = 0i32;
    for (index, token) in tokens.iter().enumerate() {
        if token.is_word() {
            continue;
        }
        for (offset, character) in token.text.char_indices() {
            match character {
                '(' | '[' | '{' => bracket_depth += 1,
                ')' | ']' | '}' => bracket_depth = (bracket_depth - 1).max(0),
                ';' if bracket_depth == 0 => {
                    if contexts
                        .get(index)
                        .is_some_and(|context| context.quote_depth > 0)
                    {
                        continue;
                    }
                    let absolute = token.start + offset;
                    let rest = &token.text[offset + character.len_utf8()..];
                    if rest
                        .chars()
                        .any(|character| matches!(character, '.' | '!' | '?'))
                    {
                        continue;
                    }
                    let Some((sentence_start, sentence_end)) = sentence_at(absolute, analysis)
                    else {
                        continue;
                    };
                    if tokens.iter().any(|token| {
                        token.is_word() && token.start > absolute && token.start < sentence_end
                    }) {
                        continue;
                    }
                    // A terminal mark already after the semicolon (`Hän tuli;.`)
                    // is typography's job; replacing `;` with `.` would double it.
                    if tokens.iter().any(|token| {
                        !token.is_word()
                            && token.start > absolute
                            && token.start < sentence_end
                            && token
                                .text
                                .chars()
                                .any(|character| matches!(character, '.' | '!' | '?'))
                    }) {
                        continue;
                    }
                    let Some(clause) =
                        analysis.clauses.iter().rev().find(|clause| {
                            clause.end <= absolute && clause.start >= sentence_start
                        })
                    else {
                        continue;
                    };
                    if clause.predicate.is_none() {
                        continue;
                    }
                    out.push(semicolon_replacement(
                        absolute,
                        absolute + 1,
                        ".",
                        "semicolon_terminal",
                    ));
                }
                _ => {}
            }
        }
    }
    out
}

fn semicolon_between(tokens: &[Token], previous: &Clause, next: &Clause) -> Option<(usize, usize)> {
    let left = last_word(previous, tokens)?;
    let right = first_word(next, tokens)?;
    if left >= right {
        return None;
    }
    tokens[left + 1..right].iter().find_map(|token| {
        if token.is_word() {
            return None;
        }
        token
            .text
            .char_indices()
            .find(|(_, character)| *character == ';')
            .map(|(offset, _)| (token.start + offset, token.start + offset + 1))
    })
}

fn semicolon_replacement(
    start: usize,
    end: usize,
    replacement: &str,
    message: &'static str,
) -> SyntaxDiagnostic {
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::SemicolonMisuse,
        Severity::Suggestion,
        Confidence::Likely,
        start..end,
        message,
    );
    diagnostic.corrections = vec![
        Correction::new(vec![Edit::new(start, end, replacement)], 0.0, message)
            .licensed_by(CorrectionLicense::Punctuation)
            .with_confidence(Confidence::Likely),
    ];
    diagnostic
}

/// Remove the semicolon and the following whitespace without joining lines.
fn semicolon_deletion(
    tokens: &[Token],
    start: usize,
    end: usize,
    next: &Clause,
) -> SyntaxDiagnostic {
    let next_start = first_word(next, tokens)
        .map(|word| tokens[word].start)
        .unwrap_or(end);
    let newline_between = tokens
        .iter()
        .filter(|token| token.end > start && token.start < next_start)
        .any(|token| token.text.contains('\n'));
    let delete_end = if newline_between { end } else { next_start };
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::SemicolonMisuse,
        Severity::Suggestion,
        Confidence::Likely,
        start..end,
        "semicolon_phrase",
    );
    diagnostic.corrections = vec![
        Correction::new(
            vec![Edit::new(start, delete_end, "")],
            0.0,
            "semicolon_phrase",
        )
        .licensed_by(CorrectionLicense::Punctuation)
        .with_confidence(Confidence::Likely),
    ];
    diagnostic
}

fn same_sentence(left: &Clause, right: &Clause, analysis: &SyntaxAnalysis) -> bool {
    sentence_at(left.start, analysis).map(|(start, _)| start)
        == sentence_at(right.start, analysis).map(|(start, _)| start)
}

fn sentence_at(byte: usize, analysis: &SyntaxAnalysis) -> Option<(usize, usize)> {
    analysis
        .sentences
        .iter()
        .find(|sentence| byte >= sentence.start && byte < sentence.end)
        .map(|sentence| (sentence.start, sentence.end))
}
