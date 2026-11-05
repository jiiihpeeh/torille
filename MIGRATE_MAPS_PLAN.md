# Migrating the remaining phf maps to front-coded blobs

Status: **in progress** · owner: engine · target: wasm/binary size

## Why

The wasm grew to **41.0 MB** after the oracle/CSV regeneration. Most of the
growth is compile-time maps that are emitted as Rust source. Measured on the
current tree:

| artifact | shape now | source size | notes |
|---|---|---|---|
| `data/precomputed/all_forms_generated.rs` | `phf::Set<&'static str>` | 10.33 MB | 279 731 keys, 3.60 MB of raw strings |
| `data/precomputed/sanalista_generated.rs` | `phf::Map<&'static str, (u32, u32)>` | 4.47 MB | duplicates data already in `sanalista.bin` |
| `data/precomputed/connegatives_generated.rs` | `phf::Map` | 1.33 MB | |
| `data/precomputed/adverbit_generated.rs` | `phf::Map` | 0.26 MB | |
| `data/precomputed/adj_degrees_generated.rs` | `phf::Map` | 0.28 MB | |
| `stem_map_noun.bin` / `stem_map_verb.bin` | front-coded blob | 17.1 / 4.3 MB | already migrated (`ab3727d`) |

A `phf::Set` of 280 k keys pays for both the strings and a
`(hash, &'static str)` entry per key (~20 bytes), on top of `rustc` having to
parse the literal. The sorted keys front-code to **1.54 MB** (1-byte shared
prefix + suffix + NUL). The sanalista index is pure duplication: the blob
already carries `WORD_ORDER`, a `u32` array of row indices sorted by headword.

Expected: **~11 MB off the wasm** in phases 1–2 (41 → ~30 MB), plus ~1.4 MB
from phase 3.

## Phase 1 — sanalista word index: binary search, no phf

The blob's `WORD_ORDER` section is already sorted by headword. Use it.

- `build.rs::write_phf_index` and the `sanalista_generated.rs` write are
  removed; `generate` returns only the blob.
- `src/kotus.rs`:
  - drop the `include!` of `sanalista_generated.rs`;
  - `by_word` becomes a lower-bound binary search over `word_order()`
    (`O(log N)` string comparisons, ~17 for 104 k rows), then a walk over the
    contiguous equal-word run (homonyms).
- Delete `data/precomputed/sanalista_generated.rs`.
- Update the stale comments in `Cargo.toml` / `AGENTS.md` that describe a
  `phf::Map` word index.

Acceptance: `kotus` unit tests (`get_is_the_first_row`,
`every_row_is_reachable_by_its_word`, `a_homonym_yields_every_row`, the
packing round-trip) pass; `by_word` returns the same rows as before for
homonyms and misses.

## Phase 2 — form set: front-coded `StaticSet`

- `src/expand/static_table.rs`: factor the existing front-coded key reader
  out of `StemMap` into a shared `Keys` (header parsing, restarts, varints,
  `key_at`, binary-search `find`). Add `StaticSet { keys: Keys }` with
  `contains`, `len`, `is_empty`.
  - New magic `SSET`; same 32-byte header layout, with the value/base offsets
    zero. `RESTART = 16` as for the stem maps.
- `build.rs`: replace `emit_form_set` (phf) with `emit_front_coded_set`:
  same CSV predicate (nominative singular + genitive singular, no
  possessive), `BTreeSet` dedup/sort, front-code, write
  `maps/all_forms.bin`.
- `src/expand/all_forms.rs`: `FORM_SET` becomes `StaticSet` over
  `include_bytes!("all_forms.bin")`; `form_set()` returns `&StaticSet`.
  `torille-wasm/src/lib.rs` keeps using `.len()` in `warm_form_set`.
- Delete `data/precomputed/all_forms_generated.rs`.
- Add a `StaticSet` round-trip test with a hand-built blob (same pattern as
  the `StemMap` `encode` helper), plus the existing form-set tests.

Trade-off: phf is O(1); front-coded search is `O(log n)` with at most one
16-record block decode per comparison. The stem maps already pay exactly this
on the hot path. If the typing path regresses, `RESTART` can be lowered to 8
(faster, slightly larger); it is one shared constant.

## Phase 3 (optional) — the small maps

- `connegatives_generated.rs` (1.33 MB): front-coded `StaticMap` (key →
  value) reusing the stem-map value table shape.
- `adverbit_generated.rs`, `adj_degrees_generated.rs`: same `StaticMap`,
  or a single combined table.
- `connectives_generated.rs` is already tiny; leave it.

## Verification / gates

1. `cargo test -p torille --lib` (763 tests) and
   `cargo test -p torille --test catalogue` (26).
2. Build the release binaries **without LTO** for local runs
   (`CARGO_PROFILE_RELEASE_LTO=false`) — the LTO link is what gets OOM-killed
   on this box.
3. Behavioural spot checks: `check_normal --fast` on a lemma, a genitive, a
   partitive, a homonym, and a word that should miss.
4. Rebuild `wasm-pack --release`, rebuild the demo, record the
   before/after `.wasm` size.
5. Compare a sample of `by_word` and `is_known_inflected_form` probes against
   the old build (a small script or the existing tests).

## Risks

- **Lookup cost** on the per-keystroke form set. Mitigation: block restarts
  (already implemented) and a smaller `RESTART` if needed.
- **Build script reuse path**: if the CSV is absent but the blob is present,
  reuse the blob (`emit_stem_map` already does this); keep that contract.
