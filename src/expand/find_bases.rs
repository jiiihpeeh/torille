//! Find the lemma(s) a given word is a form of, using the compile-time index.
//!
//! The full reverse index inflects every lemma in the sanalista to answer
//! "which lemma is this a form of?", and that takes ~30 s per invocation. For a
//! single-word lookup the answer lies in a small neighbourhood of the query, so
//! this module runs four phases and stops at the first one that finds anything:
//!
//!   1. **heads + tails** -- the same candidates the compound splitter uses
//!      ([`LexiconIndex::heads_of`], [`LexiconIndex::tails_of`]). Median bucket
//!      is three lemmas, so the cost is small. Catches every word whose lemma
//!      shares a [`PREFIX`](super::lexicon_index::PREFIX)=4 char run with the
//!      form, which is most words.
//!   2. **prefix-3 walk** -- when phase 1 fails, walk the lexicographic run
//!      of lemmas starting with the form's first three characters, inflect
//!      each, and verify. Covers stem-changing cases (`merelle` <- `meri`,
//!      `katossa` <- `katto`) where the strong stem and the inflected form
//!      share three but not four leading characters.
//!   3. **prefix-2 walk** -- the same with prefix-2, only when phase 2
//!      misses. Catches the gradated forms whose lemma diverges from the
//!      weak stem in the third character (`vesi` <- `vedestä`,
//!      `puhdas` <- `puhtaan`).
//!   4. **short-lemma scan** -- every lemma of length <= SHORTEST_HEAD, run
//!      through `inflect_all`. Catches the rare case where the lemma shares
//!      no leading characters with the form at all (`öihin` <- `yö`).
//!
//! This module is the engine-level home of the algorithm; the convenience
//! wrappers that load the embedded lexicon and cache it live in the tools
//! crate and in [`crate::suggest`].

use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::expand::clitic::strip_clitic;
use crate::expand::{ClassPrior, LexiconIndex, inflect_all};
use crate::kotus::{EmbeddedLemma, Lemma, sanalista};
use crate::spec::Catalogues;
use crate::tags::{Form, PartOfSpeech};

/// A form generated during the head/tail/prefix walks, paired with the lemma
/// that produced it.
///
/// `find_bases_with` already iterates every candidate lemma's full paradigm
/// once, and `Form` carries the morphological readings and the generation
/// confidence that callers may rank on. Earlier revisions threw that data
/// away and only kept the surface string, which forced every consumer
/// (`suggest`, tools) to re-inflect to recover it. [`SourcedForm`]
/// is the alternative: keep the lemma + the [`Form`] it emitted, so callers
/// can rank on morphology without redoing the generation.
pub struct SourcedForm<'a> {
    /// The lemma whose paradigm generated this form.
    pub lemma: &'a str,
    /// The form itself: its surface word, its readings, and the confidence
    /// with which the engine produced it (see [`Confidence`]).
    pub form: Form,
}

impl std::fmt::Debug for SourcedForm<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourcedForm")
            .field("lemma", &self.lemma)
            .field("word", &self.form.word)
            .field("confidence", &self.form.confidence)
            .field("readings", &self.form.readings.len())
            .finish()
    }
}

/// What a word is a form of.
///
/// Three layers, because spell-checking and morphological lookups want
/// different things from the sanalista:
///
///   * `direct` -- every row whose `word` field equals the query. This is the
///     answer to "is this a lemma in the lexicon?", one hash probe deep.
///   * `bases` -- the lemmas whose inflected forms include the query, in the
///     order the phases produced them. This is the answer to "what word did
///     the user mean?" -- the spell checker's main ask.
///   * `neighbours` -- lemmas that share a head, a tail, or a short prefix
///     with the query but don't inflect to it. These are the candidates a
///     spell checker would compare against the query for an edit distance.
pub struct BasesReport<'a> {
    pub direct: Vec<EmbeddedLemma>,
    pub bases: Vec<&'a Lemma<'a>>,
    pub neighbours: Vec<&'a Lemma<'a>>,
    /// Inflected forms generated during the head/tail/prefix walks, paired
    /// with the lemma that produced them. Retained so `suggest` can score
    /// them against the input via Levenshtein and so callers can
    /// see the form's readings and generation confidence. Many misspellings
    /// are within one edit of a valid inflected form whose lemma is too far
    /// from the input for the lemma-only path to consider; e.g. "jooksee" ->
    /// "juoksee" (the form is one insertion away from the input, but the
    /// lemma "juosta" is six edits away).
    pub forms: Vec<SourcedForm<'a>>,
}

impl<'a> std::fmt::Debug for BasesReport<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasesReport")
            .field(
                "direct",
                &self.direct.iter().map(|d| d.word()).collect::<Vec<_>>(),
            )
            .field(
                "bases",
                &self.bases.iter().map(|l| l.word).collect::<Vec<_>>(),
            )
            .field(
                "neighbours",
                &self.neighbours.iter().map(|l| l.word).collect::<Vec<_>>(),
            )
            .field("forms", &self.forms.len())
            .finish()
    }
}

/// Find every lemma the engine thinks the given word is a form of, in
/// phases ordered from cheapest to most exhaustive. See the module doc for
/// the five phases and what each catches.
///
/// `lexicon` and `cat` come from the embedded sanalista and paradigm
/// catalogue; a convenience wrapper that loads them for the caller is in
/// [`crate::suggest::check`].
///
/// Phase 5 is the clitic phase: when none of the four prefix walks find a
/// base, try stripping a single clitic (`-ko`/`-kö`, `-kin`, `-kaan`/`-kään`,
/// `-han`/`-hän`, `-pa`/`-pä`) and re-running the lookup on the host. A
/// word like `siellähän` is `siellä` (a lemma) with `-hän`. The colloquial
/// `-s` (`näetkös`, `mitäs`) is not accepted: it is spoken-language, so the
/// checker flags the word and the suggestion pool offers the s-less host.
/// Stacking is handled by recursion with the clitic phase disabled, so
/// `siellähänkin` peels to `siellä` correctly.
pub fn find_bases_with<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
) -> BasesReport<'a> {
    find_bases_with_inner(
        lexicon, cat, word, /* with_clitic_phase */ true, /* keep_form */ None,
        /* collect_neighbours */ true, /* recognition_only */ false,
        /* cheap_recognition */ false, /* all_phases */ false,
        /* keep_walking_with_direct */ false,
    )
}

/// Like [`find_bases_with`], but only retains generated forms for which
/// `keep_form` returns `true`.
///
/// The base/neighbour/direct lists are unchanged. This exists for `suggest`,
/// which passes its edit-distance budget here so the head-1 walk does not
/// allocate a [`SourcedForm`] (and clone its readings) for every form of
/// every lemma when almost all of them will be discarded by the ranker.
pub(crate) fn find_bases_with_form_filter<'a, F>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
    keep_form: F,
) -> BasesReport<'a>
where
    F: Fn(&str) -> bool + Sync,
{
    find_bases_with_inner(
        lexicon,
        cat,
        word,
        /* with_clitic_phase */ true,
        Some(&keep_form),
        /* collect_neighbours */ true,
        /* recognition_only */ false,
        /* cheap_recognition */ false,
        /* all_phases */ false,
        /* keep_walking_with_direct */ false,
    )
}

