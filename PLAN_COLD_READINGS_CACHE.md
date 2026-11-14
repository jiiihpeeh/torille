# PLAN_COLD_READINGS_CACHE — reuse a lemma's generated forms across its surfaces

Status: **cache experiment negative; stem-map-first reorder landed (partial win)** · 2026-11-14 · parent:
[`PLAN_GRAMMAR_STYLE_PERF.md`](PLAN_GRAMMAR_STYLE_PERF.md) Phase 2/6 ·
related: [`PLAN_COLD_SEARCH.md`](PLAN_COLD_SEARCH.md)

## 0. Result

The separate per-lemma `form_readings` cache was implemented, measured and
reverted. It made the cold page **worse**:

- 267 hits, 1,101 misses, 128,383 cached forms on the 1,959-word page;
- cold morphology **2.4 s → 2.9 s**.

The hit rate was too low because the cache key is per
lemma/inflections/pos/possessive, and the page has many distinct keys; the
miss path also cloned every generated form into the cache, which the old
`form_readings` did not do.

Temporary instrumentation of the cold pass (representative run, 2.78 s):

| component | calls | time |
|---|---:|---:|
| `form_readings` | 1,368 | 438 ms |
| `cheap_base` (`check_normal_fast`) | 959 | 906 ms |
| deep walk (`resolve_word_slow`) | 12 | 528 ms |

The rest is the direct path, stem-map base collection, possessive/proper-name
handling and allocation. The important finding is the **double walk**: the
stem-map gate builds a paradigm word set through `emits_form`, then
`form_readings` walks the same paradigm again for the readings. Sharing those
two walks is the next candidate; it can remove the ~400 ms `form_readings`
half, but the first walk per lemma (~800 ms) remains. The 12 deep walks are
~528 ms for a handful of rare words.

**Conclusion:** a separate form cache is not the answer. Even a shared
`emits_form`/`form_readings` cache leaves the first-walk cost and the deep
walks, so the 500 ms target is unlikely without a precomputed index — and
Phase 6 closed that route. Record this as a negative experiment; do not ship
the cache.

## 0b. Reorder experiment — landed (partial win)

The profile showed `cheap_base` being paid for every non-direct word even when
the stem map would resolve it. The shallow resolver now tries the stem map
first for a plain word and uses `cheap_base` only for:

- a surface that carries a possessive/clitic suffix;
- a classless compound or closed-class pronoun when the stem map names no
  matching base.

Cold morphology on the 1,959-word page: **~2.4–2.6 s → 2.14 s**. Depth counts
move from 701 stem-map / 56 unknown to **704 stem-map / 53 unknown**; the
three extra words are the classless/closed-class fallback. The full workspace,
catalogue and exact-output syntax/style tests are green.

This is a partial win, not the target. The remaining cold cost is
`form_readings`, the handful of deep walks, and the direct path; reaching
500 ms still needs a different cold-path design.

## 1. Problem

The cold 1,959-word page is **~2.1 s native / ~3.1 s wasm** after the reorder; the target is
**≤500 ms**. The Phase 6 `form → lemma` index was measured and rejected
(86.6% coverage at 2.1 MB; 95% coverage would exceed 3 MB).

The remaining cost is not the base search. It is **paradigm generation for
readings**: `form_readings` walks a lemma's whole paradigm for every surface
that belongs to it. A page has 1,146 unique surfaces but only a few hundred
distinct lemmas; `olla` alone appears 69 times. The same paradigm is
generated once per surface instead of once per lemma.

The existing `emits_form` cache stores a word set for base detection. It does
not store the readings, so `form_readings` cannot use it.

## 2. Design (tried, reverted)

Add a bounded **per-lemma generated-form cache** used by the morphology
reader:

- Key: `(lemma, inflections, pos, possessives, lexicon.is_some())`.
- Value: `Rc<Vec<Form>>` (or `Arc` if it must cross threads), built with the
  same `walk_generated`/`inflect_all` machinery.
- `form_readings` first probes the cache; on a hit it filters the cached
  forms for `form.word == surface` and unions their readings. On a miss it
  builds the cache entry once and then filters.
- Cap by total cached forms (start at 200,000) and clear wholesale on
  overflow, matching the `emits_form` cache policy. Expose hits/misses/forms
  for `perf_grammar_style`.
- Thread-local first (the wasm has one thread). A per-`MorphologyCache`
  version is a later refinement if the thread-local lifetime is a problem.

The cache is a pure memoisation of a deterministic walk. It cannot change a
reading, a diagnostic or a correction.

## 3. Measurements

Run:

```bash
cargo run --release -p torille-tools --bin perf_grammar_style -- \
  --words 0 --repeat 10 --single-thread
```

Record cold morphology, cold depth counts, form-cache hits/misses/forms, and
the wasm cold page. Acceptance for this experiment:

- exact-output syntax/style fixtures and the full workspace tests stay green;
- cold morphology drops materially (target ≤500 ms native);
- the warm path does not regress (warm all stays ≤10 ms);
- the wasm download is unchanged (no new blob).

If the cache alone does not reach 500 ms, measure the residual: the next
candidates are a per-lemma reading cache that stores only the surface→tags
rows (smaller than full `Form`s), or a bounded index of the page's most
frequent lemmas. Do not add a full reverse index; Phase 6 closed that route.

## 4. Risks

| risk | guard |
|---|---|
| a cached form is reused under a different inflection/pos | key includes every input that changes the walk |
| the cache grows the wasm heap | cap by forms, clear on overflow, expose stats |
| a reading is lost on a hit | exact-output fixtures + `form_readings` tests |
| thread-local cache leaks across textareas | bounded; clear hook if the lifetime matters |
| warm path regresses | warm-all gate in the harness |
