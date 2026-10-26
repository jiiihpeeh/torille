//! Finnish hyphenation.
//!
//! Three layers, deliberately separate:
//!
//! 1. [`patterns`] finds ordinary Knuth–Liang candidates from the
//!    MIT-licensed school pattern set in `data/hyphenation/`.
//! 2. [`compounds`] supplies compound structure from the lexicon.
//! 3. This module applies Finnish typographic policy: minimum fragments,
//!    the vowel rules, ranking and explicit exceptions.
//!
//! The public result is a set of **permitted** break points with byte
//! offsets and a ranking penalty. The core does not choose one best
//! split without knowing line width; callers use [`split`] for a
//! convenient borrowed-slice view or select among the points themselves.

mod compounds;
mod exceptions;
mod finnish;
mod offsets;
mod patterns;

use std::collections::BTreeMap;

pub use compounds::{
    CompoundAnalysis, CompoundAnalyzer, CompoundPart, LexicalCompoundAnalyzer, NoCompoundAnalyzer,
};
pub use exceptions::Exceptions;

use offsets::{byte_to_char, char_to_byte};
use patterns::finnish as finnish_patterns;

/// What to do with a break that falls between two vowels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VowelBreaks {
    /// Permit a vowel break only at a recognised compound boundary or
    /// when an explicit exception allows it. This is the default Finnish
    /// typographic policy.
    #[default]
    CompoundOnly,
    /// Never permit a vowel break, even inside a compound.
    Never,
    /// Keep every pattern candidate, including inter-vowel ones. Useful
    /// for syllabification or diagnostics, not for normal line breaking.
    Always,
}

/// Options for [`positions_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HyphenationOptions {
    /// Minimum characters before a break.
    pub left_min: usize,
    /// Minimum characters after a break.
    pub right_min: usize,
    /// Ask the morphology adapter for compound boundaries.
    pub compound_breaks: bool,
    /// How strictly to treat a break between two vowels.
    pub vowel_breaks: VowelBreaks,
}

impl Default for HyphenationOptions {
    fn default() -> Self {
        Self {
            left_min: 2,
            right_min: 2,
            compound_breaks: true,
            vowel_breaks: VowelBreaks::CompoundOnly,
        }
    }
}

/// Where a permitted break came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BreakSource {
    /// A Knuth–Liang pattern candidate.
    Pattern,
    /// The morphology layer's component boundary.
    CompoundBoundary,
    /// An explicit allow entry.
    ExplicitException,
}

impl BreakSource {
    /// A stable lower-case name for CLI/JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pattern => "pattern",
            Self::CompoundBoundary => "compound",
            Self::ExplicitException => "exception",
        }
    }
}

/// One permitted break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HyphenationPoint {
    /// Byte offset of the start of the next fragment. Always a UTF-8
    /// character boundary.
    pub byte_offset: usize,
    /// Character offset of the same position.
    pub char_offset: usize,
    /// Whether this point is a morphology-recognised compound boundary.
    pub compound_boundary: bool,
    /// Where the candidate came from.
    pub source: BreakSource,
    /// Ranking hint; lower is preferred. This is not a statistical
    /// confidence.
    pub penalty: u16,
}

/// All permitted breaks in `word` under the default Finnish policy.
pub fn positions(word: &str) -> Vec<HyphenationPoint> {
    positions_with(
        word,
        &HyphenationOptions::default(),
        &LexicalCompoundAnalyzer,
        Exceptions::finnish(),
    )
}

/// All permitted breaks in `word` under explicit options.
pub fn positions_with(
    word: &str,
    options: &HyphenationOptions,
    analyzer: &dyn CompoundAnalyzer,
    exceptions: &Exceptions,
) -> Vec<HyphenationPoint> {
    if word.is_empty() {
        return Vec::new();
    }
    let mut points = Vec::new();
    if word.contains('-') {
        let mut start = 0usize;
        for (at, ch) in word.char_indices() {
            if ch == '-' {
                if at > start {
                    collect_segment(
                        &word[start..at],
                        start,
                        options,
                        analyzer,
                        exceptions,
                        &mut points,
                    );
                }
                start = at + ch.len_utf8();
            }
        }
        if start < word.len() {
            collect_segment(
                &word[start..],
                start,
                options,
                analyzer,
                exceptions,
                &mut points,
            );
        }
    } else {
        collect_segment(word, 0, options, analyzer, exceptions, &mut points);
    }

    apply_word_exceptions(word, options, exceptions, &mut points);
    let mut points = dedup(points);
    // `char_offset` is absolute in the original word even when the
    // candidate came from a hyphen-delimited segment. Recompute it from
    // the final byte offset rather than trusting the segment-local value.
    for point in &mut points {
        point.char_offset = byte_to_char(word, point.byte_offset).unwrap_or(point.char_offset);
    }
    points
}

