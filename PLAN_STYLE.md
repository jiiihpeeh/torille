# PLAN_STYLE — the opt-in style layer

Status: **Phase 0–7 landed.** The style layer is implemented end to end:
`src/style_check/`, the curated tables, the wasm export, the `Tarkista tyyli`
command in both extensions, the Leptos panel, the evaluation corpus and
`docs/style_checker.md`. The release is `0.7.0`; the extensions are `1.2.0`.
This is the separate plan that `PLAN_SYNTAX_CHECKER.md` phase 6E asked for
before any style rule lands. It does not change the grammar checker, the
suggestion pipeline or the keystroke path.

The grammar checker answers “is this written Finnish correct?”. The style
layer answers a different, optional question: “is this sentence needlessly
repetitive, wordy or clichéd?”. A style finding is never an error. It is an
opt-in observation with a neutral Finnish label, and the user can ignore it
without changing the text.

## 1. Why a separate layer

Grammar and style fail differently:

| | grammar checker | style layer |
|---|---|---|
| question | is the form allowed? | is the wording optional-but-weak? |
| default | on demand, context menu | opt-in, separate command |
| severity | `Error`, `Warning`, `Suggestion` | `Suggestion` only |
| false positive | a bug | expected occasionally; the rule is advisory |
| correction | a defensible case/form edit | only a curated, safe phrase replacement |
| data | morphology, rection, punctuation | small curated phrase/word tables and metrics |
| corpus gate | zero `Error` on clean rows | no `Error`/`Warning`, tight suggestion budget |

Mixing the two would make the grammar checker noisy and would make every style
preference look like a correctness claim. The layers share the tokeniser, the
sentence spans and the diagnostic primitives, but not the rule pass.

## 2. Non-goals

Do not do any of these in this plan:

- no grammar rule, no change to `src/syntax_check/` rule behaviour;
- no spell checking, inflection suggestion or `Confidence` change;
- no statistical or model-based style scoring;
- no English style advice, no Oxford-comma policy, no “plain English” rules;
- no register policing: the layer does not tell the user that their dialect or
  text type is wrong;
- no automatic rewrite of a subjective sentence. A correction exists only when
  the data table names a safe replacement;
- no always-on underline. Style findings live in a panel; after the explicit
  check the same spans are also painted blue, and the paint clears on the next
  edit;
- no keystroke-path call. The style pass is on demand, like the grammar check;
- no new runtime data files. The tables are build-time blobs, as with the rest
  of `data/`.

## 3. Design principles

1. **Suggestion only.** `StyleDiagnostic` never carries `Severity::Error` or
   `Severity::Warning`. A test asserts this for every rule.
2. **Opt-in and reversible.** The default UI does not show style findings.
   A context-menu command opens a separate `Tyyli` panel; closing it changes
   nothing in the text.
3. **Table first, metric second.** Exact curated phrases and words are
   high-confidence. Repetition and sentence length are metrics with a medium
   confidence and a conservative threshold. Subjective “good writing” is not a
   rule.
4. **No overlap spam.** A sentence gets at most one long-sentence finding, and
   word-level findings suppress the long-sentence finding for that sentence.
   A phrase-table hit covers its span, so the repetition rule skips it.
5. **Neutral Finnish labels.** `toistuva sana`, `pitkä virke`,
   `kulunut ilmaus`, `täytesana`, `monisanainen ilmaus`, `arkikielinen
   ilmaus`. No “huono”, “väärä” or “älä”.
6. **Deterministic order.** Findings sort by span start, then kind name, then
   message. The fixture pins the order.
7. **Easy to remove.** One row in a CSV removes one phrase; one `match` arm
   removes one rule. No rule hides behind a suffix heuristic.

## 4. Architecture

```text
src/
  syntax_check/diagnostic.rs   shared Severity/DiagnosticConfidence/Edit/Correction
  syntax_check/                grammar rules
  style_check/
    mod.rs                     check(), StyleAnalysis, rule ordering
    diagnostic.rs              StyleKind, StyleDiagnostic, label keys
    lexicon.rs                 embedded phrase/word table reader
    repetition.rs              content-lemma repetition
    length.rs                  long-sentence metric
```

