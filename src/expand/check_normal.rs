//! The per-keystroke "is this a known Finnish word?" gate the web
//! demo's first pass uses, lifted out of `tools/src/bin/check_normal.rs`
//! into the library proper so any consumer (the wasm, the cli, a
//! future Python binding) can use the same pipeline.
//!
//! ## The pipeline
//!
//! Six gates, fastest first; the first hit wins. The gates are
//! designed so the common typing-path inputs (lemmas, common
//! inflections, clitic-bearing forms) are O(1) at load time, with
//! the slow prefix-walk engine reserved for the miss path:
//!
//! 1. **Lemma + productive noun-case stripper** -- the wasm's
//!    `is_known_word` first pass. Cheap: lemma HashSet probe +
//!    suffix-strip + reprobe.
//!
//! 2. **Compile-time form subset** -- a [`StaticSet`] of
//!    lemmas and genitive-singular forms (see
//!    [`crate::expand::all_forms`]). O(1) per probe; zero
//!    cold-start cost.
//!
//! 3. **Clitic strip + form subset** -- a word like `siellähän`
//!    is `siellä` + `-hän`. Strip a clitic and re-probe the
//!    form subset. Up to ~6 clitics stacked.
//!
//! 4. **Noun stem hashmap** -- a [`StemMap`] keyed on the
//!    stem (`ves` -> `[vesi]`), built from
//!    `data/stem_to_base_map.csv` via
//!    [`tools/build_stem_to_base_map.py`]. For each known
//!    Finnish case ending, strip and look the stem up; verify
//!    by re-inflecting the candidate base and checking if the
//!    original form is in the result.
//!
//! 4b. **Verb stem hashmap** -- same shape as gate 4, sourced
//!    from `data/verb_stem_to_base_map.csv`. Covers finite
//!    indicative, conditional, potential, imperative (active
//!    and passive), infinitives, and active participles. The
//!    stripper tries the verb endings *and* the nominal case
//!    endings, because the non-finite forms decline as nominals
//!    (`lukeneen`, `lukeneita`, `kukistumiseen`). Verify
//!    uses `PartOfSpeech::Verb`.
//!
//! 5. **Slow check** -- [`crate::expand::find_any_base_with`].
//!    The head/tail/prefix walks plus the clitic phase. Per-word,
//!    ~50-200 ms; the typing path rarely reaches this.
//!
//! The verify step (re-inflect the candidate base) is what
//! makes gates 4 and 4b correct: a wrong stem split (e.g.,
//! `vedet` -> `vede`) still has to match the original word in
//! the candidate base's paradigm to win. The AGENTS.md "no
//! shortcuts in `is_known_word`" rule is honored -- every match
//! comes from the engine's actual inflect output.
//!
//! [`tools/build_stem_to_base_map.py`]: ../../tools/build_stem_to_base_map.py

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::expand::all_forms;
use crate::expand::clitic::strip_clitic;
use crate::expand::find_any_base_with;
use crate::expand::inflect_all;
use crate::expand::static_table::StemMap;
use crate::kotus::{Lemma, Sanalista};
use crate::spec::Catalogues;
use crate::suggest::is_known_word;
use crate::tags::PartOfSpeech;

// ---------------------------------------------------------------------------
// Stem maps. Each is a single binary blob: a front-coded, sorted key
// region (the stems), a value table `(start, count)` per key, and a
// NUL-separated bases region. The whole table is `include_bytes!`, so
// the compiler never parses the keys. See
// [`crate::expand::static_table::StemMap`] for the format and reader.
// ---------------------------------------------------------------------------

/// Noun stem map: built by
/// [`tools/build_stem_to_base_map.py`] from the kotus noun oracle.
/// ~683 k unique stems, ~1500 multi-base. The blob is checked in at
/// `data/precomputed/stem_map_noun.bin`; the build script overwrites it
/// on CSV change.
pub static STEM_BUCKETS_NOUN: StemMap = StemMap::new(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/precomputed/stem_map_noun.bin",
)));

/// Verb counterpart: `data/verb_stem_to_base_map.csv`. ~268 k unique
/// stems: finite personal + infinitive + active participle forms, the
/// passive impersonal forms, and the non-finite case-stems. Same
/// `data/precomputed/` layout.
pub static STEM_BUCKETS_VERB: StemMap = StemMap::new(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/precomputed/stem_map_verb.bin",
)));

// ---------------------------------------------------------------------------
// Endings lists. Must stay in sync with the `ENDINGS_BY_CASE_NUMBER`
// (noun builder) and `PERSONAL_ENDINGS`/`INFINITIVE_ENDINGS`/
// `PARTICIPLE_ENDINGS`/`PASSIVE_PERSONAL_ENDINGS`/
// `PASSIVE_PARTICIPLE_ENDINGS` (verb builder) tables in
// `tools/build_stem_to_base_map.py` and
// `tools/build_verb_stem_to_base_map.py`. If a case ending is
// added there, add the matching strip suffix here so the runtime
// lookup can recover the stem.
// ---------------------------------------------------------------------------

pub const ENDINGS: &[&str] = &[
    // Plural `-i-` case shapes, mirroring `ENDINGS_BY_CASE_NUMBER` in
    // `tools/build_stem_to_base_map.py`. The stem they strip to is the
    // singular stem (`ajoilta` -> `ajo`); without them only the
    // `-lta`/`-sta` tail is removed and the residue `ajoi` is not a
    // key, so every plural oblique was missed.
    "iltä", "ilta", "istä", "ista", "issä", "inä", "ina", "jen", "ten", "den", "siin",
    // Comitative plural: `monine`, `koirine`, `taloine`. The stem
    // before `-ine` is the plural stem without its final i (`mon`,
    // `koir`, `talo`), which is already a stem-map key.
    "ine", // Longest first so the canonical split wins.
    "isiin", "kaan", "kään", "seen", "hon", "hön", "jaa", "jää", "ihin", "issa", "istä", "illa",
    "iltä", "ille", "iksi", "tta", "ttä", "issa", "istä", "illa", "illä", "iltä", "ille", "ssa",
    "ssä", "sta", "stä", "lla", "llä", "lle", "lta", "ltä", "ksi", "han", "hän", "kin", "isi",
    "na", "nä", "ta", "tä", "ja", "jä", "en", "an", "än", "in", "on", "ön", "un", "yn",
    // Partitive singular: `-a` / `-ä` (e.g. `taloa`, `muistavaa`).
    // Also used by the participle case stripper in `is_known_word`
    // -- the bare participle (`muistava`) is a verb stem map key,
    // and its partitive form (`muistavaa`) peels off the bare
    // `-a`. Longest-first ordering means the multi-character
    // cases above still win when both match.
    "a", "ä", "n", "t",
];

/// Possessive suffixes (`talo+nsa`, `talo+ssa+ni`). Post-inflectional
/// like a clitic but, unlike a clitic, the form they produce is not in
/// the form subset or the stem maps, so the fast path has to strip one
/// and re-probe the host.
///
/// The third person has two shapes: `-nsa`/`-nsä`, and the contracted
/// `-Vn` that follows a case ending already ending in a vowel: the
/// case-final vowel doubles before the `n` (`vaatteita` + `an` =
/// `vaatteitaan`, `itselle` + `en` = `itselleen`, `itsellä` + `än` =
/// `itsellään`). The `a`/`ä`/`e` variants are here.
const POSSESSIVE_SUFFIXES: &[&str] = &["nsa", "nsä", "nne", "mme", "si", "ni", "an", "än", "en"];