/// Every base the walks can prove, not just the first phase that hits.
///
/// [`find_bases_with`] stops once a phase finds a base; this diagnostic
/// variant runs phases 2-4 as well, so `check_normal --all` can show the later
/// walks' bases (`vireissä` reaches `viri` only in the prefix walk). It keeps
/// no generated-form payload and no neighbours, so the added cost is the
/// verification of each phase's candidates.
pub fn find_all_bases_with<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
) -> BasesReport<'a> {
    find_bases_with_inner(
        lexicon,
        cat,
        word,
        /* with_clitic_phase */ true,
        Some(&|_| false),
        /* collect_neighbours */ false,
        /* recognition_only */ false,
        /* cheap_recognition */ false,
        /* all_phases */ true,
        /* keep_walking_with_direct */ false,
    )
}

/// The recognition-only variant of [`find_bases_with`]: bases and direct
/// lemmas, no generated-form payload and no neighbour list.
pub(crate) fn find_bases_for_check<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
) -> BasesReport<'a> {
    find_bases_with_inner(
        lexicon,
        cat,
        word,
        /* with_clitic_phase */ true,
        Some(&|_| false),
        /* collect_neighbours */ false,
        /* recognition_only */ true,
        /* cheap_recognition */ false,
        /* all_phases */ false,
        /* keep_walking_with_direct */ false,
    )
}

/// The recognition-only search for callers that need every reading of a
/// surface, not just the first base.
///
/// A direct lemma short-circuits [`find_bases_for_check`], which is right for
/// the checker but wrong for the syntax layer: `lasta` is a lemma (a splinter)
/// and also the partitive of `lapsi`, and stopping at the direct lemma would
/// lose the subject/object reading. This variant keeps walking after a direct
/// lemma, but still uses the cheap paradigm probe and stops at the first walk
/// phase that finds a base.
pub(crate) fn find_bases_for_analysis<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
) -> BasesReport<'a> {
    // The extra walk exists to recover a homonym hidden behind a direct
    // lemma (`lasta` is a lemma and the partitive of `lapsi`). A word with
    // no direct lemma has nothing to hide behind, so it can stop as soon as
    // phase 1 finds its base.
    let has_direct = crate::kotus::sanalista()
        .by_word(&word.to_ascii_lowercase())
        .next()
        .is_some();
    find_bases_with_inner(
        lexicon,
        cat,
        word,
        /* with_clitic_phase */ true,
        Some(&|_| false),
        /* collect_neighbours */ false,
        /* recognition_only */ true,
        /* cheap_recognition */ true,
        /* all_phases */ false,
        /* keep_walking_with_direct */ has_direct,
    )
}

/// Worker count for the parallel walks.
///
/// `TORILLE_SINGLE_THREAD=1` forces the sequential path. The wasm always
/// takes it (one thread); the native performance probe uses the switch to
/// measure the same shape as the extension instead of the host's core count.
pub(super) fn worker_count() -> usize {
    static SINGLE: OnceLock<bool> = OnceLock::new();
    if *SINGLE.get_or_init(|| std::env::var_os("TORILLE_SINGLE_THREAD").is_some())
        || FORCE_SINGLE_THREAD.load(Ordering::Relaxed)
    {
        return 1;
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1)
}

/// Set by [`set_single_thread`] for callers that want the wasm's shape.
static FORCE_SINGLE_THREAD: AtomicBool = AtomicBool::new(false);

/// Force the engine's parallel walks onto one thread, or release them.
///
/// The wasm is single-threaded already. Native callers use this to measure
/// the extension's shape (the performance probe) or to make a test
/// deterministic. It changes only *how many workers* run; the answer is
/// identical.
pub fn set_single_thread(force: bool) {
    FORCE_SINGLE_THREAD.store(force, Ordering::Relaxed);
}

