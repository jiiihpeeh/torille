//! Lemma-only suggestion and warm-up helpers.

use crate::suggest::levenshtein::levenshtein;
use crate::tags::PartOfSpeech;

use super::{catalogue, is_known_word, is_known_word_fast, lemma_set};

pub(crate) fn gradation_plural_alternate(word: &str) -> Option<(String, String)> {
    let stem = word.strip_suffix("ten")?;
    if stem.is_empty() {
        return None;
    }
    let base = is_known_word(stem)?;
    let lemma = crate::kotus::sanalista().by_word(&base).next()?;
    let pos = crate::tags::PartOfSpeech::parse_all(lemma.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Noun);
    let inflections: Vec<_> = lemma.inflections().collect();
    let forms = crate::expand::inflect_all(&base, &inflections, catalogue(), None, pos, false);
    forms
        .into_iter()
        .find(|f| f.word.ends_with("iden") || f.word.ends_with("itten"))
        .map(|f| (f.word, base))
}

pub fn suggest_over_lemmas(word: &str, max_n: usize, max_distance: usize) -> Vec<String> {
    if word.is_empty() || max_n == 0 {
        return Vec::new();
    }
    let lower = word.to_lowercase();
    // Human-approved answers are exact and cost nothing; the per-keystroke
    // popup uses this lemma-only path too, so it must see the same curated
    // pair (`alottanutkaan` -> `aloittanutkaan`) as the full ranker.
    if let Some((right, _lemma)) = crate::suggest::candidates::reported_correction(&lower) {
        return vec![right.to_string()];
    }
    let word_chars_len = lower.chars().count();
    let cap = max_distance.saturating_add(1);
    let mut scored: Vec<(usize, String)> = Vec::new();
    for lemma in lemma_set().iter() {
        let l_len = lemma.chars().count();
        // Length-difference is a cheap upper bound on Levenshtein;
        // bail before the inner DP for everything too far off.
        if word_chars_len.abs_diff(l_len) > max_distance {
            continue;
        }
        let d = if *lemma == lower.as_str() {
            0
        } else {
            levenshtein(lower.as_str(), lemma, max_distance)
        };
        if d <= max_distance {
            scored.push((d, (*lemma).to_string()));
        }
        // `cap` (max_distance + 1) means "too far"; if we've
        // already collected `max_n` best-candidates and the
        // worst-so-far is at the limit, stop widening the pool.
        // (Optional optimisation: only relevant for very large
        // `max_n`; left as future work.)
        let _ = cap;
    }
    for (surface, _lemma) in crate::expand::derived::forms().iter() {
        let l_len = surface.chars().count();
        if word_chars_len.abs_diff(l_len) > max_distance {
            continue;
        }
        let d = levenshtein(lower.as_str(), surface, max_distance);
        if d <= max_distance {
            scored.push((d, surface.clone()));
        }
    }
    // The colloquial `-s` (`ymmärrätkös`, `mitäs`, `kukas`): the checker
    // flags the word and the correction is the s-less host, so put it
    // first. A word that genuinely ends in `-s` (`kahdeksas`, `ulos`) is
    // a lemma and is accepted, so it is never a flagged misspelling and
    // never reaches this pass.
    let s_less = s_less_host(&lower);
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut out: Vec<String> = scored.into_iter().map(|(_, s)| s).collect();
    if let Some(host) = s_less {
        out.retain(|s| s != &host);
        out.insert(0, host);
    }
    out.truncate(max_n);
    out
}

/// The s-less host of a colloquial `-s` word, when that host is itself a
/// known word: `ymmärrätkös` -> `ymmärrätkö`, `mitäs` -> `mitä`, and even
/// a plain typo like `kissas` -> `kissa`.
///
/// The `-s` (kysymysliite) is spoken-language, so the checker does not
/// accept it as a word; this is the suggestion the on-click popup offers.
/// A word that genuinely ends in `-s` is a sanalista lemma (`kahdeksas`,
/// `ulos`) and is accepted outright, so it never gets here as a
/// misspelling. This only ever *adds* a candidate, so the worst case is
/// one extra suggestion.
pub(crate) fn s_less_host(word: &str) -> Option<String> {
    let host = word.strip_suffix('s')?;
    if host.chars().count() < 2 {
        return None;
    }
    is_known_word_fast(host).map(|_| host.to_string())
}
