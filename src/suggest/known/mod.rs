//! The fast known-word layer: lemma set, compound detector and the
//! stem/paradigm probes behind `is_known_word`.
//!
//! Split out of `suggest/mod.rs`; the suffix families live in the sibling
//! modules and are re-exported here so callers keep their paths.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::kotus::Lemma;
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

mod compound;
mod derived;
mod lemmas;
mod stems;

pub(crate) use compound::{
    hyphenated_lexeme, is_known_compound, is_known_compound_dp, is_known_compound_fast,
    is_known_short_compound,
};
pub use lemmas::suggest_over_lemmas;
pub(crate) use lemmas::{gradation_plural_alternate, s_less_host};
pub(crate) use stems::is_content_head;

use compound::{detect_compound, hyphenated_base};
pub(crate) use derived::{derived_action_us_base, derived_comparative_base, derived_sti_base};

use derived::{
    bound_suffix_compound_base, derived_hko_base, derived_inen_degree_base,
    derived_superlative_base,
};
use stems::{
    find_base_via_stem_map_static, find_base_via_stem_map_static_possessive,
    possessive_base_via_stem_map, verb_stem_probe,
};

/// The embedded catalogue, decoded once and cached.
///
/// `Catalogues::embedded` already caches the decoded blob, so this
/// is a thin alias kept because the surrounding code was written
/// against it.
pub(super) fn catalogue() -> &'static Catalogues {
    Catalogues::embedded()
}

/// The embedded sanalista as a `Lemma` slice, materialised once from the
/// packed blob -- not by re-parsing the 2.86 MB source text (see
/// [`crate::kotus::lemmas`]).
pub(super) fn lexicon() -> &'static [Lemma<'static>] {
    crate::kotus::lemmas()
}

/// HashSet of every lemma (already lowercased in the sanalista) in the
/// leaked lexicon. Used as the fast path for the per-textbox API: a
/// spell-check on N tokens wants O(1) per token, not the prefix-walk
/// `find_bases_with` does for full morphology. The set is built once
/// (~1.6 MB on the embedded sanalista) and lives for the process's
/// lifetime.
static LEMMA_SET: OnceLock<HashSet<&'static str>> = OnceLock::new();

/// The negation auxiliary and its fused forms live in
/// [`crate::expand::negation`]; the fast path asks
/// [`crate::expand::negation::negation_base`] for the base. Kept out of
/// this module so the table has one home for the engine and the fast path
/// alike.
pub(super) fn lemma_set() -> &'static HashSet<&'static str> {
    LEMMA_SET.get_or_init(|| {
        // From the packed rows, not `lexicon()`: the set needs the words only,
        // and materialising 104k `Lemma`s allocates an `Inflection` vec per row
        // (~16 ms on the first miss). `EmbeddedLemma::word` borrows the blob.
        let lex = crate::kotus::sanalista();
        let mut s: HashSet<&'static str> = HashSet::with_capacity(lex.len());
        for lemma in lex.iter() {
            s.insert(lemma.word());
        }
        s
    })
}

/// True if `word` is itself a known lemma (case-insensitive). This
/// is the O(1) fast path for the per-textbox spell-check; the
/// expensive [`check`] is reached only for words that are neither
/// lemmas nor in the textarea's proper-noun set.
pub fn is_lemma(word: &str) -> bool {
    let lower = word.to_lowercase();
    lemma_set().contains(lower.as_str())
}

/// "Is this a known Finnish word?" -- find the lemma the input
/// is a form of, then verify by re-inflecting. Replaces the
/// previous hand-coded suffix-stripper + lemma-HashSet-probe
/// path. Returns `Some(base)` if the word is a real Finnish
/// word (lemma or any inflected form the engine produces),
/// `None` otherwise.
///
/// ## Pipeline
///
/// 1. **Direct lemma probe**: if `lower` is in the lemma
///    HashSet, return it as the base.
/// 2. **Noun stem hashmap**: for each known Finnish case
///    ending, strip from `lower` to get a stem, probe the
///    phf map for the stem, re-inflect each candidate base
///    with `PartOfSpeech::Noun` and check if `lower` is in
///    the result.
/// 3. **Verb stem hashmap**: same shape, `PartOfSpeech::Verb`.
///    Covers finite indicative, potential, conditional,
///    imperative (active and passive), infinitives, and active
///    participles.
///
/// The re-inflect step is the source of truth: a wrong stem
/// strip (e.g. `vedet` -> `vede`) still has to match the
/// original word in the candidate base's paradigm to win. The
/// AGENTS.md "no shortcuts in `is_known_word`" rule is honored
/// -- every match comes from the engine's actual inflect
/// output.
///
/// For the typing path: replaces the previous
/// `is_known_word` (bool) plus a separate `check` call, giving
/// the wasm a single O(1)-ish "is this real?" probe that also
/// names the base. For rare forms the stem map misses, fall
/// through to [`crate::expand::check_normal::check_normal`]
/// (which has gates 1-5 including the slow engine).
///
/// [`crate::expand::check_normal::check_normal`]:
///     ../expand/check_normal.rs
pub fn is_known_word(word: &str) -> Option<String> {
    is_known_word_with_compound(word, true)
}

