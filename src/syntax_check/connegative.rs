//! The connegative main verb after a negative auxiliary.
//!
//! Finnish negation is analytic: `en mene`, `hän ei tule`, `me emme mene`.
//! The main verb carries a negative-polarity reading; an affirmative finite
//! form (`en menen`, `hän ei menee`) is the error this rule finds.
//!
//! The negative past (`en tullut`) and the negative perfect (`en ole tullut`)
//! are also accepted: the past participle has a negative reading. A modal
//! takes its own connegative and an infinitive complement (`en voi mennä`), so
//! only the first verb after the auxiliary is checked.
//!
//! Imperative auxiliaries are detected but currently get no correction: the
//! plural/third-person imperative connegatives (`älkää menkö`, `älköön menkö`)
//! are not the declarative present connegative.

use crate::syntax::{RoleConfidence, SyntaxAnalysis, is_verb_reading};
use crate::tags::{Mood, Number, PartOfSpeech, Person, Polarity, Tags, Tense};
use crate::tokenise::Token;

use super::analysis::clause_words;
use super::correction;
use super::diagnostic::{Confidence, DiagnosticKind, Severity, SyntaxDiagnostic};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    for clause in &analysis.clauses {
        if !clause.negative {
            continue;
        }
        let words = clause_words(clause, tokens);
        let Some(auxiliary) = words.iter().copied().find(|&index| {
            analysis.tokens[index]
                .readings
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
        }) else {
            continue;
        };
        // The auxiliary and the main verb are adjacent in the common negative
        // clause (`en mene`, `hän ei menee`). Requiring adjacency keeps `Ei,
        // sillä ...` and `Eikö hän ...` out: the `Ei` there is an interjection
        // or the subject intervenes, and a wider scan would flag the next
        // affirmative verb in the same clause.
        let Some(main) = words
            .iter()
            .copied()
            .skip_while(|&index| index != auxiliary)
            .nth(1)
        else {
            continue;
        };
        if !analysis.tokens[main]
            .readings
            .iter()
            .any(is_verb_reading)
        {
            continue;
        }
        let token = &analysis.tokens[main];
        if token
            .readings
            .iter()
            .any(|reading| is_verb_reading(reading) && reading.polarity == Polarity::Negative)
        {
            continue;
        }
        // A past participle after the negative auxiliary is the negative past
        // or perfect (`eivät uupuneet`, `en ole tullut`), not an affirmative
        // finite error. Some class engines do not give the plural participle a
        // negative-polarity reading, so accept the form itself.
        if token.readings.iter().any(|reading| {
            is_verb_reading(reading)
                && reading.mood.is_none()
                && reading.tense == Some(Tense::Past)
        }) {
            continue;
        }
        let Some(reading) = affirmative_reading(token) else {
            continue;
        };
        // The conditional connegative is the 3rd-person-singular conditional
        // surface (`en pelottaisi`, `ettei häntä pelottaisi`). The engine tags
        // that surface as an affirmative conditional, not as a negative one,
        // so accept the 3sg form and flag only the other persons. The
        // potential connegative is not tagged reliably either; stay quiet.
        if reading.mood == Some(Mood::Conditional) {
            let third_singular = matches!(reading.person, None | Some(Person::Third))
                && reading.number == Some(Number::Singular);
            if third_singular {
                continue;
            }
        } else if reading.mood == Some(Mood::Potential) {
            continue;
        }
        let confident = token.confidence == RoleConfidence::Likely;
        let (severity, confidence) = if confident {
            (Severity::Error, Confidence::Certain)
        } else {
            (Severity::Warning, Confidence::Likely)
        };
        let corrections = if is_imperative_auxiliary(&tokens[auxiliary]) {
            Vec::new()
        } else {
            analysis.tokens[main]
                .lemma
                .as_deref()
                .map(|lemma| {
                    correction::connegative_corrections(
                        lemma,
                        &tokens[main],
                        reading,
                        crate::spec::Catalogues::embedded(),
                    )
                })
                .unwrap_or_default()
        };
        let mut diagnostic = SyntaxDiagnostic::new(
            DiagnosticKind::Connegative,
            severity,
            confidence,
            tokens[main].start..tokens[main].end,
            "connegative",
        );
        diagnostic.expected = Some("negative main verb form".to_string());
        diagnostic.corrections = corrections;
        diagnostics.push(diagnostic);
    }
    diagnostics
}

/// The first affirmative finite verb reading, preferring the finite moods.
fn affirmative_reading(token: &crate::syntax::TokenSyntax) -> Option<&Tags> {
    let affirmative = token
        .readings
        .iter()
        .filter(|reading| is_verb_reading(reading) && reading.polarity == Polarity::Affirmative);
    affirmative
        .clone()
        .find(|reading| {
            matches!(
                reading.mood,
                Some(Mood::Indicative | Mood::Conditional | Mood::Potential | Mood::Imperative)
            )
        })
        .or_else(|| affirmative.clone().next())
}

fn is_imperative_auxiliary(token: &Token) -> bool {
    crate::expand::negation::negation_base(&token.text.to_lowercase()) == Some("älä")
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
    fn an_affirmative_main_verb_after_ei_is_an_error() {
        let found = diagnostics("En menen.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, DiagnosticKind::Connegative);
        assert_eq!(found[0].corrections[0].edits[0].replacement, "mene");
    }

    #[test]
    fn a_connegative_main_verb_is_quiet() {
        for text in [
            "En mene.",
            "Hän ei mene.",
            "En ole.",
            "En ole tullut.",
            "En mennyt.",
            "En tullut.",
            "En voi mennä.",
            "Älä mene.",
            "Älkää menkö.",
        ] {
            assert!(diagnostics(text).is_empty(), "{text}: {:#?}", diagnostics(text));
        }
    }

    #[test]
    fn a_past_affirmative_verb_takes_the_past_participle() {
        let found = diagnostics("En menin.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].corrections[0].edits[0].replacement, "mennyt");
    }

    #[test]
    fn a_negative_past_participle_is_quiet() {
        assert!(diagnostics("He eivät uupuneet.").is_empty());
        assert!(diagnostics("En tullut.").is_empty());
    }

    #[test]
    fn an_interjection_and_a_conditional_connegative_are_quiet() {
        for text in [
            "Ei, sillä Jumala hellytti pyövelien sydämet.",
            "ettei häntä pelottaisi",
        ] {
            assert!(diagnostics(text).is_empty(), "{text}: {:#?}", diagnostics(text));
        }
    }
}
