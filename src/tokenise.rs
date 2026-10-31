//! Tokeniser for the wasm-side per-textbox API.
//!
//! Splits text into alternating word and separator runs. For each
//! word run we record the original byte range in the input, the
//! literal text, whether the first character is upper-case, and
//! (via the [`annotate_sentence_initial`] helper) whether the run
//! is sentence-initial.
//!
//! "Word" means a maximal run of Unicode letters and digits with single
//! internal hyphens, so split compounds (`nuori-isäntä`, `linja-auto`,
//! `itä-länsi-suhteet`, `20-vuotias`) stay inside one token for the wasm
//! caller. A pure number is a separator. Anything else is a separator. The
//! boundary semantics match the JS tokeniser the
//! demo and the extensions used to use, so the wasm-side output
//! has the same token granularity the JS caller was producing.
//!
//! Keeping the tokeniser in Rust means one string-to-wasm copy for
//! the whole text instead of N copies for N tokens. The render
//! path of a 100-word textarea goes from N wasm-boundary crossings
//! (each ~120µs of `wasm-bindgen` overhead) to a single crossing.
//!
//! The whole tokenizer is allocation-light: the output `Vec<Token>`
//! is the dominant cost; the helpers below do not allocate.

/// What kind of run a token represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// A run of Unicode letters/digits with single internal hyphens.
    Word,
    /// A run of non-letter characters (whitespace, punctuation,
    /// digits, etc.). May contain multiple sentences-worth of
    /// separators if the source has e.g. `"...\n\n"`.
    Separator,
}

/// A single run in the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The literal text of the run, exactly as it appears in
    /// the input (case, hyphens, and all).
    pub text: String,
    /// Byte offset of the first character of the run.
    pub start: usize,
    /// Byte offset just past the run (exclusive).
    pub end: usize,
    pub kind: TokenKind,
}

impl Token {
    /// First `char` of the run, or `None` for an empty run. Always
    /// `None` for an empty separator (which the tokeniser can
    /// produce when the input has only word characters — but the
    /// tokeniser won't emit such runs in practice).
    pub fn first_char(&self) -> Option<char> {
        self.text.chars().next()
    }

    /// `true` for a word run whose first character is upper-case
    /// (any Unicode upper-case char, including Finnish `Ä`, `Ö`,
    /// `Å`). The wasm side uses this to detect proper-noun
    /// candidates the user wants the spell-checker to learn.
    pub fn is_capitalised(&self) -> bool {
        matches!(self.kind, TokenKind::Word) && self.first_char().is_some_and(|c| c.is_uppercase())
    }

    /// `true` if this is a word run in the tokeniser's sense.
    pub fn is_word(&self) -> bool {
        matches!(self.kind, TokenKind::Word)
    }

    /// `true` if this is a separator run that ends a sentence or opens a
    /// new one: a final `.`/`!`/`?`/`:`, an opening/closing quote mark, or
    /// any line break. The per-textbox state uses this for sentence-initial
    /// detection -- a more sophisticated sentence splitter would be slow
    /// here and the engine does not need it. A newline counts because
    /// Gutenberg verse and dialogue lines start with a capital without a
    /// full stop.
    pub fn ends_sentence(&self) -> bool {
        if !matches!(self.kind, TokenKind::Separator) {
            return false;
        }
        if self.text.contains('\n') {
            return true;
        }
        let last = self.text.chars().rev().find(|c| !c.is_whitespace());
        matches!(
            last,
            Some('.')
                | Some('!')
                | Some('?')
                | Some(':')
                | Some('»')
                | Some('«')
                | Some('"')
                | Some('“')
                | Some('”')
        )
    }

    /// Lower-cased word text, or `None` for separators. The wasm
    /// side uses the lower-cased form for every lexicon probe.
    pub fn lower_word(&self) -> Option<String> {
        if !self.is_word() {
            return None;
        }
        Some(self.text.to_lowercase())
    }
}