pub const VERB_ENDINGS: &[&str] = &[
    // Long personal (conditional + potential)
    "isimme",
    "isitte",
    "isivät",
    "isivat",
    "nemme",
    "nette",
    "nevät",
    "nevat",
    // Personal past
    "imme",
    "itte",
    "ivät",
    "ivat",
    // Personal present
    "mme",
    "tte",
    // Personal (present + past singular)
    "vat",
    "vät",
    "inen", // disambiguate from "in" past
    // Imperative (longer forms first)
    "kaamme",
    "käämme",
    "koon",
    "köön",
    "koot",
    "kööt",
    "kaa",
    "kää",
    // Conditional (without personal plural)
    "isin",
    "isit",
    "isi",
    // Potential (without personal plural)
    "nen",
    "net",
    "nee",
    // Personal (single chars)
    "in",
    "it",
    "n",
    "t",
    // E-infinitive short form (e.g. "tietäen" -> "tietä")
    "en",
    "än",
    // Participles
    "va",
    "vä",
    "nnut",
    "neet",
    "nyt",
    "ma",
    "mä",
    // E-infinitive long form (e.g. "tietäessä" -> "tietä")
    "essa",
    "essä",
    // 4th infinitive (e.g. "tietäminen" -> "tietä")
    "minen",
    "mäinen",
    // ---- Passive shapes ----
    //
    // Finnish passive is impersonal -- person/number are empty --
    // and the stem is class-driven (e.g. lukea -> "lueta-",
    // tietää -> "tiedetä-"). The longest-first match tries each
    // candidate shape; the verify step (re-inflect) is the source
    // of truth, so over-emitting is harmless.
    //
    // Indicative present 3sg: "tiedetään" -> "tiedetä",
    //                          "luetaan" -> "lueta"
    "ttaisiin",
    "ttäisiin",
    "ttaneen",
    "ttäneen",
    "ttäköön",
    "ttakoon",
    "taan",
    "tään",
    "aan",
    "ään",
    // Indicative past: "tiedettiin" -> "tiedetä", "luettiin" -> "luet"
    "ttiin",
    "tiin",
    "iin",
    // Past participle: "tiedetty" -> "tiedet", "luettu" -> "luet"
    "ttu",
    "tty",
    // Present participle: "tiedettävä" -> "tiedetä",
    //                     "luettava" -> "lueta"
    "ttävä",
    "ttava",
    "tävä",
    "tava",
];

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Cached catalogue + lexicon, so the per-word verify doesn't pay
// the parse cost. Same lifetime trick `tools/src/lib.rs::find_any_base`
// uses: one parse per process, then O(1) lookups.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The catalogue (embedded, decoded once by `Catalogues::embedded`)
// and the lexicon slice (`kotus::lemmas`, materialised once from the
// blob).
// ---------------------------------------------------------------------------

mod adverb {
    //! Tabulated adverbial forms (form -> lemma), built by `build.rs`
    //! from `data/adverbit.csv`.
    use crate::expand::static_table::StemMap;

    pub static ADVERBS: StemMap = StemMap::new(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/precomputed/adverbit.bin",
    )));
}

fn cat() -> &'static Catalogues {
    Catalogues::embedded()
}

fn lex() -> &'static [Lemma<'static>] {
    crate::kotus::lemmas()
}

/// Surface form -> lemma for the closed pronoun class (Kotus 101).
///
/// The pronouns are the one class whose forms the class catalogue cannot
/// derive, so their paradigms live in `grammar::irregular`. The fast path
/// cannot afford `find_any_base_with` for every word, but the class is
/// closed and tiny: inflect every class-101 lemma once, then probe the map.
/// This is what makes `minkä` and `millä` constant-time while keeping the
/// forms themselves in the engine's paradigm, not a second hand-written
/// table.
static PRONOUN_FORMS: OnceLock<HashMap<String, &'static str>> = OnceLock::new();

fn pronoun_forms() -> &'static HashMap<String, &'static str> {
    PRONOUN_FORMS.get_or_init(|| {
        let mut map = HashMap::new();
        for lemma in lex() {
            if !lemma.inflections.iter().any(|i| i.class == 101) {
                continue;
            }
            let pos = PartOfSpeech::parse_all(lemma.part_of_speech)
                .first()
                .copied()
                .unwrap_or(PartOfSpeech::Noun);
            for form in inflect_all(lemma.word, &lemma.inflections, cat(), None, pos, false) {
                map.entry(form.word).or_insert(lemma.word);
            }
        }
        map
    })
}

/// The pronoun lemma `word` is a form of, if any.
///
/// `word` is expected lowercase. The map is built on the first miss, so the
/// typing path pays for it once; the ordinary lemma hit never does.
pub fn pronoun_base(word: &str) -> Option<&'static str> {
    pronoun_forms().get(word).copied()
}

/// The lemma of a tabulated adverbial form (`torkuksissa` -> `torkku`).
///
/// The table is the same one gate 1b probes; exposing it lets the compound
/// detector treat an adverbial member as a known word.
pub fn adverb_base(word: &str) -> Option<&'static str> {
    adverb::ADVERBS.bases(word).next()
}

// Re-export the embedded sanalista so callers can do their own
// lookups if they need them.
pub fn sanalista_view() -> &'static Sanalista {
    crate::kotus::sanalista()
}

// ---------------------------------------------------------------------------
// Outcome of a single per-word check.
// ---------------------------------------------------------------------------

/// What the gate found for a query word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedOutcome {
    /// The query is exactly a lemma in the sanalista.
    Lemma,
    /// The query is an inflected form of `base`. `base` is the
    /// lemma whose paradigm emits the query (e.g. for `vettä`
    /// the result is `Inflected { base: "vesi", .. }`).
    Inflected { base: String, gate: &'static str },
    /// The query is not in the lexicon and no clause was found.
    Unknown,
}

// ---------------------------------------------------------------------------
// The pipeline. The five gates are tried in order; the first hit
// wins. Each gate is a constant-time probe (HashSet / StaticSet
// / StemMap / pre-computed form set) except gate 5 (the slow
// engine), which is ~50-200 ms per word.
// ---------------------------------------------------------------------------

/// Run the six-gate pipeline on `word` and return the first hit.
///
/// The pipeline mirrors what the wasm's typing path fires for
/// every keystroke and what `tools/src/bin/check_normal` prints.
/// Both consumers are thin wrappers over this function; the
/// gate logic itself lives here so the lib's behaviour is the
/// single source of truth.
///
/// If `verbose` is true, the function prints which gate fired
/// (and what it tried) to stdout -- useful for debugging but
/// not for the wasm's per-keystroke path.
pub fn check_normal(word: &str, verbose: bool) -> NormalizedOutcome {
    check_normal_with(word, verbose, /* engine_gate = */ true)
}