- **Alignment**: the blob is `&[u8]`; the existing `word_order()` already
  handles the unaligned `u32` slice, so phase 1 adds no new unsafe.
- **Generated files are not hand-edited**: both `.rs` phf artifacts are
  deleted; the blobs are regenerated by `build.rs` from the CSVs.

## Phase 4 (beyond phf) — compact the stem-map values

`stem_map_noun.bin` was already front-coded, but its *values* were not. It
held 683 343 `(start, count)` pairs (5.47 MB) and 8.36 MB of NUL-separated
base strings, even though there are only **83 287 unique base strings** and
**99.5 % of keys have exactly one value**.

New value encoding (same front-coded key region):

- deduplicate values into one concatenated string pool + `(n+1)` u32 offsets;
- one `u32` per key: a direct string id (top bit clear), or `MULTI | offset`
  into an overflow region (`u8 count` + `count × u32 id`) for the 0.5 % of
  keys with 2–4 values;
- header grows from 32 to 40 bytes (`ids_off`, `overflow_off`,
  `string_offsets_off`, `string_count`, `strings_off`).

This also applies to the small maps (they share `StemMap`), and the form set
keeps only the key region.

| artifact | before | after |
|---|---:|---:|
| `stem_map_noun.bin` | 17.14 MB | **7.35 MB** |
| `stem_map_verb.bin` | 4.34 MB | **1.78 MB** |
| `connegatives.bin` | 0.75 MB | **0.42 MB** |
| `adverbit.bin` | 0.17 MB | **0.16 MB** |
| `adj_degrees.bin` | 0.18 MB | 0.22 MB (two values per key; overflow path) |
| `connectives.bin` | 0.4 KB | **0.28 KB** |

## Phase 5 — compact the sanalista blob

`sanalista.bin` was 6.29 MB: 28-byte records (2.93 MB), 2.33 MB of strings,
and a 193 KB `u32` inflections array. Three things dominated:

- the **POS** string was repeated on every row (~1.15 MB of text, plus 8
  bytes of offset+length per row);
- only **938 rows** carry a homonym, but every row paid 8 bytes for it;
- `u32` inflections and offsets where `u16` is enough (48 k entries).

New layout:

- 48-byte header with explicit section offsets;
- 8-byte per-row record: `u32` headword offset, `u8` length, `u8` POS code,
  `u16` inflection start;
- headword strings concatenated once; POS table (50 strings) and homonym
  side map (938 entries) stored once;
- inflections as `u16` (`class<<8 | gradation`).

The shared low-level readers (LE integers, section offsets, front-coded key
search) now live in `src/map_reader.rs`; `kotus` and `expand::static_table`
both use it.

`sanalista.bin` 6.29 MB → **2.96 MB**.

## Phase 6 — entropy-code the proper-name set

`proper_names.bin` is a pure set of 325 223 surface forms. Front-coding alone
left it at 1.65 MB, but the stream's bytes carry only **4.77 bits/byte** of
order-0 entropy (varint length bytes and Finnish case endings dominate). A
canonical Huffman code — length-limited to 12 bits with package-merge, decoded
through a direct 4096-entry table — takes the same front-coded stream to
**1.02 MB**, with no startup pass, no heap copy and no decompressor crate.
`build.rs::emit_huffman_set` decodes every key back before writing the blob;
`expand::huffman_set` is the in-place reader. This is the only set that uses
`HST1`; the other sets stay `SST2`.

## Progress

- [x] Phase 1: drop the sanalista phf, binary-search `WORD_ORDER`
- [x] Phase 2: front-coded form set (`all_forms.bin`)
- [x] Phase 3: connegatives / adverbit / adj_degrees / connectives
- [x] Phase 4: deduplicate stem-map values, one u32 id per key
- [x] Phase 5: compact the sanalista blob
- [x] Phase 6: Huffman-code the proper-name set (`HST1`)
- [x] Extract shared readers into `src/map_reader.rs`
- [x] Measure wasm before/after

## Result (2026-10-20)

| | before | after |
|---|---|---|
| `torille_wasm_bg.wasm` | 41 037 666 B (39.1 MiB) | **16 900 627 B (16.1 MiB)** |
| `all_forms` table | 10.33 MB Rust `phf::Set` literal | **1.82 MB** blob |
| sanalista word index | 4.47 MB Rust `phf::Map` literal | **0** (binary search over `WORD_ORDER`) |
| `stem_map_noun.bin` | 17.14 MB | **7.35 MB** |
| `stem_map_verb.bin` | 4.34 MB | **1.78 MB** |
| `sanalista.bin` | 6.29 MB | **2.96 MB** |
| `proper_names.bin` | 1.65 MB front-coded set | **1.02 MB** `HST1` set |
| small maps (4) | ~2.0 MB phf literals | **~0.8 MB** blobs |

Δ = **−24 137 039 B (−58.8 %)**. No `phf` / `phf_codegen`; every embedded
table is a binary blob read through `src/map_reader.rs`.

The proper-name set (325 k forms, added after this table) later brought the
wasm to 18 550 465 B; phase 6's Huffman set takes it back to **17 925 538 B**.

767 lib tests and 26 catalogue tests pass. Behaviour is unchanged:
`kissaa` → `kissa`, `talossa` → `talo`, `vedessä` → `vesi`,
`paistelivat` → `paistella`, `syöne` → `syödä`, `kylliksenne` → `kylliksi`,
`useimmat` → `usea`; `suurin` and the form-set hits still resolve, and
`juoni` still lists both homonym rows.
