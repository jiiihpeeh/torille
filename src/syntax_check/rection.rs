//! Verb rection and case selection from a curated valency table.
//!
//! Only verbs in `data/verb_rection.csv` are checked. A candidate argument is
//! the first nominal after the predicate that is not a complement, relative
//! pronoun or conjunction. If its case matches any frame the verb accepts,
//! there is no error; otherwise each frame contributes a correction, in table
//! priority order.

use crate::syntax::{SyntacticRole, SyntaxAnalysis};
use crate::tags::{Case, Number, PartOfSpeech};
use crate::tokenise::Token;

use super::correction;
use super::diagnostic::{
    DiagnosticConfidence, DiagnosticKind, Severity, SyntaxDiagnostic,
};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        let Some(predicate) = clause.predicate else {
            continue;
        };
        let Some(lemma) = analysis.tokens[predicate].lemma.as_deref() else {
            continue;
        };
        let frames = crate::expand::verb_rection::cases(lemma);
        if frames.is_empty() {
            continue;
        }
        let Some((argument, case)) = argument(clause, predicate, tokens, analysis) else {
            continue;
        };
        if frames.contains(&case) {
            continue;
        }
        let syntax = &analysis.tokens[argument];
        let Some(pos) = nominal_pos(syntax) else {
            continue;
        };
        let number = candidate_number(syntax, case);
        let mut corrections = Vec::new();
        for (priority, frame) in frames.iter().enumerate() {
            for mut correction in correction::case_corrections(
                syntax.lemma.as_deref().unwrap_or_default(),
                &tokens[argument],
                *frame,
                number,
                pos,
                crate::spec::Catalogues::embedded(),
            ) {
                // Keep the CSV frame order ahead of edit distance.
                correction.score += priority as f32 * 100.0;
                corrections.push(correction);
            }
        }
        corrections.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
        });
        let confident = analysis.tokens[predicate].confidence
            == crate::syntax::RoleConfidence::Likely;
        let (severity, confidence) = if confident {
            (Severity::Error, DiagnosticConfidence::High)
        } else {
            (Severity::Warning, DiagnosticConfidence::Medium)
        };
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::VerbRection,
            severity,
            confidence,
            tokens[argument].start..tokens[argument].end,
            "verb_rection",
        );
        diagnostic.expected = Some(
            frames
                .iter()
                .map(|case| case.name())
                .collect::<Vec<_>>()
                .join(" / "),
        );
        diagnostic.corrections = corrections;
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// The first post-predicate nominal that can be the governed argument.
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
        if nominal_pos(syntax).is_none() {
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

fn nominal_pos(syntax: &crate::syntax::TokenSyntax) -> Option<PartOfSpeech> {
    syntax.readings.iter().find_map(|reading| {
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
