//! The per-textbox stateful entry points: `check_fast` and
//! `suggest_with_state`.
//!
//! Split out of `suggest/mod.rs`; both are re-exported from `suggest`.

use std::collections::HashSet;

use crate::expand::{BasesReport, find_any_base_with};

use super::casing::{capitalize, match_case};
use super::known::{
    catalogue, hyphenated_lexeme, is_known_compound, is_known_word, is_known_word_no_compound,
    lemma_set, lexicon,
};
use super::levenshtein::levenshtein;
use super::{CheckOutcome, classify, suggest, suggest_detailed};

/// Spell-check with the lemma fast path. Used by the per-textbox
/// `analyze_text`: lemma hits and `proper_nouns` hits return
/// immediately, every other word falls through to [`check`] for the
/// full prefix walk over the lexicon.
///
/// Returns `None` for unknown words (the per-textbox caller treats
/// `None` as "misspelling" and asks for [`suggest`] candidates). For
/// known words, the underlying [`CheckOutcome`] is preserved so a
/// future caller can ask "which lemma inflects to this?" without
/// having to redo the work.
pub fn check_fast(word: &str, proper_nouns: &HashSet<String>) -> Option<CheckOutcome> {
    let lower = word.to_lowercase();
    if lemma_set().contains(lower.as_str()) {
        return Some(CheckOutcome::Lemma);
    }
    if proper_nouns.contains(&lower) {
        return Some(CheckOutcome::Lemma);
    }
    // A standard hyphenated spelling (`linja-auto`) is not optional. Do this
    // before the engine, which would otherwise accept the closed compound
    // `linjaauto` from its parts.
    if hyphenated_lexeme(&lower).is_some() {
        return None;
    }
    // The non-compound fast path is now cheap for degree forms too, and it
    // names the right positive: `salaperäisempi` is `salaperäinen`, not the
    // `empiä` the engine's head/tail walk can pick.
    if let Some(base) = is_known_word_no_compound(&lower) {
        return Some(inflected_outcome(base));
    }
    // Compound recognition is much cheaper than the full prefix walk and
    // covers the long words the walk is worst at: `kahvikekkereitä` costs
    // ~90 ms here versus ~1.7 s for `find_any_base_with`. Use the
    // non-compound probe above rather than `is_known_word`, so the compound
    // sides are not checked twice.
    if let Some(base) = is_known_compound(&lower) {
        return Some(inflected_outcome(base));
    }
    // The inflect check is a yes/no question: "is there a base
    // whose paradigm emits this word?". The full
    // `find_bases_with` walks every candidate the suggestion pool
    // would want; we don't need that here. `find_any_base_with`
    // short-circuits on the first match. The wasm's typing path
    // fires this per misspell; shaving 3000 candidate inflections
    // per call is the difference between a background demote
    // that lands in milliseconds and one that blocks the worker.
    //
    // Multiple bases are possible (homonyms, overlapping
    // paradigms). The inflect check accepts the word as soon as
    // ANY base matches, so short-circuiting is correct. The
    // suggestion pool is a separate code path; it uses the full
    // `find_bases_with`.
    let cat = catalogue();
    let lexicon = lexicon();
    let bases = find_any_base_with(lexicon, cat, &lower);
    if bases.is_empty() {
        // The engine walk has no compound analysis. A compound whose head
        // inflects normally (`todistelukykynsä` = `todistelu` + `kyvynsä`)
        // is still a word; the fast detector names the head.
        if let Some(base) = is_known_word(word) {
            return Some(inflected_outcome(base));
        }
        // No common-word analysis: a known proper name is still a word.
        if crate::expand::proper_names::contains_capitalised(word) {
            return Some(CheckOutcome::Lemma);
        }
        return None;
    }
    let report = BasesReport {
        direct: Vec::new(),
        bases,
        neighbours: Vec::new(),
        forms: Vec::new(),
    };
    match classify(report) {
        CheckOutcome::Unknown => None,
        other => Some(other),
    }
}

/// Build the `Inflected` outcome for a base named by the fast detector.
///
/// A derived base (`laitilalainen`) has no sanalista row; accept the word
/// without claiming a lexicon lemma. A compound names its head, which does
/// have a row.
fn inflected_outcome(base: String) -> CheckOutcome {
    if let Some(row) = crate::kotus::sanalista().by_word(&base).next() {
        return CheckOutcome::Inflected(vec![row.word()]);
    }
    CheckOutcome::Inflected(Vec::new())
}

/// Spell suggestions with a fallback to the per-textbox learned
/// proper-noun set. The plain [`suggest`] path runs first; if it
/// returns no candidates (the misspelling is too far from every
/// lexicon entry), a Damerau scan over `proper_nouns` produces the
/// fallback list.
///
/// Keeps the ranker logic in [`suggest`] simple -- it never has to
/// know about state -- at the cost of an extra `levenshtein` pass
/// over a small set in the rare empty-`suggest` case. A future slice
/// can plumb `proper_nouns` directly into `find_bases_with` if the
/// set ever grows large enough to matter.
pub fn suggest_with_state(
    word: &str,
    max_n: usize,
    max_distance: usize,
    proper_nouns: &HashSet<String>,
) -> Vec<String> {
    if proper_nouns.is_empty() {
        return suggest(word, max_n, max_distance);
    }
    let lower = word.to_ascii_lowercase();
    // Lemma-based candidates from the lib. May include
    // Finnish words that happen to be at low edit distance
    // from the input but are not what the user meant (e.g.
    // `helaini` for a typo of `Helsinki`).
    let out: Vec<(usize, String)> = suggest_detailed(&lower, max_n, max_distance)
        .into_iter()
        .map(|s| (s.edit_distance, s.word))
        .collect();
    // Proper-noun candidates: learned names the user has
    // typed before. These are the right answer when the
    // user is just typo-ing a known name.
    let mut pn: Vec<(usize, String)> = proper_nouns
        .iter()
        .filter_map(|p| {
            let d = levenshtein(&lower, p, max_distance);
            if d <= max_distance {
                Some((d, capitalize(p)))
            } else {
                None
            }
        })
        .collect();
    if pn.is_empty() && out.is_empty() {
        return Vec::new();
    }
    // De-dupe (a proper noun that's also a lemma would
    // appear in both pools).
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_, w) in &out {
        seen.insert(w.to_lowercase());
    }
    pn.retain(|(_, w)| seen.insert(w.to_lowercase()));
    // Merge. The proper noun wins ties: a learned name is
    // almost always a better answer than an unrelated
    // Finnish lemma at the same edit distance.
    let mut merged: Vec<(usize, String)> = Vec::with_capacity(out.len() + pn.len());
    let mut i = 0;
    let mut j = 0;
    while i < out.len() || j < pn.len() {
        let take_pn = match (i < out.len(), j < pn.len()) {
            (false, true) => true,
            (true, false) => false,
            (true, true) => {
                let (d_l, _) = &out[i];
                let (d_p, _) = &pn[j];
                // Tie or proper noun is closer -> PN.
                *d_p <= *d_l
            }
            (false, false) => break,
        };
        if take_pn {
            merged.push(pn[j].clone());
            j += 1;
        } else {
            merged.push(out[i].clone());
            i += 1;
        }
    }
    merged.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    merged
        .into_iter()
        .take(max_n)
        .map(|(_, s)| match_case(word, &s))
        .collect()
}
