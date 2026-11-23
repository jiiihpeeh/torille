# Python examples

Runnable tours of the `torille` Python extension. Each script is standalone and
uses only the standard library; together they cover the whole surface the
binding exposes.

## Setup

The extension is built with [maturin](https://www.maturin.rs/):

```bash
(cd bindings && env -u CONDA_PREFIX maturin develop --release)
```

`env -u CONDA_PREFIX` matters when both `VIRTUAL_ENV` and `CONDA_PREFIX` are
set; always build `--release`, because loading and inflecting are several times
faster than in a debug build.

Then run a script from the repository root:

```bash
python3 examples/python/00_showcase.py
```

`Engine()` materialises the lexicon once per process (the data is embedded, so
there is no file to find). Construct it once and reuse it.

## The scripts

| script | shows |
|---|---|
| `00_showcase.py` | the whole pipeline over one text |
| `01_inflect.py` | paradigms, confidences, cases, possessives, class override |
| `02_lexicon.py` | homonyms, parts of speech, classless compounds |
| `03_check_suggest.py` | `check`, `suggest` and the `check_normal` gate |
| `04_morphology.py` | readings, agreement, clitics, possessives, derivations |
| `05_syntax.py` | clauses, roles, relative nesting, word-order features |
| `06_grammar.py` | severities, rule ids, byte spans, corrections, re-check |
| `07_style.py` | suggestion-only style kinds and replacements |
| `08_typography.py` | the semicolon pass and `apply_typography` |
| `09_hyphenation.py` | points, fragments, options, soft hyphens |
| `10_wizard.py` | class inference from sparse forms |

## The API in one glance

```python
from torille import Engine

engine = Engine()
engine.inflect("talo", klass=None, pos=None)      # [PyForm, ...]
engine.class_candidates("mökki", "noun", [(Cell.GEN_SG, "mökin")])
engine.lookup("kuusi")                             # [(class, pos), ...]
engine.words("substantiivi", include_classless=False)
engine.check("talossa")                            # PyCheck(kind, bases)
engine.suggest("Äitj", max_n=5, max_distance=2)    # [PySuggestion, ...]
engine.check_normal("paloi")                       # PyNormal(kind, base, gate)
engine.morphology(text)                            # [PyWord, ...]
engine.syntax(text)                                # ([PySyntaxToken], [PyClause])
engine.grammar(text)                               # [PyDiagnostic, ...]
engine.style(text)                                 # [PyDiagnostic, ...]
engine.grammar_and_style(text)                     # ([grammar], [style]) over one pass
engine.typography(text)                            # [PyTypography, ...]
engine.apply_typography(text)                      # str
engine.hyphenate("tietokoneohjelmointi")           # [PyHyphenationPoint, ...]
```

`Cell` is a real enum: `Cell.ILL_SG` is the `ill_sg` question, `Cell.parse("ill_sg")`
round-trips, and `next_question` comes back as a `Cell` (or `None`), so a typo
in a cell name is a `TypeError` rather than a silently skipped question.

Two details that shape the examples:

- **Byte offsets.** Diagnostics and typography edits carry byte ranges into the
  UTF-8 source, while a Python string is indexed in characters. `ä` is two
  bytes, so slice with `text.encode("utf-8")[start:end].decode("utf-8")` (the
  scripts define a small `span_text` helper). Hyphenation points provide both
  `byte_offset` and `char_offset`; slice with the latter.
- **`klass`, not `class`.** `class` is a Python keyword, so the inflection
  class argument is spelled `klass`.

The binding's implementation and doc comments are in
[`bindings/src/lib.rs`](../../bindings/src/lib.rs); the Rust engine is described
in the main [`README.md`](../../README.md).
