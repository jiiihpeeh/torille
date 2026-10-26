//! WASM bindings for the torille Finnish spell-checker.
//!
//! This crate is the `wasm-pack` target: `wasm-pack build --release
//! --target web` produces `pkg/torille_check_bg.wasm` plus a tiny
//! JavaScript wrapper that calls it. The wasm module embeds the
//! sanalista and paradigm catalogue as static strings, so it is
//! self-contained -- no fetch, no JSON, no setup.
//!
//! Two entry points, both shaped like hunspell's:
//!
//! - [`check`] is `hunspell_spell` -- true if the word is known, false
//!   otherwise. The `wasm-bindgen` boundary makes this a JavaScript
//!   function that returns a `boolean`.
//! - [`suggest_words`] is `hunspell_suggest` -- up to `max_n`
//!   candidates, ranked by edit distance. Returns a JavaScript
//!   `Array<string>`.
//!
//! ## Why this layer exists
//!
//! The browser sandbox forbids the runtime file I/O and threading
//! that `torille` itself does at startup, so a direct port would
//! compile but fail at instantiation. wasm-bindgen is the small set
//! of concessions that matters: a self-contained module Firefox (and
//! any other wasm host) can load in one network round-trip.
//!
//! ## Allocator
//!
//! The default is Rust's standard wasm allocator (dlmalloc), which
//! reuses freed blocks. The `wee-alloc` cargo feature switches to
//! `wee_alloc` for a smaller binary; it is **off by default** because
//! `wee_alloc` is unmaintained and does not reuse freed memory -- the
//! long-lived worker pool's linear memory then grows by roughly the
//! per-call peak on every `check`, which OOMs the tab after a few
//! hundred unknown words (the Gutenberg reader hit exactly this).
//!
//! ## Limitations
//!
//! - The wasm module carries the 11 MB sanalista. After `wasm-pack`'s
//!   `opt-level = "z"` + LTO + gzip, the produced `.wasm` is roughly
//!   3-4 MB. Acceptable for a Firefox extension; not what you'd ship
//!   to a slow network.
//! - `check` and `suggest_words` are synchronous on the wasm side.
//!   Firefox runs wasm on the main thread, so a long check blocks
//!   the UI. The caller (the content script in
//!   `torille-firefox/`) is responsible for debouncing.
//! - Compound analysis is not wired in -- same as the Rust
//!   `torille::suggest::check`. A `nuori-isäntä`-shaped word that has
//!   no single-lemma source returns `false`. The Firefox extension's
//!   fallback path is to ask the daemon over the Unix socket.

use wasm_bindgen::prelude::*;

mod state;

use crate::state::{Hit, TextAreaState};

// Allocator: see the module doc. `wee_alloc` only under the opt-in
// `wee-alloc` feature; the default is Rust's standard wasm allocator,
// which reuses freed memory (the worker pool makes hundreds of
// allocation-heavy `check` calls, and `wee_alloc` grows the linear
// memory on every one of them).
#[cfg(all(target_arch = "wasm32", feature = "wee-alloc"))]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// We lowercase once at the wasm boundary so callers don't have to
// remember to do it themselves, and so the underlying
// `torille::suggest` checks against the lemma-cased form.
fn lowercase(word: &str) -> String {
    word.to_lowercase()
}

/// True if `word` is a known Finnish word (lemma, inflection, or a
/// form the stem-map gates recognise).
///
/// Maps directly to `hunspell_spell`. JavaScript callers see a
/// `boolean`.
///
/// This is the slow-pass check for a candidate the first pass flagged:
/// the first pass already ran [`torille::expand::check_normal_fast`], so
/// this only asks the compound/derivation-aware
/// [`is_known_word_fast`] whether the surface is a real form. It
/// deliberately does not run the engine walk: on a long reader page an
/// engine call per unknown word spent seconds in the suggestion pool, and
/// the only thing it bought was a rarer form the fast detector missed.
#[wasm_bindgen]
pub fn check(word: &str) -> bool {
    let word = lowercase(word);
    torille::suggest::is_known_word_fast(&word).is_some()
}

/// Up to `max_n` spell-check suggestions for `word`, ranked by edit
/// distance. Empty when the word is unknown to the index and no
/// candidate is within distance 2.
///
/// Maps directly to `hunspell_suggest`. JavaScript callers see an
/// `Array<string>`.
#[wasm_bindgen]
pub fn suggest_words(word: &str, max_n: usize) -> js_sys::Array {
    let arr = js_sys::Array::new();
    for s in torille::suggest::suggest(&lowercase(word), max_n, /* max_distance = */ 2) {
        arr.push(&JsValue::from(s));
    }
    arr
}

