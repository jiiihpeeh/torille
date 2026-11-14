//! Cached morphology for the syntax and style layers.
//!
//! The engine resolves a surface once and keeps every reading the rules need:
//! part of speech, lemma, case, number, person, tense, mood, voice, polarity,
//! possessive and clitic. The cache is keyed by the lowercased surface;
//! morphology is independent of the sentence and the document.
//!
//! This module is deliberately below `syntax`: `syntax` builds roles and
//! clauses from [`MorphologyAnalysis`], and `style_check` reads the same
//! analysis without paying for roles, clauses or lists. The wasm textarea
//! state owns one [`MorphologyCache`] and shares it between the two layers.
//!
//! The cache stores readings, not a yes/no answer. That is the difference
//! between the spell fast path (`check_normal_fast`, a known-word gate) and
//! this layer: a grammar rule needs the case a reading licenses, not just the
//! fact that the word is in the dictionary.

use std::collections::HashMap;
use std::sync::Arc;

use crate::expand::check_normal::{
    collect_bases_via_stem_map, collect_bases_via_verb_stem_map, find_base_via_stem_map,
    find_base_via_verb_stem_map,
};
use crate::expand::find_bases_for_analysis;
use crate::kotus::{Lemma, lemmas};
use crate::sentence::SentenceSpan;
use crate::spec::Catalogues;
use crate::syntax::{is_finite_predicate, is_nominal_pos, is_verb_reading};
use crate::tags::{Case, Clitic, Number, PartOfSpeech, Polarity, Possessive, Tags, add_reading};
use crate::tokenise::Token;

/// How hard the resolver had to work for one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Depth {
    /// The surface is a direct lemma or a form found by the ordinary search.
    Direct,
    /// The surface was named by the noun/verb stem map.
    StemMap,
    /// The surface came from the head/tail recognition walk.
    HeadsTails,
    /// The surface came from a prefix or short-lemma walk.
    Prefix,
    /// The surface was a host plus a clitic.
    Clitic,
    /// The surface was a host plus a possessive suffix.
    Possessive,
    /// The surface was a modifier plus an inflected head.
    Compound,
    /// A capitalized token resolved through the proper-name table.
    ProperName,
    /// No reading was found.
    #[default]
    Unknown,
}

impl Depth {
    /// Every depth, in the order the counters are stored.
    pub const ALL: [Depth; 9] = [
        Depth::Direct,
        Depth::StemMap,
        Depth::HeadsTails,
        Depth::Prefix,
        Depth::Clitic,
        Depth::Possessive,
        Depth::Compound,
        Depth::ProperName,
        Depth::Unknown,
    ];

    /// A stable index into [`CacheStats::by_depth`].
    pub fn index(self) -> usize {
        match self {
            Depth::Direct => 0,
            Depth::StemMap => 1,
            Depth::HeadsTails => 2,
            Depth::Prefix => 3,
            Depth::Clitic => 4,
            Depth::Possessive => 5,
            Depth::Compound => 6,
            Depth::ProperName => 7,
            Depth::Unknown => 8,
        }
    }

    /// A short label for the performance harness.
    pub fn label(self) -> &'static str {
        match self {
            Depth::Direct => "direct",
            Depth::StemMap => "stem_map",
            Depth::HeadsTails => "heads_tails",
            Depth::Prefix => "prefix",
            Depth::Clitic => "clitic",
            Depth::Possessive => "possessive",
            Depth::Compound => "compound",
            Depth::ProperName => "proper_name",
            Depth::Unknown => "unknown",
        }
    }
}

/// Cache counters for the performance harness and the wasm probe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub words: usize,
    pub hits: u64,
    pub misses: u64,
    /// First resolutions of a surface, by the [`Depth`] that named it.
    pub by_depth: [u64; Depth::ALL.len()],
    /// Surfaces upgraded from a shallow entry by [`MorphologyCache::resolve_deep`].
    pub deep_upgrades: u64,
    /// Capitalized tokens resolved through the proper-name table.
    pub proper_names: u64,
}

/// The morphology of one surface word.
///
/// Every field is a reading the rules consume; nothing here is a rendering
/// decision. `case` and `number` are the agreed nominal values, computed once
/// at insert time so a rule does not rescan `readings` on every call.
#[derive(Debug, Clone, Default)]
pub struct WordInfo {
    /// The lowercased surface this entry was resolved for. Rules compare
    /// against lowercase table keys; storing it once keeps a rule from
    /// allocating a new `String` per token per pass.
    pub(crate) key: String,
    pub(crate) lemma: Option<String>,
    /// The lemma of a finite-verb reading, when one exists. `tuli` is both a
    /// noun and a past-tense verb; the context decides which, so both are
    /// kept until a role is known.
    pub(crate) verb_lemma: Option<String>,
    /// The lemma of a nominal reading, for the same reason.
    pub(crate) nominal_lemma: Option<String>,
    /// True when the ordinary search found this exact surface, not just a
    /// clitic-stripped host.
    pub(crate) exact: bool,
    /// Every lemma that produced a reading for the surface, in resolution
    /// order. The chosen `lemma` is one of them.
    pub(crate) lemmas: Vec<String>,
    pub(crate) readings: Vec<Tags>,
    /// The agreed nominal case, or `None` when the readings disagree.
    pub(crate) case: Option<Case>,
    /// The agreed number within `case`, or `None` when the readings disagree.
    pub(crate) number: Option<Number>,
    /// The first part of speech reading, for callers that need one label.
    /// `readings` remains the authority for a word with several readings.
    pub(crate) part_of_speech: Option<PartOfSpeech>,
    /// The outermost clitic, when the surface carries one.
    pub(crate) clitic: Option<Clitic>,
    /// The possessive suffix, when the surface carries one.
    pub(crate) possessive: Option<Possessive>,
    /// A possible productive derivation for a surface the lexicon did not
    /// resolve. The lexicon already names lexicalised derivations, so this is
    /// only filled for an otherwise unknown word.
    pub(crate) derivation: Option<crate::derivation::Reverse>,
    pub(crate) depth: Depth,
}

