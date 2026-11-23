# PLAN_BENCHMARK_SUITE — one reproducible suite for every layer

Status: **Phase 0 landed 2026-11-22; Phases 1–4 proposed.** Baseline
`2e5e8e4`. The tree already has
good measurement primitives — a Criterion hot-path bench, `perf_grammar_style`,
`perf_check_suggest`, `perf_index`, `profile_hot`, the browser pool scripts and
the four probe corpora — but they are separate tools with separate arguments,
schemas and definitions of "cold". The suite is the missing **orchestration
and regression layer**, not another measurement engine.

## 1. What exists, and what is missing

| surface | current tool | gap |
|---|---|---|
| hot path | `benches/torille.rs` | no morphology/inflection/hyphenation groups |
| document layers | `perf_grammar_style --json` | no typography/hyphenation/alloc/RSS |
| check/suggest | `perf_check_suggest --json` | no homonym flag; ad-hoc schema |
| index build | `perf_index` | text only; RSS measured outside |
| phase profile | `profile_hot` | text only; one workload |
| browser/wasm | `e2e/pool-timing.mjs` | separate from the native report |
| corpora | page, homonyms, generator | no manifest/hash; homonyms unused |
| comparison | none | no baseline or threshold |

The probe plan (`PLAN_PROBE_PERF.md`) already fixed the methodology: four
corpora, fresh processes, median of at least seven runs, `taskset`, callgrind
as the stable secondary signal, and parity checks beside every timing. The
suite reuses those rules; it does not re-derive them.

## 2. Goals

1. One command produces one versioned JSON report covering every layer, plus a
   human-readable table.
2. Every number is reproducible from a fresh checkout with the documented
   commands; no hidden warm state, no debug builds, no single-run claims.
3. A **correctness fingerprint** travels with the timing report: differences,
   diagnostics counts, engine-output hashes. A faster wrong answer fails the
   suite.
4. A same-machine `compare` mode with a recorded reference baseline and
   per-metric tolerances.
5. Cover the shipping path: native single-thread, native 4-worker, wasm/browser.
6. Add no new runtime dependency; Criterion is already a dev-dependency, and
   the driver stays Python (the harness language) or a thin Rust binary.

## 3. Non-goals

- No optimisation work in this plan; it only measures. The probe/perf plans
  own the fixes.
- No cross-machine comparison. Absolute milliseconds are recorded with the
  machine fingerprint; the gate compares only matching fingerprints.
- No Criterion for cold-process or browser measurements: Criterion warms up
  and runs in one process, which is the wrong shape for those.
- No committed generated corpora beyond the small canonical fixtures; the
  generator plus a hash manifest is the source of truth.
- No plotting dependency and no dashboard. A JSON report and a text table are
  enough for the repository.

## 4. Architecture

### 4.1 Driver

`tools/bench.py` (Python 3, standard library only) orchestrates the existing
release binaries and writes one report:

```bash
python3 tools/bench.py run --profile quick          # ~2 minutes
python3 tools/bench.py run --profile full           # all layers, 7 runs
python3 tools/bench.py compare reports/bench-reference.json reports/bench.json
python3 tools/bench.py record reports/bench.json --name reference
python3 tools/bench.py corpora                      # write/verify hashes
```

- `--profile quick` runs one fresh process per corpus and one repeat; it is
  the developer loop and the schema smoke test.
- `--profile full` runs the probe methodology (fresh process × 7, median,
  single-thread and 4-worker) and the Criterion groups.
- `--compare` prints `metric, baseline, current, delta %, verdict` and exits
  non-zero on a regression over the tolerance (default **10 %**) or on a
  correctness-fingerprint change.
- `record` copies a report alongside a machine fingerprint; only a matching
  fingerprint is comparable.

The driver invokes, never reimplements:

```text
target/release/perf_grammar_style --json --file <corpus> --words 0 --repeat N [--single-thread|--threads 4]
target/release/perf_check_suggest  --json --corpus <corpus>
target/release/perf_index          [--single-thread]
target/release/dump_diagnostics    <corpus>            # fingerprint
cargo bench --bench torille -- --save-baseline <name>  # full profile only
```

### 4.2 Report schema

A single versioned object (`schema: 1`) with five sections:

```text
meta        git commit/dirty, rustc, profile, date, CPU, OS, taskset, repeats
corpora     path, bytes, words, sha256 for every corpus used
layers      tokenise, morphology(cold/warm/cache), syntax, rules, style,
            sentence_cache, check_suggest, index, hyphenation, typography
resources   peak RSS, allocation counts (separate alloc build), blob/wasm sizes
correctness diagnostics counts per layer, engine-output hash
```

