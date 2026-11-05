# tools/build — artifact generators

Scripts that write the committed build inputs and generated sources. They are
not part of the shipped crate; run them only when a source of truth changes
(the sanalista, an oracle, a `RULES` change) and commit the output in the same
change.

The **build itself never needs these tools or a database**. `cargo build` reads
the committed `maps/*.bin`, the committed CSVs under `data/`, and generates the
rest with `build.rs`; the blobs are embedded with `include_bytes!`.

Most generators now read the **engine** (the `torille` Python binding) rather
than an oracle database. The exceptions are listed at the end: those tables
record lexical facts the engine cannot derive, so they still need their
original source and are best treated as frozen reference data.

## From-scratch order

Run from the repository root.

```bash
# 1. regenerated from the Kotus sources / the engine
python3 tools/build/build_catalogue_csv.py
python3 tools/build/build_all_forms_csv.py        # default --source engine
python3 tools/build/build_stem_to_base_map.py     # --source engine
python3 tools/build/build_verb_stem_to_base_map.py
python3 tools/build/build_adj_degrees.py
python3 tools/build/build_adverbi_forms.py
python3 tools/build/build_proper_names.py
python3 tools/build/build_proper_name_forms.py

# 2. engine-computed blobs
cargo run --release -p torille-tools --bin gen_derived

# 3. generated Rust sources
python3 tools/build/extract_class_rules.py --rust
python3 tools/build/build_class6_i_plural.py
python3 tools/build/build_class15_long_partitive.py
python3 tools/build/build_oi_plural_lemmas.py
```

Then `cargo build -p torille` turns the CSVs into `maps/*.bin` and the tests
check the derived blobs (`the_blobs_match_compute`,
`the_pronoun_blob_matches_compute`).

The measured distance between the committed tables and engine output, and
the plan to close it, is [FIX_GENGAPS.md](../../FIX_GENGAPS.md).

## Engine vs oracle

| generator | source |
|---|---|
| `build_all_forms_csv.py` | engine (`--source engine` is the default) |
| `build_stem_to_base_map.py` | engine, plus oracle candidates in `hybrid` |
| `build_verb_stem_to_base_map.py` | Kotus verb oracle (`data/kotus_verbit.sqlite`) |
| `build_adj_degrees.py` | engine (binding) |
| `build_adverbi_forms.py` | Voikko |
| `build_catalogue_csv.py` | noun/verb oracle databases |
| `build_nomine_lexicon.py` | Omorfi + Voikko (builds the noun oracle) |
| `build_verb_lexicon.py` | Omorfi + Voikko (builds the verb oracle and the connective/connegative tables) |
| `build_proper_names.py` | Wikidata dump / CSV (network) |
| `build_proper_name_forms.py` | Omorfi |
| `build_oi_plural_lemmas.py` | noun oracle (lexical; not derivable) |
| `build_class6_i_plural.py` | noun oracle (lexical exception set) |
| `build_class15_long_partitive.py` | noun oracle (lexical exception set) |
| `extract_class_rules.py` | noun oracle (writes `rules/` and `src/expand/nominals/`) |

The last five are reference tables, not engine output: they exist precisely
because the engine does not derive those forms. Regenerating them from the
engine would delete the facts they encode.

`gen_derived` is the one Rust generator and is not in this directory; it lives
in `tools/src/bin/gen_derived.rs` because it links the engine.
