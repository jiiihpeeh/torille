# data/ — sources, committed build inputs and reference tables

`data/` holds the material a build reads; the embedded binary maps live in
`../maps/` (see `../maps/README.md`). Nothing here is written into the shipped
binary except through `build.rs` / `include_bytes!`.

Terms are in [`LICENSE`](LICENSE); `hyphenation/` has its own MIT license.
The Kotus and Wiktionary material keeps its own terms, independent of the
engine's MIT/Apache-2.0 license.

## Sources

| file | origin | notes |
|---|---|---|
| `nykysuomensanalista.txt` | Kotus *Nykysuomen sanalista* (CC-licensed) | original export, no `Johdokset` column; read by `tools/add_sanalista_word.py` |
| `nykysuomensanalista.csv` | the `.txt` plus the derivative flag | the embedded lexicon source read by `build.rs` |
| `hyphenation/hyph-fi-x-school.tex`, `exceptions.csv` | vendored MIT school patterns | see `docs/hyphenation.md` |
| `LICENSE` | — | terms for the material in this directory |

## Committed build inputs (regenerable, but committed so a clone builds)

| file | producer | used by |
|---|---|---|
| `adj_degrees.csv` | `gen_adj_degrees` (Rust bin) | `maps/adj_degrees.bin.gz` |
| `adj_degrees_extra.csv` | curated (en.wiktionary) | merged by `gen_adj_degrees` into `adj_degrees.csv` |
| `stem_to_base_map.csv` | `gen_stem_map_noun` (Rust bin) | `maps/stem_map_noun.bin.gz` |
| `verb_stem_to_base_map.csv` | `gen_stem_map_verb` (Rust bin) | `maps/stem_map_verb.bin.gz` |

Regenerate with the engine binding, then `cargo build -p torille`. Do not
hand-edit; `tools/build/compare_engine_outputs.py --strict` is the check.

## Frozen reference data

These encode lexical facts the engine does not derive. Their generators are
under `../tools/oracle/` and need the Kotus/Omorfi/Voikko databases; the
committed files are what a build uses.

| file | producer | notes |
|---|---|---|
| `catalogue.csv` | `tools/oracle/build_catalogue_csv.py` | 8 paradigm slots per class; read by `spec.rs` |
| `adverbit.csv` | `tools/oracle/build_adverbi_forms.py` | `-ksi` adverb possessives |
| `adverb_degrees.csv` | curated (en.wiktionary) | `maps/adverb_degrees.bin.gz`; adverb comparatives such as `hiljaa -> hiljempaa` |
| `feminine_nouns.csv` | curated (Kotus/en.wiktionary) | `maps/feminine_nouns.bin.gz`; historical `-nna` feminines such as `pormestarinna -> pormestari` |
| `lexical_adverbs.csv` | curated (Kotus/Kielikello) | `maps/lexical_adverbs.bin.gz`; established one-word adverbs such as `alhaaltapäin`, `äkkipikaa` |
| `proper_names.csv` | Wikidata | name source for the proper-name tables |
| `proper_name_forms.csv`, `proper_name_forms_extra.csv` | `tools/oracle/build_proper_name_forms.py` + curated extras | singular cases of names: `form`, `name`, Omorfi case code |
| `verb_connegatives.csv`, `verb_connectives.csv` | `tools/oracle/build_verb_lexicon.py` | connegatives and negation fusions |
| `verb_rection.csv` | curated (Kotus/Kielitoimiston ohjepankki) | `lemma,case,frame,priority,note` valency frames used by the syntax checker |
| `verb_arguments.csv` | curated (Kotus/Kielitoimiston ohjepankki) | `lemma,slot,case,requirement,note` argument-completeness frames used by the syntax checker |
| `oracle_exceptions.json` | curated | evidence for the harness (`tools/test_illegal_evidence.py`) |
| `wiktionary-taivutukset.txt` | Wiktionary | fetched paradigms used by `tests/catalogue.rs` |

## Curated style tables

These are optional, suggestion-only data for the style layer
(`../PLAN_STYLE.md`). They never feed the spell checker or the grammar pass.
The `note` column is curation-only and is not shipped.

| file | producer | used by |
|---|---|---|
| `style_phrases.csv` | original curation | `maps/style_phrases.bin.gz`; `phrase,kind,replacement,note` |
| `style_words.csv` | original curation | `maps/style_words.bin.gz`; `word,kind,replacement,note` |

## Local only (ignored)

| file | why it is local |
|---|---|
| `kotus_nomines.sqlite` (2.6 GB) | noun oracle; rebuild with `tools/oracle/build_nomine_lexicon.py` |
| `kotus_verbit.sqlite` (3.6 GB) | verb oracle; rebuild with `tools/oracle/build_verb_lexicon.py` |
| `kotus-tyypit.txt` | Kotus reference document; must not be redistributed |

`data/all_forms.csv` is a local dump (the committed artifact is
`maps/all_forms.bin.gz`) and is ignored too. `git status --ignored data` should
show only these files.

## What not to put here

- Binary maps: they live in `../maps/`.
- New engine-generated tables: add the producer under `tools/build/` and
  document it here.
- Large downloads: fetch them with a script under `tools/` and keep the
  output ignored.
