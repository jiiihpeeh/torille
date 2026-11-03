# AGENTS.md

Guidance for AI coding agents working in this repository. It describes what the
code does, how the pieces fit, and the rules that keep it correct. Read this
first, then the deeper docs it points at.

`torille` is a **dictionary-driven Finnish spell checker and morphology engine**
written in Rust. It answers two questions:

1. **Is this word correctly spelled?** (`check`)
2. **If not, what did the user mean?** (`suggest`)

It is not statistical. The Kotus *sanalista* supplies lemmas, the Kotus
paradigm catalogue turns each lemma into inflected forms, and unknown words fall
back to compound analysis. Every form carries a grammatical reading and a
`Confidence`; only confidently-built forms may be offered as corrections.

Deeper docs (read these when the topic comes up, do not duplicate them):

- `README.md` — the full narrative, tools, reranker slices, measured numbers.
- `docs/harness.md` — the Omorfi + Voikko verification harness, finding kinds,
  and the current known defects (A/B/C/D).
- `docs/lexicon.md` — the materialised noun lexicon used as the harness oracle.
- `docs/hyphenation.md` — line-break hyphenation: patterns, compound adapter,
  policy, offsets and exceptions. The plan is `PLAN_HYPHENATION.md`.
- `TODO.md` — known gaps ordered by user-facing impact.
- `PLAN.md` / `PLAN_REFACTOR.md` — the stem refactor and its outcome.
- `PLAN_COMPOUND.md` — the compound engine: what the spec asks for, what
  already exists, and the phases for the inflection-mode/compound-kind model.

## Repository map

Cargo workspace, root package `torille`, members declared in `Cargo.toml`:

| path | crate | role |
|---|---|---|
| `/` (`src/`) | `torille` | the engine — the only crate with the real logic |
| `tools/` | `torille-tools` | CLI drivers + `tools/*.py` verification harness |
| `bindings/` | `torille-py` | PyO3 extension `torille` (used by the harness) |
| `wrapper/` | `wrapper` | hunspell-shaped C ABI over `torille::suggest` |
| `torille-wasm/` | `torille-wasm` | wasm-bindgen bindings + per-textarea state |
| `torille-wasm-leptos/` | `torille-wasm-leptos` | Leptos front-end for the wasm demo |
| `extensions/firefox/`, `extensions/chromium/` | — | browser extensions (JS, not Rust) |

```
src/
  lib.rs              is_vowel, embedded PARADIGM_CATALOGUE + SANALISTA
  kotus.rs            sanalista + inflection-code parser; `Lemma`, `Inflection`
  spec.rs             paradigm-catalogue parser (`Catalogues`)
  tags.rs             Tags/Form/Confidence — the grammatical reading
  case.rs             declarative case model (docs/morfologia.yaml is its copy)
  compound.rs         split a word into components: kind, per-part mode, explain
  derivation.rs       productive suffix rules (-ja, -minen, -nen, ...)
  wordset.rs          compact concatenated word set (u32 offsets)
  pronoun.rs          personal pronouns (a closed paradigm)
  tokenise.rs         word/separator tokeniser for the wasm API
  sentence.rs         sentence spans and token sentence/paragraph context
  syntax.rs           on-demand clause/role analysis over the morphology engine
  hyphenation/        Finnish line-break hyphenation (patterns + policy)
  gradation.rs        consonant gradation ("*X" code) strong<->weak
  harmony.rs          vowel-harmony primitives for the hand-written engines
  suggest/            check / suggest / find-bases wrappers
    mod.rs            public re-exports, `check`, `classify`
    known/            lemma set, compound detector, stem probes, lemma suggestions
      mod.rs, compound.rs, derived.rs, stems.rs, lemmas.rs
    candidates.rs     `suggest`, `suggest_detailed`, the pipeline
    casing.rs         suggestion case restoration + case-validity policy
    state.rs          `check_fast`, `suggest_with_state`
    levenshtein.rs    bounded edit distance + keyboard tie-break
  grammar/            THE TYPE-DRIVEN (heuristic) ENGINE
    noun.rs           noun cases the 8-form catalogue omits; stems, Licence
    verb.rs           verb forms the catalogue omits
    stem.rs           class -> stem readings; gradation grades
    irregular.rs      members whose model word cannot stand in for them
    harmony.rs        ending harmonisation
  expand/             THE PIPELINE + HAND-WRITTEN PER-CLASS ENGINES
    mod.rs            inflect_in: build a form, then read it back
    inflect.rs        slot/stem machinery (class stem, plural stem, spellings)
    align.rs          align a slot's form onto a stem
    slot.rs           the catalogue slot table
    sound.rs          sound changes + phonotactic filter (is_phonotactic)
    clitic.rs         -ko -kin -kaan -han -pa
    compound.rs       compound expansion (class 50/51)
    derived_forms/    classless heads, numerals, degrees, productive suffixes
    build.rs          reverse index (`FormBuilder`) + `ClassPrior`
    find_bases.rs     four-phase "which lemma is this a form of?" search
    lexicon_index.rs  compile-time sorted head/tail index over the lexicon
    split.rs          first-occurrence `dedup`
    verbs/            hand-written per-class VERB engines (class_52..78)
    nominals/         generated per-class NOUN engines (class_1..101)
build.rs              compile-time codegen driver; `build/` holds the writers
data/                 committed Kotus sources + generated SQLite oracles
data/hyphenation/     vendored MIT school patterns + exception table
rules/                extracted per-class rule JSON (input to codegen)
tools/*.py            the verification harness
docs/                 harness.md, lexicon.md, morfologia.yaml, hyphenation.md
```

