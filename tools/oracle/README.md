# tools/oracle — reference generators (not part of a ground-up build)

These scripts read the materialised Kotus/Omorfi/Voikko databases, or query
Omorfi and libvoikko directly. They exist to refresh the **committed
reference data** when the upstream sources change, not to build the project:
`cargo build` and `tools/build/` need only the repository itself.

Most are `GPL-3.0-or-later`, because they link or drive Omorfi (GPLv3) and
libvoikko (GPLv2-or-later). The complete license text is
[`../LICENSE-GPL-3.0`](../LICENSE-GPL-3.0). Nothing here is copied into the
engine, the binding or the browser code; their output is data, and `data/`
keeps its own terms.

| script | reads | writes |
|---|---|---|
| `build_nomine_lexicon.py` | Omorfi + Voikko | `data/kotus_nomines.sqlite` |
| `build_verb_lexicon.py` | Omorfi + Voikko | `data/kotus_verbit.sqlite`, `data/verb_connectives.csv`, `data/verb_connegatives.csv` |
| `build_catalogue_csv.py` | both oracle dbs | `data/catalogue.csv` |
| `build_adverbi_forms.py` | Voikko | `data/adverbit.csv` |
| `build_proper_name_forms.py` | Omorfi | `data/proper_name_forms.csv` |
| `build_oi_plural_lemmas.py` | noun oracle | `maps/oi_plural_lemmas.bin` |
| `build_class6_i_plural.py` | noun oracle | `src/expand/nominals/class_6_i_plural.rs` |
| `build_class15_long_partitive.py` | noun oracle | `src/expand/nominals/class_15_long_partitive.rs` |
| `extract_class_rules.py` | noun oracle | `rules/`, `src/expand/nominals/` |

Run them from the repository root, as before:

```bash
python3 tools/oracle/build_nomine_lexicon.py -o data/kotus_nomines.sqlite
python3 tools/oracle/build_verb_lexicon.py -o data/kotus_verbit.sqlite
python3 tools/oracle/build_catalogue_csv.py
python3 tools/oracle/extract_class_rules.py --rust
```

Why these tables are not engine-derived is recorded in `FIX_GENGAPS.md`
(G5/G6): the engine has no authority to invent an exception it does not
implement, and deleting the table would remove exactly the fact it encodes.
