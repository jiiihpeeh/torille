//! Sentence-local content-word repetition.
//!
//! The rule is deliberately local: a word repeated in the next sentence is
//! normal cohesion, not an echo. Only a repeat with at most ten word tokens
//! between the two occurrences is reported, and only once per lemma per
//! sentence.

use std::ops::Range;

use crate::diagnostic::{Confidence, Correction, CorrectionLicense, Edit};
use crate::morphology::{MorphologyAnalysis, WordInfo};
use crate::sentence;
use crate::tags::PartOfSpeech;
use crate::tokenise::Token;

use super::{StyleDiagnostic, StyleKind};

/// Word tokens allowed between the two occurrences.
const REPETITION_WINDOW: usize = 10;

/// Lemmas that are too grammatical to count as an echo.
const LIGHT_LEMMAS: &[&str] = &["olla", "tulla", "voida", "tehdä"];

/// Established rhetorical reduplications that are not transcription slips.
const RHETORICAL_REDUPLICATION: &[&str] = &[
    "hyvin",
    "kauan",
    "pian",
    "niin",
    "joo",
    "ei",
    "kyllä",
    "kovin",
    "hitaasti",
    "nopeasti",
    "enemmän",
    "vähemmän",
    "lähemmäs",
    "kauemmas",
    "useammin",
    "harvemmin",
    "vähän",
    "paljon",
];

pub(super) fn check(
    tokens: &[Token],
    morphology: &MorphologyAnalysis,
    covered: &[Range<usize>],
    contexts: &[sentence::TokenContext],
) -> Vec<StyleDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut sentence_index = 0;
    let mut word_position: usize = 0;
    let mut last_seen: Vec<(String, usize)> = Vec::new();
    let mut reported: Vec<String> = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let current_sentence = contexts[index].sentence_index;
        if current_sentence != sentence_index {
            sentence_index = current_sentence;
            word_position = 0;
            last_seen.clear();
            reported.clear();
        }
        let position = word_position;
        word_position += 1;

        let Some(lemma) = content_lemma(
            token,
            morphology.word(index),
            contexts[index].sentence_start,
        ) else {
            continue;
        };
        if covered
            .iter()
            .any(|span| span.start <= token.start && token.end <= span.end)
        {
            continue;
        }

        if let Some((_, previous)) = last_seen.iter().find(|(seen, _)| *seen == lemma)
            && position.saturating_sub(*previous) <= REPETITION_WINDOW + 1
            && !reported.contains(&lemma)
            && !(position.saturating_sub(*previous) == 1
                && RHETORICAL_REDUPLICATION.contains(&lemma.as_str()))
        {
            let mut diagnostic = StyleDiagnostic::new(
                StyleKind::Repetition,
                crate::syntax_check::Confidence::Likely,
                token.start..token.end,
                StyleKind::Repetition.name(),
            );
            diagnostic.expected = Some(lemma.clone());
            diagnostics.push(diagnostic);
            reported.push(lemma.clone());
        }

        if let Some((_, previous)) = last_seen.iter_mut().find(|(seen, _)| *seen == lemma) {
            *previous = position;
        } else {
            last_seen.push((lemma, position));
        }
    }

    diagnostics
}

/// Adjacent identical surfaces with no punctuation between them.
///
/// This is the transcription-slip / stutter case (`että että`), not the
/// near-repeat `style_repetition`. A comma, dash, line-break punctuation or
/// any other separator breaks the pair, and the rhetorical list keeps
/// `hyvin hyvin`, `kauan kauan` and friends out.
pub(super) fn duplicate_words(
    tokens: &[Token],
    covered: &[Range<usize>],
) -> Vec<StyleDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut previous: Option<usize> = None;
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        if let Some(first) = previous {
            let surface = token.text.to_lowercase();
            let in_phrase = covered.iter().any(|span| {
                (span.start <= tokens[first].start && tokens[first].end <= span.end)
                    || (span.start <= token.start && token.end <= span.end)
            });
            if tokens[first].text.to_lowercase() == surface
                && tokens[first + 1..index]
                    .iter()
                    .all(|between| !between.is_word() && between.text.trim().is_empty())
                && !in_phrase
                && !RHETORICAL_REDUPLICATION.contains(&surface.as_str())
            {
                let mut diagnostic = StyleDiagnostic::new(
                    StyleKind::DuplicateWord,
                    Confidence::Likely,
                    token.start..token.end,
                    StyleKind::DuplicateWord.name(),
                );
                diagnostic.expected = Some(surface);
                // Remove the separator whitespace with the repeated word so
                // the result does not leave a double space behind.
                diagnostic.corrections = vec![Correction::new(
                    vec![Edit::new(tokens[first].end, token.end, "")],
                    0.0,
                    "style_duplicate_word",
                )
                .licensed_by(CorrectionLicense::Curated)
                .with_confidence(Confidence::Likely)];
                diagnostics.push(diagnostic);
            }
        }
        previous = Some(index);
    }
    diagnostics
}

