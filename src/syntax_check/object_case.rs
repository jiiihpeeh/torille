//! Object case in negative clauses.
//!
//! Finnish requires the partitive for the object of a negated clause: `en lue
//! kirjaa`, not `en lue kirja`. This is the one object-case rule with a local,
//! unambiguous cue; aspect-based partitive/total choices are suggestions and
//! belong to a later phase.

use crate::syntax::{SyntacticRole, SyntaxAnalysis};
use crate::tags::{Case, Number, PartOfSpeech};
use crate::tokenise::Token;

use super::correction;
use super::diagnostic::{DiagnosticConfidence, DiagnosticKind, Severity, SyntaxDiagnostic};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        if !clause.negative {
            continue;
        }
        let Some(predicate) = clause.predicate else {
            continue;
        };
        let Some((argument, case)) = argument(clause, predicate, tokens, analysis) else {
            continue;
        };
        if case == Case::Partitive {
            continue;
        }
        let syntax = &analysis.tokens[argument];
        let Some(pos) = nominal_pos(syntax) else {
            continue;
        };
        let Some(lemma) = syntax.lemma.as_deref() else {
            continue;
        };
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
            continue;
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
        diagnostics.push(diagnostic);
    }
    diagnostics
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
