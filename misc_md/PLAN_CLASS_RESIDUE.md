# PLAN_CLASS_RESIDUE — a stale binding, the `live` homonym, and the class-sweep gate

Status: **proposed.** Baseline `d63826e` (2026-11-22). This plan follows the
slice-2 measurement in [`PLAN_FOLLOWUPS.md`](PLAN_FOLLOWUPS.md): the recorded
`41*E`/`48*E` residue is not one defect. `41*E havas` was a **stale Python
binding**, and `48*E live` is a **missing homonym reading**: the dialectal
48-E noun is in the sanalista, the common class-8 loanword is not.

## 1. Evidence

### 1.1 The stale binding

The slice-2 sweep used `tools/report_by_class.py`, which reaches torille
through the Python extension when it is importable
(`tools/omorfi_harness.py::_load_engine`). That binding predated the recent
engine fixes and returned 139 missing + 6 not-plain forms for `havas`. After

```bash
(cd bindings && env -u CONDA_PREFIX maturin develop --release)
```

the binary and the binding emit `havaksen`, `havaksetta`, `havaksitta` and
`havaksien`; the `41*E` rows disappear. Nothing in the report says the binding
is stale, and `test_binding_agrees_with_binary.py` covers only 23 lemmas, none
of them `havas`.

This is a measurement-integrity defect, not an engine defect. It is more
dangerous than a wrong rule: it makes the harness report a defect that the
engine has already fixed, and the next fix would be unnecessary.

### 1.2 The `live` homonym

`data/nykysuomensanalista.csv` has:

```text
live  1  substantiivi  48*E
live  2  adjektiivi    99
```

Wiktionary's Finnish entry has **two nouns**:

- `live (48-E)` — the dialectal word for `lipeä`, exactly the engine's
  paradigm: `lipeen`, `livettä`, `lipeessä`, `lipeitten`, …;
- `live (8)` — the English loanword `livelähetys`, with `liven`, `liveä`,
  `livejä`, `liveen`, and the possessive `livenne`.

The sanalista has only the 48-E dialectal reading and the indeclinable
adjective. The oracle (`data/kotus_nomines.sqlite`) expects the class-8
reading too — `livenne` is the only `ok='both'` row for `live`, and
`check_word_against_omorfi.py live` reports **1 missing (`livenne`) and 165
illegal**: Voikko rejects the dialectal 48-E forms that Wiktionary accepts.
This is a reading split hidden by the class report's `unsupported` bucket
(engine-only forms are not counted as precision failures).

### 1.3 What this plan does not touch

The possessive A/B items are already closed (see `PLAN_FOLLOWUPS.md` §3); the
`41*E` claim there is superseded by §1.1. The class engines and the
`AUTHORITATIVE_CLASSES` lists are not changed.

## 2. Goals

1. Make a stale binding impossible to mistake for engine truth.
2. Add the missing class-8 `live` noun reading without disturbing the
   dialectal 48-E reading or the adjective homonym.
3. Re-measure with the fresh binding, update the trackers, and record the
   dialectal/loanword split as a documented oracle caveat.

## 3. Phase 1 — binding freshness

### 3.1 Design

- Add a canary set that exercises recent engine changes: `havas` (41*E), the
  `live` noun (both readings), `tarjoilu` (the B possessive), and a few
  arbitrary class lemmas. The canary compares the Python binding with the
  `inflect` binary (the harness already has `inflect_binary()` and the
  `TORILLE_NO_EXTENSION=1` fallback).
- `report_by_class.py` runs the canary before the sweep and fails with a
  message naming `maturin develop --release` when the binding disagrees. A
  `--no-binding`/`TORILLE_NO_EXTENSION=1` run uses the binary and skips the
  check.
- Extend `test_binding_agrees_with_binary.py` with the canary lemmas, so the
  standard gate catches the same drift.
- Document the rebuild in `docs/harness.md` (the command is already in
  `AGENTS.md`; the sweep section must repeat it).

### 3.2 Gate

- A deliberately stale binding makes the sweep fail at the canary, and the
  message names the rebuild command.
- With `TORILLE_NO_EXTENSION=1` the sweep runs on the binary and passes the
  same canary.
- The extended binding test agrees on the canaries.

## 4. Phase 2 — the `live` reading

### 4.1 Design

The class-8 reading belongs on the **noun homonym**, not on the adjective.
`sanalista_overrides` currently keys a correction by lemma alone, which would
also hit `live 2` (adjective, 99). Make the correction homonym-aware:

```rust
const CLASS_BY_HOMONYM: &[(&str, &str, &str)] = &[
    // Wiktionary `live` (8) is the live-broadcast loanword; the same
    // spelling is the dialectal 48-E `lipeä` in Kotus.
    ("live", "1", "48*E, 8"),
];
fn class_for(word: &str, homonymy: &str) -> Option<&'static str>;
```

