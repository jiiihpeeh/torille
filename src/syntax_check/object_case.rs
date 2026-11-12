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

use super::analysis::{
    candidate_number, clause_words, inside_brackets, is_marker, nominal_case, nominal_pos,
};
use super::correction;
use super::diagnostic::{
    Correction, DiagnosticConfidence, DiagnosticKind, Severity, SyntaxDiagnostic,
};

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
    let words = clause_words(clause, tokens);
    let progressive = cue_present(&words, tokens, clause.tokens.start, PROGRESSIVE_CUES);
    let completive = cue_present(&words, tokens, clause.tokens.start, COMPLETIVE_CUES);
    // Neither cue, or two contradictory cues: do not guess.
    if progressive == completive {
        return None;
    }

    let plural_completive = completive && case == Case::Partitive && number == Number::Plural;
    let target = if progressive && is_total(case) {
        (Case::Partitive, number)
    } else if completive && case == Case::Partitive {
        match number {
            Number::Singular => (Case::Genitive, Number::Singular),
            // A partitive plural may be homonymous (`kirjoja` belongs to both
            // `kirja` and `kirjo`). The helper only returns a correction when
            // every plausible base names the same total form.
            Number::Plural => (Case::Nominative, Number::Plural),
            _ => return None,
        }
    } else {
        return None;
    };

    let mut corrections = if plural_completive {
        vec![plural_total_correction(&tokens[object], syntax)?]
    } else {
        let pos = nominal_pos(syntax)?;
        let lemma = syntax.lemma.as_deref()?;
        correction::case_corrections(
            lemma,
            &tokens[object],
            target.0,
            Some(target.1),
            pos,
            crate::spec::Catalogues::embedded(),
        )
    };
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

/// The nominative plural of an unambiguous partitive plural.
///
/// A partitive plural can be homonymous (`kirjoja` belongs to both `kirja`
/// and `kirjo`). The syntax layer records every lemma that produced the
/// surface in [`crate::syntax::TokenSyntax::lemmas`], so this helper generates
/// each nominal base's nominative plural and returns a correction only when
/// every candidate names the same form. Ambiguous surfaces stay quiet instead
/// of suggesting the wrong word.
fn plural_total_correction(
    token: &Token,
    syntax: &crate::syntax::TokenSyntax,
) -> Option<Correction> {
    let mut seen_bases: Vec<&str> = Vec::new();
    let mut seen_words: Vec<String> = Vec::new();
    let mut candidates: Vec<Correction> = Vec::new();
    for lemma in &syntax.lemmas {
        if seen_bases.contains(&lemma.as_str()) {
            continue;
        }
        seen_bases.push(lemma);
        // A lemma may have several sanalista rows. Every nominal row is a
        // plausible base; a non-nominal row contributes no object form.
        for row in crate::kotus::sanalista().by_word(lemma) {
            let Some(pos) = PartOfSpeech::parse_all(row.part_of_speech())
                .first()
                .copied()
            else {
                continue;
            };
            if !is_nominal_pos(Some(pos)) {
                continue;
            }
            for correction in correction::case_corrections(
                lemma,
                token,
                Case::Nominative,
                Some(Number::Plural),
                pos,
                crate::spec::Catalogues::embedded(),
            ) {
                let Some(replacement) = correction
                    .edits
                    .first()
                    .map(|edit| edit.replacement.clone())
                else {
                    continue;
                };
                if seen_words.contains(&replacement) {
                    continue;
                }
                seen_words.push(replacement);
                candidates.push(correction);
            }
        }
    }
    if candidates.len() == 1 {
        candidates.pop()
    } else {
        None
    }
}

fn is_nominal_pos(pos: Option<PartOfSpeech>) -> bool {
    matches!(
        pos,
        Some(
            PartOfSpeech::Noun
                | PartOfSpeech::Adjective
                | PartOfSpeech::Pronoun
                | PartOfSpeech::Ordinal
        )
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

fn is_cardinal(syntax: &crate::syntax::TokenSyntax) -> bool {
    syntax
        .readings
        .iter()
        .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Numeral))
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
    fn an_unambiguous_partitive_plural_takes_the_nominative() {
        let diagnostic = suggestion("Luin autoja loppuun.");
        assert_eq!(diagnostic.expected.as_deref(), Some("nominative"));
        assert_eq!(
            diagnostic.corrections[0].edits[0].replacement,
            "autot".to_string()
        );
    }

    #[test]
    fn an_ambiguous_partitive_plural_stays_quiet() {
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