/// The lemma a repetition rule may use, or `None` when the token is not a
/// content word.
fn content_lemma(token: &Token, info: Option<&WordInfo>, sentence_start: bool) -> Option<String> {
    if token.text.chars().count() < 4 {
        return None;
    }
    // A capitalised name is normal in a text; repeating it is not an echo.
    // A sentence-initial capital is not a name signal, and `Asia` is both a
    // name and the common noun, so a lowercased sanalista row wins.
    if token.is_capitalised() && !sentence_start {
        let lower = token.text.to_lowercase();
        let common = crate::kotus::sanalista().by_word(&lower).next().is_some();
        if !common && crate::expand::proper_names::lemma(&token.text).is_some() {
            return None;
        }
    }
    let info = info?;
    let is_content = info.readings.iter().any(|reading| {
        matches!(
            reading.part_of_speech,
            Some(
                PartOfSpeech::Noun
                    | PartOfSpeech::Adjective
                    | PartOfSpeech::Verb
                    | PartOfSpeech::Adverb
            )
        )
    });
    if !is_content {
        return None;
    }
    let lemma = info
        .lemma
        .clone()
        .unwrap_or_else(|| token.text.to_lowercase());
    if LIGHT_LEMMAS.contains(&lemma.as_str()) {
        return None;
    }
    Some(lemma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style_check;
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<StyleDiagnostic> {
        style_check::check(text).diagnostics
    }

    fn repetition(text: &str) -> Vec<StyleDiagnostic> {
        diagnostics(text)
            .into_iter()
            .filter(|diagnostic| diagnostic.kind == StyleKind::Repetition)
            .collect()
    }

    #[test]
    fn a_near_repeat_is_reported_on_the_second_occurrence() {
        let text = "Tässä lauseessa asia toistuu ja asia jää mieleen.";
        let found = repetition(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        let second = text.match_indices("asia").nth(1).unwrap().0;
        assert_eq!(found[0].span, second..second + "asia".len());
        assert_eq!(found[0].expected.as_deref(), Some("asia"));
    }

    #[test]
    fn a_cross_sentence_repeat_is_not_an_echo() {
        assert!(repetition("Asia on hyvä. Asia on tärkeä.").is_empty());
    }

    #[test]
    fn ten_words_between_the_repeats_is_still_a_finding() {
        let text = "Asia ja ja ja ja ja ja ja ja ja ja asia.";
        assert_eq!(repetition(text).len(), 1);
    }

    #[test]
    fn eleven_words_between_the_repeats_is_quiet() {
        let text = "Asia ja ja ja ja ja ja ja ja ja ja ja asia.";
        assert!(repetition(text).is_empty());
    }

    #[test]
    fn a_light_verb_is_not_an_echo() {
        assert!(repetition("Olemme täällä ja olemme valmiita.").is_empty());
    }

    #[test]
    fn a_word_shorter_than_four_characters_is_not_an_echo() {
        assert!(repetition("Talo on ja on.").is_empty());
    }

    #[test]
    fn a_repeated_lemma_is_reported_once_per_sentence() {
        let found = repetition("Asia on asia ja asia on asia.");
        assert_eq!(found.len(), 1, "{found:#?}");
    }

    #[test]
    fn a_phrase_match_suppresses_a_repeat_inside_it() {
        let found = diagnostics("Tällä hetkellä hetkellä tilanne on rauhallinen.");
        assert!(
            found.iter().all(|d| d.kind == StyleKind::Wordy),
            "{found:#?}"
        );
    }

    fn duplicates(text: &str) -> Vec<StyleDiagnostic> {
        diagnostics(text)
            .into_iter()
            .filter(|diagnostic| diagnostic.kind == StyleKind::DuplicateWord)
            .collect()
    }

    #[test]
    fn an_adjacent_duplicate_is_a_single_suggestion() {
        let text = "Hän sanoi, että että tämä oli väärin.";
        let found = duplicates(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        let second = text.match_indices("että").nth(1).unwrap().0;
        assert_eq!(found[0].span, second..second + "että".len());
        assert_eq!(found[0].severity, crate::syntax_check::Severity::Suggestion);
        assert_eq!(found[0].corrections[0].edits[0].replacement, "");
    }

    #[test]
    fn punctuation_or_rhetorical_reduplication_is_quiet() {
        for text in [
            "Se oli hyvin, hyvin vaikeaa.",
            "Ei, ei se ollut niin.",
            "Se oli hyvin hyvin vaikeaa.",
            "Hän sanoi, että, että tämä oli väärin.",
        ] {
            assert!(duplicates(text).is_empty(), "{text}: {:?}", duplicates(text));
        }
    }

    #[test]
    fn a_duplicate_word_takes_precedence_over_a_near_repeat() {
        let found = diagnostics("Talo talo on suuri.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, StyleKind::DuplicateWord);
    }

    #[test]
    fn the_token_stream_path_agrees_with_the_text_path() {
        let text = "Tässä lauseessa asia toistuu ja asia jää mieleen.";
        let tokens = tokenise(text);
        assert_eq!(style_check::check(text), style_check::check_tokens(&tokens));
    }
}
