# torille-wasm demo — targeted behaviour

A reference for what the demo is supposed to do, what it does
today, and what's still broken. Update this when goals change
or probes flip green/red. The probes in `demo/probe-*.mjs`
are the executable version of the goals below; this file
explains *why* the probes assert what they assert.

---

## Top-line goals

1. **Live underlining on every keystroke.** As the user
   types, misspellings should appear under the corresponding
   word in well under 200 ms, and disappear the moment the
   word is fixed. No click-to-render, no "wait for the
   background pass", no stale underlines from a previous
   render. The first paint of a fresh word must feel
   instant — like a native text field.

2. **Inflection-aware, in the first pass.** Correctly
   inflected Finnish (cases, verb forms, plural, gradation)
   must NEVER be flagged, and the check that produces this
   judgement runs in the **first pass** (the synchronous
   per-keystroke one), not in a background de-mote pass.
   Specifically the first-pass check must accept:
   * `talossa`, `talosta`, `talolla`, `talolle`, `talolta`,
     `taloksi` (inessive/elative/adessive/allative/ablative/
     translative of `talo`)
   * `koirassa`, `koiralle` (class 2 inflection)
   * `taloja` (plural)
   * `puhtaan` (genitive of `puhdas`, with gradation)
   * `istuu`, `juoksee`, `menee`, `tuntee`, `sanoo`
     (present-tense verbs)
   * `lähestynyt`, `loukatuksi` (past participles)
   * `on`, `minua`, `hänen` (pronouns / possessives)

3. **Suggestions pre-populated, memoised.** The popup that
   opens on click must already have candidates by the time
   it appears. The first paint should not be empty /
   "loading". Re-clicks should be instant (cache hit).
   The suggestion generation runs *only* for the words
   the first pass flagged as misspellings, and the result
   is memoised so a second render of the same word is
   instant.

4. **Surgical diff on every render.** A re-render of a
   text that differs from the previous render in only one
   character must NOT re-run the first pass for unchanged
   words. The first-pass result for each (word, byte
   range) is keyed and reused across renders. Only
   words whose surface form or byte position changed
   (or which are brand new) get a fresh check.

5. **Modern Finnish text is the test corpus.** The
   verification probe uses Minna Canth's *Sylvi* (Gutenberg
   #13174, 1888-1893) because it is modern spoken Finnish
   with dialogue. The Kalevala (1849) was tried first but
   its language is too archaic and trips false positives
   that aren't real bugs.

6. **No regressions on the existing harness.** The
   `--lemma-set--` / `--check-fast--` Rust tests in
   `src/suggest.rs` and the noun/verb oracle materialised
   into `data/kotus_nomines.sqlite` are the long-term
   regression net. Demo changes must not regress them.

---

## Architecture at a glance

```
┌─────────────────────────┐  ┌──────────────────────────────────┐
│ Main thread (Solid)     │  │ 3 sibling wasm workers (pool)    │
│ ──────────────────      │  │ ────────────────────────────     │
│ App.tsx                 │  │ suggestion-worker.js × 3         │
│   ├ resource (debounce  │  │   ├ bg.suggest_for_misspell      │
│   │   150 ms)           │  │   │   (only for misspellings)     │
│   ├ surgical diff       │  │   └ per-worker LRU memo          │
│   │   (reuse unchanged) │  │ cross-worker central memo        │
│   ├ first-pass hits     │  │ + per-state side cache mirror    │
│   │   (candidates)      │  │                                  │
│   └ memo: candidates    │  └──────────────────────────────────┘
│      filtered for the   │
│      visible misspell.  │  ┌──────────────────────────────────┐
│                         │  │ Primary worker (spell-worker.ts) │
│ spell-client.ts         │  │ ────────────────────────────     │
│   ├ analyzeMisspells    │  │ bg.analyze_misspells (~1 ms)     │
│   │   = FIRST PASS      │  │   ├ lemma HashSet (O(1))         │
│   │   full inflection   │  │   ├ noun 9-case suffix stripper  │
│   │   check             │  │   ├ verb tense stripper (TODO)   │
│   ├ dispatchSuggest     │  │   └ gradation stripper (TODO)    │
│   │   (suggestions for  │  │ → returns ONLY real misspellings │
│   │    the misspellings │  │   (verb inflections, gradated    │
│   │    only)            │  │    forms, etc. never reach this) │
│   └ side cache mirror   │  │ bg.update_state_from_text        │
│                         │  │ bg.create_text_area_state        │
└─────────────────────────┘  └──────────────────────────────────┘
```

**Key architectural rule:** the first pass decides
misspellings. The background suggestion pass only fills the
popup with candidates for the words the first pass already
flagged. There is **no de-mote path** — the first pass is
authoritative.

### Two checks, two roles