/// Split `text` into word/separator runs.
///
/// A word run is a run of letters, digits, and **single** internal
/// hyphens. This keeps `nuori-isäntä`, `linja-auto`, `itä-länsi-suhteet`
/// and `20-vuotias` whole while breaking interrupted-speech markers
/// like `niin--?` and `noin--lähemmin` at the first hyphen, producing
/// `[niin, --, ?]` and `[noin, --, lähemmin]`. A hyphen is part of the
/// word only when both neighbours are word characters and the next
/// character is not another hyphen, so a doubled hyphen is punctuation.
///
/// Digits are word characters only when the run also contains a letter;
/// a pure number (`123`, `3.5`) is left as a separator so it is never
/// spell-checked. Hyphenated number compounds (`20-vuotias`) are one
/// word.
pub fn tokenise(text: &str) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let mut run_start: Option<usize> = None;
    let mut run_kind: Option<TokenKind> = None;

    // Build a `char_indices` view that also exposes the previous and next
    // characters: a hyphen is part of a word only when it sits between two
    // word characters and is not the first of a doubled hyphen.
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for i in 0..chars.len() {
        let (offset, ch) = chars[i];
        let prev_ch = i.checked_sub(1).map(|j| chars[j].1);
        let next_ch = chars.get(i + 1).map(|(_, c)| *c);
        let is_word_char = ch.is_alphabetic() || ch.is_numeric();
        let is_word_hyphen = ch == '-'
            && run_kind == Some(TokenKind::Word)
            && prev_ch.is_some_and(|c| c.is_alphabetic() || c.is_numeric())
            && next_ch.is_some_and(|c| c.is_alphabetic() || c.is_numeric())
            && next_ch != Some('-');
        let char_kind = if is_word_char || is_word_hyphen {
            TokenKind::Word
        } else {
            TokenKind::Separator
        };

        let same_kind = matches!(run_kind, Some(k) if k == char_kind);
        if !same_kind {
            // Kind change — flush the previous run, start a new one.
            if let (Some(s), Some(k)) = (run_start, run_kind) {
                out.push(Token {
                    text: text[s..offset].to_string(),
                    start: s,
                    end: offset,
                    kind: k,
                });
            }
            run_start = Some(offset);
            run_kind = Some(char_kind);
        }
    }

    if let (Some(s), Some(k)) = (run_start, run_kind) {
        out.push(Token {
            text: text[s..text.len()].to_string(),
            start: s,
            end: text.len(),
            kind: k,
        });
    }

    // A word run with no letters is a number or other non-word text. Demote
    // it to a separator, and merge adjacent separators so "123" and "3.5"
    // stay one gap rather than becoming several.
    let mut merged: Vec<Token> = Vec::with_capacity(out.len());
    for mut token in out {
        if token.kind == TokenKind::Word && !token.text.chars().any(char::is_alphabetic) {
            token.kind = TokenKind::Separator;
        }
        if let Some(last) = merged.last_mut()
            && last.kind == token.kind
        {
            last.text.push_str(&token.text);
            last.end = token.end;
            continue;
        }
        merged.push(token);
    }
    merged
}