/// Run `f` on every candidate in `candidates` in parallel across
/// the available cores, returning the first `Some(_)` it
/// produces. Candidates the function has not yet visited when a
/// peer thread reports a hit are skipped. The wasm is
/// single-threaded; on the wasm the closure runs sequentially
/// over the whole slice.
fn par_first_some<'a, T, F>(candidates: &'a [T], f: F) -> Option<&'a T>
where
    F: Fn(&'a T) -> bool + Send + Sync,
    T: Send + Sync,
{
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;

    let workers = worker_count();
    // Single-threaded fast path: the parallel machinery's
    // overhead is larger than the work for tiny candidate sets,
    // and the wasm is single-threaded by design.
    if workers == 1 {
        for c in candidates {
            if f(c) {
                return Some(c);
            }
        }
        return None;
    }
    // Multi-threaded: split into `workers` chunks; one thread
    // per chunk; the first thread to find a match flips the
    // shared flag and stores the result; the others check the
    // flag between candidates and bail. `std::thread::scope`
    // joins automatically, so we don't have to manage the
    // `JoinHandle`s.
    //
    // `Arc<F>` so every worker can hold a clone of the closure
    // without moving the original. `F: Send + Sync` is the
    // bound the closure body has to satisfy; the captured
    // environment is `&Catalogues`, `&str`, `PartOfSpeech`, and
    // `bool`, all `Send + Sync`.
    let f: Arc<F> = Arc::new(f);
    let found = Arc::new(AtomicBool::new(false));
    // Empty candidate set: nothing to walk. Bail before the
    // `chunks()` call below -- with workers > 1 the computed
    // chunk size would be 0 and `slice::chunks(0)` panics. The
    // single-threaded path above already handles this, so this is
    // purely a multi-threaded-edge guard.
    if candidates.is_empty() {
        return None;
    }
    let result: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
    let chunk_size = candidates.len().div_ceil(workers);
    let chunks: Vec<&[T]> = candidates.chunks(chunk_size).collect();
    thread::scope(|s| {
        for (i, chunk) in chunks.iter().enumerate() {
            let f = Arc::clone(&f);
            let found = Arc::clone(&found);
            let result = Arc::clone(&result);
            s.spawn(move || {
                for (j, c) in chunk.iter().enumerate() {
                    if found.load(Ordering::Relaxed) {
                        return;
                    }
                    if f(c) {
                        found.store(true, Ordering::Relaxed);
                        let idx = i * chunk_size + j;
                        *result.lock().expect("result lock poisoned") = Some(idx);
                        return;
                    }
                }
            });
        }
    });
    result
        .lock()
        .expect("result lock poisoned")
        .map(|i| &candidates[i])
}

///
/// The full [`find_bases_with`] walks the whole prefix space and
/// collects every lemma whose paradigm emits the query word; for
/// `juoksemme` that's 3097 candidates and 165 generated forms, even
/// though the inflect check only needs the *first* matching base
/// to answer the question "is this a real Finnish word?". The
/// rest is the demo's suggestion pool's input -- the ranker needs
/// every candidate to score the closest edit distance -- but the
/// typing path doesn't.
///
/// `find_any_base_with` runs the same five phases, but stops the
/// instant a candidate's `inflect_all` produces the query. For
/// `juoksemme` the first candidate that emits the word is `juosta`
/// (in the prefix-3 walk), so the remaining 3096 candidates'
/// `inflect_all` calls never run. For a 100-word text where every
/// word is a real inflected form, this is a ~100x speedup on the
/// slow path.
///
/// **Multiple bases are possible.** A word like `voidaan` is a
/// form of `voida` (modal verb, 1sg passive) and also
/// morphologically similar to several `voida`-class entries; the
/// sanalista carries homonyms. The inflect check accepts the word
/// as soon as ANY base matches, so the short-circuit is correct
/// for the yes/no question. Callers that need the full
/// candidate set (the suggestion pool) use
/// [`find_bases_with`], which collects every match.
///
/// **The returned base is "the first one we hit", not "the
/// canonical one".** For homonyms, the sanalista's order is the
/// order of the embedding (lemma HashSet iteration, not
/// alphabetical), so the first match is deterministic but not
/// semantically chosen. Diagnostic output should say "first base"
/// not "the base" -- see the verbose mode of `check_normal` for
/// an example.
///
/// The clitic phase (phase 5) is preserved: a `siellähän` whose
/// host is a known form still wins, and stacked clitics peel one
/// layer at a time.
///
/// Returns a single-element `Vec` on hit, empty on miss. The
/// return type mirrors the multi-base API so callers that don't
/// care about the count pay no cost; callers that need the full
/// list use [`find_bases_with`].
/// Whether `lemma` and `word` share their first two characters.
///
/// A form is the lemma's stem plus an ending: the stem is a prefix of the lemma
/// and gradation changes only its last consonant, so a regular form shares at
/// least the lemma's first two characters. The suppletive `olla` forms (`on`,
/// `ovat`) are the exception, and they reach the checker through the verb stem
/// map and the short-lemma scan rather than this walk. The suggestion walks
/// keep their first-character match, because there a near miss is the point.
fn shares_first_two(lemma: &str, word: &str) -> bool {
    let mut l = lemma.chars().map(|c| c.to_ascii_lowercase());
    let mut w = word.chars().map(|c| c.to_ascii_lowercase());
    match (l.next(), w.next()) {
        (Some(a), Some(b)) if a == b => {}
        _ => return false,
    }
    match (l.next(), w.next()) {
        (Some(a), Some(b)) => a == b,
        // A one-character lemma has only one character to share.
        _ => true,
    }
}

/// The lexicon row for `word`, if the lexicon contains it.
///
/// The callers that rank a small set of names against [`ClassPrior`] need the
/// `Lemma` (its classes and part of speech), not just the embedded row.
pub fn lemma_by_word<'a>(lexicon: &'a [Lemma<'a>], word: &str) -> Option<&'a Lemma<'a>> {
    lemma_index(lexicon, word).map(|idx| &lexicon[idx])
}

/// Ranks base lemmas the way the reverse index does, without building it.
///
/// The reverse index ranks with the generation confidence first, then the
/// noun-over-derived-adjective rule, then the class prior. The bounded walks
/// here do not keep the per-form confidence, so they use the remaining two
/// keys: confidence is a property of how the form was attached and every walk
/// has already verified the surface; the class prior and the derivation rule
/// are what choose between genuine homonyms.
///
/// This is deliberately a stable sort: the walk's own order is the final tie
/// breaker, so a caller that ranks the same set twice gets the same answer and
/// the sanalista's order still shows through where the evidence is equal.
pub fn rank_lemma_bases<'a>(bases: Vec<&'a Lemma<'a>>, prior: &ClassPrior) -> Vec<&'a Lemma<'a>> {
    let mut scored: Vec<(bool, usize, &'a Lemma<'a>)> = bases
        .iter()
        .map(|lemma| {
            (
                lemma_is_derived_adjective(lemma, &bases),
                prior.score(&lemma.inflections),
                *lemma,
            )
        })
        .collect();
    scored.sort_by_key(|(derived, score, _)| (*derived, std::cmp::Reverse(*score)));
    scored.into_iter().map(|(_, _, lemma)| lemma).collect()
}

/// The [`FormBuilder::is_derived_adjective`](crate::expand::FormBuilder) rule
/// for a bare lemma: an adjective whose `-inen`/`-nen` source is another base
/// of the same word and is a noun.
fn lemma_is_derived_adjective(lemma: &Lemma, bases: &[&Lemma]) -> bool {
    if !PartOfSpeech::parse_all(lemma.part_of_speech).contains(&PartOfSpeech::Adjective) {
        return false;
    }
    let reverse = crate::derivation::reverse(lemma.word);
    let mut sources: Vec<&str> = reverse
        .iter()
        .map(|reverse| reverse.source.as_str())
        .collect();
    if let Some(direct) = lemma.word.strip_suffix("inen") {
        sources.push(direct);
    }
    if let Some(direct) = lemma.word.strip_suffix("nen") {
        sources.push(direct);
    }
    sources.iter().any(|source| {
        bases.iter().any(|other| {
            other.word != lemma.word
                && *source == other.word
                && PartOfSpeech::parse_all(other.part_of_speech).contains(&PartOfSpeech::Noun)
        })
    })
}

/// The one base to name for a check, without the full reverse index.
///
/// Phase 1 (heads + tails) is cheap and is where almost every form lands, so
/// its candidates are collected in full and ranked with [`rank_lemma_bases`].
/// When it misses, the bounded prefix walks keep their existing first-match
/// behaviour: they are the expensive path and a single answer is all the
/// per-keystroke check needs.
pub fn find_best_base_with<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
    prior: &ClassPrior,
) -> Vec<&'a Lemma<'a>> {
    // The cheap exact answers, mirroring `find_any_base_with`.
    if let Some(idx) = lemma_index(lexicon, word) {
        return vec![&lexicon[idx]];
    }
    if let Some(base) = crate::expand::negation::negation_base(word)
        && let Some(idx) = lemma_index(lexicon, base)
    {
        return vec![&lexicon[idx]];
    }
    if let Some(base) = crate::expand::connegative::base(word)
        && let Some(idx) = lemma_index(lexicon, base)
    {
        return vec![&lexicon[idx]];
    }

    let index = LexiconIndex::new(lexicon);
    let mut candidates: Vec<&Lemma<'a>> = Vec::new();
    let mut seen: BTreeSet<*const Lemma<'a>> = BTreeSet::new();
    for lemma in index.heads_of(word).into_iter().chain(index.tails_of(word)) {
        if seen.insert(lemma as *const _) && lemma_emit(cat, lemma, word, PartOfSpeech::Noun, true)
        {
            candidates.push(lemma);
        }
    }
    if !candidates.is_empty() {
        return rank_lemma_bases(candidates, prior);
    }

    // Phase 1.5 and the prefix walks: first match, as before. Re-running the
    // cheap phase-1 probes costs a handful of inflections, not the walk.
    find_any_base_with(lexicon, cat, word)
}

