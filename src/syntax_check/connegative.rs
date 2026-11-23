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
//! The 2sg imperative auxiliary gets the plain 2sg imperative surface
//! (`älä menet` -> `mene`). The plural/third-person imperative connegatives
//! (`älkää menkö`, `älköön menkö`) are `Repaired` forms in the engine and must
//! not be offered as corrections, so those findings stay uncorrected.

use crate::syntax::{RoleConfidence, SyntaxAnalysis, is_verb_reading};
use crate::tags::{Mood, Number, PartOfSpeech, Person, Polarity, Tags, Tense};
use crate::tokenise::Token;

use super::analysis::clause_words;
use super::correction;
use super::diagnostic::{Confidence, DiagnosticKind, Severity, SyntaxDiagnostic};

pub(super) fn check(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic> {
    // Adverbs that can stand between the auxiliary and its main verb:
    // `ei toisin säädetä`, `ei enää tule`. Anything else after the auxiliary
    // ends the search, which keeps `eikä tarpeetonta estettä ...` from
    // reaching a later participle.
    const NEGATION_ADVERBS: &[&str] = &[
        "toisin",
        "enää",
        "vielä",
        "koskaan",
        "ikin",
        "sitten",
        "nyt",
        "aina",
        "usein",
        "myöskään",
        "edes",
        "ollenkaan",
        "lainkaan",
        "mitenkään",
    ];
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
        // The auxiliary and the main verb are usually adjacent (`en mene`,
        // `hän ei menee`), but a closed set of adverbs can intervene
        // (`ei toisin säädetä`, `ei enää tule`). Anything else after the
        // auxiliary ends the search, so `eikä tarpeetonta estettä ...` does
        // not reach a later participle.
        let mut main = None;
        for index in words
            .iter()
            .copied()
            .skip_while(|&index| index != auxiliary)
            .skip(1)
        {
            let token = &analysis.tokens[index];
            if NEGATION_ADVERBS.contains(&token.key.as_str()) {
                continue;
            }
            if token.readings.iter().any(is_verb_reading) {
                main = Some(index);
            }
            break;
        }
        let Some(main) = main else {
            continue;
        };
        if !analysis.tokens[main].readings.iter().any(is_verb_reading) {
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
            is_verb_reading(reading) && reading.mood.is_none() && reading.tense == Some(Tense::Past)
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
            analysis.tokens[main]
                .lemma
                .as_deref()
                .map(|lemma| {
                    correction::imperative_connegative_corrections(
                        lemma,
                        &tokens[main],
                        &tokens[auxiliary],
                        crate::spec::Catalogues::embedded(),
                    )
                })
                .unwrap_or_default()
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
    crate::expand::negation::negation_base(&crate::text::lowercase(&token.text)) == Some("älä")
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
    fn an_imperative_connegative_gets_the_2sg_form() {
        let found = diagnostics("Älä menet.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].corrections[0].edits[0].replacement, "mene");

        // The plural/third-person connegative is a `Repaired` form in the
        // catalogue, so it must stay uncorrected rather than offer `menkö`.
        let found = diagnostics("Älkää menette.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert!(found[0].corrections.is_empty(), "{found:#?}");
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
            assert!(
                diagnostics(text).is_empty(),
                "{text}: {:#?}",
                diagnostics(text)
            );
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
    fn a_passive_connegative_after_an_adverb_is_quiet() {
        for text in [
            "Ei säädetä.",
            "jos tässä laissa ei toisin säädetä.",
        ] {
            assert!(
                diagnostics(text).is_empty(),
                "{text}: {:#?}",
                diagnostics(text)
            );
        }
    }

    #[test]
    fn an_affirmative_passive_after_ei_is_an_error() {
        let found = diagnostics("Ei säädetään.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, DiagnosticKind::Connegative);
    }

    #[test]
    fn an_interjection_and_a_conditional_connegative_are_quiet() {
        for text in [
            "Ei, sillä Jumala hellytti pyövelien sydämet.",
            "ettei häntä pelottaisi",
            "En menne.",
            "En menisi.",
        ] {
            assert!(
                diagnostics(text).is_empty(),
                "{text}: {:#?}",
                diagnostics(text)
            );
        }
    }
}
