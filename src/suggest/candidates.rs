//! Candidate generation and ranking: `suggest` and `suggest_detailed`.
//!
//! Split out of `suggest/mod.rs`; the public functions are re-exported from
//! `suggest` so their paths do not change.

use super::detail::{Suggestion, SuggestionSource};
use crate::expand::{find_any_base_with, find_bases_with_form_filter};
use crate::tags::Confidence;

use super::casing::match_case;
use super::known::{
    catalogue, gradation_plural_alternate, is_known_compound_fast, is_known_word_fast,
    is_known_word_no_compound, lexicon, s_less_host,
};
use super::levenshtein::{keyboard_bonus, levenshtein};

/// Restore the input's capitalization on every returned suggestion.
fn apply_input_case(input: &str, mut suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
    for suggestion in &mut suggestions {
        suggestion.word = match_case(input, &suggestion.word);
    }
    suggestions
}

/// A missing doubled vowel (`kiväriään` for `kivääriään`) is a more
/// specific typo than a same-distance substitution, so it wins the tie.
const DOUBLED_LETTER_BONUS: f32 = 0.1;

/// Human-approved morphological corrections that edit distance alone cannot
/// surface: the wrong surface is not a typo in spelling but a different
/// inflection of the same verb, so it is several edits away and alphabetically
/// behind unrelated candidates. Keep this tiny and explicit.
const REPORTED_CORRECTIONS: &[(&str, &str, &str)] = &[
    // The clitic form of the past participle: `aloittaa` + `-nut` + `-kaan`.
    // The general clitic pre-pass also reaches this, but it still runs the
    // host's candidate walk; the curated answer is instant.
    ("alottanutkaan", "aloittanutkaan", "aloittaa"),
    // Frequentative-looking past 1sg instead of `ahertaa`'s `ahersin`.
    ("ahertelin", "ahersin", "ahertaa"),
    // A reported reading error: the intended word is the present active
    // participle of `liekehtiä`, not the nearer-looking `litisevä`.
    ("liektisevä", "liekehtivä", "liekehtiä"),
    // A transposition typo whose head/tail walk was taking seconds; the
    // curated path is the fast, deterministic answer.
    ("silmäränpäyksen", "silmänräpäyksen", "silmänräpäys"),
    // `-aikainen` loses its `i`; the standard compound is `vanhanaikainen`.
    ("vanhanaikanen", "vanhanaikainen", "vanhanaikainen"),
    // `salamyhkää` is not a standard form; the reported intended word is the
    // adverb `salamyhkäisesti`, which is its own sanalista lemma.
    ("salamyhkää", "salamyhkäisesti", "salamyhkäisesti"),
    // The frequentative of `maiskuttaa` loses the extra `a`.
    ("maiskauttelee", "maiskuttelee", "maiskutella"),
    // A nonstandard frequentative past inserts `hd` into `kolauttaa`'s
    // `kolautti`; the two-deletion repair is several edits behind unrelated
    // head-1 candidates and the walk costs seconds.
    ("kolahdutti", "kolautti", "kolauttaa"),
    // Modern Finnish writes the conjunction as two words; the fused
    // spelling is a misspelling, not a lexicalised alternative.
    ("ennenkuin", "ennen kuin", "ennen"),
    ("aivankuin", "aivan kuin", "aivan"),
    ("avainkuin", "aivan kuin", "aivan"),
    ("niinkuin", "niin kuin", "niin"),
    // `aivotus` is rejected as a variant; the standard noun is `aivoitus`,
    // whose genitive plural `aivoitusten` is the reported correction.
    ("aivotusten", "aivoitusten", "aivoitus"),
    // Double-`t` variants of `eltaantua`; Wiktionary gives the single-`t`
    // spelling and `eltaantuminen`.
    ("elttaantuneen", "eltaantuneen", "eltaantua"),
    ("elttaantuminen", "eltaantuminen", "eltaantua"),
    // `pingoittaa` is a nonstandard alt form of `pingottaa`.
    ("pingoittuivat", "pingottuivat", "pingottaa"),
    // `rusoittaa` is not a standard lemma; Wiktionary gives `rusottaa`.
    ("rusoittava", "rusottava", "rusottaa"),
];

