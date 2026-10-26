//! Compound boundaries for hyphenation.
//!
//! The spell checker already knows how to split a compound, but that
//! answer is a single "which lemma is this a form of" result and the
//! full reverse index is expensive to build. Hyphenation needs a
//! *partition* of the written word into components, and it needs it on
//! the per-keystroke/wasm path.
//!
//! [`LexicalCompoundAnalyzer`] is the bounded implementation: it probes
//! the embedded lexicon and the fast word gates, never builds a
//! `FormBuilder`, and returns the best lexical partition it can defend.
//! [`NoCompoundAnalyzer`] is the explicit pattern-only fallback used by
//! tests and by callers that do not want morphology.

use std::collections::HashMap;

use crate::kotus::sanalista;

/// One component of a compound, with its byte range in the original
/// word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompoundPart {
    /// The surface as it appears in the original word.
    pub text: String,
    /// Byte offset of the first character.
    pub byte_start: usize,
    /// Byte offset just past the last character.
    pub byte_end: usize,
}

/// A compound partition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompoundAnalysis {
    /// Left-to-right components. A simple word has no analysis rather
    /// than a one-part one.
    pub parts: Vec<CompoundPart>,
}

impl CompoundAnalysis {
    /// Byte offsets between components, from left to right.
    pub fn boundaries(&self) -> Vec<usize> {
        self.parts
            .iter()
            .skip(1)
            .map(|part| part.byte_start)
            .collect()
    }
}

/// The morphology adapter hyphenation asks for compound structure.
pub trait CompoundAnalyzer {
    /// The best compound partition of `word`, or `None` when the word
    /// should be treated as a single lexical unit.
    fn analyze(&self, word: &str) -> Option<CompoundAnalysis>;
}

/// Pattern-only fallback.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoCompoundAnalyzer;

impl CompoundAnalyzer for NoCompoundAnalyzer {
    fn analyze(&self, _word: &str) -> Option<CompoundAnalysis> {
        None
    }
}

/// Bounded lexical compound analysis.
///
/// This is intentionally conservative about what counts as a compound:
///
/// * a word the sanalista lists with a real inflection class is a single
///   lexical word (`kansalaisuus`), not a compound;
/// * a classless row is a compound or an uninflected surface
///   (`tietokone`, `laivamatka`), so it may still split;
/// * a class-50/51 row is a compound by declaration, so it may split;
/// * every component must be a known modifier/head, and a head must be a
///   content word rather than a function word or connegative.
#[derive(Clone, Copy, Debug, Default)]
pub struct LexicalCompoundAnalyzer;

impl CompoundAnalyzer for LexicalCompoundAnalyzer {
    fn analyze(&self, word: &str) -> Option<CompoundAnalysis> {
        let lower = word.to_lowercase();
        if lower.chars().count() != word.chars().count() || lower.chars().count() < 4 {
            return None;
        }
        if listed_as_inflecting(&lower) {
            return None;
        }
        let mut segmenter = Segmenter {
            lower: &lower,
            cache: HashMap::new(),
        };
        let scored = segmenter.best(0, word.len(), 0)?;
        if scored.parts.len() < 2 {
            return None;
        }
        Some(CompoundAnalysis {
            parts: scored
                .parts
                .into_iter()
                .map(|part| CompoundPart {
                    text: word[part.start..part.end].to_string(),
                    byte_start: part.start,
                    byte_end: part.end,
                })
                .collect(),
        })
    }
}

#[derive(Clone, Debug)]
struct Part {
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
struct Scored {
    score: i32,
    parts: Vec<Part>,
}

struct Segmenter<'a> {
    lower: &'a str,
    cache: HashMap<(usize, usize), Option<Scored>>,
}

const MAX_DEPTH: usize = 8;
const MIN_LEFT_CHARS: usize = 3;
const MIN_RIGHT_CHARS: usize = 2;

impl<'a> Segmenter<'a> {
    fn best(&mut self, start: usize, end: usize, depth: usize) -> Option<Scored> {
        if start >= end || depth >= MAX_DEPTH {
            return None;
        }
        let key = (start, end);
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }
        // A listed word with a real paradigm is one lexical unit. Its
        // own lemma may be a compound grammatically, but the sanalista
        // has chosen to present it as a word, and breaking it here would
        // turn every derived nominal into a false compound.
        if listed_as_inflecting(&self.lower[start..end]) {
            self.cache.insert(key, None);
            return None;
        }

        let chars: Vec<usize> = self.lower[start..end]
            .char_indices()
            .map(|(offset, _)| start + offset)
            .collect();
        let mut best: Option<Scored> = None;

