//! Class 50/51 compounds must have one paradigm source.
//!
//! A class-50/51 row is a compound by declaration, so its paradigm is built
//! from the two parts (`iso` + `isä`) and cannot exist without the lexicon
//! that finds them. `inflect_all` used to take an explicit
//! `Option<&LexiconIndex>`, and the cheap recognition path passed `None` while
//! the full walk passed the index. With `None` a class-50/51 lemma fell
//! through to the catalogue model, which aligns the compound against
//! `isoisä`/`kylmä` and lands gradation inside it: `kylmätila` produced
//! `kylmädilillä` and 394 forms instead of 155, and the same word got four
//! different answers depending on which entry point asked.
//!
//! The invariants here are:
//!
//! - a paradigm built without a lexicon never adds a form the indexed one does
//!   not have (the fabricated forms are a strict superset violation);
//! - `check`, `check_fast`, `check_normal` and `is_known_word` agree on the
//!   compound's own forms;
//! - the fabricated spellings are unknown everywhere, including at
//!   `check_normal_fast`, which answers from the cheap gates alone.

use std::collections::HashSet;

use torille::expand::check_normal::{NormalizedOutcome, check_normal, check_normal_fast};
use torille::expand::{LexiconIndex, inflect_all_in};
use torille::kotus::{Inflection, Lemma, lemmas, sanalista};
use torille::tags::PartOfSpeech;

/// Every sanalista lemma with a class-50 or class-51 reading.
fn compound_lemmas() -> Vec<&'static Lemma<'static>> {
    lemmas()
        .iter()
        .filter(|lemma| {
            lemma
                .inflections
                .iter()
                .any(|inflection| matches!(inflection.class, 50 | 51))
        })
        .collect()
}

/// The lemma's class-50/51 readings only, so the sweep cannot be satisfied by
/// a second, non-compound class (`ahkeraliisa` is `9, 50`).
fn compound_inflections(lemma: &Lemma<'_>) -> Vec<Inflection> {
    lemma
        .inflections
        .iter()
        .copied()
        .filter(|inflection| matches!(inflection.class, 50 | 51))
        .collect()
}

fn lemmas_of(word: &str) -> &'static Lemma<'static> {
    let row = sanalista()
        .by_word(word)
        .next()
        .unwrap_or_else(|| panic!("{word} is not in the sanalista"));
    &lemmas()[row.row() as usize]
}

/// The entry points that answer "is this a known word?", as named results.
///
/// `check_normal_fast` is the bounded pipeline: it answers from the cheap
/// gates alone and is documented to miss a word that only the engine gate
/// recognises. It is included only where the expected answer is "unknown"
/// (the fabricated table); the parity test asks the four full entry points.
fn unknown_at(word: &str, proper: &HashSet<String>, bounded: bool) -> Vec<&'static str> {
    let mut unknown = Vec::new();
    if !torille::suggest::check(word).is_known() {
        unknown.push("check");
    }
    if !torille::suggest::check_fast(word, proper).is_some_and(|outcome| outcome.is_known()) {
        unknown.push("check_fast");
    }
    if check_normal(word, false) == NormalizedOutcome::Unknown {
        unknown.push("check_normal");
    }
    if bounded && check_normal_fast(word, false) == NormalizedOutcome::Unknown {
        unknown.push("check_normal_fast");
    }
    if torille::suggest::is_known_word(word).is_none() {
        unknown.push("is_known_word");
    }
    unknown
}