/// The same pipeline with the slow engine gate removed.
///
/// The cheap gates (1, 2, 3, 4, 4b) are constant-time
/// HashMap / StaticSet / StemMap probes; the *engine gate* is the
/// head/tail/prefix walk (`find_any_base_with`), which retains every
/// candidate form it visits. It is the dominant cost by orders of
/// magnitude -- in the wasm it is tens to hundreds of MB of
/// transient heap and up to seconds per word -- and both gate 5 and
/// the base-naming step inside gates 2/3 call it.
///
/// This variant answers "is this a known form?" from the cheap gates
/// alone. A word that only the engine gate would recognise comes
/// back [`NormalizedOutcome::Unknown`]. Callers that need the engine
/// answer (the CLI, the first-pass diagnostics) use
/// [`check_normal`]; callers on a hot path that must stay bounded
/// (the wasm's per-candidate confirm pass) use this.
pub fn check_normal_fast(word: &str, verbose: bool) -> NormalizedOutcome {
    // The gate is a pure function of the word: the dictionary is static and
    // no text-dependent input reaches it. A page repeats words constantly
    // (`the`, names, case forms of the same lemma), and the per-textarea
    // wasm cache covers only one panel, so a thread-local memo pays for
    // itself even on the first render.
    if verbose {
        return check_normal_with(word, true, /* engine_gate = */ false);
    }
    if let Some(hit) = FAST_CACHE.with_borrow(|cache| cache.get(word).cloned()) {
        return hit;
    }
    let outcome = check_normal_with(word, false, /* engine_gate = */ false);
    FAST_CACHE.with_borrow_mut(|cache| {
        if cache.len() >= FAST_CACHE_LIMIT {
            // A large paste can hold tens of thousands of distinct tokens.
            // Clearing keeps the memo bounded; the words still on screen are
            // re-checked once.
            cache.clear();
        }
        cache.insert(word.to_string(), outcome.clone());
    });
    outcome
}

thread_local! {
    /// Thread-local memo for [`check_normal_fast`]. Per-thread rather than
    /// process-wide so the native multi-threaded walks do not contend on a
    /// lock; the wasm is one thread.
    static FAST_CACHE: RefCell<HashMap<String, NormalizedOutcome>> =
        RefCell::new(HashMap::new());
}

const FAST_CACHE_LIMIT: usize = 16_384;

/// The cheap derivation probes from `is_known_word_no_compound`, without
/// its noun/verb stem loops. Productive `-inen`/`-kas`/`-lainen` adjectives,
/// derived verbs and agent nouns, the lexical supplement and the closed
/// pronoun class.
fn cheap_derived_base(word: &str) -> Option<String> {
    let lower = word.to_lowercase();
    if let Some(base) = crate::expand::check_normal::adverb_base(&lower) {
        return Some(base.to_string());
    }
    if let Some(base) = crate::expand::derived::base(&lower) {
        return Some(base.to_string());
    }
    if let Some(base) = crate::expand::fast_path::base(&lower) {
        return Some(base.to_string());
    }
    if let Some(base) = crate::expand::check_normal::pronoun_base(&lower) {
        return Some(base.to_string());
    }
    // Shape guards keep the candidate generators off ordinary words:
    // `inen_candidates` strips every case ending before testing.
    if (lower.contains("is") || lower.contains("ine"))
        && let Some(base) = crate::expand::derived_inen_base(&lower, cat(), None)
    {
        return Some(base);
    }
    if (lower.contains("kas")
        || lower.contains("käs")
        || lower.contains("kka")
        || lower.contains("kkä"))
        && let Some(base) = crate::expand::derived_kas_base(&lower, cat(), None)
    {
        return Some(base);
    }
    if (lower.contains("lainen")
        || lower.contains("läinen")
        || lower.contains("lais")
        || lower.contains("läis"))
        && let Some(base) = crate::expand::derived_lainen_base(&lower, cat(), None)
    {
        return Some(base);
    }
    if (lower.contains("tar") || lower.contains("tär"))
        && let Some(base) = crate::expand::derived_tar_base(&lower, cat(), None)
    {
        return Some(base);
    }
    crate::expand::derived_agent_base(&lower, cat(), None)
}

