# tools/build — the engine-driven build tools

Tools that regenerate the committed build inputs and blobs from **the
project itself**: the engine's own generator binaries and the committed
source tables. A ground-up build needs no Python binding, no Omorfi/Voikko
database and no sqlite oracle.

```bash
# from the repository root
cargo run --release -p torille-tools --bin gen_all_forms   # -> data/all_forms.csv (local)
cargo run --release -p torille-tools --bin gen_adj_degrees # -> data/adj_degrees.csv
cargo run --release -p torille-tools --bin gen_stem_map_noun   # -> data/stem_to_base_map.csv
cargo run --release -p torille-tools --bin gen_stem_map_verb   # -> data/verb_stem_to_base_map.csv
cargo run --release -p torille-tools --bin gen_derived   # -> maps/derived_*.bin
```

`cargo build -p torille` then turns the CSVs into `maps/*.bin` and embeds
them. `build_proper_names.py` is the one network fetch here (Wikidata); its
output `data/proper_names.csv` is committed.

The per-file inventory of `data/` is [`../../data/README.md`](../../data/README.md).

The tables that record lexical facts the engine cannot derive — the
catalogue, the noun/verb oracle databases, the proper-name cases, the
`-oi-` plural set, the class-6/15 exception sets and the per-class rule
extraction — are preserved under [`../oracle/`](../oracle/README.md). They
are reference generators, not part of a ground-up build.

## Verification

`compare_engine_outputs.py` regenerates engine output and compares it with
the committed tables (and, for `all_forms`, decodes the `SST2` blob). It is
the gate for the work in `FIX_GENGAPS.md`:

```bash
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/build/compare_engine_outputs.py --artifact stem_map_verb
```

Known oracle/lexical tables and their provenance are listed in
`FIX_GENGAPS.md` (G6). The build path here is engine-only; nothing in this
directory reads a sqlite database.
