# tools/build — the engine-driven build tools

Scripts that regenerate the committed build inputs and blobs from **the
project itself**: the `torille` Python binding and the committed source
tables. A ground-up build needs no Omorfi/Voikko database and no sqlite
oracle.

```bash
# from the repository root
python3 tools/build/build_all_forms_csv.py        # -> data/all_forms.csv (local dump)
python3 tools/build/build_stem_to_base_map.py     # -> data/stem_to_base_map.csv
python3 tools/build/build_verb_stem_to_base_map.py
python3 tools/build/build_adj_degrees.py
cargo run --release -p torille-tools --bin gen_derived   # -> maps/derived_*.bin
```

`cargo build -p torille` then turns the CSVs into `maps/*.bin` and embeds
them. `build_proper_names.py` is the one network fetch here (Wikidata); its
output `data/proper_names.csv` is committed.

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