## How it works

### The two data files (embedded, no runtime files)

- `data/nykysuomensanalista.csv` — 104k lemmas, tab-separated:
  `lemma  homonymy  part_of_speech  inflection_info`. The class column carries
  `9`, `9*B` (class + gradation), `9, 50` (two classes), `(21)` (parenthesised).
- `data/kotus-tyypit.txt` — the paradigm catalogue. Each class is a number and
  a model word followed by **8 forms in fixed order**: singular nominative,
  genitive, partitive, illative, then the same four plural. Case and number are
  read off a slot's **position**, not a label.

Both are `include_str!`'d (`lib.rs`), so the shipped crate needs no data files.

`data/precomputed/{derived_{by_form,nominal,modifiers},pronoun_forms}.bin` are
the exception to the build-script writers below: those tables are built with the
engine (`expand::derived::compute`,
`expand::check_normal::compute_pronoun_forms`), which a build script cannot
call. Run `cargo run --release -p torille-tools --bin gen_derived` after a
`RULES`, sanalista or pronoun/class change and commit the blobs;
`expand::derived::tests::the_blobs_match_compute` and
`expand::check_normal::tests::the_pronoun_blob_matches_compute` fail if they
drift.

`data/precomputed/oi_plural_lemmas.bin` is the one oracle-derived table: the
class-10 lemmas whose `ok='both'` paradigm has the alternative `-oi-` plural
(`ampuja` -> `ampujoita`). It is lexical, not derivable (`alkaja` is class 10
and has no such plural), so `tools/build_oi_plural_lemmas.py` reads it off the
rebuilt oracle and writes the sorted set that `expand/nominals/class_10.rs`
embeds.

### Build-time codegen (`build.rs` + `build/`)

Parsing the 2.8 MB sanalista at every process start costs ~31 ms and ~420k
allocations. `build.rs` is the driver; `build/sanalista.rs` parses and writes
the lexicon blob, `build/front_coded.rs` writes the set/map blobs, and
`build/huffman.rs` writes the `HST1` proper-name set. It emits
`data/precomputed/sanalista.bin`: 8-byte per-row records (headword,
length, POS code, inflection start), the headword strings, `u16`
inflections, small POS/homonym side tables, and the two sorted orders
(`WORD_ORDER` by word, `TAIL_ORDER` by reversed word). **Nothing here is
built at run time.** `kotus.rs` embeds the blob with `include_bytes!` and
binary-searches `WORD_ORDER` for `by_word`; there is no phf word index and no
second copy of the words. The noun/verb stem maps and the form subset are
front-coded blobs too; the shared blob/endian/front-coded-key readers live in
`src/map_reader.rs`. Only the small tables (connegatives, `-ksi` adverbs,
adjective degrees, connectives) are also blobs, not phf literals.

### The inflection engine has two halves

