# PLAN_SYNTAX_REFACTOR — modularizing the syntax layer and checker

Status: **Phase 0–2 landed**. The shared-helper extraction is in
`src/syntax_check/analysis.rs`; punctuation is now a module tree under
`src/syntax_check/punctuation/`. Phases 3–6 remain planned, not implemented.

The checker is feature-complete after `PLAN_SYNTAX_CHECKER.md`; this plan asks
what should be restructured before the next feature lands, and what should be
left alone.

## 1. Why a refactor is worth considering

The syntax work grew quickly and the shape now shows it:

| file | lines | what it mixes |
|---|---:|---|
| `src/syntax.rs` | 2256 | cache, morphology resolution, sentence grouping, clause segmentation, role resolution, predicates, tests |
| `src/syntax_check/punctuation.rs` | 868 | list diagnostics, comma rules, clause kinds, questions, quotes, parentheticals, tests |
| `src/syntax_list.rs` | 588 | list arena, nested parsing, classification, tests |
| `extensions/firefox/src/content/content.ts` | 2401 | overlay, contenteditable, popup, worker client, context menu, grammar panel |
| `extensions/firefox/src/spell-worker/spell-core.ts` | 455 | wasm lifecycle, caches, all worker handlers |

Concrete duplication found during the checker work:

- `nominal_pos`, `nominal_case` and `is_marker` exist in `rection.rs`,
  `object_case.rs` and `arguments.rs` with near-identical bodies.
- `inside_brackets` exists in `object_case.rs` and `punctuation.rs`.
- `comma_between`, `first_word`, `last_word` and clause-word collection live in
  `punctuation.rs`, while sibling rules re-implement small pieces.
- `clause_kind` is derived inside `punctuation.rs`, although it is a general
  clause property.

Two workarounds point at a model limit rather than a missing rule:

- `TokenSyntax` stores **one** chosen `lemma`. A partitive plural such as
  `kirjoja` belongs to both `kirja` and `kirjo`; the checker has to re-run the
  base search to decide whether a plural completive suggestion is safe.
- `Clause` is a linear range. A relative clause inside its host
  (`Mies, joka tuli, söi`) splits the host into contiguous segments. The
  checker works around this in comma rules but a UI cannot ask for the host
  clause as one unit.

That is enough evidence for a **modularisation** refactor. It is not enough
evidence for a parser rewrite: the rules are conservative, the tests are
data-driven, and the engine is intentionally not statistical.

## 2. Non-goals

Do not change any of these in this plan:

- the morphology engine, lexicon, catalogue or `Confidence` model;
- `src/sentence.rs` or the keystroke path;
- the diagnostic JSON shape (`kind`, `severity`, `confidence`, `span`,
  `message`, `expected`, `corrections`);
- the list arena semantics: post-order child lists, `List.parent`, stable
  indices;
- rule behaviour. Every refactor phase must keep the fixture and evaluation
  baselines byte-identical unless the phase explicitly says it is a feature;
- word order or the style layer. They remain deliberate boundaries.

## 3. Decision summary

A full rewrite is not justified. The recommended order is:

| phase | recommendation | why |
|---|---|---|
| 1. Shared token/clause helpers | **do first** | Three copies already, low risk |
| 2. Split punctuation | **do before the next punctuation rule** | 868 lines, six rule families |
| 3. Split `syntax.rs` | **do when the next analysis change lands** | 2256 lines, clear seams |
| 4. Per-reading lemmas | **only when a second homonym-sensitive rule needs it** | Additive model change |
| 5. Clause tree for relatives | **only when a consumer needs host nesting** | Feature, not cleanup |
| 6. Split extension content | **when the next extension UI change lands** | Bundled single file, clear seams |

Each phase is a separate commit. A phase that only moves code must not change a
single diagnostic, span, correction or test expectation. If a move forces a
behaviour change, stop and split the commit.

## 4. Target layout

### 4.1 `src/syntax/` (from `src/syntax.rs`)

Keep the public module path `crate::syntax::*` by making `syntax/mod.rs`
re-export the public items.