The shared primitives stay in `syntax_check/diagnostic.rs` for v1. If a later
phase adds a third consumer, move them to `src/diagnostic.rs` and re-export
them from `syntax_check` so existing paths stay valid. That move is not part
of this plan’s v1.

`src/style_check/` never imports a grammar rule and never calls
`syntax_check::check`. It may use:

- `crate::tokenise` for tokens;
- `crate::sentence` for sentence spans;
- `crate::syntax` for lemmas/readings and finite-predicate checks when a rule
  needs morphology;
- `crate::syntax_check::diagnostic` for the shared diagnostic primitives.

### 4.1 Public API

```rust
pub use style_check::{StyleAnalysis, StyleDiagnostic, StyleKind, check};

pub struct StyleAnalysis {
    pub diagnostics: Vec<StyleDiagnostic>,
}

pub fn check(text: &str) -> StyleAnalysis;
pub fn check_tokens(tokens: &[Token]) -> StyleAnalysis;
```

`src/lib.rs` adds `pub mod style_check;`. Callers use
`crate::style_check::{check, StyleAnalysis, StyleDiagnostic, StyleKind}`; do
not re-export style types at the crate root, so a reader cannot mistake
`StyleDiagnostic` for a grammar diagnostic.

`check` tokenises and calls `check_tokens`. The caller owns any cache; the
style layer’s morphology use is small enough that the first version can call
`syntax::analyze` on demand. If that shows up in a profile, add a
`check_with_cache(tokens, &mut AnalysisCache)` the same way `syntax` did.

### 4.2 Diagnostic shape

```rust
pub enum StyleKind {
    Repetition,
    LongSentence,
    Cliche,
    Filler,
    Wordy,
    Colloquial,
}

pub struct StyleDiagnostic {
    pub kind: StyleKind,
    /// Always `Severity::Suggestion`; kept for the shared wire shape.
    pub severity: Severity,
    pub confidence: DiagnosticConfidence,
    pub span: Range<usize>,
    /// Stable key, e.g. `style_repetition`.
    pub message: &'static str,
    /// The matched phrase or lemma, for debugging and the panel.
    pub expected: Option<String>,
    /// Empty unless the table row names a safe replacement.
    pub corrections: Vec<Correction>,
}
```

`StyleKind::name()` returns stable strings:

| kind | name |
|---|---|
| `Repetition` | `style_repetition` |
| `LongSentence` | `style_long_sentence` |
| `Cliche` | `style_cliche` |
| `Filler` | `style_filler` |
| `Wordy` | `style_wordy` |
| `Colloquial` | `style_colloquial` |

The wasm shape mirrors the grammar diagnostic object:
`{ kind, severity, confidence, start, end, message, expected, corrections }`.
`severity` is always `"suggestion"`. The style kind prefix keeps the two
streams distinguishable even when a UI merges them.

## 5. Rules

### 5.1 Curated phrase and word table (v1)

Two CSVs, both lowercased and matched on word boundaries:

`data/style_phrases.csv`

```csv
phrase,kind,replacement,note
tällä hetkellä,cliche,nyt,usein tarpeeton
viime kädessä,cliche,,tarpeeton korostus
kuin myös,wordy,ja,kahdennus
voidaan todeta,wordy,,tarpeeton passiivi
```

`data/style_words.csv`

```csv
word,kind,replacement,note
oikeastaan,filler,,usein tarpeeton
tavallaan,filler,,usein tarpeeton
haasteellinen,cliche,vaikea,kulunut
```

- `kind` maps to `StyleKind`; an unknown kind is a build error.
- `replacement` is optional. When present, the rule emits one
  `Correction::replacement` over the matched span. When absent, the diagnostic
  has no correction.
- A multi-word phrase matches a run of consecutive word tokens whose
  lowercased text joins with single spaces. The span runs from the first
  token’s `start` to the last token’s `end`.
- A single word matches one token. Do not match inside a compound or a
  longer word: `tavallaan` matches only the standalone token.
