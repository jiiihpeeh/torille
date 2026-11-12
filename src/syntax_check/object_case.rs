//! Object case: negation and aspect.
//!
//! Finnish requires the partitive for the object of a negated clause: `en lue
//! kirjaa`, not `en lue kirja`. That rule is an `Error`.
//!
//! The total/partitive choice outside negation depends on aspect. The checker
//! cannot know the speaker's aspect, so it only reacts to a closed cue list:
//! a progressive cue plus a total object suggests the partitive, and a
//! completive cue plus a partitive object suggests the total object. Both are
//! `Suggestion`/`Medium` and never automatic corrections. A clause with no cue
//! stays quiet.

use crate::syntax::{SyntacticRole, SyntaxAnalysis};
use crate::tags::{Case, Number, PartOfSpeech};
use crate::tokenise::Token;

use super::correction;
use super::diagnostic::{DiagnosticConfidence, DiagnosticKind, Severity, SyntaxDiagnostic};

/// Adverbs that mark an ongoing action.
const PROGRESSIVE_CUES: &[&str] = &[
    "parhaillaan",
    "juuri nyt",
    "tällä hetkellä",
    "kesken",
    "paraikaa",
];

/// Adverbs that mark a completed action.
///
/// `viimeistään` is deliberately absent: it usually means “at the latest”, not
/// “to completion”, and a wrong total-object suggestion is worse than silence.
const COMPLETIVE_CUES: &[&str] = &["loppuun", "valmiiksi", "kokonaan"];

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        if clause.negative {
            if let Some(diagnostic) = negative_object(clause, tokens, analysis) {
                diagnostics.push(diagnostic);
            }
        } else if let Some(diagnostic) = aspect_suggestion(clause, tokens, analysis) {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// The negative-clause partitive rule.
fn negative_object(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<SyntaxDiagnostic> {
    let predicate = clause.predicate?;
    let (argument, case) = argument(clause, predicate, tokens, analysis)?;
    if case == Case::Partitive {
        return None;
    }
    let syntax = &analysis.tokens[argument];
    let pos = nominal_pos(syntax)?;
    let lemma = syntax.lemma.as_deref()?;
    let number = candidate_number(syntax, case);
    let mut corrections = correction::case_corrections(
        lemma,
        &tokens[argument],
        Case::Partitive,
        number,
        pos,
        crate::spec::Catalogues::embedded(),
    );
    for correction in &mut corrections {
        correction.note = "object_case";
    }
    if corrections.is_empty() {
        return None;
    }
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::ObjectCase,
        Severity::Error,
        DiagnosticConfidence::High,
        tokens[argument].start..tokens[argument].end,
        "object_case",
    );
    diagnostic.expected = Some(Case::Partitive.name().to_string());
    diagnostic.corrections = corrections;
    Some(diagnostic)
}

/// The affirmative-clause aspect suggestion.
fn aspect_suggestion(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<SyntaxDiagnostic> {
    let object = clause.object?;
    let syntax = &analysis.tokens[object];
    let case = syntax.case?;
    if is_cardinal(syntax) || nominal_pos(syntax).is_none() {
        return None;
    }
    if inside_brackets(tokens, clause.tokens.start, object) {
        return None;
    }
    let number = candidate_number(syntax, case)?;
    let words: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    let progressive = cue_present(&words, tokens, clause.tokens.start, PROGRESSIVE_CUES);
    let completive = cue_present(&words, tokens, clause.tokens.start, COMPLETIVE_CUES);
    // Neither cue, or two contradictory cues: do not guess.
    if progressive == completive {
        return None;
    }

    let target = if progressive && is_total(case) {
        (Case::Partitive, number)
    } else if completive && case == Case::Partitive {
        match number {
            Number::Singular => (Case::Genitive, Number::Singular),
            // Deferred: a partitive plural is often homonymous (`kirjoja` is
            // both `kirja` and `kirjo`), so the syntax layer's chosen lemma
            // can be the wrong one and the "total" suggestion would name a
            // different word (`kirjot`). Stay quiet rather than guess.
            Number::Plural => return None,
            _ => return None,
        }
    } else {
        return None;
    };

    let pos = nominal_pos(syntax)?;
    let lemma = syntax.lemma.as_deref()?;
    let mut corrections = correction::case_corrections(
        lemma,
        &tokens[object],
        target.0,
        Some(target.1),
        pos,
        crate::spec::Catalogues::embedded(),
    );
    for correction in &mut corrections {
        correction.note = "object_case";
    }
    corrections.truncate(1);
    if corrections.is_empty() {
        return None;
    }
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::ObjectCase,
        Severity::Suggestion,
        DiagnosticConfidence::Medium,
        tokens[object].start..tokens[object].end,
        "object_case",
    );
    diagnostic.expected = Some(target.0.name().to_string());
    diagnostic.corrections = corrections;
    Some(diagnostic)
}

fn is_total(case: Case) -> bool {
    matches!(
        case,
        Case::Genitive | Case::Accusative | Case::Nominative
    )
}

/// Whether one of `cues` occurs as a word run in this clause.
///
/// A cue inside a parenthetical is ignored: `Luen (parhaillaan) kirjan` is not
/// an aspect cue for the host clause.
fn cue_present(
    words: &[usize],
    tokens: &[Token],
    clause_start: usize,
    cues: &[&str],
) -> bool {
    cues.iter().any(|cue| {
        let cue_words: Vec<&str> = cue.split_whitespace().collect();
        if cue_words.is_empty() || words.len() < cue_words.len() {
            return false;
        }
        words.windows(cue_words.len()).any(|window| {
            window.iter().zip(&cue_words).all(|(&index, expected)| {
                tokens[index].text.to_lowercase() == *expected
            }) && window
                .iter()
                .all(|&index| !inside_brackets(tokens, clause_start, index))
        })
    })
}

fn inside_brackets(tokens: &[Token], start: usize, index: usize) -> bool {
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

fn argument(
    clause: &crate::syntax::Clause,
    predicate: usize,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<(usize, Case)> {
    let words: Vec<usize> = clause
        .tokens
        .clone()
        .filter(|&index| tokens[index].is_word())
        .collect();
    let predicate_position = words.iter().position(|&index| index == predicate)?;
    for &index in words.iter().skip(predicate_position + 1) {
        let syntax = &analysis.tokens[index];
        if !matches!(
            syntax.role,
            SyntacticRole::Object | SyntacticRole::Subject | SyntacticRole::Unknown
        ) || is_marker(&tokens[index].text)
        {
            continue;
        }
        let Some(case) = nominal_case(syntax) else {
            continue;
        };
        if nominal_pos(syntax).is_none() || is_cardinal(syntax) {
            continue;
        }
        return Some((index, case));
    }
    None
}

fn is_marker(word: &str) -> bool {
    let lower = word.to_lowercase();
    crate::syntax::is_coordinating(&lower)
        || crate::syntax::is_subordinator(&lower)
        || matches!(lower.as_str(), "joka" | "mikä" | "ken" | "kuka")
}

fn nominal_pos(syntax: &crate::syntax::TokenSyntax) -> Option<PartOfSpeech> {
    syntax.readings.iter().find_map(|reading| {
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

fn nominal_case(syntax: &crate::syntax::TokenSyntax) -> Option<Case> {
    if let Some(case) = syntax.case {
        return Some(case);
    }
    let mut cases: Vec<Case> = syntax
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

fn is_cardinal(syntax: &crate::syntax::TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Numeral))
}

fn candidate_number(syntax: &crate::syntax::TokenSyntax, case: Case) -> Option<Number> {
    let mut numbers: Vec<Number> = syntax
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        check(&tokens, &analysis)
    }

    fn suggestion(text: &str) -> SyntaxDiagnostic {
        let found = diagnostics(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].severity, Severity::Suggestion);
        found[0].clone()
    }

    #[test]
    fn a_progressive_cue_suggests_the_partitive() {
        let diagnostic = suggestion("Luen parhaillaan kirjan.");
        assert_eq!(diagnostic.kind, DiagnosticKind::ObjectCase);
        assert_eq!(diagnostic.expected.as_deref(), Some("partitive"));
        assert_eq!(
            diagnostic.corrections[0].edits[0].replacement,
            "kirjaa".to_string()
        );
    }

    #[test]
    fn a_completive_cue_suggests_the_total_object() {
        let diagnostic = suggestion("Luin kirjaa loppuun.");
        assert_eq!(diagnostic.expected.as_deref(), Some("genitive"));
        assert_eq!(
            diagnostic.corrections[0].edits[0].replacement,
            "kirjan".to_string()
        );
    }

    #[test]
    fn a_partitive_plural_stays_quiet_until_the_lemma_is_unambiguous() {
        assert!(diagnostics("Luin kirjoja loppuun.").is_empty());
    }

    #[test]
    fn matching_cases_and_missing_cues_stay_quiet() {
        for text in [
            "Luen parhaillaan kirjaa.",
            "Luin kirjan loppuun.",
            "Luen kirjan.",
            "Luen kirjaa.",
        ] {
            assert!(diagnostics(text).is_empty(), "{text}: {:#?}", diagnostics(text));
        }
    }

    #[test]
    fn a_cue_inside_a_parenthetical_is_ignored() {
        assert!(diagnostics("Luen (parhaillaan) kirjan.").is_empty());
    }

    #[test]
    fn a_negative_clause_keeps_the_error() {
        let found = diagnostics("En lue kirja.");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Error);
    }
}