pub fn find_any_base_with<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
) -> Vec<&'a Lemma<'a>> {
    // Cheap pre-check: is the word a lemma in the sanalista? If
    // so, that IS the base. The sanalista is sorted by word; a
    // binary search via `partition_point` is `O(log n)`.
    if let Some(idx) = lemma_index(lexicon, word) {
        return vec![&lexicon[idx]];
    }
    // The negation auxiliary and the words it fuses with (`enkä`,
    // `ettet`, `ellette`) are not lemmas themselves; the closed table
    // names the lemma they belong to (`ei`, `että`, `ellei`).
    if let Some(base) = crate::expand::negation::negation_base(word)
        && let Some(idx) = lemma_index(lexicon, base)
    {
        return vec![&lexicon[idx]];
    }
    // The bare verb connegative (`syöne`, `ajatelko`) names its lemma
    // through the derived table.
    if let Some(base) = crate::expand::connegative::base(word)
        && let Some(idx) = lemma_index(lexicon, base)
    {
        return vec![&lexicon[idx]];
    }
    let index = LexiconIndex::new(lexicon);
    let pos = PartOfSpeech::Noun;
    let possessives = true;
    // Phase 1: heads + tails.
    for lemma in index.heads_of(word) {
        if lemma_emit(cat, lemma, word, pos, possessives) {
            return vec![lemma];
        }
    }
    for lemma in index.tails_of(word) {
        if lemma_emit(cat, lemma, word, pos, possessives) {
            return vec![lemma];
        }
    }
    // Phase 1.5: head-1 walk. Capped at HEAD1_CAP to bound the
    // worst case; matches the `find_bases_with_inner` cap.
    const HEAD1_CAP: usize = 3000;
    let f_len = word.chars().count();
    if let Some(c) = word.chars().next() {
        let head_start: usize = sanalista().word_order().partition_point(|&row| {
            let fc = lexicon[row as usize].word.chars().next();
            fc.is_none_or(|c0| c0 < c)
        });
        let mut walked = 0usize;
        for &row in &sanalista().word_order()[head_start..] {
            let l = &lexicon[row as usize];
            if l.word.is_empty() || !l.word.starts_with(c) {
                break;
            }
            // A form is at most two characters shorter than its lemma (the
            // `-da/-dä` infinitive against the stem), and never longer than
            // the lemma by less than an ending. A lemma far longer than the
            // query cannot make it; verifying its paradigm was the walk's cost.
            if l.word.chars().count() > f_len + 2 {
                continue;
            }
            // A regular form shares the lemma's first two characters; a lemma
            // that shares only the first cannot produce this word, and
            // verifying its paradigm is what the walk spent its time on.
            if !shares_first_two(l.word, word) {
                continue;
            }
            if lemma_emit(cat, l, word, pos, possessives) {
                return vec![l];
            }
            walked += 1;
            if walked >= HEAD1_CAP {
                break;
            }
        }
    }
    // Phase 2/3/4: prefix walks. Same length filter as
    // `find_bases_with_inner`: the lemma must be no longer than
    // the form (otherwise it can't inflect to a shorter string).
    // The candidate walk runs in parallel across the available
    // cores via `par_first_some`; the first matching base wins
    // and the remaining workers bail on the atomic flag.
    for n in [3usize, 2, 4] {
        if let Some(prefix) = take_chars(word, n) {
            let candidates: Vec<&Lemma<'_>> = prefix_candidates(lexicon, &prefix)
                .into_iter()
                .filter(|l| l.word.chars().count() <= f_len)
                .collect();
            if let Some(lemma) =
                par_first_some(&candidates, |l| lemma_emit(cat, l, word, pos, possessives))
            {
                return vec![lemma];
            }
        }
    }
    // Phase 4: short-lemma scan. Lemmas of length <= 3 are tried
    // individually; the inflect-all cost is small for short
    // lemmas.
    for lemma in lexicon.iter() {
        if lemma.word.chars().count() <= 3
            && shares_first_two(lemma.word, word)
            && lemma_emit(cat, lemma, word, pos, possessives)
        {
            return vec![lemma];
        }
    }
    // Phase 5: clitic strip + recurse (with the clitic phase
    // disabled, so stacked clitics terminate). Mirrors
    // `find_bases_with_inner's` phase 5.
    if let Some((host, _clitic)) = strip_clitic(word) {
        let inner = find_any_base_with_no_clitic(lexicon, cat, &host);
        if !inner.is_empty() {
            return inner;
        }
    }
    // Phase 6 would be the colloquial `-s` (`ymmärrätkös`), but that is
    // spoken-language: it is *not* accepted as a base. The checker flags
    // the word and the suggestion pass offers the s-less host instead
    // (see `suggest::suggest_over_lemmas`). A word that genuinely ends in
    // `-s` (`kahdeksas`, `ulos`) is a lemma and is found in the phases
    // above.
    Vec::new()
}

/// `find_any_base_with` minus the clitic phase, used by the clitic
/// phase itself so a stacked-clitic word like `siellähänkin` peels
/// one layer at a time and terminates.
fn find_any_base_with_no_clitic<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
) -> Vec<&'a Lemma<'a>> {
    if let Some(idx) = lemma_index(lexicon, word) {
        return vec![&lexicon[idx]];
    }
    let index = LexiconIndex::new(lexicon);
    let pos = PartOfSpeech::Noun;
    let possessives = true;
    for lemma in index.heads_of(word) {
        if lemma_emit(cat, lemma, word, pos, possessives) {
            return vec![lemma];
        }
    }
    for lemma in index.tails_of(word) {
        if lemma_emit(cat, lemma, word, pos, possessives) {
            return vec![lemma];
        }
    }
    if let Some(c) = word.chars().next() {
        let head_start: usize = sanalista().word_order().partition_point(|&row| {
            let fc = lexicon[row as usize].word.chars().next();
            fc.is_none_or(|c0| c0 < c)
        });
        let mut walked = 0usize;
        for &row in &sanalista().word_order()[head_start..] {
            let l = &lexicon[row as usize];
            if l.word.is_empty() || !l.word.starts_with(c) {
                break;
            }
            if lemma_emit(cat, l, word, pos, possessives) {
                return vec![l];
            }
            walked += 1;
            if walked >= 3000 {
                break;
            }
        }
    }
    let f_len = word.chars().count();
    for n in [3usize, 2, 4] {
        if let Some(prefix) = take_chars(word, n) {
            for lemma in prefix_candidates(lexicon, &prefix) {
                if lemma.word.chars().count() > f_len {
                    continue;
                }
                if lemma_emit(cat, lemma, word, pos, possessives) {
                    return vec![lemma];
                }
            }
        }
    }
    for lemma in lexicon.iter() {
        if lemma.word.chars().count() <= 3 && lemma_emit(cat, lemma, word, pos, possessives) {
            return vec![lemma];
        }
    }
    Vec::new()
}

