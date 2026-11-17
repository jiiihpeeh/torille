//! Possessive-suffix agreement.
//!
//! Standard written Finnish agrees the possessive suffix with an expressed
//! genitive personal pronoun: `minun kirjani`, `sinun kirjasi`,
//! `hänen kirjansa`. Colloquial Finnish often drops the suffix (`mun kirja`),
//! so a missing suffix is a `Suggestion`; a suffix that names the wrong person
//! is a `Warning`. Neither is an `Error`.
//!
//! Only an adjacent personal pronoun licenses the check; a suffix standing on
//! its own (`Kirjani on uusi.`) is perfectly good and is left alone.

use crate::syntax::{SyntacticRole, SyntaxAnalysis, TokenSyntax};
use crate::tags::{Case, PartOfSpeech, Possessive};
use crate::tokenise::Token;

use super::analysis::{clause_words, nominal_case, nominal_pos};
use super::diagnostic::{Confidence, DiagnosticKind, Severity, SyntaxDiagnostic};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        let words = clause_words(clause, tokens);
        if words.len() < 2 {
            continue;
        }
        // A pronoun plus a head that already carries a suffix.
        for (position, &head) in words.iter().enumerate() {
            if nominal_pos(&analysis.tokens[head]) != Some(PartOfSpeech::Noun) {
                continue;
            }
            let Some(Some(actual)) = possessive_state(&analysis.tokens[head]) else {
                continue;
            };
            let Some(pronoun) = preceding_possessor(&words[..position], &analysis.tokens) else {
                continue;
            };
            let Some(expected) = possessor_of(&analysis.tokens[pronoun]) else {
                continue;
            };
            if actual == expected {
                continue;
            }
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::PossessiveAgreement,
                Severity::Warning,
                Confidence::Certain,
                tokens[head].start..tokens[head].end,
                "possessive_agreement",
            );
            diagnostic.expected = Some(format!(
                "{} possessive",
                possessive_name(expected)
            ));
            diagnostics.push(diagnostic);
        }
        // A genitive personal pronoun followed by a head without a suffix.
        for (position, &pronoun) in words.iter().enumerate() {
            let Some(expected) = possessor_of(&analysis.tokens[pronoun]) else {
                continue;
            };
            // A genitive pronoun before a necessive verb is that verb's
            // subject (`teidän täytyy ...`), not a possessor.
            if clause.tokens.clone().any(|index| {
                tokens[index].is_word()
                    && index > pronoun
                    && matches!(
                        analysis.tokens[index].lemma.as_deref(),
                        Some("täytyä" | "pitää" | "kannattaa" | "sopia" | "kuulua")
                    )
            }) {
                continue;
            }
            let Some(head) = following_head(&words[position + 1..], &analysis.tokens) else {
                continue;
            };
            if possessive_state(&analysis.tokens[head]) != Some(None) {
                continue;
            }
            let mut diagnostic = SyntaxDiagnostic::new(
                DiagnosticKind::PossessiveAgreement,
                Severity::Suggestion,
                Confidence::Likely,
                tokens[head].start..tokens[head].end,
                "possessive_missing",
            );
            diagnostic.expected = Some(format!(
                "{} possessive",
                possessive_name(expected)
            ));
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// The person/number a genitive personal pronoun names.
fn possessor_of(token: &TokenSyntax) -> Option<Possessive> {
    if nominal_case(token) != Some(Case::Genitive) {
        return None;
    }
    Some(match token.lemma.as_deref()? {
        "minä" => Possessive::FirstSingular,
        "sinä" => Possessive::SecondSingular,
        "hän" => Possessive::Third,
        "me" => Possessive::FirstPlural,
        "te" => Possessive::SecondPlural,
        "he" => Possessive::Third,
        _ => return None,
    })
}

/// The unique possessive state of a nominal token.
///
/// `Some(Some(p))` is a suffix for `p`, `Some(None)` is no suffix, and `None`
/// means the nominal readings disagree.
fn possessive_state(token: &TokenSyntax) -> Option<Option<Possessive>> {
    let mut states: Vec<Option<Possessive>> = Vec::new();
    for reading in &token.readings {
        if !crate::syntax::is_nominal_pos(reading.part_of_speech) {
            continue;
        }
        states.push(reading.possessive);
    }
    let first = *states.first()?;
    states.iter().all(|state| *state == first).then_some(first)
}

/// The nearest preceding personal pronoun, through adjective/adverb modifiers.
fn preceding_possessor(words: &[usize], out: &[TokenSyntax]) -> Option<usize> {
    for &index in words.iter().rev() {
        if possessor_of(&out[index]).is_some() {
            return Some(index);
        }
        let token = &out[index];
        if matches!(
            nominal_pos(token),
            Some(PartOfSpeech::Adjective | PartOfSpeech::Ordinal | PartOfSpeech::Numeral)
        ) || token
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Adverb))
        {
            continue;
        }
        break;
    }
    None
}

/// The next head noun, through adjective/adverb/numeral modifiers.
fn following_head(words: &[usize], out: &[TokenSyntax]) -> Option<usize> {
    for &index in words {
        let token = &out[index];
        match nominal_pos(token) {
            Some(PartOfSpeech::Noun) => {
                return matches!(
                    token.role,
                    SyntacticRole::Subject
                        | SyntacticRole::Object
                        | SyntacticRole::Complement
                        | SyntacticRole::Unknown
                )
                .then_some(index);
            }
            Some(
                PartOfSpeech::Adjective | PartOfSpeech::Ordinal | PartOfSpeech::Numeral,
            ) => continue,
            _ if token
                .readings
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Adverb)) =>
            {
                continue;
            }
            _ => return None,
        }
    }
    None
}

fn possessive_name(possessive: Possessive) -> &'static str {
    match possessive {
        Possessive::FirstSingular => "first singular",
        Possessive::SecondSingular => "second singular",
        Possessive::Third => "third person",
        Possessive::FirstPlural => "first plural",
        Possessive::SecondPlural => "second plural",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn diagnose(text: &str) -> Vec<SyntaxDiagnostic> {
        let tokens = tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        check(&tokens, &analysis)
    }

    #[test]
    fn a_wrong_suffix_is_a_warning() {
        let diagnostics = diagnose("Minun kirjasi on uusi.");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::PossessiveAgreement);
        assert_eq!(diagnostics[0].message, "possessive_agreement");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
        assert!(diagnostics[0].corrections.is_empty());
    }

    #[test]
    fn a_missing_suffix_is_a_suggestion() {
        let diagnostics = diagnose("Minun kirja on uusi.");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "possessive_missing");
        assert_eq!(diagnostics[0].severity, Severity::Suggestion);
    }

    #[test]
    fn a_matching_suffix_or_a_standalone_suffix_is_quiet() {
        for text in [
            "Minun kirjani on uusi.",
            "Sinun kirjasi on uusi.",
            "Hänen kirjansa on uusi.",
            "Meidän kirjamme on uusi.",
            "Teidän kirjanne on uusi.",
            "Heidän kirjansa on uusi.",
            "Kirjani on uusi.",
            "Minun on mentävä.",
            "Minun uusi kirjani on uusi.",
        ] {
            assert!(diagnose(text).is_empty(), "{text}: {:?}", diagnose(text));
        }
    }
}
