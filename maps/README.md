# maps/ — the embedded binary maps

**These files are not tracked on `master`.** They live on a single-commit
orphan branch named `maps`, so the repository never accumulates old versions
of the generated blobs. The first `cargo build` fetches them automatically
when they are missing. The explicit forms (`status`, `fetch`, `publish`) are
subcommands of the standalone `torille-maps` crate (`tools/maps-sync/`),
which does not depend on the engine and therefore still builds on a fresh
clone:

```bash
cargo run -p torille-maps -- status
cargo run -p torille-maps -- fetch
cargo run -p torille-maps -- publish   # after regenerating
```

## Automatic fetch on first build

A fresh clone has no `maps/*.bin.gz`, and `torille` cannot compile without
them. The root `build.rs` therefore fetches them on the first build:

- `tools/maps-sync/` (`torille-maps`) is a **build-dependency**, so it
  compiles before `torille` and works even though the engine cannot. The same
  crate provides the `status`/`fetch`/`publish` CLI above.
- `build.rs` checks the five blobs in `REQUIRED` (`derived_by_form`,
  `derived_nominal`, `derived_modifiers`, `pronoun_forms`,
  `oi_plural_lemmas`) — the ones the build cannot regenerate itself. If any
  is missing it calls `fetch`.
- `fetch` uses the clone's **`origin`** remote by default; `MAPS_REMOTE`
  overrides the name. It runs `git fetch --force <remote> maps:<ref>` (where
  `<ref>` is `refs/remotes/<remote>/maps`), retries that network step up to
  three times (750 ms apart) for flaky links, then writes **every**
  `maps/*.bin.gz` on the branch into the working tree — not just the five
  required ones.
- A failed fetch leaves already-present blobs untouched: files are written
  only after the fetch and listing succeed.

This runs when the root package's build script runs — `cargo build`,
`cargo check`, `cargo test`, normally with `--workspace` or `-p torille`.
`cargo run -p torille-maps -- fetch` does not trigger it (that crate does not
depend on `torille`). If the build stops with

```
… is missing: the binary maps live on the 'maps' branch; run
`cargo run -q -p torille-maps -- fetch` (or unset TORILLE_NO_FETCH …)
```

the automatic fetch was disabled or failed; run the explicit command.

Set `TORILLE_NO_FETCH=1`, `CARGO_NET_OFFLINE=1`, or `DOCS_RS=1` to forbid the
build-time fetch (an offline CI build, docs.rs). The build then requires the
blobs to already be present. The fetch is cross-platform Rust
(`std::process::Command` against `git`, argv only): no shell, `tar`, globbing,
or platform-specific script — only a `git` executable is needed.

`maps publish` writes one parentless commit and force-pushes it, so the
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
so change the generator and rerun it, then `cargo run -p torille-maps -- publish`. Use
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
| `proper_name_cases.bin.gz` | `build.rs` → `build/map_builders/front_coded.rs` | `src/expand/proper_names.rs` | `(proper-name form → case)` map |
| `adj_degrees.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/mod.rs` | positive → comparative/superlative |
| `adverbit.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/check_normal.rs` | `-ksi` adverbs and their possessives |
| `connegatives.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/connegative.rs` | bare potential/imperative connegatives |
| `connectives.bin.gz` | `build.rs` → `build/csv_builders/string_map.rs` | `src/expand/negation.rs` | negation fusions and finite auxiliaries |
| `oi_plural_lemmas.bin.gz` | `python3 tools/oracle/build_oi_plural_lemmas.py` | `src/expand/nominals/class_10.rs` | class-10 lemmas with the alternative `-oi-` plural (lexical, read off the Omorfi oracle) |
| `derived_by_form.bin.gz` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived surface → derived lemma |
| `derived_nominal.bin.gz` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived nominal set |
| `derived_modifiers.bin.gz` | `gen_derived` → `tools/src/der1.rs` | `src/expand/derived.rs` | derived modifier set |
| `degree_forms.bin.gz` | `gen_derived` | `src/expand/mod.rs` | front-coded reverse degree forms: surface -> positive/adverb lemma |
| `adverb_possessive.bin.gz` | `gen_derived` | `src/expand/check_normal.rs` | possessive adverb surface -> lemma (`keskenämme` -> `keskenään`) |
| `feminine_nouns.bin.gz` | `gen_derived` | `src/expand/check_normal.rs` | lexical `-nna` feminine surface -> masculine base (`pormestarinna` -> `pormestari`) |
| `lexical_adverbs.bin.gz` | `build.rs` | `src/expand/check_normal.rs` | established one-word adverb -> base (`alhaaltapäin` -> `alhaalta`) |
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