/// Binary-search the sanalista for the lemma whose `word` is exactly
/// `target`. Returns the index into `lexicon`, or `None`.
///
/// The `lexicon` slice is in *file* order, which is not fully
/// sorted (it has ~1.5k inversions). Binary-searching it directly
/// silently misses lemmas, so the search runs over
/// [`sanalista().word_order()`] -- the blob's sorted-by-word row
/// order -- and maps the hit back to a `lexicon` row. This relies
/// on the invariant the rest of this module already assumes:
/// `lexicon[row]` describes the same lemma as blob row `row`.
fn lemma_index(lexicon: &[Lemma<'_>], target: &str) -> Option<usize> {
    let order = sanalista().word_order();
    let start = order.partition_point(|&row| lexicon[row as usize].word < target);
    order.get(start).and_then(|&row| {
        let row = row as usize;
        if lexicon[row].word == target {
            Some(row)
        } else {
            None
        }
    })
}

/// Helper: does `lemma` inflect to `word` under the given pos?
/// Mirrors the per-candidate inner loop in
/// `find_bases_with_inner`.
fn lemma_emit(
    cat: &Catalogues,
    lemma: &Lemma<'_>,
    word: &str,
    pos: PartOfSpeech,
    possessives: bool,
) -> bool {
    super::emits_form(
        lemma.word,
        &lemma.inflections,
        cat,
        None,
        pos,
        possessives,
        word,
    )
}

/// All lemmas whose word starts with `prefix`, in word order.
///
/// Like [`lemma_index`], this walks [`sanalista().word_order()`]
/// rather than binary-searching the file-order `lexicon` slice.
/// The slice is not sorted by word (~1.5k inversions), so a
/// direct `partition_point` would land in the wrong place and
/// silently drop matches.
fn prefix_candidates<'a>(lexicon: &'a [Lemma<'a>], prefix: &str) -> Vec<&'a Lemma<'a>> {
    let order = sanalista().word_order();
    let start = order.partition_point(|&row| lexicon[row as usize].word < prefix);
    let mut out = Vec::new();
    for &row in order.iter().skip(start) {
        let l = &lexicon[row as usize];
        if !l.word.starts_with(prefix) {
            break;
        }
        out.push(l);
    }
    out
}

/// Engine-level search without the clitic phase. Used by the clitic phase
/// itself (so a stacked-clitic word like `siellähänkin` peels one layer at
/// a time and terminates) and exposed for callers that want the raw
/// prefix-walk result without the clitic fallback.
#[allow(clippy::too_many_arguments)] // the flags are the phase switches
fn find_bases_with_inner<'a>(
    lexicon: &'a [Lemma<'a>],
    cat: &Catalogues,
    word: &str,
    with_clitic_phase: bool,
    keep_form: Option<&(dyn Fn(&str) -> bool + Sync)>,
    collect_neighbours: bool,
    recognition_only: bool,
    cheap_recognition: bool,
    all_phases: bool,
    keep_walking_with_direct: bool,
) -> BasesReport<'a> {
    let index = LexiconIndex::new(lexicon);
    let pos = PartOfSpeech::Noun;
    let possessives = true;

    // The bounded walks (head-1, prefix-3, prefix-2, short-lemma
    // scan) are all case-sensitive: they look up lemmas whose
    // first two or three characters match the input's first
    // two or three. Every lemma in the sanalista is
    // already lowercase, so the user typing `Baromeetteri`
    // (sentence-initial) lands the prefix walks on `Bar` /
    // `Ba`, never reaching the lowercase `barometri`. The
    // head/tail walks use suffix strings and don't have this
    // problem, but they too would benefit from a consistent
    // lowercase input. Lowercase once at the top so the
    // bounded walks can find the modern-spelling lemma.
    let word_lc = word.to_ascii_lowercase();
    let direct: Vec<EmbeddedLemma> = crate::kotus::sanalista().by_word(&word_lc).collect();

    let mut seen: BTreeSet<*const Lemma<'a>> = BTreeSet::new();
    let mut bases: Vec<&'a Lemma<'a>> = Vec::new();
    let mut neighbours: Vec<&'a Lemma<'a>> = Vec::new();

    // Recognition returns as soon as the direct lemma answers: the walk
    // phases below exist to collect the suggestion pool's neighbours and
    // forms, and `find_bases_for_check` does not need either.
    if recognition_only && !direct.is_empty() && !keep_walking_with_direct {
        return BasesReport {
            direct,
            bases,
            neighbours,
            forms: Vec::new(),
        };
    }

    // The negation auxiliary and the words it fuses with (`enkä`,
    // `ettet`, `ellette`): a closed table, not a paradigm. The word is
    // accepted outright and the base lemma is the table's. Inserting it
    // first makes the walk phases below short-circuit, which is right --
    // none of these forms has another base.
    if let Some(base) = crate::expand::negation::negation_base(&word_lc)
        && let Some(idx) = lemma_index(lexicon, base)
    {
        bases.push(&lexicon[idx]);
    }
    // The bare verb connegative (`syöne`, `ajatelko`) is a word in its own
    // right in running text; the derived table names its lemma.
    if let Some(base) = crate::expand::connegative::base(&word_lc)
        && let Some(idx) = lemma_index(lexicon, base)
    {
        bases.push(&lexicon[idx]);
    }

    if recognition_only && !bases.is_empty() && !keep_walking_with_direct {
        return BasesReport {
            direct,
            bases,
            neighbours,
            forms: Vec::new(),
        };
    }

    // Phase 1: heads + tails.
    let mut candidates: Vec<&'a Lemma<'a>> = Vec::new();
    for lemma in index.heads_of(word).into_iter().chain(index.tails_of(word)) {
        let key = lemma as *const _;
        if seen.insert(key) {
            candidates.push(lemma);
        }
    }
    let mut forms: Vec<SourcedForm<'a>> = Vec::new();
    for lemma in &candidates {
        if recognition_only && cheap_recognition {
            if lemma_emit(cat, lemma, word, pos, possessives) {
                bases.push(lemma);
            }
            continue;
        }
        let generated = inflect_all(
            lemma.word,
            &lemma.inflections,
            cat,
            Some(&index),
            pos,
            possessives,
        );
        // Capture the bases-vs-neighbours decision, then drain `generated`
        // into `forms` paired with the lemma that produced it. After this
        // block `generated` is moved into `forms`, so the `any` decision
        // must come first.
        let owns_input = generated.iter().any(|f| f.word == word);
        for f in generated {
            if keep_form.is_none_or(|keep| keep(&f.word)) {
                forms.push(SourcedForm {
                    lemma: lemma.word,
                    form: f,
                });
            }
        }
        if owns_input {
            bases.push(lemma);
        } else if collect_neighbours {
            neighbours.push(lemma);
        }
    }

    // Recognition can stop here: phase 1 found the base. Phases 1.5-4 exist
    // to find bases the head/tail ranges missed and to fill the ranker's
    // neighbour list, and the checker needs neither. The analysis search
    // keeps going through phase 1.5 so a homonym behind a direct lemma
    // (`lapsi` behind `lasta`) is not lost.
    if recognition_only && !bases.is_empty() && !keep_walking_with_direct {
        return BasesReport {
            direct,
            bases,
            neighbours,
            forms,
        };
    }

    // Phase 1.5: head-1 walk -- lemmas starting with the same first
    // character. Cheap, catches one-character-substitution typos where
    // the head prefix differs from the input's head, e.g. "jooksee"
    // -> lemma "juosta" (starts with "j", not "jook"). Capped at
    // `HEAD1_CAP` lemmas so the worst case stays bounded.
    //
    // WORD_ORDER is sorted by word and contains leading non-alphabetic
    // lemmas (entries like "-suhta"); we binary-search for the head
    // rather than scanning the whole list, then walk forward while the
    // first character matches. The first char comes from the
    // lowercase input (`word_lc`) because every lemma is already
    // lowercase -- a sentence-initial `Baromeetteri` would
    // otherwise land on `B` and miss every `b`-lemma.
    const HEAD1_CAP: usize = 3000;
    if let Some(c) = word_lc.chars().next() {
        let head_start: usize = sanalista().word_order().partition_point(|&row| {
            let fc = lexicon[row as usize].word.chars().next();
            fc.is_none_or(|c0| c0 < c)
        });
        let mut head1: Vec<&'a Lemma<'a>> = Vec::new();
        for &row in &sanalista().word_order()[head_start..] {
            let l = &lexicon[row as usize];
            if l.word.is_empty() || !l.word.starts_with(c) {
                break;
            }
            // In recognition the question is "is this a form of that lemma",
            // not "which lemma is it near": a lemma that shares only the first
            // character cannot make the word (the `olla` suppletives are
            // handled by the stem map and the short-lemma scan).
            if recognition_only
                && (!shares_first_two(l.word, &word_lc)
                    || l.word.chars().count() > word_lc.chars().count() + 2)
            {
                continue;
            }
            if seen.insert(l as *const _) {
                head1.push(l);
                if head1.len() >= HEAD1_CAP {
                    break;
                }
            }
        }
        let walked = walk_head1(
            &head1,
            &index,
            cat,
            &word_lc,
            keep_form,
            collect_neighbours,
            recognition_only,
            cheap_recognition,
            pos,
            possessives,
        );
        bases.extend(walked.bases);
        neighbours.extend(walked.neighbours);
        forms.extend(walked.forms);
    }

    if recognition_only && !bases.is_empty() {
        return BasesReport {
            direct,
            bases,
            neighbours,
            forms,
        };
    }

    // Phase 2: prefix-3 walk.
    if all_phases || bases.is_empty() {
        run_prefix_walk(
            lexicon,
            &word_lc,
            cat,
            &index,
            pos,
            possessives,
            &mut seen,
            &mut bases,
            &mut neighbours,
            &mut forms,
            keep_form,
            collect_neighbours,
            recognition_only,
            cheap_recognition,
            3,
        );
    }

    // Phase 3: prefix-2 walk.
    if all_phases || bases.is_empty() {
        run_prefix_walk(
            lexicon,
            &word_lc,
            cat,
            &index,
            pos,
            possessives,
            &mut seen,
            &mut bases,
            &mut neighbours,
            &mut forms,
            keep_form,
            collect_neighbours,
            recognition_only,
            cheap_recognition,
            2,
        );
    }

    // Phase 4: short-lemma scan.
    if all_phases || bases.is_empty() {
        for lemma in short_lemma_run(lexicon) {
            if !shares_first_two(lemma.word, &word_lc) {
                continue;
            }
            if seen.insert(lemma as *const _) {
                if recognition_only && cheap_recognition {
                    if lemma_emit(cat, lemma, word, pos, possessives) {
                        bases.push(lemma);
                    }
                    continue;
                }
                let generated = inflect_all(
                    lemma.word,
                    &lemma.inflections,
                    cat,
                    Some(&index),
                    pos,
                    possessives,
                );
                if generated.iter().any(|f| f.word == word) {
                    for f in generated {
                        if keep_form.is_none_or(|keep| keep(&f.word)) {
                            forms.push(SourcedForm {
                                lemma: lemma.word,
                                form: f,
                            });
                        }
                    }
                    bases.push(lemma);
                }
            }
        }
    }

    // Phase 5: clitic strip. When the four prefix walks all miss, the
    // word might be a clitic-bearing form: `siellähän` is `siellä` +
    // `-hän`; `sanopa` is `sano` (imperative 2sg of `sanoa`) + `-pa`.
    // Strip a single clitic and re-run the engine on the host. Recursion
    // goes through `find_bases_with_inner` with `with_clitic_phase =
    // false` so a stacked-clitic word like `siellähänkin` peels one layer
    // at a time (the inner call's phase 5 is skipped; the outer call's
    // phase 5 only sees the host after one strip).
    //
    // The colloquial `-s` is deliberately never stripped here (see
    // `find_any_base_with`).
    // We only run the clitic phase when no base was found by the
    // earlier phases; if phase 1 found a base, the word is recognised
    // already and a clitic-stripped host would just be the same lemma
    // counted twice.
    if with_clitic_phase
        && (all_phases || bases.is_empty())
        && let Some((host, clitic)) = strip_clitic(word)
    {
        // Skip recursing into a host that is itself a clitic; that
        // would mean the original word has two trailing clitics
        // (e.g. `siellähänkin`) and the recursion must peel the
        // inner one first.
        let inner = find_bases_with_inner(
            lexicon,
            cat,
            &host,
            /* with_clitic_phase */ false,
            keep_form,
            collect_neighbours,
            recognition_only,
            cheap_recognition,
            all_phases,
            keep_walking_with_direct,
        );
        if !inner.bases.is_empty() || !inner.direct.is_empty() {
            // The host is a known form. The original is too, with
            // the same bases (and the clitic stacked on top). We
            // don't try to produce a `Form` for the cliticised
            // shape here -- `cliticize_all` in
            // `crate::expand::clitic` does that when a caller
            // wants the rendered surface. The checker just needs
            // the recognition.
            for b in inner.bases {
                if !bases.iter().any(|existing| existing.word == b.word) {
                    bases.push(b);
                }
            }
            // Forms: the clitic-stripped host's forms, paired with
            // their lemmas, are also valid forms of the original
            // (just cliticised). We re-emit them so callers that
            // inspect `forms` see the
            // morphological data without re-inflecting.
            //
            // The merge is keyed through a set rather than a
            // `forms.iter().any(...)` scan: both sides can hold
            // ~10^5 forms (the head-1 walk emits every form of
            // 3000 lemmas), so the linear scan made this
            // quadratic -- `siellähän` alone ran for minutes.
            let mut present: std::collections::HashSet<(String, String)> = forms
                .iter()
                .map(|sf| (sf.lemma.to_string(), sf.form.word.clone()))
                .collect();
            for sf in inner.forms {
                if present.insert((sf.lemma.to_string(), sf.form.word.clone())) {
                    forms.push(sf);
                }
            }
            // Quiet the unused-warning; `clitic` is the layered
            // information that the metadata side would carry
            // once a caller asks for it. For now it just documents the
            // path the recognition took.
            let _ = clitic;
        }
    }

    // Phase 6 would be the colloquial `-s` (see `find_any_base_with`):
    // spoken-language, so it is not accepted as a base. The s-less host
    // is offered as a suggestion instead.

    BasesReport {
        direct,
        bases,
        neighbours,
        forms,
    }
}