- The table is small and curated. Every row needs a fixture case. A row that
  cannot be defended in one sentence stays out.
- Do not copy a style guide’s list. Original curation only; record the origin
  and terms in `data/README.md`, alongside the other curated CSV tables. Do not
  relabel a copied list as original data.

**Why not a suffix rule?** `-minen`, `-uus` and `-sti` are productive and
usually correct. A suffix heuristic would flag legitimate text. A phrase table
is auditable and removable.

### 5.2 Repetition (v1)

Flag the same content lemma when it repeats close enough to read as an echo.

- Work sentence by sentence, using `sentence::spans` and the syntax lemmas.
- Consider content readings only: noun, adjective, verb, adverb. Skip
  pronouns, numerals, conjunctions, auxiliaries and the light verbs `olla`,
  `tulla`, `voida`, `tehdä` when they are purely grammatical.
- Skip words shorter than four characters and proper names.
- Window: the second occurrence is flagged when at most **10 word tokens**
  separate the two occurrences.
- Emit at most one finding per repeated lemma per sentence; the span is the
  second occurrence.
- Confidence `Medium`; message `style_repetition`; `expected` is the lemma.
- No correction: a synonym is not safe without context.

Examples:

```text
Tämä on hyvä asia. Asia on hyvä.        # two sentences: no finding
Tässä lauseessa asia toistuu, ja asia jää mieleen.   # finding on the second `asia`
```

The first example is deliberately not a finding: cross-sentence repetition is
normal in cohesive prose and would make the rule noisy. A later phase may add
a document-level density metric if the corpus shows it is useful.

### 5.3 Long sentence (v1)

Flag a sentence over **35 word tokens**.

- Skip sentences with no finite predicate: headings, captions and list
  fragments are not prose sentences.
- Skip a sentence that already has a word-level style finding; one finding per
  sentence is enough.
- Span: the sentence span. Confidence `Medium`; message
  `style_long_sentence`; no correction.
- This is a readability nudge, not a claim that the sentence is wrong. Finnish
  formal prose legitimately contains long sentences, so the threshold is high
  and the label is neutral.

### 5.4 Later rules (not v1)

Only add these after the v1 corpus shows the layer is quiet enough:

- repeated sentence openings (`Ja …`, `Myös …`) across adjacent sentences;
- intensifier density (`erittäin`, `hyvin`, `todella`, `äärimmäisen`) when a
  sentence contains two or more;
- nominal-style patterns (`suorittaa` + noun, `tehdä` + noun) from a curated
  pattern table;
- document-level repetition density (same lemma across paragraphs).

Do not add “passive voice”, “word order” or “sentence variety” as style rules.
Finnish passive is normal, word order is information structure, and sentence
variety is not mechanically decidable here.

## 6. Data and build integration

Both tables are build-time inputs, not runtime files:

- `build.rs` reads `data/style_phrases.csv` and `data/style_words.csv` and
  emits `maps/style_phrases.bin.gz` and `maps/style_words.bin.gz`.
- The writer lives in `build/csv_builders/` (new `emit_style_table`, or an
  extension of `string_map.rs`). The blob is a sorted front-coded
  `phrase -> kind + replacement` record.
- `src/style_check/lexicon.rs` embeds the blobs with `include_bytes!`, using
  the shared readers in `src/map_reader.rs`.
- `cargo:rerun-if-changed` covers both CSVs, as with `verb_rection.csv`.
- The tables are tiny (target: 20–40 phrases, 20–40 words), so a linear scan
  after decompression is fine. Do not build a runtime index until the table
  grows past a few hundred rows.
- Add both files to `data/README.md` with their columns and origin.

If the blob pipeline turns out to be more machinery than the table warrants,
the fallback is `include_str!` plus a one-time parse into a `OnceLock<Vec<_>>`.
Measure before choosing; the default is the blob, because every other curated
table in the crate already uses it.

## 7. UI and wasm

### 7.1 Wasm

Add one stateless export:

```rust
#[wasm_bindgen]
pub fn check_style(text: &str) -> js_sys::Object;
```

The object shape is:

