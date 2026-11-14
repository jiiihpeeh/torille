//! Curated phrase and word matching.
//!
//! The rule scans word runs, longest phrase first, and never crosses a
//! sentence boundary. A match covers the whole phrase; after a hit the scan
//! resumes after it, so overlapping table rows cannot double-report.

use crate::sentence;
use crate::syntax_check::{Correction, DiagnosticConfidence};
use crate::tokenise::Token;

use super::{StyleDiagnostic, lexicon};

#[cfg(test)]
pub(super) fn check(tokens: &[Token]) -> Vec<StyleDiagnostic> {
    let contexts = sentence::analyze(tokens);
    check_with_contexts(tokens, &contexts)
}

/// The same scan with sentence context the caller already computed.
pub(super) fn check_with_contexts(
    tokens: &[Token],
    contexts: &[sentence::TokenContext],
) -> Vec<StyleDiagnostic> {
    let words: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.is_word())
        .map(|(index, _)| index)
        .collect();
    // Lowercase each word token once. The scan tries every phrase length at
    // each position, so lowercasing inside that loop repeats the same work.
    let keys: Vec<Option<String>> = tokens.iter().map(Token::lower_word).collect();

    let mut diagnostics = Vec::new();
    let mut position = 0;
    while position < words.len() {
        let first = words[position];
        let first_key = keys[first].as_deref().unwrap_or("");
        let can_start_phrase = lexicon::starts_phrase(first_key);
        let sentence_index = contexts[first].sentence_index;
        let mut matched = None;
        for length in (1..=lexicon::MAX_PHRASE_WORDS).rev() {
            if length > 1 && !can_start_phrase {
                continue;
            }
            let last_position = position + length - 1;
            if last_position >= words.len() {
                continue;
            }
            let last = words[last_position];
            if contexts[last].sentence_index != sentence_index {
                continue;
            }
            let mut key = String::new();
            for (offset, &index) in words[position..=last_position].iter().enumerate() {
                if offset > 0 {
                    key.push(' ');
                }
                key.push_str(keys[index].as_deref().unwrap_or(""));
            }
            let entry = if length == 1 {
                lexicon::word(&key).or_else(|| lexicon::phrase(&key))
            } else {
                lexicon::phrase(&key)
            };
            if let Some(entry) = entry {
                matched = Some((last_position, last, key, entry));
                break;
            }
        }

        if let Some((last_position, last, key, entry)) = matched {
            let span = tokens[first].start..tokens[last].end;
            let mut diagnostic = StyleDiagnostic::new(
                entry.kind,
                DiagnosticConfidence::High,
                span.clone(),
                entry.kind.name(),
            );
            diagnostic.expected = Some(key);
            if let Some(replacement) = entry.replacement {
                let replacement = sentence_case(replacement, contexts[first].sentence_start);
                diagnostic.corrections.push(Correction::replacement(
                    span.start,
                    span.end,
                    replacement,
                ));
            }
            diagnostics.push(diagnostic);
            position = last_position + 1;
        } else {
            position += 1;
        }
    }
    diagnostics
}

/// Uppercase the first character when the match starts a sentence.
///
/// A phrase table stores lowercase surfaces, but a replacement at the start
/// of a sentence must keep the sentence's capital: `Tällä hetkellä` becomes
/// `Nyt`, not `nyt`.
fn sentence_case(replacement: &str, sentence_start: bool) -> String {
    if !sentence_start {
        return replacement.to_string();
    }
    let mut chars = replacement.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style_check::StyleKind;
    use crate::tokenise::tokenise;

    fn diagnostics(text: &str) -> Vec<StyleDiagnostic> {
        check(&tokenise(text))
    }

    fn span_of(text: &str, needle: &str) -> std::ops::Range<usize> {
        let start = text.find(needle).expect("needle in text");
        start..start + needle.len()
    }

    #[test]
    fn a_multi_word_phrase_is_matched_as_one_span() {
        let text = "Tällä hetkellä tilanne on rauhallinen.";
        let found = diagnostics(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, StyleKind::Wordy);
        assert_eq!(found[0].span, span_of(text, "Tällä hetkellä"));
        assert_eq!(found[0].expected.as_deref(), Some("tällä hetkellä"));
        assert_eq!(
            found[0].corrections[0].edits[0].replacement,
            "Nyt".to_string()
        );
    }

    #[test]
    fn phrase_matching_is_case_insensitive() {
        let text = "Kuin myös naapurit tulivat.";
        let found = diagnostics(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, StyleKind::Wordy);
        assert_eq!(found[0].span, span_of(text, "Kuin myös"));
    }

    #[test]
    fn a_label_only_phrase_has_no_correction() {
        let text = "Viime kädessä ratkaisu on sinun.";
        let found = diagnostics(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, StyleKind::Cliche);
        assert!(found[0].corrections.is_empty());
    }

    #[test]
    fn a_single_word_entry_matches_a_standalone_token() {
        let text = "Systeemi toimii.";
        let found = diagnostics(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].kind, StyleKind::Colloquial);
        assert_eq!(found[0].span, span_of(text, "Systeemi"));
        assert_eq!(
            found[0].corrections[0].edits[0].replacement,
            "Järjestelmä".to_string()
        );
    }

    #[test]
    fn a_word_entry_does_not_match_inside_a_longer_word() {
        assert!(diagnostics("Systeeminen virhe.").is_empty());
    }

    #[test]
    fn a_phrase_does_not_cross_a_sentence_boundary() {
        assert!(diagnostics("Tällä. Hetkellä tilanne on rauhallinen.").is_empty());
    }

    #[test]
    fn a_phrase_is_reported_once() {
        let found = diagnostics("Tällä hetkellä hetkellä tilanne on rauhallinen.");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(
            found[0].span,
            span_of(
                "Tällä hetkellä hetkellä tilanne on rauhallinen.",
                "Tällä hetkellä"
            )
        );
    }

    #[test]
    fn a_sentence_initial_replacement_keeps_the_capital() {
        let text = "Tällä hetkellä tilanne on rauhallinen.";
        let found = diagnostics(text);
        assert_eq!(
            found[0].corrections[0].edits[0].replacement,
            "Nyt".to_string()
        );
        let mid = diagnostics("Tilanne on tällä hetkellä rauhallinen.");
        assert_eq!(
            mid[0].corrections[0].edits[0].replacement,
            "nyt".to_string()
        );
    }

    #[test]
    fn clean_prose_is_quiet() {
        for text in [
            "Talo on kaunis.",
            "Kissa istuu pöydällä.",
            "Luen kirjaa.",
            "Hän tuli kotiin.",
        ] {
            assert!(
                diagnostics(text).is_empty(),
                "{text}: {:#?}",
                diagnostics(text)
            );
        }
    }
}
