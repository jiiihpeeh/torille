# Benchmarking

`tools/bench.py` is the suite driver. It does not measure anything itself: it
runs the release binaries, normalises their JSON into one versioned report,
fingerprints the corpora and the correctness output, and compares two reports
from the same machine. The measurement primitives are the Criterion bench
(`benches/torille.rs`) and the `perf_*` tools in `tools/src/bin/`.

The staged design is in [`../misc_md/PLAN_BENCHMARK_SUITE.md`](../misc_md/PLAN_BENCHMARK_SUITE.md).

## Quick start

```bash
python3 tools/bench.py corpora --check       # generate/verify the corpus set
python3 tools/bench.py run --profile quick   # ~1 minute, one run per point
python3 tools/bench.py run --profile full    # 7 fresh processes per point
python3 tools/bench.py compare bench/baselines/reference.json reports/bench-full.json
```

`record` copies a report into `bench/baselines/`:

```bash
python3 tools/bench.py record reports/bench-full.json --name reference
```

`full` runs every corpus, both the single-thread and 4-worker shapes, and the
reverse-index build. It does not run Criterion unless `--criterion` is passed.
`--taskset 0-3` pins every command (and records the mask).

## Corpora

`bench/corpora.json` is the manifest: path, word/byte count and SHA-256 for
every corpus, plus the generator seed for the derived ones.

| name | what it is |
|---|---|
| `page-2000` | the clean 1,959-word shipping page |
| `homonyms` | the reading-collision corpus (`tuli`, `kuusi`, `sata`, …) |
| `line-wrap` | the sentence-cache parity contexts |
| `page-errors-8pct` | ~8 % of the unique words with one vowel substituted |
| `page-errors-x` | every word with an appended `x` (all unknown) |

The derived corpora are produced by `tools/gen_perf_corpora.py` (seed 7) into
`reports/corpora/`; `corpora --check` regenerates a missing file and fails if
its hash drifts from the manifest. `corpora --update` writes new hashes after a
deliberate corpus change.

## Report

One JSON object, `schema: 1`:

```text
meta        generator, date, git/dirty, profile, build profile, repeats,
            taskset, CPU/OS/rustc and the machine fingerprint
corpora     path, bytes, words, sha256 for every corpus used
metrics     flat dotted keys; each value is {min, median, max, runs}
correctness corpus hash, diagnostics total/hash and per-kind counts
resources   reserved for allocations/RSS (Phase 1)
raw         the first raw tool output per stage, only with --raw
```

`compare` only compares metrics present in both reports and only those ending
in a timing suffix (`_ms`, `_us`, `_s`, `_bytes`, `_kb`, `_mb`). A median more
than `--tolerance` (default 10 %) above the baseline is a regression. The
correctness section must match exactly; a faster wrong answer fails.

Reports from different machines are not comparable: the fingerprint (CPU, OS,
rustc, build profile) must match, or `compare` refuses unless
`--allow-machine-mismatch` is passed for inspection.

## Layers and metrics

| prefix | source | examples |
|---|---|---|
| `grammar_style.<corpus>.single` | `perf_grammar_style --single-thread` | `morphology_cold_ms`, `all_ms`, `sentence_cache_warm_ms` |
| `grammar_style.<corpus>.threads4` | `perf_grammar_style --threads 4` | the same keys, 4-worker |
| `check_suggest.<corpus>` | `perf_check_suggest` | `corpus.p50_us`, `word.<word>.check_us` |
| `index.single_thread` | `perf_index` | `build_ms`, `known_bytes`, `total_bytes` |

`morphology_cold_ms` is the fresh-process morphology pass; `morphology_warm_ms`
is the in-process second pass; the ratchet between them is the cache. The
sentence-cache metrics are a whole pass, an all-hit pass and a one-edit pass.
`perf_grammar_style` also reports cache hit/miss and depth counts in the raw
tool output.

## Methodology

- `full` runs each point in a **fresh process**, 7 times, and records min,
  median and max. `quick` is one run and is not a regression gate.
- Release builds only. The fingerprint includes the build profile and rustc.
- Pin with `--taskset` on the reference machine and record the mask.
- Compare medians, never a single run. The box is shared; the probe plan's
  callgrind instruction counts are the stable secondary signal (Phase 3).
- The correctness fingerprint is part of the report; a diagnostics count or
  hash change is a failure even when the timings improve.
- A deliberate optimisation records a new reference with its commit; do not
  overwrite a baseline to hide a regression.

## Reference

The current reference is the first `full` run recorded on the development
machine (AMD Ryzen 5 4600H, Linux, rustc 1.99.0, release,
fingerprint `f762dae2d048b2a6`). Reproduce it with
`bench.py run --profile full`; a different machine records its own baseline.
The full report is `bench/baselines/reference.json`.

| metric | median |
|---|---|
| `page-2000` cold morphology, single | 684.1 ms |
| `page-2000` 4-worker morphology | 206.1 ms |
| `page-2000` warm morphology | 0.46 ms |
| `page-2000` grammar+style warm | 19.39 ms |
| `page-2000` sentence-cache warm | 0.76 ms |
| 8 % typo corpus, cold morphology | 741.2 ms |
| all-unknown corpus, cold morphology | 787.3 ms |
| `check_suggest` p50 / p95 | 3 µs / 509 µs |
| reverse-index build / size | 22.6 s / 1.23 GB |

Correctness in the same report: 3 diagnostics on `page-2000`, 6 on the 8 %
corpus, 1 on the all-unknown corpus, and the eight form-canary hashes.

`cargo bench --bench torille` is the warmed hot-path complement: it covers
Levenshtein, `suggest`, `form_of` and the `check*` recognition entry points
with Criterion's statistical intervals. It is a separate stage because cold,
per-process and browser shapes are not Criterion's model.

## Current gaps (Phases 1–4 of the plan)

- Criterion groups for morphology, inflection, reverse lookup, hyphenation,
  typography, syntax and style.
- Allocation counts (a separate counting-allocator build) and peak RSS.
- Typography/hyphenation document timings; the homonym corpus in more tools.
- Browser/wasm cold load, post-seed and per-keystroke timing in the report.
- A manual/nightly CI job; the PR gate stays correctness-only.