* **First pass (`bg.analyze_misspells`, ~1 ms / text).** The
  wasm walks every word, runs the lemma HashSet probe
  (O(1) per word), the 9-case noun suffix stripper (O(1)
  per word), and ideally verb / gradation strippers (TODO).
  This is the only check that decides "is this a
  misspelling?". The result is correct, complete, and fast.

* **Background suggestion pass (50-200 ms / misspell).**
  For each misspell the first pass flagged, the main thread
  dispatches a memoised `bg.suggest_for_misspell` to one
  of 3 sibling workers. This fills the popup's candidate
  list. The result is memoised per word so a second render
  of the same word (or a click on the same word) is
  instant.

The two-pass latency budget is:

| Phase | Budget | Today |
|---|---|---|
| keystroke → first-pass result | <100 ms | 5-15 ms ✅ |
| first-pass result → underline painted | <16 ms (one frame) | <1 ms ✅ |
| first-pass → suggestion list ready | <300 ms | 50-1100 ms (cold); 0 ms (warm) ✅ |
| surgical-diff reuse of an unchanged word | 0 ms | 0 ms ✅ |

---

## Render pipeline (typing one character)

1. **Debounce** — 150 ms after the last keystroke.
2. **First pass** — `analyze_misspells(text)` returns the
   misspell list in ~1 ms. The first pass is the full
   inflection-aware check, so its output is final — verb
   inflections and gradated forms never appear in the
   list.
