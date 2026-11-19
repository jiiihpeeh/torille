# PLAN_FORM_TABLE — an exact surface → base + reading table

Status: **proposed, not implemented** · 2026-11-19 · branch `form-table` ·
base `be4e13a` (same tree as `syntax`/`master`)

This plan exists because one measured number changed: the grouped two-level
reverse table costs **3.15 MB gzip without readings and ~3.7 MB with them**
for the whole nominal + verb lexicon, not the ~8 MB a flat table costs. That is
close enough to the Phase 6 ceiling to be a download-budget decision instead
of an engineering impossibility. Nothing here is built yet — see
`PLAN_GRAMMAR_STYLE_PERF.md` §Phase 10 for the pricing measurements.

Contents: [§1 the question](#1-the-question) ·
[§2 current cost](#2-what-the-cold-pass-costs-today) ·
[§3 layout](#3-the-layout-that-fits) ·
[§4 blob format](#4-blob-format-fmt1) ·
[§5 wiring](#5-wiring-and-the-fallback-contract) ·
[§6 generator](#6-the-generator) ·
[§7 phases](#7-phases-and-gates) · [§8 risks](#8-risks-and-abort-criteria) ·
[§9 harness](#9-measurement-harness) · [§10 decisions](#10-decisions)

## 1. The question

`resolve_word_fast` answers "which lemma is this surface a form of, and with
what readings" by naming candidate bases from the stem maps and then
**building each candidate's whole paradigm** (`emits_form`) to confirm it.
A verb paradigm is ~700 forms; a page builds ~2,000 of them for ~1,100 unique
surfaces. The table turns the confirmation, and the reading extraction that
follows it, into a lookup.

## 2. What the cold pass costs today

`page-2000`, cold `analyze_morphology`, single-thread, release, after the
Phase 11 numeral fix (instrumented by call site, counters removed after the
run):

| caller | builds | of which fail | form visits | build ms |
|---|---:|---:|---:|---:|
| noun stem-map verify | 740 | 123 | 62 k | 130 |
| verb stem-map verify | 602 | 271 | 813 k | 320 |
| head-1 walk | 43 | 28 | 30 k | 15 |
| other (cheap gate) | 599 | 486 | 469 k | 220 |
| **total** | **1,984** | **908** | **1.38 M** | **685** |

The pass is 1,170–1,210 ms single-thread, 319–348 ms on four workers, and
364–395 ms per fresh page in the browser pool. Paradigm construction is ~57 %
of the single-thread pass.

The table attacks the **1,342 stem-map verifies**, of which 948 succeed. A
successful verify is followed by a `form_readings` walk unless the Phase 8
probe answers it — those walks are not in the table above because
`form_readings` does not store a paradigm. An ending-only table removes the
verify but not the walk (the base still needs its readings); a table that
carries the reading removes both. That is why the reading byte is the whole
point.

Out of scope, and left on the current path: possessives and clitics,
classless compounds and their heads, proper names, and the derived layers
(degrees, numerals, `-inen` adjectives).

## 3. The layout that fits

Measured on the real lexicon (`PLAN_GRAMMAR_STYLE_PERF.md` §Phase 10):

| layout | raw | gzip |
|---|---:|---:|
| nominal flat `surface → base` | 21.0 MB | 2.68 MB |
| nominal naive two-level (base per entry) | 11.8 MB | 2.85 MB |
| **nominal grouped `stem → base → endings`** | **8.2 MB** | **1.35 MB** |
| verb flat | 40.8 MB | 5.44 MB |
| **verb grouped** | **15.4 MB** | **1.79 MB** |
| **total grouped** | **23.6 MB** | **3.15 MB** |
| the same + a reading per entry (estimated) | ~28 MB | **~3.7 MB** |

The grouping is the trick: `static_table.rs` records that 99.5 % of stem keys
have exactly one base, so the base id is written **once per stem**, not once
per `(ending, base)` entry. Prefix sharding is not part of this: it saved 33 KB
of 2.68 MB (1.2 %), so it is a lookup-locality choice at best.

## 4. Blob format (`FMT1`)

The grouped layout is not the existing `StemMap` shape (one key, one value
list), so it gets its own reader. Same house rules as `static_table.rs`:
little-endian, front-coded sorted keys, restart points, string pool, no
runtime parsing of CSV.

```text
 0  u32 magic "FMT1"        u32 version
 8  u32 stem_count          u32 base_count
16  u32 reading_count       u32 reserved
20  u32 stems_off           u32 groups_off
28  u32 bases_off           u32 readings_off
36  u32 restarts[]
     stems:    front-coded sorted stem keys; a u32 id per key
     groups:   per stem: u16 base_group_count, then per group
                 varint base_id, u16 entry_count,
                 per entry: u8 ending_len, ending bytes, u8 reading_count,
                            reading ids (u16 each)
     bases:    u32 count, (count+1) u32 offsets, UTF-8 lemma pool
     readings: u32 count, 4 bytes each: case, number, confidence, mood,
               tense, person, voice, polarity packed into u32
```

`ending` is the exact remainder after the stem, not a case suffix name, so the
reader never needs the morphology tables. `reading` is the same `Tags` data
`form_readings` returns minus `part_of_speech` and the lemma, which the caller
fills from the base (exactly what `add_readings_from_base_matched` does today).

Packing: `case` and `number` are `Option`, so 5 + 2 bits; `confidence` 2;
`mood` 4; `tense` 3; `person` 3; `voice` 2; `polarity` 1 — 22 bits, padded to
u32. The reading table is deduplicated, so a verb page with three moods pays
for three readings, not three per form.

`build.rs` treats the blob as **optional**, copying the `all_forms` pattern:
if `data/form_table.csv` exists, build it; else if `maps/form_table.bin.gz`
exists, decompress it; else emit an empty table (`stem_count = 0`). An empty
table makes every lookup miss, which is exactly the feature-off behaviour, so
a clone without the blob still builds. The blob is regenerable, so it does
**not** join `torille-maps::REQUIRED`.

## 5. Wiring and the fallback contract

**The table never rejects.** It is a cache of what the engine already emits:

- lookup hit → the bases and readings are used as-is;
- lookup miss (`stem` absent, `(stem, ending)` absent, or `stem_count == 0`) →
  today's `emits_form` + `form_readings` path runs unchanged.

That is the safety property that makes the change reviewable: a missing,
stale or partial table can only make the resolver slower, never wrong, and it
cannot lose a reading. It also means the table can ship as a frequency-bounded
subset later without touching the resolver.

Touch points:

| file | change |
|---|---|
| `src/expand/form_table.rs` (new) | `FMT1` reader, `bases(stem, ending)`, `readings(id)` |
| `src/expand/check_normal.rs:1256` | `collect_bases_via_stem_map`: probe the table first |
| `src/expand/check_normal.rs:1320` | `collect_bases_via_verb_stem_map`: same |
| `src/expand/mod.rs:679` | `form_readings`: answer from the table when the hit carries readings |
| `src/morphology/mod.rs:1336` | `add_stem_map_bases`: unchanged; it calls `form_readings` |
| `build.rs` | optional blob, `form_table.bin` into `OUT_DIR` |

`collect_bases_via_verb_stem_map` currently probes the residue and its
vowel-trimmed form. The generator records the split it actually used, so the
table hit comes from the raw residue; the trimmed probe stays as the fallback.
No new ending lists are introduced: the table is keyed by whatever split the
generator produced, and the resolver still walks `ENDINGS` / `VERB_ENDINGS`.

## 6. The generator

`tools/src/bin/gen_form_table.rs`, run after a `RULES` or sanalista change:

1. For every lexicon row with a class, take the row's own part of speech
   (`lemma_pos`, the same function the verify uses) and inflections.
2. Generate with the **same call the runtime makes**:
   `inflect_all(lemma, &row.inflections, cat, Some(&index), pos, false)` —
   no reimplementation of a class engine.
3. For each form, compute the split the runtime would take: the longest
   prefix of the surface that is a stem-map key for that base (the
   `data/stem_to_base_map.csv` / `verb_…csv` pair sets are the authority), and
   the remainder as the ending.
4. Merge the form's readings per `(stem, ending, base)` with
   `tags::add_reading`, which is the merge `Form::absorb` performs, so a
   surface with several slots keeps every reading.
5. Drop `Guessed` forms, negative-polarity forms, clitics, possessives and
   multi-word forms, matching the mask generator's filters.
6. Write `data/form_table.csv` (`stem  ending  base  reading…`, one row per
   merged reading) and print the entry/byte counts.

Two counts to check at once: the probe measured 1,105,860 nominal and
2,295,351 verb entries; a materially different number means the split or the
filters changed.

## 7. Phases and gates

### P0 — branch and plan (this commit)

No code. The plan, the AGENTS entry and the roadmap link land together.

### P1 — generator, price with readings (Gate A)

Generator + `data/form_table.csv` + size report for both variants:
ending-only and with readings. **Gate A: the reading-carrying blob must be
≤ 4.5 MB gzip; abort above 5 MB.** The estimated 3.7 MB is an estimate until
this runs.

### P2 — blob and reader

`src/expand/form_table.rs` with a hand-built blob in the tests, mirroring
`static_table.rs`: front-coded keys, offsets, empty-table behaviour, and a
`lookup` unit test on a two-stem fixture. `build.rs` gains the optional
`form_table.bin` (CSV → build; gz → reuse; neither → empty). No resolver
change yet.

### P3 — wiring behind `form-table` (default off)

`collect_bases_*` and `form_readings` consult the table; every miss falls
through. Feature flag in the root `Cargo.toml` gated **around the resolver
calls only**, never around the `include_bytes!`, so the blob's presence is a
data question and the flag is a behaviour question.

**Gate C (parity):** with the flag on, diagnostics are byte-identical on
`tests/fixtures/syntax_diagnostics.json`, `style_diagnostics.json`,
`page-2000`, the word-order fixtures and the prose corpus; and
`form_readings`-from-table equals `form_readings`-from-walk for every row.
The exhaustive comparison is `#[ignore]`d for the normal suite and runs in the
nightly job; the normal suite samples every 97th lemma, the same pattern
`gen_lemma_stem_shapes` uses.

### P4 — measure (Gate B, Gate D)

- **Gate B:** single-thread cold ≤ 800 ms (from ~1,190), 4-worker ≤ 260 ms,
  pool fresh page ≤ 300 ms. If the lookup cannot beat the builds it removes,
  stop.
- **Gate D:** the wasm grows by the blob; record
  `torille_wasm_bg.wasm` and the brotli sidecar, and re-run
  `e2e/pool-timing.mjs`. The **session first check must not regress**: the
  first check instantiates the module in four workers, so 23 MB of new data
  segments can cost more than the per-page saving.

### P5 — decide

Pass → publish (`cargo run -p torille-maps -- publish`), add the blob and CSV
to `data/README.md` and `maps/README.md`, make the feature default, update
`AGENTS.md`, `README.md`, `PLAN_GRAMMAR_STYLE_PERF.md` and
`TODO_GRAMMAR_STYLE.md`. Fail → record the negative result in the plan and on
the roadmap, delete the generator and the reader, and keep the class-level
`accepts` idea as the remaining lever.

## 8. Risks and abort criteria

| risk | guard |
|---|---|
| the lookup costs as much as the builds it removes | Gate B; the ending loop is already O(#endings) front-coded probes, and the table adds one per hit |
| table readings drift from the engine | generated by the engine; exhaustive staleness test + regeneration diff |
| the blob makes session start worse | Gate D measures the first check, not only the per-page one |
| a partial table silently loses a reading | misses fall back; the table cannot reject |
| verb splits differ from the runtime probe | the split is computed from the committed `(stem, base)` pairs, i.e. the runtime's own key set; 0.13 % of forms had no split in the probe and simply stay on the fallback |
| a class engine changes | regenerate; the staleness test fails first |

**Abort** if any of: Gate A (blob > 5 MB), Gate B (no measurable win),
Gate C (parity cannot be shown exhaustively), Gate D (first check regresses
by more than the per-page saving).

## 9. Measurement harness

Reuse what exists:

```bash
cargo run --release -p torille-tools --bin perf_grammar_style -- \
  --words 1959 --repeat 3 --single-thread
cargo run --release -p torille-tools --bin perf_grammar_style -- \
  --words 1959 --repeat 3 --threads 4
cd extensions/firefox && npm run build && node e2e/pool-timing.mjs
```

The generator prints raw and gzip bytes for the blob it would write, so Gate A
never needs a committed file to fail. `tests/catalogue.rs` stays the
correctness net; the exact-output fixtures are the parity net.

## 10. Decisions

- **Grouped, not flat, not sharded.** 3.15 MB against 8.1 MB, and sharding
  saves 1.2 %.
- **Readings in the table.** Ending-only removes 948 verifies and leaves every
  `form_readings` walk in place; it is not worth 3.15 MB. The reading byte is
  what pays.
- **Never reject.** Misses fall back, so a partial or stale table cannot
  change a result.
- **Optional blob, feature-gated resolver.** The default build keeps today's
  download size until Gate D says the module growth is paid back.
- **Possessives, clitics, classless compounds and derived layers stay out.**
  They are 26 of 1,124 surfaces on the fixture and each has its own path.