/// [`is_known_word`] without compound detection: the lemma, the negation
/// forms, the possessive stripper and the stem maps, but no split.
///
/// The base this returns is the word's own, not a compound head, so a
/// caller that needs to *name* the base (`check_normal`'s fast path)
/// can use it. `taloilta` returns `None` here but the compound-aware
/// `is_known_word` returns the head `ilta`, which would be wrong to
/// report as the base.
pub fn is_known_word_no_compound(word: &str) -> Option<String> {
    is_known_word_with_compound(word, false)
}

/// The bounded recognition probe: the non-compound gates plus the
/// engine-free compound detector (which still recovers class-6 plural-only
/// heads such as `kekkereitä`). Used by the wasm slow-check pass, where an
/// unknown word must not pay the full engine walk.
pub fn is_known_word_fast(word: &str) -> Option<String> {
    is_known_word_no_compound(word)
        .or_else(|| is_known_compound_fast(word))
        .or_else(|| is_known_short_compound(word))
}
thread_local! {
    /// Memo for [`is_known_word_with_compound`], see there.
    static KW_MEMO: std::cell::RefCell<
        std::collections::HashMap<String, [Option<Option<String>>; 2]>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The recognition stack, memoised per thread.
///
/// The result is a pure function of the word and the static dictionary, and
/// the compound split search asks about overlapping substrings, so a page
/// reuses most probes (`kaksikymmenvuotiaalla` and
/// `kaksikymmenvuotiaana` share their modifier). Bounded at 20 000 words;
/// the memo is a speed-up, not state, so dropping it is safe.
fn is_known_word_with_compound(word: &str, allow_compound: bool) -> Option<String> {
    const CAP: usize = 20_000;
    let slot = usize::from(allow_compound);
    if let Some(out) = KW_MEMO.with_borrow(|memo| {
        memo.get(word)
            .and_then(|entry| entry[slot].as_ref())
            .cloned()
    }) {
        return out;
    }
    let out = is_known_word_with_compound_uncached(word, allow_compound);
    KW_MEMO.with_borrow_mut(|memo| {
        if memo.len() >= CAP && !memo.contains_key(word) {
            // A large paste has tens of thousands of distinct tokens; the
            // memo is a speed-up, not state, so dropping it is safe.
            memo.clear();
        }
        memo.entry(word.to_string()).or_default()[slot] = Some(out.clone());
    });
    out
}

fn is_known_word_with_compound_uncached(word: &str, allow_compound: bool) -> Option<String> {
    use crate::expand::check_normal::STEM_BUCKETS_NOUN;
    let lower = word.to_lowercase();
    if crate::expand::closed_spellings::is_nonword(&lower) {
        return None;
    }
    let lex = lemma_set();

    // 1. Direct lemma probe.
    if lex.contains(lower.as_str()) {
        return Some(lower);
    }

    // 1a. Tabulated adverbial form. `torkuksissa` is an adverb of `torkku`,
    // not a noun case, and compounds use it as a member (`puolitorkuksiin`).
    if let Some(base) = crate::expand::check_normal::adverb_base(&lower) {
        return Some(base.to_string());
    }
    if let Some(base) = crate::expand::check_normal::adverb_possessive_base(&lower) {
        return Some(base.to_string());
    }
    if let Some(base) = crate::expand::check_normal::feminine_noun_base(&lower) {
        return Some(base.to_string());
    }

    // 1'. Hyphenated compound spelling. This is a writing rule, not a
    //     paradigm: `linja-auto` and `EU-maat` are compounds whose parts the
    //     ordinary walk can resolve, while `kirja-hylly` is rejected because
    //     the closed spelling `kirjahylly` is already the word. Do this before
    //     the stem maps, which cannot see the hyphen.
    if word.contains('-') {
        return hyphenated_base(word);
    }

    // 1a. Productive derived verbs that Kotus does not list as lemmas
    //     (`nostatella`, `jouduttautua`). The class engines generate the
    //     forms; the supplement only names the derived lexeme.
    if let Some(base) = crate::expand::derived::base(&lower) {
        return Some(base.to_string());
    }

    if let Some(base) = crate::expand::closed_spellings::base(&lower) {
        return Some(base.to_string());
    }

    // 1'. Closed pronoun class (Kotus 101). The class catalogue cannot
    //    derive these forms, so `grammar::irregular` lists their paradigms;
    //    `pronoun_base` inflects that tiny class once and probes the map.
    //    Without this, `minkä` and `millä` reached only the slow engine.
    if let Some(base) = crate::expand::check_normal::pronoun_base(&lower) {
        return Some(base.to_string());
    }

    // 1''. Productive `-lainen`/`-läinen` place adjective. This sits before
    //     the possessive stripper because the comitative possessives
    //     (`laitilalaisineni`) otherwise split as unrelated compounds. The
    //     shape guard keeps the cost off ordinary words.
    if (lower.contains("lainen")
        || lower.contains("läinen")
        || lower.contains("lais")
        || lower.contains("läis"))
        && let Some(base) = crate::expand::derived_lainen_base(&lower, catalogue(), None)
    {
        return Some(base);
    }

    // 1a. The negation auxiliary and its fused forms
    //     (`en/et/ei/emme/ette/eivät`, `enkä`, `ettet`, `ellette`).
    //     The kotus verb oracle stores negation only as part of a
    //     multi-word form (`ei tule`), so the stripper doesn't see
    //     them, and the fused conjunctions are not a verb paradigm at
    //     all. The closed table lives in
    //     [`crate::expand::negation`], shared with the engine.
    //
    //     This sits at the top of the function so it fires before the
    //     noun strip loop: the noun map has stripped stems that could
    //     otherwise land on `en` (`-n` strips it to `e`, and the verb
    //     map has `en` -> `ei`).
    if let Some(base) = crate::expand::negation::negation_base(&lower) {
        return Some(base.to_string());
    }

    // 1a'. The bare verb connegatives (`syöne`, `ajatelko`), which the
    //      engine emits only inside the phrase (`en syöne`). A form ->
    //      lemma table derived from the oracle's own phrases
    //      (`tools/oracle/build_verb_lexicon.py`).
    if let Some(base) = crate::expand::connegative::base(&lower) {
        return Some(base.to_string());
    }

    // 1b. Possessive-suffix stripper. Finnish possessive
    //     suffixes (`-ni` 1sg, `-si` 2sg, `-nsa`/`-nsä` 3sg /
    //     3pl, `-mme` 1pl, `-nne` 2pl) attach to a case form
    //     (e.g. `tuttavuut+ta+nne`, `talo+ssa+ni`,
    //     `isä+ni`). The noun case stripper below doesn't see
    //     them; without this gate `tuttavuuttanne`, `talossani`,
    //     `isäni` all fall through to the slow path even
    //     though the host resolves immediately via the lemma
    //     set or the noun stem map.
    //
    //     Vowel harmony: the 3sg/3pl form is `-nsa` after a
    //     back-vowel syllable (`talo+nsa`, `vede+nsa`) and
    //     `-nsä` after a front-vowel syllable (`isä+nsä`,
    //     `tyttö+nsä`). Both forms live in the list. The
    //     other forms are invariant.
    //
    //     Recursion depth is bounded by `allow_compound=false`
    //     -- the host re-enters without compound detection,
    //     so we don't double-strip a possessive then re-enter
    //     compound detection on the same span.
    const POSSESSIVE_SUFFIXES: &[&str] = &[
        "nsa", "nsä", "nne", "mme", "si", "ni",
        // Contracted 3rd-person possessive. Only the comitative shape
        // (`monineen` = `monine` + `-en`, `vedenpitävineen`) is handled
        // here; a bare `-en` would turn `tehtäneen` into the connegative
        // `tehtäne` and `luen` into an unrelated noun.
        "en",
    ];
    for ending in POSSESSIVE_SUFFIXES {
        let Some(host) = lower.strip_suffix(ending) else {
            continue;
        };
        if host.is_empty() {
            continue;
        }
        if *ending == "en" && !host.ends_with("ine") {
            continue;
        }
        // Direct noun stem-map probe for the bare host.
        // Some noun class paradigms carry the genitive /
        // illative / partitive stem (without the case
        // ending) as a stem-map key (`tuttavuude → tut
        // tavuus`, `tuttavuute → tuttavuus`). Stripping the
        // possessive lands on a bare stem, and the noun
        // case stripper can't reduce it further (no
        // ENDINGS suffix fits). The recursive probe below
        // would lose the path -- probe the noun stem map
        // directly here.
        //
        // Verify a short host. The stem map key need not be a full stem for
        // every class: `kä` is a map key, but `käki` has no `kämme`, and
        // trusting the map turned `lyökämme` into `lyö` + `käki`. Long hosts
        // are full case forms (`tuttavuutta`), and some possessive cells are
        // still outside the engine's output, so those keep the old trust.
        if STEM_BUCKETS_NOUN.contains(host) {
            if host.chars().count() <= 4 {
                for base in STEM_BUCKETS_NOUN.bases(host) {
                    if let Some(b) = find_base_via_stem_map_static_possessive(&lower, base) {
                        return Some(b);
                    }
                }
            } else if let Some(first_base) = STEM_BUCKETS_NOUN.bases(host).next() {
                return Some(first_base.to_string());
            }
        }
        // A genitive plural contracts its `-en` before the possessive:
        // `talojen` + `-nsa` -> `talojensa`, `veljien` + `-nsä` ->
        // `veljiensä`. Stripping the possessive leaves `taloje`/
        // `veljie`, one `e` away from the engine stem (`taloj`/`velji`).
        // The direct probe above cannot see that; strip the `e` and
        // verify the full possessive form, so the acceptance is still
        // backed by the paradigm rather than by the shape alone.
        if let Some(stem) = host.strip_suffix('e')
            && STEM_BUCKETS_NOUN.contains(stem)
        {
            for base in STEM_BUCKETS_NOUN.bases(stem) {
                if let Some(b) = find_base_via_stem_map_static_possessive(&lower, base) {
                    return Some(b);
                }
            }
        }
        // Illative `-seen` + possessive contracts one `n`:
        // `ruumiiseen` + `nne` -> `ruumiiseenne`, so the host after
        // stripping the possessive is `ruumiisee`. Put the `n` back,
        // resolve the illative, and verify the possessive form.
        let illative = format!("{host}n");
        if let Some(base) = is_known_word_with_compound(&illative, false)
            && let Some(b) = find_base_via_stem_map_static_possessive(&lower, &base)
        {
            return Some(b);
        }
        // The illative surface can itself be a lemma (`kotiin`), while the
        // possessive belongs to the noun behind it (`koti` -> `kotiinsa`).
        // Try the noun stem-map bases of the illative; both `koti` and the
        // homographic `kota` make the same surface, so either verified base
        // is a correct answer.
        if let Some(b) = possessive_base_via_stem_map(&lower, &illative) {
            return Some(b);
        }
        // Otherwise the host has its own case ending
        // (e.g. `tuttavuutta` from `tuttavuuttanne` -- the
        // stem lives one step further in, at `tuttavuu`).
        // Recurse so the noun case stripper gets a turn.
        if let Some(b) = is_known_word_with_compound(host, false) {
            return Some(b);
        }
    }

    // 1c. Translative stem `-kse-` + possessive. A noun's translative is
    //     `-ksi` (`talo` -> `taloksi`), and its possessive forms are built
    //     on the `-kse-` stem: `talokseni`, `taloksesi`, `talokseen` (the
    //     3rd person contracts `-nsa`/`-nsä` to `-en`), `taloksemme`,
    //     `taloksenne`. The same `-kse-` + possessive builds the long 1st
    //     infinitive of a verb (`ymmärtääkseni`, `saattaakseen`) and the
    //     translative of an adjective (`omakseen`). The 1b stripper does
    //     not reach any of them: it strips the possessive off the whole
    //     word, but the `-kse-` sits *before* the possessive, so the host
    //     is a bare translative stem that is not a word. Strip the
    //     possessive, then the `-kse-`, and the base is the lemma.
    const KSE_POSSESSIVES: &[&str] = &["nsa", "nsä", "ni", "si", "mme", "nne", "en"];
    for ending in KSE_POSSESSIVES {
        let Some(host) = lower.strip_suffix(ending) else {
            continue;
        };
        let Some(base) = host.strip_suffix("kse") else {
            continue;
        };
        if base.is_empty() {
            continue;
        }
        if let Some(b) = is_known_word_with_compound(base, false) {
            return Some(b);
        }
        // The bare translative stem can be the *weak* grade of a graded
        // lemma (`palkakseni` -> `palka`, the weak stem of `palkka`), which
        // is not itself a lemma, so the lemma probe above misses it. The
        // noun stem map carries the weak stem as a key back to the lemma.
        if let Some(first_base) = STEM_BUCKETS_NOUN.bases(base).next() {
            return Some(first_base.to_string());
        }
    }

    // 2. Noun stem hashmap. The stripper is the same
    //    `ENDINGS` table the lib uses; the verify is
    //    `inflect_all(base, ..., Noun, false)`.
    //
    // `ENDINGS` mixes noun case endings with verb personal
    // endings (e.g. `-in` for verb 1sg past, `-n` / `-t` for
    // verb 1sg / 2sg present). The lemma probe is only safe
    // for the noun-shaped endings -- a verb personal strip
    // can land on an unrelated noun that happens to be a lemma
    // (e.g. `tiesin` -> `ties`, where `ties` is the adverb
    // lemma rather than the verb's past root). Skip the lemma
    // probe for those verb-endings so the verb strip loop below
    // gets a chance to fire with the right stem.
    const NOUN_LEMMA_PROBE_ENDINGS: &[&str] = &[
        // Productive singular cases.
        "ssa", "ssä", "sta", "stä", "seen", "lle", "lta", "ltä", "ksi", "na", "nä", "ta", "tä",
        // Productive plural cases.
        "ja", "jä",
        // Possessive / clitic-shaped endings that attach to a
        // case form, not a verb personal form.
        "han", "hän", "kin", "isi", // Compound plural shapes.
        "isiin", "kaan", "kään",
    ];
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        let Some(stem) = lower.strip_suffix(ending) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        // 2a. Lemma-set probe (noun-shaped strips only). For
        //     short nouns where the case-form's stem *is* the
        //     lemma (`puussa` -> `puu`, `talossa` -> `talo`,
        //     `puistossa` -> `puisto`), the noun stem map
        //     doesn't have an entry -- the lemma set does. The
        //     previous `KNOWN_SUFFIXES` stripper used the
        //     lemma set directly; the phf stem-map refactor
        //     would otherwise lose coverage for every lemma
        //     whose stem happens to equal its surface form.
        if NOUN_LEMMA_PROBE_ENDINGS.contains(ending) && lex.contains(stem) {
            // A stem ending in `t` may be a plural-only lemma (`housut`), in
            // which case the singular-shaped strip is not a real form
            // (`housutja`, `housutssa`). Verify those; the common singular
            // stems keep the cheap lemma probe.
            if !stem.ends_with('t') {
                return Some(stem.to_string());
            }
            if let Some(base) =
                find_base_via_stem_map_static(&lower, stem, stem, PartOfSpeech::Noun)
            {
                return Some(base);
            }
        }
        // 2b. Stem-map probe. Handles multi-base stems
        //     (gradated, class-50/51 compounds, etc.).
        if !STEM_BUCKETS_NOUN.contains(stem) {
            continue;
        }
        for base in STEM_BUCKETS_NOUN.bases(stem) {
            if let Some(b) = find_base_via_stem_map_static(&lower, stem, base, PartOfSpeech::Noun) {
                return Some(b);
            }
        }
    }

    // 3. Verb stem hashmap. Verify with `PartOfSpeech::Verb`.
    //
    // 3a. Direct stem probe. The verb oracle records
    //     third-person-singular present indicative with an empty
    //     personal ending (`gen_stem_map_verb.rs`, `personal_endings`
    //     for indicative/present/third/singular = `[""]`), which means
    //     the surface form
    //     is *also* the verb stem. The CSV consequently emits
    //     `nousee -> nousta`, `lukee -> lukea`, `istuu ->
    //     istua`, `juoksee -> juosta`, `menee -> mennae`, etc.,
    //     and the phf map is keyed on those exact strings. The
    //     strip loop below only fires when the input has a real
    //     personal suffix (--1pp, past, conditional, ...), so it
    //     never tries the bare form. Probe the bare word first
    //     so 3sg-present verbs don't fall through to the FORM_SET
    //     probe and get flagged as misspellings by the typing path.
    if let Some(b) = verb_stem_probe(&lower, lower.as_str()) {
        return Some(b);
    }

    // 3b. Strip loop. Covers forms with personal endings.
    for ending in crate::expand::check_normal::VERB_ENDINGS {
        if ending.is_empty() {
            continue;
        }
        let Some(stem) = lower.strip_suffix(ending) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        if let Some(b) = verb_stem_probe(&lower, stem) {
            return Some(b);
        }
    }

    // 3c. Participle case stripper. The bare present/past/agent
    //     participle (`muistava`, `tiennyt`, `lukema`) is in the
    //     verb stem map as a stem → base entry (see
    //     `gen_stem_map_verb`). The
    //     participle inflects like an adjective (`muistava`
    //     → gen `muistavan`, part `muistavaa`, iness
    //     `muistavassa`, allat `muistavalle`, ...), so we peel
    //     noun case endings off the input and verify against
    //     the verb stem map.
    //
    //     This sits *after* the noun stem-map strip loop
    //     (gate 2), so a bare participle that also happens to
    //     be a noun stem (e.g. `tietävä` -> `tietä`, which is a
    //     verb class-99 stem) routes via the noun path first.
    //     The noun-shapes-only `NOUN_LEMMA_PROBE_ENDINGS` list
    //     isn't relevant here -- this loop walks the full
    //     `ENDINGS` table, plus the possessive suffixes so a
    //     non-finite form's possessive (`lopettamaisillani`) can
    //     strip to the stem the map stores.
    for ending in crate::expand::check_normal::ENDINGS
        .iter()
        .copied()
        .chain(crate::expand::check_normal::POSSESSIVE_SUFFIXES.iter().copied())
    {
        if ending.is_empty() {
            continue;
        }
        let Some(stem) = lower.strip_suffix(ending) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        if let Some(b) = verb_stem_probe(&lower, stem) {
            return Some(b);
        }
    }

    // 3d. Clitic strip. The clitic suffixes (`-ko`, `-kin`,
    //     `-kaan`, `-han`, `-pa`) attach to an inflected form
    //     without gradation. The fast path above doesn't see
    //     them -- a word like `nousenko` (1sg present of
    //     `nousta` + clitic `-ko`) or `sanoinko` (1sg past of
    //     `sanoa` + `-ko`) falls through even though the host
    //     (`nousen`, `sanoin`) resolves via the verb stem map.
    //
    //     The clitic list lives in
    //     `crate::expand::clitic::CLITICS`; the surface
    //     variants (`-kö`/`-ko`, `-hän`/`-han`, ...) are tried
    //     by `strip_clitic` and the longest prefix match wins.
    //     Recursion is bounded: `strip_clitic` returns one
    //     layer, and the recursion disables compound
    //     detection so a stacked clitic (`siellähänkin`) peels
    //     one layer per recursion until the host resolves or
    //     the clitic strip exhausts itself. In practice the
    //     clitic stack is at most five layers.
    //
    //     The recursion uses `allow_compound=false` -- the
    //     host's compound detection is already covered by the
    //     current call's path 4 below, and re-running it
    //     inside a clitic-strip recursion would let a single
    //     word trigger two compound walks.
    {
        use crate::expand::clitic::strip_clitic;
        let mut host_opt = strip_clitic(&lower);
        let mut guard = 0;
        while let Some((host, _clitic)) = host_opt {
            if host.is_empty() {
                break;
            }
            guard += 1;
            if guard > 5 {
                break;
            }
            if let Some(b) = is_known_word_with_compound(&host, false) {
                return Some(b);
            }
            host_opt = strip_clitic(&host);
        }
    }

    // 3e. Derived `-inen` adjective. `-inen` is the productive NOUN ->
    //     ADJECTIVE rule (`tahko` -> `tahkoinen`), but the derived word is
    //     not a sanalista lemma, so it is in neither the stem maps nor the
    //     form set -- `tahkoinen`/`tahkoisella` resolve only here. The
    //     derivation layer reconstructs the nominative and inflects class
    //     38; see `expand::derived_inen_base`.
    if let Some(base) = crate::expand::derived_inen_base(&lower, catalogue(), None) {
        return Some(base);
    }

    // 1e. Productive `-kas`/`-käs` adjectives and their degrees
    //     (`äänne` -> `äännekäs` -> `äännekkäämmäksi`).
    if let Some(base) = crate::expand::derived_kas_base(&lower, catalogue(), None) {
        return Some(base);
    }
    // The degree forms of a classless `-inen` adjective (`salaperäinen` ->
    // `salaperäisempi` / `salaperäisemmäksi` / `salaperäisin`). The engine
    // derives them in `degree_forms`, but the fast path only has the
    // surface; reconstruct the positive `-inen` nominative and verify the
    // whole paradigm emits the surface.
    if let Some(base) = derived_inen_degree_base(&lower) {
        return Some(base);
    }
    // `-llinen` on a base that is not the `i`/`ni`/`de` shape:
    // `kiulu` -> `kiulullinen`, `kiulullisen`, `kiulullisia`.
    if let Some(base) = crate::expand::derived_llinen_base(&lower, catalogue(), None) {
        return Some(base);
    }
    // NOUN -> ADJECTIVE privative `-ton`/`-tön`: `koriste` ->
    // `koristeeton`, `koristeettominta`; `taide` -> `taiteeton`.
    if (lower.contains("tom")
        || lower.contains("töm")
        || lower.contains("tonta")
        || lower.contains("töntä")
        || lower.ends_with("ton")
        || lower.ends_with("tön"))
        && let Some(base) = crate::expand::derived_ton_base(&lower, catalogue(), None)
    {
        return Some(base);
    }
    // ADJECTIVE -> NOUN `-uus`/`-yys`: `irstainen` -> `irstaisuus`,
    // `irstaisuutta` (class 40). The derived noun is not a lemma.
    if (lower.contains("uu") || lower.contains("yy"))
        && let Some(base) = crate::expand::derived_uus_base(&lower, catalogue(), None)
    {
        return Some(base);
    }

    // 3e2. Derived `-ja`/`-jä` agent noun. `heiluttaja` is `heiluttaa`
    //      under the productive agent rule, and `heiluttajan` is its
    //      genitive; the derived noun is not a sanalista lemma, so the
    //      stem maps cannot see it. The same derivation layer reconstructs
    //      the nominative and inflects it as class 10/12.
    if let Some(base) = crate::expand::derived_agent_base(&lower, catalogue(), None) {
        return Some(base);
    }
    // Reverse degree index: `enemmittä` -> `enempi`, `loitommalla` ->
    // `loitto`, `hiljempaa` -> `hiljaa`.
    if let Some(base) = crate::expand::degree_base(&lower) {
        return Some(base.to_string());
    }

    // Productive feminine `-tar`/`-tär`: `siivooja` -> `siivoojatar`.
    if let Some(base) = crate::expand::derived_tar_base(&lower, catalogue(), None) {
        return Some(base);
    }

    // 3e2b. Productive `-sti` adverb. The host must itself be a known
    //       adjective or a verb form (the `-va` participle): `nopeasti`
    //       from `nopea`, `halveksivasti` from the participle of
    //       `halveksia`. Requiring the host's part of speech is what keeps
    //       a noun + `sti` (`talosti`) out.
    if let Some(base) = derived_sti_base(&lower) {
        return Some(base);
    }

    // 3e2c. The productive VERB -> NOUN `-us`/`-ys` action noun
    //       (`hoilata` -> `hoilaus`, `vastata` -> `vastaus`). The derived
    //       noun is not a sanalista lemma, so neither the form set nor the
    //       stem maps know it; reconstruct the nominative and verify the
    //       whole class-39 paradigm.
    if let Some(base) = derived_action_us_base(&lower) {
        return Some(base);
    }

    // 3e2d. The adjective `-hko`/`-hkö` (`kostea` -> `kosteahko`,
    //       `kosteahkoa`). The base must be an adjective in the lexicon;
    //       class 1 verifies the case form.
    if let Some(base) = derived_hko_base(&lower) {
        return Some(base);
    }

    // 3e3. A superlative of a participle. `huomattu` is a passive past
    //      participle of `huomata`, and `huomatuin`/`huomatuimpia` are its
    //      superlative; the degree is derived from the participle, not
    //      tabulated in `ADJ_DEGREES`, so the class-36 paradigm is built
    //      here after the reconstructed nominative passes the fast gates.
    if let Some(base) = derived_superlative_base(&lower) {
        return Some(base);
    }
    // 3e3b. A comparative of a participle: `selittävä` -> `selittävämpi`,
    //       `selittävämmin`. The positive is a verb form, not an adjective
    //       lemma, so `ADJ_DEGREES` does not carry it.
    if let Some(base) = derived_comparative_base(&lower) {
        return Some(base);
    }

    // 3f. Contracted third-person possessive on a nominal case form.
    //     `kulku` -> `kululta` -> `kulultaan`; `-nsa`/`-nsä` contracts to
    //     `-an`/`-än` after a consonant-final case form. This has to run
    //     *after* the verb map: `tiedetään` also ends in `-an`, but it is
    //     a passive verb and step 3 already named `tietää`; stripping the
    //     `-an` first (as the generic possessive loop would) landed on the
    //     unrelated noun `tiede`. A compound head depends on this too:
    //     `juonenkulultaan` = `juonen` + `kulultaan`.
    for ending in ["an", "än"] {
        let Some(host) = lower.strip_suffix(ending) else {
            continue;
        };
        if host.is_empty() {
            continue;
        }
        if let Some(b) = is_known_word_with_compound(host, false) {
            return Some(b);
        }
    }

    // 3g. Proper names (Wikidata lemmas + their Omorfi singular cases).
    //     This is a *fallback*: only reached when every common-word gate
    //     missed. A surname such as `Talo` puts `talossa` in the name set,
    //     and accepting it here first would name the surface instead of
    //     `talo`; ordering it last keeps the common reading. Names are
    //     accepted, never suggested.
    if crate::expand::proper_names::contains_capitalised(word) {
        return Some(
            crate::expand::proper_names::lemma(word)
                .unwrap_or(&lower)
                .to_string(),
        );
    }

    // 4. Compound detection. Finnish compounds have their head
    //    at the right end; the modifier is any inflected form of
    //    any lemma. Try every split position from longest-
    //    modifier to shortest -- Finnish prefers the longest-
    //    possible prefix as the modifier, so the longest split
    //    that resolves wins. Both halves must be known words;
    //    the head's lemma is returned.
    //
    //    The length floor (6 chars) keeps this off short words
    //    where the chance of a real compound is low and the
    //    lemma / stem-map probes above already cover the common
    //    cases. `detect_compound` lets the *left* half nest
    //    (`henkivartio` + `väki`), bounded by stack depth; the
    //    right half stays a single word because it is the head.
    if allow_compound
        && lower.chars().count() >= 6
        && let Some(head) = detect_compound(&lower)
    {
        // A closed spelling of a lexically hyphenated compound is a
        // misspelling: `linjaauto` must be `linja-auto`. The sanalista
        // carries the standard spelling as a lemma, so a single probe per
        // boundary is enough; `kirjahylly` has no such lemma and stays
        // closed.
        if hyphenated_lexeme(&lower).is_some() {
            return None;
        }
        return Some(head);
    }

    // A compound whose head is a bound suffix row (`‑mielisyys`, `‑peräinen`):
    // the sanalista writes those with a leading non-breaking hyphen because
    // they are not standalone lemmas. This runs only after the ordinary
    // compound walk, so a real head (`ruumis` in `henkiruumiiseenne`) wins
    // over a homographic suffix adjective (`‑ruumiinen`).
    if allow_compound && let Some(base) = bound_suffix_compound_base(&lower) {
        return Some(base);
    }

    // The compile-time form subset carries common inflections (every genitive
    // singular) that the oracle-derived stem map does not always key. The
    // compound walk asks this function about each half with
    // `allow_compound=false`, so without this probe a perfectly ordinary
    // modifier or head (`päivän`, `puolen`) came back unknown and took the
    // whole compound with it.
    if crate::expand::all_forms::is_known_inflected_form(&lower) {
        return Some(lower);
    }

    None
}

/// The standard hyphenated spelling of a closed word, when the sanalista
/// lists one.
///
/// Finnish writes ordinary compounds closed, but a handful are lexicalised
/// with a hyphen (`linja-auto`, `maa-alue`, `nuori-isäntä`). A closed
/// spelling of one of those is a misspelling even though the compound
/// detector would otherwise accept its parts. The search is bounded by the
/// word length and only runs for an unhyphenated word.
pub fn warm_caches() {
    let _ = lemma_set();
    let _ = catalogue();
    // The non-compound detector is the last lazy layer: it builds the
    // derivation tables and the stem-map verifiers. A miss warms all of them
    // without accepting a word, so the first unknown word on a page does not
    // pay the ~100 ms cold start.
    let _ = is_known_word_no_compound("xyzzy");
}
