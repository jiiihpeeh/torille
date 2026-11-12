//! Sentence grouping and clause segmentation.

use crate::sentence;
use crate::tokenise::Token;

use super::{
    TokenSyntax, is_coordinating, is_infinitive, is_predicate_candidate, is_relative_word,
    is_subordinator,
};

/// Words grouped for clause segmentation.
///
/// The sentence layer starts a new sentence at an opening or closing bracket
/// so the case checker accepts a capital after `(`. A parenthetical inside a
/// clause must not tear that clause apart, so the syntax layer merges two
/// adjacent sentence groups whose only boundary is a bracket. A period,
/// question mark, exclamation mark, colon or line break still ends a group:
/// `(Tämä on uusi virke.) Toinen virke.` stays two groups.
pub(super) fn clause_groups(tokens: &[Token], contexts: &[sentence::TokenContext]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_sentence: Option<usize> = None;
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let sentence_index = contexts[index].sentence_index;
        if current_sentence != Some(sentence_index) {
            groups.push(Vec::new());
            current_sentence = Some(sentence_index);
        }
        if let Some(group) = groups.last_mut() {
            group.push(index);
        }
    }

    let mut merged: Vec<Vec<usize>> = Vec::new();
    for group in groups {
        let merge = merged
            .last()
            .and_then(|previous| previous.last().copied())
            .and_then(|left| group.first().copied().map(|right| (left, right)))
            .is_some_and(|(left, right)| bracket_only_boundary(tokens, left, right));
        if merge {
            if let Some(previous) = merged.last_mut() {
                previous.extend(group);
            }
        } else {
            merged.push(group);
        }
    }
    merged
}

/// True when the separators between two words are only brackets.
///
/// A terminal mark, colon, newline or quote disqualifies the boundary. The
/// period wins over a neighbouring bracket (`.)` and `).` are both terminal).
fn bracket_only_boundary(tokens: &[Token], left: usize, right: usize) -> bool {
    let mut bracket = false;
    for token in tokens.iter().take(right).skip(left + 1) {
        if token.is_word() {
            return false;
        }
        if token.text.contains('\n') {
            return false;
        }
        let text = token.text.trim();
        if text
            .chars()
            .any(|c| matches!(c, '.' | '!' | '?' | ':' | ';'))
        {
            return false;
        }
        if text
            .chars()
            .any(|c| matches!(c, '(' | ')' | '[' | ']' | '{' | '}'))
        {
            bracket = true;
        } else if text
            .chars()
            .any(|c| matches!(c, '»' | '«' | '"' | '“' | '”' | '‘' | '’'))
        {
            return false;
        }
    }
    bracket
}

/// Split one sentence's words into clause-sized runs.
///
/// This is deliberately conservative. A coordinating conjunction starts a
/// clause only when the next word is a finite verb (`ja joi`), so `Sami ja
/// Liisa söivät` stays one clause. Subordinators and relative pronouns always
/// start a clause. A comma before a finite verb also starts one, which keeps
/// `Kun hän tuli, söin` from merging into a single predicate.
pub(super) fn segment_clause_words(
    words: &[usize],
    tokens: &[Token],
    out: &[TokenSyntax],
    contexts: &[sentence::TokenContext],
) -> Vec<Vec<usize>> {
    let mut starts = vec![0usize];
    for position in 1..words.len() {
        // A top-level semicolon separates clauses. It is not a sentence
        // boundary for the case layer, so the split belongs here.
        if top_level_semicolon(tokens, words[position - 1], words[position], contexts) {
            starts.push(position);
            continue;
        }
        let lower = tokens[words[position]].text.to_lowercase();
        if is_relative_word(words[position], tokens, out) {
            starts.push(position);
            continue;
        }
        if is_subordinator(&lower) {
            // `sekä ... että` coordinates two nominals; the `että` is not a
            // clause opener when no predicate stands between the pair.
            if lower == "että" && is_seka_etta(words, position, tokens, out) {
                continue;
            }
            starts.push(position);
            continue;
        }
        if is_coordinating(&lower) {
            if position + 1 < words.len() && is_predicate_candidate(words[position + 1], out) {
                starts.push(position);
            }
            continue;
        }
        if is_infinitive(words[position], out)
            && words[..position]
                .iter()
                .any(|&index| is_predicate_candidate(index, out))
        {
            starts.push(position);
            continue;
        }
        if is_predicate_candidate(words[position], out)
            && has_separator_between(tokens, words[position - 1], words[position], ',')
            && words[..position]
                .iter()
                .any(|&index| is_predicate_candidate(index, out))
        {
            starts.push(position);
        }
    }
    starts.sort_unstable();
    starts.dedup();

    let mut segments = Vec::new();
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(words.len());
        if start < end {
            segments.push(words[start..end].to_vec());
        }
    }
    segments
}

/// `sekä ... että` without a predicate between them coordinates nominals, so
/// the `että` must not open a clause.
fn is_seka_etta(words: &[usize], position: usize, tokens: &[Token], out: &[TokenSyntax]) -> bool {
    for &index in words[..position].iter().rev() {
        let lower = tokens[index].text.to_lowercase();
        if lower == "sekä" {
            return true;
        }
        if is_predicate_candidate(index, out) {
            return false;
        }
    }
    false
}

fn has_separator_between(tokens: &[Token], left: usize, right: usize, needle: char) -> bool {
    tokens
        .iter()
        .skip(left + 1)
        .take(right.saturating_sub(left + 1))
        .filter(|token| !token.is_word())
        .any(|token| token.text.contains(needle))
}

/// A semicolon at bracket depth zero and outside a quote.
///
/// `Hän tuli; minä lähdin` is two clauses, but a semicolon inside `(…)` or a
/// quotation must not split the host clause. The quote check uses the depth at
/// the word before the separator, which is where the semicolon is read.
fn top_level_semicolon(
    tokens: &[Token],
    left: usize,
    right: usize,
    contexts: &[sentence::TokenContext],
) -> bool {
    if contexts.get(left).is_some_and(|context| context.quote_depth > 0) {
        return false;
    }
    // Start at the beginning of the text so an opening bracket before the
    // pair still counts: `Hän tuli (nopeasti; hiljaa) kotiin` must not split.
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().take(right) {
        if token.is_word() {
            continue;
        }
        for character in token.text.chars() {
            match character {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = (depth - 1).max(0),
                ';' if depth == 0 && index > left => return true,
                _ => {}
            }
        }
    }
    false
}