impl WordInfo {
    pub fn lemma(&self) -> Option<&str> {
        self.lemma.as_deref()
    }

    pub fn readings(&self) -> &[Tags] {
        &self.readings
    }

    pub fn case(&self) -> Option<Case> {
        self.case
    }

    pub fn number(&self) -> Option<Number> {
        self.number
    }

    pub fn part_of_speech(&self) -> Option<PartOfSpeech> {
        self.part_of_speech
    }

    pub fn clitic(&self) -> Option<Clitic> {
        self.clitic
    }

    pub fn possessive(&self) -> Option<Possessive> {
        self.possessive
    }

    pub fn derivation(&self) -> Option<&crate::derivation::Reverse> {
        self.derivation.as_ref()
    }

    pub fn depth(&self) -> Depth {
        self.depth
    }

    pub fn is_empty(&self) -> bool {
        self.readings.is_empty()
    }

    fn push_lemma(&mut self, lemma: &str) {
        if !self.lemmas.iter().any(|known| known == lemma) {
            self.lemmas.push(lemma.to_string());
        }
    }
}

/// A capped cache of resolved words.
///
/// The morphology of a surface is independent of the sentence and the
/// document, so the cache can be reused across analyses and across the
/// grammar and style layers. The cap keeps a large paste's vocabulary from
/// growing the wasm heap without bound.
#[derive(Debug, Default, Clone)]
pub struct MorphologyCache {
    words: HashMap<String, Arc<WordInfo>>,
    /// Surfaces whose entry came from the deep recognition walk.
    deep: std::collections::HashSet<String>,
    hits: u64,
    misses: u64,
    /// First resolutions of a surface, by the [`Depth`] that named it.
    by_depth: [u64; Depth::ALL.len()],
    /// Surfaces upgraded from a shallow entry by [`Self::resolve_deep`].
    deep_upgrades: u64,
    /// Capitalized tokens resolved through the proper-name table.
    proper_names: u64,
}

/// The cap on resolved words. One entry holds a few `Tags` and lemma strings;
/// 4096 covers a long article.
const MORPHOLOGY_CACHE_LIMIT: usize = 4096;

impl MorphologyCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop every resolved word and reset the counters.
    pub fn clear(&mut self) {
        self.words.clear();
        self.deep.clear();
        self.hits = 0;
        self.misses = 0;
        self.by_depth = [0; Depth::ALL.len()];
        self.deep_upgrades = 0;
        self.proper_names = 0;
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            words: self.words.len(),
            hits: self.hits,
            misses: self.misses,
            by_depth: self.by_depth,
            deep_upgrades: self.deep_upgrades,
            proper_names: self.proper_names,
        }
    }

    /// The cached entry for a lowercased surface, without resolving it.
    pub fn get(&self, lower: &str) -> Option<&Arc<WordInfo>> {
        self.words.get(lower)
    }

    /// Count a capitalized token resolved through the proper-name table.
    ///
    /// The overlay is token-specific (a name's reading depends on the
    /// capitalization), so it is not inserted into the lowercased cache; the
    /// counter is what keeps it visible in the harness.
    pub(crate) fn note_proper_name(&mut self) {
        self.proper_names += 1;
    }

    /// Resolve one lowercased surface, sharing the cached entry.
    ///
    /// This is the shallow first pass: a word the cheap ladder cannot name
    /// stays [`Depth::Unknown`]. A rule that needs the head/tail ambiguity
    /// calls [`Self::resolve_deep`] for that token.
    pub(crate) fn resolve(
        &mut self,
        cat: &Catalogues,
        lower: &str,
    ) -> Arc<WordInfo> {
        if let Some(info) = self.words.get(lower) {
            self.hits += 1;
            return Arc::clone(info);
        }
        self.misses += 1;
        let info = Arc::new(resolve_word(cat, lower));
        self.by_depth[info.depth.index()] += 1;
        if self.words.len() >= MORPHOLOGY_CACHE_LIMIT {
            self.words.clear();
        }
        self.words.insert(lower.to_string(), Arc::clone(&info));
        info
    }

    /// Resolve one surface through the deep recognition walk, once.
    ///
    /// The shallow entry is upgraded in place so a later ordinary lookup sees
    /// the extra readings too. The `deep` set keeps a warm re-check from
    /// repeating the walk.
    pub(crate) fn resolve_deep(
        &mut self,
        lexicon: &[Lemma<'_>],
        cat: &Catalogues,
        lower: &str,
    ) -> Arc<WordInfo> {
        if let Some(info) = self.words.get(lower)
            && self.deep.contains(lower)
        {
            self.hits += 1;
            return Arc::clone(info);
        }
        self.misses += 1;
        let was_cached = self.words.contains_key(lower);
        let info = Arc::new(resolve_word_slow(lexicon, cat, lower));
        if was_cached {
            self.deep_upgrades += 1;
        } else {
            self.by_depth[info.depth.index()] += 1;
        }
        if !info.readings.is_empty() {
            if self.words.len() >= MORPHOLOGY_CACHE_LIMIT {
                self.words.clear();
                self.deep.clear();
            }
            self.deep.insert(lower.to_string());
            self.words.insert(lower.to_string(), Arc::clone(&info));
        }
        info
    }
}

/// The words of one text, ready for the syntax or style stage.
///
/// `tokens` is aligned with the input token stream; separators are `None`.
/// The entries are shared with the cache, so passing this around does not
/// copy a reading.
#[derive(Debug, Clone, Default)]
pub struct MorphologyAnalysis {
    pub sentences: Vec<SentenceSpan>,
    pub tokens: Vec<Option<Arc<WordInfo>>>,
}

impl MorphologyAnalysis {
    pub fn word(&self, index: usize) -> Option<&WordInfo> {
        self.tokens.get(index).and_then(|word| word.as_deref())
    }
}