/// Finnish hyphenation points for `word`.
///
/// Returns an `Array` of `{ byteOffset, charOffset, compound, source,
/// penalty }` objects. The default Finnish policy is used (two-character
/// minimum fragments, no inter-vowel breaks except compound boundaries
/// or explicit exceptions); `compound_breaks = false` gives the
/// pattern-only fallback. Existing soft hyphens are preserved and
/// returned with `source = "soft-hyphen"`.
///
/// This is a presentation helper. It deliberately does not run the full
/// reverse index, so a rare compound may fall back to its component
/// patterns rather than a morphology-recognised boundary.
#[wasm_bindgen]
pub fn hyphenate(
    word: &str,
    left_min: u32,
    right_min: u32,
    compound_breaks: bool,
) -> js_sys::Array {
    use torille::hyphenation::{
        Exceptions, HyphenationOptions, LexicalCompoundAnalyzer, NoCompoundAnalyzer, positions_with,
    };

    let options = HyphenationOptions {
        left_min: left_min as usize,
        right_min: right_min as usize,
        compound_breaks,
        ..HyphenationOptions::default()
    };
    let compound = LexicalCompoundAnalyzer;
    let no_compound = NoCompoundAnalyzer;
    let analyzer: &dyn torille::hyphenation::CompoundAnalyzer = if compound_breaks {
        &compound
    } else {
        &no_compound
    };
    let arr = js_sys::Array::new();
    for point in positions_with(word, &options, analyzer, Exceptions::finnish()) {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &obj,
            &"byteOffset".into(),
            &JsValue::from(point.byte_offset as u32),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &"charOffset".into(),
            &JsValue::from(point.char_offset as u32),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &"compound".into(),
            &JsValue::from(point.compound_boundary),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &"source".into(),
            &JsValue::from(point.source.as_str()),
        );
        let _ = js_sys::Reflect::set(&obj, &"penalty".into(), &JsValue::from(point.penalty));
        arr.push(&obj);
    }
    arr
}

/// One-shot: returns a JavaScript object that bundles a full spell-check
/// report for the caller. Useful when the caller wants everything in
/// one round-trip rather than two wasm boundary crossings.
///
/// The shape is `{ known: bool, lemmas: string[] }`. `lemmas` is
/// empty when `known` is `false`; otherwise it is the lemmas the
/// engine thinks `word` is a form of, in priority order.
#[wasm_bindgen]
pub fn check_report(word: &str) -> js_sys::Object {
    use js_sys::{Array, Object};
    use torille::suggest::CheckOutcome;
    let lower = lowercase(word);
    let outcome = torille::suggest::check(&lower);
    let obj = Object::new();
    let lemmas = Array::new();
    let known = !matches!(outcome, CheckOutcome::Unknown);
    js_sys::Reflect::set(&obj, &"known".into(), &JsValue::from(known)).unwrap();
    match outcome {
        CheckOutcome::Lemma => {
            // The lemma is the word itself, lowercased.
            lemmas.push(&JsValue::from(lower));
        }
        CheckOutcome::Inflected(ls) => {
            for l in ls {
                lemmas.push(&JsValue::from(l));
            }
        }
        CheckOutcome::Unknown => {}
    }
    js_sys::Reflect::set(&obj, &"lemmas".into(), &lemmas).unwrap();
    obj
}

/// Module init helper. Modern demos call `bg.default(bytes)`
/// directly; the `#[wasm_bindgen(start)]` attribute is left off
/// so the symbol `start` doesn't collide with a same-name export
/// from a downstream crate (e.g. `torille-wasm-leptos`).
#[allow(dead_code)]
pub fn init_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    {
        // Allow Rust `console.log!` etc. to print into the browser dev
        // tools console. The no-op branch on non-wasm keeps native
        // tests compilable.
        let _ = console_error_panic_hook::set_once();
    }
}

/// High-level, non-`wasm-bindgen` API for callers that compile
/// into the same wasm as the torille core (no postMessage layer).
/// The Leptos demo lives in `torille-wasm-leptos/` and imports
/// this module directly. Browser-extension callers use the
/// `wasm_bindgen`-exported functions above (per-textbox state
/// pool lives on the worker side); this module is for the
/// "single wasm, no worker" deployment shape.
///
/// The state pool is *not* used here -- the caller owns a
/// `TextAreaState` directly. That's the whole point: in the
/// same-wasm shape the per-overlay state lives at the same
/// Rust ownership level as the UI, so signals take its place.
pub mod wasm_demo {
    use super::state::TextAreaState;

