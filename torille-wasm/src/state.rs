//! Per-textarea state for the wasm spell-checker.
//!
//! One [`TextAreaState`] per live text input. The state is owned by
//! the worker thread; the main thread holds a `u32` handle (the slot
//! index in a process-global `Vec<Option<TextAreaState>>`) and
//! talks to it through the wasm-bindgen exports in `lib.rs`.
//!
//! ## What the state holds
//!
//! - `proper_nouns` -- single-word, lower-cased. Built on every
//!   [`update`] call from the *current* text. Re-built (not
//!   accumulated) so a removed word disappears from the state on the
//!   next update.
//! - `phrases` -- multi-word proper nouns / technical terms from
//!   the text. 2-3 consecutive capitalised non-lexicon tokens get
//!   stored with a U+001F separator (Unit Separator -- never appears
//!   in user text, so it can't collide on whitespace inside words).
//! - `generation` -- bumped by every `update` / `clear`. Reserved
//!   for the generation-tagged cache slice; not consulted today.
//!
//! ## Efficiency
//!
//! `update` is O(text-length) and binds in a few microseconds for
//! the sizes the demo and extensions care about. `analyze` is the
//! same shape plus the per-misspelling `suggest` call -- a typical
//! 100-word textarea with one misspelling finishes well under 50 ms
//! on the host.
//!
//! [`update`]: TextAreaState::update

use std::collections::{HashMap, HashSet};

use torille::expand::check_normal::{NormalizedOutcome, check_normal_fast};

/// One spelling error in the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The literal text of the misspelled token (case preserved).
    pub word: String,
    /// Byte offset of the first character in the original input.
    pub start: usize,
    /// Byte offset just past the token (exclusive).
    pub end: usize,
    /// Up to `max_suggestions` ranked candidates. May be empty when
    /// the ranker finds nothing within the configured distance.
    pub suggestions: Vec<String>,
}

/// First-pass misspell anchor: word + byte range, no suggestions.
/// The demo's render writes these into the UI immediately, then
/// back-fills `suggestions` via the on-click pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisspellRange {
    pub word: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct TextAreaState {
    /// Single-word proper nouns learned from the textarea.
    /// Lower-cased keys.
    pub proper_nouns: HashSet<String>,
    /// Multi-word phrases (joined with `\u{1F}`).
    pub phrases: HashSet<String>,
    /// Bumped by every `update` / `clear`. Reserved for the
    /// generation-tagged cache slice; not consulted today.
    pub generation: u64,
}

impl TextAreaState {
    pub fn new() -> Self {
        TextAreaState {
            proper_nouns: HashSet::new(),
            phrases: HashSet::new(),
            generation: 0,
        }
    }

    /// Drop every learned value and re-extract from `text`. Called
    /// on every `update_state_from_text` so the state mirrors the
    /// text exactly: nothing the user deleted remains.
    pub fn update(&mut self, text: &str) {
        self.proper_nouns.clear();
        self.phrases.clear();
        self.generation += 1;

        // The text is walked in windows, never as one token vector:
        // a per-render allocation of every token in the document is
        // what pushed a large paste's wasm memory up with the
        // document, and wasm linear memory never shrinks.
        //
        // Pass 1 records the words seen capitalised *mid-sentence*. That is
        // the proper-noun signal: a sentence-initial capital is just the
        // normal way to start a sentence ("This", "The", "Helsinki"), so it
        // is not evidence of a name on its own. A name that also appears
        // mid-sentence capitalised is learned; one that only ever starts
        // sentences is left to the spell-checker to flag.
        let mut mid_caps: HashSet<String> = HashSet::new();
        let mut sentence_initial = true;
        for window in text_windows(text) {
            for tok in torille::tokenise::tokenise(window) {
                if tok.is_word() {
                    let at_sentence_start = sentence_initial;
                    sentence_initial = false;
                    if tok.is_capitalised()
                        && !at_sentence_start
                        && let Some(lower) = tok.lower_word()
                        && lower.chars().count() >= 2
                    {
                        mid_caps.insert(lower);
                    }
                } else if tok.ends_sentence() {
                    sentence_initial = true;
                }
            }
        }
        if mid_caps.is_empty() {
            return;
        }

        // Pass 2 extracts proper nouns and phrases, carrying the phrase
        // buffer across window boundaries. A phrase is a run of capitalised
        // non-lexicon words; another word breaks the run.
        let mut buf: Vec<String> = Vec::new();
        for window in text_windows(text) {
            for tok in torille::tokenise::tokenise(window) {
                let Some(lower) = tok.lower_word() else {
                    continue;
                };
                let long_enough = lower.chars().count() >= 2;
                let not_in_lexicon = !torille::suggest::is_lemma(&lower);
                let is_proper = tok.is_capitalised()
                    && long_enough
                    && not_in_lexicon
                    && mid_caps.contains(&lower);
                if is_proper {
                    self.proper_nouns.insert(lower.clone());
                    buf.push(lower);
                } else {
                    flush_phrase_buf(&buf, &mut self.phrases);
                    buf.clear();
                }
            }
        }
        // Flush any run that walked off the end of the input.
        flush_phrase_buf(&buf, &mut self.phrases);
    }