/// Resolve every word token and compute the sentence spans.
///
/// This is stage 1 of the analysis. It is the only stage that searches the
/// lexicon; roles, clauses and style rules consume the result.
pub fn analyze_morphology(tokens: &[Token], cache: &mut MorphologyCache) -> MorphologyAnalysis {
    let sentences = crate::sentence::spans(tokens);
    let lexicon = lemmas();
    let cat = Catalogues::embedded();
    let mut words: Vec<Option<Arc<WordInfo>>> = Vec::with_capacity(tokens.len());
    for token in tokens {
        if !token.is_word() {
            words.push(None);
            continue;
        }
        let lower = token.text.to_lowercase();
        let info = cache.resolve(cat, &lower);
        let info = if info.readings.is_empty()
            && token.is_capitalised()
            && let Some(name) = crate::expand::proper_names::lemma(&token.text)
        {
            cache.note_proper_name();
            Arc::new(proper_name_word(
                &info,
                &token.text,
                name,
                crate::expand::proper_names::case(&token.text),
                &lower,
            ))
        } else {
            info
        };
        words.push(Some(info));
    }

    // A numeral governs a partitive (`kaksi lasta`). A direct lemma can hide
    // that reading (`lasta` is also the splinter lemma), and the cheap path
    // does not pay for every hidden homonym. Upgrade only the token after a
    // numeral when it has no partitive reading; the rest of the page keeps the
    // shallow result.
    let mut previous: Option<Arc<WordInfo>> = None;
    for (index, token) in tokens.iter().enumerate() {
        if !token.is_word() {
            continue;
        }
        let Some(info) = words[index].clone() else {
            continue;
        };
        let governed_by_numeral = previous.as_ref().is_some_and(|previous| {
            previous
                .readings
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Numeral))
        });
        if governed_by_numeral
            && !info
                .readings
                .iter()
                .any(|reading| reading.case == Some(Case::Partitive))
        {
            let lower = token.text.to_lowercase();
            let deep = cache.resolve_deep(lexicon, cat, &lower);
            if !deep.readings.is_empty() {
                words[index] = Some(deep);
            }
        }
        previous = words[index].clone();
    }

    MorphologyAnalysis {
        sentences,
        tokens: words,
    }
}

/// Resolve a lowercase word through the shallow ladder.
///
/// The cheap sources run first: a direct lemma or a noun/verb stem-map base.
/// A clitic or possessive host is peeled when the host is cheap. A word none
/// of them names stays [`Depth::Unknown`]; [`resolve_word_slow`] is the deep
/// upgrade a rule requests for an ambiguous token.
fn resolve_word(cat: &Catalogues, lower: &str) -> WordInfo {
    if let Some(mut fast) = resolve_word_fast(cat, lower) {
        add_possessive_readings(&mut fast, lower, cat);
        return fast;
    }
    let (host, clitics) = crate::expand::clitic::strip_all_clitics(lower);
    if !clitics.is_empty()
        && host != lower
        && let Some(mut resolved) = resolve_word_fast(cat, &host)
    {
        // `strip_all_clitics` returns the stack from the outside in. One
        // slot is all `Tags` has, so record the outermost particle.
        for reading in &mut resolved.readings {
            reading.clitic = clitics.first().copied();
        }
        resolved.clitic = clitics.first().copied();
        resolved.exact = true;
        resolved.depth = Depth::Clitic;
        return resolved;
    }
    if let Some((host, possessive)) = strip_possessive(lower)
        && let Some(mut resolved) = resolve_word_fast(cat, host)
    {
        for reading in &mut resolved.readings {
            reading.possessive = Some(possessive);
        }
        resolved.possessive = Some(possessive);
        resolved.exact = true;
        resolved.depth = Depth::Possessive;
        return resolved;
    }
    // A bare verb connegative (`syöne`, `ajatelko`) is a word in its own
    // right, not a form the paradigm emits. The closed table names the lemma;
    // add its citation reading so the rules still see a verb.
    if let Some(base) = crate::expand::connegative::base(lower)
        && let Some(row) = crate::kotus::sanalista().by_word(base).next()
    {
        let mut out = WordInfo {
            lemma: Some(base.to_string()),
            verb_lemma: Some(base.to_string()),
            lemmas: vec![base.to_string()],
            readings: vec![direct_reading(&row)],
            ..WordInfo::default()
        };
        finish(&mut out, Depth::Direct, lower);
        return out;
    }

    let mut out = WordInfo::default();
    add_negation(&mut out, lower);
    if !out.readings.is_empty() {
        finish(&mut out, Depth::Direct, lower);
        return out;
    }
    finish(&mut out, Depth::Unknown, lower);
    out
}

/// Strip one possessive suffix and name whose it is.
///
/// The suffix attaches after the case ending (`talo+ssa+ni`), so the host is
/// a complete case form and resolves through the cheap ladder.
fn strip_possessive(lower: &str) -> Option<(&str, Possessive)> {
    const SUFFIXES: &[(&str, Possessive)] = &[
        ("nsa", Possessive::Third),
        ("nsä", Possessive::Third),
        ("nne", Possessive::SecondPlural),
        ("mme", Possessive::FirstPlural),
        ("si", Possessive::SecondSingular),
        ("ni", Possessive::FirstSingular),
    ];
    for (suffix, possessive) in SUFFIXES {
        if let Some(host) = lower.strip_suffix(suffix)
            && !host.is_empty()
        {
            return Some((host, *possessive));
        }
    }
    // The contracted third-person possessive sits on a comitative `-ine`
    // stem (`monineen`). A bare `-en` is not a possessive.
    if let Some(host) = lower.strip_suffix("en")
        && host.ends_with("ine")
        && !host.is_empty()
    {
        return Some((host, Possessive::Third));
    }
    // `kidassaan`, `taudistaan`, `mukanaan`: the third-person possessive
    // contracts to `-an`/`-än` after a vowel. Require the vowel so an ordinary
    // genitive `sanan` (host `san`) is not treated as one.
    for (suffix, possessive) in [("an", Possessive::Third), ("än", Possessive::Third)] {
        if let Some(host) = lower.strip_suffix(suffix)
            && !host.is_empty()
            && host
                .chars()
                .last()
                .is_some_and(|ch| matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'ä' | 'ö'))
        {
            return Some((host, possessive));
        }
    }
    None
}

