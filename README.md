# torille

A Finnish spell checker and morphology engine. It answers two questions:

- **is this word correctly spelled?** — `check`
- **if not, what did the user mean?** — `suggest`

It is dictionary-driven, not statistical. The Kotus *sanalista* supplies the
lemmas, a paradigm catalogue (generated from the Omorfi and Voikko oracles)
supplies each class's slots, and the engine works out which slots a particular
member actually has. Unknown words
fall through to compound analysis and the productive derivation rules.

## Quick start

```bash
cargo build --workspace         # the first build fetches maps/*.bin.gz automatically
cargo test --release --workspace

cargo run --release -p torille-tools --bin inflect -- talo
cargo run --release -p torille-tools --bin check -- talo
cargo run --release -p torille-tools --bin hyphenate -- tietokoneohjelmointi
cargo run --release -p torille-tools --bin check_syntax -- "Koira nukkuvat."
```

The first build fetches `maps/*.bin.gz` from the orphan `maps` branch through
the `torille-maps` build-dependency; see
[`maps/README.md`](maps/README.md#automatic-fetch-on-first-build) for the
remote, the offline switches, and what happens when the fetch fails.

The wasm demo and the browser extensions are separate builds:

```bash
cd torille-wasm/demo && npm install && node build.mjs
cargo run --release -p torille-demo-server      # serves the demo on :8765

cd extensions/firefox && npm install && npm run build   # dist/ and the XPI
```

Load the built extension from `extensions/firefox/dist/manifest.json` (or the
XPI) in `about:debugging`, and from `extensions/chromium/dist/` as an unpacked
extension.

## How it works

The pipeline is:

```text
sanalista row ──> class + gradation ──> catalogue slots ──> stem + ending
                                                   │
                              compound / derivation fallbacks
                                                   ↓
                                          Form { reading, confidence }
```

Every generated form carries a **confidence** (`src/tags.rs`):

| | |
|---|---|
| `Plain` | a plain attachment, no repair. These are the only forms a correction may be drawn from. |
| `Repaired` | built by repairing the lemma (gradation, epenthesis, an alternation read off the genitive). A real word often enough to accept, never to suggest. |
| `Guessed` | a type-rule guess with no catalogue form behind it. Accepted, never suggested. |

A form also carries one or more `Tags` readings (POS, case, number, tense,
person, mood, voice, possessive, clitic, polarity). `Form::union` is the
intersection of what several readings agree on, which is why `taloni` is
legitimately case-less.

`suggest` ranks candidates by Levenshtein distance (with transpositions and an
extracted edit script), then the deterministic `existing_score`, then the word.
A small QWERTY-adjacency table breaks distance-1 ties. The known defects —
recall, and `illegal` forms from wrong stems — are tracked in
[`docs/harness.md`](docs/harness.md) and [`TODO.md`](TODO.md).

## Workspace

```text
torille/
  src/                  the engine
    kotus.rs            sanalista and paradigm-catalogue parsers
    spec.rs             the parsed catalogue
    tags.rs             Confidence, Tags, Form
    grammar/            type-driven noun/verb classes, stems, gradation
    expand/             the pipeline, per-class engines, compounds, clitics
    suggest/            check, suggest, candidate ranking, edit distance
    hyphenation/        Finnish line-break hyphenation
    compound.rs         splitting an unknown word
    sentence.rs         sentence spans and token context
    syntax.rs           on-demand clause/role analysis
    syntax_check/       grammar diagnostics and corrections
    style_check/        opt-in style suggestions (separate pass)
  maps/                 embedded binary maps, with a README
  data/                 Kotus sources, committed build inputs, reference tables
  tools/                CLI drivers, build generators, oracle reference, harness
  bindings/             PyO3 module `torille`
  wrapper/              hunspell-shaped C ABI
  torille-wasm/         wasm-bindgen bindings + the Solid demo
  torille-wasm-leptos/  Leptos front end
  demo-server/          the static server for the demo
  extensions/           Firefox and Chromium extensions
  tests/                catalogue, candidate generation, hyphenation, Wiktionary
  docs/                 harness.md, hyphenation.md, lexicon.md
```

The root package is the library and the workspace root. `tools`,
`tools/maps-sync` (the `torille-maps` maps-branch helper), `bindings`, the
wasm crates and the demo server are members, so one `cargo build` covers the
Rust side. The library has no third-party dependencies: the lexicon and
the maps are embedded with `include_str!` / `include_bytes!`, so a consumer
needs no data file at runtime.

## Data and regeneration

`maps/` holds every binary map the engine embeds. `data/` holds the Kotus
sources plus the committed build inputs; both directories have their own
README with a per-file inventory and provenance.

The engine-driven tables are regenerated **from the project itself**, with no
sqlite database and no Python:

```bash
cargo run --release -p torille-tools --bin gen_all_forms
cargo run --release -p torille-tools --bin gen_adj_degrees
cargo run --release -p torille-tools --bin gen_stem_map_noun
cargo run --release -p torille-tools --bin gen_stem_map_verb
cargo run --release -p torille-tools --bin gen_derived
cargo build -p torille
```

`tools/build/compare_engine_outputs.py --strict` checks the committed tables
against a fresh engine run (0 lost / 0 new is the current baseline). The
lexical exceptions the engine does not derive — the paradigm catalogue,
proper-name cases, the `-oi-` plural set, the class-6/15 tables — are frozen
reference data; their generators live in [`tools/oracle/`](tools/oracle/README.md)
and need Omorfi/Voikko.

A clean clone builds from the tracked files alone; this was verified by
cloning `HEAD`, building `--workspace --all-targets` and running the test
suite with no untracked data. See [`data/README.md`](data/README.md) and
[`FIX_GENGAPS.md`](FIX_GENGAPS.md).

## Command-line tools

Build all drivers with `cargo build --release -p torille-tools`. The full
inventory is in [`tools/README.md`](tools/README.md).

| binary | purpose |
|---|---|
| `inflect` | inflect a lemma and print every form with confidence and reading |
| `lookup` | build the reverse index and report what a word is a form of |
| `check` | compound-aware spelling check |
| `check_normal` | the per-keystroke gate the wasm demo uses |
| `check_syntax` | grammar and punctuation diagnostics with corrections |
| `check_style` | opt-in style suggestions (never an error) |
| `derive` | productive derivations of a base word |
| `hyphenate` | permitted Finnish line breaks, with `--json` offsets |
| `suggest_correct` | ranked suggestions for a misspelling |

`inflect` is the quickest way to inspect a paradigm:

```text
$ cargo run --release -q -p torille-tools --bin inflect -- talo
talo  class 1  substantiivi

## plain (160)
talo                        plain    noun nominative singular
taloa                       plain    noun partitive singular
...
```

The transcript is abridged; the full paradigm has 160 plain forms, including
the possessive and comitative columns.

The three confidence sections are separate on purpose: suggestions, accepted
forms, and merely tolerated forms. `lookup` builds the whole reverse index
(about 17 million words) once, then answers instantly; bases are ranked by
`ClassPrior`, the same ranking the Python binding uses.

Generators (`gen_*`) and the verification harness are described in
[`tools/build/README.md`](tools/build/README.md) and
[`tools/README.md`](tools/README.md).

## Verification

The engine is judged against two external authorities, never against itself:
**Omorfi** decides which forms exist, **Voikko** decides which are real. The
harness checks both directions — a real form not offered is a recall bug, and
a `plain` form that is not real is a precision bug — and reports
`confirmed`, `missing`, `not_plain`, `mistagged`, `illegal` and `unsupported`.
The full design, baselines and known defects are in
[`docs/harness.md`](docs/harness.md).

Before a change lands:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
python3 tools/test_binding_agrees_with_binary.py
python3 tools/test_reverse_lookup_recall.py
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/test_illegal_evidence.py
```

`cargo test` at the repository root tests only `torille` and silently skips
the other targets; always pass `--workspace`. `cargo fmt --check` is **not** a
gate: `src/expand/mod.rs` and `src/expand/build.rs` are hand-formatted, and
the generated nominal modules are not rustfmt-clean. Check the files you
touched instead.

For a class-level defect, the report tools turn findings into a change:

```bash
python3 tools/report_by_class.py -n 5 -j 8 --split reports/classes
python3 tools/summarize_class_report.py reports/noun-classes.json
```

Rank classes by `illegal / confirmed`, not by raw count — the largest classes
simply have the most forms. `reports/` is gitignored and regenerable.

## The Python binding

`bindings/` builds a PyO3 module called `torille` with one class, `Engine`:

```python
from torille import Engine

e = Engine()
e.inflect("kissa")                   # every form, with named fields
e.inflect("kissa", klass="9", pos="substantiivi")
e.lookup("kauppa")                   # [('9*B', 'substantiivi')]
e.words("substantiivi")              # every lemma of that POS with its class
e.hyphenate("tietokoneohjelmointi")  # points with byte/char offsets + source
```

The class argument is `klass`, not `class`, because Python does not accept a
reserved word as a keyword argument. A `klass` is enough to inflect a word the
sanalista has never seen; without one, an unknown word is refused because
there is nothing to fall back on. `inflect` returns objects with named fields
(`pos`, `case`, `number`, ...) rather than the binary's printed tag words.

Build it with:

```bash
(cd bindings && env -u CONDA_PREFIX maturin develop --release)
```

`env -u CONDA_PREFIX` is required because maturin refuses to run when both
`VIRTUAL_ENV` and `CONDA_PREFIX` are set. Always use `--release`: inflection
and data load are several times faster than in a debug build. The binding is
needed for the verification harness; the engine and the build generators do
not need it.

## Hyphenation

`src/hyphenation/` combines the MIT-licensed school pattern set with the
engine's compound structure, then applies Finnish typographic policy (minimum
fragments, no inter-vowel breaks except at a compound boundary or an explicit
exception). It is a separate layer: `check` and `suggest` do not call it.

The Rust API is `torille::hyphenation::{positions, positions_with, split}`;
the wasm export is `hyphenate`, and `Engine.hyphenate` exposes the same points
to Python. Soft hyphens are preserved and returned as explicit
`soft-hyphen` breaks. See [`docs/hyphenation.md`](docs/hyphenation.md).

## License

The engine and the shipping surfaces — `src/`, `tests/`, `benches/`,
`bindings/`, `wrapper/`, the wasm crates, the demo server and the browser
extensions — are dual-licensed under **MIT OR Apache-2.0**. Pick either; the
texts are in [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

The tooling in `tools/` is also **MIT OR Apache-2.0**, except the files tied to
Omorfi or Voikko. Those oracle-bridge and harness files are
**GPL-3.0-or-later**; see [tools/LICENSE-GPL-3.0](tools/LICENSE-GPL-3.0) and
[tools/README.md](tools/README.md) for the exact list. The GPL applies to the
oracle bridge, not to the engine or to source generated by the tools.

`data/` is not covered by either source license. The Kotus material has its own
terms; see [data/LICENSE](data/LICENSE) and [NOTICE](NOTICE).

## Documentation

- [`docs/syntax_checker.md`](docs/syntax_checker.md) — grammar diagnostics,
  severity policy, corrections, evaluation and known limitations.
- [`docs/style_checker.md`](docs/style_checker.md) — the opt-in style layer:
  curated tables, repetition and long-sentence rules, UI and baseline.
- [`PLAN_SYNTAX_REFACTOR.md`](PLAN_SYNTAX_REFACTOR.md) — a phased
  modularisation plan for the syntax layer, checker and extension content.
- [`PLAN_STYLE.md`](PLAN_STYLE.md) — the design for a separate opt-in style
  layer: suggestion-only diagnostics, curated tables and a panel UI.
- [`PLAN_GRAMMAR_STYLE_PERF.md`](PLAN_GRAMMAR_STYLE_PERF.md) — the grammar and
  style performance plan: one morphology pass with cached case information,
  cold-path candidate pruning, sentence-level incremental checks and worker
  chunking.
- [`docs/harness.md`](docs/harness.md) — the verification harness, baselines
  and the table of known defects.
- [`docs/hyphenation.md`](docs/hyphenation.md) — patterns, compound adapter,
  typographic policy and offsets.
- [`docs/lexicon.md`](docs/lexicon.md) — the materialised noun lexicon oracle.
- [`data/README.md`](data/README.md) — every file in `data/`, its origin and
  what regenerates it.
- [`maps/README.md`](maps/README.md) — the embedded maps and their formats.
- [`tools/build/README.md`](tools/build/README.md) — the engine-driven
  generators and the comparison gate.
- [`tools/oracle/README.md`](tools/oracle/README.md) — the GPL reference
  generators.
- [`FIX_GENGAPS.md`](FIX_GENGAPS.md) — what still keeps a generator on an
  oracle, with measurements and exit criteria.
- [`DATA_CLEANUP_PLAN.md`](DATA_CLEANUP_PLAN.md) — the `data/` cleanup record.
- [`PLAN.md`](PLAN.md) / [`PLAN_REFACTOR.md`](PLAN_REFACTOR.md) — the stem
  refactor and its outcome.
- [`TODO.md`](TODO.md) — known gaps, ordered by user-facing impact.
