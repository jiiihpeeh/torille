//! Lightweight sentence boundaries for the typo/case layer.
//!
//! This is not a parser. It turns a token stream into sentence context:
//! which sentence each token belongs to, whether a word starts a sentence,
//! whether it starts a paragraph/line block, and the active quote depth.
//!
//! The rules are the ones the case layer needs: terminal punctuation,
//! line breaks, colons and quotes. The syntax layer (`src/syntax.rs`,
//! planned) consumes these spans; the hot render path only needs
//! [`analyze`].
//!
//! Byte offsets are source-text offsets. A sentence span starts at the
//! previous boundary and ends at the next one, so the spans partition the
//! text exactly once.

use crate::tokenise::{Token, TokenKind};

/// Closed list of Finnish abbreviations whose final period is not a
/// sentence boundary. Matched without the period and case-insensitively:
/// the tokeniser keeps `esim` and `.` in separate runs.
const ABBREVIATIONS: &[&str] = &[
    "esim", "ym", "yms", "jne", "jms", "mm", "ns", "n", "s", "v", "t", "klo", "nro", "prof", "tri",
    "toht", "hra", "rva", "nr", "siv", "vrt", "ts",
];

/// Per-token sentence context.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenContext {
    /// 0-based sentence number in this text.
    pub sentence_index: usize,
    /// True for the first word token of a sentence.
    pub sentence_start: bool,
    /// True for the first sentence after a line break (or the first
    /// sentence of the text).
    pub paragraph_start: bool,
    /// Active quotation depth at this token. A quote before the word opens
    /// the depth; a closing quote after it leaves it.
    pub quote_depth: u16,
}

/// A sentence's byte span in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentenceSpan {
    /// Start byte. The first span starts at 0; every other span starts at
    /// the first word of the sentence, so leading separators belong to the
    /// preceding span.
    pub start: usize,
    /// Exclusive end byte. The last span ends at the end of the text.
    pub end: usize,
    /// 0-based sentence number.
    pub index: usize,
    /// True when this is the first sentence after a line break.
    pub paragraph_start: bool,
}

/// Determine which separator tokens close a sentence.
///
/// This is the single boundary predicate behind [`Analyzer`]. It is
/// public so callers that already hold tokens and future syntax code do
/// not duplicate the rules.
pub fn separator_ends_sentence(token: &Token) -> bool {
    if !matches!(token.kind, TokenKind::Separator) {
        return false;
    }
    if token.text.contains('\n') {
        return true;
    }
    let trimmed = token.text.trim();
    if trimmed.is_empty() {
        return false;
    }
    // A closing quote is a boundary on its own, and so is a separator that
    // ends with one (`?»`, `!»`).
    let last = trimmed.chars().last();
    if matches!(last, Some('»' | '«' | '"' | '“' | '”')) {
        return true;
    }
    // Dialogue dashes may follow the terminal punctuation (`.--Kuulkaa`)
    // or stand alone (`-- Kuulkaa`). Strip them, then look for the real
    // terminal mark. A run of only dashes is a dialogue turn of its own.
    let core = trimmed.trim_end_matches(['-', '–', '—']);
    if core.is_empty() {
        return trimmed.chars().any(|c| ['-', '–', '—'].contains(&c));
    }
    matches!(core.chars().last(), Some('.') | Some('!') | Some('?') | Some(':'))
}

/// Whether a separator contains a newline. A newline is a paragraph/line
/// boundary even when the line did not end with punctuation.
pub fn separator_breaks_line(token: &Token) -> bool {
    matches!(token.kind, TokenKind::Separator) && token.text.contains('\n')
}

