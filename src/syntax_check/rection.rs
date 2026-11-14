//! Verb rection and case selection from a curated valency table.
//!
//! Only verbs in `data/verb_rection.csv` are checked. A candidate argument is
//! the first nominal after the predicate that is not a complement, relative
//! pronoun or conjunction. If its case matches any frame the verb accepts,
//! there is no error; otherwise each frame contributes a correction, in table
//! priority order.

use crate::syntax::{SyntacticRole, SyntaxAnalysis};
use crate::tags::Case;
use crate::tokenise::Token;

use super::analysis::{
    candidate_number, clause_words, is_marker, nominal_case, nominal_or_numeral_pos,
};
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
        if nominal_or_numeral_pos(syntax).is_none() {
            continue;
        }
        let number = candidate_number(syntax, case);
        let mut corrections = Vec::new();
        for (lemma_position, (lemma, base_pos)) in correction_bases(syntax).iter().enumerate() {
            for (priority, frame) in frames.iter().enumerate() {
                for mut correction in correction::case_corrections(
                    lemma,
                    &tokens[argument],
                    *frame,
                    number,
                    *base_pos,
                    crate::spec::Catalogues::embedded(),
                ) {
                    // Keep the CSV frame order ahead of edit distance, and the
                    // chosen lemma's forms ahead of a homonym's.
                    correction.score +=
                        priority as f32 * 100.0 + lemma_position as f32 * 1000.0;
                    corrections.push(correction);
                }
            }
        }
        corrections.sort_by(|a, b| {
            a.score
                .total_cmp(&b.score)
                .then_with(|| a.edits[0].replacement.cmp(&b.edits[0].replacement))
        });
        corrections.dedup_by(|a, b| a.edits[0].replacement == b.edits[0].replacement);
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

/// Every nominal lemma behind the argument surface, chosen lemma first.
///
/// A homonymous surface (`kirjoja`, `tuli`) belongs to more than one base. The
/// syntax layer records them in [`crate::syntax::TokenSyntax::lemmas`], so the
/// rection rule can offer the right paradigm without re-running the base
/// search. Only rows the sanalista marks nominal or numeral contribute.
fn correction_bases(syntax: &crate::syntax::TokenSyntax) -> Vec<(&str, crate::tags::PartOfSpeech)> {
    use crate::tags::PartOfSpeech;

    let mut ordered: Vec<&str> = Vec::new();
    if let Some(lemma) = syntax.lemma.as_deref() {
        ordered.push(lemma);
    }
    for lemma in &syntax.lemmas {
        let lemma = lemma.as_str();
        if !ordered.contains(&lemma) {
            ordered.push(lemma);
        }
    }

    let mut out = Vec::new();
    for lemma in ordered {
        for row in crate::kotus::sanalista().by_word(lemma) {
            let Some(pos) = PartOfSpeech::parse_all(row.part_of_speech())
                .first()
                .copied()
            else {
                continue;
            };
            if matches!(
                pos,
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Pronoun
                    | PartOfSpeech::Numeral
                    | PartOfSpeech::Ordinal
            ) {
                out.push((lemma, pos));
                break;
            }
        }
    }
    out
}

/// The first post-predicate nominal that can be the governed argument.
fn argument(
    clause: &crate::syntax::Clause,
    predicate: usize,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<(usize, Case)> {
    let words = clause_words(clause, tokens);
    let predicate_position = words.iter().position(|&index| index == predicate)?;
    for &index in words.iter().skip(predicate_position + 1) {
        let syntax = &analysis.tokens[index];
        if !matches!(
            syntax.role,
            SyntacticRole::Object | SyntacticRole::Subject | SyntacticRole::Unknown
        ) || is_marker(&analysis.tokens[index].key)
        {
            continue;
        }
        let Some(case) = nominal_case(syntax) else {
            continue;
        };
        if nominal_or_numeral_pos(syntax).is_none() {
            continue;
        }
        return Some((index, case));
    }
    None
}


