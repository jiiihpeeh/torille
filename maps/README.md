# maps/ — the embedded binary maps

**These files are not tracked on `master`.** They live on a single-commit
orphan branch named `maps`, so the repository never accumulates old versions
of the generated blobs. After cloning:

```bash
tools/fetch_maps.sh        # fetch origin/maps and unpack maps/*.bin.gz
```

After regenerating them:

```bash
tools/publish_maps.sh      # replace the `maps` branch with the current blobs
```

`publish_maps.sh` writes one parentless commit and force-pushes it, so the
branch always has exactly one commit with the current blobs. The previous
commit becomes unreachable; GitLab prunes it during its next housekeeping
(run the project's *Clean up* job to do it immediately).

`build.rs` decompresses the fetched copies into `OUT_DIR`; the compiler
embeds the plain bytes. The maps are the only generated artefacts treated
this way — `data/*.csv` and `data/*.sqlite` have their own rules below.


Every `.bin.gz` file here is **tooling-level compression only**. The blobs
are stored gzip-compressed at the maximum level, but `build.rs` decompresses
each one into `OUT_DIR` and the crate embeds the *plain* bytes with
`include_bytes!`. The runtime and the wasm have
no gzip dependency and still read the maps in place: no runtime file
discovery, no parsing, no allocation for the map itself.

**Do not hand-edit these files.** Each one is regenerated from its source
data; the table below names the producer. A stale blob is a correctness bug,
so change the generator and rerun it, then `tools/publish_maps.sh`. Use
`cargo run -p torille-tools --bin map_gz -- test maps/X.bin.gz` to inspect
one, or `map_gz compress|decompress` for a manual round-trip.

## What is here

| blob | producer | reader | contents |
|---|---|---|---|
| `sanalista.bin.gz` | `build.rs` → `build/csv_builders/sanalista.rs` | `src/kotus.rs` | the 104 743-row lexicon: headwords, POS/inflection codes, and the by-word / by-reversed-word orders |
| `catalogue.bin.gz` | `build.rs` → `build/csv_builders/catalogue.rs` | `src/spec.rs` | paradigm templates, 8 slots per class (`data/catalogue.csv`) |
| `all_forms.bin.gz` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/all_forms.rs` | front-coded set of lemma + genitive forms for the fast pass |
| `stem_map_noun.bin.gz` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/check_normal.rs` | noun `(stem → bases)` map |
| `stem_map_verb.bin.gz` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/check_normal.rs` | verb `(stem → bases)` map |
| `proper_names.bin.gz` | `build.rs` → `build/map_builders/huffman.rs` | `src/expand/proper_names.rs` | `HST1` static-Huffman set of accepted proper-name forms |
| `proper_name_lemmas.bin.gz` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/proper_names.rs` | `(proper-name form → lemma)` map |
| `adj_degrees.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/mod.rs` | positive → comparative/superlative |
| `adverbit.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/check_normal.rs` | `-ksi` adverbs and their possessives |
| `connegatives.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/connegative.rs` | bare potential/imperative connegatives |
| `connectives.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/negation.rs` | negation fusions and finite auxiliaries |
| `oi_plural_lemmas.bin.gz` | `python3 tools/oracle/build_oi_plural_lemmas.py` | `src/expand/nominals/class_10.rs` | class-10 lemmas with the alternative `-oi-` plural (lexical, read off the Omorfi oracle) |
| `derived_by_form.bin.gz` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived surface → derived lemma |
| `derived_nominal.bin.gz` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived nominal set |
| `derived_modifiers.bin.gz` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived modifier set |
| `pronoun_forms.bin.gz` | `gen_derived` → `tools/src/der1.rs` | `src/expand/check_normal.rs` | the closed pronoun paradigm |

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
python3 tools/oracle/build_oi_plural_lemmas.py
```

After regenerating, run the drift checks:

```bash
cargo test --release -p torille --lib the_blobs_match_compute
cargo test --release -p torille --lib the_pronoun_blob
```

## Formats

There are four blob shapes; each has one reader module that documents the
layout:

- **Lexicon** — `sanalista.bin.gz`, documented by `build/csv_builders/sanalista.rs`
  and read in `src/kotus.rs`.
- **`TCAT`** — the paradigm catalogue, documented and read in `src/spec.rs`.
- **Front-coded set/map** — `all_forms`, the stem maps, `proper_name_lemmas`
  and the small string maps; see `src/map_reader.rs` and
  `src/expand/static_table.rs`. `proper_names.bin.gz` adds the `HST1`
  static-Huffman layer, read in `src/expand/huffman_set.rs`.
- **`DER1`** — the sorted key/value tables written by `tools/src/der1.rs`;
  read by `src/expand/blob.rs`.

The writers live in `build/` (build-script side) and `tools/src/der1.rs`
(tool side); both gzip at `flate2::Compression::best()`. `build.rs`
decompresses the tool-written tables into `OUT_DIR` for the compiler. This
directory holds only the compressed data; the plain bytes live in `OUT_DIR`
during a build.