/// Stateful sentence analyzer.
///
/// Feed tokens in order with [`Analyzer::push`]; the state (sentence
/// number, line/paragraph start, quote depth, previous word) survives
/// across calls. This is what lets the wasm proper-name learner walk a
/// text in bounded windows without treating every window start as a new
/// sentence.
#[derive(Debug, Clone)]
pub struct Analyzer {
    sentence_index: usize,
    sentence_start: bool,
    paragraph_start: bool,
    quote_depth: u16,
    saw_word: bool,
    previous_word: Option<String>,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    /// A fresh analyzer at the start of a document.
    pub fn new() -> Self {
        Self {
            sentence_index: 0,
            sentence_start: true,
            paragraph_start: true,
            quote_depth: 0,
            saw_word: false,
            previous_word: None,
        }
    }

    /// Consume one token and return its sentence context.
    pub fn push(&mut self, token: &Token) -> TokenContext {
        if token.is_word() {
            if self.sentence_start && self.saw_word {
                self.sentence_index += 1;
            }
            let context = TokenContext {
                sentence_index: self.sentence_index,
                sentence_start: self.sentence_start,
                paragraph_start: self.paragraph_start,
                quote_depth: self.quote_depth,
            };
            self.saw_word = true;
            self.sentence_start = false;
            self.paragraph_start = false;
            self.previous_word = Some(token.text.clone());
            return context;
        }

        // A separator can both close a sentence and open a quote. Process
        // the quote first so a word after `»` sees the new depth.
        self.quote_depth = update_quote_depth(self.quote_depth, &token.text);
        if separator_breaks_line(token) {
            self.sentence_start = true;
            self.paragraph_start = true;
        } else if separator_ends_sentence(token)
            && !is_abbreviation_boundary(token, self.previous_word.as_deref())
        {
            self.sentence_start = true;
            self.paragraph_start = false;
        }
        TokenContext {
            sentence_index: self.sentence_index,
            sentence_start: false,
            paragraph_start: false,
            quote_depth: self.quote_depth,
        }
    }
}

/// One context per token, in token order. See [`Analyzer`] for the
/// streaming form.
pub fn analyze(tokens: &[Token]) -> Vec<TokenContext> {
    let mut analyzer = Analyzer::new();
    tokens.iter().map(|token| analyzer.push(token)).collect()
}

/// Sentence spans derived from [`analyze`]. The spans partition
/// `[0, tokens.last().end]`: the first starts at 0, the last ends at the
/// end of the text, and every boundary is the first word of the next
/// sentence.
pub fn spans(tokens: &[Token]) -> Vec<SentenceSpan> {
    let contexts = analyze(tokens);
    let Some(text_end) = tokens.last().map(|t| t.end) else {
        return Vec::new();
    };

    let mut out: Vec<SentenceSpan> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if !token.is_word() || !contexts[i].sentence_start {
            continue;
        }
        if out.is_empty() {
            out.push(SentenceSpan {
                start: 0,
                end: text_end,
                index: contexts[i].sentence_index,
                paragraph_start: contexts[i].paragraph_start,
            });
        } else {
            let previous_end = token.start;
            if let Some(previous) = out.last_mut() {
                previous.end = previous_end;
            }
            out.push(SentenceSpan {
                start: previous_end,
                end: text_end,
                index: contexts[i].sentence_index,
                paragraph_start: contexts[i].paragraph_start,
            });
        }
    }
    out
}

/// Is this period the end of an abbreviation or a single initial, rather
/// than the end of a sentence?
///
/// The check is deliberately narrow: only a separator whose trimmed text is
/// exactly `.`, after a known abbreviation (`esim.`, `ym.`) or a one-letter
/// initial (`J. Virtanen`). Ellipses and multi-dot separators are not
/// abbreviations. A single initial is always treated as non-terminal: the
/// analyzer is streaming and does not look ahead, and this only relaxes
/// capitalization, never makes a word unrecognized.
fn is_abbreviation_boundary(token: &Token, previous_word: Option<&str>) -> bool {
    if !matches!(token.kind, TokenKind::Separator) || token.text.trim() != "." {
        return false;
    }
    let Some(previous) = previous_word else {
        return false;
    };
    let lower = previous.to_lowercase();
    if ABBREVIATIONS.contains(&lower.as_str()) {
        return true;
    }
    let mut chars = previous.chars();
    let (Some(first), None) = (chars.next(), chars.next()) else {
        return false;
    };
    first.is_uppercase()
}