```text
src/syntax/
  mod.rs        public API, analyze/analyze_with orchestration, SyntaxAnalysis
  cache.rs      AnalysisCache, resolve_word, clitic handling
  readings.rs   WordAnalysis, form-to-reading conversion, proper-name fallback
  segments.rs   clause_groups, bracket_only_boundary, segment_clause_words,
                top_level_semicolon
  roles.rs      resolve_clause, choose_predicate, role marking helpers
  predicates.rs is_coordinating, is_subordinator, is_relative_word,
                is_finite_predicate, is_adverbial_case, local cases
  tests.rs      the existing module tests, split by concern if useful
```

No function changes in the move phase. `syntax/mod.rs` should contain only the
public types, `analyze`, `analyze_with_cache`, and the orchestration in
`analyze_with`.

`src/syntax_list.rs` stays a sibling. It is cohesive and does not need to move
into a directory yet.

### 4.2 `src/syntax_check/` (from the flat rule files)

```text
src/syntax_check/
  mod.rs          orchestration and public re-exports
  diagnostic.rs   unchanged
  analysis.rs     NEW: shared token/clause helpers
  clause.rs       NEW: ClauseKind and clause-boundary predicates
  agreement.rs    unchanged
  rection.rs      uses analysis.rs
  object_case.rs  uses analysis.rs
  arguments.rs    uses analysis.rs
  correction.rs   unchanged
  rank.rs         unchanged
  punctuation/
    mod.rs        check(): list + comma + question + quote + parenthetical
    lists.rs      list diagnostics and missing/duplicate separators
    comma.rs      subordinate/relative comma rules and pair balancing
    question.rs   question-mark rule and terminal punctuation
    quote.rs      unbalanced-quote warning
    parenthetical.rs  esimerkiksi/kuten pairs
```

The public API stays `syntax_check::{check, check_with_cache, check_analysis,
...}`. Only the internal module tree changes.

### 4.3 Extension content (optional, later)

Keep `content.ts` as the esbuild entry point, but move the bodies behind it:

```text
extensions/firefox/src/content/
  content.ts          entry: imports and boot only
  worker-client.ts    createWorkerClient and the message wrappers
  overlay/
    textarea.ts       textarea/input overlay
    contenteditable.ts
    popup.ts
  grammar-panel.ts    showGrammarPanel, apply helpers, Finnish labels
  ignore-ranges.ts    already separate
  context-menu.ts     pending-input tracking and menu messages
```

Chromium keeps the same layout and the source-sync test should grow to cover
every shared module that is meant to stay byte-identical.

## 5. Phases

### Phase 0 — freeze the behaviour baseline

- [x] Record the current gates in this plan: 968 lib tests, 43 syntax fixture
      tests, 164 evaluation sentences (112 clean + 52 cases), 37 extension
      tests per browser.
- [x] Record the current `syntax_eval` per-kind table in the Phase 1 commit
      message.
