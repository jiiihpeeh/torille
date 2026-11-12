//! Per-word morphology and the capped analysis cache.
//!
//! The cache stores the readings of one surface, not a sentence: morphology is
//! independent of context. The role rules still disambiguate from word order.

use std::collections::HashMap;

use crate::expand::find_bases_for_analysis;
use crate::spec::Catalogues;
use crate::tags::{Case, PartOfSpeech, Polarity, Tags, add_reading};

use super::{is_finite_predicate, is_nominal_pos, is_verb_reading};

/// The morphology of one surface word, reduced to what a role rule needs.
#[derive(Debug, Clone, Default)]
pub(super) struct WordAnalysis {
    pub(super) lemma: Option<String>,
    /// The lemma of a finite-verb reading, when one exists. `tuli` is both a
    /// noun and a past-tense verb; the context decides which, so both are
    /// kept until a role is known.
    pub(super) verb_lemma: Option<String>,
    /// The lemma of a nominal reading, for the same reason.
    pub(super) nominal_lemma: Option<String>,
    /// True when the ordinary search found this exact surface, not just a
    /// clitic-stripped host. A clitic word (`onko`, `talohan`) is only
    /// re-analysed through its host when this is false.
    pub(super) exact: bool,
    pub(super) readings: Vec<Tags>,
}

/// A capped cache of resolved words for the syntax layer.
///
/// The morphology of a surface is independent of the sentence and the
/// document, so the cache can be reused across analyses. That is what makes a
/// re-render of a paragraph cheap: the second pass is a hash probe per word
/// instead of a base search. The cap keeps a large paste's vocabulary from
/// growing the wasm heap without bound; it clears wholesale when full.
#[derive(Debug, Default, Clone)]
pub struct AnalysisCache {
    words: HashMap<String, WordAnalysis>,
}

/// The cap on resolved words. One entry holds a few small `Tags` structs and
/// lemma strings; 4096 covers a long article.
const ANALYSIS_CACHE_LIMIT: usize = 4096;

impl AnalysisCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop every resolved word.
    pub fn clear(&mut self) {
        self.words.clear();
    }

    /// Number of resolved words currently cached.
    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub(super) fn resolve_word(
        &mut self,
        lexicon: &[crate::kotus::Lemma<'_>],
        cat: &Catalogues,
        lower: &str,
    ) -> WordAnalysis {
        if let Some(analysis) = self.words.get(lower) {
            return analysis.clone();
        }
        let analysis = resolve_word(lexicon, cat, lower);
        if self.words.len() >= ANALYSIS_CACHE_LIMIT {
            self.words.clear();
        }
        self.words.insert(lower.to_string(), analysis.clone());
        analysis
    }
}

/// Resolve a lowercase word, peeling its clitic layers when the host is the
/// real word.
///
/// The base search recognises a clitic-bearing word through its host, but it
/// keeps no form for the surface (`onko` has no generated `onko`), so the
/// ordinary resolution falls back to a bare lemma reading with no mood or
/// case. `onko` then cannot be the predicate of a question. Resolving the
/// host and carrying its readings over fixes that. The clitic itself is kept
/// on the reading; it is discourse information, not a role cue.
fn resolve_word(
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
    lower: &str,
) -> WordAnalysis {
    let ordinary = resolve_word_plain(lexicon, cat, lower);
    if ordinary.exact {
        return ordinary;
    }
    let (host, clitics) = crate::expand::clitic::strip_all_clitics(lower);
    if clitics.is_empty() || host == lower {
        return ordinary;
    }
    let mut resolved = resolve_word_plain(lexicon, cat, &host);
    if resolved.readings.is_empty() {
        return ordinary;
    }
    // `strip_all_clitics` returns the stack from the outside in. One slot is
    // all `Tags` has, so record the outermost particle.
    for reading in &mut resolved.readings {
        reading.clitic = clitics.first().copied();
    }
    resolved.exact = true;
    resolved
}