    /// Drop every learned value, keep the slot. Generation bumps
    /// so any future cache slice can invalidate without expensive
    /// re-keying.
    pub fn clear(&mut self) {
        self.proper_nouns.clear();
        self.phrases.clear();
        self.generation += 1;
    }

    /// True if the state contains `word` as a learned proper
    /// noun. Used by the popup's suggestion path so words the
    /// user has typed before survive as candidates.
    pub fn knows_proper_noun(&self, word: &str) -> bool {
        let lower = word.to_lowercase();
        self.proper_nouns.contains(&lower)
    }

    /// Walk `text`, return the list of [`Hit`]s with only word +
    /// byte ranges (no suggestions filled in). Used by the demo's
    /// first-pass render so the misspelling underlines can land on
    /// the front-end immediately, without waiting for the slower
    /// per-word suggestion pass.
    ///
    /// "Misspelling" here is "not in the lemma HashSet, not in the
    /// learned proper-noun set, not part of a learned phrase".
    /// The fast path is `is_lemma` (O(1) HashSet probe); inflected
    /// forms the user typed correctly will be *misreported* here as
    /// misspellings, but the suggestion pass on click can find the
    /// lemma they meant. For 99% of real-world demo inputs (typos
    /// of lemmas, English noise, made-up words) this is correct
    /// on the first pass.
    pub fn find_misspells(&self, text: &str) -> Vec<MisspellRange> {
        let mut out = Vec::new();
        let tokens = torille::tokenise::tokenise(text);
        let n = tokens.len();
        // Reader pages repeat names and dialect forms (`sortavalan`,
        // `sitte`) dozens of times. Once a surface is decided known or
        // unknown in this pass, every later occurrence gets the same
        // answer without another derivation/stem-map probe.
        let mut decision: HashMap<String, bool> = HashMap::new();
        let mut i = 0;
        while i < n {
            if let Some(consumed) = match_phrase_at(&tokens[i..], &self.phrases) {
                i += consumed;
                continue;
            }
            let tok = &tokens[i];
            if !tok.is_word() {
                i += 1;
                continue;
            }
            let lower = tok
                .lower_word()
                .expect("is_word implies lower_word is Some");
            // Acceptance gates, fastest first. The gate pipeline
            // itself lives in the library
            // (`torille::expand::check_normal::check_normal_fast`:
            // lemma, pre-computed form set, clitic strip, noun- and
            // verb-stem maps -- every gate a constant-time probe, no
            // engine walk). The demo adds the one input the library
            // cannot know about: the proper nouns it has learned
            // from the text.
            let known = *decision.entry(lower.clone()).or_insert_with(|| {
                !matches!(check_normal_fast(&lower, false), NormalizedOutcome::Unknown)
            });
            if self.proper_nouns.contains(&lower)
                || torille::expand::proper_names::contains_capitalised(&tok.text)
                || known
            {
                i += 1;
                continue;
            }
            out.push(MisspellRange {
                word: tok.text.clone(),
                start: tok.start,
                end: tok.end,
            });
            i += 1;
        }
        out
    }