```text
{
  diagnostics: [{ kind, severity, confidence, start, end,
                  message, expected, corrections }]
}
```

`start`/`end` are UTF-16 offsets, matching `check_syntax` and the overlay
protocol. If the text is large enough that copying it twice matters, add a
state-aware `check_style_with_state(idx, text)` later; do not add it before a
measurement.

### 7.2 Browser extension

- New context-menu item **Tarkista tyyli**, separate from **Tarkista
  kielioppi**.
- New content-to-background message `torille:check-style-pending-input` and a
  worker message `torille:check-style`.
- The content script masks ignored ranges before the call and drops any
  diagnostic whose span intersects an ignored range, exactly as the spell
  overlay does. Code blocks, URLs and BBCode never produce style findings.
- The panel title is `Tyyli`; the labels are the neutral Finnish strings from
  §3. Corrections, when present, use the existing reverse-order edit replay.
- After the explicit check the same spans are painted blue solid, separate
  from the spelling red and the grammar amber/orange. The paint is a snapshot:
  it clears on the next edit and is never part of the keystroke path.

### 7.3 Leptos demo

- The demo gets a separate `Tyyli` panel next to `Kielioppi`.
- It reuses the diagnostic list classes and the suggestion accent, never the
  red error styling.
- Opening or checking one panel must not re-run the other checker; each owns
  its own call.
- The panel shows a short empty state: `Ei tyylihuomautuksia.`

## 8. Evaluation

Two new fixtures, mirroring the syntax layer:

`tests/fixtures/style_diagnostics.json`

- exact output per case: kind, span, message, corrections;
- cases for each table row, each repetition shape and each length boundary
  (34/35/36 words);
- a case proving a phrase inside a longer word does not match;
- a case proving a sentence with a word-level finding gets no long-sentence
  finding.

`tests/fixtures/style_eval.json`

- `clean`: neutral Finnish prose, at least 100 sentences, no table hits and no
  repetition within the window;
- `cases`: per-kind rows with expected counts;
- metrics: per-kind precision/recall and a clean-corpus budget.

Baseline policy:

- zero `Error` and zero `Warning` findings, always;
- zero **High-confidence table hits** on clean rows;
- at most **1 metric finding per 20 clean sentences** (long sentence or
  repetition, combined) before the rule ships;
- a new rule that exceeds the budget is not merged, even if its case recall is
  good.

The budget is a guard against a style checker that is technically correct but
unusable. It is deliberately stricter than a typical linter.

## 9. Phases

Each phase is a separate commit. A phase that only adds structure must keep the
grammar fixture and evaluation unchanged.

### Phase 0 — decision and baseline

- [x] Record the grammar/syntax separation in this plan.
- [x] Confirm `cargo test --release -p torille --lib`, `--tests`, clippy and
      the wasm32 checks are green.
- [x] Do not add a style rule yet.

### Phase 1 — module and diagnostic skeleton

- [x] Add `src/style_check/{mod,diagnostic}.rs`.
- [x] Add `StyleKind`, `StyleDiagnostic`, `StyleAnalysis`, `check`.
- [x] `check` returns an empty analysis; no rule yet.
- [x] Reuse `Severity`, `DiagnosticConfidence` and `Correction` from
      `syntax_check::diagnostic`; do not duplicate them.
- [x] Add a test that `check` never emits `Error`/`Warning`.
- [x] Keep the public grammar API unchanged.

Phase 1 landed with `src/style_check/{mod,diagnostic}.rs` and
`pub mod style_check` in `lib.rs`. `check()` tokenises and returns an empty
`StyleAnalysis`; `StyleDiagnostic::new` pins `Severity::Suggestion`.

Acceptance: empty output on every existing syntax fixture; no new wasm export
yet.

### Phase 2 — curated table pipeline

- [x] Add `data/style_phrases.csv` and `data/style_words.csv`.
- [x] Emit the two blobs from `build.rs`; add the readers in
      `style_check/lexicon.rs`.
- [x] Implement the phrase/word matcher with exact spans and optional
      corrections.