- [x] Confirm `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Confirm Firefox and Chromium `npm run typecheck`, `npm run build` and
      `npm test` (unchanged from the previous commit).

Acceptance: every gate is green before the first move.

### Phase 1 — shared analysis helpers (behaviour-preserving)

Add `syntax_check/analysis.rs` with:

```rust
pub(super) fn nominal_pos(token: &TokenSyntax) -> Option<PartOfSpeech>;
pub(super) fn nominal_case(token: &TokenSyntax) -> Option<Case>;
pub(super) fn is_marker(word: &str) -> bool;
pub(super) fn is_finite(token: &TokenSyntax) -> bool;
pub(super) fn clause_words(clause: &Clause, tokens: &[Token]) -> Vec<usize>;
pub(super) fn first_word(clause: &Clause, tokens: &[Token]) -> Option<usize>;
pub(super) fn last_word(clause: &Clause, tokens: &[Token]) -> Option<usize>;
pub(super) fn comma_between(tokens: &[Token], left: usize, right: usize) -> bool;
pub(super) fn inside_brackets(tokens: &[Token], start: usize, index: usize) -> bool;
```

Before merging a copy, diff the three implementations. `nominal_case` in
`object_case.rs` and `rection.rs` is the same; `arguments.rs` uses the same
shape. `is_finite` exists in `punctuation.rs` and `agreement.rs` with different
visibility contracts; keep the more general one and document the difference if
it is real.

- [x] Add `analysis.rs` and move the helpers.
- [x] Replace the copies in `rection.rs`, `object_case.rs`, `arguments.rs` and
      `punctuation.rs`.
- [x] Keep the helper tests data-driven through the existing fixture; there
      were no direct unit tests to move.
- [x] Do not touch rule bodies beyond imports and call sites.

Landed in `src/syntax_check/analysis.rs`. Two deliberate variants are kept:

- `nominal_pos` excludes numerals; `nominal_or_numeral_pos` keeps them for
  rection.
- `analysis::is_finite` is the punctuation sense (imperatives count).
  `agreement::finite_reading` remains separate because an imperative is never
  the agreement bearer.

Acceptance: the exact-output fixture and evaluation table are unchanged; no
public API changes.

### Phase 2 — split punctuation by rule family

- [x] Move `ClauseKind` and `clause_kind` into `clause.rs`.
- [x] Split `punctuation.rs` into `punctuation/{lists,comma,question,quote,
      parenthetical}.rs`.
- [x] Keep `punctuation/mod.rs::check` as the same ordered pipeline.
- [x] Keep the tests in `punctuation/mod.rs`; they exercise `check`, not a
      rule's private helpers.
- [x] Keep `DiagnosticKind`, spans, severities and correction order identical.

Acceptance: fixture and evaluation unchanged. The only diff is file layout and
imports.

### Phase 3 — split `syntax.rs`

- [ ] Create `src/syntax/` and move the sections listed in §4.1.
- [ ] Keep `crate::syntax` re-exports stable; no caller changes.
- [ ] Keep `Clause`, `TokenSyntax`, `SyntaxAnalysis`, `RoleConfidence`,
      `SyntacticRole` and `AnalysisCache` public paths identical.
- [ ] Move the `#[cfg(test)] mod tests` with the roles that own them.
- [ ] Verify `cargo doc` still links the public items.

Acceptance: fixture, evaluation, `tests/catalogue.rs`, and the wasm32 check are
unchanged. No public path breaks.

### Phase 4 — per-reading lemma candidates (optional feature)

This is the clean fix for the homonym workaround.

- [ ] Add a reading-level lemma representation, either:
      `TokenSyntax.readings: Vec<Reading>` with `Reading { tags: Tags, lemma:
      Option<String> }`, or an additive `TokenSyntax.lemmas: Vec<String>`.
- [ ] Populate it in `resolve_word` while `add_forms_from_lemma` already knows
      the base.
- [ ] Use it in `object_case::plural_total_correction` and `rection::argument`
      to avoid re-running the base search for every candidate.
- [ ] Keep the wasm shape additive (a `lemma` per reading or a `lemmas` array).
- [ ] Add tests for `kirjoja` (two lemmas), `autoja` (one), and a rection
      homonym.

Acceptance: `Luin autoja loppuun.` still suggests `autot`, `Luin kirjoja
loppuun.` stays quiet, and the evaluation table is unchanged.

Do this only when a second rule needs the per-reading lemma. If the plural
completive helper remains the only consumer, leave it as a documented
workaround.

### Phase 5 — clause tree for relatives (optional feature)

This is a model extension, not cleanup. Do it only when a UI or rule needs the
host clause as one unit.

- [ ] Add additive fields to `Clause`:
      `parent: Option<usize>`, `children: Vec<usize>`,
      `antecedent: Option<usize>`.
- [ ] Keep the linear `start`, `end`, `tokens` and roles unchanged.
- [ ] Build the tree in the segmentation pass: a relative clause is a child of
      the host segment; its antecedent link points at the modified nominal
      when it is unambiguous.
- [ ] Keep comma diagnostics on the linear ranges until the tree is proven.
- [ ] Expose the tree in the wasm shape only after the checker uses it.

Acceptance: the existing linear clause array is unchanged; the tree is
additive; the comma fixture is unchanged.

### Phase 6 — split the extension content script (optional)

Do this when the next extension UI change lands, not before.

- [ ] Move the worker client, overlays, popup, grammar panel, context-menu
      tracking and boot code into the layout in §4.3.
