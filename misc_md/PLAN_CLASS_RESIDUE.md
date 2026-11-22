# PLAN_CLASS_RESIDUE — a multi-class audit bug, a stale binding, and two homonyms

Status: **landed 2026-11-22.** Baseline `d63826e`. This plan follows the
slice-2 measurement in [`PLAN_FOLLOWUPS.md`](PLAN_FOLLOWUPS.md). The recorded
`41*E`/`48*E` residue turned out to be **three** different things, and only one
of them was an engine gap.

## 1. What the residue was

### 1.1 The live audit read one class at a time

`havas` is `41*E, 39` in the sanalista. `_audit_lexicon` already inflects every
class of a multi-class lemma, but the **live** path (`Harness.audit` with
Omorfi/Voikko) called `torille_forms(lemma, klass)` for the reported class
only. The 41-E reading does not carry `havaksen`/`havaksetta`/`havaksien`;
the 39 reading does. The audit therefore reported 139 missing forms and 6
not-plain forms that the engine produces through its other class.

The fix mirrors `_audit_lexicon`: build the unit's forms from every class of
the lemma (`class_tokens`), except under `--class-strict`, which keeps the
single-class view for debugging. After the fix the n=5 live sweep is
**0 missing / 0 not-plain / 0 illegal**.

Wiktionary confirms the two paradigms: `havas (41-E)` gives `hapaat`,
`hapaan`, `havaan`-family, and `havas (39)` gives `havakset`, `havaksen`,
`havaksia`. Both are generated; the 41-E forms are a Wiktionary-accepted
reading that Voikko does not cover.

### 1.2 The stale binding is a separate trap

The first sweep used the Python extension, which predated the recent engine
fixes. Nothing warned: `test_binding_agrees_with_binary.py` covers only 23
lemmas. The binding has since been rebuilt and agrees on the canaries, but the
trap is real and is now guarded:

- `omorfi_harness.binding_is_stale()` compares the installed `.so` mtime with
  the engine sources; `report_by_class.py` refuses to sweep a stale binding and
  names `maturin develop --release`.
- `test_binding_agrees_with_binary.py` gained the canary lemmas (`havas`,
  `live`, `tarjoilu`, `suurkaani`, `enempi`).

### 1.3 `live` is two nouns

Wiktionary Finland gives:

- `live (48-E)` — the dialectal `lipeä` noun: `lipeen`, `livettä`, `lipeessä`,
  `lipeitten`;
- `live (8)` — the English loanword `livelähetys`: `liven`, `liveä`, `livejä`,
  `liveen`, and the possessive `livenne`.

The sanalista row is the 48-E noun and the adjective (`live 2`, class 99).
The common class-8 reading was missing. The oracle confirms `livenne` as the
only `ok='both'` row, and `check_word_against_omorfi.py live` was missing it.

The fix is a homonym-aware `sanalista_overrides` entry:

```rust
CLASS_BY_HOMONYM: &[("live", "1", "48*E, 8")]
```

The adjective stays 99 and indeclinable. The class-8 paradigm is generated
alongside the 48-E one; `livenne`, `liven`, `liveä`, `livet` now exist, while
`lipeen` and `livettä` keep their dialectal reading.

### 1.4 The oracle caveat

Voikko does not know the `live` lemma at all (it reports even `live` as
illegal), and it rejects the 41-E `hapa-` forms of `havas`. Both readings are
Wiktionary/Kotus words. The engine keeps them; the harness records the
`unsupported`/`illegal` rows as a Voikko coverage gap, not an engine defect.
Do not delete a reading to make the live pair quiet.

## 2. What landed

- **`tools/omorfi_harness.py`** — `CANARY_LEMMAS`, `binding_is_stale()`, and
  the multi-class union in `Harness.audit` (mirroring `_audit_lexicon`).
- **`tools/report_by_class.py`** — a stale-binding preflight with the rebuild
  command; `TORILLE_NO_EXTENSION=1` runs the binary path and skips it.
- **`tools/test_binding_agrees_with_binary.py`** — the canary lemmas.
- **`src/sanalista_overrides.rs`** — homonym-aware `CLASS_BY_HOMONYM` and
  `class_for(word, homonymy)`.
- **`src/kotus.rs`, `build/csv_builders/sanalista.rs`** — pass the homonymy.
- **`build.rs`** — `rerun-if-changed` for `src/sanalista_overrides.rs` and
  `src/rejected.rs`, so the sanalista blob follows the override.
- **`data/stem_to_base_map.csv`** — regenerated with `gen_stem_map_noun` (adds
  the class-8 `livej` stem).
- **`maps/stem_endings_*.bin.gz`** — regenerated with `gen_stem_endings`; the
  ending signatures are position-aligned with the stem map, so a new stem key
  shifts every later signature and would reject ordinary forms (`taloa`) until
  the table is rebuilt.
- **`tests/catalogue.rs`** — `the_live_homonyms_keep_their_own_readings`:
  both classes, both paradigms, the adjective stays 99, the engine lists
  `live` as a base of `livenne`.

## 3. Measured

| run | confirmed | missing / not-plain / illegal |
|---|---:|---|
| n=5 live, before the union | 79,830 | 140 / 6 / 0 |
| n=5 live, after all fixes | **80,277** | **0 / 0 / 0** |

The per-word live check has 0 missing for `live` (`livenne` is now confirmed);
its remaining `illegal` rows are the Voikko coverage gap from §1.4, including
`live` itself. `havas` likewise has 0 missing; its 41-E `hapa-` forms are the
same coverage gap.

## 4. Follow-ups kept open

- Publish the regenerated `sanalista.bin.gz`, `stem_map_noun.bin.gz` and
  `stem_endings_*.bin.gz` to the `maps` branch
  (`cargo run -p torille-maps -- publish`) so a fresh clone gets the class-8
  `live` blob and the matching ending signatures.
- The per-class `confirmed` totals now include the union of a multi-class
  lemma's paradigms, so a dual-class form can be counted in both class
  buckets. That is deliberate and documented; the defect statuses are the
  gate, not the confirmed sum.
- The `live` 48-E/8 and `havas` 41-E/39 splits are recorded in
  `docs/harness.md`; a Voikko coverage update would clear the live check, not
  an engine change.

## 5. Gates

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
python3 tools/test_binding_agrees_with_binary.py
python3 tools/report_by_class.py -n 5 --live
python3 tools/check_docs.py
```

- The live sweep is 0 missing / 0 not-plain / 0 illegal.
- The binding canary passes; a stale binding fails the preflight.
- `check_word_against_omorfi.py live` has 0 missing (the `illegal` rows are the
  documented Voikko gap).
- `page-2000` is unchanged at 3 diagnostics; the syntax/style gates do not move.