    pub use crate::state::Hit;

    /// New, empty per-textarea state.
    pub fn new_state() -> TextAreaState {
        TextAreaState::new()
    }

    /// Run the full analyse pipeline on `text`, returning one
    /// [`Hit`] per misspelling (with byte ranges + suggestion
    /// lists). Cheap microseconds for texts under ~1 KB.
    pub fn analyze(state: &TextAreaState, text: &str, max: usize) -> Vec<Hit> {
        state.analyze(text, max)
    }

    /// Re-extract proper-noun and phrase candidates from
    /// `text`. Call this before [`analyze`] when the user
    /// has edited the text, so the wasm-side state matches
    /// what the renderer last saw.
    pub fn update(state: &mut TextAreaState, text: &str) {
        state.update(text);
    }

    /// Drop the slot -- e.g. on textarea close/replace.
    pub fn clear(state: &mut TextAreaState) {
        state.clear();
    }

    /// Render `word` with a visible hyphen at every permitted break.
    ///
    /// This is the display half of [`crate::hyphenate`]; callers that
    /// need offsets use the exported binding instead. Existing soft
    /// hyphens are shown as ordinary hyphens and removed from the
    /// display, because the UI string is not the source.
    pub fn hyphenate(word: &str) -> String {
        let points = torille::hyphenation::positions(word);
        torille::hyphenation::split(word, &points)
            .into_iter()
            .map(|part| part.replace('\u{00AD}', ""))
            .collect::<Vec<_>>()
            .join("-")
    }
}

// ─────────────────────────────────────────────────────────────────────
// Per-textarea state. One slot per live text input; the worker
// thread owns the data and the main thread holds a u32 handle.
// ─────────────────────────────────────────────────────────────────────

use std::sync::Mutex;

type StateSlot = Option<TextAreaState>;
type StateVec = Vec<StateSlot>;

/// Process-global pool of states. Mutex-protected so the static is
/// `Sync`; on a wasm single-thread the lock never actually
/// contends, but the Rust compiler doesn't know that.
fn states() -> &'static Mutex<StateVec> {
    static STATES: Mutex<StateVec> = Mutex::new(StateVec::new());
    &STATES
}

/// Allocate a slot for a fresh state. Reuses a `None` slot if
/// there is one; otherwise appends. Returns the slot index.
fn alloc_slot(states: &mut StateVec) -> u32 {
    if let Some((i, slot)) = states.iter_mut().enumerate().find(|(_, s)| s.is_none()) {
        *slot = Some(TextAreaState::new());
        i as u32
    } else {
        states.push(Some(TextAreaState::new()));
        (states.len() - 1) as u32
    }
}

/// Create a new per-textarea state. The returned `u32` is the
/// handle every other `..._with_state` function expects. The slot
/// lives until [`destroy_text_area_state`] is called.
#[wasm_bindgen]
pub fn create_text_area_state() -> u32 {
    let mut s = states().lock().expect("states mutex poisoned");
    alloc_slot(&mut s)
}

/// Drop the state at `idx`. Out-of-bounds / already-destroyed
/// indices are silently ignored -- callers may not know whether
/// the worker is on its first lifetime.
#[wasm_bindgen]
pub fn destroy_text_area_state(idx: u32) {
    let mut s = states().lock().expect("states mutex poisoned");
    if let Some(slot) = s.get_mut(idx as usize) {
        *slot = None;
    }
}

/// Wipe the learned values in the state at `idx` without
/// releasing the slot. Used when a textarea's contents are
/// replaced wholesale (e.g. the demo's "reset" button).
#[wasm_bindgen]
pub fn clear_state(idx: u32) {
    let mut s = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = s.get_mut(idx as usize) {
        state.clear();
    }
}

/// Re-extract proper nouns and phrases from `text` for the state
/// at `idx`. Out-of-bounds indices are a silent no-op.
///
/// The wasm side does the tokenisation; the main thread sends
/// the text exactly once per textarea update. The previous
/// learned values are *replaced*, never accumulated, so a word
/// the user deleted disappears from the state on the next
/// update.
#[wasm_bindgen]
pub fn update_state_from_text(idx: u32, text: &str) {
    let mut s = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = s.get_mut(idx as usize) {
        state.update(text);
    }
}