    /// Suggestions for a single already-known-misspelled word.
    /// Bounded Levenshtein over the embedded lemma list -- no
    /// paradigm generation, runs in a few hundred milliseconds for
    /// any Finnish word. The slower-than-`is_lemma` cost is why
    /// the render path does misspell detection first and triggers
    /// this only on click.
    pub fn suggest_for(&self, word: &str, max_n: usize, max_distance: usize) -> Vec<String> {
        torille::suggest::suggest_over_lemmas(word, max_n, max_distance)
    }

    /// Walk `text`, return the list of [`Hit`]s (with
    /// suggestions). Phrases in the learned set suppress every
    /// word inside them so a known multi-token name like "New
    /// York" doesn't get reported as one misspelling per token.
    ///
    /// **Slow path** -- kept for callers that want the full
    /// result in one round-trip. The demo's render uses
    /// [`find_misspells`] for the first pass and
    /// [`suggest_for`] for the on-click second pass.
    pub fn analyze(&self, text: &str, max_suggestions: usize) -> Vec<Hit> {
        let mut out = Vec::new();
        let tokens = torille::tokenise::tokenise(text);
        let n = tokens.len();

        let mut i = 0;
        while i < n {
            // Try to match a known 3-token then 2-token phrase starting at i.
            if let Some(consumed) = match_phrase_at(&tokens[i..], &self.phrases) {
                i += consumed;
                continue;
            }

            let tok = &tokens[i];
            if !tok.is_word() {
                i += 1;
                continue;
            }
            let lower = tok
                .lower_word()
                .expect("is_word implies lower_word is Some");

            // check_fast covers lemma + proper-noun; falls through
            // to find_bases_with for inflected forms.
            if torille::expand::proper_names::contains_capitalised(&tok.text)
                || torille::suggest::check_fast(&lower, &self.proper_nouns).is_some()
            {
                i += 1;
                continue;
            }

            let suggestions = torille::suggest::suggest_with_state(
                &lower,
                max_suggestions,
                2,
                &self.proper_nouns,
            );
            out.push(Hit {
                word: tok.text.clone(),
                start: tok.start,
                end: tok.end,
                suggestions,
            });
            i += 1;
        }

        out
    }
}

/// Split `text` into windows of at most `WINDOW_BYTES`, each ending
/// just after a whitespace run so no word straddles two windows (the
/// tokeniser's trailing-hyphen rule needs the word's own next
/// character). A single word longer than a window is kept whole.
fn text_windows(text: &str) -> Vec<&str> {
    const WINDOW_BYTES: usize = 1 << 16;
    if text.len() <= WINDOW_BYTES {
        return vec![text];
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(text.len() / WINDOW_BYTES + 1);
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + WINDOW_BYTES).min(text.len());
        if end < text.len() {
            while end > start && !bytes[end - 1].is_ascii_whitespace() {
                end -= 1;
            }
            if end == start {
                // One word longer than a window: take it whole.
                end = (start + WINDOW_BYTES).min(text.len());
                while end < text.len() && !bytes[end].is_ascii_whitespace() {
                    end += 1;
                }
                while end < text.len() && bytes[end].is_ascii_whitespace() {
                    end += 1;
                }
            }
        }
        out.push(&text[start..end]);
        start = end;
    }
    out
}

/// Flush a phrase buffer into `phrases` if it has 2-3 words.
/// Single-word buffers are handled by `proper_nouns` already;
/// longer buffers are too rare to be worth the storage cost.
fn flush_phrase_buf(buf: &[String], phrases: &mut HashSet<String>) {
    if (2..=3).contains(&buf.len()) {
        phrases.insert(buf.join("\u{1F}"));
    }
}

