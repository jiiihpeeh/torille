//! Shared token and clause helpers for the syntax-check rules.
//!
//! These were copied into several rule modules as the checker grew. Keeping one
//! implementation makes the guards easier to audit: a rule either uses the
//! shared predicate or documents why it needs a different one.

use crate::syntax::{Clause, TokenSyntax};
use crate::tags::{Case, Mood, Number, PartOfSpeech, Polarity};
use crate::tokenise::Token;

/// The first nominal part of speech, excluding numerals.
///
/// Object-case and argument rules use this; the numeral is handled separately
/// because a cardinal governs the partitive (`kaksi miestä`).
pub(super) fn nominal_pos(token: &TokenSyntax) -> Option<PartOfSpeech> {
    token.readings.iter().find_map(|reading| {
        matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Pronoun
                    | PartOfSpeech::Ordinal
            )
        )
        .then_some(reading.part_of_speech?)
    })
}

/// The first nominal part of speech, including numerals.
///
/// Rection keeps a numeral as a possible argument: `vastata kahteen` is a
/// case-governed answer, not a free adverbial.
pub(super) fn nominal_or_numeral_pos(token: &TokenSyntax) -> Option<PartOfSpeech> {
    token.readings.iter().find_map(|reading| {
        matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Pronoun
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
            )
        )
        .then_some(reading.part_of_speech?)
    })
}

/// The token's case, or the only nominal case its readings agree on.
pub(super) fn nominal_case(token: &TokenSyntax) -> Option<Case> {
    if let Some(case) = token.case {
        return Some(case);
    }
    let mut cases: Vec<Case> = token
        .readings
        .iter()
        .filter(|reading| {
            matches!(
                reading.part_of_speech,
                Some(
                    PartOfSpeech::Noun
                        | PartOfSpeech::Adjective
                        | PartOfSpeech::Pronoun
                        | PartOfSpeech::Ordinal
                )
            )
        })
        .filter_map(|reading| reading.case)
        .collect();
    cases.sort_unstable();
    cases.dedup();
    match cases.as_slice() {
        [case] => Some(*case),
        _ => None,
    }
}

/// The token's number when every reading in `case` agrees.
pub(super) fn candidate_number(token: &TokenSyntax, case: Case) -> Option<Number> {
    let mut numbers: Vec<Number> = token
        .readings
        .iter()
        .filter(|reading| reading.case == Some(case))
        .filter_map(|reading| reading.number)
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    match numbers.as_slice() {
        [number] => Some(*number),
        _ => None,
    }
}

/// A coordinator, subordinator or relative pronoun that must not be read as an
/// argument merely because the lexicon also gives it a nominal reading.
pub(super) fn is_marker(word: &str) -> bool {
    let lower = word.to_lowercase();
    crate::syntax::is_coordinating(&lower)
        || crate::syntax::is_subordinator(&lower)
        || matches!(lower.as_str(), "joka" | "mikä" | "ken" | "kuka")
}

/// A finite verb reading in the punctuation sense.
///
/// Agreement uses `agreement::finite_reading` instead: an imperative is a
/// finite predicate for clause punctuation but never the agreement bearer.
pub(super) fn is_finite(token: &TokenSyntax) -> bool {
    token.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Verb | PartOfSpeech::NegativeVerb)
        ) && reading.polarity == Polarity::Affirmative
            && matches!(
                reading.mood,
                Some(
                    Mood::Indicative
                        | Mood::Conditional
                        | Mood::Imperative
                        | Mood::Potential
                )
            )
    })
}

/// The word token indices inside a clause, in source order.
pub(super) fn clause_words(clause: &Clause, tokens: &[Token]) -> Vec<usize> {
    clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect()
}

pub(super) fn first_word(clause: &Clause, tokens: &[Token]) -> Option<usize> {
    clause
        .tokens
        .clone()
        .find(|&index| tokens[index].is_word())
}

pub(super) fn last_word(clause: &Clause, tokens: &[Token]) -> Option<usize> {
    clause
        .tokens
        .clone()
        .rev()
        .find(|&index| tokens[index].is_word())
}

/// Is there a comma separator between two token indices?
pub(super) fn comma_between(tokens: &[Token], left: usize, right: usize) -> bool {
    if left == right {
        return false;
    }
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

/// Is the token at `index` inside an unclosed `()`/`[]`/`{}` pair?
///
/// `start` is the clause's first token; scanning begins there so an opening
/// bracket before the token still counts.
pub(super) fn inside_brackets(tokens: &[Token], start: usize, index: usize) -> bool {
    let mut depth = 0i32;
    for token in tokens.iter().take(index).skip(start) {
        if token.is_word() {
            continue;
        }
        for character in token.text.chars() {
            match character {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = (depth - 1).max(0),
                _ => {}
            }
        }
    }
    depth > 0
}
