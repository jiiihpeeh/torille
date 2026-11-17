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
use crate::tags::{Case, Mood, Number, PartOfSpeech};
use crate::tokenise::Token;

use super::analysis::{
    candidate_number, clause_words, inside_brackets, is_marker, nominal_bases, nominal_case,
    nominal_pos,
};
use super::correction;
use super::diagnostic::{
    Correction, Confidence, DiagnosticKind, Severity, SyntaxDiagnostic,
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
        } else {
            diagnostics.extend(bare_nominative_objects(clause, tokens, analysis));
            if let Some(diagnostic) = aspect_suggestion(clause, tokens, analysis) {
                diagnostics.push(diagnostic);
            }
        }
    }
    diagnostics
}

/// A singular bare nominative object outside the constructions that license it.
///
/// The total object is genitive singular or nominative plural; a nominative
/// **singular** object is correct only in an imperative, passive or necessive
/// clause. `Minä näen kirja.` is therefore an error, while `Näen kirjat.`,
/// `Syö puuro!`, `Syötiin puuro.` and `Minun täytyy lukea kirja.` are not.
fn bare_nominative_objects(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic> {
    if clause.existential {
        return Vec::new();
    }
    let Some(predicate) = clause.predicate else {
        return Vec::new();
    };
    if analysis.tokens[predicate].readings.iter().any(|reading| {
        reading.mood == Some(Mood::Imperative)
            && crate::syntax::is_finite_predicate(reading)
    }) || crate::syntax::predicate_is_passive(predicate, &analysis.tokens)
    {
        return Vec::new();
    }
    // A genitive subject is the necessive construction (`Minun täytyy ...`).
    if clause
        .subject
        .is_some_and(|index| analysis.tokens[index].case == Some(Case::Genitive))
    {
        return Vec::new();
    }

    // The role layer's `object` is the one it committed to. Walking every
    // Object-role token instead reports coordinated subjects (`Matti` after
    // `ja`) and other second-pass noise.
    let Some(index) = clause.object else {
        return Vec::new();
    };
    let Some(subject) = clause.subject else {
        return Vec::new();
    };
    // A dropped subject (`Näen kirja.`) and a postverbal subject
    // (`sanoi julma mies`) are readings where the role pass is less reliable.
    // Keep the rule to the ordinary preverbal-subject, postverbal-object
    // order.
    if subject >= predicate || index <= predicate || subject == index {
        return Vec::new();
    }
    // A bare nominative object is a learner error in a simple clausal object
    // slot. Requiring it to be the clause's final word keeps the rule away
    // from the many role-path readings where a later nominal is an adverbial,
    // a coordinated subject, or part of a comparison.
    let words = clause_words(clause, tokens);
    if words.last() != Some(&index) {
        return Vec::new();
    }
    if clause.tokens.clone().any(|word| {
        tokens[word].is_word()
            && word > predicate
            && word < index
            && matches!(analysis.tokens[word].key.as_str(), "kuten" | "kuin")
    }) {
        return Vec::new();
    }
    if !tokens[index].is_word() || tokens[index].is_capitalised() {
        return Vec::new();
    }
    let syntax = &analysis.tokens[index];
    if syntax.role != SyntacticRole::Object
        || nominal_pos(syntax) != Some(PartOfSpeech::Noun)
        || syntax
            .readings
            .iter()
            .any(|reading| reading.possessive.is_some())
    {
        return Vec::new();
    }
    let Some(number) = unique_nominative_number(syntax) else {
        return Vec::new();
    };
    if number != Number::Singular {
        return Vec::new();
    }
    let pos = PartOfSpeech::Noun;
    let mut corrections = Vec::new();
    for target in [Case::Genitive, Case::Partitive] {
        for lemma in nominal_bases(syntax) {
            let found: Vec<Correction> = correction::case_corrections(
                lemma,
                &tokens[index],
                target,
                Some(Number::Singular),
                pos,
                crate::spec::Catalogues::embedded(),
            )
            .into_iter()
            .map(|mut correction| {
                correction.note = "object_case";
                correction.with_confidence(Confidence::Likely)
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
    let confident = syntax.confidence == crate::syntax::RoleConfidence::Likely;
    let (severity, confidence) = if confident {
        (Severity::Error, Confidence::Certain)
    } else {
        (Severity::Warning, Confidence::Likely)
    };
    let mut diagnostic = SyntaxDiagnostic::new(
        DiagnosticKind::ObjectCase,
        severity,
        confidence,
        tokens[index].start..tokens[index].end,
        "bare_nominative_object",
    );
    diagnostic.expected = Some("genitive or partitive".to_string());
    diagnostic.corrections = corrections;
    vec![diagnostic]
}

/// The unique case and number of a nominal token.
///
/// `lasta` is the partitive of `lapsi` and the nominative of `lasta`; the
/// role pass may commit to the nominative and hide the valid partitive
/// object. A surface with competing nominal cases is not a bare nominative.
fn unique_nominative_number(syntax: &crate::syntax::TokenSyntax) -> Option<Number> {
    let mut cases = Vec::new();
    let mut numbers = Vec::new();
    for reading in &syntax.readings {
        if !crate::syntax::is_nominal_pos(reading.part_of_speech) {
            continue;
        }
        let Some(case) = reading.case else {
            continue;
        };
        cases.push(case);
        if case == Case::Nominative && let Some(number) = reading.number {
            numbers.push(number);
        }
    }
    cases.sort_unstable();
    cases.dedup();
    if cases != [Case::Nominative] {
        return None;
    }
    numbers.sort_unstable();
    numbers.dedup();
    match numbers.as_slice() {
        [number] => Some(*number),
        _ => None,
    }
}

/// The negative-clause partitive rule.
fn negative_object(
    clause: &crate::syntax::Clause,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<SyntaxDiagnostic> {
    let predicate = clause.predicate?;
    let (argument, case) = argument(clause, predicate, clause.subject, tokens, analysis)?;
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
        Confidence::Certain,
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
    let progressive = cue_present(
        &words,
        tokens,
        analysis,
        clause.tokens.start,
        PROGRESSIVE_CUES,
    );
    let completive = cue_present(
        &words,
        tokens,
        analysis,
        clause.tokens.start,
        COMPLETIVE_CUES,
    );
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
        Confidence::Likely,
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
    analysis: &SyntaxAnalysis,
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
                analysis.tokens[index].key == *expected
            }) && window
                .iter()
                .all(|&index| !inside_brackets(tokens, clause_start, index))
        })
    })
}

fn argument(
    clause: &crate::syntax::Clause,
    predicate: usize,
    subject: Option<usize>,
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Option<(usize, Case)> {
    let words = clause_words(clause, tokens);
    let predicate_position = words.iter().position(|&index| index == predicate)?;
    for &index in words.iter().skip(predicate_position + 1) {
        // A merged clause can hold a second, later predicate with its own
        // subject (`... sulkeutui huoneeseensa eikä ... maistanut ...`). The
        // subject is never the object of the negation, even though its
        // nominative case would otherwise make it a candidate.
        if Some(index) == subject {
            continue;
        }
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
