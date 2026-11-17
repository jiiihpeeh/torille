//! Adposition case government.
//!
//! A closed-class adposition governs a lexical case: `pöydän alla`,
//! `illallisen jälkeen`, `ilman rahaa`. The table is
//! `data/adposition_cases.csv`; the rule only fires when the adposition is
//! lexically present, the complement is adjacent and unambiguously nominal, and
//! the observed case is one of the common learner slips (nominative or
//! partitive against a genitive frame, and the reverse). Everything else stays
//! quiet rather than guessing from the surface.

use crate::syntax::{RoleConfidence, SyntaxAnalysis, TokenSyntax};
use crate::tags::{Case, PartOfSpeech};
use crate::tokenise::Token;

use super::analysis::{
    candidate_number, clause_words, nominal_bases, nominal_case, nominal_pos,
};
use super::correction;
use super::diagnostic::{Confidence, DiagnosticKind, Severity, SyntaxDiagnostic};

/// Which side of the adposition its complement can stand on.
#[derive(Clone, Copy)]
struct Sides {
    preposition: bool,
    postposition: bool,
}

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        let words = clause_words(clause, tokens);
        for (position, &index) in words.iter().enumerate() {
            let sides = sides(&analysis.tokens[index]);
            if !sides.preposition && !sides.postposition {
                continue;
            }
            // The lemma is the right key for an inflected adposition
            // (`luonani` -> `luona`); the surface is the fallback for a
            // homonym whose chosen lemma is a noun (`ilman` -> `ilma`).
            let syntax = &analysis.tokens[index];
            let mut frames = syntax
                .lemma
                .as_deref()
                .map(crate::expand::adposition_cases::cases)
                .unwrap_or_default();
            if frames.is_empty() {
                frames = crate::expand::adposition_cases::cases(&syntax.key);
            }
            // A multi-case adposition is intentionally left quiet: the table's
            // order is not a meaning model.
            let [expected] = frames.as_slice() else {
                continue;
            };
            let Some(complement) = complement(position, sides, &words, tokens, &analysis.tokens)
            else {
                continue;
            };
            let syntax = &analysis.tokens[complement];
            let Some(case) = nominal_case(syntax) else {
                continue;
            };
            if !is_recoverable_mismatch(*expected, case) {
                continue;
            }
            let Some(pos) = nominal_pos(syntax) else {
                continue;
            };
            let number = candidate_number(syntax, case);
            let mut corrections = Vec::new();
            for lemma in nominal_bases(syntax) {
                let found: Vec<_> = correction::case_corrections(
                    lemma,
                    &tokens[complement],
                    *expected,
                    number,
                    pos,
                    crate::spec::Catalogues::embedded(),
                )
                .into_iter()
                .map(|mut correction| {
                    for edit in &mut correction.edits {
                        if edit.start == tokens[complement].start && edit.end == tokens[complement].end
                        {
                            edit.replacement = crate::suggest::casing::match_case(
                                &tokens[complement].text,
                                &edit.replacement,
                            );
                        }
                    }
                    correction.note = "case_selection";
                    correction
                })
                .collect();
                if !found.is_empty() {
                    corrections = found;
                    break;
                }
            }
            let confident = syntax.confidence == RoleConfidence::Likely;
            let (severity, confidence) = if confident {
                (Severity::Error, Confidence::Certain)
            } else {
                (Severity::Warning, Confidence::Likely)
            };
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::CaseSelection,
                severity,
                confidence,
                tokens[complement].start..tokens[complement].end,
                "adposition_case",
            );
            diagnostic.expected = Some(expected.name().to_string());
            diagnostic.corrections = corrections;
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// The adposition's allowed complement positions.
///
/// The morphology layer keeps one part of speech per reading, so `ennen`
/// (listed as `adverbi, prepositio, postpositio`) is read only as an adverb.
/// The sanalista row is the authority, consulted only for a token that already
/// has an adverb/adposition reading.
fn sides(token: &TokenSyntax) -> Sides {
    let candidate = token.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(PartOfSpeech::Adverb | PartOfSpeech::Preposition | PartOfSpeech::Postposition)
        )
    });
    if !candidate {
        return Sides {
            preposition: false,
            postposition: false,
        };
    }
    let mut preposition = token
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Preposition));
    let mut postposition = token
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Postposition));
    for key in [Some(token.key.as_str()), token.lemma.as_deref()]
        .into_iter()
        .flatten()
    {
        for row in crate::kotus::sanalista().by_word(key) {
            for pos in PartOfSpeech::parse_all(row.part_of_speech()) {
                match pos {
                    PartOfSpeech::Preposition => preposition = true,
                    PartOfSpeech::Postposition => postposition = true,
                    _ => {}
                }
            }
        }
    }
    Sides {
        preposition,
        postposition,
    }
}

/// The adjacent complement, preferring the preposition side when a token can
/// be both (`ennen` is a preposition; `kautta` can be either).
fn complement(
    position: usize,
    sides: Sides,
    words: &[usize],
    tokens: &[Token],
    out: &[TokenSyntax],
) -> Option<usize> {
    let adposition = words[position];
    let mut candidates = Vec::with_capacity(2);
    if sides.preposition && let Some(&next) = words.get(position + 1) {
        candidates.push(next);
    }
    if sides.postposition && let Some(previous) = position.checked_sub(1) {
        candidates.push(words[previous]);
    }
    candidates.into_iter().find(|&index| {
        !comma_between(tokens, adposition, index)
            && is_reliable_nominal(&out[index])
            && nominal_case(&out[index]).is_some()
    })
}

/// A nominal with no competing finite-verb reading.
fn is_reliable_nominal(token: &TokenSyntax) -> bool {
    nominal_pos(token).is_some()
        && !token
            .readings
            .iter()
            .any(crate::syntax::is_finite_predicate)
}

/// Whether a mismatch is the kind of learner slip this rule can correct.
///
/// A genitive frame accepts a nominative or partitive error, and a partitive
/// frame accepts a nominative error. A genitive after a partitive-governing
/// preposition is deliberately quiet: it is often the modifier of a following
/// nominal or participle (`ilman Jumalan tahtomatta`, `ennen illan tuloa`),
/// not a wrong-case complement. A complement in any other case may belong to
/// a different construction, so it also stays quiet.
fn is_recoverable_mismatch(expected: Case, observed: Case) -> bool {
    if expected == observed {
        return false;
    }
    matches!(
        (expected, observed),
        (Case::Genitive, Case::Nominative | Case::Partitive) | (Case::Partitive, Case::Nominative)
    )
}

/// A comma between the adposition and its complement breaks the phrase.
fn comma_between(tokens: &[Token], left: usize, right: usize) -> bool {
    let (left, right) = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    tokens[left + 1..right]
        .iter()
        .filter(|token| !token.is_word())
        .any(|token| token.text.contains(','))
}