- **`grammar/`** — the type-driven / "heuristic" engine. It reads the class
  model word and its genitive, works out stems, applies gradation and harmony,
  and fills in the cases the 8-form catalogue omits. It is deliberately
  *over-complete*: a class holds words that disagree (`ovi`/`joki`), so it emits
  both candidate stems and marks each reading with a `Licence`.
- **`expand/nominals/` and `expand/verbs/`** — hand-written per-class engines.
  Each class supplies its own stems and endings rather than inferring one from a
  model word.

The two are merged in `expand::inflect_in` by a per-class switch:

- `nominals::dispatch` routes a class string to its module.
- If the class is in `nominals::AUTHORITATIVE_CLASSES`, the generated forms
  **replace** the heuristic output; otherwise they are **additive** (joined to
  it). A class is promoted to authoritative only once it is verified against the
  noun lexicon. Same shape for verbs via `grammar::is_verb_class` +
  `verbs::dispatch`.

### The nominal class engines

`expand/nominals/` is the single generated per-class tree, ported onto the
verb-side shape (`Decl` + `decline` + a builder). The old parallel
`nominals-refactored/` tree and the `#[path]` swap machinery were retired on
2026-10-24: every class now uses the ported implementation, and the old
per-class files were deleted.

The safety net for that port was
`expand::tests::the_two_nominal_trees_agree_form_for_form`, which compared the
two trees form-for-form. With one tree the gate has done its job and is gone;
what guards the classes now is their own `mod tests`, the fixture and
`tests/catalogue.rs`. `expand::tests::a_word_without_possessives_gets_no_possessive_column`
still runs over the fixture and asserts the contract directly (it used to run
it against both trees).

### `expand::inflect_in` — the pipeline

Called for every lemma/class; short-circuits in order:

1. `try_irregular` — classes 101/99 and hand-spelled paradigms (`minä`,
   `aprilli`) supersede everything.
2. `try_compound_marker` — class 50/51 compounds are inflected part by part.
   Class 50 with no split falls back to its head's class; class 51 deliberately
   does not (its semantics are the opposite). A class-50 tail may be a bound
   combining form (`‑kulmio` in `suorakulmio`, hyphen stripped), the
   nominative plural of a lemma (`vanhemmat` in `isovanhemmat`, resolved to
   `vanhempi`; only the plural cells are emitted), or a compound of its own
   (`viinimarja` in `mustaviinimarja`). A class-51 *head* may be a participle
   (`särkynyt` in `särkynytsydän`, inflected with class 47) or the nominative
   plural of its lemma (`isot` in `isotaivot`, `pitkät` in `pitkäthousut`),
   which makes the compound plural-only.
3. Heuristic engine over the catalogue model (`inflect_from`, with `spellings`
   covering length compensation and epenthetic vowels).
4. Hand-written `nominals`/`verbs` modules (authoritative or additive, above).
5. `dedup`.

