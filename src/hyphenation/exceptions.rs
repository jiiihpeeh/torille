//! Explicit break exceptions.
//!
//! Patterns are a baseline, not a verdict. A loanword can contain a
//! vowel sequence that Finnish line typography normally avoids, and a
//! proper name can have a break that no general rule predicts. The table
//! here is deliberately small: a missing *compound* boundary belongs in
//! the morphology layer, not in a growing suffix list.
//!
//! Built-in rows live in `data/hyphenation/exceptions.csv`; callers can
//! build an [`Exceptions`] value with their own allow/deny entries.

use std::collections::HashMap;
use std::sync::OnceLock;

const SOURCE: &str = include_str!("../../data/hyphenation/exceptions.csv");

/// Allow and deny offsets for one spelling.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Rule {
    allow: Vec<usize>,
    deny: Vec<usize>,
}

impl Rule {
    fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }
}

/// Explicit break overrides, keyed by lower-cased spelling.
///
/// Offsets are character counts from the start of the word: an allow
/// offset of `4` means "a break is permitted after the fourth
/// character", matching [`super::HyphenationPoint::char_offset`].
#[derive(Clone, Debug, Default)]
pub struct Exceptions {
    rules: HashMap<String, Rule>,
}

impl Exceptions {
    /// No exceptions at all.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The built-in table parsed from `data/hyphenation/exceptions.csv`.
    pub fn finnish() -> &'static Self {
        static TABLE: OnceLock<Exceptions> = OnceLock::new();
        TABLE.get_or_init(|| {
            Self::parse(SOURCE).unwrap_or_else(|error| {
                panic!("the embedded hyphenation exception table is invalid: {error}")
            })
        })
    }

    /// Permit a break after `char_offset` in `word`.
    pub fn allow(&mut self, word: &str, char_offset: usize) {
        let rule = self.rules.entry(word.to_lowercase()).or_default();
        if !rule.allow.contains(&char_offset) {
            rule.allow.push(char_offset);
        }
    }

    /// Forbid a break after `char_offset` in `word`, even when a pattern
    /// or the compound layer offers one.
    pub fn deny(&mut self, word: &str, char_offset: usize) {
        let rule = self.rules.entry(word.to_lowercase()).or_default();
        if !rule.deny.contains(&char_offset) {
            rule.deny.push(char_offset);
        }
    }

    /// Whether `word` has an explicit allow at `char_offset`.
    pub(crate) fn allows(&self, word: &str, char_offset: usize) -> bool {
        self.rules
            .get(&word.to_lowercase())
            .is_some_and(|rule| rule.allow.contains(&char_offset))
    }

    /// Whether `word` has an explicit deny at `char_offset`.
    pub(crate) fn denies(&self, word: &str, char_offset: usize) -> bool {
        self.rules
            .get(&word.to_lowercase())
            .is_some_and(|rule| rule.deny.contains(&char_offset))
    }

    /// Character offsets explicitly allowed for `word`.
    pub(crate) fn allow_offsets(&self, word: &str) -> &[usize] {
        self.rules
            .get(&word.to_lowercase())
            .map_or(&[], |rule| rule.allow.as_slice())
    }

    /// Character offsets explicitly denied for `word`.
    pub(crate) fn deny_offsets(&self, word: &str) -> &[usize] {
        self.rules
            .get(&word.to_lowercase())
            .map_or(&[], |rule| rule.deny.as_slice())
    }

    fn parse(text: &str) -> Result<Self, String> {
        let mut exceptions = Self::default();
        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("word,") {
                continue;
            }
            let mut fields = line.split(',');
            let word = fields.next().unwrap_or("").trim();
            let allow = fields.next().unwrap_or("").trim();
            let deny = fields.next().unwrap_or("").trim();
            if word.is_empty() {
                return Err(format!("line {}: missing word", line_no + 1));
            }
            for offset in parse_offsets(allow, line_no + 1)? {
                exceptions.allow(word, offset);
            }
            for offset in parse_offsets(deny, line_no + 1)? {
                exceptions.deny(word, offset);
            }
        }
        exceptions.rules.retain(|_, rule| !rule.is_empty());
        Ok(exceptions)
    }
}

fn parse_offsets(field: &str, line_no: usize) -> Result<Vec<usize>, String> {
    if field.is_empty() {
        return Ok(Vec::new());
    }
    field
        .split(';')
        .map(|part| {
            part.trim()
                .parse::<usize>()
                .map_err(|_| format!("line {line_no}: {part:?} is not a character offset"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_table_has_the_loanword_exception() {
        let builtins = Exceptions::finnish();
        assert!(builtins.allows("kofeiini", 4));
        assert!(!builtins.denies("kofeiini", 4));
    }

    #[test]
    fn callers_can_allow_and_deny() {
        let mut exceptions = Exceptions::empty();
        exceptions.allow("Nimeni", 3);
        assert!(exceptions.allows("nimeni", 3));
        exceptions.deny("nimeni", 2);
        assert!(exceptions.denies("nimeni", 2));
    }

    #[test]
    fn malformed_offsets_are_reported() {
        let error =
            Exceptions::parse("word,allow,deny,note\nkissa,x,,\n").expect_err("x is not an offset");
        assert!(error.contains("line 2"));
    }
}