/// The ordinary, no-clitic resolution.
fn resolve_word_plain(
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
    lower: &str,
) -> WordAnalysis {
    // Recognition-only search: it uses the shared paradigm probe rather than
    // building the suggestion pool's forms and neighbours for every candidate.
    // The readings are produced below for the few winning bases only.
    let report = find_bases_for_analysis(lexicon, cat, lower);
    let mut out = WordAnalysis {
        exact: !report.direct.is_empty(),
        ..WordAnalysis::default()
    };

    // A direct lemma is the surface itself and is the right default lemma.
    // A homonymous base (`minä` is also an essive of `mikä`) must not replace
    // it; the role resolver below still sees every reading.
    if let Some(direct) = report.direct.first() {
        out.lemma = Some(direct.word().to_string());
    } else if let Some(base) = report.bases.first() {
        out.lemma = Some(base.word.to_string());
    }

    let mut seen: Vec<&str> = Vec::new();
    for direct in &report.direct {
        let word = direct.word();
        seen.push(word);
        let pos = PartOfSpeech::parse_all(direct.part_of_speech())
            .first()
            .copied();
        let inflections: Vec<_> = direct.inflections().collect();
        if add_forms_from_lemma(word, &inflections, pos, lower, cat, &mut out) {
            out.exact = true;
        }
    }
    for base in &report.bases {
        if seen.contains(&base.word) {
            continue;
        }
        seen.push(base.word);
        let pos = PartOfSpeech::parse_all(base.part_of_speech)
            .first()
            .copied();
        if add_forms_from_lemma(base.word, &base.inflections, pos, lower, cat, &mut out) {
            out.exact = true;
        }
    }

    // The prefix walks miss some stem-changing paradigm members (`söi` is
    // `syödä` across a two-character stem change). The cheap stem-map gate
    // can name the base even when those walks do not, so use it as a
    // fallback and re-inflect the base for the readings.
    if out.readings.is_empty()
        && let Some(base) = cheap_base(lower)
    {
        out.lemma = Some(base.clone());
        add_readings_from_base(&base, lower, cat, &mut out.readings);
        if is_nominal_pos(lemma_pos(&base)) {
            out.nominal_lemma = Some(base);
        }
    }

    // A direct lemma can hide a homonymous verb: `tuli` is the noun and the
    // past tense of `tulla`, and the lemma probe stops before the verb stem
    // map is asked. Recover the verb reading when the ordinary walk did not
    // find one; the words themselves are context-disambiguated later.
    if !out.readings.iter().any(is_verb_reading)
        && let Some(base) = crate::expand::check_normal::find_base_via_verb_stem_map(lower, false)
    {
        add_readings_from_base(&base, lower, cat, &mut out.readings);
        if out.readings.iter().any(is_finite_predicate) {
            out.verb_lemma = Some(base);
        }
    }

    // A direct lemma or a base with no retained form still has one reading:
    // the sanalista's part of speech and, for a nominal, the citation case.
    if out.readings.is_empty() {
        for direct in &report.direct {
            add_reading(&mut out.readings, direct_reading(direct));
            if is_nominal_pos(
                PartOfSpeech::parse_all(direct.part_of_speech())
                    .first()
                    .copied(),
            ) {
                out.nominal_lemma = Some(direct.word().to_string());
            }
        }
        if out.readings.is_empty() {
            for base in &report.bases {
                if let Some(row) = crate::kotus::sanalista().by_word(base.word).next() {
                    add_reading(&mut out.readings, direct_reading(&row));
                    if is_nominal_pos(
                        PartOfSpeech::parse_all(row.part_of_speech())
                            .first()
                            .copied(),
                    ) {
                        out.nominal_lemma = Some(base.word.to_string());
                    }
                }
            }
        }
    }

    // The negation auxiliary is a closed table, not a general verb
    // paradigm. Add the auxiliary reading even when a homonymous noun
    // reading exists (`ei` is also a noun), and leave the noun reading alone;
    // the clause flag keys off the auxiliary, not on every reading.
    if let Some(base) = crate::expand::negation::negation_base(lower) {
        if !out
            .readings
            .iter()
            .any(|reading| reading.part_of_speech == Some(PartOfSpeech::NegativeVerb))
        {
            add_reading(
                &mut out.readings,
                Tags {
                    part_of_speech: Some(PartOfSpeech::NegativeVerb),
                    polarity: Polarity::Negative,
                    ..Tags::UNKNOWN
                },
            );
        } else {
            for reading in &mut out.readings {
                if reading.part_of_speech == Some(PartOfSpeech::NegativeVerb) {
                    reading.polarity = Polarity::Negative;
                }
            }
        }
        if out.lemma.is_none() {
            out.lemma = Some(base.to_string());
        }
    }

    out
}

/// Name a base through the cheap per-word pipeline, without the prefix walks.
///
/// `check_normal_fast` runs the lemma, form, clitic, and stem-map gates. It is
/// exactly the set that catches a stem change the prefix neighbourhood misses,
/// and it is much cheaper than the full reverse index.
fn cheap_base(lower: &str) -> Option<String> {
    use crate::expand::check_normal::{NormalizedOutcome, check_normal_fast};

    match check_normal_fast(lower, false) {
        NormalizedOutcome::Lemma => Some(lower.to_string()),
        NormalizedOutcome::Inflected { base, .. } => Some(base),
        NormalizedOutcome::Unknown => None,
    }
}