/// Split `word` at the supplied points, returning borrowed slices.
///
/// Invalid, duplicate, and out-of-range points are ignored; the source
/// string is never modified and the concatenation of the returned
/// slices is always the original word.
pub fn split<'a>(word: &'a str, points: &[HyphenationPoint]) -> Vec<&'a str> {
    let mut offsets: Vec<usize> = points
        .iter()
        .map(|point| point.byte_offset)
        .filter(|&at| at > 0 && at < word.len() && word.is_char_boundary(at))
        .collect();
    offsets.sort_unstable();
    offsets.dedup();
    let mut out = Vec::with_capacity(offsets.len() + 1);
    let mut start = 0;
    for at in offsets {
        out.push(&word[start..at]);
        start = at;
    }
    out.push(&word[start..]);
    out
}

fn collect_segment(
    segment: &str,
    base: usize,
    options: &HyphenationOptions,
    analyzer: &dyn CompoundAnalyzer,
    exceptions: &Exceptions,
    out: &mut Vec<HyphenationPoint>,
) {
    if segment.is_empty() {
        return;
    }
    let analysis = options
        .compound_breaks
        .then(|| analyzer.analyze(segment))
        .flatten()
        .filter(|analysis| analysis.parts.len() > 1);
    if let Some(analysis) = analysis {
        for part in &analysis.parts {
            collect_patterns(&part.text, base + part.byte_start, options, exceptions, out);
        }
        for boundary in analysis.boundaries() {
            let Some((left, right)) = finnish::fragment_lengths(segment, boundary) else {
                continue;
            };
            if left < options.left_min || right < options.right_min {
                continue;
            }
            let vowel = finnish::vowel_vowel(segment, boundary);
            if vowel && options.vowel_breaks == VowelBreaks::Never {
                continue;
            }
            push_point(
                segment,
                base,
                boundary,
                BreakSource::CompoundBoundary,
                true,
                0,
                out,
            );
        }
    } else {
        collect_patterns(segment, base, options, exceptions, out);
    }
}

fn collect_patterns(
    segment: &str,
    base: usize,
    options: &HyphenationOptions,
    exceptions: &Exceptions,
    out: &mut Vec<HyphenationPoint>,
) {
    let lower = segment.to_lowercase();
    for char_offset in finnish_patterns().break_chars(segment) {
        let Some(byte) = char_to_byte(segment, char_offset) else {
            continue;
        };
        if !valid_fragment(segment, byte, options) {
            continue;
        }
        if exceptions.denies(&lower, char_offset) {
            continue;
        }
        let vowel = finnish::vowel_vowel(segment, byte);
        if exceptions.allows(&lower, char_offset) {
            if vowel && options.vowel_breaks == VowelBreaks::Never {
                continue;
            }
            push_point(
                segment,
                base,
                byte,
                BreakSource::ExplicitException,
                false,
                0,
                out,
            );
            continue;
        }
        if vowel && options.vowel_breaks != VowelBreaks::Always {
            continue;
        }
        let penalty = 10
            + if finnish::next_fragment_starts_with_vowel(segment, byte) {
                5
            } else {
                0
            };
        push_point(
            segment,
            base,
            byte,
            BreakSource::Pattern,
            false,
            penalty,
            out,
        );
    }
}

fn apply_word_exceptions(
    word: &str,
    options: &HyphenationOptions,
    exceptions: &Exceptions,
    points: &mut Vec<HyphenationPoint>,
) {
    let lower = word.to_lowercase();
    for &char_offset in exceptions.deny_offsets(&lower) {
        if let Some(byte) = char_to_byte(word, char_offset) {
            points.retain(|point| point.byte_offset != byte);
        }
    }
    for &char_offset in exceptions.allow_offsets(&lower) {
        let Some(byte) = char_to_byte(word, char_offset) else {
            continue;
        };
        if !valid_fragment(word, byte, options) {
            continue;
        }
        if finnish::vowel_vowel(word, byte) && options.vowel_breaks == VowelBreaks::Never {
            continue;
        }
        push_point(
            word,
            0,
            byte,
            BreakSource::ExplicitException,
            false,
            0,
            points,
        );
    }
}

