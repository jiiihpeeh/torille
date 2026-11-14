# PLAN_PARALLEL_MORPHOLOGY — a morphology-only worker pool

Status: **design + native prototype** · 2026-11-14 · parent:
[`PLAN_GRAMMAR_STYLE_PERF.md`](PLAN_GRAMMAR_STYLE_PERF.md) Phase 5/6 ·
related: [`PLAN_COLD_SEARCH.md`](PLAN_COLD_SEARCH.md)

## 1. Why this is different from the rejected pool

The earlier parallel probe ran the **whole** grammar/style check per sentence
chunk. Each worker built its own cold `MorphologyCache`, lost the
cross-sentence reuse, and produced diagnostics for a chunk rather than for the
document. Eight workers reached 1.23 s wall time but still missed 500 ms.

This design keeps the expensive phase separate:

1. the pool resolves **morphology only** for disjoint groups of surfaces;
2. the owning state collects the `WordInfo` rows;
3. syntax and style run **once** over the full token stream and the merged
   `MorphologyAnalysis`.

The full-document passes are ~7 ms warm, so they do not need a pool. The
cold cost is the per-surface paradigm walk; that is what the pool splits.

## 2. Two granularities

### A. Surface-partitioned morphology (prototype first)

- The main thread tokenises, collects the unique lowercased surfaces, and
  groups them by a cheap key.
- Each worker owns a `MorphologyCache` and resolves its group through
  `MorphologyCache::resolve`.
- The worker returns `surface -> WordInfo` (native prototype: `Arc<WordInfo>`).
- The main thread merges the rows, applies the proper-name overlay and the
  numeral-governed partitive upgrade, and runs syntax/style.

Group by the **first four characters** of the surface, then split the sorted
list into balanced groups. Case forms of one lemma share a prefix, so most of
a lemma's surfaces land in the same worker and the thread-local `emits_form`
cache keeps its reuse. This is a heuristic; the prototype measures the loss.

**Advantages:** simple, no changes to the resolver, and the merge is a
surface→`WordInfo` map. **Cost:** the workers cannot share the main thread's
`emits_form` cache, and a wasm implementation needs a compact `WordInfo`
codec.

### B. Candidate-verification pool (only if A is not enough)

- The main thread does the cheap stem-map candidate generation and collects
  `(lemma, inflections, pos, possessives, surface)` verification tasks.
- Workers run `emits_form`/`form_readings` on batches and return the matching
  readings.
- The main thread assembles the `WordInfo` with its own cache.

This preserves the main cache and avoids serialising `WordInfo`, but it
requires splitting `find_base_via_stem_map` into a batchable task list. It is
a larger refactor and should follow a positive A measurement.

## 3. Native prototype

Add `--threads N` to `tools/src/bin/perf_grammar_style.rs`:

- tokenise once;
- collect unique lowercased surfaces in first-occurrence order;
- group by the first four characters, split into `N` balanced groups;
- spawn `N` threads, each with a fresh `MorphologyCache`, resolving its group;
- merge `surface -> Arc<WordInfo>`;
- build the token vector, apply the proper-name and numeral upgrades, compute
  sentence spans globally;
- time morphology-only and the full syntax/style pass.

The single-thread `--threads 1` path must match the existing
`analyze_morphology` result exactly, including the depth counts and the
diagnostics. The exact-output fixtures are the correctness gate.

### Measured (2026-11-14)

Native, 1,959-word fixture, morphology only:

| threads | morphology | grammar/style vs single-thread |
|---:|---:|---|
| 1 | 1.71 s | identical |
| 2 | 0.97 s | identical |
| 4 | 0.73 s | identical |
| 8 | 0.67 s | identical |

Depth counts match exactly (`direct=343 stem_map=704 ... unknown=53`); the
native `deep_upgrades` counter is 0 in the parallel path because the numeral
upgrade uses a separate cache, but the diagnostics are identical.

Wasm morphology-only probes (same fixture, unique surfaces):

| split | 2 workers | 4 workers | 8 workers |
|---|---:|---:|---:|
| static prefix groups | 2.64 s | 2.16 s | 1.83 s |
| round-robin groups | 2.62 s | 1.41 s | 1.45 s |
| dynamic queue, persistent state cache | 1.89 s | 1.36 s | **1.21 s** |

