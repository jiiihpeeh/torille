//! The productive `-kas`/`-käs` adjective recogniser.

use crate::kotus::Inflection;
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

use crate::expand::inflect_all;
use crate::expand::lexicon_index::LexiconIndex;

/// The derived `-kas`/`-käs` adjective (and its degrees) a surface belongs to.
///
/// `-kas`/`-käs` is the productive NOUN -> ADJECTIVE rule (`voima` ->
/// `voimakas`, `äänne` -> `äännekäs`), but the derived adjective is usually
/// not a sanalista lemma, so it is in neither the stem maps nor
/// `find_bases`. Reconstruct the positive from the degree/normal surface,
/// require its stem to be a noun lemma, then inflect the whole class-41
/// adjective and verify the surface. This also covers the productive
/// comparative (`äännekkäämpi`) and superlative (`äännekkäin`).
pub fn derived_kas_base(
    word: &str,
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
) -> Option<String> {
    for candidate in kas_candidates(word) {
        let Some(stem) = candidate
            .strip_suffix("käs")
            .or_else(|| candidate.strip_suffix("kas"))
        else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        let stem_is_noun = crate::kotus::sanalista()
            .by_word(stem)
            .next()
            .is_some_and(|row| {
                crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                    .contains(&PartOfSpeech::Noun)
            });
        if !stem_is_noun {
            continue;
        }
        let forms = inflect_all(
            &candidate,
            &[Inflection {
                class: 41,
                gradation: Some(crate::kotus::Gradation(1)),
            }],
            cat,
            lexicon,
            PartOfSpeech::Adjective,
            true,
        );
        if forms.iter().any(|f| f.word == word) {
            return Some(candidate);
        }
    }
    None
}

/// Candidate `-kas`/`-käs` positives for a surface.
///
/// A surface can be the adjective itself, a case form of it, or a degree
/// (`-kkäämpi`/`-kkäin`). Strip the possible case ending, then undo the
/// degree or the kk gradation and put the positive suffix back.
fn kas_candidates(word: &str) -> Vec<String> {
    fn add(s: &str, out: &mut Vec<String>) {
        if s.ends_with("kas") || s.ends_with("käs") {
            out.push(s.to_string());
        }
        // Comparative and superlative stems after a case ending has been
        // stripped: `äännekkäämmä` -> `äännekäs`, `voimakkaimma` ->
        // `voimakas`.
        for (needle, replacement) in [
            ("kkäämm", "käs"),
            ("kkaamm", "kas"),
            ("kkäimm", "käs"),
            ("kkaimm", "kas"),
        ] {
            if let Some(at) = s.find(needle) {
                out.push(format!("{}{replacement}", &s[..at]));
            }
        }
        // A case form of the positive itself: `äännekkäässä` loses `ssä`
        // first, leaving `äännekkää`, whose kk belongs to the suffix.
        for (needle, replacement) in [("kkää", "käs"), ("kkaa", "kas")] {
            if let Some(prefix) = s.strip_suffix(needle) {
                out.push(format!("{prefix}{replacement}"));
            }
        }
    }

    let mut out = Vec::new();
    add(word, &mut out);
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        if let Some(stem) = word.strip_suffix(ending) {
            add(stem, &mut out);
        }
    }
    out.sort();
    out.dedup();
    out
}