/// One misspell in the rendered text. The JS shape is
/// `{ word, start, end, suggestions }` -- a JS object whose
/// `suggestions` field is a JS array of ranked candidates.
fn hit_to_js(hit: &Hit) -> js_sys::Object {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &"word".into(), &JsValue::from(hit.word.as_str()));
    let _ = js_sys::Reflect::set(&obj, &"start".into(), &JsValue::from(hit.start as u32));
    let _ = js_sys::Reflect::set(&obj, &"end".into(), &JsValue::from(hit.end as u32));
    let suggestions = js_sys::Array::new();
    for s in &hit.suggestions {
        suggestions.push(&JsValue::from(s.as_str()));
    }
    let suggestions_js: JsValue = suggestions.into();
    let _ = js_sys::Reflect::set(&obj, &"suggestions".into(), suggestions_js.as_ref());
    obj
}

/// Spell-check the whole text in one wasm call. Walks the tokens,
/// short-circuits on phrase matches, drops lemma hits on the fast
/// path, falls back to the full engine only for the rare inflected
/// form, and returns every misspelling's byte range plus up to
/// `max_suggestions` per hit.
///
/// One string-to-wasm copy for the whole text, instead of one per
/// token. The 17th-call `unreachable` the per-word path hit goes
/// away because we never recurse through the wasm-bindgen bridge
/// inside an analyze_text call.
#[wasm_bindgen]
pub fn analyze_text(idx: u32, text: &str, max_suggestions: u32) -> js_sys::Array {
    let out = js_sys::Array::new();
    let s = states().lock().expect("states mutex poisoned");
    if let Some(Some(state)) = s.get(idx as usize) {
        for hit in state.analyze(text, max_suggestions as usize) {
            out.push(&hit_to_js(&hit));
        }
    }
    out
}

/// **First-pass** misspell detection. Returns just the byte
/// ranges; suggestions are filled in by [`suggest_for_misspell`]
/// on click.
///
/// Use this in the per-keystroke render path: every word is
/// checked against the lemma HashSet (O(1) each) and the learned
/// phrases/proper-nouns, so even a 1-KB text completes in
/// single-digit milliseconds instead of the multi-second cost
/// of running the inflection generator per token.
///
/// The JS-side caller should never call `analyze_text` *and*
/// `analyze_misspells` for the same render -- that would duplicate
/// work. `analyze_misspells` is the one to call for the demo's
/// "red underline first, suggestions on click" UX.
#[wasm_bindgen]
pub fn analyze_misspells(idx: u32, text: &str) -> js_sys::Array {
    let out = js_sys::Array::new();
    let s = states().lock().expect("states mutex poisoned");
    let Some(Some(state)) = s.get(idx as usize) else {
        return out;
    };
    for hit in state.find_misspells(text) {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&obj, &"word".into(), &JsValue::from(hit.word));
        let _ = js_sys::Reflect::set(&obj, &"start".into(), &JsValue::from(hit.start as u32));
        let _ = js_sys::Reflect::set(&obj, &"end".into(), &JsValue::from(hit.end as u32));
        out.push(&obj);
    }
    out
}

/// **Second-pass** suggestion builder. Bounded Levenshtein over
/// the embedded lemma list only; no paradigm generation, no
/// `find_bases_with`. Use this to fill the suggestion popup when
/// the user clicks a misspelling identified by
/// [`analyze_misspells`].
///
/// `max_distance` defaults to 2 in the worker; passing it
/// explicitly here lets the popup tighten to 1 for higher
/// precision when the user is editing a known word.
#[wasm_bindgen]
pub fn suggest_for_misspell(_idx: u32, word: &str, max_n: u32, max_distance: u32) -> js_sys::Array {
    let arr = js_sys::Array::new();
    for w in torille::suggest::suggest_over_lemmas(word, max_n as usize, max_distance as usize) {
        arr.push(&JsValue::from(w));
    }
    arr
}

/// Pre-warm the lazy statics the per-textbox render relies on.
/// Call this once at worker boot (right after `initTorilleWasm`
/// finishes) to pay the cold-start cost of `lemma_set()` and
/// `catalogue()` *before* the first user keystroke. The user
/// sees the first misspell highlight in single-digit
/// milliseconds instead of ~1s.
#[wasm_bindgen]
pub fn warm_caches() {
    torille::suggest::warm_caches();
}