/// The first `n` characters of `word` as a `String`, or `None` if `word` is
/// shorter than `n`. Same rules as `prefix_chars` in the lexicon index --
/// characters, not bytes -- and a fresh allocation rather than a slice,
/// because the `partition_point` below wants a `&str` whose borrow outlives
/// the loop.
fn take_chars(word: &str, n: usize) -> Option<String> {
    let mut out = String::with_capacity(n * 4);
    for (i, c) in word.chars().enumerate() {
        if i == n {
            break;
        }
        out.push(c);
    }
    if out.chars().count() == n {
        Some(out)
    } else {
        None
    }
}

/// The three output lists of one head-1 chunk, kept per chunk so a parallel
/// walk can concatenate them in lemma order and produce the same report as the
/// sequential path.
struct WalkChunk<'a> {
    bases: Vec<&'a Lemma<'a>>,
    neighbours: Vec<&'a Lemma<'a>>,
    forms: Vec<SourcedForm<'a>>,
}

/// Inflect every head-1 lemma, in parallel when the target has threads.
///
/// The head-1 walk is the dominant cost of `find_bases_with`: it inflects up
/// to `HEAD1_CAP` full paradigms. The work is independent per lemma, so the
/// chunks can run on separate cores; the chunks are joined in input order, so
/// bases, neighbours, and forms keep the exact order the sequential loop
/// produced. The wasm has one thread, and the small-input path stays
/// sequential to avoid spawn overhead.
#[allow(clippy::too_many_arguments)]
fn walk_head1<'a>(
    head1: &[&'a Lemma<'a>],
    index: &LexiconIndex<'a>,
    cat: &Catalogues,
    word: &str,
    keep_form: Option<&(dyn Fn(&str) -> bool + Sync)>,
    collect_neighbours: bool,
    recognition_only: bool,
    cheap_recognition: bool,
    pos: PartOfSpeech,
    possessives: bool,
) -> WalkChunk<'a> {
    const PARALLEL_MIN: usize = 64;
    let workers = worker_count();
    if workers <= 1 || head1.len() < PARALLEL_MIN {
        return walk_head1_chunk(
            head1,
            index,
            cat,
            word,
            keep_form,
            collect_neighbours,
            recognition_only,
            cheap_recognition,
            pos,
            possessives,
        );
    }
    let chunk_size = head1.len().div_ceil(workers);
    let parts: Vec<WalkChunk<'a>> = std::thread::scope(|scope| {
        let handles: Vec<_> = head1
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    walk_head1_chunk(
                        chunk,
                        index,
                        cat,
                        word,
                        keep_form,
                        collect_neighbours,
                        recognition_only,
                        cheap_recognition,
                        pos,
                        possessives,
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("head-1 worker panicked"))
            .collect()
    });
    let mut out = WalkChunk {
        bases: Vec::new(),
        neighbours: Vec::new(),
        forms: Vec::new(),
    };
    for part in parts {
        out.bases.extend(part.bases);
        out.neighbours.extend(part.neighbours);
        out.forms.extend(part.forms);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn walk_head1_chunk<'a>(
    head1: &[&'a Lemma<'a>],
    index: &LexiconIndex<'a>,
    cat: &Catalogues,
    word: &str,
    keep_form: Option<&(dyn Fn(&str) -> bool + Sync)>,
    collect_neighbours: bool,
    recognition_only: bool,
    cheap_recognition: bool,
    pos: PartOfSpeech,
    possessives: bool,
) -> WalkChunk<'a> {
    let mut out = WalkChunk {
        bases: Vec::new(),
        neighbours: Vec::new(),
        forms: Vec::new(),
    };
    for &lemma in head1 {
        if recognition_only && cheap_recognition {
            if lemma_emit(cat, lemma, word, pos, possessives) {
                out.bases.push(lemma);
            }
            continue;
        }
        let generated = inflect_all(
            lemma.word,
            &lemma.inflections,
            cat,
            Some(index),
            pos,
            possessives,
        );
        // The `owns_input` check is a sanity step: the form is generated
        // against the lemma's paradigm and we want to know whether this lemma
        // *is* the base of the input (edit distance 0). Both sides are
        // lowercased, so the comparison is case-stable.
        let owns_input = generated.iter().any(|f| f.word == word);
        for f in generated {
            if keep_form.is_none_or(|keep| keep(&f.word)) {
                out.forms.push(SourcedForm {
                    lemma: lemma.word,
                    form: f,
                });
            }
        }
        if owns_input {
            out.bases.push(lemma);
        } else if collect_neighbours {
            out.neighbours.push(lemma);
        }
    }
    out
}