/// One `bool` per token, `true` when the token is a word at the
/// start of a sentence. A token is sentence-initial if it is the
/// first word of the input, or if the most recent preceding
/// separator run ends a sentence (`ends_sentence` is `true`).
///
/// Used by the wasm side to filter proper-noun candidates: every
/// sentence in Finnish starts with a capital, so a sentence-
/// initial capital is ambiguous and the conservative rule is to
/// only learn capitalised words that appear 2+ times in the text.
pub fn annotate_sentence_initial(tokens: &[Token]) -> Vec<bool> {
    let mut out = vec![false; tokens.len()];
    let mut sentence_initial = true;
    for (i, tok) in tokens.iter().enumerate() {
        if tok.is_word() {
            out[i] = sentence_initial;
            // Any word ends the "start of sentence" state, regardless
            // of whether the sentence continues after it.
            sentence_initial = false;
        } else if tok.ends_sentence() {
            sentence_initial = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        tokenise(text)
            .into_iter()
            .filter(|t| t.is_word())
            .map(|t| t.text)
            .collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(tokenise("").is_empty());
    }

    #[test]
    fn single_letter_token_is_one_word() {
        let t = tokenise("a");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].kind, TokenKind::Word);
        assert_eq!(t[0].text, "a");
    }

    #[test]
    fn ascii_words_split_on_whitespace() {
        assert_eq!(
            words("talo kissa koira"),
            vec!["talo".to_string(), "kissa".to_string(), "koira".to_string()]
        );
    }

    #[test]
    fn punctuation_is_a_separator() {
        // "talo, kissa." produces four tokens: the comma+space is a
        // separator, "kissa" is its own word, and the trailing "."
        // is its own separator. Sentence-final punctuation is not
        // part of the preceding word in this tokeniser.
        let t = tokenise("talo, kissa.");
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].kind, TokenKind::Word);
        assert_eq!(t[0].text, "talo");
        assert_eq!(t[1].kind, TokenKind::Separator);
        assert_eq!(t[1].text, ", ");
        assert_eq!(t[2].kind, TokenKind::Word);
        assert_eq!(t[2].text, "kissa");
        assert_eq!(t[3].kind, TokenKind::Separator);
        assert_eq!(t[3].text, ".");
    }

    #[test]
    fn finnish_letters_are_letters() {
        // ö, ä, å are alphabetic characters and must not become
        // separators. The spaces between the words are separators.
        let t = tokenise("öljy äiti åke");
        let ws: Vec<&str> = t
            .iter()
            .filter(|t| t.is_word())
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(ws, vec!["öljy", "äiti", "åke"]);
    }

    #[test]
    fn capitalized_word_is_marked() {
        let t = tokenise("Helsinki on kaupunki");
        assert_eq!(t.iter().filter(|t| t.is_word()).count(), 3);
        let helsinki = t.iter().find(|t| t.text == "Helsinki").unwrap();
        assert!(helsinki.is_capitalised());
        let on = t.iter().find(|t| t.text == "on").unwrap();
        assert!(!on.is_capitalised());
    }

    #[test]
    fn trailing_hyphen_keeps_compound_as_one_word() {
        let t = tokenise("nuori-isäntä");
        // Exactly one word run: `nuori-isäntä`.
        assert_eq!(t.len(), 1);
        assert!(t[0].is_word());
        assert_eq!(t[0].text, "nuori-isäntä");
    }

    #[test]
    fn interrupted_speech_marker_splits_at_first_hyphen() {
        // `niin--?` (Canth's Sylvi) is the lemma `niin` plus an
        // interrupted-speech marker. The first hyphen must be
        // a separator so the lemma probe can match `niin`.
        let t = tokenise("niin--?");
        let words: Vec<&str> = t
            .iter()
            .filter(|t| t.is_word())
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(words, vec!["niin"]);
    }

    #[test]
    fn two_hyphens_break_the_run() {
        // `iso--auto`: a trailing hyphen is only kept when it's
        // followed by a letter (so `nuori-isäntä` lands as one
        // word). When the next char is itself a hyphen, the
        // first hyphen is a separator and the run breaks -- so
        // `iso--auto` is `[iso, --, auto]`. The original JS
        // regex absorbed the first hyphen unconditionally,
        // which produced `[iso-, -, auto]`; that contract
        // caused `niin--?` to be tokenised as `niin-`,
        // missing the lemma probe for `niin`.
        let t = tokenise("iso--auto");
        let words: Vec<&str> = t
            .iter()
            .filter(|t| t.is_word())
            .map(|t| t.text.as_str())
            .collect();
        let seps: Vec<&str> = t
            .iter()
            .filter(|t| !t.is_word())
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(words, vec!["iso", "auto"]);
        assert_eq!(seps, vec!["--"]);
    }

    #[test]
    fn digits_inside_a_word_stay_together() {
        assert_eq!(words("abc123def"), vec!["abc123def".to_string()]);
    }

    #[test]
    fn a_pure_number_is_a_separator() {
        let t = tokenise("123");
        assert_eq!(t.len(), 1);
        assert!(!t[0].is_word());
    }

    #[test]
    fn multiple_internal_hyphens_stay_in_one_word() {
        assert_eq!(
            words("itä-länsi-suhteet ja linja-auto"),
            vec![
                "itä-länsi-suhteet".to_string(),
                "ja".to_string(),
                "linja-auto".to_string()
            ]
        );
    }

    #[test]
    fn number_hyphen_word_stays_in_one_word() {
        assert_eq!(words("20-vuotias"), vec!["20-vuotias".to_string()]);
    }

    #[test]
    fn ends_sentence_recognises_period() {
        let t = tokenise("eka. toka");
        let dot = t
            .iter()
            .find(|t| t.kind == TokenKind::Separator)
            .expect("separator after eka");
        assert!(dot.ends_sentence());
    }

    #[test]
    fn ends_sentence_recognises_multi_char_with_whitespace() {
        let t = tokenise("eka.\n\ntoka");
        let s = t.iter().find(|t| t.kind == TokenKind::Separator).unwrap();
        assert!(s.ends_sentence());
    }

    #[test]
    fn ends_sentence_handles_comma() {
        let t = tokenise("eka, toka");
        let s = t.iter().find(|t| t.kind == TokenKind::Separator).unwrap();
        assert!(!s.ends_sentence());
    }

    #[test]
    fn annotate_sentence_initial_marks_first_word() {
        let t = tokenise("Helsinki on kaupunki.");
        let a = annotate_sentence_initial(&t);
        assert_eq!(a.iter().filter(|b| **b).count(), 1);
        assert!(a[0]); // "Helsinki"
    }

    #[test]
    fn annotate_sentence_initial_marks_after_period() {
        let t = tokenise("Kaupunki. Helsinki");
        let a = annotate_sentence_initial(&t);
        // Two sentence-initial words: "Kaupunki" (start) and "Helsinki" (after `.`).
        assert!(a[0]);
        let helsinki_idx = t.iter().position(|t| t.text == "Helsinki").unwrap();
        assert!(a[helsinki_idx]);
    }

    #[test]
    fn annotate_sentence_initial_marks_after_a_line_break() {
        // Gutenberg verse/dialogue lines start with a capital even when the
        // previous line did not end in a full stop.
        let t = tokenise("Pieniä eläjiä olivat.\nElä ole milläsikään");
        let a = annotate_sentence_initial(&t);
        let ela = t.iter().position(|t| t.text == "Elä").unwrap();
        assert!(a[ela]);
    }

    #[test]
    fn annotate_sentence_initial_marks_after_a_colon_or_opening_quote() {
        for text in ["Hän sanoi: Elä tule.", "Hän sanoi: »Elä tule."] {
            let t = tokenise(text);
            let a = annotate_sentence_initial(&t);
            let ela = t.iter().position(|t| t.text == "Elä").unwrap();
            assert!(a[ela], "{text}");
        }
    }

    #[test]
    fn annotate_sentence_initial_skips_comma() {
        let t = tokenise("Helsinki, kaupunki");
        let a = annotate_sentence_initial(&t);
        // Only "Helsinki" is sentence-initial; the comma does not
        // start a new sentence.
        let kaupunki_idx = t.iter().position(|t| t.text == "kaupunki").unwrap();
        assert!(!a[kaupunki_idx]);
    }

    #[test]
    fn byte_offsets_are_correct() {
        let s = "talo, kissa";
        let t = tokenise(s);
        assert_eq!(t[0].text, "talo");
        assert_eq!(t[0].start, 0);
        assert_eq!(t[0].end, 4);
        assert_eq!(t[1].text, ", ");
        assert_eq!(t[1].start, 4);
        assert_eq!(t[1].end, 6);
        assert_eq!(t[2].text, "kissa");
        assert_eq!(t[2].start, 6);
        assert_eq!(t[2].end, s.len());
    }
}
