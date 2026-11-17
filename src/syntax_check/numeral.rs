//! Numeral government: a cardinal above one takes the partitive singular.
//!
//! `kaksi lasta`, `kolme miestä`, `sata autoa`. The common learner errors are
//! the nominative (`kaksi lapsi`, `kaksi lapset`) and the partitive plural
//! (`kaksi lapsia`). Both are detected; the correction is the partitive
//! singular.
//!
//! The rule stays deliberately narrow. `yksi` takes the nominative singular,
//! plural numerals (`kahdet kengät`) take the nominative plural, and a numeral
//! followed by an adjective is checked on the first nominal (`kaksi pientä
//! lasta` is quiet; `kaksi pieni lasta` flags `pieni`). A non-nominative,
//! non-partitive complement (`kaksi kolmesta`) is left alone because it is not
//! a simple counted-noun phrase.

use crate::syntax::{RoleConfidence, SyntaxAnalysis};
use crate::tags::{Case, Number, PartOfSpeech};
use crate::tokenise::Token;

use super::analysis::{clause_words, nominal_pos};
use super::correction;
use super::diagnostic::{Confidence, DiagnosticKind, Severity, SyntaxDiagnostic};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        let words = clause_words(clause, tokens);
        for (position, &numeral) in words.iter().enumerate() {
            if !is_cardinal(&analysis.tokens[numeral]) {
                continue;
            }
            if analysis.tokens[numeral].lemma.as_deref() == Some("yksi") {
                continue;
            }
            let mut complement = None;
            for &index in &words[position + 1..] {
                let token = &analysis.tokens[index];
                let finite = token
                    .readings
                    .iter()
                    .any(crate::syntax::is_finite_predicate);
                let adjective = token
                    .readings
                    .iter()
                    .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Adjective));
                if finite && !adjective {
                    break;
                }
                if nominal_pos(token).is_some() {
                    complement = Some(index);
                    break;
                }
            }
            let Some(complement) = complement else {
                continue;
            };
            let Some((case, number)) = nominal_shape(&analysis.tokens[complement]) else {
                continue;
            };
            let wrong = case == Case::Nominative
                || (case == Case::Partitive && number == Number::Plural);
            if !wrong {
                continue;
            }
            let Some(pos) = nominal_pos(&analysis.tokens[complement]) else {
                continue;
            };
            let mut candidates: Vec<&str> = Vec::new();
            if let Some(lemma) = analysis.tokens[complement].lemma.as_deref() {
                candidates.push(lemma);
            }
            for lemma in &analysis.tokens[complement].lemmas {
                if !candidates.contains(&lemma.as_str()) {
                    candidates.push(lemma.as_str());
                }
            }
            let mut corrections: Vec<_> = Vec::new();
            for lemma in candidates {
                let found: Vec<_> = correction::case_corrections(
                    lemma,
                    &tokens[complement],
                    Case::Partitive,
                    Some(Number::Singular),
                    pos,
                    crate::spec::Catalogues::embedded(),
                )
                .into_iter()
                .map(|mut correction| {
                    correction.note = "case_selection";
                    correction
                })
                .collect();
                if !found.is_empty() {
                    corrections = found;
                    break;
                }
            }
            let confident = analysis.tokens[complement].confidence == RoleConfidence::Likely;
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
                "numeral_case",
            );
            diagnostic.expected = Some("partitive singular".to_string());
            diagnostic.corrections = corrections;
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// A cardinal numeral whose own number is not plural.
///
/// `kahdet`, `kolmet` and the other plural numerals take the nominative
/// plural and are skipped.
fn is_cardinal(token: &crate::syntax::TokenSyntax) -> bool {
    let Some(lemma) = token.lemma.as_deref() else {
        return false;
    };
    if matches!(
        lemma,
        "toinen" | "ensimmäinen" | "viimeinen" | "mones" | "monesko"
    ) {
        return false;
    }
    let numeral = token
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Numeral));
    if !numeral {
        return false;
    }
    !token
        .readings
        .iter()
        .any(|reading| reading.number == Some(Number::Plural))
}

/// The unambiguous nominal case and number of a counted head.
fn nominal_shape(token: &crate::syntax::TokenSyntax) -> Option<(Case, Number)> {
    let mut shape: Option<(Case, Number)> = None;
    for reading in &token.readings {
        if !crate::syntax::is_nominal_pos(reading.part_of_speech) {
            continue;
        }
        let Some(candidate) = reading.case.zip(reading.number) else {
            continue;
        };
        match shape {
            None => shape = Some(candidate),
            Some(seen) if seen == candidate => {}
            Some(_) => return None,
        }
    }
    shape
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::analyze;
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = analyze(&tokens);
        check(&tokens, &analysis)
    }

    #[test]
    fn a_nominative_counted_noun_is_an_error() {
        let found = diagnostics("Kaksi lapsi tuli.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, DiagnosticKind::CaseSelection);
        assert_eq!(found[0].corrections[0].edits[0].replacement, "lasta");
    }

    #[test]
    fn a_partitive_plural_counted_noun_is_an_error() {
        let found = diagnostics("Kaksi lapsia tuli.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].corrections[0].edits[0].replacement, "lasta");
    }

    #[test]
    fn one_and_plural_numerals_are_quiet() {
        for text in [
            "Kaksi lasta tuli.",
            "Kaksi miestä tuli.",
            "Yksi lapsi tuli.",
            "Kahdet kengät.",
            "Kaksi pientä lasta.",
            "Kaksi kolmesta.",
            "Sata autoa.",
            "Kaksikymmentä lasta.",
        ] {
            assert!(diagnostics(text).is_empty(), "{text}: {:#?}", diagnostics(text));
        }
    }
}
