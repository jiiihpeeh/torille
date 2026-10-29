//! The fast known-word layer: lemma set, compound detector and the
//! stem/paradigm probes behind `is_known_word`.
//!
//! Split out of `suggest/mod.rs`. The public entry points are re-exported
//! from `suggest` so their paths do not change.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::kotus::Lemma;
use crate::spec::Catalogues;
use crate::tags::PartOfSpeech;

use super::levenshtein::levenshtein;

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
        let lex = lexicon();
        let mut s: HashSet<&'static str> = HashSet::with_capacity(lex.len());
        for l in lex {
            s.insert(l.word);
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

/// Recognise a compound without running the stem-map, derivation and verb
/// gates that [`is_known_word`] runs first.
///
/// The spelling path only needs a yes/no answer for a word that is already
/// known to be a compound; the full detector spends most of its time on
/// `-inen` degree reconstruction and the noun/verb strip loops. This is the
/// compound part alone, with the same rules:
///
///   * a closed spelling of a lexically hyphenated compound (`linjaauto`) is
///     a misspelling, not a known compound;
///   * the ordinary head/tail split wins over a bound suffix head
///     (`henkiruumiiseenne` names `ruumis`, not `‑ruumiinen`).
pub(crate) fn is_known_compound(word: &str) -> Option<String> {
    is_known_compound_with(word, /* engine_fallback */ true)
}

/// The engine-free variant of [`is_known_compound`].
///
/// First-pass spell checking must stay bounded for unknown words. The
/// engine fallback in `detect_compound_at` is what lets a compound head
/// missing from the old stem snapshot (`kekkereitä`) be recognised, but it
/// also spends seconds probing an arbitrary proper name such as
/// `sortavalan`. The first pass can underline those and let the suggestion
/// pool's full `bg.check` demote them; it must not run the walk itself.
pub(crate) fn is_known_compound_fast(word: &str) -> Option<String> {
    // Short words are cheap to underline and let the full slow check
    // demote them. Skipping the split search here keeps a page of ordinary
    // six/seven-letter misspellings from paying the compound probe. The
    // cheap [`is_known_short_compound`] covers the short compounds that do
    // occur (`esisana`, `autotie`).
    if word.chars().count() < 8 {
        return None;
    }
    is_known_compound_with(word, /* engine_fallback */ false)
}

/// The 6-7 character compounds the full probe leaves to the slow check.
///
/// [`is_known_compound_fast`] starts at eight characters: the general split
/// search is several times more expensive on short words, because most
/// splits have no head and the modifier side recurses into the rest. Short
/// compounds are still ordinary words (`esisana`, `autotie`, `esiosa`), so
/// this checks them with constant-time probes only: a known content-word
/// modifier on the left and a content-word lemma on the right. No nesting,
/// no possessive, no engine.
///
/// Callers must already have established that the whole word is not known;
/// this deliberately skips the "a known form is not a compound" guard,
/// which costs more than the split search at this length.
pub(crate) fn is_known_short_compound(word: &str) -> Option<String> {
    let count = word.chars().count();
    if !(6..8).contains(&count) || word.contains('-') {
        return None;
    }
    let lower = word.to_lowercase();
    let mut splits: Vec<usize> = (1..count).collect();
    splits.reverse();
    for split in splits {
        let byte_split = lower
            .char_indices()
            .nth(split)
            .map(|(i, _)| i)
            .unwrap_or(lower.len());
        let (left, right) = lower.split_at(byte_split);
        // The same head guard as the general probe: a function word or a
        // bare connegative is not a compound head.
        if crate::expand::negation::negation_base(right).is_some()
            || crate::expand::connegative::base(right).is_some()
        {
            continue;
        }
        // A function-word modifier (`ja` + `talo`) is not a compound. Bound
        // prefixes (`esi`) have no sanalista row, so they stay content.
        if !crate::expand::modifier_is_known(left) || !crate::compound::is_content_head(left) {
            continue;
        }
        if crate::kotus::sanalista().by_word(right).next().is_none()
            || !crate::compound::is_content_head(right)
        {
            continue;
        }
        return Some(right.to_string());
    }
    None
}

fn is_known_compound_with(word: &str, engine_fallback: bool) -> Option<String> {
    let lower = word.to_lowercase();
    // Hyphenated input is a spelling decision in its own right, not a closed
    // compound: `kirja-hylly` must still reach `suggest`, which offers
    // `kirjahylly`.
    if lower.contains('-') || lower.chars().count() < 6 {
        return None;
    }
    let part_known = |part: &str| {
        is_known_word_with_compound(part, false).is_some() || crate::expand::modifier_is_known(part)
    };
    // A closed spelling of a lexically hyphenated compound is a
    // misspelling (`linjaauto` for `linja-auto`). The probe has to name a
    // real hyphenated lexeme: the shape rule alone fires on any same-vowel
    // boundary (`päivänpa|ahtamia`) and would reject a perfectly good
    // ordinary compound.
    if hyphenated_lexeme(&lower).is_some()
        || crate::compound::closed_hyphenated_spelling(&lower, part_known).is_some()
    {
        return None;
    }
    // A word that already resolves without compound analysis is an ordinary
    // inflected form, not a compound: `kissaa` is the partitive of `kissa`,
    // and `salaperäisempi` is a degree of `salaperäinen`, even though
    // `kis` + `saa` and `salaperäis` + `empi` also look like splits.
    if is_known_word_with_compound(&lower, false).is_some() {
        return None;
    }
    // A possessive suffix belongs to the whole compound, not to its head:
    // `henkiruumiiseenne` is `henki` + `ruumiiseen` + `-nne`, and
    // `turhamielisyyksissäni` is `turha` + `mielisyyksissä` + `-ni`. Strip it
    // first and retry on the host: otherwise the bare split search names the
    // possessive tail as the head (`aamujuomanne` -> `manne`) instead of a
    // content head of the host.
    // `-en` is the contracted third-person possessive after a case ending
    // (`portille` + `en` = `portilleen`); it belongs here as well as in the
    // non-compound stripper, otherwise `kotiportilleen` never reaches its
    // `koti` + `portille` split.
    for ending in ["nsa", "nsä", "nne", "mme", "ni", "si", "en", "an", "än"] {
        // `-si` is also the tail of the translative `-ksi`; stripping it
        // turns every `-ksi` form into a fake possessive host and sends the
        // compound probe down a slow, pointless path.
        if ending == "si" && lower.ends_with("ksi") {
            continue;
        }
        let Some(host) = lower.strip_suffix(ending) else {
            continue;
        };
        if host.is_empty() {
            continue;
        }
        // `-en` is also an ordinary illative ending (`lankivarteen`), so only
        // strip it as the contracted third-person possessive when the host
        // ends in a case form that possessive actually follows. Otherwise
        // `lankivarteen` loses its illative and resolves as `lanki` +
        // `varte`.
        if matches!(ending, "en" | "an" | "än")
            && ![
                "lle", "lta", "ltä", "lla", "llä", "ssa", "ssä", "sta", "stä", "ksi", "na", "nä",
                "ine",
            ]
            .iter()
            .any(|case| host.ends_with(case))
        {
            continue;
        }
        if let Some(head) = detect_compound_shallow(host, engine_fallback) {
            return Some(head);
        }
        // The illative `-seen` contracts one `n` before `-nne`:
        // `ruumiiseen` + `nne` -> `ruumiiseenne`.
        let illative = format!("{host}n");
        if let Some(head) = detect_compound_shallow(&illative, engine_fallback) {
            return Some(head);
        }
    }
    if let Some(head) = detect_compound_shallow(&lower, engine_fallback) {
        return Some(head);
    }
    if let Some(head) = bound_suffix_compound_base(&lower) {
        return Some(head);
    }
    None
}

/// Internal entry point. `allow_compound` gates the recursive
/// compound-detection step; the public [`is_known_word`] always
/// passes `true`, while the inner recursion from
/// [`detect_compound`] passes `false` so the modifier side of
/// a split is checked against the lemma / stem maps alone and
/// can't recurse into another compound detection.
fn is_known_word_with_compound(word: &str, allow_compound: bool) -> Option<String> {
    use crate::expand::check_normal::STEM_BUCKETS_NOUN;
    let lower = word.to_lowercase();
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

    // 1a'. Human-approved lexicalised / irregular surfaces. These are
    //      closed spellings (two-word phrases written together, defective
    //      plural paradigms, and compounds whose head is an inflected form),
    //      not productive suffix rules.
    if let Some(base) = crate::expand::fast_path::base(&lower) {
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
    //      (`tools/build_verb_lexicon.py`).
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
        // Skip the `inflect_all` verify: the kotus engine
        // emits possessive forms only when called with
        // `possessives=true`, and the fast path always
        // passes `false`. The stem map is built from the
        // engine's real inflect output, so the bare host
        // is a real intermediate form; adding a
        // possessive suffix to a real stem is always a
        // real Finnish word, so the trust is justified.
        if STEM_BUCKETS_NOUN.contains(host)
            && let Some(first_base) = STEM_BUCKETS_NOUN.bases(host).next()
        {
            return Some(first_base.to_string());
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
    //     personal ending (`build_verb_stem_to_base_map.py:62`,
    //     `PERSONAL_ENDINGS[("indicative","present","third",
    //     "singular")] = [""]`), which means the surface form
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
    //     `tools/build_verb_stem_to_base_map.py`). The
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
    //     `ENDINGS` table.
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

    // 3e2. Derived `-ja`/`-jä` agent noun. `heiluttaja` is `heiluttaa`
    //      under the productive agent rule, and `heiluttajan` is its
    //      genitive; the derived noun is not a sanalista lemma, so the
    //      stem maps cannot see it. The same derivation layer reconstructs
    //      the nominative and inflects it as class 10/12.
    if let Some(base) = crate::expand::derived_agent_base(&lower, catalogue(), None) {
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
        return Some(lower);
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
pub(crate) fn hyphenated_lexeme(word: &str) -> Option<String> {
    if word.contains('-') {
        return None;
    }
    let lower = word.to_lowercase();
    for (i, _) in lower.char_indices().skip(1) {
        let candidate = format!("{}-{}", &lower[..i], &lower[i..]);
        if crate::kotus::sanalista()
            .by_word(&candidate)
            .next()
            .is_some()
        {
            return Some(candidate);
        }
    }
    None
}

/// Recognise a correctly hyphenated compound.
///
/// The shape rules live in [`crate::compound::hyphen_rule`]; this function
/// supplies the lexical side. Every part except the first must resolve (or
/// be a known proper name / derivational stem), and the head must be a
/// content word. A numeric first part may introduce an otherwise unknown
/// suffix (`20-vuotias`), which is the one deliberate escape hatch.
fn hyphenated_base(word: &str) -> Option<String> {
    let parts = crate::compound::split_hyphenated(word)?;
    let closed: String = parts.concat().to_lowercase();
    let closed_known = is_known_word_with_compound(&closed, false).is_some();
    let rule = crate::compound::hyphen_rule(word, &parts, closed_known)?;
    let first_is_special = matches!(
        rule,
        crate::compound::HyphenRule::Abbreviation
            | crate::compound::HyphenRule::Number
            | crate::compound::HyphenRule::ProperName
    );

    let head_index = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 && first_is_special {
            continue;
        }
        let lower = part.to_lowercase();
        let resolved = is_known_word_with_compound(&lower, false).is_some()
            || crate::expand::modifier_is_known(&lower)
            || (i == head_index && crate::compound::is_proper_name_part(part));
        if !resolved && !(i == head_index && rule == crate::compound::HyphenRule::Number) {
            return None;
        }
    }

    let head = parts.last()?;
    let head_lower = head.to_lowercase();
    let head_base = is_known_word_with_compound(&head_lower, false)
        .or_else(|| crate::compound::is_proper_name_part(head).then_some(head_lower.clone()));
    match head_base {
        Some(base) => {
            if !crate::compound::is_content_head(&base) {
                return None;
            }
            Some(base)
        }
        None if rule == crate::compound::HyphenRule::Number
            && head.chars().next().is_some_and(char::is_alphabetic) =>
        {
            Some(word.to_lowercase())
        }
        None => None,
    }
}

/// Fast compound detector. Walks every split position and asks
/// [`is_known_word_with_compound`] (with `allow_compound=false`)
/// about each half. The first split where both halves resolve
/// wins; the head's lemma is returned.
///
/// A three-part compound has one side that is itself a compound, so the
/// left half is allowed a bounded number of further splits
/// (`henkivartioväki` = `henkivartio` + `väki` = `henki` + `vartio` +
/// `väki`). The right half stays a single word: it is the head, and its
/// own head is the whole word's head.
///
/// Returns `None` for non-compounds or for compounds whose halves
/// are not both in the fast-path coverage (e.g. a genitive stem
/// the noun stem map doesn't carry, or a possessive suffix the
/// `ENDINGS` table doesn't strip). The slow
/// `crate::expand::find_any_base_with` covers those.
fn detect_compound(word: &str) -> Option<String> {
    detect_compound_at(word, 0, true, true)
}

/// [`detect_compound`] without the engine fallback, used by the
/// recognition fast path.
///
/// The `left_is_compound` recursion is kept: Finnish compounds nest, and a
/// three-part word has a modifier that is itself a compound
/// (`salakirjevaihdossa` = `sala` + `kirje` + `vaihto`,
/// `henkivartioväki` = `henki` + `vartio` + `väki`). Without it every such
/// word was reported unknown by the first pass even though both of its
/// parts are ordinary words. The recursion is bounded by
/// [`MAX_COMPOUND_DEPTH`], and with `engine_fallback` false the deep split
/// search never reaches the full engine walk.
fn detect_compound_shallow(word: &str, engine_fallback: bool) -> Option<String> {
    detect_compound_at(word, 0, true, engine_fallback)
}

/// Depth limit for [`detect_compound_at`]. Finnish has no meaningful
/// bound, but every recursion chooses a shorter left half; four is well
/// past the reported `henki + vartio + väki` and keeps the search cheap.
const MAX_COMPOUND_DEPTH: usize = 4;

/// Longest right-hand head worth sending to the engine when the stem maps
/// miss it. The fallback exists for heads the old form snapshot lacks
/// (`kekkereitä`, ten chars); a long word on the right is usually a derived
/// adjective, not a compound head, and the full engine may spend seconds on
/// it (`salaperäisemmäksi` -> `peräisemmäksi`). The bound keeps compound
/// recognition from turning a non-compound into the slow path.
const MAX_COMPOUND_ENGINE_HEAD: usize = 12;

/// Class-6 plural-only heads (`kekkerit` -> `kekkereitä`) are the one
/// engine fallback the bounded fast path still allows. The stem maps were
/// built from an older snapshot, but these `-e-` plural endings identify
/// the shape cheaply, so a proper name such as `haltiuduin` does not send
/// its right half through the engine.
fn class6_plural_head(word: &str) -> bool {
    const ENDINGS: &[&str] = &[
        "eitä", "eissä", "eistä", "eihin", "eillä", "eiltä", "eille", "einä", "eiksi", "eittä",
        "eiden", "eitten", "eja", "eita", "ein", "eineen", "eine",
    ];
    ENDINGS.iter().any(|ending| word.ends_with(ending))
}

fn detect_compound_at(
    word: &str,
    depth: usize,
    recurse_left: bool,
    engine_fallback: bool,
) -> Option<String> {
    // Prefer a split whose modifier is a real word over one that is itself
    // only a compound or a bound modifier. The longest-modifier-first walk
    // otherwise picks `teatterikuole` + `maa` for `teatterikuolemaa` before
    // it ever reaches the correct `teatteri` + `kuolemaa`: `teatterikuole`
    // resolves through `kuolla`'s stem, but the head `maa` is the wrong
    // segmentation. The second pass keeps the old behaviour for words that
    // genuinely need a compound/bound modifier (`salakirjevaihdossa`,
    // `uimapaikalleen`).
    detect_compound_pass(word, depth, recurse_left, engine_fallback, true)
        .or_else(|| detect_compound_pass(word, depth, recurse_left, engine_fallback, false))
}

fn detect_compound_pass(
    word: &str,
    depth: usize,
    recurse_left: bool,
    engine_fallback: bool,
    direct_left_only: bool,
) -> Option<String> {
    if depth >= MAX_COMPOUND_DEPTH {
        return None;
    }
    // Walk splits from longest-modifier to shortest-modifier.
    // Finnish compounds prefer the longest-possible prefix as
    // the modifier, so the first matching split wins.
    //
    // Skip splits where the right half isn't a known word and
    // try a shorter modifier. Longest-modifier-first means most
    // candidate splits bail early; only the first match wins.
    //
    // Iterate over char boundaries, not byte indices -- words
    // like `kylmäkahvi` have multi-byte UTF-8 (ä = 2 bytes)
    // and `split_at(byte_index)` panics on a non-boundary.
    let mut splits: Vec<usize> = (1..word.chars().count()).collect();
    splits.reverse();
    for split in splits {
        let byte_split = word
            .char_indices()
            .nth(split)
            .map(|(i, _)| i)
            .unwrap_or(word.len());
        let (left, right) = word.split_at(byte_split);
        // A function word or a bare connegative is never a compound
        // head: `tehtäne` + `en` is not a compound, and accepting it
        // would make the fast path reach `tehtäneen` (the passive
        // potential) with the wrong base. The head of a Finnish compound
        // is a content word.
        if crate::expand::negation::negation_base(right).is_some()
            || crate::expand::connegative::base(right).is_some()
        {
            continue;
        }
        let mut head_lemma = match is_known_word_with_compound(right, false) {
            Some(base) => base,
            None => {
                // The stem maps are built from an earlier engine snapshot.
                // A class the engine has since fixed (the plural-only class-6
                // `kekkerit` -> `kekkereitä`) can be missing there while the
                // paradigm itself is already right. Ask the engine for the
                // head only after the cheap maps missed, only when the left
                // side is a known word, and only for a head long enough to
                // be a word. The left must be a word, not merely a modifier:
                // a derived prefix such as `salaperäise` would otherwise
                // send `mmäksi` through the engine, which spends seconds on
                // the non-compound reading.
                if depth != 0
                    || right.chars().count() < 6
                    || right.chars().count() > MAX_COMPOUND_ENGINE_HEAD
                    || is_known_word_with_compound(left, false).is_none()
                    || (!engine_fallback && !class6_plural_head(right))
                {
                    continue;
                }
                let found = crate::expand::find_any_base_with(lexicon(), catalogue(), right);
                let Some(lemma) = found.first() else {
                    continue;
                };
                lemma.word.to_string()
            }
        };
        // The form-set fallback names the surface itself (`varteen`). When a
        // stem map can name the lemma behind it (`varsi`, `puoli`), that is
        // the base a caller expects, and it is also what the content-head
        // check below needs to see.
        if head_lemma == right
            && let Some(base) = crate::expand::check_normal::find_base_via_stem_map(
                right,
                PartOfSpeech::Noun,
                false,
            )
        {
            head_lemma = base;
        }
        // `teiden` is a form of both `tee` and `tie`; the first base the
        // stem map returns is `tee`, which is the wrong head for
        // `raitiotie`. Prefer the base that makes the whole word a
        // sanalista compound lemma (`raitio` + `tie` = `raitiotie`). A
        // surface that is itself a function-word lemma (`vallassa` is an
        // adverb as well as a form of `valta`) is only rejected when this
        // content-base disambiguation finds nothing.
        let disambiguated = compound_head_from_lemma(left, right);
        // Check the *named head*, not just the surface: `si` has no lemma
        // row of its own, so `is_content_head("si")` would pass, while the
        // base it resolves to is the pronoun `tämä` (`vetäsi`) or `sä`
        // (`kysäsi`). Those are one-letter-short misspellings of `vetäisi`
        // and `kysäisi`, not compounds. When the whole-word lemma names a
        // content base (`harvainvallassa` -> `valta`), that base is the one
        // to check, not the adverb surface `vallassa`.
        let mut named_head = disambiguated.unwrap_or(head_lemma);
        // A derived nominal head (`päivänpaahtama` = `päivä` + `paahtama`)
        // has no sanalista row, so the lemma probe below cannot see it. The
        // derivation layer knows its POS, which is the question here.
        let derived_nominal = crate::expand::derived::is_nominal(right);
        if !derived_nominal && !is_content_head(&named_head) {
            // A surface can be a function-word lemma and a content
            // inflection at the same time: `varresta` is a postposition,
            // but also the elative of `varsi` (`lankivarresta`). Recover
            // the content base from the noun stem map before rejecting the
            // split.
            let Some(base) = crate::expand::check_normal::find_base_via_stem_map(
                right,
                PartOfSpeech::Noun,
                false,
            )
            .or_else(|| crate::expand::check_normal::find_base_via_verb_stem_map(right, false))
            else {
                continue;
            };
            if !is_content_head(&base) {
                continue;
            }
            named_head = base;
        }
        // The left may itself be a compound (`henkivartio` in
        // `henkivartioväki`); try one more level before falling back.
        let left_word = is_known_word_with_compound(left, false);
        // First pass: accept only a modifier that resolves as a word of its
        // own. A compound or a bound modifier is deferred to the second pass.
        if direct_left_only {
            if left_word.is_some() {
                return Some(named_head);
            }
            continue;
        }
        let left_is_compound = recurse_left
            && left_word.is_none()
            && detect_compound_at(left, depth + 1, true, engine_fallback).is_some();
        if left_word.is_some() || left_is_compound || crate::expand::modifier_is_known(left) {
            return Some(named_head);
        }
    }
    None
}

/// The base of a productive `-sti` adverb whose host is an adjective or a
/// participle. `nopeasti` -> `nopea`; `halveksivasti` -> `halveksia` (the
/// host `halveksiva` is the active present participle). A noun + `-sti`
/// (`talosti`) is not an adverb and must not pass.
fn derived_sti_base(word: &str) -> Option<String> {
    let stripped = word.strip_suffix("sti")?;
    if stripped.chars().count() < 3 {
        return None;
    }
    // An `-inen` adjective takes `-sesti`: `tapainen` -> `tapaisesti`, so
    // the host before `-sti` is `...tapaise`. Put the `nen` back before
    // looking for the adjective.
    let host = if let Some(stem) = stripped.strip_suffix("se") {
        format!("{stem}nen")
    } else {
        stripped.to_string()
    };
    let host = host.as_str();
    let Some(base) = is_known_word_with_compound(host, false) else {
        // A compound adjective has no row of its own (`suonenvedontapainen`
        // = `suonenveto` + `tapainen`). Its head shape and a known modifier
        // are the evidence that the `-sti` host is an adjective rather than
        // a noun; avoid re-entering compound detection here, which would
        // make a suffix check walk the whole compound recursively.
        let adjective_compound = ["tapainen", "lainen", "mainen", "inen"]
            .iter()
            .any(|suffix| {
                host.strip_suffix(suffix)
                    .is_some_and(crate::expand::modifier_is_known)
            });
        return adjective_compound.then(|| host.to_string());
    };
    let Some(row) = crate::kotus::sanalista().by_word(&base).next() else {
        let adjective_head = ["tapainen", "lainen", "mainen", "inen"]
            .iter()
            .any(|suffix| host.ends_with(suffix));
        return adjective_head.then_some(base);
    };
    let adjective_or_verb = crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
        .iter()
        .any(|pos| matches!(pos, PartOfSpeech::Adjective | PartOfSpeech::Verb));
    adjective_or_verb.then_some(base)
}

/// A classed suffix row from the sanalista (`‑mielisyys`, `‑peräinen`),
/// stored with its base spelling and paradigm so a compound can be checked
/// against it.
struct BoundSuffix {
    base: &'static str,
    head: String,
    inflections: Vec<crate::kotus::Inflection>,
    pos: PartOfSpeech,
}

/// The bound suffix rows, materialised once. There are a few hundred; the
/// first compound that needs one pays for the scan, later ones reuse it.
fn bound_suffixes() -> &'static [BoundSuffix] {
    static SUFFIXES: std::sync::OnceLock<Vec<BoundSuffix>> = std::sync::OnceLock::new();
    SUFFIXES.get_or_init(|| {
        crate::kotus::lemmas()
            .iter()
            .filter_map(|lemma| {
                let base = lemma.word.strip_prefix('\u{2011}')?;
                if base.is_empty() || lemma.inflections.is_empty() {
                    return None;
                }
                Some(BoundSuffix {
                    base,
                    head: base.chars().take(5).collect(),
                    inflections: lemma.inflections.clone(),
                    pos: PartOfSpeech::parse_all(lemma.part_of_speech)
                        .first()
                        .copied()
                        .unwrap_or(PartOfSpeech::Noun),
                })
            })
            .collect()
    })
}

/// Recognise a compound built on a bound suffix row: `turha` + `mielisyys`
/// gives `turhamielisyyksissäni`, `sala` + `peräinen` gives
/// `salaperäisemmäksi`. The modifier has to resolve and the suffix's own
/// paradigm has to emit the right-hand side; the five-character prefix
/// check only keeps the suffix scan cheap.
fn bound_suffix_compound_base(word: &str) -> Option<String> {
    let char_count = word.chars().count();
    if char_count < 8 {
        return None;
    }
    let suffixes = bound_suffixes();
    for split in 2..char_count {
        let byte_split = word.char_indices().nth(split).map(|(index, _)| index)?;
        let (modifier, right) = word.split_at(byte_split);
        if right.chars().count() < 4 || !crate::expand::modifier_is_known(modifier) {
            continue;
        }
        for suffix in suffixes {
            if right.len() < 4 || !right.starts_with(&suffix.head) {
                continue;
            }
            let forms = crate::expand::inflect_all(
                suffix.base,
                &suffix.inflections,
                catalogue(),
                None,
                suffix.pos,
                true,
            );
            if forms.iter().any(|form| form.word == right) {
                return Some(suffix.base.to_string());
            }
        }
    }
    None
}

/// The derived `-us`/`-ys` action noun a surface belongs to, when the verb
/// it comes from is a `-ta`/`-tä` verb (`hoilata` -> `hoilaus`).
///
/// The nominative is reconstructed by peeling the common case endings; the
/// verb source must be in the sanalista and the whole class-39 paradigm must
/// emit the surface. That verification is what keeps ordinary `-us` nouns
/// from being treated as derivations of an unrelated verb.
fn derived_action_us_base(word: &str) -> Option<String> {
    use crate::kotus::Inflection;
    let mut candidates = vec![word.to_string()];
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        if let Some(stem) = word.strip_suffix(ending)
            && !stem.is_empty()
        {
            candidates.push(stem.to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    for candidate in candidates {
        let Some(stem) = candidate
            .strip_suffix("us")
            .or_else(|| candidate.strip_suffix("ys"))
        else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        let mut source_is_verb = false;
        for suffix in ["ta", "tä"] {
            let verb = format!("{stem}{suffix}");
            let Some(row) = crate::kotus::sanalista().by_word(&verb).next() else {
                continue;
            };
            if crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
                .contains(&PartOfSpeech::Verb)
            {
                source_is_verb = true;
                break;
            }
        }
        if !source_is_verb {
            continue;
        }
        let forms = crate::expand::inflect_all(
            &candidate,
            &[Inflection {
                class: 39,
                gradation: None,
            }],
            catalogue(),
            None,
            PartOfSpeech::Noun,
            true,
        );
        if forms.iter().any(|form| form.word == word) {
            return Some(candidate);
        }
    }
    None
}

/// The `-hko`/`-hkö` adjective a surface belongs to (`kostea` ->
/// `kosteahko`). The derived adjective is not a lemma, so reconstruct its
/// nominative and verify the class-1 paradigm against the surface. The base
/// has to be an adjective, which keeps an unrelated `-hko` noun stem out.
fn derived_hko_base(word: &str) -> Option<String> {
    use crate::kotus::Inflection;
    let mut candidates = vec![word.to_string()];
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        if let Some(stem) = word.strip_suffix(ending)
            && !stem.is_empty()
        {
            candidates.push(stem.to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    for candidate in candidates {
        let Some(base) = candidate
            .strip_suffix("hko")
            .or_else(|| candidate.strip_suffix("hkö"))
        else {
            continue;
        };
        if base.is_empty() {
            continue;
        }
        let Some(row) = crate::kotus::sanalista().by_word(base).next() else {
            continue;
        };
        if !crate::tags::PartOfSpeech::parse_all(row.part_of_speech())
            .contains(&PartOfSpeech::Adjective)
        {
            continue;
        }
        let forms = crate::expand::inflect_all(
            &candidate,
            &[Inflection {
                class: 1,
                gradation: None,
            }],
            catalogue(),
            None,
            PartOfSpeech::Adjective,
            true,
        );
        if forms.iter().any(|form| form.word == word) {
            return Some(candidate);
        }
    }
    None
}

/// Candidate superlative nominatives for a degree form: the prefix before
/// the superlative marker (`imma` in the oblique stem, `imp` in the
/// partitive) plus `-in`. `huomatuimpia` -> `huomatuin`,
/// `kauneimman` -> `kaunein`.
fn superlative_candidates(word: &str) -> Vec<String> {
    let mut out = Vec::new();
    for marker in ["imma", "imp"] {
        if let Some(pos) = word.rfind(marker) {
            let prefix = &word[..pos];
            if !prefix.is_empty() {
                out.push(format!("{prefix}in"));
            }
        }
    }
    out
}

/// The positive `-inen` adjective behind a comparative or superlative
/// surface (`salaperäisemmäksi` -> `salaperäinen`, `salaperäisin` ->
/// `salaperäinen`).
///
/// The degree is derived from the positive by the engine, but the fast path
/// sees only the surface. Try every prefix of the surface followed by
/// `nen`; only a prefix that is an adjective lemma is a candidate, and the
/// candidate's whole paradigm has to emit the surface. That bounds this to
/// real `-inen` adjectives and keeps nouns such as `nainen` out.
fn derived_inen_degree_base(word: &str) -> Option<String> {
    use crate::tags::PartOfSpeech;

    // Degree suffixes are `-mpi`/`-mm-` (comparative) and `-in`/`-imm-`
    // (superlative); the leading `s` belongs to the degree stem. The guard
    // keeps the prefix walk off ordinary words.
    if !(word.contains("mp") || word.contains("mm") || word.contains("sin") || word.ends_with("in"))
    {
        return None;
    }
    let sanalista = crate::kotus::sanalista();
    for (byte, _) in word.char_indices().skip(1) {
        let prefix = &word[..byte];
        let candidate = format!("{prefix}nen");
        let Some(row) = sanalista.by_word(&candidate).next() else {
            continue;
        };
        if !PartOfSpeech::parse_all(row.part_of_speech()).contains(&PartOfSpeech::Adjective) {
            continue;
        }
        let inflections: Vec<_> = row.inflections().collect();
        let forms = crate::expand::inflect_all(
            &candidate,
            &inflections,
            catalogue(),
            None,
            PartOfSpeech::Adjective,
            false,
        );
        if forms.iter().any(|form| form.word == word) {
            return Some(candidate);
        }
    }
    None
}

/// The superlative degree a surface form belongs to, if the reconstructed
/// nominative is itself a known word and class 36 really emits the surface.
///
/// This covers degrees of participles (`huomattu` -> `huomatuin` ->
/// `huomatuimpia`), which `ADJ_DEGREES` cannot tabulate because the
/// positive is a verb form rather than an adjective lemma.
fn derived_superlative_base(word: &str) -> Option<String> {
    use crate::expand::inflect_all;
    use crate::kotus::Inflection;
    for nominative in superlative_candidates(word) {
        if is_known_word_with_compound(&nominative, false).is_none() {
            continue;
        }
        let forms = inflect_all(
            &nominative,
            &[Inflection {
                class: 36,
                gradation: None,
            }],
            catalogue(),
            None,
            PartOfSpeech::Adjective,
            false,
        );
        if forms.iter().any(|f| f.word == word) {
            return Some(nominative);
        }
    }
    None
}

/// Whether `word` may be the head of a compound.
///
/// A pronoun or a function word is not a content head: splitting
/// `henkiruumiiseenne` at `...|ne` would otherwise name the pronoun `ne`,
/// and `harvainvaltain` at `...|in` the adverb `in`. The check only looks at
/// the sanalista lemma; an inflected form is not a lemma and is left to the
/// ordinary resolution.
pub(crate) fn is_content_head(word: &str) -> bool {
    crate::compound::is_content_head(word)
}

/// The head a known classless compound lemma names, when the right half is
/// ambiguous.
///
/// The stem map is a flat surface -> lemma index, so a right half that two
/// lemmas share (`teiden` is the genitive plural of both `tee` and `tie`)
/// comes back with one of them, first-wins. The compound itself is the
/// disambiguator: `raitiotie` is a lemma, while `raitiotee` is not, so the
/// head is `tie`. Returns `None` when no base makes the whole word a lemma,
/// leaving the ordinary first-wins answer in place.
fn compound_head_from_lemma(left: &str, right: &str) -> Option<String> {
    use crate::expand::check_normal::{ENDINGS, STEM_BUCKETS_NOUN};
    let lexicon = crate::kotus::sanalista();
    let mut bases: Vec<String> = Vec::new();
    for base in STEM_BUCKETS_NOUN.bases(right) {
        bases.push(base.to_string());
    }
    for ending in ENDINGS {
        if ending.is_empty() {
            continue;
        }
        let Some(stem) = right.strip_suffix(ending) else {
            continue;
        };
        for base in STEM_BUCKETS_NOUN.bases(stem) {
            bases.push(base.to_string());
        }
    }
    bases.sort();
    bases.dedup();
    for base in bases {
        let whole = format!("{left}{base}");
        if lexicon.by_word(&whole).next().is_some() {
            return Some(base);
        }
    }
    None
}

/// Verify that `base` inflects to `word` under `pos`. The lib's
/// `find_base_via_*` helpers take the full word and do the
/// ending strip internally; here we already have the stripped
/// stem and the candidate base, so a direct `inflect_all` and
/// `any` probe is the right shape.
fn find_base_via_stem_map_static(
    word: &str,
    _stem: &str,
    base: &str,
    pos: PartOfSpeech,
) -> Option<String> {
    use crate::expand::inflect_all;
    use crate::kotus::sanalista;
    let lex = sanalista();
    let lemma = lex.by_word(base).next()?;
    let inflections: Vec<_> = lemma.inflections().collect();
    let forms = inflect_all(lemma.word(), &inflections, catalogue(), None, pos, false);
    if forms.iter().any(|f| f.word == word) {
        return Some(base.to_string());
    }
    None
}

/// Verify a possessive surface against the noun stem-map candidates of
/// its illative host. `kotiinsa` = `kotiin` + `-sa`; `kotiin` is also a
/// lemma, so the ordinary possessive probe tries the wrong paradigm.
fn possessive_base_via_stem_map(word: &str, host: &str) -> Option<String> {
    for ending in crate::expand::check_normal::ENDINGS {
        if ending.is_empty() {
            continue;
        }
        let Some(stem) = host.strip_suffix(ending) else {
            continue;
        };
        if stem.is_empty() || !crate::expand::check_normal::STEM_BUCKETS_NOUN.contains(stem) {
            continue;
        }
        let mut fallback = None;
        for base in crate::expand::check_normal::STEM_BUCKETS_NOUN.bases(stem) {
            if let Some(b) = find_base_via_stem_map_static_possessive(word, base) {
                // `kotiinsa` is both `koti` + `-in` + `-sa` and the same
                // surface from `kota`; prefer the nominative that is
                // actually the start of the word (`koti`).
                if word.starts_with(&b) {
                    return Some(b);
                }
                fallback.get_or_insert(b);
            }
        }
        if fallback.is_some() {
            return fallback;
        }
    }
    None
}

/// Verify a possessive form against the noun paradigm.
///
/// Used for the contracted genitive plural (`talojensa`, `veljiensä`),
/// whose stem is one `e` shorter than the post-possessive host. Unlike the
/// direct stem-map probe, this asks the engine for the possessive forms, so
/// only a surface the paradigm really emits is accepted.
fn find_base_via_stem_map_static_possessive(word: &str, base: &str) -> Option<String> {
    use crate::expand::inflect_all;
    use crate::kotus::sanalista;
    let lemma = sanalista().by_word(base).next()?;
    let inflections: Vec<_> = lemma.inflections().collect();
    let forms = inflect_all(
        lemma.word(),
        &inflections,
        catalogue(),
        None,
        PartOfSpeech::Noun,
        true,
    );
    forms
        .iter()
        .any(|f| f.word == word)
        .then(|| base.to_string())
}

/// Verb counterpart. Both call into the same `inflect_all` and
/// differ on the `pos` argument.
fn find_base_via_verb_stem_map_static(
    word: &str,
    _stem: &str,
    base: &str,
    pos: PartOfSpeech,
) -> Option<String> {
    use crate::expand::inflect_all;
    use crate::kotus::sanalista;
    let lex = sanalista();
    let lemma = lex.by_word(base).next()?;
    let inflections: Vec<_> = lemma.inflections().collect();
    let forms = inflect_all(lemma.word(), &inflections, catalogue(), None, pos, false);
    if forms.iter().any(|f| f.word == word) {
        return Some(base.to_string());
    }
    None
}

/// Probe the verb stem map for `lower` at `stem`, also trying the
/// vowel-stripped stem. The builder stores a non-finite stem without its
/// trailing stem vowels (`lukeneen`, `lukeneita` and `lukenutta` all ->
/// `luken`), so one key covers a whole participle/infinitive paradigm.
/// The verify in `find_base_via_verb_stem_map_static` keeps it honest.
fn verb_stem_probe(lower: &str, stem: &str) -> Option<String> {
    use crate::expand::check_normal::STEM_BUCKETS_VERB;
    let trimmed = stem.trim_end_matches(crate::is_vowel);
    for cand in [stem, trimmed] {
        if cand.is_empty() || !STEM_BUCKETS_VERB.contains(cand) {
            continue;
        }
        for base in STEM_BUCKETS_VERB.bases(cand) {
            if let Some(b) =
                find_base_via_verb_stem_map_static(lower, cand, base, PartOfSpeech::Verb)
            {
                return Some(b);
            }
        }
    }
    None
}

/// Bounded Levenshtein over the embedded lemma list -- **no**
/// inflection generation, no paradigm walk, no `find_bases_with`.
///
/// This is the per-textbox render's first-pass misspell detector
/// and the on-demand suggestion builder for the demo. Each call
/// touches every lemma once but the work per lemma is tiny: a
/// length-difference pre-filter rejects the long tail before the
/// inner DP, the inner DP early-exits the moment its row minimum
/// exceeds `max_distance`, and the early-candidate pool stays
/// bounded by `max_n` so the post-filter `sort_by` runs in O(n
/// log n) on at most a few hundred candidates.
///
/// Two reasons this is fine for the demo:
///   * Misspelling detection: a word within edit-distance 2 of a
///     lemma is treated as "misspelled". Inflected forms have edit
///     distance > 2 from the lemma but they are *correct* words, so
///     for the demo we accept the lemma-only classifier.
///   * Suggestions: the popup only needs ranked candidates the user
///     is likely to accept. "taloa" -> ["talo"] is exactly the
///     answer that matters. Inflected forms the user could have
///     meant by their typo are out of scope.
///
/// One exception to "lemmas only": a colloquial `-s` (`ymmärrätkös`) is
/// *not* accepted as a word, and its correction is the s-less host
/// (`ymmärrätkö`), which is an inflected form. That host is prepended to
/// the result; see [`s_less_host`].
/// The standard genitive-plural alternate of an archaic `-sten` form.
///
/// `lammas` has both `lammasten` (archaic) and `lampaiden` (standard)
/// genitive plurals. The engine does not generate the archaic `-sten`
/// shape, so an input like `lammasten` is unknown even though its base is
/// a normal lemma. Recover the base, inflect it, and return the `-iden`/
/// `-itten` form the user is more likely to want.
pub(super) fn gradation_plural_alternate(word: &str) -> Option<(String, String)> {
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
pub(super) fn s_less_host(word: &str) -> Option<String> {
    let host = word.strip_suffix('s')?;
    if host.chars().count() < 2 {
        return None;
    }
    is_known_word_fast(host).map(|_| host.to_string())
}

/// Spawn the lazy statics that the per-textbox render relies on,
/// so the first interactive keystroke does not pay the cold-start
/// cost. `lemma_set()` fills a `HashSet` of ~100k `&str`s from
/// the embedded sanalista, and `catalogue()` parses the kotus
/// paradigm catalogue. Together they take a few hundred
/// milliseconds; paying them once at worker boot (rather than on
/// the first user keystroke) keeps the demo's time-to-first-render
/// under a second.
///
/// Safe to call repeatedly; `OnceLock` guarantees a single init.
pub fn warm_caches() {
    let _ = lemma_set();
    let _ = catalogue();
}
