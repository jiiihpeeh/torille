//! Sentence-local content-word repetition.
//!
//! The rule is deliberately local: a word repeated in the next sentence is
//! normal cohesion, not an echo. Only a repeat with at most ten word tokens
//! between the two occurrences is reported, and only once per lemma per
//! sentence.

use std::ops::Range;

use crate::sentence;
use crate::syntax::{SyntaxAnalysis, TokenSyntax};
use crate::tags::PartOfSpeech;
use crate::tokenise::Token;

use super::{StyleDiagnostic, StyleKind};

/// Word tokens allowed between the two occurrences.
const REPETITION_WINDOW: usize = 10;

/// Lemmas that are too grammatical to count as an echo.
const LIGHT_LEMMAS: &[&str] = &["olla", "tulla", "voida", "tehdä"];

pub(super) fn check(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
    covered: &[Range<usize>],
) -> Vec<StyleDiagnostic> {
    let contexts = sentence::analyze(tokens);
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
            &analysis.tokens[index],
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
        {
            let mut diagnostic = StyleDiagnostic::new(
                StyleKind::Repetition,
                crate::syntax_check::DiagnosticConfidence::Medium,
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

/// The lemma a repetition rule may use, or `None` when the token is not a
/// content word.
fn content_lemma(token: &Token, syntax: &TokenSyntax, sentence_start: bool) -> Option<String> {
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
    let is_content = syntax.readings.iter().any(|reading| {
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
    let lemma = syntax
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
        let tokens = crate::tokenise::tokenise(text);
        let analysis = crate::syntax::analyze(&tokens);
        for (i, token) in tokens.iter().enumerate() {
            eprintln!(
                "{i} {:?} word={} lemma={:?}",
                token.text,
                token.is_word(),
                analysis.tokens[i].lemma
            );
        }
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

    #[test]
    fn the_token_stream_path_agrees_with_the_text_path() {
        let text = "Tässä lauseessa asia toistuu ja asia jää mieleen.";
        let tokens = tokenise(text);
        assert_eq!(style_check::check(text), style_check::check_tokens(&tokens));
    }
}