pub(super) fn reported_correction(word: &str) -> Option<(&'static str, &'static str)> {
    REPORTED_CORRECTIONS
        .iter()
        .find(|(wrong, _, _)| *wrong == word)
        .map(|(_, right, lemma)| (*right, *lemma))
}

/// The modern form of an archaic plural genitive with a third-person
/// possessive suffix: `palvelijainsa` (`palvelijain` + `-sa`) ->
/// `palvelijoidensa` (`palvelijoiden` + `-sa`).
///
/// The old `-in` genitive plural plus possessive is several edits behind
/// unrelated one-letter fixes, so the ranker never brings it into the
/// popup. Recover the lemma from the stem, inflect its possessive
/// paradigm, and pick the genitive plural third-person form directly.
fn archaic_genitive_plural_possessive(word: &str) -> Option<(String, String)> {
    let stem = word
        .strip_suffix("insa")
        .or_else(|| word.strip_suffix("insä"))?;
    if stem.chars().count() < 3 {
        return None;
    }
    let base = is_known_word_fast(stem)?;
    let lemma = crate::kotus::sanalista().by_word(&base).next()?;
    let pos = crate::tags::PartOfSpeech::parse_all(lemma.part_of_speech())
        .first()
        .copied()
        .unwrap_or(crate::tags::PartOfSpeech::Noun);
    let inflections: Vec<_> = lemma.inflections().collect();
    let forms = crate::expand::inflect_all(
        &base,
        &inflections,
        catalogue(),
        None,
        pos,
        /* possessives */ true,
    );
    forms
        .into_iter()
        .find(|f| {
            f.readings.iter().any(|t| {
                t.case == Some(crate::tags::Case::Genitive)
                    && t.number == Some(crate::tags::Number::Plural)
                    && t.possessive == Some(crate::tags::Possessive::Third)
            })
        })
        .map(|f| (f.word, base))
}

/// Repair a plural genitive written with the nonstandard `-jen` ending.
///
/// Finnish `-ia`/`-iä` nouns form the plural genitive in `-iden` (or
/// `-itten`); `komppaniojen` is a common mis-spelling of
/// `komppanioiden`/`vapaakomppanioiden`. The compound is not a lemma, so
/// the head/tail walks never generate the whole word and an ordinary
/// suggestion comes back empty. This is a zero-candidate repair: rewrite
/// the ending and accept it only when the repaired word is a known form
/// (including a known compound).
fn repaired_plural_genitive(word: &str) -> Option<String> {
    let stem = word.strip_suffix("jen")?;
    if stem.chars().count() < 3 {
        return None;
    }
    let candidate = format!("{stem}iden");
    is_known_word_fast(&candidate)
        .is_some()
        .then_some(candidate)
}

/// Edit-distance suggestions for `word`, ranked by ascending distance.
///
/// Up to `max_n` candidates are returned, each within `max_distance`
/// Levenshtein units of `word`, with one exception: a human-curated
/// correction from [`REPORTED_CORRECTIONS`] is returned even when it is
/// farther away, because the table is the licence. Its
/// [`Suggestion::edit_distance`] still reports the true distance. The
/// candidate set is `find_bases`'s
/// `neighbours` -- the lemmas that share a head, tail, or short prefix
/// with `word` -- augmented with the lemma of any direct hit, since a
/// typo on a known lemma is the common case.
///
/// This is a thin wrapper over [`suggest_detailed`] that throws away every
/// [`Suggestion`] field except the surface word. The hunspell-compatible C
/// API in `wrapper/` and the existing tool `tools/src/bin/suggest_correct`
/// both call this. New callers should reach for [`suggest_detailed`] so
/// they can read the morphology / source metadata.
pub fn suggest(word: &str, max_n: usize, max_distance: usize) -> Vec<String> {
    suggest_detailed(word, max_n, max_distance)
        .into_iter()
        .map(|s| s.word)
        .collect()
}

