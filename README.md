# torille

A Finnish spell checker, morphology engine and on-demand grammar/style checker.
The core answers two questions:

- **is this word correctly spelled?** — `check`
- **if not, what did the user mean?** — `suggest`

It is dictionary-driven, not statistical. The Kotus *sanalista* supplies the
lemmas, a committed paradigm catalogue — seeded once from the Omorfi and Voikko
oracles, then rewritten from the engine's own output until the templates are
self-consistent — supplies each class's slots, and the engine works out which
slots a particular member actually has. Unknown words fall through to compound
analysis and the productive derivation rules.

The same morphology engine also powers an **opt-in grammar and style checker**
(`check_syntax`, `check_style`). It runs only when asked and never on the
keystroke path.

## Quick start

```bash
cargo build --workspace         # the first build fetches maps/*.bin.gz automatically
cargo test --release --workspace

cargo run --release -p torille-tools --bin inflect -- talo
cargo run --release -p torille-tools --bin check -- talo
cargo run --release -p torille-tools --bin hyphenate -- tietokoneohjelmointi
cargo run --release -p torille-tools --bin check_syntax -- "Koira nukkuvat."
cargo run --release -p torille-tools --bin check_style -- "Tällä hetkellä tilanne on rauhallinen."
```

The first build fetches `maps/*.bin.gz` from the orphan `maps` branch through
the `torille-maps` build-dependency; see
[`maps/README.md`](maps/README.md#automatic-fetch-on-first-build) for the
remote, the offline switches, and what happens when the fetch fails.

Both remotes (GitLab `origin` and the GitHub mirror) and the procedure for
keeping the normal and `maps` branches in sync are in
[`docs/remotes.md`](docs/remotes.md).

The wasm demo and the browser extensions are separate builds:

```bash
# wasm package shared by the demo and the extensions
(cd torille-wasm && wasm-pack build --release)

# demo
(cd torille-wasm/demo && npm install && node build.mjs)
cargo run --release -p torille-demo-server      # serves the demo on :8765

# extension
(cd extensions/firefox && npm install && npm run build)   # dist/ and the XPI
(cd extensions/chromium && npm install && npm run build)  # dist/
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
| `Repaired` | built by repairing the lemma (gradation, epenthesis, a genitive alternation). Often real enough to accept, never to suggest. |
| `Guessed` | a type-rule guess with no catalogue form behind it. Accepted, never suggested. |

A form also carries one or more `Tags` readings (POS, case, number, tense,
person, mood, voice, possessive, clitic, polarity). `Form::union` is the
intersection of what several readings agree on, which is why `taloni` is
legitimately case-less.

`suggest` ranks candidates by Levenshtein distance (with transpositions and an
extracted edit script), then the deterministic `existing_score`, then the word.
A small QWERTY-adjacency table breaks distance-1 ties. The known defects —
recall, and `illegal` forms from wrong stems — are tracked in
[`docs/harness.md`](docs/harness.md) and [`misc_md/TODO.md`](misc_md/TODO.md).

## The word wizard

`word_wizard` finds the Kotus inflection class of a word that is not in the
sanalista — a name, a loanword, a dialect word — from a few inflected forms.
It asks the engine what every candidate class would generate and compares the
answers; the class sheet is generated from the engine and checked for
staleness.

```bash
cargo run --release -p torille-tools --bin word_wizard -- mökki \
  --form gen_sg=mökin --form par_sg=mökkiä --form ill_sg=mökkiin
```

The same wizard is the demo page (`torille-wasm/demo/wizard.html`) and the
**Omat sanat** page in both browser extensions. Saved words are stored as the
`torille-user-words` TSV in `localStorage` / `browser.storage.local`; in the
extensions the worker accepts the generated forms and suggests them within two
edits. Class `99` (`--indeclinable`, or the **taipumaton** checkbox) means the
word is accepted as written and not inflected.

The committed sheet is `data/class_examples.tsv`; regenerate it with
`word_wizard --write-sheet data/class_examples.tsv` after a class-engine
change. The design, the tail-weighted tie-break and the measured curve are in
[`misc_md/PLAN_WORD_WIZARD.md`](misc_md/PLAN_WORD_WIZARD.md): adaptive top-1 base class 94.2%
at two answers on the recorded 412-noun sample, 99.8% top-3.

## Grammar and style

Two opt-in layers sit on top of the morphology engine. Neither runs in the
keystroke path, and `style_check` never emits an error or a warning.

- `syntax_check` — grammar and punctuation diagnostics with corrections.
- `style_check` — suggestion-only style observations: repetition, long
  sentences, clichés, filler, wordy phrases and colloquialisms.

`torille::check_all` runs both over **one** morphology pass; `MorphologyCache`
keeps the readings plus the agreed case and number for each surface. A
per-textarea `SentenceCache` keys each grammar unit (a sentence plus the left
context its first word needs) by its text and reuses its diagnostics across
edits, so an edit re-checks only the changed units. On a long text the demo
resolves the morphology in a pool of up to **4 workers**, then runs
syntax/style once over the full document; the Firefox extension does the same
with a `grammarThreads` setting (1–4, default 4).
Chromium's MV3 service worker cannot create nested workers, so it keeps the
sentence-batch fallback. Both paths report progress and cancel on the next
edit. The Gutenberg reader shows the same spans as underlays. In the
extensions the check runs from the context menu (**Tarkista kielioppi**).

Measured on the committed 1,959-word fixture
(`tests/fixtures/perf/page-2000.txt`):

| workload | time |
|---|---:|
| cold morphology, native single-thread | 576 ms |
| cold morphology, native 4-worker | 168 ms |
| warm morphology | 0.50 ms |
| warm syntax analysis | 5.17 ms |
| warm grammar rules | 12.54 ms |
| warm style | 2.57 ms |
| warm grammar + style (full pass) | 18.95 ms |
| one edited sentence (native) | 3.04 ms |
| cold page, release wasm, 4 workers alive | 364–395 ms |
| first check of a browser session (worker start-up) | ~950 ms |

The native rows are the benchmark reference below; the browser rows are the
last recorded wasm measurements. The **≤500 ms per-page target is met** once
the four workers exist: a fresh page takes 364–395 ms, and the first check of a
session pays one ~950 ms start-up for the four worker spawns and wasm
instantiations. The first progress lands at ~110 ms on the full page, so it is
not frozen while the cold pass runs. A precomputed reverse index for the wasm was
measured and rejected on size and coverage (86.6% at 2.1 MB; 95% would exceed
3 MB). See
[`misc_md/PLAN_GRAMMAR_STYLE_PERF.md`](misc_md/PLAN_GRAMMAR_STYLE_PERF.md),
[`misc_md/PLAN_COLD_SEARCH.md`](misc_md/PLAN_COLD_SEARCH.md) and
[`misc_md/PLAN_COLD_READINGS_CACHE.md`](misc_md/PLAN_COLD_READINGS_CACHE.md).

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
    tokenise.rs         word/separator tokeniser shared by the checkers and wasm
    syntax_list.rs      list structure parser used by the syntax checker
    morphology/         cached readings shared by grammar and style: case,
                        number, POS, clitic, possessive
    check.rs            combined grammar + style pass over one morphology result
    syntax/             on-demand clause/role analysis over the morphology engine
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
  tests/                catalogue, candidate generation, hyphenation,
                        syntax/style, Wiktionary
  examples/python/      runnable tours of the Python extension
  bench/                corpora manifest and the recorded reference report
  benches/              the Criterion hot-path benchmarks
  docs/                 harness, lexicon, syntax/style checkers,
                        hyphenation, morfologia, benchmarking
```

The root package is the library and the workspace root. `tools`,
`tools/maps-sync` (the `torille-maps` maps-branch helper), `bindings`,
`wrapper`, the wasm crates and the demo server are members, so
`cargo build --workspace` covers the Rust side. The library has no third-party
dependencies: the lexicon and the maps are embedded as `include_bytes!` blobs
(the committed CSV is also embedded for the public text parser), so a consumer
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
cargo run --release -p torille-tools --bin gen_stem_endings
cargo run --release -p torille-tools --bin gen_lemma_stem_shapes
cargo run --release -p torille-tools --bin gen_derived
cargo build -p torille
```

`tools/build/compare_engine_outputs.py --strict` checks the committed tables
against a fresh engine run. The gate requires 0 lost forms; the current run has
0 lost, with one extra all-forms row and four swapped verb-pair keys as the
recorded residue. The paradigm catalogue is bootstrapped rather than regenerated
from the oracle every time: seed `data/catalogue.csv` once with
`python3 tools/oracle/build_catalogue_csv.py`, then rewrite it from the engine's
own output until the CSV stops changing:

```bash
cargo run -p torille-tools --bin build_catalogue
cargo build
```

The other lexical exceptions the engine does not derive — proper-name cases,
the `-oi-` plural set, the class-6/15 tables — are frozen reference data; their
generators live in [`tools/oracle/`](tools/oracle/README.md) and need
Omorfi/Voikko.

A clean clone builds with no local data files; this was verified by cloning
`HEAD`, building `--workspace --all-targets` and running the test suite after
the automatic `maps` fetch. See [`data/README.md`](data/README.md) and
[`misc_md/FIX_GENGAPS.md`](misc_md/FIX_GENGAPS.md).

## Command-line tools

Build all drivers with `cargo build --release -p torille-tools`. The full
inventory is in [`tools/README.md`](tools/README.md).

| binary | purpose |
|---|---|
| `inflect` | inflect a lemma and print every form with confidence and reading |
| `word_wizard` | find a Kotus class from example forms (with `--write-sheet`/`--check-sheet`) |
| `lookup` | build the reverse index and report what a word is a form of |
| `check` | compound-aware spelling check |
| `check_normal` | the per-keystroke gate the wasm demo uses |
| `check_syntax` | grammar and punctuation diagnostics with corrections |
| `check_style` | opt-in style suggestions (never an error) |
| `syntax_tree` | print the typed syntax tree (`--json`, `--check`, `--order`) |
| `perf_grammar_style` | cold/warm morphology, rules, style and sentence-cache timings |
| `derive` | productive derivations of a base word |
| `hyphenate` | permitted Finnish line breaks, with `--json` offsets |
| `suggest_correct` | ranked suggestions for a misspelling |

`inflect` is the quickest way to inspect a paradigm:

```text
$ cargo run --release -q -p torille-tools --bin inflect -- talo
talo  class 1  substantiivi

## plain (159)
talo                        plain    noun nominative singular
taloa                       plain    noun partitive singular
...
```

The transcript is abridged; the full paradigm has 159 plain forms (plus 3
repaired), including the possessive and comitative columns.

The three confidence sections are separate on purpose: suggestions, accepted
forms, and merely tolerated forms. `lookup` builds the whole reverse index
(about 17 million words) once on the command line, then answers instantly;
bases are ranked by `ClassPrior`. The index is a tool, not a wasm payload:
shipping it to the wasm is the route the cold-page work rejected on size and
coverage.

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
(cd extensions/firefox && npm run typecheck && npm test)
(cd extensions/chromium && npm run typecheck && npm test)
python3 tools/check_docs.py
python3 tools/bench.py selftest
```

`cargo test` at the repository root tests only `torille` and silently skips
the other targets; always pass `--workspace`. `cargo fmt --check` is **not** a
gate: `src/expand/mod.rs` and `src/expand/build.rs` are hand-formatted, and the
per-class nominal modules are hand-formatted rather than rustfmt-clean. Check
the files you touched instead.

For a class-level defect, the report tools turn findings into a change:

```bash
python3 tools/report_by_class.py -n 5 -j 8 \
  -o reports/noun-classes.json --split reports/classes
python3 tools/summarize_class_report.py reports/noun-classes.json
```

Rank classes by `illegal / confirmed`, not by raw count — the largest classes
simply have the most forms. `reports/` is gitignored and regenerable.

## Benchmarks

`tools/bench.py` is the reproducible suite. It does not measure anything
itself: it runs the release tools (`perf_*`, `dump_diagnostics`), normalises
their JSON into one versioned report, hashes the full paradigm of eight canary
lemmas, fingerprints the corpora and the correctness output, and compares two
reports from the same machine. The measurement primitives remain the Criterion
bench (`benches/torille.rs`) and the `perf_*` tools in `tools/src/bin/`.

```bash
python3 tools/bench.py corpora --check       # verify or generate the corpora
python3 tools/bench.py run --profile quick   # ~1 minute, one run per point
python3 tools/bench.py run --profile full    # 7 fresh processes per point
python3 tools/bench.py compare bench/baselines/reference.json reports/bench-full.json
python3 tools/bench.py selftest              # check the comparator
```

One report, `schema: 1`, carries the machine fingerprint, the corpus SHA-256
and word/byte counts, every metric as `min/median/max/runs`, and a
**correctness fingerprint**: the diagnostics counts and hash per corpus plus
the full paradigm hash of eight canary lemmas (`kissa`, `talo`, `havas`,
`live`, `juosta`, `pieni`, `ahkeraliisa`, `pikkuaivot`). A faster wrong answer
fails the comparison, so a morphology regression that the syntax diagnostics
cannot see is still caught. `compare` is same-machine only (the fingerprint
covers CPU, OS, rustc and build profile), uses a 10 % default tolerance on
median timings, and requires the correctness block to match exactly.

`quick` covers `page-2000` and `homonyms`; `full` covers all five corpora
(including the deterministic 8 % typo and all-unknown variants), both the
single-thread and 4-worker shapes, and the reverse-index build. The native
release reference (`bench/baselines/reference.json`, fingerprint
`f762dae2d048b2a6`, 594 metrics) medians:

| metric | median |
|---|---:|
| `page-2000` cold morphology, single-thread | 575.9 ms |
| `page-2000` cold morphology, 4 workers | 168.1 ms |
| `page-2000` warm morphology | 0.50 ms |
| `page-2000` warm grammar + style | 18.95 ms |
| `page-2000` warm sentence cache | 0.76 ms |
| 8 % typo corpus, cold morphology | 621.9 ms |
| all-unknown corpus, cold morphology | 551.9 ms |
| `check` / `suggest` p50 / p95 | 3 µs / 473 µs |
| reverse-index build / size | 23.3 s / 1.23 GB |

These are native release numbers; the browser (wasm) shape is measured
separately and is not part of this report. The commands, the full schema, the
corpus manifest and the methodology are in
[`docs/benchmarking.md`](docs/benchmarking.md); the staged design is
[`misc_md/PLAN_BENCHMARK_SUITE.md`](misc_md/PLAN_BENCHMARK_SUITE.md).

## The Python binding

`bindings/` builds a PyO3 module called `torille` with one class, `Engine`:

```python
from torille import Engine, Luokka, Pos

e = Engine()
e.inflect("kissa")                              # every form, with named fields
e.inflect("kissa", luokka=Luokka(9), pos=Pos.NOUN)
e.lookup("kauppa")                              # [('9*B', 'substantiivi')]
e.words(Pos.NOUN)                               # every classed lemma of that POS with its first class
e.hyphenate("tietokoneohjelmointi")             # points with byte/char offsets + source
```

The class argument is `luokka` -- Finnish for class -- because Python does not
accept a reserved word as a keyword argument. `Luokka(9)` and `Luokka("9*B")`
are the typed spellings; the string form (`luokka="9*B"`, `pos="substantiivi"`)
is still accepted for tools that already speak it. A `luokka` is enough to inflect
a word the sanalista has never seen; without one, an unknown word is refused
because there is nothing to fall back on. `inflect` returns objects with named
fields (`pos`, `case`, `number`, ...) rather than the binary's printed tag
words.

The extension also exposes the check/suggest pair, the morphology and syntax
layers, the grammar/style pass, typography and the class wizard. Runnable tours
of each live in [`examples/python/`](examples/python/README.md); start with
`python3 examples/python/00_showcase.py`.

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
- [`docs/grammar_syntax.md`](docs/grammar_syntax.md) — the complete feature
  reference for the morphology grammar, syntax analysis, AST and every grammar
  diagnostic.
- [`docs/word_order.md`](docs/word_order.md) — the word-order specification:
  clause features, the allow-list and the construction tests.
- [`docs/style_checker.md`](docs/style_checker.md) — the opt-in style layer:
  curated tables, repetition and long-sentence rules, UI and baseline.
- [`docs/benchmarking.md`](docs/benchmarking.md) — the benchmark suite's
  commands, report schema, corpora, methodology and reference baseline.
- [`misc_md/PLAN_BENCHMARK_SUITE.md`](misc_md/PLAN_BENCHMARK_SUITE.md) — the
  suite's staged design and the recorded reference.
- [`misc_md/PLAN_SYNTAX_REFACTOR.md`](misc_md/PLAN_SYNTAX_REFACTOR.md) — a phased
  modularisation plan for the syntax layer, checker and extension content.
- [`misc_md/PLAN_STYLE.md`](misc_md/PLAN_STYLE.md) — the implemented opt-in style
  layer: suggestion-only diagnostics, curated tables and a panel UI.
- [`misc_md/PLAN_GRAMMAR_STYLE_PERF.md`](misc_md/PLAN_GRAMMAR_STYLE_PERF.md) — the grammar and
  style performance plan: one morphology pass with cached case information,
  cold-path candidate pruning, sentence-level incremental checks and worker
  chunking.
- [`misc_md/PLAN_PARALLEL_MORPHOLOGY.md`](misc_md/PLAN_PARALLEL_MORPHOLOGY.md) — the
  morphology-only worker pool: codec, state merge, demo and extension.
- [`misc_md/PLAN_COLD_SEARCH.md`](misc_md/PLAN_COLD_SEARCH.md) — the cold morphology search:
  base ladder, stem-shape filter and the deferred phases.
- [`misc_md/PLAN_COLD_READINGS_CACHE.md`](misc_md/PLAN_COLD_READINGS_CACHE.md) — the negative
  per-lemma form-cache experiment and the stem-map-first reorder.
- [`misc_md/PLAN_SYNTAX_CHECKER.md`](misc_md/PLAN_SYNTAX_CHECKER.md) — grammar and punctuation
  diagnostics, ranking and evaluation.
- [`misc_md/PLAN_SYNTAX.md`](misc_md/PLAN_SYNTAX.md) — the advisory syntax analysis layer
  underneath the checker.
- [`misc_md/PLAN_SYNTAX_AST.md`](misc_md/PLAN_SYNTAX_AST.md) — the typed surface
  tree over `syntax`: node kinds, the pass pipeline and the invariants.
- [`misc_md/PLAN_SYNTAX_AST_REST.md`](misc_md/PLAN_SYNTAX_AST_REST.md) — the remaining
  surface-tree work: nested lists, coordination, the wasm export and checker
  consumers. Phase 5b (wasm `analyze_ast`) and the 5c.1 parity audit landed.
- [`misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md`](misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md) — the
  grammar/syntax fix record: predicate-less phrases, relative closing commas,
  imperative connegative corrections and constituent views.
- [`misc_md/REVIEW_PUNCTUATION_SPEC.md`](misc_md/REVIEW_PUNCTUATION_SPEC.md) — the
  clause-by-clause gap analysis against the programmatic Finnish punctuation
  specification.
- [`misc_md/PLAN_PUNCTUATION_SPEC.md`](misc_md/PLAN_PUNCTUATION_SPEC.md) — the staged
  punctuation plan; indirect-question and vocative commas landed, typography,
  main-clause coordination, relative openings and lists remain.
- [`misc_md/REVIEW_SEMICOLON.md`](misc_md/REVIEW_SEMICOLON.md) — the semicolon
  gap analysis: the `,;` boundary defect, forbidden-semicolon cases, complex
  lists and spacing.
- [`misc_md/PLAN_SEMICOLON.md`](misc_md/PLAN_SEMICOLON.md) — the semicolon slice:
  the boundary fix, suggestion-only misuse, list awareness and the separate
  typography pass.
- [`misc_md/PLAN_GRAMMAR_SYNTAX_REFERENCE.md`](misc_md/PLAN_GRAMMAR_SYNTAX_REFERENCE.md) —
  the review-driven hardening plan for the grammar/syntax feature reference.
- [`misc_md/PLAN_GRAMMAR_SYNTAX_LAYOUT.md`](misc_md/PLAN_GRAMMAR_SYNTAX_LAYOUT.md) —
  the render-safe layout plan for the same reference.
- [`misc_md/PLAN_COMPOUND.md`](misc_md/PLAN_COMPOUND.md) — the compound engine's inflection
  model and remaining work.
- [`misc_md/PLAN_HYPHENATION.md`](misc_md/PLAN_HYPHENATION.md) — the hyphenation milestones
  and the remaining corpus triage.
- [`misc_md/PLAN_WORD_WIZARD.md`](misc_md/PLAN_WORD_WIZARD.md) — the text-first word wizard:
  class sheet, sparse answer maps, nominal and verb inference, the tail prior,
  the browser pages and the measured oracle curve.
- [`docs/harness.md`](docs/harness.md) — the verification harness, baselines
  and the table of known defects.
- [`docs/hyphenation.md`](docs/hyphenation.md) — patterns, compound adapter,
  typographic policy and offsets.
- [`docs/lexicon.md`](docs/lexicon.md) — the materialised noun lexicon oracle.
- [`docs/remotes.md`](docs/remotes.md) — the two remotes and the branch/`maps`
  sync procedure.
- [`data/README.md`](data/README.md) — every file in `data/`, its origin and
  what regenerates it.
- [`maps/README.md`](maps/README.md) — the embedded maps and their formats.
- [`tools/build/README.md`](tools/build/README.md) — the engine-driven
  generators and the comparison gate.
- [`tools/oracle/README.md`](tools/oracle/README.md) — the GPL reference
  generators.
- [`misc_md/FIX_GENGAPS.md`](misc_md/FIX_GENGAPS.md) — what still keeps a generator on an
  oracle, with measurements and exit criteria.
- [`misc_md/DATA_CLEANUP_PLAN.md`](misc_md/DATA_CLEANUP_PLAN.md) — the `data/` cleanup record.
- [`misc_md/PLAN.md`](misc_md/PLAN.md) / [`misc_md/PLAN_REFACTOR.md`](misc_md/PLAN_REFACTOR.md) — the stem
  refactor and its outcome.
- [`misc_md/TODO.md`](misc_md/TODO.md) — known gaps, ordered by user-facing impact.