/// Without a lexicon a class-50/51 paradigm cannot be built, and it must fail
/// closed rather than fall through to the catalogue model. "Cannot answer" is
/// a subset of the indexed answer, never a different one.
#[test]
fn a_paradigm_without_a_lexicon_never_adds_a_form() {
    let cat = torille::spec::Catalogues::embedded();
    let index = LexiconIndex::new(lemmas());
    let mut checked = 0usize;
    for lemma in compound_lemmas() {
        let inflections = compound_inflections(lemma);
        let with = inflect_all_in(
            lemma.word,
            &inflections,
            cat,
            Some(&index),
            PartOfSpeech::Noun,
            true,
        );
        let without = inflect_all_in(
            lemma.word,
            &inflections,
            cat,
            None,
            PartOfSpeech::Noun,
            true,
        );
        if without.is_empty() {
            continue;
        }
        let indexed: HashSet<&str> = with.iter().map(|form| form.word.as_str()).collect();
        for form in &without {
            assert!(
                indexed.contains(form.word.as_str()),
                "{} fabricates {:?} without a lexicon; the indexed paradigm has {} forms",
                lemma.word,
                form.word,
                with.len()
            );
        }
        checked += 1;
    }
    // Degree forms are emitted without any inflection, so a handful of
    // adjectives still answer; the point is that the adds are those, not a
    // catalogue-built compound paradigm.
    assert!(
        checked < compound_lemmas().len(),
        "every compound lemma answered without a lexicon; the class-50/51 \
         path is not failing closed"
    );
}

/// The fabricated spellings in the review's E4 table are unknown everywhere.
/// They are `Plain` forms the old `None` path built by aligning the compound
/// against the class model.
#[test]
fn a_fabricated_compound_form_is_unknown_everywhere() {
    let proper = HashSet::new();
    for word in [
        "kylmädilillä",
        "syväasfaltdeiksi",
        "ahkeraldin",
        "nuori-idenkuun",
    ] {
        assert_eq!(
            unknown_at(word, &proper, true),
            [
                "check",
                "check_fast",
                "check_normal",
                "check_normal_fast",
                "is_known_word"
            ],
            "{word}"
        );
    }
}

/// The four full entry points agree on the compound's own forms. The sample is
/// a stride over the 696 class-50/51 lemmas (the full sweep is hours at the
/// reference walk's cost per word); the exhaustive per-form check is
/// `a_paradigm_without_a_lexicon_never_adds_a_form` above.
#[test]
fn every_entry_point_agrees_on_a_compound_paradigm() {
    let cat = torille::spec::Catalogues::embedded();
    let index = LexiconIndex::new(lemmas());
    let proper = HashSet::new();
    let mut checked = 0usize;
    for (n, lemma) in compound_lemmas().iter().enumerate() {
        if n % 10 != 0 {
            continue;
        }
        let forms = inflect_all_in(
            lemma.word,
            &lemma.inflections,
            cat,
            Some(&index),
            PartOfSpeech::Noun,
            true,
        );
        let standalone: Vec<&str> = forms
            .iter()
            .filter(|form| !form.combining && form.word != lemma.word)
            .map(|form| form.word.as_str())
            .collect();
        let Some(first) = standalone.first() else {
            continue;
        };
        // The first oblique form and the last (usually a possessive or the
        // comitative) cover both ends of the paradigm.
        for word in [*first, *standalone.last().unwrap()] {
            assert_eq!(
                unknown_at(word, &proper, false),
                [] as [&str; 0],
                "{word} (a form of {})",
                lemma.word
            );
            checked += 1;
        }
    }
    assert!(checked >= 100, "only {checked} compound forms were checked");
}

/// The indexed paradigms are pinned by count: a change here means the
/// compound engine changed, not that the lexicon was supplied or not.
#[test]
fn the_compound_form_counts_are_the_index_paths() {
    let cat = torille::spec::Catalogues::embedded();
    let index = LexiconIndex::new(lemmas());
    for (word, expected) in [("kylmätila", 155), ("nuori-isäntä", 176)] {
        let lemma = lemmas_of(word);
        let inflections = compound_inflections(lemma);
        let with = inflect_all_in(
            word,
            &inflections,
            cat,
            Some(&index),
            PartOfSpeech::Noun,
            true,
        );
        let without = inflect_all_in(word, &inflections, cat, None, PartOfSpeech::Noun, true);
        assert_eq!(with.len(), expected, "{word}: indexed paradigm size");
        assert!(
            without.is_empty(),
            "{word}: {} forms without a lexicon",
            without.len()
        );
    }
}
