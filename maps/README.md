# maps/ — the embedded binary maps

Every `.bin` file here is compiled into the crate with `include_bytes!` and
read in place: no runtime file discovery, no parsing, no allocation for the
map itself. The blobs are committed so a fresh clone (and CI, and the wasm
build) needs only `cargo build`.

**Do not hand-edit these files.** Each one is regenerated from its source
data; the table below names the producer. A stale blob is a correctness bug,
so change the generator and rerun it in the same commit.

## What is here

| blob | producer | reader | contents |
|---|---|---|---|
| `sanalista.bin` | `build.rs` → `build/csv_builders/sanalista.rs` | `src/kotus.rs` | the 104 743-row lexicon: headwords, POS/inflection codes, and the by-word / by-reversed-word orders |
| `catalogue.bin` | `build.rs` → `build/csv_builders/catalogue.rs` | `src/spec.rs` | paradigm templates, 8 slots per class (`data/catalogue.csv`) |
| `all_forms.bin` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/all_forms.rs` | front-coded set of lemma + genitive forms for the fast pass |
| `stem_map_noun.bin` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/check_normal.rs` | noun `(stem → bases)` map |
| `stem_map_verb.bin` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/check_normal.rs` | verb `(stem → bases)` map |
| `proper_names.bin` | `build.rs` → `build/map_builders/huffman.rs` | `src/expand/proper_names.rs` | `HST1` static-Huffman set of accepted proper-name forms |
| `proper_name_lemmas.bin` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/proper_names.rs` | `(proper-name form → lemma)` map |
| `adj_degrees.bin` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/mod.rs` | positive → comparative/superlative |
| `adverbit.bin` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/check_normal.rs` | `-ksi` adverbs and their possessives |
| `connegatives.bin` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/connegative.rs` | bare potential/imperative connegatives |
| `connectives.bin` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/negation.rs` | negation fusions and finite auxiliaries |
| `oi_plural_lemmas.bin` | `python3 tools/build/build_oi_plural_lemmas.py` | `src/expand/nominals/class_10.rs` | class-10 lemmas with the alternative `-oi-` plural (lexical, read off the Omorfi oracle) |
| `derived_by_form.bin` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived surface → derived lemma |
| `derived_nominal.bin` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived nominal set |
| `derived_modifiers.bin` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived modifier set |
| `pronoun_forms.bin` | `gen_derived` → `tools/src/der1.rs` | `src/expand/check_normal.rs` | the closed pronoun paradigm |

## How to regenerate

The `build.rs` tables rerun automatically when their CSV changes;
`cargo:rerun-if-changed` is declared per input. To force a rebuild:

```bash
touch data/nykysuomensanalista.csv   # or the relevant CSV
cargo build -p torille
```

The four engine-computed tables cannot be produced by a build script (it
cannot link the engine it is building):

```bash
cargo run --release -p torille-tools --bin gen_derived
```

The oracle-derived table needs the materialised Kotus database:

```bash
python3 tools/build/build_oi_plural_lemmas.py
```

After regenerating, run the drift checks:

```bash
cargo test --release -p torille --lib the_blobs_match_compute
cargo test --release -p torille --lib the_pronoun_blob
```

## Formats

There are four blob shapes; each has one reader module that documents the
layout:

- **Lexicon** — `sanalista.bin`, documented by `build/csv_builders/sanalista.rs`
  and read in `src/kotus.rs`.
- **`TCAT`** — the paradigm catalogue, documented and read in `src/spec.rs`.
- **Front-coded set/map** — `all_forms`, the stem maps, `proper_name_lemmas`
  and the small string maps; see `src/map_reader.rs` and
  `src/expand/static_table.rs`. `proper_names.bin` adds the `HST1`
  static-Huffman layer, read in `src/expand/huffman_set.rs`.
- **`DER1`** — the sorted key/value tables written by `tools/src/der1.rs`;
  read by `src/expand/blob.rs`.

The writers live in `build/` (build-script side) and `tools/src/der1.rs`
(tool side). This directory holds only the data.
