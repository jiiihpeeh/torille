//! Golden and invariant tests for Finnish hyphenation.
//!
//! The examples are the Korpela/Kielitoimisto problem cases the design
//! was built around: compound preference, vowel sequences, loanwords and
//! UTF-8 offsets. The invariants run over a sample of the sanalista so a
//! future pattern or morphology change cannot silently produce an offset
//! inside a character or lose a fragment.

use torille::hyphenation::{
    BreakSource, Exceptions, HyphenationOptions, NoCompoundAnalyzer, VowelBreaks, positions,
    positions_with, split,
};

fn offsets(word: &str) -> Vec<usize> {
    positions(word)
        .into_iter()
        .map(|point| point.char_offset)
        .collect()
}

#[test]
fn the_reference_examples_have_the_expected_breaks() {
    assert_eq!(offsets("matka"), vec![3]);
    assert_eq!(offsets("radio"), vec![2]);
    assert_eq!(offsets("paprika"), vec![3, 5]);
    assert_eq!(offsets("housut"), vec![3]);
    assert_eq!(offsets("asia"), Vec::<usize>::new());
    assert_eq!(offsets("veroprosentti"), vec![2, 4, 7, 11]);
    assert_eq!(
        offsets("tietokoneohjelmointi"),
        vec![3, 5, 7, 9, 11, 14, 18]
    );
    assert_eq!(offsets("varaosat"), vec![2, 4]);
    assert_eq!(offsets("äidinkieli"), vec![2, 5, 8]);
}

#[test]
fn a_compound_boundary_is_marked_and_ranked_first() {
    let points = positions("laivamatka");
    let boundary = points
        .iter()
        .find(|point| point.char_offset == 5)
        .expect("laiva|matka");
    assert!(boundary.compound_boundary);
    assert_eq!(boundary.source, BreakSource::CompoundBoundary);
    assert_eq!(boundary.penalty, 0);
    assert!(
        points
            .iter()
            .filter(|point| point.source == BreakSource::Pattern)
            .all(|point| point.penalty > boundary.penalty)
    );
}

#[test]
fn an_ambiguous_compound_prefers_the_lemma_modifier() {
    let points = positions("autonostaja");
    let compound: Vec<_> = points
        .iter()
        .filter(|point| point.compound_boundary)
        .collect();
    assert_eq!(
        compound
            .iter()
            .map(|point| point.char_offset)
            .collect::<Vec<_>>(),
        vec![4]
    );
}

#[test]
fn explicit_loanword_exceptions_allow_a_vowel_break() {
    assert!(offsets("kofeiini").contains(&4));
    let points = positions_with(
        "kofeiini",
        &HyphenationOptions {
            vowel_breaks: VowelBreaks::Never,
            ..HyphenationOptions::default()
        },
        &torille::hyphenation::LexicalCompoundAnalyzer,
        Exceptions::finnish(),
    );
    assert!(!points.iter().any(|point| point.char_offset == 4));
}

#[test]
fn the_pattern_fallback_still_reassembles() {
    let word = "veroprosentti";
    let points = positions_with(
        word,
        &HyphenationOptions::default(),
        &NoCompoundAnalyzer,
        &Exceptions::empty(),
    );
    assert!(!points.is_empty());
    assert!(points.iter().all(|point| !point.compound_boundary));
    assert_eq!(split(word, &points).concat(), word);
}

#[test]
fn a_sampled_lexicon_word_never_loses_or_duplicates_a_character() {
    let options = HyphenationOptions::default();
    let exceptions = Exceptions::finnish();
    let mut checked = 0usize;
    for (index, entry) in torille::sanalista().iter().enumerate() {
        if index % 97 != 0 {
            continue;
        }
        let word = entry.word();
        let points = positions_with(
            word,
            &options,
            &torille::hyphenation::LexicalCompoundAnalyzer,
            exceptions,
        );
        let mut previous = 0usize;
        for point in &points {
            assert!(
                word.is_char_boundary(point.byte_offset),
                "{word}: offset {} is not a character boundary",
                point.byte_offset
            );
            assert!(
                point.byte_offset > previous,
                "{word}: offsets are not strictly increasing"
            );
            assert!(point.byte_offset < word.len(), "{word}: break at end");
            let left = word[..point.byte_offset].chars().count();
            let right = word[point.byte_offset..].chars().count();
            assert!(left >= options.left_min, "{word}: short left fragment");
            assert!(right >= options.right_min, "{word}: short right fragment");
            previous = point.byte_offset;
        }
        assert_eq!(
            split(word, &points).concat(),
            word,
            "{word}: split lost characters"
        );
        checked += 1;
    }
    assert!(checked > 500, "only {checked} lemmas were checked");
}

#[test]
fn a_soft_hyphen_is_an_explicit_break_and_stays_in_the_source() {
    let word = "kissa\u{00AD}koira";
    let points = positions(word);
    let soft = *points
        .iter()
        .find(|point| point.source == BreakSource::SoftHyphen)
        .expect("supplied soft hyphen");
    assert_eq!(soft.char_offset, 6);
    assert_eq!(split(word, &[soft]), vec!["kissa\u{00AD}", "koira"]);
    assert_eq!(split(word, &points).concat(), word);
}

#[test]
fn a_soft_hyphen_does_not_bypass_the_fragment_minimum() {
    for word in ["k\u{00AD}issa", "kissa\u{00AD}k"] {
        assert!(
            !positions(word)
                .iter()
                .any(|point| point.source == BreakSource::SoftHyphen),
            "{word:?}"
        );
    }
}

#[test]
fn vowel_breaks_never_returns_a_vowel_boundary() {
    let word = "kofeiini";
    let points = positions_with(
        word,
        &HyphenationOptions {
            vowel_breaks: VowelBreaks::Never,
            ..HyphenationOptions::default()
        },
        &torille::hyphenation::LexicalCompoundAnalyzer,
        &Exceptions::empty(),
    );
    for point in points {
        let left = word[..point.byte_offset].chars().next_back().unwrap();
        let right = word[point.byte_offset..].chars().next().unwrap();
        assert!(
            !(torille::is_vowel(left) && torille::is_vowel(right)),
            "{word}: vowel break at {}",
            point.byte_offset
        );
    }
}