fn check_normal_with(word: &str, verbose: bool, engine_gate: bool) -> NormalizedOutcome {
    // ── Gate 1: lemma + productive noun-case stripper ─────────────
    if verbose {
        println!("    gate 1: is_known_word({word:?})...");
    }
    // The base `is_known_word` reaches *without* a compound split. It is
    // the word's own lemma, so it is safe to report; the compound-aware
    // `is_known_word` would hand back a head (`taloilta` -> `ilta`).
    // Held as the last-resort base: the later gates name the base more
    // precisely, but when they cannot -- and the fast path has no engine
    // gate to fall back on -- this keeps a recognised word from turning
    // into `Unknown` (the negation forms, possessives and compounds the
    // gate-1 stripper knows and the stem maps do not).
    //
    // It is *expensive*: `is_known_word_no_compound` runs the whole
    // non-compound gate stack, which the later gates then run again. Do not
    // compute it here; the fast and full paths ask for it only after their
    // own gates have missed. For `avioliittoaikeita` this alone was 109 ms
    // of the 1.2 s.
    // A direct lemma is the one thing gate 1 settles on its own; every
    // other recognised form falls through so a later gate can name the
    // base. The sanalista probe is the lemma test.
    if crate::kotus::sanalista().by_word(word).next().is_some() {
        if verbose {
            println!("    gate 1: hit (direct lemma)");
        }
        return NormalizedOutcome::Lemma;
    }
    if verbose {
        if crate::suggest::is_known_word_no_compound(word).is_some() {
            println!("    gate 1: productive form, not a lemma");
        } else {
            println!("    gate 1: miss");
        }
    }

    // A closed lexical exception (e.g. `hennonnut` -> `hennota`, or
    // `Selmallakaan` -> `Selma`) is exact and must not be re-analysed by the
    // stem maps: those would name `hennoa` or `selmalla` instead.
    if let Some(base) = crate::expand::fast_path::base(&word.to_lowercase()) {
        return NormalizedOutcome::Inflected {
            base: base.to_string(),
            gate: "lexical exception",
        };
    }

    // ── Gate 1b: tabulated adverbial form ──────────────────────────
    //
    // A few `-ksi` adverbs take a possessive (`kylliksi` ->
    // `kylliksenne`), which no rule derives and Omorfi cannot generate;
    // the confirmed set is embedded (`tools/build_adverbi_forms.py`).
    if let Some(base) = adverb::ADVERBS.bases(word).next() {
        if verbose {
            println!("    gate 1b: adverb {word:?} -> {base}");
        }
        return NormalizedOutcome::Inflected {
            base: base.to_string(),
            gate: "adverb",
        };
    }

    // ── Gate 1c: negation auxiliary / fused forms ──────────────────
    //
    // The closed table names the lemma (`mikset` -> `miksei`, `etten` ->
    // `että`). Without this gate the verb stem map can claim the surface
    // as an unrelated verb form (`mikset` -> `miksata`).
    if let Some(base) = crate::expand::negation::negation_base(word) {
        if verbose {
            println!("    gate 1c: negation {word:?} -> {base}");
        }
        return NormalizedOutcome::Inflected {
            base: base.to_string(),
            gate: "negation",
        };
    }

    // ── Gate 2: pre-computed form subset ───────────────────────────
    if verbose {
        println!("    gate 2: is_known_inflected_form({word:?})...");
    }
    if all_forms::is_known_inflected_form(word) {
        if verbose {
            println!("    gate 2: hit (word is in the form set)");
        }
        // The form set knows the surface but not the lemma; ask
        // the slow check for the base so the answer is useful.
        // `check_normal_fast` skips that engine call -- the
        // surface being in the form set is the answer; the base
        // is a nicety, not a correctness gate.
        if let Some(base) = engine_base(word, engine_gate, verbose) {
            return NormalizedOutcome::Inflected {
                base,
                gate: "form set",
            };
        }
        if verbose {
            if engine_gate {
                println!(
                    "    gate 2: base_for() missed (form set says known, \
                     slow check disagrees -- accepting anyway)"
                );
            } else {
                println!("    gate 2: engine gate skipped (form set says known)");
            }
        }
        return NormalizedOutcome::Inflected {
            base: "(in form set)".to_string(),
            gate: "form set",
        };
    } else if verbose {
        println!("    gate 2: miss");
    }

    // ── Gate 3: clitic strip + form set ───────────────────────────
    if verbose {
        println!("    gate 3: clitic strip + form set");
    }
    let mut current: String = word.to_string();
    let mut layer = 0;
    while let Some((host, clitic)) = strip_clitic(&current) {
        layer += 1;
        if verbose {
            println!(
                "      layer {layer}: strip {clitic:?} from {current:?} \
                 -> host {host:?}"
            );
        }
        if all_forms::is_known_inflected_form(&host) {
            if verbose {
                println!("      layer {layer}: host {host:?} in form set");
            }
            if let Some(base) = engine_base(word, engine_gate, verbose) {
                return NormalizedOutcome::Inflected {
                    base,
                    gate: "clitic of form set",
                };
            }
            return NormalizedOutcome::Inflected {
                base: "(clitic of form set entry)".to_string(),
                gate: "clitic of form set",
            };
        }
        // The form subset carries lemmas and genitive singulars only, so a
        // clitic on a verb form (`nouseeko`) or another case (`ajoiltaanko`)
        // is not in it. Try the cheap gates on the host before dropping a
        // layer: the host is a lemma, or resolves through a stem map.
        // The first pass must not run the full compound detector on a
        // clitic host: a long unknown such as `niistätyttänytkään` spent
        // seconds there. Use the same fast compound probe as gate 4c and
        // leave the deep engine answer to the suggestion pool.
        let host_base = if engine_gate {
            is_known_word(&host)
        } else {
            crate::suggest::is_known_word_no_compound(&host)
                .or_else(|| crate::suggest::is_known_compound_fast(&host))
        };
        if let Some(base) = host_base {
            if verbose {
                println!("      layer {layer}: host {host:?} is a known word -> {base}");
            }
            return NormalizedOutcome::Inflected {
                base,
                gate: "clitic of known word",
            };
        }
        if let Some(base) = find_base_via_verb_stem_map(&host, verbose) {
            return NormalizedOutcome::Inflected {
                base,
                gate: "clitic of verb stem",
            };
        }
        if let Some(base) = find_base_via_stem_map(&host, PartOfSpeech::Noun, verbose) {
            return NormalizedOutcome::Inflected {
                base,
                gate: "clitic of noun stem",
            };
        }
        if let Some((base, gate)) = find_through_possessive(&host, verbose) {
            return NormalizedOutcome::Inflected { base, gate };
        }
        if verbose {
            println!(
                "      layer {layer}: host {host:?} not in form set, \
                 try next layer"
            );
        }
        if host.len() >= current.len() {
            break;
        }
        current = host;
    }
    if verbose && layer > 0 {
        println!("      no clitic-bearing host landed in the form set");
    }

    // ── Gate 3b: possessive suffix ──────────────────────────────────
    //
    // `is_known_word` (gate 1) already accepts possessives, but the
    // base it hands back is the compound head for a compound-looking
    // word, so it cannot be returned directly. Name the base here
    // through the stem maps: `vaimonsa` strips `nsa` to `vaimo`.
    if let Some((base, gate)) = find_through_possessive(word, verbose) {
        if verbose {
            println!("    gate 3b: {base} via {gate}");
        }
        return NormalizedOutcome::Inflected { base, gate };
    }

    // The colloquial `-s` (`ymmärrätkös`, `mitäs`) is deliberately *not*
    // a gate here. It is spoken-language, so the checker flags the word
    // and the on-click pass offers the s-less host instead (see
    // `suggest::suggest_over_lemmas`). A word that genuinely ends in `-s`
    // -- an ordinal like `kahdeksas`, an adverb like `ulos` -- is a
    // sanalista lemma and is accepted at gate 1, before this point.

    // ── Gate 4: noun stem hashmap ───────────────────────────────────
    if verbose {
        println!("    gate 4: stem hashmap ({} known endings)", ENDINGS.len());
    }
    if let Some(base) = find_base_via_stem_map(word, PartOfSpeech::Noun, verbose) {
        if verbose {
            println!("    gate 4: hit -> {base}");
        }
        return NormalizedOutcome::Inflected {
            base,
            gate: "noun stem",
        };
    }
    if verbose {
        println!("    gate 4: miss -> falling through to gate 4b");
    }

    // ── Gate 4b: verb stem hashmap ─────────────────────────────────
    if verbose {
        println!(
            "    gate 4b: verb stem hashmap ({} known endings)",
            VERB_ENDINGS.len()
        );
    }
    if let Some(base) = find_base_via_verb_stem_map(word, verbose) {
        if verbose {
            println!("    gate 4b: hit -> {base}");
        }
        return NormalizedOutcome::Inflected {
            base,
            gate: "verb stem",
        };
    }
    // ── Gate 4b-extension: `-toista` numeral ────────────────────────
    //
    // The teens and their ordinals keep `toista` fixed and inflect the
    // first member (`kuuden`|`toista`), so the case ending is *internal*
    // and the suffix-stripping stem map cannot reach them. The engine
    // derives the paradigm the same way (`expand::numeral_forms`); this
    // is its fast-path mirror: strip the invariant `toista`, resolve the
    // prefix, and name the numeral from it.
    if let Some(prefix) = word.strip_suffix("toista")
        && !prefix.is_empty()
    {
        if let Some(base) = find_base_via_stem_map(prefix, PartOfSpeech::Noun, verbose)
            .or_else(|| find_base_via_verb_stem_map(prefix, verbose))
        {
            let base = format!("{base}toista");
            if verbose {
                println!("    gate 4b-numeral: {word:?} -> {base}");
            }
            return NormalizedOutcome::Inflected {
                base,
                gate: "numeral",
            };
        }
        if crate::kotus::sanalista().by_word(prefix).next().is_some() {
            let base = format!("{prefix}toista");
            if verbose {
                println!("    gate 4b-numeral: {word:?} -> {base}");
            }
            return NormalizedOutcome::Inflected {
                base,
                gate: "numeral",
            };
        }
    }
    if verbose {
        println!("    gate 4b: miss -> falling through to gate 5");
    }

    // ── Gate 4b2: proper name ──────────────────────────────────────
    //
    // Wikidata place/personal names expanded to their Omorfi singular
    // cases. This is a *fallback*: a surname such as `Talo` puts `talossa`
    // in the name set, and returning it before gate 4 would name the
    // surface instead of `talo`. Names are accepted, never suggested.
    if crate::expand::proper_names::contains_capitalised(word) {
        if verbose {
            println!("    gate 4b2: proper name");
        }
        return NormalizedOutcome::Inflected {
            base: word.to_lowercase(),
            gate: "proper name",
        };
    }

    // ── Gate 5: slow check `find_any_base_with` ─────────────────────
    //
    // The engine gate. `check_normal_fast` turns it off: every
    // earlier gate has already missed, so without the engine the
    // word is unknown.
    if !engine_gate {
        if verbose {
            println!("    gate 5: skipped (engine gate disabled)");
        }
        // Gate 4c: compound, fast flavour. `is_known_compound_fast` does
        // the same modifier/head split as `is_known_word` without the deep
        // compound recursion *or* the engine fallback. On the first pass
        // that difference is the whole render: a page of unknown/proper
        // words used to spend 1-2 s per word trying splits, so a 2000-word
        // page took ~38 s before any underline appeared. Heads the engine
        // fallback would recover (`kekkereitä`) are left to the suggestion
        // pool's full `bg.check`.
        // A compound of 6-7 characters (`esisana`) is below the general
        // fast probe's length floor. Only consult it when the cheap gates
        // found nothing at all: `äesteli` is a verb form gate 1 already
        // knows, and splitting it as `äes` + `teli` would name the wrong
        // base.
        // Long compounds are the case where the non-compound stack is
        // expensive and the DP segmenter is cheap. Short words keep the
        // old order: the direct probe is only microseconds there, and it
        // keeps a known verb form from being renamed to a compound head.
        // A productive derivation is a single word, not a compound: name it
        // before the segmenter gets a chance to split it
        // (`aikaansaamiseksi`, `kolmitahkoisella`).
        if let Some(base) = cheap_derived_base(word) {
            if verbose {
                println!("    gate 4c0: derived -> {base}");
            }
            return NormalizedOutcome::Inflected {
                base,
                gate: "derived",
            };
        }
        let long = word.chars().count() >= 12;
        if long && let Some(base) = crate::suggest::is_known_compound_dp(word) {
            if verbose {
                println!("    gate 4c: compound (dp) -> {base}");
            }
            return NormalizedOutcome::Inflected {
                base,
                gate: "compound",
            };
        }
        let direct_base = crate::suggest::is_known_word_no_compound(word);
        let short_compound = if direct_base.is_none() {
            crate::suggest::is_known_short_compound(word)
        } else {
            None
        };
        if direct_base.is_none()
            && let Some(base) = crate::suggest::is_known_compound_fast(word).or(short_compound)
        {
            if verbose {
                println!("    gate 4c: compound -> {base}");
            }
            return NormalizedOutcome::Inflected {
                base,
                gate: "compound",
            };
        }
        return match direct_base {
            Some(base) => NormalizedOutcome::Inflected {
                base,
                gate: "known word",
            },
            None => NormalizedOutcome::Unknown,
        };
    }
    if verbose {
        println!("    gate 5: find_bases_with (slow check)");
    }
    if let Some(base) = find_any_base_via_engine(word, verbose) {
        if verbose {
            println!("    gate 5: hit -> {base}");
        }
        return NormalizedOutcome::Inflected {
            base,
            gate: "engine",
        };
    }
    // Gate 4c (full path): the engine does not split compounds, so a
    // compound falls here.
    if let Some(base) = is_known_word(word) {
        if verbose {
            println!("    gate 4c: compound -> {base}");
        }
        return NormalizedOutcome::Inflected {
            base,
            gate: "compound",
        };
    }
    let direct_base = crate::suggest::is_known_word_no_compound(word);
    if let Some(base) = direct_base {
        if verbose {
            println!("    gate 5: miss -> known word {base}");
        }
        return NormalizedOutcome::Inflected {
            base,
            gate: "known word",
        };
    }
    if verbose {
        println!("    gate 5: miss -> unknown");
    }
    NormalizedOutcome::Unknown
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The engine call gates 2, 3 and 5 share, gated so
/// [`check_normal_fast`] can skip the slow walk entirely.
fn engine_base(word: &str, engine_gate: bool, verbose: bool) -> Option<String> {
    if engine_gate {
        find_any_base_via_engine(word, verbose)
    } else {
        None
    }
}

/// Slow check: ask the engine's head/tail/prefix walks + clitic
/// phase for the base. Returns the first matching lemma's word.
fn find_any_base_via_engine(word: &str, verbose: bool) -> Option<String> {
    if verbose {
        println!(
            "      base_for({word:?}) -> find_any_base_with \
             (short-circuits on first match; pure inflect check)"
        );
    }
    let bases = find_any_base_with(lex(), cat(), word);
    let first = bases.first()?;
    if verbose {
        println!("      base_for: {:?}", first.word);
    }
    Some(first.word.to_string())
}

/// Look up `word` in the noun stem map. For each known Finnish
/// case ending, strip from `word` to get a stem, O(1)-probe the
/// phf index for that stem, and verify by re-inflecting each
/// candidate base with `PartOfSpeech::Noun`. The verify step is
/// what makes the lookup correct when a stem happens to also
/// be a prefix of a different lemma's forms.
/// The part of speech the sanalista records for `lemma`, or `fallback`.
///
/// The stem maps are noun- and verb-shaped, but an adjective is a
/// noun-like word: verifying `suuri` as a *noun* would skip the degree
/// derivation, which [`inflect_all`] adds only for
/// [`PartOfSpeech::Adjective`], so the verify inflects under the lemma's
/// own part of speech.
fn lemma_pos(lemma: &crate::kotus::EmbeddedLemma, fallback: PartOfSpeech) -> PartOfSpeech {
    crate::tags::PartOfSpeech::parse_all(lemma.part_of_speech())
        .first()
        .copied()
        .unwrap_or(fallback)
}

pub fn find_base_via_stem_map(word: &str, pos: PartOfSpeech, verbose: bool) -> Option<String> {
    // The empty ending first: a form that carries no case suffix has
    // its own stem as the map key (2sg imperative `sano`), and
    // stripping every suffix in turn without ever probing the whole
    // word missed those.
    let endings = std::iter::once("");
    for ending in endings.chain(ENDINGS.iter().copied()) {
        let Some(stem) = word.strip_suffix(ending) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        if !STEM_BUCKETS_NOUN.contains(stem) {
            continue;
        }
        let candidates: Vec<&str> = STEM_BUCKETS_NOUN.bases(stem).collect();
        if verbose {
            println!(
                "      gate 4: stem {stem:?} (stripped {ending:?}) -> \
                 {} candidate(s)",
                candidates.len()
            );
        }
        for base in candidates {
            // O(1) hash lookup, not a scan over the whole lexicon: the
            // verify runs once per candidate base and a linear scan made
            // the gate cost the same as the engine walk it exists to
            // avoid.
            // Every homonym row, not just the first: `kuusi` is the
            // spruce (class 24) and the numeral (class 27), and only the
            // numeral makes `kuuden`.
            let mut hit = false;
            for lemma in crate::kotus::sanalista().by_word(base) {
                let inflections: Vec<_> = lemma.inflections().collect();
                let forms = inflect_all(
                    lemma.word(),
                    &inflections,
                    cat(),
                    None,
                    lemma_pos(&lemma, pos),
                    false,
                );
                if forms.iter().any(|f| f.word == word) {
                    hit = true;
                    break;
                }
            }
            if hit {
                if verbose {
                    println!("      gate 4: verify {base:?} inflects to {word:?} (hit)");
                }
                return Some(base.to_string());
            }
        }
    }
    if verbose {
        println!("      gate 4: no stem match + verify");
    }
    None
}

/// Verb counterpart of [`find_base_via_stem_map`].
pub fn find_base_via_verb_stem_map(word: &str, verbose: bool) -> Option<String> {
    // The negation fusions (`enkä`, `ettet`, ...) are stem-map keys the
    // engine cannot reproduce -- it does not inflect the auxiliary's
    // clitics -- so the `inflect_all` verify below is skipped for them.
    let trust = crate::expand::negation::negation_base(word).is_some();
    // Same as the noun map: probe the bare stem too, which is the key
    // for the forms the builder strips nothing from (3sg present
    // `nousee`/`sanoo`, 2sg imperative `sano`).
    //
    // The nominal `ENDINGS` list is chained after the verb endings
    // because participles and infinitives decline as nominals: their
    // oblique forms (`lukeneen`, `lukeneita`, `kukistumiseen`) are
    // reached by peeling a *case* ending, and the builder emitted the
    // matching case-stem. The verify step re-inflects the base and
    // checks the exact surface, so an extra ending can only add cheap
    // probes, never a false accept.
    let endings = std::iter::once("");
    for ending in endings
        .chain(VERB_ENDINGS.iter().copied())
        .chain(ENDINGS.iter().copied())
    {
        let Some(stem) = word.strip_suffix(ending) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }
        // The builder stores a non-finite stem with its trailing stem
        // vowels stripped (`lukeneen`, `lukeneita`, `lukenutta` all ->
        // `luken`), so one key covers a whole participle/infinitive
        // paradigm. Probe the raw stem and that vowel-stripped form; the
        // verify (re-inflect the base) keeps it honest.
        let trimmed = stem.trim_end_matches(crate::is_vowel);
        for cand in [stem, trimmed] {
            if cand.is_empty() || !STEM_BUCKETS_VERB.contains(cand) {
                continue;
            }
            let candidates: Vec<&str> = STEM_BUCKETS_VERB.bases(cand).collect();
            if verbose {
                println!(
                    "      gate 4b: stem {cand:?} (stripped {ending:?}) -> \
                     {} candidate(s)",
                    candidates.len()
                );
            }
            for base in candidates {
                if trust {
                    if verbose {
                        println!("      gate 4b: trusted stem-map key {cand:?} -> {base:?}");
                    }
                    return Some(base.to_string());
                }
                let mut hit = false;
                for lemma in crate::kotus::sanalista().by_word(base) {
                    let inflections: Vec<_> = lemma.inflections().collect();
                    let forms = inflect_all(
                        lemma.word(),
                        &inflections,
                        cat(),
                        None,
                        lemma_pos(&lemma, PartOfSpeech::Verb),
                        false,
                    );
                    if forms.iter().any(|f| f.word == word) {
                        hit = true;
                        break;
                    }
                }
                if hit {
                    if verbose {
                        println!(
                            "      gate 4b: verify {base:?} inflects to \
                             {word:?} (hit)"
                        );
                    }
                    return Some(base.to_string());
                }
            }
        }
    }
    if verbose {
        println!("      gate 4b: no stem match + verify");
    }
    None
}