- [x] Add `tests/fixtures/style_diagnostics.json` and `style_check.rs`.
- [x] Every table row has a case; unknown kinds fail the build.

Phase 2 landed with eight phrases and six words. The build writer normalises
the key, validates the kind against `cliche`/`filler`/`wordy`/`colloquial`,
rejects duplicate keys and rows longer than four words, and stores
`kind + replacement` as the blob values. The matcher tries the longest phrase
first, stops at sentence boundaries, and resumes after a hit so a phrase is
reported once.

Acceptance: the exact-output fixture is stable; no overlap duplicates.

### Phase 3 — repetition

- [x] Add `style_check/repetition.rs`.
- [x] Sentence-scoped lemma repetition with the 10-token window.
- [x] Skip function words, light verbs, short words and proper names.
- [x] Add positive and negative cases, including the cross-sentence negative.
- [x] One finding per lemma per sentence.

Phase 3 landed with a sentence-local scan over the syntax lemmas. It skips
function words, light verbs (`olla`, `tulla`, `voida`, `tehdä`), words shorter
than four characters and non-initial proper names; a sentence-initial capital
is not a name signal, and `Asia` is both a name and the common noun. Phrase
table spans are excluded, and the fixture covers the near repeat, the
one-per-sentence rule, the cross-sentence negative and the light-verb
negative.

Acceptance: clean corpus stays within the metric budget.

### Phase 4 — long sentence

- [x] Add `style_check/length.rs`.
- [x] 35-word threshold, finite-predicate guard, one-per-sentence rule.
- [x] Add 34/35/36-word boundary cases.
- [x] Suppress when the sentence already has a word-level finding.

Phase 4 landed with `LONG_SENTENCE_WORDS = 35` (over 35 is reported), a
finite-predicate check over the sentence's readings, and sentence-span
suppression when any phrase or repetition finding sits inside it. The fixture
pins the 36-word case and the inclusive 35-word negative.

Acceptance: clean corpus budget unchanged; no finding on headings or list
fragments.

### Phase 5 — evaluation corpus

- [x] Add `tests/fixtures/style_eval.json` and `tests/style_eval.rs`.
- [x] Record per-kind precision/recall and the clean budget.
- [x] Tune the repetition window and length threshold only from measured
      corpus results; do not tune to a single example.
- [x] Add the baseline table to this plan and, after implementation, to
      `docs/style_checker.md`.

Phase 5 baseline (2026-11-13):

```text
clean rows 40: 0 table hits, 0 metric findings, budget 2
cases 6: every emitted kind precision 1.00, recall 1.00
```

The thresholds did not need tuning: the 10-token repetition window and the
35-word sentence threshold already keep the clean corpus quiet. The case set
covers all six emitted kinds.

Acceptance: zero `Error`/`Warning`, zero table hits on clean rows, metric
budget met.

### Phase 6 — wasm and UI

- [x] Add `check_style` to `torille-wasm`.
- [x] Add the `torille:check-style` worker path and the
      `torille:check-style-pending-input` content path.
- [x] Add **Tarkista tyyli** to both extension context menus.
- [x] Add a separate `Tyyli` panel to the Leptos demo.
- [x] Filter style findings whose spans intersect an ignored range, so code
      blocks, URLs and BBCode never produce style advice.
- [x] Run both extensions’ typecheck, build and tests.

Phase 6 landed with a stateless `check_style(text)` wasm export, a separate
`check-style` worker message and `Tarkista tyyli` menu item, a `Tyyli` panel in
both browsers, and a separate Leptos panel. The wasm worker converts byte
offsets to UTF-16 with the same helper as the grammar path. Both browsers pass
48 tests, including a new `check-style` integration case.

Acceptance: style is invisible until requested; grammar output is unchanged;
the worker protocol has one new message type and no changed old one.

### Phase 7 — docs, version and gates

- [x] Add `docs/style_checker.md`: labels, thresholds, table columns, wasm
      shape, UI contract, limitations and the measured baseline.
- [x] Link it from `README.md` and `AGENTS.md` only after the document exists.
- [x] Bump the shipping crates to `0.7.0` and both extension packages to
      `1.2.0`, in the release commit.