/// Whether `lower` carries a possessive or clitic suffix.
///
/// This is only a performance gate for reading extraction: a false positive
/// costs the extra possessive column, while a false negative would lose a
/// form, so over-approximating is safe.
fn surface_carries_suffix(lower: &str) -> bool {
    strip_possessive(lower).is_some()
        || !crate::expand::clitic::strip_all_clitics(lower).1.is_empty()
}

/// The cheap sources: a direct lemma, or the base named by the stem maps.
///
/// This is the shallow resolver. It deliberately does not run the head/tail
/// walk; a word that it cannot name falls through to [`resolve_word_slow`].
fn resolve_word_fast(cat: &Catalogues, lower: &str) -> Option<WordInfo> {
    let direct: Vec<crate::kotus::EmbeddedLemma> =
        crate::kotus::sanalista().by_word(lower).collect();
    if !direct.is_empty() {
        let mut out = WordInfo {
            exact: true,
            ..WordInfo::default()
        };
        let mut direct_words: Vec<&str> = Vec::new();
        for row in &direct {
            let word = row.word();
            direct_words.push(word);
            let pos = PartOfSpeech::parse_all(row.part_of_speech())
                .first()
                .copied();
            let inflections: Vec<_> = row.inflections().collect();
            add_forms_from_lemma(word, &inflections, pos, lower, cat, &mut out);
        }
        if out.readings.is_empty() {
            for row in &direct {
                add_reading(&mut out.readings, direct_reading(row));
                out.push_lemma(row.word());
                if is_nominal_pos(
                    PartOfSpeech::parse_all(row.part_of_speech())
                        .first()
                        .copied(),
                ) {
                    out.nominal_lemma = Some(row.word().to_string());
                }
            }
        }
        if out.lemma.is_none()
            && let Some(row) = direct.first()
        {
            out.lemma = Some(row.word().to_string());
        }
        add_stem_map_bases(&mut out, lower, cat, &direct_words);
        add_negation(&mut out, lower);
        finish(&mut out, Depth::Direct, lower);
        return Some(out);
    }

    // A surface that carries a possessive/clitic suffix needs the cheap gate
    // first: the stem map indexes the host, not the suffixed surface. The
    // same gate names a classless compound. A plain word goes straight to the
    // stem map below, which is the common case and avoids paying
    // `check_normal_fast` (and its paradigm walk) twice.
    if surface_carries_suffix(lower)
        && let Some(base) = cheap_base(lower)
    {
        if let Some(info) = resolve_from_base_surface(lower, &base, cat) {
            return Some(info);
        }
        if let Some(info) = resolve_compound_surface(lower, &base, cat) {
            return Some(info);
        }
        // Closed-class words (pronouns) are not in the noun/verb stem maps.
        // A cheap gate still names their base, so verify the one paradigm
        // instead of sending every relative-pronoun form to the deep walk.
        if is_closed_class_base(&base)
            && let Some(info) = resolve_from_base_surface(lower, &base, cat)
        {
            return Some(info);
        }
    }

    // A partitive plural can be a form of several lemmas (`kirjoja` belongs
    // to `kirja` and `kirjo`, `pieniä` to the adjective `pieni` and the verb
    // `pieniä`). The stem map already keeps every base; collect them all
    // rather than sending the surface to the head/tail walk.
    let ambiguous = needs_full_base_search(lower);
    if ambiguous
        && let Some(base) = cheap_base(lower)
        && let Some(info) = resolve_compound_surface(lower, &base, cat)
    {
        return Some(info);
    }

    let mut bases: Vec<String> = Vec::new();
    if ambiguous {
        bases.extend(collect_bases_via_stem_map(
            lower,
            PartOfSpeech::Noun,
            false,
        ));
        for base in collect_bases_via_verb_stem_map(lower, false) {
            if !bases.contains(&base) {
                bases.push(base);
            }
        }
    } else {
        if let Some(base) = find_base_via_stem_map(lower, PartOfSpeech::Noun, false) {
            bases.push(base);
        }
        if let Some(base) = find_base_via_verb_stem_map(lower, false)
            && !bases.contains(&base)
        {
            bases.push(base);
        }
    }
    if bases.is_empty() {
        return resolve_cheap_fallback(lower, cat);
    }

    let mut out = WordInfo::default();
    let mut matched_any = false;
    for base in &bases {
        let before = out.readings.len();
        if !add_readings_from_base_matched(base, lower, cat, &mut out.readings) {
            continue;
        }
        matched_any = true;
        out.push_lemma(base);
        let added = &out.readings[before..];
        if out.nominal_lemma.is_none()
            && added
                .iter()
                .any(|reading| is_nominal_pos(reading.part_of_speech))
        {
            out.nominal_lemma = Some(base.clone());
        }
        if out.verb_lemma.is_none() && added.iter().any(is_finite_predicate) {
            out.verb_lemma = Some(base.clone());
        }
    }
    if !matched_any {
        return resolve_cheap_fallback(lower, cat);
    }
    if out.lemma.is_none() {
        out.lemma = bases.first().cloned();
    }
    out.exact = true;
    add_negation(&mut out, lower);
    finish(&mut out, Depth::StemMap, lower);
    Some(out)
}

/// The fallback when the stem map names no base or names one that cannot
/// emit the surface.
///
/// A classless compound and a closed-class pronoun have no stem-map entry, and
/// a suffixed surface is indexed by its host. The cheap gate names those, and
/// the single-paradigm verification keeps the result a reading, not a guess.
fn resolve_cheap_fallback(lower: &str, cat: &Catalogues) -> Option<WordInfo> {
    let base = cheap_base(lower)?;
    if is_classless_lemma(&base)
        && let Some(info) = resolve_from_base_surface(lower, &base, cat)
    {
        return Some(info);
    }
    if let Some(info) = resolve_compound_surface(lower, &base, cat) {
        return Some(info);
    }
    if is_closed_class_base(&base)
        && let Some(info) = resolve_from_base_surface(lower, &base, cat)
    {
        return Some(info);
    }
    None
}