A sanalista row with **no class** is a compound (56 486 rows): Kotus omits the
type because the word declines like its final component. `inflect_all` runs
`compound_head_forms` (modifier fixed, head's class), the derived `-inen`
adjectives, and `compound_verb_forms` for a verb head (`aliarvioida` is `ali` +
`arvioida`), so `inflect` and the binding reach the whole paradigm. A compound
form takes the **head's confidence**, not a fresh one: a plain head form is
plain, and a repaired one (`väkein` for `väkien`) stays repaired. The reverse
index does **not** include the rows: `FormBuilder` walks the row's classes and
a classless row has none, so only the citation form is indexed — the same limit
as compound analysis in the fast `check`/`suggest`. `expand::head_inflections`
gives the class the head declines as: the head must be a listed **free**
nominal, because a bound head (`‑mielisyys`) is a suffix and an ending never
classifies. `report_by_class.py --classless` buckets the rows under
`(none:<class>)`, so a defect lands on the class engine that produced it.

Public wrappers: `inflect`, `inflect_all` (preserves first-occurrence order and
drops only fully-identical copies), `form_of`/`form_of_in` (filter by `Tags`
with `None` as a wildcard), `forms_of`.

### Confidence and readings

Every generated `Form` carries a `Confidence` (`tags.rs`):

- **`Plain`** — a plain attachment, no repair. These are the only forms safe to
  offer as corrections. The harness judges this line.
- **`Repaired`** — built by repairing the lemma (gradation, epenthesis, an
  alternation read off the genitive). Accepted as a word, **never suggested**.
- **`Guessed`** — a type-rule guess with no catalogue form behind it. Accepted,
  never suggested.

A `Form` also carries one or more `Tags` readings; `Form::union` is the
intersection of what several readings agree on (a dimension absent across
readings means they disagree, not "unknown"). This is what makes `taloni`
legitimately case-less.

### Reverse lookup, check and suggest (`suggest/`)

- `find_bases_with` is the engine-level "which lemma is this a form of?" search.
  It runs four phases and stops at the first that hits: **heads + tails**
  (`LexiconIndex` binary-search ranges), **prefix-3 walk**, **prefix-2 walk**,
  **short-lemma scan**. It does not build the full reverse index (~27 s); it
  searches a bounded neighbourhood.
- `FormBuilder::build` is the opposite direction: it inflects the *whole*
  lexicon into the in-memory reverse index (`known` / `suggestible` / `lemmas`
  word sets, `analyses`, `first_base`/`more_bases`). `ClassPrior` ranks a word's
  bases by how many lemmas the sanalista gives their class.
- `check` → `CheckOutcome::{Lemma, Inflected, Unknown}`. `check_fast` is the
  per-textarea fast path: lemma-set and learned-proper-noun hits return
  immediately, everything else falls through to `find_bases_with`. Note compound
  analysis is **not** wired into the fast `check`/`suggest`; the full path is in
  `tools/bin/check.rs`.
- `suggest` / `suggest_detailed` rank candidates by Levenshtein (with Damerau
  transpositions and an extracted edit script). A small `KEYBOARD_ADJACENT` table
  adds a tie-break bonus to `existing_score` for distance-1 keyboard typos. The
  final sort is: `edit_distance` asc → `existing_score` desc → word asc.
- `suggest_with_state` is the per-textarea wasm entry point.

### Reranker (`rerank/`) — scaffolding, off by default

An optional two-role neural layer. `Pipeline::disabled()` / `NeuralConfig`
defaults are off, so with no model loaded behaviour is byte-identical to plain
`suggest`. Contract: a broken or missing model must **never** make the engine
unusable — it falls back to the deterministic ranking. `mlp.rs` is the Model-A
forward pass (raw logits, query features computed once, no per-candidate
allocation); it mirrors `tools/train/model.py` and is pinned by
`tools/train/parity_test.py`. Training is pure numpy; see README.

### Other layers

`gradation.rs` (strong↔weak consonant alternation), `harmony.rs` +
`grammar/harmony.rs` (front/back vowel harmony), `expand/clitic.rs` (post-
inflectional particles — no gradation), `derivation.rs` (new lexemes, not
inflections), `compound.rs` (component analysis: `CompoundKind`, the per-part
`ComponentInflection` mode, and `explain`), `pronoun.rs` (closed paradigm).

## Public API (crate root)

`check`, `suggest`, `suggest_detailed`, `levenshtein_with_ops`, `EditOp`;
`compound::{analyze, explain, Analysis, Compound, Component, CompoundKind,
ComponentInflection, Explanation, Action}`;
`inflect`/`inflect_all`/`form_of`/`forms_of` under `expand`; `sanalista`,
`parse_sanalista`, `read_sanalista`, `Sanalista`, `Lemma`, `Inflection`;
`Tags`, `Form`, `Confidence`; `sentence::{analyze, spans, Analyzer, TokenContext,
SentenceSpan}`; `syntax::{analyze, SyntaxAnalysis, Clause, TokenSyntax,
SyntacticRole, RoleConfidence}`; the `rerank` traits/types.

## Build, test, lint

```bash
cargo build --workspace                 # all crates
cargo build --release -p torille-tools  # the CLI drivers
cargo test --release --workspace        # the whole suite
cargo test -p torille                   # engine only (lib + integration)
```

The gates before a change lands (from README):

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release --workspace
python3 tools/test_binding_agrees_with_binary.py
python3 tools/test_reverse_lookup_recall.py
python3 tools/test_experimental_features.py
python3 tools/test_illegal_evidence.py
```

`test_illegal_evidence.py` generates the raw (`--raw-illegal`) 5-lemma report,
so it is slower than the other Python gates; it is the gate for the curated
oracle exceptions in `data/oracle_exceptions.json`.

The Python binding (needed for the harness, not for the engine):

```bash
(cd bindings && env -u CONDA_PREFIX maturin develop --release)
```

`env -u CONDA_PREFIX` is required because maturin refuses to run when both
`VIRTUAL_ENV` and `CONDA_PREFIX` are set. Always `--release`: inflection is
~6x faster and the data load is ~7x faster.

### The verification harness (`tools/*.py`)

torille is judged against **two external authorities**, never against itself:
**Omorfi** decides which forms *exist*, **Voikko** decides which are *real*.
The confirmed set is `ok='both'`. Both directions are checked — recall (a real
form not offered) and precision (a `plain` form that is not real) — because both
are spell-checker bugs. Finding kinds: `confirmed`, `missing`, `not_plain`,
`mistagged`, `illegal`, `unsupported`. Full detail in `docs/harness.md`.

```bash
python3 tools/check_word_against_omorfi.py kissa       # strict, per word
python3 tools/report_by_class.py -n 5 -j 8 --split reports/classes
python3 tools/summarize_class_report.py reports/noun-classes.json
```

The noun half reads the materialised oracle in `data/kotus_nomines.sqlite`
(`ok='both'`) instead of live Omorfi/Voikko; see `docs/lexicon.md`. `reports/`
is gitignored and generated.

## Conventions and invariants

- **Keep the license split.** The engine, shipping surfaces, and most tools
  are `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`). Only the tool
  files tied to Omorfi or Voikko are `GPL-3.0-or-later`
  (`tools/LICENSE-GPL-3.0`; the exact list is in `tools/README.md`). `data/`
  is separate and stays under its own terms (`data/LICENSE`). A tool's output
  is not relicensed by the tool, but do not copy the GPL oracle-harness code
  into the engine or a binding.
- **Do not hand-edit generated files.** `expand/nominals/` is produced by
  `tools/extract_class_rules.py --rust`; `src/expand/nominals/mod.rs` says so at
  the top. Regenerate, or fix `grammar/` / `expand/inflect.rs`. Generated-module
  lint noise is scoped with one `#[allow(...)]` on `mod nominals;` in
  `expand/mod.rs`; hand-written code is warning-free.
- **Rejected dictionary rows.** `build.rs`'s `REJECTED_LEMMAS` is the one place
  a sanalista row can be removed outright. It is filtered from the lexicon blob,
  the fast form set, and the stem maps, so the row and its paradigm disappear
  from `check`, `suggest`, and the wasm. Keep the list small, documented, and
  regenerate the precomputed blobs after changing it.
- **Plain vs offered.** A `Repaired`/`Guessed` form is accepted as a word but
  must never reach the suggestion set. `tests/catalogue.rs` pins this
  (`a_repaired_form_is_not_suggestible`,
  `a_form_the_class_does_not_license_is_never_suggested`).
- **Tests live beside the code** in a `mod tests` at the bottom of each file,
  not in a separate tree. `src/expand/inflect.rs` is the deliberate exception —
  its output is tested through the pipeline in `expand/mod.rs`.
- **Data-driven tests are the regression net.** `tests/catalogue.rs` runs every
  catalogue form of every class member and every fetched Wiktionary paradigm
  (with a `GAPS` list whose entries carry staleness checks, so a fix turns a gap
  into a red test). Hand-written word lists are not the guard.
- **The `GAPS`/`AUTHORITATIVE_CLASSES`/`DISPATCH` lists are commit-time
  switches,** not trivia: promoting a class to authoritative, or porting a file
  with `#[path]`, changes which engine owns that class. Adjust the fixtures in
  the same commit.
- **Features:** `generated-nominals` and `generated-verbs` are both default. The
  generated per-class engines are behind them.
- **No shortcuts in the inflection detection.** The right place for a
  missing-form bug is the inflection engine (`src/expand/`,
  `src/expand/verbs/`, `src/expand/find_bases.rs`, `src/grammar/`), never
  another suffix-stripper entry in `is_known_word` in `src/suggest/known/`.
  Each suffix added there is a bypass around the actual paradigm table
  / gradation / inflection code, and the bypass does not generalise:
  if `tiedä → tietää` is missing, the fix is to make `find_bases_with`
  find `tietää` as a base and `inflect_all("tietää")` emit `tiedä`, not
  to teach `is_known_word` the `d → t` pair and a new `"-taa"/"-tää"`
  suffix list. The fast first-pass stripper is a lemma-HashSet probe
  plus a tiny noun-case suffix strip; verb coverage goes through the
  engine. If a class is broken, fix the class (`grammar/verb.rs`,
  `expand/verbs/`, `expand/nominals*/`) so the bug closes for every
  member of the class, not for one example word.

## Traps (each has bitten before)

- **`cargo test` at the repo root tests only `torille`** and silently skips the
  integration tests and the other crates. Always pass `--workspace`.
- **`cargo fmt` is not clean workspace-wide.** `src/expand/mod.rs` and
  `src/expand/build.rs` are hand-formatted around a few calls. Check the files
  you touched; do not let the formatter rewrite those two.
- **The two oracle libraries have different spellings.** `POSS=SG1` in the omor
  string vs `poss 1sg` in torille's reading; `tools/omorfi_harness.py`'s
  `POSSESSIVE_NAMES_MAP` is the only place they meet. Getting this wrong
  silently halves coverage with no error.
- **`instructive` is both a case and a mood.** Only the binding's named fields
  (or the harness's ordered greedy walk) disambiguate; positional parsing is
  wrong and silent.
- **`check_nouns_with_voikko.py` and `check_verbs_with_voikko.py` are not
  gates** and their exit codes should be ignored — they are historical coverage
  smoke tests whose denominators changed as more forms reached `plain`. The
  strict gate is `report_by_class.py`; see README for why the counts are not
  regressions.
- **Don't count `illegal` by class without dividing by confirmed.** The largest
  classes simply have the most forms; rank by *rate*. The current known defects
  (wrong stems, missing possessives, `taloni`'s missing case) are catalogued in
  `docs/harness.md` and `TODO.md`.

## Where to change what

| Task | Go to |
|---|---|
| a case/stem/gradation rule for a class | `src/grammar/noun.rs`, `src/grammar/verb.rs`, `src/grammar/stem.rs` |
| a class whose model can't stand in | `src/grammar/irregular.rs` |
| the per-class generated engines | regenerate via `tools/extract_class_rules.py`; edit `src/expand/nominals/` |
| slot/stem arithmetic, spellings | `src/expand/inflect.rs`, `src/expand/align.rs`, `src/expand/slot.rs` |
| suggestion ranking | `src/suggest/candidates.rs` |
| edit distance / keyboard tie-break | `src/suggest/levenshtein.rs` |
| "which lemma is this a form of" | `src/expand/find_bases.rs`, `src/expand/lexicon_index.rs` |
| the reverse index / class prior | `src/expand/build.rs` |
| compile-time blob writers | `build.rs`, `build/*.rs` |
| a new verb/noun class engine | `src/expand/verbs/`, `src/expand/nominals*/`, then `dispatch` + `AUTHORITATIVE_CLASSES` |
| the grammar reading model | `src/tags.rs` |
| hyphenation patterns/policy | `src/hyphenation/`, `data/hyphenation/` |
| the wasm/browser surface | `torille-wasm/src/`, `extensions/firefox/`, `extensions/chromium/` |
| verification | `tools/*.py`, `tests/catalogue.rs`, `docs/harness.md` |

## Current known state

- The workspace builds and the test suite is green. The
  `tests/candidate_generation.rs` golden list and `data/rerank/*.jsonl` were
  regenerated together after the keyboard-adjacency tie-break (`b08be1e`)
  moved `koiraa` and `talo`; the test now also allows the documented 0.05
  `existing_score` bonus. The corpus was regenerated from the same first
  300/109 source rows, so the split sizes stay 300/109 and the truncated
  final dev line is gone. Keep the golden list and corpus regenerating
  together.
- The morphology engine has known correctness gaps (recall ~70%, a large
  `illegal` count from stems built on the wrong stem). These are tracked defects,
  not the build status; read `TODO.md` and `docs/harness.md` before touching
  stems or possessives, and never regress the harness baselines.