/// The detailed spelling pipeline.
///
/// Returns up to `max_n` [`Suggestion`]s, each within `max_distance`
/// Levenshtein units of `word`. The candidate set is the same one
/// [`suggest`] uses -- every lemma that shares a head, tail, or short
/// prefix with `word`, plus every inflected form generated during the
/// head/tail/prefix walks.
///
/// Ordering matches [`suggest`] -- Levenshtein ascending, alphabetical
/// tie-break.
pub fn suggest_detailed(word: &str, max_n: usize, max_distance: usize) -> Vec<Suggestion> {
    if word.is_empty() || max_n == 0 {
        return Vec::new();
    }
    let original = word;
    // The bounded walks inside `find_bases_with` and the
    // candidates it returns are all lowercase (every lemma
    // in the sanalista is). Lowercasing here keeps the edit-
    // distance comparison case-stable so a sentence-initial
    // `Baromeetteri` (old spelling) sees the same distance
    // to `barometri` as `baromeetteri` would. The default
    // demo `maxDistance=2` would still filter the modern
    // spelling out, but a distance of 3 is now enough to
    // surface it -- one less thing the user has to know
    // about their input case.
    let word = word.to_ascii_lowercase();
    // A reported correction is a human-approved answer. Return it before the
    // head/tail/prefix walks: `silmäränpäyksen` has a two-edit transposition
    // but the bounded walk spent seconds reaching its neighbours.
    if let Some((right, lemma)) = reported_correction(&word) {
        // This candidate is licensed by the table rather than by the caller's
        // distance budget, so it is returned even when `max_distance` is small.
        // Its distance is still the true one: a consumer reading
        // `Suggestion::edit_distance` must not be told that a four-edit repair
        // is an exact match.
        let edit_distance = levenshtein(&word, right, usize::MAX);
        return apply_input_case(
            original,
            vec![Suggestion {
                word: right.to_string(),
                edit_distance,
                lemma: Some(lemma.to_string()),
                source: SuggestionSource::GeneratedForm {
                    lemma: lemma.to_string(),
                },
                readings: Vec::new(),
                confidence: Confidence::Plain,
                original_rank: 0,
                existing_score: 1.0 / (1.0 + edit_distance as f32) + 0.5,
            }],
        );
    }
    // A correctly written compound needs no correction. Short-circuit before
    // the head-1 walk: `kahvikekkereitä` takes ~30 s there. Use the fast
    // detector here: the full one sends the head through the engine, which
    // costs seconds on an ordinary typo (`kiirempää` was ~3 s before this),
    // while the engine-free pass answers the same question for compounds the
    // form and stem maps can already name. `is_known_compound` deliberately
    // rejects a closed spelling of a lexically hyphenated compound, so
    // `linjaauto` still reaches the engine and gets its `linja-auto`
    // suggestion.
    if is_known_compound_fast(&word).is_some() {
        return Vec::new();
    }
    // A compound whose head is inflected with `-jen` (`vapaakomppaniojen`)
    // is not a lemma, so the head/tail walk has no candidate to repair:
    // the whole surface is missing from the forms the walk generates.
    // Rewrite the plural genitive to `-iden` and accept only a known form.
    if is_known_word_fast(&word).is_none()
        && let Some(right) = repaired_plural_genitive(&word)
    {
        let edit_distance = levenshtein(&word, &right, usize::MAX);
        if edit_distance <= max_distance {
            return apply_input_case(
                original,
                vec![Suggestion {
                    word: right.clone(),
                    edit_distance,
                    lemma: Some(right.clone()),
                    source: SuggestionSource::GeneratedForm { lemma: right },
                    readings: Vec::new(),
                    confidence: Confidence::Plain,
                    original_rank: 0,
                    existing_score: 1.0 / (1.0 + edit_distance as f32) + 0.5,
                }],
            );
        }
    }
    // An archaic `-in` genitive plural plus a possessive suffix is far
    // behind unrelated one-edit fixes in the normal ranker
    // (`palvelijainsa` -> `palvelijoidensa` sits around rank 50). The
    // morphological repair is exact and cheap, so answer it directly.
    if is_known_word_fast(&word).is_none()
        && let Some((right, lemma)) = archaic_genitive_plural_possessive(&word)
    {
        let edit_distance = levenshtein(&word, &right, usize::MAX);
        if edit_distance <= max_distance {
            return apply_input_case(
                original,
                vec![Suggestion {
                    word: right,
                    edit_distance,
                    lemma: Some(lemma.clone()),
                    source: SuggestionSource::GeneratedForm { lemma },
                    readings: Vec::new(),
                    confidence: Confidence::Plain,
                    original_rank: 0,
                    existing_score: 1.0 / (1.0 + edit_distance as f32) + 0.5,
                }],
            );
        }
    }
    // A clitic attaches to the whole inflected form, not to a lemma: the
    // candidate pool generates forms, so `aloittanutkaan` (the past
    // participle + `-kaan`) is never in it, and the cliticized input spends
    // the whole search to find `aloittanuttaan`. Strip the clitic, suggest
    // on the host, and put the same suffix back. Only do this when the full
    // word is unknown (a known `kokin` is a form of `kokki`, not `ko`+`kin`).
    let mut clitic_candidates: Vec<Suggestion> = Vec::new();
    if let Some((host, _clitic)) = crate::expand::clitic::strip_clitic(&word)
        && host.chars().count() >= 3
    {
        let suffix = &word[host.len()..];
        let host_known = is_known_word_no_compound(&host).is_some();
        let mut out = suggest_detailed(&host, max_n, max_distance);
        if !out.is_empty() {
            for suggestion in &mut out {
                suggestion.word.push_str(suffix);
            }
            if host_known {
                // A real clitic form with a known host: the cliticised
                // reading is the only sensible one, and skipping the full
                // search is what makes this fast.
                return apply_input_case(original, out);
            }
            // The host is itself a misspelling: keep the normal search for
            // its own corrections (`metafysiikko` -> `metafyysikko`) and
            // merge the cliticised candidates below.
            clitic_candidates = out;
        }
    }
    let cat = catalogue();
    let lexicon = lexicon();
    // Filter the generated forms while the engine still owns them. The
    // head-1 walk produces every form of up to 3000 lemmas; only the ones
    // inside the caller's edit budget can ever become suggestions, and
    // cloning the rest costs more than the distance check.
    let report = find_bases_with_form_filter(lexicon, cat, &word, |surface| {
        levenshtein(&word, surface, max_distance) <= max_distance
    });

    // The candidate pool: every lemma that could plausibly be a one-typo
    // fix, plus every inflected form generated during the head/tail/
    // prefix walks. Many misspellings are within one edit of an
    // inflected form whose lemma is too far from the input for the
    // lemma-only path to consider; e.g. "jooksee" -> "juoksee" (the
    // form is one insertion away from the input, but the lemma
    // "juosta" is six edits away).
    //
    // Insert order matches the existing `suggest` (which sorted
    // alphabetically, then dedup'd keeping the first occurrence). Each
    // Keep the candidate source on the Suggestion record.
    let mut candidates: Vec<Suggestion> = Vec::with_capacity(
        report.bases.len() + report.neighbours.len() + report.direct.len() + report.forms.len(),
    );

    // 1. Lemmas that inflect to the input (bases). They are full words
    //    in the sanalista -- a homonym form or an irregular lemma -- so
    //    their confidence is `Plain` and they have no readings here.
    for lemma in &report.bases {
        candidates.push(Suggestion {
            word: lemma.word.to_string(),
            edit_distance: 0,
            lemma: Some(lemma.word.to_string()),
            source: SuggestionSource::NeighbourLemma,
            readings: Vec::new(),
            confidence: Confidence::Plain,
            original_rank: 0,
            existing_score: 0.0,
        });
    }
    // 2. Lemmas that share a head / tail / prefix but do not inflect to
    //    the input (neighbours).
    for lemma in &report.neighbours {
        candidates.push(Suggestion {
            word: lemma.word.to_string(),
            edit_distance: 0,
            lemma: Some(lemma.word.to_string()),
            source: SuggestionSource::NeighbourLemma,
            readings: Vec::new(),
            confidence: Confidence::Plain,
            original_rank: 0,
            existing_score: 0.0,
        });
    }
    // 3. Direct sanalista hit (the input is itself a lemma).
    for direct in &report.direct {
        candidates.push(Suggestion {
            word: direct.word().to_string(),
            edit_distance: 0,
            lemma: Some(direct.word().to_string()),
            source: SuggestionSource::DirectLemma,
            readings: Vec::new(),
            confidence: Confidence::Plain,
            original_rank: 0,
            existing_score: 0.0,
        });
    }
    // 4. Generated inflected forms. These carry the source lemma and the
    //    morphological readings the engine produced. The engine already
    //    dropped everything outside `max_distance`.
    for sf in &report.forms {
        candidates.push(Suggestion {
            word: sf.form.word.clone(),
            edit_distance: 0,
            lemma: Some(sf.lemma.to_string()),
            source: SuggestionSource::GeneratedForm {
                lemma: sf.lemma.to_string(),
            },
            readings: sf.form.readings.clone(),
            confidence: sf.form.confidence,
            original_rank: 0,
            existing_score: 0.0,
        });
    }

    // 4b. Supplemental derived-verb forms. These lexemes are not in the
    //     sanalista, so the head/tail walks cannot see them; the typo
    //     `jouduttaitui` must still find `jouduttautui`.
    if max_distance > 0 {
        let input_len = word.chars().count();
        for (surface, lemma) in crate::expand::derived::forms() {
            if surface.chars().count().abs_diff(input_len) > max_distance {
                continue;
            }
            candidates.push(Suggestion {
                word: surface.to_string(),
                edit_distance: 0,
                lemma: Some(lemma.to_string()),
                source: SuggestionSource::GeneratedForm {
                    lemma: lemma.to_string(),
                },
                readings: Vec::new(),
                confidence: Confidence::Plain,
                original_rank: 0,
                existing_score: 0.0,
            });
        }
    }

    // 4c. Common verb-ending typo: a surface ending in `oo` is usually
    //     the 3rd person `-oaa` or 2nd person `-oa` (`kohoo` ->
    //     `kohoaa` / `kohoa`). The ordinary candidate walk finds `kohoa`
    //     but not `kohoaa` because the latter is a generated form of a
    //     base that the misspelling does not select. Try both endings and
    //     keep only forms the fast path confirms.
    if let Some(prefix) = word.strip_suffix("oo") {
        for ending in ["oaa", "oa"] {
            let candidate = format!("{prefix}{ending}");
            let base = is_known_word_fast(&candidate).or_else(|| {
                find_any_base_with(lexicon, cat, &candidate)
                    .first()
                    .map(|lemma| lemma.word.to_string())
            });
            if let Some(base) = base {
                candidates.push(Suggestion {
                    word: candidate,
                    edit_distance: 0,
                    lemma: Some(base.clone()),
                    source: SuggestionSource::GeneratedForm { lemma: base },
                    readings: Vec::new(),
                    confidence: Confidence::Plain,
                    original_rank: 0,
                    existing_score: 0.0,
                });
            }
        }
    }

    // 5. The s-less host of a colloquial `-s` (`ymmärrätkös` ->
    //    `ymmärrätkö`). It is an inflected form, not a lemma, so the
    //    walks above will not always produce it; the checker flags the
    //    `-s` word and this is the correction the caller wants.
    if let Some(host) = s_less_host(&word) {
        let base = is_known_word_fast(&host);
        candidates.push(Suggestion {
            word: host,
            edit_distance: 0,
            lemma: base.clone(),
            source: SuggestionSource::GeneratedForm {
                lemma: base.unwrap_or_default(),
            },
            readings: Vec::new(),
            confidence: Confidence::Plain,
            original_rank: 0,
            existing_score: 0.0,
        });
    }

    // 4d. Archaic `-sten` genitive plural: recover the base and offer
    //     the standard `-iden`/`-itten` alternate (`lammasten` ->
    //     `lampaiden`). This is a morphological variant, not a one-edit
    //     typo, so only keep it when the caller's distance budget allows it.
    let gradation_alt = gradation_plural_alternate(&word);
    if let Some((alt, base)) = &gradation_alt {
        let d = levenshtein(&word, alt, max_distance);
        if d <= max_distance {
            candidates.push(Suggestion {
                word: alt.clone(),
                edit_distance: d,
                lemma: Some(base.clone()),
                source: SuggestionSource::GeneratedForm {
                    lemma: base.clone(),
                },
                readings: Vec::new(),
                confidence: Confidence::Plain,
                original_rank: 0,
                existing_score: 0.0,
            });
        }
    }

    // A hyphen is part of the spelling, not a typo, so the ordinary
    // candidate walk does not always produce the other spelling. Add the
    // closed/hyphenated twin when the engine confirms it.
    push_hyphen_candidates(original, &word, max_distance, &mut candidates);

    // A reported wrong inflection the edit-distance pool can reach only at
    // distance 3 (`ahertelin` -> `ahersin`). Add it explicitly; it is licensed
    // by the table rather than the caller's distance budget, so it survives the
    // `retain` below even when that budget is smaller than its true distance.
    let correction = reported_correction(&word).map(|(right, _)| right);
    if let Some((right, lemma)) = reported_correction(&word)
        && !candidates.iter().any(|c| c.word == right)
    {
        candidates.push(Suggestion {
            word: right.to_string(),
            edit_distance: levenshtein(&word, right, usize::MAX),
            lemma: Some(lemma.to_string()),
            source: SuggestionSource::GeneratedForm {
                lemma: lemma.to_string(),
            },
            readings: Vec::new(),
            confidence: Confidence::Plain,
            original_rank: 0,
            existing_score: 0.0,
        });
    }

    candidates.extend(clitic_candidates);

    // Alphabetical sort + dedup keeps the first occurrence's metadata,
    // matching the existing `suggest` body.
    candidates.sort_by(|a, b| a.word.cmp(&b.word));
    candidates.dedup_by(|a, b| a.word == b.word);

    // Levenshtein: filter by `max_distance`, score `existing_score`.
    for c in &mut candidates {
        // A curated correction is not a spelling edit: `liektisevä` is four
        // edits from `liekehtivä` but the intended reading is the
        // participle. Keep it in the pool regardless of the caller's
        // distance budget; the explicit table is the license. Its
        // `edit_distance` is still the real distance.
        let curated = correction == Some(c.word.as_str());
        let d = if curated {
            levenshtein(&word, &c.word, usize::MAX)
        } else {
            levenshtein(&word, &c.word, max_distance)
        };
        c.edit_distance = d;
        // Score combines the distance-driven base with a small
        // keyboard-adjacency tie-break for single-substitution typos.
        // The base alone is 1/(1+d); a non-adjacent distance-1
        // candidate scores 0.5, a keyboard-adjacent one 0.55 -- enough
        // to break the alphabetical tie-break between them.
        let mut score = 1.0 / (1.0 + d as f32);
        if d > 0 {
            score += keyboard_bonus(&word, &c.word);
            if is_doubling_repair(&word, &c.word) {
                score += DOUBLED_LETTER_BONUS;
            }
        }
        if correction == Some(c.word.as_str()) {
            // Rank the known repair ahead of unrelated candidates at the
            // same distance without pretending it is a shorter edit.
            score += 0.5;
        }
        c.existing_score = score;
    }
    candidates.retain(|c| c.edit_distance <= max_distance || correction == Some(c.word.as_str()));

    // Last resort: a compound typo (`todistelukykynnsä`) has no lemma
    // neighbourhood close enough to surface its correction, because the
    // only word one edit away is a *compound*, not a lemma. Delete one
    // character at a time and keep the results the compound-aware fast
    // path accepts. This is bounded by the word length and only runs when
    // the ordinary candidate pool came up empty.
    //
    // Restrict it to words with an adjacent repeat: a doubled letter is
    // the common accidental-key-repeat typo, and the guard keeps the
    // extra probes off ordinary unknown words.
    if candidates.is_empty() && max_distance > 0 && has_adjacent_repeat(&word) {
        for candidate in deletion_corrections(&word) {
            if is_known_word_fast(&candidate).is_none() {
                continue;
            }
            let d = levenshtein(&word, &candidate, max_distance);
            if d > max_distance {
                continue;
            }
            candidates.push(Suggestion {
                word: candidate,
                edit_distance: d,
                lemma: None,
                source: SuggestionSource::NeighbourLemma,
                readings: Vec::new(),
                confidence: Confidence::Plain,
                original_rank: 0,
                existing_score: 0.0,
            });
        }
    }

    // Final deterministic ranking: distance ascending, then a higher
    // `existing_score` first (so the keyboard-adjacent bonus actually
    // surfaces at slice 0), then alphabetical tie-break (so two
    // candidates at the same distance *and* score are stable).
    candidates.sort_by(|a, b| {
        a.edit_distance
            .cmp(&b.edit_distance)
            .then(
                b.existing_score
                    .partial_cmp(&a.existing_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.word.cmp(&b.word))
    });

    // A word ending in `oo` is most often a mistyped 3rd-person
    // `-oaa` (`kohoo` -> `kohoaa`), even though the plain distance to
    // `kohoaa` is two edits while the 2nd-person `kohoa` is one. Put the
    // repaired `-oaa` form first so it is visible in the small popup.
    if let Some(prefix) = word.strip_suffix("oo") {
        let repaired = format!("{prefix}oaa");
        if let Some(pos) = candidates.iter().position(|c| c.word == repaired) {
            let candidate = candidates.remove(pos);
            candidates.insert(0, candidate);
        }
    }

    if let Some((alt, _)) = &gradation_alt
        && let Some(pos) = candidates.iter().position(|c| c.word == *alt)
    {
        let candidate = candidates.remove(pos);
        candidates.insert(0, candidate);
    }

    // Stamp the deterministic rank onto each Suggestion so tools can
    // explain the ordering.
    for (i, c) in candidates.iter_mut().enumerate() {
        c.original_rank = i;
    }

    candidates.truncate(max_n);
    apply_input_case(original, candidates)
}

/// Add the other spelling of a compound whose hyphen is optional-looking:
/// a wrong hyphen is offered closed (`kirja-hylly` -> `kirjahylly`), and a
/// closed form with a standard hyphenated lexeme is offered hyphenated
/// (`linjaauto` -> `linja-auto`, `suomalaisruotsalainen` ->
/// `suomalais-ruotsalainen`, `EUmaat` -> `EU-maat`).
///
/// Every candidate is confirmed by `is_known_word_fast`, so this cannot invent a
/// spelling. The insertion search is gated by a cheap shape test: a doubled
/// vowel, a `-lainen` pair, an acronym prefix or a number prefix. Without
/// that gate the search would run for every ordinary misspelling.
fn push_hyphen_candidates(
    original: &str,
    lower: &str,
    max_distance: usize,
    candidates: &mut Vec<Suggestion>,
) {
    if lower.contains('-') {
        let closed = lower.replace('-', "");
        if is_known_word_fast(lower).is_none()
            && let Some(base) = is_known_word_fast(&closed)
        {
            candidates.push(Suggestion {
                word: closed,
                edit_distance: 0,
                lemma: Some(base.clone()),
                source: SuggestionSource::GeneratedForm { lemma: base },
                readings: Vec::new(),
                confidence: Confidence::Plain,
                original_rank: 0,
                existing_score: 0.0,
            });
        }
        return;
    }
    if max_distance == 0 || !hyphen_plausible(original) {
        return;
    }
    for (i, _) in lower.char_indices().skip(1) {
        let candidate = format!("{}-{}", &lower[..i], &lower[i..]);
        if let Some(base) = is_known_word_fast(&candidate) {
            candidates.push(Suggestion {
                word: candidate,
                edit_distance: 0,
                lemma: Some(base.clone()),
                source: SuggestionSource::GeneratedForm { lemma: base },
                readings: Vec::new(),
                confidence: Confidence::Plain,
                original_rank: 0,
                existing_score: 0.0,
            });
        }
    }
    if original != lower {
        for (i, _) in original.char_indices().skip(1) {
            let candidate = format!("{}-{}", &original[..i], &original[i..]);
            if let Some(base) = is_known_word_fast(&candidate) {
                candidates.push(Suggestion {
                    word: candidate,
                    edit_distance: 0,
                    lemma: Some(base.clone()),
                    source: SuggestionSource::GeneratedForm { lemma: base },
                    readings: Vec::new(),
                    confidence: Confidence::Plain,
                    original_rank: 0,
                    existing_score: 0.0,
                });
            }
        }
    }
}

/// Whether a closed word has the shape that can hide a hyphen: a doubled
/// vowel (`linjaauto`, `maa-alue`), a `-lainen` pair, an acronym prefix
/// (`EUmaat`) or a number prefix (`10vuotias`).
fn hyphen_plausible(word: &str) -> bool {
    let mut previous = None;
    for c in word.chars() {
        if previous == Some(c) && crate::is_vowel(c) {
            return true;
        }
        previous = Some(c);
    }
    word.contains("lainen")
        || word.contains("läinen")
        || word.chars().take_while(|c| c.is_uppercase()).count() >= 2
        || word.chars().next().is_some_and(|c| c.is_numeric())
}

/// Whether `word` has two identical characters in a row, the shape the
/// deletion fallback targets.
/// `candidate` is `query` with one doubled letter restored.
///
/// `kiväriään` -> `kivääriään`: the extra character is the same as its
/// neighbour, which is the shape of a dropped long vowel or consonant.
fn is_doubling_repair(query: &str, candidate: &str) -> bool {
    let q: Vec<char> = query.chars().collect();
    let c: Vec<char> = candidate.chars().collect();
    if c.len() != q.len() + 1 {
        return false;
    }
    for at in 1..c.len() {
        // Long vowels are the reported shape (`kiväriään` -> `kivääriään`).
        // Doubled consonants are already handled by the ordinary ranking and
        // would otherwise reorder unrelated words such as `talo`/`tallo`.
        if c[at] == c[at - 1] && crate::is_vowel(c[at]) {
            let mut without = c.clone();
            without.remove(at);
            if without == q {
                return true;
            }
        }
    }
    false
}

fn has_adjacent_repeat(word: &str) -> bool {
    let mut previous = None;
    for c in word.chars() {
        if previous == Some(c) {
            return true;
        }
        previous = Some(c);
    }
    false
}

/// Every word one deletion away from `word`, in input order, without
/// duplicates.
///
/// A doubled letter is the common accidental-key-repeat typo, and in a
/// compound (`todistelukykynnsä`) the correction is not a lemma the
/// ordinary suggestion walks can reach. Deleting each character keeps the
/// search bounded by the word length.
fn deletion_corrections(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut out: Vec<String> = Vec::with_capacity(chars.len());
    for i in 0..chars.len() {
        let mut candidate = String::with_capacity(word.len());
        for (j, c) in chars.iter().enumerate() {
            if j != i {
                candidate.push(*c);
            }
        }
        if !candidate.is_empty() && candidate != word && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}