fn valid_fragment(word: &str, byte: usize, options: &HyphenationOptions) -> bool {
    match finnish::fragment_lengths(word, byte) {
        Some((left, right)) => left >= options.left_min && right >= options.right_min,
        None => false,
    }
}

fn push_point(
    word: &str,
    base: usize,
    byte: usize,
    source: BreakSource,
    compound: bool,
    penalty: u16,
    out: &mut Vec<HyphenationPoint>,
) {
    let Some(char_offset) = byte_to_char(word, byte) else {
        return;
    };
    out.push(HyphenationPoint {
        byte_offset: base + byte,
        char_offset,
        compound_boundary: compound,
        source,
        penalty,
    });
}

fn dedup(points: Vec<HyphenationPoint>) -> Vec<HyphenationPoint> {
    let mut by_offset: BTreeMap<usize, HyphenationPoint> = BTreeMap::new();
    for point in points {
        let replace = match by_offset.get(&point.byte_offset) {
            None => true,
            Some(existing) => {
                point.source > existing.source
                    || (point.source == existing.source
                        && point.compound_boundary
                        && !existing.compound_boundary)
                    || (point.source == existing.source
                        && point.compound_boundary == existing.compound_boundary
                        && point.penalty < existing.penalty)
            }
        };
        if replace {
            by_offset.insert(point.byte_offset, point);
        }
    }
    by_offset.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offsets(word: &str) -> Vec<usize> {
        positions(word).into_iter().map(|p| p.char_offset).collect()
    }

    fn sources(word: &str) -> Vec<BreakSource> {
        positions(word).into_iter().map(|p| p.source).collect()
    }

    #[test]
    fn simple_words_use_the_patterns() {
        assert_eq!(offsets("matka"), vec![3]);
        assert_eq!(offsets("radio"), vec![2]);
        assert_eq!(offsets("paprika"), vec![3, 5]);
    }

    #[test]
    fn a_single_vowel_is_not_left_alone() {
        assert!(offsets("asia").is_empty());
    }

    #[test]
    fn compounds_prefer_the_component_boundary() {
        let points = positions("laivamatka");
        let at_five = points
            .iter()
            .find(|point| point.char_offset == 5)
            .expect("the component boundary after laiva");
        assert!(at_five.compound_boundary);
        assert_eq!(at_five.source, BreakSource::CompoundBoundary);
        assert_eq!(at_five.penalty, 0);
    }

    #[test]
    fn a_compound_is_hyphenated_component_by_component() {
        assert_eq!(offsets("veroprosentti"), vec![2, 4, 7, 11]);
        assert_eq!(
            offsets("tietokoneohjelmointi"),
            vec![3, 5, 7, 9, 11, 14, 18]
        );
    }

    #[test]
    fn a_lemma_modifier_wins_an_ambiguous_compound() {
        let points = positions("autonostaja");
        let compound = points
            .iter()
            .find(|point| point.compound_boundary)
            .expect("auto + nostaja");
        assert_eq!(compound.char_offset, 4);
    }

    #[test]
    fn explicit_exceptions_override_the_vowel_filter() {
        let points = positions("kofeiini");
        assert!(points.iter().any(|point| point.char_offset == 4));
    }

    #[test]
    fn the_pattern_only_fallback_still_produces_offsets() {
        let points = positions_with(
            "veroprosentti",
            &HyphenationOptions::default(),
            &NoCompoundAnalyzer,
            &Exceptions::empty(),
        );
        assert!(!points.is_empty());
        assert!(points.iter().all(|point| !point.compound_boundary));
    }

    #[test]
    fn splitting_reassembles_the_source() {
        let points = positions("tietokoneohjelmointi");
        assert_eq!(
            split("tietokoneohjelmointi", &points).concat(),
            "tietokoneohjelmointi"
        );
    }

    #[test]
    fn multibyte_offsets_are_character_boundaries() {
        let word = "Äänekoski";
        for point in positions(word) {
            assert!(word.is_char_boundary(point.byte_offset));
            assert_eq!(
                byte_to_char(word, point.byte_offset),
                Some(point.char_offset)
            );
        }
    }

    #[test]
    fn an_existing_hyphen_is_not_a_generated_point() {
        assert!(
            positions("linja-auto")
                .iter()
                .all(|point| point.byte_offset != 5)
        );
        assert_eq!(sources("linja-auto"), vec![BreakSource::Pattern; 2]);
    }
}