/// Whether the lexicon lists the base as a closed-class pronoun.
///
/// The stem maps cover nouns and verbs only. A pronoun's inflected form can be
/// named by the cheap gate (`jossa` -> `joka`) but has no stem-map entry, so
/// the shallow resolver verifies the single paradigm instead of leaving the
/// form unknown.
fn is_closed_class_base(word: &str) -> bool {
    crate::kotus::sanalista().by_word(word).any(|row| {
        matches!(
            PartOfSpeech::parse_all(row.part_of_speech()).first(),
            Some(PartOfSpeech::Pronoun)
        )
    })
}

/// Whether the lexicon lists the base without an inflection class.
///
/// A classless row is a compound: Kotus omits the type because it declines
/// like its head. `menevä` is a classed adjective and must not be treated as
/// one just because `head_inflections` can split its spelling.
fn is_classless_lemma(word: &str) -> bool {
    crate::kotus::sanalista()
        .by_word(word)
        .next()
        .is_some_and(|row| row.inflections().next().is_none())
}

/// Verify the whole surface against one named base, including possessives.
fn resolve_from_base_surface(surface: &str, base: &str, cat: &Catalogues) -> Option<WordInfo> {
    if base == surface {
        return None;
    }
    let rows: Vec<_> = crate::kotus::sanalista().by_word(base).collect();
    if rows.is_empty() {
        return None;
    }
    let pos = rows
        .iter()
        .find_map(|row| {
            PartOfSpeech::parse_all(row.part_of_speech())
                .first()
                .copied()
        })
        .unwrap_or(PartOfSpeech::Unknown);
    // A lemma can have more than one sanalista row (`joka` is class 99 and
    // class 101). Collect every row's inflections: the first row alone can be
    // an irregular model that does not spell the surface.
    let inflections: Vec<_> = rows.iter().flat_map(|row| row.inflections()).collect();
    // A classless compound row has no class of its own. `add_forms_from_lemma`
    // with an empty inflection list runs the classless-compound path
    // (`compound_head_forms`), which covers both a free head (`pariskunta` =
    // `paris` + `kunta`) and a bound suffix head (`salaperäinen` =
    // `sala` + `‑peräinen`). `head_inflections` deliberately refuses the
    // bound head, so it is not the right source here.
    let inflections: Vec<_> = if inflections.is_empty() {
        Vec::new()
    } else {
        inflections
    };
    let mut out = WordInfo {
        lemma: Some(base.to_string()),
        ..WordInfo::default()
    };
    if !add_forms_from_lemma(base, &inflections, Some(pos), surface, cat, &mut out) {
        return None;
    }
    out.exact = true;
    if out.nominal_lemma.is_none()
        && out
            .readings
            .iter()
            .any(|reading| is_nominal_pos(reading.part_of_speech))
    {
        out.nominal_lemma = Some(base.to_string());
    }
    if out.verb_lemma.is_none() && out.readings.iter().any(is_finite_predicate) {
        out.verb_lemma = Some(base.to_string());
    }
    finish(&mut out, Depth::StemMap, surface);
    add_possessive_readings(&mut out, surface, cat);

    // The cheap gate names one base (`roihu` for `roihusi`), but the surface
    // can also be a verb form (`roihuta`). Add the other stem-map base so a
    // verb/nominal ambiguity stays visible to the role rules.
    for other in [
        find_base_via_stem_map(surface, PartOfSpeech::Noun, false),
        find_base_via_verb_stem_map(surface, false),
    ]
    .into_iter()
    .flatten()
    {
        if other == base {
            continue;
        }
        let before = out.readings.len();
        if !add_readings_from_base_matched(&other, surface, cat, &mut out.readings) {
            continue;
        }
        out.push_lemma(&other);
        let added = &out.readings[before..];
        if out.nominal_lemma.is_none()
            && added
                .iter()
                .any(|reading| is_nominal_pos(reading.part_of_speech))
        {
            out.nominal_lemma = Some(other.clone());
        }
        if out.verb_lemma.is_none() && added.iter().any(is_finite_predicate) {
            out.verb_lemma = Some(other);
        }
    }
    finish(&mut out, Depth::StemMap, surface);
    Some(out)
}