/// Inflect one known base and add the readings of `lower` to `out`.
///
/// Returns true when the base emits the exact surface. The base search has
/// already decided which lemmas can produce it; this is the only inflection
/// the syntax path does, so the suggestion pool's full form list is never
/// built.
fn add_forms_from_lemma(
    base: &str,
    inflections: &[crate::kotus::Inflection],
    pos: Option<PartOfSpeech>,
    lower: &str,
    cat: &Catalogues,
    out: &mut WordAnalysis,
) -> bool {
    let pos = pos.unwrap_or(PartOfSpeech::Unknown);
    let mut exact = false;
    for form in crate::expand::inflect_all(base, inflections, cat, None, pos, true) {
        if form.word != lower {
            continue;
        }
        exact = true;
        if out.lemma.is_none() {
            out.lemma = Some(base.to_string());
        }
        for reading in &form.readings {
            let tags = reading_with_pos(*reading, Some(pos));
            add_reading(&mut out.readings, tags);
            if is_finite_predicate(&tags) {
                out.verb_lemma = Some(base.to_string());
            }
            if is_nominal_pos(tags.part_of_speech) {
                out.nominal_lemma = Some(base.to_string());
            }
        }
    }
    exact
}

/// Inflect `base` and add the readings for `lower`.
fn add_readings_from_base(base: &str, lower: &str, cat: &Catalogues, readings: &mut Vec<Tags>) {
    let Some(row) = crate::kotus::sanalista().by_word(base).next() else {
        return;
    };
    let pos = PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Unknown);
    let inflections: Vec<_> = row.inflections().collect();
    for form in crate::expand::inflect_all(base, &inflections, cat, None, pos, true) {
        if form.word != lower {
            continue;
        }
        for reading in &form.readings {
            add_reading(readings, reading_with_pos(*reading, Some(pos)));
        }
    }
    if readings.is_empty() {
        add_reading(readings, direct_reading(&row));
    }
}

/// Replace a generated reading's part of speech with the source lemma's.
///
/// The base search inflects every lemma with `PartOfSpeech::Noun` (the
/// suggestion pool only needs surfaces), so an adverb or pronoun form arrives
/// tagged as a noun. The lemma row is the authority here: `eilen` must stay
/// an adverb and `sinä` a pronoun, or the role rules mistake them for
/// nominals. A form may be a reading of several lemmas, and each
/// `SourcedForm` names its own lemma, so this stays correct for homonyms
/// (`tuli` the noun vs. `tulla` the verb).
fn reading_with_pos(mut tags: Tags, pos: Option<PartOfSpeech>) -> Tags {
    if let Some(pos) = pos {
        tags.part_of_speech = Some(pos);
    }
    tags
}

/// The sanalista's first part of speech for a lemma.
fn lemma_pos(lemma: &str) -> Option<PartOfSpeech> {
    crate::kotus::sanalista()
        .by_word(lemma)
        .next()
        .and_then(|row| {
            PartOfSpeech::parse_all(row.part_of_speech())
                .first()
                .copied()
        })
}

/// A reading for a lemma that no generated form reached.
fn direct_reading(row: &crate::kotus::EmbeddedLemma) -> Tags {
    let pos = PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied();
    let mut tags = match pos {
        Some(
            PartOfSpeech::Noun
            | PartOfSpeech::Adjective
            | PartOfSpeech::Pronoun
            | PartOfSpeech::Numeral
            | PartOfSpeech::Ordinal,
        ) => Tags::nominal(Case::Nominative),
        _ => Tags::UNKNOWN,
    };
    tags.part_of_speech = pos;
    tags
}

/// A reading for a recognized proper name.
///
/// A recorded case wins. Without one, the citation spelling is nominative and
/// any other form stays case-less: role selection then leaves it `Unknown`
/// instead of inventing a nominative.
pub(super) fn proper_name_reading(surface: &str, lemma: &str, case: Option<Case>) -> Tags {
    if let Some(case) = case {
        Tags::nominal(case).of_pos(PartOfSpeech::Noun)
    } else if surface.eq_ignore_ascii_case(lemma) {
        Tags::nominal(Case::Nominative).of_pos(PartOfSpeech::Noun)
    } else {
        Tags::UNKNOWN.of_pos(PartOfSpeech::Noun)
    }
}
