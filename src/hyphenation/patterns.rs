//! Knuth–Liang pattern matching for Finnish.
//!
//! The vendored pattern file is the MIT-licensed school set from
//! `hyph-utf8` (`data/hyphenation/hyph-fi-x-school.tex`). It is parsed
//! once into a character trie; matching a word is a walk from every
//! character position, which is cheap for the few hundred patterns this
//! set contains.
//!
//! Pattern output is only the *ordinary* candidate set. Compound
//! boundaries come from [`super::compounds`], and the Finnish policy in
//! [`super::finnish`] decides what is typographically permitted.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The vendored school pattern set. See `data/hyphenation/README.md`.
const PATTERN_SOURCE: &str = include_str!("../../data/hyphenation/hyph-fi-x-school.tex");

/// One trie node. A node that ends a pattern carries the pattern's score
/// values for the gaps before, between and after its letters.
#[derive(Debug, Default)]
struct Node {
    children: HashMap<char, usize>,
    values: Vec<u8>,
}

/// A parsed pattern set.
#[derive(Debug)]
pub(crate) struct PatternSet {
    nodes: Vec<Node>,
    longest: usize,
}

impl PatternSet {
    /// Parse the `\patterns{...}` block from a hyph-utf8 `.tex` file.
    ///
    /// Comments and the surrounding TeX metadata are ignored. A line
    /// that is not inside the block is ignored too, so the vendored file
    /// keeps its copyright header and provenance.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let mut set = Self {
            nodes: vec![Node::default()],
            longest: 0,
        };
        let mut in_patterns = false;
        let mut seen = 0usize;

        for (line_no, raw) in text.lines().enumerate() {
            // TeX comments run to end of line. Patterns themselves never
            // contain `%`.
            let line = raw.split('%').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if !in_patterns {
                if let Some(rest) = line.strip_prefix("\\patterns") {
                    let rest = rest.trim_start();
                    let Some(rest) = rest.strip_prefix('{') else {
                        return Err(format!(
                            "line {}: \\patterns is not followed by '{{'",
                            line_no + 1
                        ));
                    };
                    in_patterns = true;
                    if !rest.trim().is_empty() {
                        // Support a one-line `\patterns{ ... }` block.
                        if let Some(body) = rest.split('}').next() {
                            seen += set.parse_tokens(body, line_no + 1)?;
                        }
                    }
                }
                continue;
            }
            if let Some(body) = line.split('}').next() {
                let body = body.trim();
                if body.is_empty() {
                    break;
                }
                seen += set.parse_tokens(body, line_no + 1)?;
                if line.contains('}') {
                    break;
                }
            }
        }

        if seen == 0 {
            return Err("no hyphenation patterns found".to_string());
        }
        Ok(set)
    }

    fn parse_tokens(&mut self, body: &str, line_no: usize) -> Result<usize, String> {
        let mut count = 0;
        for token in body.split_whitespace() {
            if token.is_empty() || token == "\\patterns{" {
                continue;
            }
            let token = token.trim_matches(|c: char| c == '{' || c == '}');
            if token.is_empty() {
                continue;
            }
            if !token
                .chars()
                .all(|c| matches!(c, 'a'..='z' | 'å' | 'ä' | 'ö' | 'š' | 'ž' | '.' | '0'..='9'))
            {
                return Err(format!(
                    "line {line_no}: pattern {token:?} contains a character outside the Finnish pattern alphabet"
                ));
            }
            if !token.chars().any(|c| c.is_ascii_digit()) {
                // A pattern with no score can never allow a break.
                continue;
            }
            let mut letters = String::new();
            let mut values = vec![0u8];
            for c in token.chars() {
                if let Some(digit) = c.to_digit(10) {
                    *values.last_mut().expect("values is never empty") = digit as u8;
                } else {
                    letters.push(c);
                    values.push(0);
                }
            }
            if letters.is_empty() {
                continue;
            }
            self.longest = self.longest.max(letters.chars().count());
            self.insert(&letters, &values);
            count += 1;
        }
        Ok(count)
    }

    fn insert(&mut self, letters: &str, values: &[u8]) {
        let mut node = 0usize;
        for c in letters.chars() {
            let next = match self.nodes[node].children.get(&c) {
                Some(&next) => next,
                None => {
                    let next = self.nodes.len();
                    self.nodes.push(Node::default());
                    self.nodes[node].children.insert(c, next);
                    next
                }
            };
            node = next;
        }
        // If a pattern is repeated, keep the strongest score at each gap.
        let slot = &mut self.nodes[node].values;
        if slot.len() < values.len() {
            slot.resize(values.len(), 0);
        }
        for (at, &value) in values.iter().enumerate() {
            slot[at] = slot[at].max(value);
        }
    }

    /// The break positions of `word`, as character counts from the start
    /// (always between 1 and `word.chars().count() - 1`).
    ///
    /// This is the raw pattern result. Minimum fragments, vowel rules and
    /// compound boundaries are applied by the caller.
    pub(crate) fn break_chars(&self, word: &str) -> Vec<usize> {
        let chars: Vec<char> = word
            .chars()
            .map(|c| c.to_lowercase().next().unwrap_or(c))
            .collect();
        let n = chars.len();
        if n < 2 {
            return Vec::new();
        }

        // A dot on either side lets patterns anchor at the word edge.
        let mut padded = Vec::with_capacity(n + 2);
        padded.push('.');
        padded.extend(chars.iter().copied());
        padded.push('.');

        let mut scores = vec![0u8; padded.len() + 1];
        for start in 0..padded.len() {
            let mut node = 0usize;
            let last = (start + self.longest).min(padded.len());
            for &c in &padded[start..last] {
                let Some(&next) = self.nodes[node].children.get(&c) else {
                    break;
                };
                node = next;
                let values = &self.nodes[node].values;
                if values.is_empty() {
                    continue;
                }
                for (offset, &value) in values.iter().enumerate() {
                    let at = start + offset;
                    if at < scores.len() {
                        scores[at] = scores[at].max(value);
                    }
                }
            }
        }

        // `padded` is '.' + word + '.'. A break after original character
        // `c` sits at padded gap `c + 2`; `c` runs from 0 to n - 2.
        (0..n.saturating_sub(1))
            .filter(|&c| scores[c + 2] % 2 == 1)
            .map(|c| c + 1)
            .collect()
    }
}