/// Resolve a compound surface from the head base the DP/state machine names.
///
/// `kotirantaan` is `koti` + the illative of `ranta`, `lempiviikkomme` is
/// `lempi` + the possessive form of `viikko`. `check_normal_fast` already
/// segments the compound and returns the head (`ranta`, `viikko`); this
/// inflects that head and accepts the surface when the remaining prefix is a
/// known modifier. No head/tail candidate walk is needed.
fn resolve_compound_surface(surface: &str, base: &str, cat: &Catalogues) -> Option<WordInfo> {
    if base == surface {
        return None;
    }
    let row = crate::kotus::sanalista().by_word(base).next()?;
    let pos = PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Noun);
    let inflections: Vec<_> = row.inflections().collect();
    let mut best: Option<(String, Vec<Tags>)> = None;
    for form in crate::expand::inflect_all(base, &inflections, cat, None, pos, true) {
        if form.word.contains(' ') || form.word.len() >= surface.len() {
            continue;
        }
        let Some(prefix) = surface.strip_suffix(&form.word) else {
            continue;
        };
        if prefix.is_empty() || !crate::expand::modifier_is_known(prefix) {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(known, _)| prefix.len() > known.len())
        {
            best = Some((
                prefix.to_string(),
                form.readings
                    .iter()
                    .map(|reading| reading_with_pos(*reading, Some(pos)))
                    .collect(),
            ));
        }
    }
    let (prefix, head_readings) = best?;
    let whole = format!("{prefix}{base}");

    // Prefer the whole classless compound's own paradigm. The head's form can
    // carry several readings (`puuronsa` is both a genitive and a possessive
    // nominative), while the compound paradigm knows the slot the surface is
    // in. When the head cannot classify the whole, keep the head readings.
    let readings = crate::expand::head_inflections(&whole, cat, None)
        .and_then(|inflections| {
            crate::expand::inflect_all(&whole, &inflections, cat, None, pos, true)
                .into_iter()
                .find(|form| form.word == surface)
                .map(|form| {
                    form.readings
                        .iter()
                        .map(|reading| reading_with_pos(*reading, Some(pos)))
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or(head_readings);

    let mut out = WordInfo {
        lemma: Some(whole.clone()),
        nominal_lemma: Some(whole.clone()),
        exact: true,
        lemmas: vec![whole],
        readings,
        ..WordInfo::default()
    };
    if out.readings.iter().any(is_finite_predicate) {
        out.verb_lemma = Some(base.to_string());
    }
    finish(&mut out, Depth::Compound, surface);
    Some(out)
}

/// Partitive-plural endings whose surface can belong to more than one lemma.
fn needs_full_base_search(lower: &str) -> bool {
    const AMBIGUOUS: &[&str] = &[
        "ja", "jä", "ia", "iä", "ita", "itä", "uja", "yjä", "oja", "öjä", "eja", "ejä",
    ];
    AMBIGUOUS.iter().any(|suffix| lower.ends_with(suffix))
}

/// The no-clitic recognition walk. Reached only when the cheap sources miss.
/// The ordinary, no-clitic resolution.
fn resolve_word_slow(
    lexicon: &[crate::kotus::Lemma<'_>],
    cat: &Catalogues,
    lower: &str,
) -> WordInfo {
    // Recognition-only search: it uses the shared paradigm probe rather than
    // building the suggestion pool's forms and neighbours for every candidate.
    // The readings are produced below for the few winning bases only.
    let report = find_bases_for_analysis(lexicon, cat, lower);
    let mut out = WordInfo {
        exact: !report.direct.is_empty(),
        ..WordInfo::default()
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
        out.push_lemma(&base);
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
        out.push_lemma(&base);
        if out.readings.iter().any(is_finite_predicate) {
            out.verb_lemma = Some(base);
        }
    }

    // A direct lemma or a base with no retained form still has one reading:
    // the sanalista's part of speech and, for a nominal, the citation case.
    if out.readings.is_empty() {
        for direct in &report.direct {
            add_reading(&mut out.readings, direct_reading(direct));
            out.push_lemma(direct.word());
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
                    out.push_lemma(base.word);
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

    add_negation(&mut out, lower);

    let depth = if out.readings.is_empty() {
        Depth::Unknown
    } else {
        Depth::HeadsTails
    };
    finish(&mut out, depth, lower);
    out
}

/// Compute the agreed nominal case and number once, at insert time.
fn fill_case_number(out: &mut WordInfo) {
    out.case = agreed_nominal_case(&out.readings);
    out.number = out.case.and_then(|case| agreed_number(&out.readings, case));
}

fn is_agreed_nominal(pos: Option<PartOfSpeech>) -> bool {
    matches!(
        pos,
        Some(
            PartOfSpeech::Noun
                | PartOfSpeech::Adjective
                | PartOfSpeech::Pronoun
                | PartOfSpeech::Ordinal
        )
    )
}

fn agreed_nominal_case(readings: &[Tags]) -> Option<Case> {
    let mut case = None;
    for reading in readings {
        if !is_agreed_nominal(reading.part_of_speech) {
            continue;
        }
        let Some(current) = reading.case else {
            continue;
        };
        match case {
            None => case = Some(current),
            Some(known) if known == current => {}
            Some(_) => return None,
        }
    }
    case
}

fn agreed_number(readings: &[Tags], case: Case) -> Option<Number> {
    let mut number = None;
    for reading in readings.iter().filter(|reading| reading.case == Some(case)) {
        let Some(current) = reading.number else {
            continue;
        };
        match number {
            None => number = Some(current),
            Some(known) if known == current => {}
            Some(_) => return None,
        }
    }
    number
}

/// Add the negation auxiliary when the closed table names one.
fn add_negation(out: &mut WordInfo, lower: &str) {
    let Some(base) = crate::expand::negation::negation_base(lower) else {
        return;
    };
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
    out.push_lemma(base);
}

/// Add the bases the stem maps name, skipping the direct lemmas already read.
///
/// A direct lemma can hide a homonym: `pieniä` is a verb, but it is also the
/// partitive plural of the adjective `pieni`, and `tuli` is a noun and the
/// past tense of `tulla`. The stem maps name the hidden base cheaply.
fn add_stem_map_bases(out: &mut WordInfo, lower: &str, cat: &Catalogues, skip: &[&str]) {
    let mut bases: Vec<String> = Vec::new();
    if let Some(base) = find_base_via_stem_map(lower, PartOfSpeech::Noun, false) {
        bases.push(base);
    }
    if let Some(base) = find_base_via_verb_stem_map(lower, false)
        && !bases.contains(&base)
    {
        bases.push(base);
    }
    for base in bases {
        if skip.contains(&base.as_str()) {
            continue;
        }
        let before = out.readings.len();
        if !add_readings_from_base_matched(&base, lower, cat, &mut out.readings) {
            continue;
        }
        out.push_lemma(&base);
        let added = &out.readings[before..];
        if out.nominal_lemma.is_none()
            && added
                .iter()
                .any(|reading| is_nominal_pos(reading.part_of_speech))
        {
            out.nominal_lemma = Some(base.clone());
        }
        if out.verb_lemma.is_none() && added.iter().any(is_finite_predicate) {
            out.verb_lemma = Some(base);
        }
    }
}

/// Add a nominal possessive reading to a verb-only fast result.
///
/// `roihusi` is the past tense of `roihtua`, but it is also `roihu` + `-si`
/// ("your blaze"). The old full search saw both and marked the role `Weak`;
/// the cheap path sees only the verb. The host's noun reading is cheap to add
/// and keeps the ambiguity that the role rules expect.
fn add_possessive_readings(out: &mut WordInfo, lower: &str, cat: &Catalogues) {
    if out
        .readings
        .iter()
        .any(|reading| is_nominal_pos(reading.part_of_speech))
    {
        return;
    }
    let Some((host, possessive)) = strip_possessive(lower) else {
        return;
    };
    let Some(mut host_info) = resolve_word_fast(cat, host) else {
        return;
    };
    if !host_info
        .readings
        .iter()
        .any(|reading| is_nominal_pos(reading.part_of_speech))
    {
        return;
    }
    for reading in &mut host_info.readings {
        reading.possessive = Some(possessive);
    }
    let WordInfo {
        readings,
        lemmas,
        nominal_lemma,
        ..
    } = host_info;
    for reading in readings {
        add_reading(&mut out.readings, reading);
    }
    for lemma in lemmas {
        out.push_lemma(&lemma);
    }
    if out.nominal_lemma.is_none() {
        out.nominal_lemma = nominal_lemma;
    }
    out.possessive = Some(possessive);
    let depth = out.depth;
    finish(out, depth, lower);
}

fn finish(out: &mut WordInfo, depth: Depth, lower: &str) {
    if out.key != lower {
        out.key.clear();
        out.key.push_str(lower);
    }
    fill_case_number(out);
    out.part_of_speech = out
        .readings
        .iter()
        .find_map(|reading| reading.part_of_speech);
    out.clitic = out.readings.iter().find_map(|reading| reading.clitic);
    out.possessive = out.readings.iter().find_map(|reading| reading.possessive);
    if out.readings.is_empty() {
        out.derivation = crate::derivation::reverse(lower).into_iter().next();
    }
    out.depth = depth;
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
/// built. `possessives` is paid only when the surface actually carries a
/// possessive or clitic; the host path has already stripped one otherwise.
fn add_forms_from_lemma(
    base: &str,
    inflections: &[crate::kotus::Inflection],
    pos: Option<PartOfSpeech>,
    lower: &str,
    cat: &Catalogues,
    out: &mut WordInfo,
) -> bool {
    let pos = pos.unwrap_or(PartOfSpeech::Unknown);
    // Class 50/51 compounds need the lexicon index to split their parts; the
    // wrapper is free (two precomputed orders over the embedded lexicon).
    let index = crate::expand::LexiconIndex::new(crate::kotus::lemmas());
    let Some(readings) = crate::expand::form_readings(
        base,
        inflections,
        cat,
        Some(&index),
        pos,
        surface_carries_suffix(lower),
        lower,
    ) else {
        return false;
    };
    if out.lemma.is_none() {
        out.lemma = Some(base.to_string());
    }
    out.push_lemma(base);
    for reading in readings {
        let tags = reading_with_pos(reading, Some(pos));
        add_reading(&mut out.readings, tags);
        if is_finite_predicate(&tags) {
            out.verb_lemma = Some(base.to_string());
        }
        if is_nominal_pos(tags.part_of_speech) {
            out.nominal_lemma = Some(base.to_string());
        }
    }
    true
}

/// Inflect `base` and add the readings for `lower`.
///
/// The fallback citation reading keeps the slow resolver useful for a base the
/// engine cannot generate. The fast path uses
/// [`add_readings_from_base_matched`] instead: a fallback reading for a
/// mismatched base (`jotka` as a singular `joka`) would be wrong.
fn add_readings_from_base(base: &str, lower: &str, cat: &Catalogues, readings: &mut Vec<Tags>) {
    let _ = add_readings_from_base_impl(base, lower, cat, readings, true);
}

/// Add only readings from a form that actually matched the surface.
fn add_readings_from_base_matched(
    base: &str,
    lower: &str,
    cat: &Catalogues,
    readings: &mut Vec<Tags>,
) -> bool {
    add_readings_from_base_impl(base, lower, cat, readings, false)
}

fn add_readings_from_base_impl(
    base: &str,
    lower: &str,
    cat: &Catalogues,
    readings: &mut Vec<Tags>,
    fallback: bool,
) -> bool {
    let Some(row) = crate::kotus::sanalista().by_word(base).next() else {
        return false;
    };
    let pos = PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Unknown);
    let inflections: Vec<_> = row.inflections().collect();
    let index = crate::expand::LexiconIndex::new(crate::kotus::lemmas());
    let mut matched = false;
    if let Some(found) = crate::expand::form_readings(
        base,
        &inflections,
        cat,
        Some(&index),
        pos,
        surface_carries_suffix(lower),
        lower,
    ) {
        matched = true;
        for reading in found {
            add_reading(readings, reading_with_pos(reading, Some(pos)));
        }
    }
    if !matched && fallback {
        add_reading(readings, direct_reading(&row));
    }
    matched
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

/// A `WordInfo` for a capitalized token that the proper-name table knows.
///
/// The entry is not inserted into the cache: the key is the lowercased
/// surface, and a name's reading depends on the token's capitalization.
fn proper_name_word(
    info: &WordInfo,
    surface: &str,
    name: &str,
    case: Option<Case>,
    key: &str,
) -> WordInfo {
    let mut readings = info.readings.clone();
    add_reading(&mut readings, proper_name_reading(surface, name, case));
    let mut lemmas = info.lemmas.clone();
    if !lemmas.iter().any(|known| known == name) {
        lemmas.push(name.to_string());
    }
    let mut out = WordInfo {
        key: String::new(),
        lemma: Some(name.to_string()),
        verb_lemma: info.verb_lemma.clone(),
        nominal_lemma: Some(name.to_string()),
        exact: info.exact,
        lemmas,
        readings,
        case: None,
        number: None,
        part_of_speech: None,
        clitic: None,
        possessive: None,
        derivation: None,
        depth: Depth::ProperName,
    };
    finish(&mut out, Depth::ProperName, key);
    out
}

/// A reading for a recognized proper name.
///
/// A recorded case wins. Without one, the citation spelling is nominative and
/// any other form stays case-less: role selection then leaves it `Unknown`
/// instead of inventing a nominative.
fn proper_name_reading(surface: &str, lemma: &str, case: Option<Case>) -> Tags {
    if let Some(case) = case {
        Tags::nominal(case).of_pos(PartOfSpeech::Noun)
    } else if surface.eq_ignore_ascii_case(lemma) {
        Tags::nominal(Case::Nominative).of_pos(PartOfSpeech::Noun)
    } else {
        Tags::UNKNOWN.of_pos(PartOfSpeech::Noun)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{Mood, Person, Tense};
    use crate::tokenise::tokenise;

    fn analyzed(text: &str) -> (MorphologyAnalysis, MorphologyCache) {
        let tokens = tokenise(text);
        let mut cache = MorphologyCache::new();
        let morphology = analyze_morphology(&tokens, &mut cache);
        (morphology, cache)
    }

    #[test]
    fn a_closed_class_pronoun_form_resolves_in_the_shallow_pass() {
        // `joka` has two sanalista rows (99 and 101). The first row alone does
        // not spell `jossa`, and pronouns are absent from the stem maps, so
        // the shallow resolver must verify every row of the cheap base.
        let (morphology, _) = analyzed("jossa");
        let info = morphology.word(0).expect("jossa resolved");
        assert_eq!(info.lemma(), Some("joka"));
        let inessive_pronoun = info.readings().iter().any(|reading| {
            reading.part_of_speech == Some(PartOfSpeech::Pronoun)
                && reading.case == Some(Case::Inessive)
        });
        assert!(inessive_pronoun);
        assert_eq!(info.depth(), Depth::StemMap);
    }

    #[test]
    fn a_stem_map_form_caches_case_and_number() {
        let (morphology, _) = analyzed("talossa");
        let info = morphology.word(0).expect("talossa resolved");
        assert_eq!(info.lemma(), Some("talo"));
        assert_eq!(info.case(), Some(Case::Inessive));
        assert_eq!(info.number(), Some(Number::Singular));
        assert_eq!(info.depth(), Depth::StemMap);
    }

    #[test]
    fn a_classless_compound_uses_its_head_class() {
        // `pariskunta` has no class of its own in the sanalista; `kunta` is
        // 10*J and `paris` is the combining `-s` form of `pari`.
        let (morphology, _) = analyzed("pariskunnan");
        let info = morphology.word(0).expect("pariskunnan resolved");
        assert_eq!(info.lemma(), Some("pariskunta"));
        assert_eq!(info.case(), Some(Case::Genitive));
        assert_eq!(info.number(), Some(Number::Singular));
        assert_eq!(info.depth(), Depth::StemMap);
    }

    #[test]
    fn a_clitic_is_cached_on_the_host() {
        let (morphology, _) = analyzed("onko");
        let info = morphology.word(0).expect("onko resolved");
        assert_eq!(info.lemma(), Some("olla"));
        assert_eq!(info.clitic(), Some(Clitic::Ko));
        assert_eq!(info.depth(), Depth::Clitic);
    }

    #[test]
    fn a_possessive_is_cached_with_the_case() {
        let (morphology, _) = analyzed("talossani");
        let info = morphology.word(0).expect("talossani resolved");
        assert_eq!(info.lemma(), Some("talo"));
        assert_eq!(info.case(), Some(Case::Inessive));
        assert_eq!(info.number(), Some(Number::Singular));
        assert_eq!(info.possessive(), Some(Possessive::FirstSingular));
    }

    #[test]
    fn the_part_of_speech_summary_comes_from_a_reading() {
        let (morphology, _) = analyzed("koira");
        let info = morphology.word(0).expect("koira resolved");
        assert_eq!(info.part_of_speech(), Some(PartOfSpeech::Noun));
    }

    #[test]
    fn the_cache_counts_hits_and_misses() {
        let (_, cache) = analyzed("kissa kissa");
        let stats = cache.stats();
        assert_eq!(stats.words, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
    }

    #[test]
    fn a_verb_reading_keeps_person_tense_and_mood() {
        let (morphology, _) = analyzed("luen");
        let info = morphology.word(0).expect("luen resolved");
        assert!(info.readings().iter().any(|reading| {
            reading.part_of_speech == Some(PartOfSpeech::Verb)
                && reading.person == Some(Person::First)
                && reading.tense == Some(Tense::Present)
                && reading.mood == Some(Mood::Indicative)
        }));
    }

    #[test]
    fn an_unresolved_word_records_a_derivation_candidate() {
        let mut info = WordInfo::default();
        finish(&mut info, Depth::Unknown, "puhuminen");
        let derivation = info.derivation().expect("a productive derivation");
        assert_eq!(derivation.rule, "ACTION_MINEN");
        assert_eq!(derivation.source, "puhua");
    }

    #[test]
    fn separators_have_no_word_entry() {
        let (morphology, _) = analyzed("kissa, koira");
        assert!(morphology.word(1).is_none(), "the comma is a separator");
        assert!(morphology.word(0).is_some());
        assert!(morphology.word(2).is_some());
    }

    /// A numeral governs a partitive, and the shallow `lasta` (a splinter
    /// lemma) hides the partitive of `lapsi`. The numeral upgrade is the one
    /// automatic deep walk left in the first pass.
    #[test]
    fn a_numeral_upgrades_the_partitive_homonym() {
        let (morphology, _) = analyzed("kaksi lasta");
        let info = morphology
            .tokens
            .iter()
            .filter_map(|token| token.as_deref())
            .next_back()
            .expect("lasta resolved");
        assert!(
            info.readings()
                .iter()
                .any(|reading| reading.case == Some(Case::Partitive)),
            "lasta should gain the partitive reading: {:?}",
            info.readings()
        );
    }

    /// A direct lemma keeps the hidden verb reading the stem map names.
    #[test]
    fn a_direct_lemma_keeps_the_hidden_verb_reading() {
        let (morphology, _) = analyzed("tuli");
        let info = morphology.word(0).expect("tuli resolved");
        assert!(
            info.readings()
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Noun))
        );
        assert!(
            info.readings()
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Verb))
        );
    }

    /// `kuusi` is the tree and the numeral; both homonym rows are read.
    #[test]
    fn kuusi_keeps_the_tree_and_numeral_readings() {
        let (morphology, _) = analyzed("kuusi");
        let info = morphology.word(0).expect("kuusi resolved");
        assert!(
            info.readings()
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Numeral))
        );
        assert!(
            info.readings()
                .iter()
                .any(|reading| reading.part_of_speech == Some(PartOfSpeech::Noun))
        );
    }
}