/// Resolve a noun stem that a possessive was stripped from, by asking
/// whether the stem's candidate base inflects *with possessives* to the
/// original word. `sisare` + `nsa` gives `sisarensa`; the bare stem
/// `sisare` is not a surface form, but the possessive is.
fn find_base_via_possessive_noun(stem: &str, possessive_word: &str) -> Option<String> {
    if !STEM_BUCKETS_NOUN.contains(stem) {
        return None;
    }
    for base in STEM_BUCKETS_NOUN.bases(stem) {
        for lemma in crate::kotus::sanalista().by_word(base) {
            let inflections: Vec<_> = lemma.inflections().collect();
            let forms = inflect_all(
                lemma.word(),
                &inflections,
                cat(),
                None,
                lemma_pos(&lemma, PartOfSpeech::Noun),
                /* possessives = */ true,
            );
            if forms.iter().any(|f| f.word == possessive_word) {
                return Some(base.to_string());
            }
        }
    }
    None
}

/// Strip a possessive suffix from `host` (if any) and resolve the bare
/// form it leaves behind through the cheap gates. `None` when `host`
/// carries no possessive or the host is not a known bare form.
fn find_through_possessive(host: &str, verbose: bool) -> Option<(String, &'static str)> {
    for suffix in POSSESSIVE_SUFFIXES {
        let Some(inner) = host.strip_suffix(suffix) else {
            continue;
        };
        if inner.is_empty() {
            continue;
        }
        if let Some(base) = find_base_via_stem_map(inner, PartOfSpeech::Noun, verbose) {
            return Some((base, "possessive of noun"));
        }
        // The stripped inner is sometimes a bare *stem* rather than a
        // surface form (`sisarensa` -> `sisare`, whose genitive is
        // `sisaren`), so the bare verify above finds nothing. Verify
        // the original possessive word against the base instead.
        if let Some(base) = find_base_via_possessive_noun(inner, host) {
            if verbose {
                println!("      possessive stem {inner:?} -> {base} (possessive verify)");
            }
            return Some((base, "possessive of noun"));
        }
        if let Some(base) = find_base_via_verb_stem_map(inner, verbose) {
            return Some((base, "possessive of verb"));
        }
        if crate::kotus::sanalista().by_word(inner).next().is_some() {
            return Some((inner.to_string(), "possessive of lemma"));
        }
        if all_forms::is_known_inflected_form(inner) {
            return Some((inner.to_string(), "possessive of form set"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_lemma() {
        let r = check_normal("vesi", false);
        assert_eq!(r, NormalizedOutcome::Lemma);
    }

    #[test]
    fn productive_noun_case() {
        let r = check_normal("vettä", false);
        match r {
            NormalizedOutcome::Inflected { base, .. } => assert_eq!(base, "vesi"),
            other => panic!("expected Inflected, got {other:?}"),
        }
    }

    #[test]
    fn verb_personal_form() {
        let r = check_normal("tiedän", false);
        match r {
            NormalizedOutcome::Inflected { base, .. } => assert_eq!(base, "tietää"),
            other => panic!("expected Inflected, got {other:?}"),
        }
    }

    #[test]
    fn verb_personal_passive() {
        let r = check_normal("tiedetään", false);
        match r {
            NormalizedOutcome::Inflected { base, .. } => assert_eq!(base, "tietää"),
            other => panic!("expected Inflected, got {other:?}"),
        }
    }

    #[test]
    fn unknown_word() {
        let r = check_normal("xyzzy", false);
        assert_eq!(r, NormalizedOutcome::Unknown);
    }

    /// `check_normal_fast` keeps every cheap gate: a direct lemma and
    /// a productive case form still answer without the engine.
    #[test]
    fn fast_variant_keeps_the_cheap_gates() {
        assert_eq!(check_normal_fast("talo", false), NormalizedOutcome::Lemma);
        assert!(matches!(
            check_normal_fast("taloa", false),
            NormalizedOutcome::Inflected { .. }
        ));
    }

    /// Possessive suffixes are post-inflectional, so neither the form
    /// subset nor the stem maps carry them; gate 3b strips one and
    /// re-probes the host.
    #[test]
    fn fast_variant_handles_possessives() {
        for word in ["vaimonsa", "talossani", "kissamme"] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not resolved by the fast path"
            );
        }
    }

    /// A `-ksi` adverb's possessive (`kylliksi` -> `kylliksenne`) is in
    /// the embedded adverb table.
    #[test]
    fn fast_variant_accepts_possessive_adverbs() {
        assert!(matches!(
            check_normal_fast("kylliksenne", false),
            NormalizedOutcome::Inflected { .. }
        ));
    }

    /// The third person's contracted possessive `-an`/`-än` (the
    /// partitive plural `vaatteita` + `an`) and `-en` (the allative
    /// `itselle` + `en`).
    #[test]
    fn fast_variant_accepts_contracted_possessive() {
        for word in [
            "vaatteitaan",
            "taloaan",
            "talojaan",
            "miehiään",
            "itselleen",
            "itsellään",
        ] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted by the fast path"
            );
        }
    }

    /// A compound whose modifier is a combining form (`nais-` of
    /// `nainen`, in `naisvanki`), not a producible form.
    #[test]
    fn fast_variant_accepts_combining_form_compounds() {
        for word in ["naisvanki", "naisvangin", "naisvankien"] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted by the fast path"
            );
        }
    }

    /// A productive `-sti` manner adverb that is not itself a lemma is
    /// derived from its adjective and is in the table.
    #[test]
    fn fast_variant_derives_sti_adverbs() {
        assert!(matches!(
            check_normal_fast("aamuisesti", false),
            NormalizedOutcome::Inflected { .. }
        ));
    }

    /// The personal pronouns' accusatives (`hän` -> `hänet`,
    /// `minä` -> `minut`, `me` -> `meidät`) are in the stem map.
    #[test]
    fn fast_variant_accepts_pronoun_accusatives() {
        for word in ["hänet", "minut", "meidät", "teidät", "heidät"] {
            assert!(
                !matches!(check_normal_fast(word, false), NormalizedOutcome::Unknown),
                "{word} not accepted by the fast path"
            );
        }
    }

    /// A compound the gate-1 stripper knows: the head is real, so the
    /// fast path accepts it. `kirjoituspöydän` is `kirjoitus` + `pöytä`.
    #[test]
    fn fast_variant_accepts_compounds() {
        for word in ["kirjoituspöydän", "metsäherra", "työpöydän"] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted by the fast path"
            );
        }
    }

    /// A three-part compound is a modifier that is itself a compound:
    /// `salakirjevaihdossa` is `sala` + `kirje` + `vaihto` and
    /// `henkivartioväki` is `henki` + `vartio` + `väki`. Neither side
    /// is a sanalista lemma, so the fast path has to recurse on the
    /// modifier before it can accept the word.
    #[test]
    fn fast_variant_accepts_three_part_compounds() {
        for word in [
            "salakirjevaihto",
            "salakirjevaihdossa",
            "henkivartioväki",
            "henkivartioväen",
        ] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted by the fast path"
            );
        }
    }

    /// `tehtäneen` (the passive potential of `tehdä`) is a reading the
    /// stem maps do not carry, so only the head/tail/prefix walk (the
    /// engine gate) reaches it: `check_normal_fast` reports unknown while
    /// `check_normal` finds it. It is the precision the wasm trades away
    /// for bounded per-call cost, and the reason the two variants exist.
    #[test]
    fn fast_variant_skips_the_engine_gate() {
        assert_eq!(
            check_normal_fast("tehtäneen", false),
            NormalizedOutcome::Unknown
        );
        assert!(matches!(
            check_normal("tehtäneen", false),
            NormalizedOutcome::Inflected { .. }
        ));
    }

    /// Non-finite forms (participles, infinitives) decline as nominals, so
    /// their oblique cases are reached by peeling a *case* ending against
    /// the verb stem map (`lukeneen`, `lukeneita`, `kukistumiseen`). The
    /// builder emits those case-stems and gate 4b strips the nominal
    /// `ENDINGS` list. Before that, these were `Unknown` and some
    /// mis-split (`kukistumiseen` -> `mi`, `lukeneen` -> `kuka`).
    #[test]
    fn fast_variant_accepts_non_finite_obliques() {
        for (word, base) in [
            ("lukenut", "lukea"),
            ("lukeneen", "lukea"),
            ("lukeneita", "lukea"),
            ("tienneen", "tietää"),
            ("tienneitä", "tietää"),
            ("päässeet", "päästä"),
            ("saamiaan", "saada"),
            ("vihkiytyneen", "vihkiytyä"),
            ("ryhdyttäessä", "ryhtyä"),
            ("varttuneita", "varttua"),
            ("kunnostautuneille", "kunnostautua"),
            ("vastanneen", "vastata"),
            ("rakennetut", "rakentaa"),
            ("kukistumiseen", "kukistua"),
        ] {
            match check_normal_fast(word, false) {
                NormalizedOutcome::Inflected { base: got, .. } => {
                    assert_eq!(got, base, "{word}: wrong base");
                }
                other => panic!("{word}: expected Inflected, got {other:?}"),
            }
        }
    }

    /// The suppletive nominative plurals (`jotka` of `joka`, `ketkä` of
    /// `kuka`, `mitkä` of `mikä`) do not end in `-t`, so no case ending
    /// strips off them. The noun builder emits the whole form as a key
    /// (its nominative fallback) and the empty-ending probe reaches it.
    #[test]
    fn fast_variant_accepts_suppletive_nominative_plurals() {
        for (word, base) in [("jotka", "joka"), ("ketkä", "kuka"), ("mitkä", "mikä")] {
            match check_normal_fast(word, false) {
                NormalizedOutcome::Inflected { base: got, .. } => {
                    assert_eq!(got, base, "{word}: wrong base");
                }
                other => panic!("{word}: expected Inflected, got {other:?}"),
            }
        }
    }

    /// The reported reading words that the class and derivation fixes now
    /// cover. This is the first-pass gate the wasm render uses, so a
    /// regression here is directly user-visible.
    #[test]
    fn fast_variant_accepts_the_reported_missing_words() {
        for (word, base) in [
            ("ilkialastomina", "ilkialaston"),
            ("alastomia", "alaston"),
            ("kristinuskon", "usko"),
            ("liplatteli", "liplatella"),
            ("töräytytteli", "töräytytellä"),
            ("aikaansaamiseksi", "aikaansaaminen"),
            ("kaihotonta", "kaihoton"),
            ("puvuton", "puvuton"),
            ("eteemme", "eteen"),
            ("uupuneemmaksi", "uupua"),
            ("hoputtelemaan", "hoputella"),
            ("savupatsasten", "savupatsas"),
            ("valjaisiin", "valjaat"),
            ("ryntääjät", "ryntääjä"),
            ("livahtelivat", "livahtella"),
            ("vihollispuolen", "puoli"),
            ("enemmittä", "enempi"),
            ("kotiportilleen", "portti"),
            ("hörppäillä", "hörppäillä"),
            ("ikeniinsä", "ien"),
            ("parhaat", "paras"),
            ("antimia", "antimet"),
            ("huostaansa", "huosta"),
            ("ohimennessään", "mennä"),
            ("käräjiä", "käräjät"),
            ("käräjätuvissa", "tupa"),
            ("toimeenpanijat", "panija"),
            ("keskenämme", "kesken"),
            ("lasittomat", "lasiton"),
            ("pihteihin", "pihdit"),
            ("jauhoitus", "jauhoitus"),
            ("varjottomaan", "varjoton"),
            ("teatterikuolemaa", "kuolema"),
            ("kyynelettömät", "kyyneletön"),
            ("saunoitti", "saunoittaa"),
            ("talttumuksella", "talttumus"),
            ("itsekunkin", "itse"),
            ("vankkurien", "vankkurit"),
            ("kimotusta", "kimotus"),
            ("elttaantuneen", "elttaantua"),
            ("palmikoitsivat", "palmikoitsia"),
            ("katajankaarekselle", "kaares"),
            ("torkuksiin", "torkkua"),
            ("puolitorkuksiin", "torkkua"),
            ("toiviomatkan", "matka"),
            ("toiviomatkalla", "matka"),
            ("toivioretkellä", "retki"),
            ("toivioretkiä", "retki"),
            ("synnyinseudullaan", "seutu"),
            ("muistotonta", "muistoton"),
            ("sammalpäisten", "sammalpäinen"),
            ("pengermittäin", "pengertää"),
            ("parhaan", "paras"),
            ("purkautumismuotoja", "muoto"),
            ("rakastettavaisuus", "rakastettavaisuus"),
            ("rakastettavaisuutta", "rakastettavaisuus"),
            ("Väinämöinen", "väinämöinen"),
            ("saksilla", "sakset"),
            ("tungeskeleva", "tungeskella"),
            ("hölynpölyä", "pöly"),
            ("mitattomien", "mitaton"),
            ("mittaamattomien", "mittaamaton"),
            ("eteensä", "eteen"),
            ("tapasin", "tavata"),
            ("karitonta", "kariton"),
            ("siunatumpia", "siunata"),
            ("suurkaanin", "suurkaani"),
            ("helmettömiä", "helmetön"),
            ("sointuvinta", "sointua"),
            ("yllinkyllin", "kyllin"),
            ("tilkitsijän", "tilkitsijä"),
            ("haltuunsa", "haltuun"),
            ("viittovan", "viittoa"),
            ("erkanivat", "erkaantua"),
            ("tuontuosta", "tuosta"),
            ("tuontuostakin", "tuosta"),
            ("raamatunlauseet", "lause"),
            ("silloinkun", "silloin"),
            ("juhlatamineitaan", "tamineet"),
            ("lehdetön", "lehdetön"),
            ("kahdenkesken", "kesken"),
            ("lopettamaisillaan", "lopettaa"),
            ("naksahtelun", "naksahtelu"),
            ("äesteli", "äestellä"),
            ("aivotusten", "aivotus"),
            ("säilikön", "säilikkö"),
            ("tikkaiden", "tikkaat"),
            ("tikkaita", "tikkaat"),
            ("tikasten", "tikkaat"),
            ("edistyneemmät", "edistyä"),
            ("solvaajalle", "solvaaja"),
            ("tiuskasi", "tiuskata"),
            ("osaavampi", "osata"),
            ("ajattelemisaikaa", "aika"),
            ("narraaja", "narraaja"),
            ("palpattelisit", "palpatella"),
            ("lehditettynä", "lehdittää"),
            ("mairittelija", "mairittelija"),
            ("simasuu", "suu"),
            ("ruikuttelevaan", "ruikutella"),
            ("ohimennessä", "mennä"),
            ("morsiussuudelma", "suudelma"),
            ("äännekkäämmäksi", "äännekäs"),
            ("äännekäs", "äännekäs"),
        ] {
            match check_normal_fast(word, false) {
                NormalizedOutcome::Inflected { base: got, .. } => {
                    assert_eq!(got, base, "{word}: wrong base");
                }
                other => panic!("{word}: expected Inflected, got {other:?}"),
            }
        }
    }

    /// The `synnyinseutu` compound remains productive across the paradigm:
    /// the modifier `synnyin` is bound and the head is an ordinary class-1
    /// noun, so every case (including the rare abessive and possessive
    /// shapes) goes through the same compound path.
    #[test]
    fn fast_variant_accepts_the_synnyinseutu_paradigm() {
        for word in [
            "synnyinseudulla",
            "synnyinseudulta",
            "synnyinseudulle",
            "synnyinseudussa",
            "synnyinseudusta",
            "synnyinseuduitta",
            "synnyinseuduittaan",
            "synnyinseuduikseen",
        ] {
            assert!(
                !matches!(check_normal_fast(word, false), NormalizedOutcome::Unknown),
                "{word} not accepted by the fast path"
            );
        }
    }

    /// `länkisääri` is a classless compound whose modifier `länki` is a
    /// bound form. It is accepted; the named head is still the ordinary
    /// longest-modifier split, so only acceptance is asserted here.
    #[test]
    fn fast_variant_accepts_the_lanki_compound() {
        assert!(matches!(
            check_normal_fast("länkisääreksi", false),
            NormalizedOutcome::Inflected { .. }
        ));
    }

    /// The productive feminine `-tar`/`-tär` noun: `siivooja` ->
    /// `siivoojatar`, inflected as class 32*C. The lexicalised `valtiatar`
    /// is a lemma, and the misspelling `valtijatar` is not accepted.
    #[test]
    fn fast_variant_accepts_feminine_tar_nouns() {
        for word in ["siivoojatar", "siivoojattaren", "siivoojattarelle"] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted"
            );
        }
        assert!(matches!(check_normal_fast("valtiatar", false), NormalizedOutcome::Lemma));
        assert!(matches!(
            check_normal_fast("valtijatar", false),
            NormalizedOutcome::Unknown
        ));
    }

    /// `-llinen` derives from a stem, not always a lemma: `orsi` -> `orre`
    /// -> `orrellinen`. The noun stem map names the base.
    #[test]
    fn fast_variant_accepts_llinen_from_a_stem() {
        for word in ["orrellinen", "orrellisen", "orrellisella", "orrellisessa"] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted"
            );
        }
    }

    /// The DP compound segmenter handles long compounds without enumerating
    /// every split, and still rejects a doubled possessive.
    #[test]
    fn fast_variant_segments_long_compounds() {
        for word in [
            "avioliittoaikeita",
            "tietokoneohjelmointikurssi",
            "kotiportilleen",
        ] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted"
            );
        }
        assert!(matches!(
            check_normal_fast("todistelukykynsä", false),
            NormalizedOutcome::Inflected { .. }
        ));
        assert!(matches!(
            check_normal_fast("todistelukykynnsä", false),
            NormalizedOutcome::Unknown
        ));
    }

    /// The modifier of a `-minen` action noun is its `-mis-` stem. No lemma
    /// or inflected form spells `edistämis`, so without that stem rule the
    /// whole compound falls through to `Unknown`.
    #[test]
    fn fast_variant_accepts_minen_stem_compounds() {
        for word in ["edistämisvaroista", "edistämisohjelma", "edistämiskampanja"] {
            assert!(
                matches!(
                    check_normal_fast(word, false),
                    NormalizedOutcome::Inflected { .. }
                ),
                "{word} not accepted by the fast path"
            );
        }
    }

    /// Six- and seven-letter compounds are below the general compound
    /// probe's length floor; the cheap short probe has to accept them.
    #[test]
    fn fast_variant_accepts_short_compounds() {
        for (word, base) in [("esisana", "sana"), ("esiosa", "osa"), ("autotie", "tie")] {
            match check_normal_fast(word, false) {
                NormalizedOutcome::Inflected { base: got, .. } => {
                    assert_eq!(got, base, "{word}: wrong base");
                }
                other => panic!("{word}: expected Inflected, got {other:?}"),
            }
        }
        // A function-word modifier is not a compound modifier.
        assert!(matches!(
            check_normal_fast("jatalo", false),
            NormalizedOutcome::Unknown
        ));
    }

    /// A derived `-inen` adjective (`tahkoinen` of the noun `tahko`,
    /// `derivation`'s `ADJ_INEN`) is not a sanalista lemma, so neither
    /// the stem maps nor the form set carry it. The derivation layer
    /// reconstructs the nominative and inflects it as class 38; a
    /// compound head of that shape (`kolmitahkoinen` = `kolmi` +
    /// `tahkoinen`) works through the split.
    #[test]
    fn fast_variant_accepts_derived_inen_adjectives() {
        for (word, base) in [
            ("tahkoisella", "tahkoinen"),
            ("kolmitahkoisella", "kolmitahkoinen"),
            ("kolmitahkoinen", "kolmitahkoinen"),
            ("vastenmielisiä", "vastenmielinen"),
            ("henkiruumiisen", "henkiruumiinen"),
        ] {
            match check_normal_fast(word, false) {
                NormalizedOutcome::Inflected { base: got, .. } => {
                    assert_eq!(got, base, "{word}: wrong base");
                }
                other => panic!("{word}: expected Inflected, got {other:?}"),
            }
        }
    }
}