/// The embedded Finnish pattern set, parsed on first use.
pub(crate) fn finnish() -> &'static PatternSet {
    static SET: OnceLock<PatternSet> = OnceLock::new();
    SET.get_or_init(|| {
        PatternSet::parse(PATTERN_SOURCE).unwrap_or_else(|error| {
            panic!("the vendored Finnish hyphenation patterns are invalid: {error}")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(word: &str) -> Vec<usize> {
        finnish().break_chars(word)
    }

    #[test]
    fn the_vendored_set_parses() {
        assert!(finnish().nodes.len() > 300);
    }

    #[test]
    fn simple_words_follow_the_school_patterns() {
        assert_eq!(chars("matka"), vec![3]);
        assert_eq!(chars("kissa"), vec![3]);
        assert_eq!(chars("talo"), vec![2]);
        // The raw school set also gives `ra-di-o`; the policy removes
        // the final one-character fragment later.
        assert_eq!(chars("radio"), vec![2, 4]);
        assert_eq!(chars("paprika"), vec![3, 5]);
    }

    #[test]
    fn vowel_sequences_are_left_to_the_policy() {
        // The school set deliberately offers more than line typography
        // should use; `asia` has no two-character first fragment.
        assert_eq!(chars("asia"), vec![1, 3]);
    }

    #[test]
    fn a_pattern_match_is_not_confused_by_case() {
        assert_eq!(chars("Äänekoski"), chars("äänekoski"));
    }
}