/// If `tokens[..]` starts with a known phrase in `phrases`,
/// return how many tokens that consumes (2 or 3). 3 is tried
/// first so longer phrases beat their shorter suffixes.
fn match_phrase_at(
    tokens: &[torille::tokenise::Token],
    phrases: &HashSet<String>,
) -> Option<usize> {
    for &run_len in &[3usize, 2] {
        if tokens.len() < run_len {
            continue;
        }
        let parts: Vec<String> = tokens[..run_len]
            .iter()
            .filter_map(|t| t.lower_word())
            .collect();
        if parts.len() != run_len {
            continue;
        }
        let key = parts.join("\u{1F}");
        if phrases.contains(&key) {
            return Some(run_len);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_state_analyze_returns_no_hits() {
        let st = TextAreaState::new();
        let r = st.analyze("", 8);
        assert!(r.is_empty());
    }

    #[test]
    fn analyze_finds_misspellings() {
        let st = TextAreaState::new();
        let r = st.analyze("taloo koirra xyzzy", 8);
        let words: Vec<&str> = r.iter().map(|h| h.word.as_str()).collect();
        assert_eq!(words, vec!["taloo", "koirra", "xyzzy"]);
    }

    #[test]
    fn analyze_returns_byte_ranges() {
        let st = TextAreaState::new();
        let r = st.analyze("talo taloo kissa", 8);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].word, "taloo");
        assert_eq!(r[0].start, 5);
        assert_eq!(r[0].end, 10);
    }

    #[test]
    fn analyze_returns_suggestions_for_misspellings() {
        let st = TextAreaState::new();
        let r = st.analyze("taloo", 5);
        assert_eq!(r.len(), 1);
        assert!(
            r[0].suggestions.iter().any(|s| s == "talo"),
            "expected 'talo' suggestion, got {:?}",
            r[0].suggestions
        );
    }

    #[test]
    fn update_extracts_capitalised_proper_nouns() {
        // Mid-sentence capitals are accepted on first occurrence;
        // only sentence-initial capitals require 2+ occurrences to
        // disambiguate from a sentence-starter "Tämä".
        let mut st = TextAreaState::new();
        st.update("Matkustan Helsinkiin huomenna.");
        assert!(st.knows_proper_noun("helsinkiin"));
        // "Matkustan" is sentence-initial and appears only once --
        // NOT learned.
        assert!(!st.knows_proper_noun("matkustan"));
    }

    #[test]
    fn update_ignores_lexicon_capitalised_words() {
        // "Talo" is a Finnish lemma capitalised at sentence start.
        // It should NOT be added to proper_nouns -- it's already
        // known via the lemma fast path.
        let mut st = TextAreaState::new();
        st.update("Talo on talo.");
        assert!(!st.knows_proper_noun("talo"));
    }

    #[test]
    fn update_does_not_learn_a_word_only_seen_at_sentence_start() {
        // "Helsinki" appears twice, but only at sentence starts. That is
        // the same shape as an English "This"/"The", so it is not evidence
        // of a name; it stays visible to the spell-checker.
        let mut st = TextAreaState::new();
        st.update("Helsinki on iso. Helsinki on kaunis.");
        assert!(!st.knows_proper_noun("helsinki"));
    }

    #[test]
    fn update_learns_a_name_that_appears_mid_sentence() {
        // A mid-sentence capital is the proper-noun signal.
        let mut st = TextAreaState::new();
        st.update("Helsinki on iso. Kaupunki Helsinki on kaunis.");
        assert!(st.knows_proper_noun("helsinki"));
    }

    #[test]
    fn repeated_sentence_initial_english_words_are_still_flagged() {
        // "This" and "The" both start two sentences. Capitalisation alone
        // must not whitelist them as Finnish proper nouns.
        let mut st = TextAreaState::new();
        st.update("This is one. This is two. The end. The end.");
        assert!(!st.knows_proper_noun("this"));
        assert!(!st.knows_proper_noun("the"));
        let hits = st.find_misspells("this the");
        assert_eq!(hits.len(), 2, "expected both to be flagged, got {hits:?}");
    }

    #[test]
    fn text_windows_partition_at_word_boundaries() {
        // A window ends just after whitespace, so no word straddles
        // two calls and the concatenation is the original text.
        let text = "sana toinen kolmas. ".repeat(20000); // > one window
        let windows = text_windows(&text);
        assert!(windows.len() > 1, "expected several windows");
        assert_eq!(windows.concat(), text, "windows must reconstruct the text");
        for w in &windows[..windows.len() - 1] {
            assert!(
                w.ends_with(|c: char| c.is_whitespace()),
                "window does not end at whitespace: {:?}",
                &w[w.len().saturating_sub(24)..]
            );
        }
    }

    #[test]
    fn update_streams_a_text_larger_than_one_window() {
        // Counts, the sentence-initial state and phrase runs all have
        // to survive a window boundary.
        let mut st = TextAreaState::new();
        let mut text = "kissa istuu pöydällä ja katselee ikkunasta. ".repeat(2500);
        assert!(text.len() > (1 << 16), "test text must exceed one window");
        text.push_str("Helsinki on iso. Kaupunki Helsinki on kaunis. Menin New Yorkiin eilen.");
        st.update(&text);
        assert!(
            st.knows_proper_noun("helsinki"),
            "proper noun after a window boundary was not learned"
        );
        assert!(
            st.phrases.iter().any(|p| p.contains("yorkiin")),
            "phrase after a window boundary was not learned: {:?}",
            st.phrases
        );
    }

    #[test]
    fn update_does_not_learn_single_occurrence_at_sentence_start() {
        // "Helsinki" appears once, at sentence start. Without 2+
        // occurrences in the text we cannot tell whether it is a
        // proper noun or the first word of an ordinary sentence.
        let mut st = TextAreaState::new();
        st.update("Helsinki on iso kaupunki.");
        assert!(!st.knows_proper_noun("helsinki"));
    }

    #[test]
    fn update_extracts_multi_token_phrases() {
        let mut st = TextAreaState::new();
        st.update("Menin New Yorkiin eilen.");
        let phrases: Vec<&String> = st.phrases.iter().collect();
        assert!(
            phrases.iter().any(|p| p.as_str() == "new\u{1F}yorkiin"),
            "expected phrase 'new\\u{{1F}}yorkiin', got {phrases:?}"
        );
        // "Menin" is sentence-initial + single occurrence -- not a phrase.
        assert_eq!(phrases.len(), 1);
    }

    #[test]
    fn analyze_suppresses_words_in_a_known_phrase() {
        let mut st = TextAreaState::new();
        st.update("Menin New Yorkiin eilen.");
        // Analyzing text containing the same multi-word phrase
        // suppresses both words in the phrase.
        let r = st.analyze("New Yorkiin on kaupunki", 8);
        let words: Vec<&str> = r.iter().map(|h| h.word.as_str()).collect();
        assert!(
            !words.contains(&"New"),
            "expected 'New' suppressed by phrase, got {words:?}"
        );
        assert!(
            !words.contains(&"Yorkiin"),
            "expected 'Yorkiin' suppressed by phrase, got {words:?}"
        );
    }

    #[test]
    fn clear_drops_everything() {
        let mut st = TextAreaState::new();
        st.update("Matkustan Helsinkiin huomenna.");
        assert!(st.knows_proper_noun("helsinkiin"));
        st.clear();
        assert!(!st.knows_proper_noun("helsinkiin"));
    }

    #[test]
    fn update_drops_words_removed_from_text() {
        // If the text changes and the user deletes a proper noun,
        // the next `update` should drop it from the state.
        let mut st = TextAreaState::new();
        st.update("Matkustan Helsinkiin huomenna.");
        assert!(st.knows_proper_noun("helsinkiin"));
        st.update("Matkustan Tampereelle huomenna.");
        assert!(!st.knows_proper_noun("helsinkiin"));
        assert!(st.knows_proper_noun("tampereelle"));
    }
}
