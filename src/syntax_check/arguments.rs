//! Missing and extra arguments from a tiny curated table.
//!
//! This module only asks whether an argument exists; `rection.rs` owns which
//! case it takes. A missing argument is a `Warning` because Finnish permits
//! ellipsis, and an extra argument is a `Warning` because a cognate object or
//! an idiomatic phrase can look transitive. Neither rule emits a correction:
//! deleting an argument or inventing one would be a guess.

use crate::syntax::{SyntacticRole, SyntaxAnalysis, TokenSyntax};
use crate::tags::{Case, PartOfSpeech};
use crate::tokenise::Token;

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
        let frames: Vec<crate::expand::verb_arguments::Frame> =
            crate::expand::verb_arguments::frames(lemma).collect();
        if frames.is_empty() {
            continue;
        }
        if crate::syntax::predicate_is_passive(predicate, &analysis.tokens) {
            continue;
        }
        let words: Vec<usize> = clause
            .tokens
            .clone()
            .filter(|&index| tokens[index].is_word())
            .collect();

        if let Some(diagnostic) =
            missing_argument(clause, predicate, &words, &frames, tokens, analysis)
        {
            diagnostics.push(diagnostic);
        }
        if let Some(diagnostic) =
            extra_argument(clause, predicate, &words, &frames, tokens, analysis)
        {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn missing_argument(
    clause: &crate::syntax::Clause,
    predicate: usize,
    words: &[usize],
    frames: &[crate::expand::verb_arguments::Frame],
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<SyntaxDiagnostic> {
    use crate::expand::verb_arguments::Requirement;

    let obligatory: Vec<_> = frames
        .iter()
        .filter(|frame| frame.requirement == Requirement::Obligatory)
        .collect();
    if obligatory.is_empty() {
        return None;
    }
    // A one-word clause (`Rakastan.`) is a plausible elliptical answer. A
    // clause with a subject or another complement is not.
    if words.iter().all(|&index| index == predicate) {
        return None;
    }
    if ends_with_ellipsis(clause, tokens) {
        return None;
    }
    if has_argument_nominal(predicate, words, tokens, analysis) {
        // Either the argument is present, or it is present in the wrong case
        // and rection owns the finding. Do not double-report.
        return None;
    }

    let expected = obligatory
        .iter()
        .map(|frame| match frame.case {
            Some(case) => format!("{} {}", frame.slot.name(), case.name()),
            None => frame.slot.name().to_string(),
        })
        .collect::<Vec<_>>()
        .join(" / ");
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::MissingArgument,
        Severity::Warning,
        DiagnosticConfidence::Medium,
        tokens[predicate].end..tokens[predicate].end,
        "missing_argument",
    );
    diagnostic.expected = Some(expected);
    Some(diagnostic)
}

fn extra_argument(
    clause: &crate::syntax::Clause,
    predicate: usize,
    words: &[usize],
    frames: &[crate::expand::verb_arguments::Frame],
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<SyntaxDiagnostic> {
    use crate::expand::verb_arguments::Requirement;

    let forbidden = frames
        .iter()
        .find(|frame| frame.requirement == Requirement::Forbidden)?;
    if forbidden.slot != crate::expand::verb_arguments::Slot::Object {
        return None;
    }
    let predicate_position = words.iter().position(|&index| index == predicate)?;
    for &index in words.iter().skip(predicate_position + 1) {
        let syntax = &analysis.tokens[index];
        if syntax.role == SyntacticRole::Predicate {
            // The object belongs to the next predicate, not this one.
            break;
        }
        if is_marker(&tokens[index].text)
            || nominal_case(syntax).is_none()
            || nominal_pos(syntax).is_none()
        {
            continue;
        }
        // The syntax layer reads a post-predicate nominative as a subject
        // (`Nukun kirja`) or as an object (`Herää mies`). A nominative that
        // agrees with the predicate can be a genuine presentational subject;
        // one that does not agree is the learner error this rule is for.
        let agrees = subject_agrees(predicate, index, analysis);
        let presentational_subject = syntax.role == SyntacticRole::Object
            && nominal_case(syntax) == Some(Case::Nominative)
            && clause.subject.is_none()
            && agrees;
        let candidate = match syntax.role {
            SyntacticRole::Object => !presentational_subject,
            SyntacticRole::Subject => !agrees,
            _ => false,
        };
        if !candidate {
            continue;
        }
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::ExtraArgument,
            Severity::Warning,
            DiagnosticConfidence::Medium,
            tokens[index].start..tokens[index].end,
            "extra_argument",
        );
        diagnostic.expected = Some("no object".to_string());
        return Some(diagnostic);
    }
    None
}

/// Whether a post-predicate nominative can be the predicate's subject.
fn subject_agrees(predicate: usize, candidate: usize, analysis: &SyntaxAnalysis) -> bool {
    let Some(reading) = super::agreement::finite_reading(&analysis.tokens[predicate]) else {
        return true;
    };
    let (Some(predicate_person), Some(predicate_number)) = (reading.person, reading.number) else {
        return true;
    };
    let syntax = &analysis.tokens[candidate];
    let number = syntax
        .readings
        .iter()
        .filter(|reading| reading.case == Some(Case::Nominative))
        .find_map(|reading| reading.number);
    let Some(number) = number else {
        return true;
    };
    let person = syntax
        .readings
        .iter()
        .find_map(|reading| reading.person)
        .unwrap_or(crate::tags::Person::Third);
    person == predicate_person && number == predicate_number
}

/// A nominal that can stand in for the predicate's argument.
///
/// A pre-predicate subject is the one nominal that cannot be the missing
/// object; a post-predicate nominal can be an argument even when the role
/// resolver guessed “subject” (`Luotan sinä`). A later predicate ends this
/// predicate's scan, so the object of `on` in
/// `Koira, joka nukkuu sohvalla on minun` is not read as `nukkuu`'s object.
fn has_argument_nominal(
    predicate: usize,
    words: &[usize],
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> bool {
    let Some(predicate_position) = words.iter().position(|&index| index == predicate) else {
        return false;
    };
    let end = words
        .iter()
        .enumerate()
        .skip(predicate_position + 1)
        .find(|&(_, &index)| analysis.tokens[index].role == SyntacticRole::Predicate)
        .map(|(position, _)| position)
        .unwrap_or(words.len());
    words[..end].iter().copied().any(|index| {
        let syntax = &analysis.tokens[index];
        index != predicate
            && syntax.role != SyntacticRole::Predicate
            && syntax.role != SyntacticRole::Auxiliary
            && syntax.role != SyntacticRole::Modifier
            && !(index < predicate && syntax.role == SyntacticRole::Subject)
            && !is_marker(&tokens[index].text)
            && nominal_case(syntax).is_some()
            && nominal_pos(syntax).is_some()
    })
}

fn ends_with_ellipsis(clause: &crate::syntax::Clause, tokens: &[Token]) -> bool {
    for token in tokens.iter().skip(clause.tokens.end) {
        if token.is_word() {
            return false;
        }
        if token.text.contains("...") || token.text.contains('…') {
            return true;
        }
    }
    false
}

fn is_marker(word: &str) -> bool {
    let lower = word.to_lowercase();
    crate::syntax::is_coordinating(&lower)
        || crate::syntax::is_subordinator(&lower)
        || matches!(lower.as_str(), "joka" | "mikä" | "ken" | "kuka")
}

fn nominal_pos(syntax: &TokenSyntax) -> Option<PartOfSpeech> {
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

fn nominal_case(syntax: &TokenSyntax) -> Option<Case> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        check(&tokens, &analysis)
    }

    fn kinds(text: &str) -> Vec<DiagnosticKind> {
        diagnostics(text)
            .into_iter()
            .map(|diagnostic| diagnostic.kind)
            .collect()
    }

    #[test]
    fn a_missing_obligatory_argument_is_a_warning() {
        let found = diagnostics("Hän luottaa.");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, DiagnosticKind::MissingArgument);
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn a_present_argument_is_quiet() {
        assert!(kinds("Luotan sinuun.").is_empty());
        assert!(kinds("Minä rakastan sinua.").is_empty());
    }

    #[test]
    fn a_one_word_clause_is_not_missing() {
        assert!(kinds("Rakastan.").is_empty());
        assert!(kinds("Luotan.").is_empty());
    }

    #[test]
    fn a_wrong_case_argument_is_left_to_rection() {
        assert!(!kinds("Luotan sinä.").contains(&DiagnosticKind::MissingArgument));
        assert!(!kinds("Minä rakastan kirja.").contains(&DiagnosticKind::MissingArgument));
    }

    #[test]
    fn an_extra_object_is_a_warning() {
        let found = diagnostics("Nukun kirja.");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, DiagnosticKind::ExtraArgument);
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn a_presentational_subject_is_not_extra() {
        assert!(kinds("Herää mies.").is_empty());
    }

    #[test]
    fn a_later_predicate_owns_its_object() {
        assert!(!kinds("Koira, joka nukkuu sohvalla on minun.")
            .contains(&DiagnosticKind::ExtraArgument));
    }
}