`class_for` checks the homonym table first, then the existing lemma table, so
the six current overrides keep their behaviour. Update both parsers
(`src/kotus.rs` and `build/csv_builders/sanalista.rs`) to pass the homonymy.

The dual expression `48*E, 8` makes the engine emit both paradigms. The
dedup pass merges identical surfaces; the 48-E and class-8 stems differ
(`lipe-` vs `live-`), so both readings survive. The adjective homonym stays
99 and indeclinable.

### 4.2 Tests

- `inflect("live")` contains the 48-E forms (`lipeen`, `livettä`, `lipeessä`,
  `lipeitten`) and the class-8 forms (`liven`, `liveä`, `livejä`, `liveen`,
  `livenne`).
- The adjective `live 2` has no case forms beyond the citation.
- `check_word_against_omorfi.py live` has `livenne` confirmed; the dialectal
  48-E rows are adjudicated below.
- `tarjoilu`/`lovi` canaries still pass (the override table is unchanged).

### 4.3 The dialectal/loanword oracle caveat

Voikko rejects the dialectal 48-E forms that Wiktionary accepts, and the
oracle marks them `engine`/`omorfi` rather than `both`. After the class-8
reading lands, either:

- the curated `data/oracle_exceptions.json` records the 48-E forms as an
  accepted dialectal reading, with the Wiktionary citation; or
- the harness reports them as `unsupported`/`not_plain` and the known-defects
  entry names the dialectal reading.

Choose one, record it in `docs/harness.md`, and never remove the 48-E reading
just to make Voikko quiet. The class-8 reading is the fix; the dialectal
reading is a separate, accepted word.

## 5. Phase 3 — re-measure and update the record

- [ ] Rebuild the binding (`maturin develop --release`) and run the sweep:
      `python3 tools/report_by_class.py -n 20 -j 8 --live`.
- [ ] Run the per-word canaries: `check_word_against_omorfi.py havas live
      tarjoilu`.
- [ ] Regenerate the sanalista blob and publish the `maps` branch
      (`cargo build`, then `cargo run -p torille-maps -- publish`) if the
      override changes the embedded blob.
- [ ] Update `docs/harness.md` Known defects: remove the `41*E` stale claim,
      record the `live` 48-E/8 split, and add the binding-freshness note.
- [ ] Update `misc_md/TODO.md` and `PLAN_FOLLOWUPS.md` §3/§8: the `41*E` rows
      were a stale binding, not an engine gap.
- [ ] Keep the class sweep's `unsupported` bucket honest: a `--verify-illegal`
      or per-word canary run is part of the release gate, because engine-only
      forms do not show up as `illegal` there.

## 6. Gates

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
python3 tools/test_binding_agrees_with_binary.py   # extended canaries
python3 tools/report_by_class.py -n 20 --live      # canary + sweep
python3 tools/test_reverse_lookup_recall.py
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/test_illegal_evidence.py
python3 tools/check_docs.py
```

Plus:

- `check_word_against_omorfi.py live` no longer has the `livenne` missing row.
- `page-2000` stays at 3 diagnostics; the syntax/style gates are unchanged.
- No new `GAPS` entry or `AUTHORITATIVE_CLASSES`/`DISPATCH` change.

## 7. Files

- `src/sanalista_overrides.rs` — homonym-aware class correction.
- `src/kotus.rs`, `build/csv_builders/sanalista.rs` — pass the homonymy.
- `tools/omorfi_harness.py` — binding canary and rebuild message.
- `tools/report_by_class.py` — preflight check.
- `tools/test_binding_agrees_with_binary.py` — canary lemmas.
- `tests/` — the `live` reading tests (catalogue or a dedicated test).
- `docs/harness.md`, `misc_md/TODO.md`, `misc_md/PLAN_FOLLOWUPS.md`.

## 8. Risks and stop conditions

- **The binding canary is slow.** It runs a handful of lemmas through the
  binary; cache the result per process and fail only on a real difference.
- **The homonym override changes the blob.** Regenerate and measure the maps
  diff; if the blob cannot be published in the same change, stop after the
  tests and record the pending publish.
- **The dual reading changes suggestion order.** The dedup and `ClassPrior`
  order are observable; if a suggestion golden list moves, regenerate the
  golden list only after checking the new order is the intended one.
- **The dialectal 48-E reading is not a Voikko word.** Do not delete it and do
  not teach Voikko; record the caveat.
- **Stop after Phase 1** if the binding canary already prevents the class
  sweep from lying; the `live` fix can land separately.

## 9. What this closes

- The false `41*E` defect and the class-sweep trap.
- The missing `livenne` and the class-8 `live` paradigm.
- The unrecorded dialectal 48-E reading.