        let max_left = chars.len().saturating_sub(MIN_RIGHT_CHARS - 1);
        for &split in chars.iter().take(max_left).skip(MIN_LEFT_CHARS) {
            let left = &self.lower[start..split];
            let right = &self.lower[split..end];
            if right.chars().count() < MIN_RIGHT_CHARS || !is_modifier(left) {
                continue;
            }
            if crate::expand::negation::negation_base(right).is_some()
                || crate::expand::connegative::base(right).is_some()
            {
                continue;
            }

            let left_parts = self
                .best(start, split, depth + 1)
                .map(|scored| scored.parts)
                .unwrap_or_else(|| vec![Part { start, end: split }]);

            let right_parts = match self.best(split, end, depth + 1) {
                Some(scored) => scored.parts,
                None if is_head(right) => vec![Part { start: split, end }],
                None => continue,
            };

            let mut parts = left_parts;
            parts.extend(right_parts);
            if parts.len() < 2 {
                continue;
            }
            let score = self.score(&parts);
            if best.as_ref().is_none_or(|current| score > current.score) {
                best = Some(Scored { score, parts });
            }
        }

        self.cache.insert(key, best.clone());
        best
    }

    fn score(&self, parts: &[Part]) -> i32 {
        // More components is generally the more informative partition
        // (`tieto` + `kone` + `ohjelmointi` beats the two-part reading
        // that treats `tietokone` as a unit). A lemma modifier is a
        // stronger signal than an inflected modifier: `auto` + `nostaja`
        // must beat `auton` + `ostaja` in `autonostaja`.
        let mut score = parts.len() as i32 * 100;
        for (at, part) in parts.iter().enumerate() {
            let surface = &self.lower[part.start..part.end];
            if is_lemma(surface) {
                score += if at == 0 { 30 } else { 10 };
            } else if is_modifier(surface) {
                score += 5;
                if at == 0 {
                    score -= 10;
                }
            }
        }
        if is_listed(self.lower) {
            score += 15;
        }
        score
    }
}

/// A word the sanalista gives a model declension. These are lexical
/// units for our purposes even when they are etymological compounds.
fn listed_as_inflecting(word: &str) -> bool {
    sanalista().by_word(word).any(|row| {
        row.inflections()
            .any(|inflection| !matches!(inflection.class, 50 | 51 | 99 | 100 | 101))
    })
}

fn is_listed(word: &str) -> bool {
    sanalista().by_word(word).next().is_some()
}

fn is_lemma(word: &str) -> bool {
    sanalista().by_word(word).next().is_some()
}

fn is_modifier(word: &str) -> bool {
    crate::expand::modifier_is_known(word)
}

fn is_head(word: &str) -> bool {
    // This is the hyphenation hot path: `is_known_word_no_compound`
    // reaches the derivation and stem gates and is far too slow to run
    // for every candidate split of every word. The cheap gates cover the
    // heads that matter for line breaking; a rarer head simply falls back
    // to pattern-only hyphenation.
    crate::suggest::is_content_head(word)
        && (sanalista().by_word(word).next().is_some()
            || crate::expand::all_forms::is_known_inflected_form(word)
            || crate::expand::check_normal::STEM_BUCKETS_NOUN.contains(word)
            || crate::expand::fast_path::base(word).is_some()
            || crate::expand::derived::base(word).is_some()
            || crate::expand::derived::is_nominal(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(word: &str) -> Vec<String> {
        LexicalCompoundAnalyzer
            .analyze(word)
            .map(|analysis| analysis.parts.into_iter().map(|part| part.text).collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_classless_compound_is_partitioned() {
        assert_eq!(parts("tietokone"), ["tieto", "kone"]);
        assert_eq!(parts("laivamatka"), ["laiva", "matka"]);
        assert_eq!(parts("veroprosentti"), ["vero", "prosentti"]);
        assert_eq!(parts("aamukahvi"), ["aamu", "kahvi"]);
    }

    #[test]
    fn a_nested_compound_is_partitioned_recursively() {
        assert_eq!(
            parts("tietokoneohjelmointi"),
            ["tieto", "kone", "ohjelmointi"]
        );
    }

    #[test]
    fn a_lemma_modifier_beats_an_inflected_one() {
        assert_eq!(parts("autonostaja"), ["auto", "nostaja"]);
    }

    #[test]
    fn an_ordinary_inflected_word_is_not_a_compound() {
        assert!(parts("kansalaisuus").is_empty());
        assert!(parts("kissa").is_empty());
        assert!(parts("talossa").is_empty());
    }

    #[test]
    fn boundaries_are_byte_offsets_into_the_original() {
        let word = "äidinkieli";
        let analysis = LexicalCompoundAnalyzer
            .analyze(word)
            .expect("äiti + kieli is a compound");
        assert_eq!(analysis.boundaries(), vec!["äidin".len()]);
        assert!(
            analysis
                .boundaries()
                .iter()
                .all(|&at| at > 0 && at < word.len())
        );
    }
}
