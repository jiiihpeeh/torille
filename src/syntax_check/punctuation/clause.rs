//! Clause kinds shared by the punctuation rules.

use crate::syntax::{Clause, SyntaxAnalysis, TokenSyntax};
use crate::tags::Mood;
use crate::tokenise::Token;

use super::super::analysis::first_word;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClauseKind {
    Main,
    Subordinate,
    Relative,
    Coordinate,
    Infinitive,
}

pub(super) fn clause_kind(clause: &Clause, tokens: &[Token], analysis: &SyntaxAnalysis) -> ClauseKind {
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

fn is_infinitive(syntax: &TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| reading.mood == Some(Mood::Infinitive))
}