- [ ] Keep `content.ts` as the esbuild entry point.
- [ ] Keep the message protocol and DOM behaviour identical.
- [ ] Extend `tests/source-sync.test.mjs` to every shared content module.
- [ ] Run both extensions' typecheck, build and tests.

Acceptance: 37 tests per browser still pass; no change to the worker protocol.

## 6. API and compatibility notes

- Rust module paths are preserved by re-exports in `syntax/mod.rs` and
  `syntax_check/mod.rs`.
- `SyntaxAnalysis`, `Clause` and `TokenSyntax` are public structs. Adding
  fields is an additive change in 0.x, but document it in the plan/commit and
  update the wasm shape comment.
- `ListItemKind::List(Vec<usize>)` is already the multi-child shape; do not
  change it again in this refactor.
- Diagnostic kinds, severity, span and correction ordering are a public
  contract. A move must not reorder them.
- The extension worker protocol (`torille:check-syntax`,
  `torille:check-grammar-pending-input`, `check_syntax_with_state`) is stable.

## 7. Test and gate plan

Every phase runs:

```bash
cargo test --release -p torille --lib
cargo test --release -p torille --tests
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p torille-wasm --target wasm32-unknown-unknown
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/test_binding_agrees_with_binary.py
(cd extensions/firefox && npm run typecheck && npm run build && npm test)
(cd extensions/chromium && npm run typecheck && npm run build && npm test)
```

The syntax-specific gates are:

```bash
cargo test --release -p torille --test syntax_check
cargo test --release -p torille --test syntax_eval
cargo run --release -p torille-tools --bin check_syntax -- "..."
```

A pure move phase must produce zero diff in the fixture and evaluation output.
A feature phase must update the fixture and record the new baseline in
`PLAN_SYNTAX_CHECKER.md` and `docs/syntax_checker.md`.

## 8. Risks and mitigations

| risk | mitigation |
|---|---|
| A helper move silently changes a guard | Diff the copies first; move only after the three bodies are proven identical |
| Punctuation split changes rule order | Keep `punctuation/mod.rs::check` as the single ordered pipeline; test after each move |
| `syntax/` split breaks a public path | Re-export every public item from `syntax/mod.rs`; compile the wasm and Leptos crates |
| Per-reading lemmas change role resolution | Additive field only; keep the chosen `lemma` for existing callers |
| Clause tree changes comma diagnostics | Build the tree alongside the linear ranges; do not switch rules in the same phase |
| Extension split breaks the bundle | One esbuild entry; run both builds and the 37-test suites |
| Refactor hides a real bug | Every phase is a separate commit with the exact fixture as the gate |

## 9. Open questions

1. **Reading representation.** Is `TokenSyntax.lemmas: Vec<String>` enough, or
   does a rule need the lemma tied to a specific `Tags` reading? Recommendation:
   start with a parallel `Vec<String>` only if the order matches `readings`;
   upgrade to `Reading { tags, lemma }` if the mapping becomes ambiguous.
2. **Clause tree ownership.** Should the tree live in `syntax::Clause` or in a
   separate `ClauseTree` returned by analysis? Recommendation: additive fields
   on `Clause`; the linear array stays the source of truth for existing rules.
3. **Punctuation module name.** `punctuation/lists.rs` vs keeping list
   diagnostics in `syntax_list.rs`. Recommendation: diagnostics belong to
   `syntax_check`, list structure stays in `syntax_list`.
4. **Extension module sync.** Should the source-sync test cover background and
   worker too? Recommendation: only modules that must stay byte-identical;
   background already differs by host on purpose.

## 10. Definition of done

For the modularisation part:

- `syntax.rs` is a directory with no behaviour change.
- `punctuation.rs` is a directory with no behaviour change.
- `analysis.rs` owns the shared token/clause helpers; no rule module has a
  private copy.
- All public paths, diagnostics and wasm fields are unchanged.
- The fixture, evaluation, lib tests, extension tests, clippy and wasm32 check
  are green.

For the optional feature part (Phase 4/5/6), the gate is a new test that fails
on the old model and a documented additive shape. If no consumer needs it,
leave it out: a smaller model with a documented workaround is better than a
speculative one.