/// Phase-2/3 walk: every lemma whose first `prefix_len` characters match the
/// form's first `prefix_len` characters and whose length does not exceed the
/// form's length. A lemma longer than the form cannot be the lemma whose
/// inflection produced it. Each candidate is run through `inflect_all` and any
/// lemma that emits the form is added to `bases`; everything else is added to
/// `neighbours` (unless already there).
///
/// This is the `heads_of` walk the lexicon index would do if `PREFIX` were
/// `prefix_len` instead of 4.
//
// The argument list mirrors the other phase walkers; grouping it into a
// struct would only move the same list one level out.
#[allow(clippy::too_many_arguments)]
fn run_prefix_walk<'a>(
    lexicon: &'a [Lemma<'a>],
    form: &str,
    cat: &Catalogues,
    index: &LexiconIndex<'a>,
    pos: PartOfSpeech,
    possessives: bool,
    seen: &mut BTreeSet<*const Lemma<'a>>,
    bases: &mut Vec<&'a Lemma<'a>>,
    neighbours: &mut Vec<&'a Lemma<'a>>,
    forms: &mut Vec<SourcedForm<'a>>,
    keep_form: Option<&(dyn Fn(&str) -> bool + Sync)>,
    collect_neighbours: bool,
    recognition_only: bool,
    cheap_recognition: bool,
    prefix_len: usize,
) {
    let Some(prefix) = take_chars(form, prefix_len) else {
        return;
    };
    let f_len = form.chars().count();
    let start = sanalista()
        .word_order()
        .partition_point(|&row| lexicon[row as usize].word < prefix.as_str());
    for &row in sanalista().word_order().iter().skip(start) {
        let l = &lexicon[row as usize];
        if !l.word.starts_with(prefix.as_str()) {
            break;
        }
        if l.word.chars().count() > f_len {
            continue;
        }
        if !seen.insert(l as *const _) {
            continue;
        }
        if recognition_only && cheap_recognition {
            if lemma_emit(cat, l, form, pos, possessives) {
                bases.push(l);
            }
            continue;
        }
        let generated = inflect_all(l.word, &l.inflections, cat, Some(index), pos, possessives);
        let owns_form = generated.iter().any(|f| f.word == form);
        for f in generated {
            if keep_form.is_none_or(|keep| keep(&f.word)) {
                forms.push(SourcedForm {
                    lemma: l.word,
                    form: f,
                });
            }
        }
        if owns_form {
            bases.push(l);
        } else if collect_neighbours && !neighbours.iter().any(|n| n.word == l.word) {
            neighbours.push(l);
        }
    }
}