Every timing is `{runs: [..], min, median, max}`; a single number without its
run list is not written. The browser section is optional and carries its own
`available: false` marker when no browser is installed.

### 4.3 Corpora

- Canonical, committed: `tests/fixtures/perf/page-2000.txt`,
  `homonyms.txt`, `line-wrap.txt`.
- Generated with `tools/gen_perf_corpora.py`, deterministic seed, written to
  `reports/corpora/`: the 8 % typo and all-unknown variants of `page-2000`.
- Large text: a bounded book excerpt (a few of the existing
  `torille-wasm/demo/books` files, or a 100k-word concatenation) for the
  scaling curve.
- A `bench/corpora.json` manifest records the source, generator seed, SHA-256
  and word count; `bench.py corpora` fails if a corpus drifts.
- The homonym corpus is mandatory for every morphology/browser run: an
  optimisation that drops a reading can look fast and clean on the page.

## 5. Phases

### Phase 0 — schema, driver skeleton, fingerprints (landed)

**Landed 2026-11-22:** `tools/bench.py` with `run/compare/record/corpora/
selftest`, the versioned report, `bench/corpora.json` with hashes, the
correctness fingerprint (diagnostics plus the full paradigm of eight canary
lemmas), and `--json` on `perf_index`/`dump_diagnostics`. The quick profile is
110 metrics over `page-2000` and `homonyms`; the full profile is 7 fresh
processes over all five corpora, both thread shapes and the index build.
`docs/benchmarking.md` is the contract.

- Define the schema as a Rust struct in `tools/src/bin/bench_report.rs` (or a
  JSON schema in `tools/bench_schema.json`) and have `bench.py` validate it.
- Add `--json` to `perf_index` (same field names as the text output) and a
  `--json` summary to `dump_diagnostics`.
- Implement `run --profile quick`, `record`, `compare` and `corpora` against
  the existing tools; no new measurement code.
- Capture the reference report on the reference machine and record the date,
  CPU and command in `docs/benchmarking.md`.

Gate: quick run completes, schema validates, `compare` passes against itself,
and a deliberately edited metric fails.

### Phase 1 — fill the measurement gaps

- Extend `benches/torille.rs` (or add `benches/layers.rs` with a second
  `[[bench]]`) with groups for: morphology (`analyze_morphology` hit/miss),
  `inflect`/`inflect_all` per class, `find_bases_with`, `hyphenate`,
  `typography::check`, `syntax_check::check` on a sentence, `style_check`.
- Extend `perf_grammar_style --json` with the typography and hyphenation
  timings, the 4-worker wall time (not only `single_thread`), and the
  `probe_stats` class table when the feature is on.
- Add an **allocation build**: a `bench-alloc` feature plus a
  `tools/src/bin/bench_alloc.rs` with a counting global allocator. It runs
  the same corpora and reports allocations per word and per pass; it is a
  separate binary/build so the counter never perturbs the wall-time run.
- Measure peak RSS from `/usr/bin/time -v` and record it in `resources`.
- Add the homonym corpus to `perf_grammar_style` and
  `perf_check_suggest` (a `--corpus` path already works; the driver names it).
- Add a `--scaling` mode: 1k/10k/100k words from the book, with the same
  single/four-worker split.

Gate: every layer has a primitive that the driver can call, and each primitive
has a JSON or parseable text output.

### Phase 2 — browser/wasm

- Make `e2e/pool-timing.mjs` (and the content-script pool script) emit JSON:
  cold load, first check, post-seed pass, per-keystroke, panel render, wasm
  bytes and peak JS heap where available.
- Add `bench.py run --browser` (skipped with `available: false` when no
  browser is installed); it serves the demo and runs the existing scripts.
- Record the wasm size (`torille_wasm_bg.wasm`) and the gzip size.

Gate: the browser section is reproducible on the reference machine and the
native/browser tables agree on the layer names.

### Phase 3 — baselines, comparison, docs

- `docs/benchmarking.md`: methodology, commands, schema, what each metric
  means, how to record a baseline, noise policy, callgrind/allocations, and
  the current reference table with the machine fingerprint.
- A `tools/bench.py compare` table with a 10 % default tolerance and a
  per-metric override list; timing gates are same-machine only.
- `bench/baselines/reference.json` (machine-fingerprinted) is the recorded
  reference; a new machine records its own, never overwrites the reference.
- Optional `--instructions`: run selected binaries under
  `valgrind --tool=callgrind`, parse the instruction count and write it as the
  stable secondary metric. This is slower and only runs in the full profile.