The static prefix split is limited by one expensive group; round-robin
balances better but loses lemma reuse. The dynamic queue with a persistent
per-worker `MorphologyCache` is the best shape. The **shipped wasm default is
4 workers**: 1.36 s morphology-only, ~1.5 s with syntax/style. Eight workers
reach 1.21 s, but the extra 0.15 s doubles the wasm instances (each ~28 MB
raw) and is not worth the memory. The pool would roughly halve the current
~2.8 s wasm cold page, not reach 500 ms.

The native 4/8-thread result is below the plan's 1 s go/no-go line, so the
pool is technically viable. Native callers default to
`min(available_parallelism, 8)`; the wasm path defaults to 4 and is
configurable 1–4 in the extension settings.

### Acceptance

- `--threads 1` diagnostics are byte-identical to the current pass.
- `--threads 2/4/8` diagnostics are byte-identical to `--threads 1`.
- Morphology-only wall time scales materially: 4 threads should be well under
  1 s native if the pool is worth porting.
- Warm all stays ≤ 10 ms.
- If the scaling plateaus above ~1 s native, stop; the wasm pool will not
  reach 500 ms.

## 4. Wasm design (after a positive prototype)

The pool cannot share `Arc<WordInfo>` across workers. Use a compact codec:

- worker export `analyze_morphology_chunk(text) -> Uint8Array` returns a
  length-prefixed list of `surface`, lemma(s), reading bytes, flags, clitic,
  possessive, derivation code and depth;
- the main state decodes it into `WordInfo` and stores it in its own
  `MorphologyCache` (or a collected map);
- a new export `check_from_morphology(idx, text, morphology, syntax, style)`
  runs the full-document passes over the merged rows;
- the existing `check_grammar_style_with_state` stays as the single-worker
  fallback.

The codec is the main risk. Measure its encode/decode cost separately; if it
exceeds the morphology saving, do not ship it.

## 5. Worker/state protocol

- The state owns the pool and the generation token.
- An edit bumps the generation; stale chunk results are dropped.
- The state posts `morphology-chunk` messages with a chunk id and surface
  group, and collects `morphology-result` messages.
- Progress is `(done, total)` over surface groups, so the existing progress
  toast works unchanged.
- A chunk result must carry the token range or surface keys so the state can
  reject a stale generation before merging.
- Pool size policy:
  - **wasm**: default **4**, capped at 4 (each instance is ~28 MB raw);
  - **native/CLI**: default `min(available_parallelism, 8)`;
  - **extension setting** `grammarThreads`: **1–4**, default 4.
  `1` uses the existing single-worker path; the demo defaults to 4 and can
  expose the same control.
- The setting is read at pool creation and on `storage.onChanged`; changing it
  drains the old pool and creates the new one, so no request sees a mixed
  pool size.

## 6. Risks

| risk | guard |
|---|---|
| prefix grouping loses `emits_form` reuse | measure threads 1 vs 2/4/8; if the total work grows, try candidate-verification or lemma grouping |
| wasm codec cost erases the saving | encode/decode benchmark before shipping |
| worker memory | each wasm instance is ~28 MB raw; cap the pool at 4 and let the extension setting lower it |
| stale chunk results | generation token on every message |
| chunk boundary changes the numeral upgrade | apply the upgrade after the merge on the main thread |
| proper-name overlay depends on capitalization | apply it after the merge, as the current `analyze_morphology` does |
| diagnostics drift | exact-output fixtures plus a page-wide diff against the single-thread pass |
| warm path regresses | the state keeps the merged rows and the existing sentence cache |

## 7. Phases

0. **Native prototype** — `--threads N`, morphology-only and full timings,
   diagnostics identical. This is the go/no-go gate.
1. **Wasm codec** — encode/decode `WordInfo`; measure the codec alone.
2. **Pool + state merge** — one worker pool in the demo, then the extension;
   progress and cancellation. The extension setting `grammarThreads` (1–4)
   selects the pool size.
3. **Reader/extension e2e** — the existing `probe-grammar-chunk` /
   `probe-reader-grammar` flows plus a browser-level extension test.

Do not add a reverse index or a new data blob for this work.