- [x] Run the full gates in §10.

Phase 7 landed with `docs/style_checker.md` linked from both entry points.
`PLAN_SYNTAX_CHECKER.md` §6E now points at the implemented layer as well as
the plan.

## 10. Gates

Every phase runs the existing project gates:

```bash
cargo test --release -p torille --lib
cargo test --release -p torille --tests
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p torille-wasm --target wasm32-unknown-unknown
cargo check -p torille-wasm-leptos --target wasm32-unknown-unknown
python3 tools/build/compare_engine_outputs.py --strict
(cd extensions/firefox && npm run typecheck && npm run build && npm test)
(cd extensions/chromium && npm run typecheck && npm run build && npm test)
```

The style-specific gates are:

```bash
cargo test --release -p torille --test style_check
cargo test --release -p torille --test style_eval
cargo run --release -p torille-tools --bin check_style -- "..."
```

A structure phase must produce no diff in
`tests/fixtures/syntax_diagnostics.json` or `syntax_eval.json`. A rule phase
updates only the style fixtures.

## 11. Risks and mitigations

| risk | mitigation |
|---|---|
| Subjective rules annoy users | Opt-in panel; underlines only after the explicit check and cleared on edit; neutral labels |
| A table row is wrong or dated | One row per case; one-line removal; no suffix heuristics |
| Clean prose becomes noisy | Metric budget; zero table hits on clean rows; a failing rule is not merged |
| Style duplicates grammar | Ownership table in §1; `style_check` never calls the grammar rules; grammar kinds unchanged |
| Overlapping findings spam | Word-level findings suppress long-sentence; phrase spans suppress repetition |
| Blob/build drift | Build emits from CSV; `compare_engine_outputs.py` covers the existing blobs; add a style blob check |
| Runtime cost on large text | Linear pass, on demand; reuse sentence spans and the syntax cache if needed |
| UI conflates style with errors | Separate `Tyyli` title and colour; `severity` always `suggestion` |
| Register policing | No dialect/register verdicts; `arkikielinen` only for clearly informal, curated words |
| Data licensing | Original curation; document origin and licence in `data/README.md` |

## 12. Open questions

1. **Shared primitives.** Should `Severity`/`Correction` move to
   `src/diagnostic.rs` now? Recommendation: not in v1. Share them from
   `syntax_check::diagnostic`; move only when a third consumer appears.
2. **Panel or inline.** Should style findings ever be underlined? The shipped
   layer paints blue underlines only after the explicit `Tarkista tyyli`
   command and clears them on the next edit. A persistent per-site setting is
   still open.
3. **Table size.** How large may the curated tables grow? Recommendation: start
   with 20–40 rows each; a linear scan is fine until a few hundred.
4. **Repetition scope.** Should cross-sentence repetition count? Recommendation:
   no in v1. Cohesive prose repeats deliberately; a document-level density
   metric is a later rule.
5. **Thresholds.** Are 10 tokens and 35 words right for Finnish? Recommendation:
   start there and tune only from the evaluation corpus, never from one
   sentence.
6. **Corrections.** Should the table offer replacements at all? Recommendation:
   yes, but only where the replacement is safe without context. Otherwise the
   finding is label-only.
7. **State-aware wasm.** Does `check_style(text)` need a per-textarea state
   slot? Recommendation: no until a measurement shows the text copy matters.

## 13. Definition of done

For the first shipped style layer:

- `style_check` is a separate module with its own kinds and a Suggestion-only
  contract;
- the phrase/word tables are build-time blobs with one fixture case per row;
- repetition and long-sentence rules meet the clean-corpus budget;
- `docs/style_checker.md` exists and is linked from README/AGENTS;
- the browser extensions expose **Tarkista tyyli** as a separate opt-in
  command, and the Leptos panel has a `Kielioppi` / `Tyyli` switch;
- no grammar diagnostic, span, correction or fixture changed;
- the full gate suite in §10 is green.

If the first corpus run cannot meet the budget, ship only the curated table
and drop the metrics. A small, quiet style layer is better than a noisy one.