/// Advance the quotation depth over a separator run.
///
/// `»` and `"` toggle, because both are used as opening and closing marks
/// in the corpora. The typographic pairs `“…”` and `‘…’` are directional.
/// This is a context hint, not a parser; case validation only needs to
/// know that a quote has opened.
fn update_quote_depth(mut depth: u16, text: &str) -> u16 {
    for c in text.chars() {
        match c {
            '»' | '"' => {
                if depth == 0 {
                    depth = 1;
                } else {
                    depth -= 1;
                }
            }
            '«' => depth = depth.saturating_sub(1),
            '“' | '‘' => depth = depth.saturating_add(1),
            '”' | '’' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenise::tokenise;

    fn words(text: &str) -> Vec<String> {
        tokenise(text)
            .into_iter()
            .filter(|t| t.is_word())
            .map(|t| t.text)
            .collect()
    }

    #[test]
    fn first_word_is_sentence_initial() {
        let tokens = tokenise("Talo on kaunis.");
        let ctx = analyze(&tokens);
        let first = tokens.iter().position(Token::is_word).unwrap();
        assert!(ctx[first].sentence_start);
        assert!(ctx[first].paragraph_start);
    }

    #[test]
    fn a_period_starts_the_next_sentence() {
        let tokens = tokenise("Talo on kaunis. Kissa istuu.");
        let ctx = analyze(&tokens);
        let kissa = tokens.iter().position(|t| t.text == "Kissa").unwrap();
        assert!(ctx[kissa].sentence_start);
        assert!(!ctx[kissa].paragraph_start);
        assert_eq!(ctx[kissa].sentence_index, 1);
    }

    #[test]
    fn a_newline_starts_a_new_sentence_and_paragraph() {
        let tokens = tokenise("Pieniä eläjiä olivat.\nElä ole milläsikään.");
        let ctx = analyze(&tokens);
        let ela = tokens.iter().position(|t| t.text == "Elä").unwrap();
        assert!(ctx[ela].sentence_start);
        assert!(ctx[ela].paragraph_start);
        assert_eq!(ctx[ela].sentence_index, 1);
    }

    #[test]
    fn a_colon_or_opening_quote_starts_a_sentence() {
        for text in ["Hän sanoi: Elä tule.", "Hän sanoi: »Elä tule."] {
            let tokens = tokenise(text);
            let ctx = analyze(&tokens);
            let ela = tokens.iter().position(|t| t.text == "Elä").unwrap();
            assert!(ctx[ela].sentence_start, "{text}");
        }
    }

    #[test]
    fn an_abbreviation_period_is_not_a_sentence_boundary() {
        let tokens = tokenise("Hän tuli esim. eilen.");
        let ctx = analyze(&tokens);
        let eilen = tokens.iter().position(|t| t.text == "eilen").unwrap();
        assert!(!ctx[eilen].sentence_start);
        assert_eq!(ctx[eilen].sentence_index, 0);
    }

    #[test]
    fn a_single_initial_does_not_start_a_sentence() {
        let tokens = tokenise("J. Virtanen tuli.");
        let ctx = analyze(&tokens);
        let virtanen = tokens.iter().position(|t| t.text == "Virtanen").unwrap();
        assert!(!ctx[virtanen].sentence_start);
        assert_eq!(ctx[virtanen].sentence_index, 0);
    }

    #[test]
    fn an_ellipsis_is_still_a_sentence_boundary() {
        let tokens = tokenise("Hän tuli... Eilen.");
        let ctx = analyze(&tokens);
        let eilen = tokens.iter().position(|t| t.text == "Eilen").unwrap();
        assert!(ctx[eilen].sentence_start);
    }

    #[test]
    fn a_comma_does_not_start_a_sentence() {
        let tokens = tokenise("Helsinki, kaupunki");
        let ctx = analyze(&tokens);
        let kaupunki = tokens.iter().position(|t| t.text == "kaupunki").unwrap();
        assert!(!ctx[kaupunki].sentence_start);
    }

    #[test]
    fn spans_partition_the_text_exactly_once() {
        let text = "Pieniä eläjiä olivat.\nElä ole milläsikään. Aivan niin.";
        let tokens = tokenise(text);
        let spans = spans(&tokens);
        assert_eq!(spans.first().unwrap().start, 0);
        assert_eq!(spans.last().unwrap().end, text.len());
        for pair in spans.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    #[test]
    fn spans_do_not_split_words() {
        let text = "Talo on kaunis. Kissa istuu.";
        let tokens = tokenise(text);
        let spans = spans(&tokens);
        for token in tokens.iter().filter(|t| t.is_word()) {
            let containing = spans
                .iter()
                .filter(|span| token.start >= span.start && token.start < span.end)
                .count();
            assert_eq!(
                containing, 1,
                "word {:?} at {} belongs to {containing} spans",
                token.text, token.start
            );
        }
    }

    #[test]
    fn empty_and_single_word() {
        assert!(analyze(&tokenise("")).is_empty());
        assert!(spans(&tokenise("")).is_empty());
        let tokens = tokenise("talo");
        let ctx = analyze(&tokens);
        assert_eq!(ctx.len(), 1);
        assert!(ctx[0].sentence_start);
        assert_eq!(spans(&tokens).len(), 1);
    }

    #[test]
    fn quote_depth_opens_and_closes() {
        let tokens = tokenise("Hän sanoi: »Elä tule.»");
        let ctx = analyze(&tokens);
        let ela = tokens.iter().position(|t| t.text == "Elä").unwrap();
        assert_eq!(ctx[ela].quote_depth, 1);
    }

    #[test]
    fn separator_predicate_recognises_terminal_runs() {
        let tokens = tokenise("eka. toka");
        let dot = tokens
            .iter()
            .find(|t| !t.is_word())
            .expect("separator after eka");
        assert!(separator_ends_sentence(dot));

        let tokens = tokenise("eka.\n\ntoka");
        let line = tokens.iter().find(|t| !t.is_word()).unwrap();
        assert!(separator_ends_sentence(line));
        assert!(separator_breaks_line(line));
    }

    #[test]
    fn separator_predicate_rejects_commas() {
        let tokens = tokenise("eka, toka");
        let comma = tokens.iter().find(|t| !t.is_word()).unwrap();
        assert!(!separator_ends_sentence(comma));
        assert!(!separator_breaks_line(comma));
    }

    /// A dialogue dash may follow the period (`.--Kuulkaa`) or stand alone
    /// (`-- Kuulkaa`). Both open a new utterance, so the capital is
    /// sentence-initial, not a mid-sentence case error.
    #[test]
    fn dialogue_dashes_open_a_sentence() {
        for text in [
            "»Ei todellakaan hätä ole tämän näköinen.--Kuulkaa, istukaa.",
            "joka olet mainio kirjamies.--Hyvästi vain.»",
            "-- Kuulkaa nyt.",
            "»Niin.--Kuulkaa, Nikkilä.»",
        ] {
            let tokens = tokenise(text);
            let ctx = analyze(&tokens);
            let first_after_dash = tokens
                .iter()
                .zip(&ctx)
                .find(|(token, _)| token.text == "Kuulkaa" || token.text == "Hyvästi")
                .unwrap_or_else(|| panic!("no test word in {text:?}"));
            assert!(
                first_after_dash.1.sentence_start,
                "{:?} should start a sentence in {text:?}",
                first_after_dash.0.text
            );
        }
    }

    #[test]
    fn words_are_preserved() {
        assert_eq!(
            words("Talo on kaunis. Kissa istuu."),
            vec!["Talo", "on", "kaunis", "Kissa", "istuu"]
        );
    }
}