/// Pre-warm the pre-computed inflected form set so the first-pass
/// `find_misspells` can recognise every form the engine can produce
/// (gradated nouns, verb persons, possessive chains, clitic stacks,
/// ...), not just the lemma + productive noun-case strip.
///
/// Walks every lemma in the sanalista and runs its full paradigm
/// once. On a desktop build this is a few seconds; on the wasm
/// it's a comparable single-digit-seconds pass. The work is
/// idempotent (`OnceLock`), so calling it twice is free. The
/// first-pass check reads [`form_set_ready`] and falls back to the
/// fast lemma + noun-case strip until this returns, so the typing
/// path is never blocked.
///
/// Since the form set is a compile-time binary blob (FNV-1a
/// open-addressing table embedded via `include_bytes!`), there is
/// nothing to warm -- the set is ready the moment the wasm module
/// is loaded. This function is kept for API compatibility with
/// callers that schedule the warmup; it now just reports the time
/// the no-op took, which is essentially zero.
#[wasm_bindgen]
pub fn warm_form_set() -> f64 {
    let t0 = js_sys::Date::now();
    // Touch the static so any DCE pass that would otherwise
    // eliminate the form-set linkage doesn't win.
    let _ = torille::expand::all_forms::form_set().len();
    js_sys::Date::now() - t0
}

/// Per-word spell suggestions that include the state-aware fallback
/// for proper nouns the user has typed. Used by the suggestion
/// popup so a click on `Helsnki` returns `["helsinki"]` when
/// `Helsinki` has been learned.
#[wasm_bindgen]
pub fn suggest_words_with_state(
    idx: u32,
    word: &str,
    max_n: u32,
    max_distance: u32,
) -> js_sys::Array {
    let arr = js_sys::Array::new();
    let s = states().lock().expect("states mutex poisoned");
    let Some(Some(state)) = s.get(idx as usize) else {
        return arr;
    };
    for w in torille::suggest::suggest_with_state(
        word,
        max_n as usize,
        max_distance as usize,
        &state.proper_nouns,
    ) {
        arr.push(&JsValue::from(w));
    }
    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wasm-only tests are guarded by `target_arch = "wasm32"`,
    /// because they touch `js_sys` types that are only meaningful in a
    /// wasm host. The pure-Rust `check` function can be tested on
    /// either target.
    #[test]
    fn known_word_returns_true() {
        assert!(check("talo"));
        assert!(check("taloa"));
        assert!(check("koiralta"));
        assert!(check("puhtaan")); // gradated, phase-3 of find_bases
        assert!(check("öihin")); // short lemma, phase-4
    }

    #[test]
    fn unknown_word_returns_false() {
        assert!(!check("xyzzy"));
        assert!(!check(""));
    }

    /// Returns the suggestions as a plain `Vec<String>` -- the same
    /// data the wasm side hands back as a `js_sys::Array`, just
    /// without the JavaScript bridge. Tests on the native target
    /// exercise the *logic*; the wasm-specific test then exercises
    /// the bridge.
    #[test]
    fn suggest_vec_round_trip() {
        let words: Vec<String> = torille::suggest::suggest("taloo", 5, 2)
            .into_iter()
            .collect();
        assert!(
            words.iter().any(|w| w == "talo"),
            "taloo should suggest talo: {words:?}"
        );
    }

    #[test]
    fn report_contains_the_lemma_for_a_finite_a_form() {
        // Drives `check_report` end-to-end on the native side. The
        // js_sys::Object path only runs under wasm; here we just
        // confirm the underlying decision is correct.
        assert!(check("taloa"));
        assert!(!check("xyzzy"));
    }

    #[test]
    fn the_demo_hyphenation_display_uses_the_engine_policy() {
        assert_eq!(
            wasm_demo::hyphenate("tietokoneohjelmointi"),
            "tie-to-ko-ne-oh-jel-moin-ti"
        );
        assert_eq!(wasm_demo::hyphenate("kissa\u{00AD}koira"), "kis-sa-koi-ra");
        assert_eq!(wasm_demo::hyphenate("asia"), "asia");
    }
}

// The console-error-panic-hook dependency is only useful on wasm; we
// keep it in scope conditionally to avoid dragging in std-only deps
// for the native tests.
//
// wasm-bindgen's default project template includes this hook so
// `unwrap` panics land in the browser dev console instead of dying
// silently.
#[cfg(target_arch = "wasm32")]
extern crate console_error_panic_hook;
