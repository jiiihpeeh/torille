//! Existential subject case.
//!
//! An existential's subject is nominative or partitive:
//! `Pöydällä on kirja.`, `Pöydällä on kirjoja.` A genitive subject is a learner
//! error (`Pöytällä on kirjan.`) and is never correct.
//!
//! Detection and correction are separate here. The diagnostic is certain: the
//! clause is existential and the final nominal is genitive. The rewrite is
//! not: Finnish chooses nominative or partitive by boundedness and count/mass,
//! which the engine cannot know. We offer both engine-licensed forms as
//! `Likely` corrections and never invent a form.

use crate::syntax::SyntaxAnalysis;
use crate::tags::{Case, PartOfSpeech};
use crate::tokenise::Token;

use super::analysis::{candidate_number, clause_words, nominal_bases, nominal_case, nominal_pos};
use super::correction;
use super::diagnostic::{
    Confidence, Correction, CorrectionLicense, DiagnosticKind, Severity, SyntaxDiagnostic,
};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        if !clause.existential {
            continue;
        }
        let Some(predicate) = clause.predicate else {
            continue;
        };
        let words = clause_words(clause, tokens);
        let Some(&last) = words.last() else {
            continue;
        };
        if last <= predicate {
            continue;
        }
        // A line break or an unpunctuated coordinator can end a clause in the
        // middle of a sentence (`... laulujen\nja kanteleen maa`). The case
        // error we can defend is a sentence-final genitive, not any
        // clause-final one.
        if !sentence_ends(tokens, last) {
            continue;
        }
        let syntax = &analysis.tokens[last];
        // A genitive modifier of a following head is not the final word, and
        // a proper name or pronoun at the end is a different construction
        // (`Täällä on minun.`). The target is a common noun or adjective.
        if !matches!(
            nominal_pos(syntax),
            Some(PartOfSpeech::Noun | PartOfSpeech::Adjective)
        ) || tokens[last].is_capitalised()
        {
            continue;
        }
        if nominal_case(syntax) != Some(Case::Genitive) {
            continue;
        }
        let number = candidate_number(syntax, Case::Genitive);
        let Some(pos) = nominal_pos(syntax) else {
            continue;
        };
        let mut corrections = Vec::new();
        for target in [Case::Nominative, Case::Partitive] {
            for lemma in nominal_bases(syntax) {
                let found: Vec<Correction> = correction::case_corrections(
                    lemma,
                    &tokens[last],
                    target,
                    number,
                    pos,
                    crate::spec::Catalogues::embedded(),
                )
                .into_iter()
                .map(|mut correction| {
                    for edit in &mut correction.edits {
                        if edit.start == tokens[last].start && edit.end == tokens[last].end {
                            edit.replacement = crate::suggest::casing::match_case(
                                &tokens[last].text,
                                &edit.replacement,
                            );
                        }
                    }
                    correction
                        .licensed_by(CorrectionLicense::Inflection)
                        .with_confidence(Confidence::Likely)
                })
                .collect();
                if !found.is_empty() {
                    corrections.extend(found);
                    break;
                }
            }
        }
        let mut seen: Vec<String> = Vec::new();
        corrections.retain(|correction| {
            let Some(edit) = correction.edits.first() else {
                return false;
            };
            if seen.contains(&edit.replacement) {
                return false;
            }
            seen.push(edit.replacement.clone());
            true
        });
        for correction in &mut corrections {
            correction.note = "case_selection";
        }
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::CaseSelection,
            Severity::Error,
            Confidence::Certain,
            tokens[last].start..tokens[last].end,
            "existential_subject_case",
        );
        diagnostic.expected = Some("nominative or partitive".to_string());
        diagnostic.corrections = corrections;
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// Whether the next real punctuation after `last` ends a sentence.
///
/// A line break is not a sentence end: the prose corpus wraps sentences across
/// lines, and a clause split at the wrap must not turn a genitive modifier
/// into a genitive subject. A word before terminal punctuation means the
/// clause continues, so the finding stays quiet.
fn sentence_ends(tokens: &[Token], last: usize) -> bool {
    for token in &tokens[last + 1..] {
        if token.is_word() {
            return false;
        }
        if token
            .text
            .chars()
            .any(|character| matches!(character, '.' | '!' | '?' | '…'))
        {
            return true;
        }
    }
    true
}