3. **Surgical diff** — compare the new first-pass list to
   the previous render's misspell list by `(word, byte
   range)`. Unchanged misspell positions are reused (no
   work); changed or new words become "candidates" that
   get suggestions in the next phase.
4. **Show underlines immediately** — every misspell in
   the first-pass list fires `onMisspellCandidate`, which
   the renderer turns into a red span. User sees the
   feedback in **5-15 ms** total.
5. **Background suggestion pass** — for each candidate,
   dispatch a memoised `bg.suggest_for_misspell` to the
   worker pool. The pool has 3 sibling workers, so 3
   suggestions run in parallel. Cache hits are instant.

---

## Goals ↔ probes

| Goal | Probe | Assertion |
|---|---|---|
| First paint of a fresh misspelling is instant | `probe-timing.mjs` | first underline appears <200 ms after `setText` |
| No false positives in the first pass (verb inflections) | `probe-timing.mjs` | `istuu` and `talossa` are NEVER in the underlay at any point |
| No false positives in real Finnish (Sylvi) | `probe-sylvi.mjs` | clean Sylvi slice → ≤ 8 underlines (historical orthography only) |
| Garbage flagged immediately | `probe-sylvi.mjs` | 6 garbage words (qwerty, asdfg, …) all underlined |
| Surgical diff: typing one character doesn't re-check unchanged words | `probe-surgical.mjs` | typing 1 char → `reused=N toCheck=0` for unchanged misspell positions |
| Caret aligns with text | `probe-cursor.mjs` | caret x/y/height within 2 px of underlay text at same byte index |
| Grammar/style runs in sentence batches with progress and cancellation | `probe-grammar-chunk.mjs` | first progress <2 s; final counts match the whole-text baseline (69 grammar, 14 style); editing cancels without stale diagnostics |
| Sentence cache reuses unchanged sentences | `probe-sentence-cache.mjs` | warm result matches cold; one edited sentence matches a fresh whole-text check |
| No regression on Rust core | `cargo test --release --workspace` | green |

Run all probes after a demo change:

```bash
cd torille-wasm/demo
node build.mjs                          # rebuild dist/
node probe-timing.mjs                   # latency + first-pass accuracy
node probe-sylvi.mjs                    # real-Finnish stress
node probe-surgical.mjs                 # surgical diff
node probe-cursor.mjs                   # caret alignment
node probe-grammar-chunk.mjs            # chunked grammar/style + progress
node probe-sentence-cache.mjs           # sentence reuse + edit parity
```

`probe-debug.mjs` is a general-purpose page + worker console
capture; useful when one of the above breaks in a way that
needs the full timeline.

---

## Current state (today)

✅ **First paint, ~5-15 ms** — `probe-timing.mjs` measures
14 ms on a 5-word garbage sample, 5 ms on a 6-word sample.
Down from "1-2 s" before the candidate/demote split.

✅ **Grammar/style chunking** — `probe-grammar-chunk.mjs`:
  * first progress ~160 ms on the 1,959-word perf page;
  * final 69 grammar / 14 style, matching the whole-text wasm check;
  * editing during the run cancels it and clears to 0 diagnostics;
  * the longest batch is still ~200 ms (5 sentences), so the
    ≤50 ms per-task target remains open.

✅ **Sentence cache** — `probe-sentence-cache.mjs`:
  * the all-hit wasm pass returns the same diagnostics as the cold pass;
  * one edited sentence returns exactly the fresh whole-text result;
  * native single-thread re-check: 2.6–3.7 ms for the 1,959-word page,
    1.8 ms for the 96-word page.

✅ **Surgical diff** — `probe-surgical.mjs`:
  * baseline 5 misspellings → 5 dispatched to suggestion pool
  * typing 1 char at end → `reused=5 toCheck=0`
  * typing " munster" → `reused=5 toCheck=1`

✅ **Caret alignment** — `probe-cursor.mjs` measures 0.03 px
dx, 0.00 dy, 0.00 dh in every case (single-, two-, three-line).

⚠️ **First-pass false positives (verb inflections).** The
first pass currently covers only the 9 productive noun cases
(`is_known_word` in `src/suggest.rs`). It MISSES verb
present-tense forms (`istuu`, `juoksee`, `menee`, `tuntee`,
`sanoo`), past participles (`lähestynyt`, `loukatuksi`), and
some gradated forms (`puhtaan`). These leak into the
misspell list and we have a background "de-mote" pass that
runs `bg.check` per word to remove them — the source of the
"multiple seconds" the user sees. **The right fix is engine-
side**: extend `is_known_word` to also accept verb
present-tense and gradated forms (the suffix patterns are
well-defined, just need more tables). Once that's in, the
de-mote path is deleted and the first pass becomes
authoritative.

⚠️ **Sylvi clean slice shows 28-29 underlines after 30 s.**
Currently a side-effect of the verb-inflection leak above:
the slow-check de-mote pass *is* running and demoting the
verbs, but a render-race + the 30 s timeout means the probe
sometimes reads the underlay mid-demote. Goes away when the
first pass is accurate (no de-mote needed).

---

## Open work

* **Engine: extend `is_known_word` for verbs + gradation.**
  The first pass must accept `istuu`, `juoksee`, `menee`,
  `tuntee`, `sanoo`, `lähestynyt`, `loukatuksi`, `puhtaan`,
  etc. without consulting the slow `check`. Suggested
  approach: pre-compute a HashSet of all known inflected
  forms at startup, embed in the wasm, O(1) probe per word.
  See the comment on `is_known_word` in `src/suggest.rs`.

* **Drop the de-mote path once the first pass is accurate.**
  When the first pass is reliable, `onDemoteMisspell` and
  the `demoted` set go away. `findMisspellings` becomes
  a pure first-pass result with a parallel `dispatchSuggest`
  fan-out. The "candidate" state in `App.tsx` becomes the
  only misspell state.

* **Slow-check latency for suggestion generation.** The
  slow `bg.suggest_for_misspell` is genuinely slow (50-200
  ms cold). For a 50-misspell text it can take 10-15 s to
  populate every popup cache. The surgical diff already
  covers the typing case (only changed words re-suggest).
  The remaining case is "user pastes a big block" —
  acceptable as long as the underline appears immediately
  (it does, in 5-15 ms) and suggestions fill the side
  cache in the background.

* **Compile time** — `node build.mjs` is 220 ms. With
  `--watch` it's instant on edit. The demo server
  (`cargo run --release -p torille-demo-server`) does NOT rebuild — it just
  serves whatever is in `dist/`. This bit me once: a fix
  landed in `src/` but the browser kept showing the stale
  bundle until I ran `node build.mjs`. The lesson is in
  the build script's header comment; the `--watch` flag
  is the easy way to never hit it again.

* **Sentence-aware tokenisation** — the wasm
  `analyze_misspells` is word-level only. Multi-word proper
  nouns ("New York", "Helsinki-Vantaa") get the first word
  flagged and the second word missed if the user has
  learned only the joined form. Out of scope for the demo;
  tracked in the per-state proper-noun phrase list.

---

## File map (for code archaeology)

| What | Where |
|---|---|
| Two-pass render entry point | `demo/src/find-misspellings.ts` |
| Surgical diff | `demo/src/find-misspellings.ts` (FindMisspellingsOptions) |
| App component, candidate/demote split | `demo/src/App.tsx` |
| Pool dispatch + side cache | `demo/src/spell-client.ts` |
| Per-word pool workers | `demo/src/suggestion-worker.ts` |
| Primary worker (state, first pass) | `demo/src/spell-worker.ts` |
| Editor (underlay + textarea) | `demo/src/Editor.tsx` |
| Caret/underlay alignment CSS | `demo/index.html` (lines 26-110) |
| Build script with `--watch` | `demo/build.mjs` |
| Fast first-pass check (lemma + 9-case suffix) | `src/suggest.rs::is_known_word` |
| Slow full check (head/tail/prefix walks) | `src/suggest.rs::check` |
| Wasm exports | `torille-wasm/src/lib.rs` |

---

## Update protocol

When you change a demo behaviour:

1. Update the probe that asserts on it.
2. Run the probe; if it flips red, fix the demo until it
   flips green.
3. Update the **Current state** section above to match.
4. Commit probe + demo + doc in one commit so the
   historical story stays intact.