Gate: the release gate can be “the full suite on the reference machine is
within tolerance and the fingerprint is unchanged”; a PR gate stays
correctness-only (CI is engine-only and noisy).

### Phase 4 — CI hook

- Add a `workflow_dispatch` (and optional nightly) job that builds release,
  runs `bench.py run --profile quick --json`, uploads the report as an
  artifact and fails only on the schema/correctness check, not on timing.
- Keep the PR `ci.yml` unchanged: timing on a shared GitHub runner is not a
  gate.

## 6. Method

- **Cold** — fresh process, fresh caches, the same corpus bytes.
- **Warm** — second pass in the same process, reported separately.
- **Per-keystroke** — one word through the shipping state/caches, not a
  document pass.
- **Runs** — at least 7 fresh processes for the full profile; report min,
  median and max, never a single run. The quick profile reports the raw run
  and marks itself non-comparable.
- **Pinning** — `taskset -c` on a documented CPU set; record the mask in
  `meta`.
- **Tolerances** — 10 % default on medians; 2 % on instruction counts; a
  correctness change is always a failure.
- **Machines** — reports carry a fingerprint (CPU model, OS, rustc, target,
  release profile, git commit); `compare` refuses a mismatch.
- **Correctness first** — every report includes the diagnostics counts and an
  engine-output hash; the suite never compares two different outputs.

## 7. Reference baseline (2026-11-23, this machine)

The latest full suite run, re-recorded after the lowercase-fold optimisation
and checked in as `bench/baselines/reference.json`
(fingerprint `f762dae2d048b2a6`). These are the numbers the suite pins; they
are not budgets.

```text
page-2000 cold morphology      575.9 ms median (single), 168.1 ms (4 workers)
page-2000 warm morphology        0.50 ms
page-2000 grammar+style warm    18.95 ms
page-2000 sentence-cache warm    0.76 ms
8 % typo corpus cold           621.9 ms
all-unknown corpus cold        551.9 ms
check_suggest p50 / p95          3 us / 473 us
reverse-index build / size      23.3 s / 1.23 GB
correctness                     3 / 6 / 1 diagnostics; 8 form canaries
```

The full report has 594 metrics over five corpora, both thread shapes and the
index build. A different machine records its own baseline; `compare` refuses a
fingerprint mismatch.

## 8. Files

- `tools/bench.py` (new driver), `tools/bench_schema.json` (or a Rust struct).
- `benches/torille.rs` (extend) and/or `benches/layers.rs` + `Cargo.toml`.
- `tools/src/bin/perf_grammar_style.rs`, `perf_check_suggest.rs`,
  `perf_index.rs` (JSON/extra metrics).
- `tools/src/bin/bench_alloc.rs` (new) and the `bench-alloc` feature.
- `tools/src/bin/dump_diagnostics.rs` (JSON summary/fingerprint).
- `tools/gen_perf_corpora.py`, `bench/corpora.json`.
- `docs/benchmarking.md` (new), `misc_md/PLAN_BENCHMARK_SUITE.md`.
- `.github/workflows/bench.yml` (manual/nightly, optional).
- `AGENTS.md` deep-docs list.

## 9. Risks

- **Noise.** The box is shared; pinning plus median-of-7 plus the instruction
  count is the mitigation, and the suite must not gate on a single run.
- **Criterion scope creep.** Criterion is for warmed microbenchmarks only;
  cold/process/browser measurements stay in the driver.
- **Fixture drift.** The hash manifest is the guard; a changed corpus is a new
  baseline, not a silent comparison.
- **Allocation counters change the code.** The counting allocator lives only
  in the separate `bench-alloc` build, never in the shipping library or the
  wall-time binaries.
- **Baseline rot.** The reference is dated and machine-fingerprinted; a
  deliberate optimisation records a new reference with its commit.
- **Browser flakiness.** The browser stage is optional and never part of the
  PR gate; it reports `available: false` rather than failing a headless box.
- **Measuring the harness.** The driver invokes release binaries and adds no
  work inside the measured region; the JSON parsing happens after the run.

## 10. Out of scope / related plans

- Optimisation work: `PLAN_PROBE_PERF.md`, `PLAN_GRAMMAR_STYLE_PERF.md`,
  `PLAN_PARALLEL_MORPHOLOGY.md`, `PERFORMANCE_TODO.md`.
- Correctness gates: `docs/harness.md`, `tests/catalogue.rs`, the syntax/style
  fixtures.
- A public benchmark site or cross-machine leaderboard: not planned.
