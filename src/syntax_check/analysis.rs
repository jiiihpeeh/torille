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
    // Prefer a pronoun reading over a noun homonym, the same way
    // `nominal_has_case` does: `häntä` is the pronoun's partitive before it
    // is the noun “tail”.
    let prefer_pronoun = token
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Pronoun));
    let mut found: Option<Case> = None;
    for reading in &token.readings {
        if !matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Pronoun
                    | PartOfSpeech::Ordinal
            )
        ) {
            continue;
        }
        if prefer_pronoun && reading.part_of_speech != Some(PartOfSpeech::Pronoun) {
            continue;
        }
        let Some(case) = reading.case else {
            continue;
        };
        match found {
            None => found = Some(case),
            Some(seen) if seen == case => {}
            Some(_) => return None,
        }
    }
    found
}

/// Every nominal lemma behind a surface, the chosen lemma first.
///
/// A case correction must try the reading the syntax layer chose before a
/// homonym; `kirjojen` is a form of both `kirja` and `kirjo`.
pub(super) fn nominal_bases(token: &TokenSyntax) -> Vec<&str> {
    let mut ordered: Vec<&str> = Vec::new();
    if let Some(lemma) = token.lemma.as_deref() {
        ordered.push(lemma);
    }
    for lemma in &token.lemmas {
        if !ordered.contains(&lemma.as_str()) {
            ordered.push(lemma.as_str());
        }
    }
    ordered
}

/// The token's number when every reading in `case` agrees.
pub(super) fn candidate_number(token: &TokenSyntax, case: Case) -> Option<Number> {
    let mut found: Option<Number> = None;
    for reading in &token.readings {
        if reading.case != Some(case) {
            continue;
        }
        let Some(number) = reading.number else {
            continue;
        };
        match found {
            None => found = Some(number),
            Some(seen) if seen == number => {}
            Some(_) => return None,
        }
    }
    found
}

/// A coordinator, subordinator or relative pronoun that must not be read as an
/// argument merely because the lexicon also gives it a nominal reading.
///
/// `key` is the token's lowercased surface (`TokenSyntax::key`).
pub(super) fn is_marker(key: &str) -> bool {
    crate::syntax::is_coordinating(key)
        || crate::syntax::is_subordinator(key)
        || matches!(key, "joka" | "mikä" | "ken" | "kuka" | "kuin")
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
            // A finite verb does not take a possessive suffix. The reading
            // that has one is the homonym: `yllään` is the postposition
            // `yllä` plus the 3rd-person possessive, not a finite `yltää`,
            // and treating it as a predicate invents a clause boundary
            // (`jolla oli yllään …`).
            && reading.possessive.is_none()
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

/// Is there already a clause boundary between two token indices?
///
/// A comma, semicolon, colon or dash all mark a boundary. The comma rules use
/// this to avoid inserting a second mark: without the semicolon the subordinate
/// rule would write a comma before `;` (`Kun tulin kotiin; kaikki
/// nukkuivat.`). This is a boundary test, not a claim that the mark is the
/// right one; the semicolon rules decide that separately.
pub(super) fn boundary_between(tokens: &[Token], left: usize, right: usize) -> bool {
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
        .any(|token| {
            token
                .text
                .chars()
                .any(|character| matches!(character, ',' | ';' | ':' | '–' | '—'))
                || token.text.contains("--")
        })
}

/// Is the token a finite affirmative imperative verb?
///
/// `Hän tulee` also has an imperative homonym reading on `tulee` in some
/// paradigms, so the mood is not enough on its own: the caller pairs it with a
/// structural signal (a direct address, an object, or an exclamation mark).
pub(super) fn is_imperative(token: &TokenSyntax) -> bool {
    token.readings.iter().any(|reading| {
        reading.part_of_speech == Some(PartOfSpeech::Verb)
            && reading.mood == Some(Mood::Imperative)
            && reading.polarity == Polarity::Affirmative
    })
}

/// A capitalised or possessive-marked nominal is how an address presents
/// itself when the lexicon has no proper-name paradigm for it (`Maija`,
/// `Pekka`). A finite predicate is never an address, and a personal pronoun is
/// a subject or object rather than a form of address (`En pidä sinä` is a
/// rection error, not a vocative).
pub(super) fn looks_like_address(
    index: usize,
    tokens: &[Token],
    analysis: &crate::syntax::SyntaxAnalysis,
) -> bool {
    const PERSONAL_PRONOUNS: &[&str] = &["minä", "sinä", "hän", "me", "te", "he"];

    let syntax = &analysis.tokens[index];
    if is_finite(syntax) || PERSONAL_PRONOUNS.contains(&crate::text::lowercase(&tokens[index].text).as_ref())
    {
        return false;
    }
    if tokens[index].is_capitalised()
        || syntax
            .readings
            .iter()
            .any(|reading| reading.possessive.is_some())
    {
        return true;
    }
    syntax
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Pronoun))
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