/// Phase-4 candidates: every lemma with at most `SHORTEST_HEAD` characters.
/// This is the only pass that catches a form whose lemma shares no leading
/// characters with the form at all -- `öihin` <- `yö` is the case in point,
/// because the lemma's strong stem is `yö` and the form's leading `ö` is the
/// weak stem after gradation.
fn short_lemma_run<'a>(lexicon: &'a [Lemma<'a>]) -> Vec<&'a Lemma<'a>> {
    const SHORTEST_HEAD: usize = 3;
    lexicon
        .iter()
        .filter(|l| l.word.chars().count() <= SHORTEST_HEAD)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The analysis search must not stop at a direct lemma: `lasta` is a
    /// lemma of its own and the partitive of `lapsi`.
    #[test]
    fn analysis_search_keeps_homonyms_behind_a_direct_lemma() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        let report = find_bases_for_analysis(&lexicon, &cat, "lasta");
        let bases: Vec<_> = report.bases.iter().map(|lemma| lemma.word).collect();
        assert!(bases.contains(&"lasta"), "{bases:?}");
        assert!(bases.contains(&"lapsi"), "{bases:?}");
    }

    /// A small fix: every lemma the engine says inflects to the form really
    /// does, and the form is in the lemma's emitted set. Without this, a
    /// future refactor that confuses `bases` and `neighbours` would pass
    /// the smoke test silently.
    #[test]
    fn bases_actually_inflect_to_the_query() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for word in [
            "talo", "arvailen", "merelle", "katossa", "vedestä", "puhtaan", "öihin",
        ] {
            let r = find_bases_with(&lexicon, &cat, word);
            for lemma in &r.bases {
                let forms = inflect_all(
                    lemma.word,
                    &lemma.inflections,
                    &cat,
                    Some(&LexiconIndex::new(&lexicon)),
                    PartOfSpeech::Noun,
                    true,
                );
                assert!(
                    forms.iter().any(|f| f.word == word),
                    "{word}: claimed base {:?} does not inflect to it",
                    lemma.word
                );
            }
        }
    }

    /// Dropping generated forms (or filtering them) is a memory decision,
    /// not a recognition decision: the base/direct answer must not move.
    #[test]
    fn form_filtering_preserves_bases() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for word in ["talo", "taloa", "taloo", "jooksee", "puhtaan"] {
            let full = find_bases_with(&lexicon, &cat, word);
            let filtered = find_bases_with_form_filter(&lexicon, &cat, word, |surface| {
                surface.chars().count().is_multiple_of(2)
            });
            let check = find_bases_for_check(&lexicon, &cat, word);

            let full_bases: Vec<_> = full.bases.iter().map(|l| l.word).collect();
            let filtered_bases: Vec<_> = filtered.bases.iter().map(|l| l.word).collect();
            let check_bases: Vec<_> = check.bases.iter().map(|l| l.word).collect();
            assert_eq!(filtered_bases, full_bases, "{word}: bases moved");
            // A direct lemma answers recognition on its own; the walk's
            // base list is the suggestion pool's payload, not the check's.
            if check.direct.is_empty() {
                assert_eq!(check_bases, full_bases, "{word}: check bases moved");
            } else {
                assert_eq!(
                    check.direct.len(),
                    full.direct.len(),
                    "{word}: check direct moved"
                );
            }
            assert!(
                filtered
                    .forms
                    .iter()
                    .all(|sf| sf.form.word.chars().count().is_multiple_of(2)),
                "{word}: filter let an odd-length form through"
            );
            assert!(check.forms.is_empty(), "{word}: check kept forms");
            assert!(check.neighbours.is_empty(), "{word}: check kept neighbours");
        }
    }

    /// Clitic-bearing words are recognised by phase 5 (strip a single
    /// clitic and re-run the prefix walks on the host). The host can be
    /// a lemma directly (`siellähän` -> `siellä`) or an inflected verb
    /// form (`sanopa` is `sano` + `-pa`). The colloquial `-s` is not
    /// accepted (see `the_colloquial_s_is_not_a_base`).
    #[test]
    fn bases_recognise_clitic_bearing_words() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for (word, expected_lemma) in [
            // host is a lemma.
            ("siellähän", "siellä"),
            ("talohan", "talo"),
            ("tämäkään", "tämä"),
            ("aamukin", "aamu"),
            // host is an inflected form (imperative 2sg of katsoa,
            // imperative 2sg of sanoa, past participle of tietää).
            ("katsopa", "katsoa"),
            ("sanopa", "sanoa"),
            ("tiennytkään", "tietää"),
            // NOTE: the colloquial `-s` (kysymysliite) is deliberately
            // *not* accepted as a base -- `näetkös`, `mitäs`, `kukas`
            // and friends are flagged instead (see
            // `the_colloquial_s_is_not_a_base`), and the suggestion
            // pass offers the s-less host.
        ] {
            let r = find_bases_with(&lexicon, &cat, word);
            let lemmas: Vec<&str> = r.bases.iter().map(|l| l.word).collect();
            assert!(
                lemmas.contains(&expected_lemma),
                "{word}: expected base {expected_lemma:?}, got {lemmas:?}",
            );
        }
    }

    /// The `-s` kysymysliite attaches to a verb form or a question
    /// word; it is not a noun case. A noun with a trailing `-s` must
    /// not acquire a base just because the character is there --
    /// `kissas` is a misspelling, not `kissa` plus a particle.
    #[test]
    fn the_s_particle_does_not_attach_to_nouns() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for word in ["kissas", "talos", "aamus", "kaupunnis"] {
            let r = find_bases_with(&lexicon, &cat, word);
            assert!(
                r.bases.is_empty(),
                "{word}: expected no bases, got {:?}",
                r.bases.iter().map(|l| l.word).collect::<Vec<_>>(),
            );
        }
    }

    /// The colloquial `-s` (kysymysliite) is spoken-language, so it is
    /// *not* accepted as a base either -- not on a verb form, not on a
    /// question word. The checker flags the word and the suggestion
    /// pass offers the s-less host (`suggest::s_less_host`). A genuine
    /// `-s` lemma (`kahdeksas`) is found in the phases above this one.
    #[test]
    fn the_colloquial_s_is_not_a_base() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for word in [
            "ymmärrätkös",
            "näetkös",
            "tiedätkös",
            "mitäs",
            "mikäs",
            "kukas",
            "missäs",
        ] {
            let r = find_bases_with(&lexicon, &cat, word);
            assert!(
                r.bases.is_empty(),
                "{word}: expected no bases, got {:?}",
                r.bases.iter().map(|l| l.word).collect::<Vec<_>>(),
            );
        }
    }

    /// `find_any_base_with` short-circuits on the first matching
    /// base. The returned base must be one of the bases that
    /// `find_bases_with` would have returned (the first one in
    /// iteration order). For an unknown word, the result is
    /// empty. For a clitic-bearing word, the clitic phase still
    /// fires before the slow check gives up.
    ///
    /// This is the regression net for the inflect-check pipeline:
    /// the wasm's `check_fast` and `check_normal`'s gate 4 both
    /// rely on this short-circuit returning a real base, not
    /// silently `None` on a word that does have a base.
    #[test]
    fn find_any_base_returns_first_matching_base() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for word in [
            "talo",
            "arvailen",
            "veden",
            "veteen",
            "siellähän",
            "juoksemme",
        ] {
            let any = find_any_base_with(&lexicon, &cat, word);
            assert!(
                !any.is_empty(),
                "{word}: find_any_base_with returned empty, but the \
                 word has a base"
            );
            // The short-circuit base must be in the full set of
            // bases the multi-base API would have returned.
            let full = find_bases_with(&lexicon, &cat, word);
            let full_words: Vec<&str> = full.bases.iter().map(|l| l.word).collect();
            assert!(
                full_words.contains(&any[0].word),
                "{word}: short-circuit base {:?} not in the full set {:?}",
                any[0].word,
                full_words,
            );
        }
    }

    /// `find_any_base_with` returns an empty Vec for an unknown
    /// word. This pins the "miss" path so a future refactor that
    /// accidentally accepts garbage as a base fails the test.
    #[test]
    fn find_any_base_returns_empty_for_unknown() {
        let cat = Catalogues::embedded().clone();
        let lexicon = crate::kotus::parse_sanalista(crate::SANALISTA);
        for word in ["xyzzy", "qwerty", "asdfg", "zzz"] {
            let any = find_any_base_with(&lexicon, &cat, word);
            assert!(
                any.is_empty(),
                "{word}: find_any_base_with returned {:?}, expected empty",
                any.iter().map(|l| l.word).collect::<Vec<_>>(),
            );
        }
    }
}
