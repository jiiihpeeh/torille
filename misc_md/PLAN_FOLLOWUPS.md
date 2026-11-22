# PLAN_FOLLOWUPS — the next four slices

Status: **proposed.** Baseline `ccad6f4` (2026-11-21). The semicolon plan
(0–5) and the probe-performance plan (0–6) are both closed, so these are the
first items from the standing trackers rather than an open phase. Each slice
is independent, lands as its own commit, and carries the standard gate list in
§6.

Order of work:

1. **Sentence-cache parity** — a shipping path returns diagnostics the other
   does not. Correctness first.
2. **Possessive coverage** — the largest user-visible morphology gap (A gates
   B; see §3).
3. **Typography surfaces** — the pass exists but no UI can reach it.
4. **Closed-spelling audit and residue** — small hygiene, independent of 1–3.

## 1. Why these four

- **`TODO_GRAMMAR_STYLE.md` P2** — the sentence-cache path and the
  whole-document pass disagree on `page-2000` by 3 diagnostics.
- **`TODO.md` high priority** — oblique possessives (A), then one spelling per
  slot (B); the recorded gap is 14,026 / 4,359 missing forms.
- **`PLAN_SEMICOLON.md` Phase 4 note** — typography is implemented but not
  surfaced in the wasm, the demo or the two extensions.
- **`TODO_CLOSED_SPELLINGS.md` Phase 0/5** — the audit harness and the
  `kuulten` residue; 32 historical rows, 6 live.

Nothing here changes the suggestion ranking, a published reading or the
diagnostic contract beyond fixing the parity bug itself.

## 2. Slice 1 — sentence-cache parity

### 2.1 Problem

`check_grammar_style_with_state` (the shipping wasm/extension path) checks one
sentence at a time through `src/syntax_check/sentence_cache.rs`.
`check_syntax` / `syntax_check::check` check the whole document. On
`page-2000` they disagree:

```text
cached 6 diagnostics, whole 3
  existential_subject_case   byte 236   "Siellä oli laulujen\n"
  comma_missing              byte 711   "kun ajattelin niitä arvoja, …\n"
  question_mark_missing      byte 9357  "miten kauheasti puut heiluvat …\n"
```

The three shared ones are the two `on` agreement rows and the document-end
quote. The three extras are sentence-path artefacts, not new findings.

### 2.2 Root cause (verified)

`src/sentence.rs::separator_ends_sentence` treats every newline as a boundary
by design: `spans` is the line/paragraph layer the case and spelling paths
want. `SentenceCache::check` slices at `morphology.sentences`, and the spans
partition the text with the **leading separators belonging to the previous
span**. Therefore a cache unit can

- start at the first word of a line, losing the comma, dash or newline the rule
  reads (`preceded_by_comma` at 9357/711; `boundary_between` at 711), and
- end at a newline in the middle of a clause, truncating it before the subject
  or predicate (`Siellä oli laulujen` at 236).

The whole-document pass builds clauses over the full token stream, so it sees
the whole clause and the leading separator.

### 2.3 Design

Keep the line-based `sentence::spans` for the case/spelling layers. Add a
grammar unit in `sentence_cache.rs`:

```rust
struct GrammarUnit {
    /// The sentence range the unit's diagnostics must fall in.
    core: Range<usize>,
    /// The text to analyse: the core plus the separators before it.
    slice: Range<usize>,
}
```

- **Merge line fragments.** Consecutive spans are one unit while the text
  before the boundary does not end in a terminal mark (`.`/`!`/`?`, optionally
  followed by a closing quote or a dialogue dash). A newline alone does not end
  a grammar unit.
- **Include the leading separators.** `slice.start` is the previous unit's last
  word end (0 for the first unit), so the comma/dash/newline context is
  visible. Consecutive slices overlap only on separators.
- **Filter after the check.** Keep a diagnostic only when its span lies inside
  `core` (relative to the slice). This prevents the shared separator from
  producing the same finding twice.
- **Cache key unchanged.** The hash already covers the exact slice text,
  length and quote depth; a merged unit is just a different key.
- Document-global rules stay where they are: the unbalanced-quote warning is
  still added once from the full token stream.

Adjudicate the three rows while landing: the grammar reading says all three
are sentence-path false positives (`laulujen ja kanteleen maa` is a legitimate
nominative existential subject; the dash and comma already mark the `kun`
boundary; the `miten` line continues a comma). Pin them as quiet in the unit
tests, not as whole-pass regressions.

### 2.4 Steps

- [ ] Add `grammar_units` with merging and leading-separator slices.
- [ ] Switch `SentenceCache::check_with_decisions` to the units and the
      `core` filter; keep `sentence::spans` untouched.
- [ ] Add `tests/fixtures/perf/line-wrap.txt` with the three contexts (two
      preceding/following sentences each) and compare the cache path with the
      whole pass.
- [ ] Add the three strings to the existing
      `the_sentence_cache_matches_the_whole_text_pass` list.
- [ ] Add an `#[ignore]` full-`page-2000` parity test for the release gate.

### 2.5 Gate

- Cache and whole pass are byte-identical on `line-wrap.txt` and `page-2000`.
- `page-2000` stays at 3 diagnostics, prose and exact fixtures unchanged.
- The warm post-seed grammar pass is not slower than the recorded ~72 ms (the
  cache still hits unchanged units; a moved newline inside a unit is a miss).
- `cargo test --release --workspace`, clippy and the perf harness are green.

### 2.6 Stop condition

If merging changes a rule's output on the clean corpus, the merged unit is
wrong for that rule: fall back to passing the neighbouring sentence as context
without merging, and record the rows that still diverge.

### 2.7 Files

`src/syntax_check/sentence_cache.rs`, `src/sentence.rs` (read-only),
`tests/fixtures/perf/line-wrap.txt` (new), `tests/syntax_prose.rs` or a new
`tests/sentence_cache.rs`, `docs/syntax_checker.md` (known-limitations note).

## 3. Slice 2 — possessive coverage

### 3.1 Problem

Two recorded gaps in `docs/harness.md` → Known defects:

| defect | recorded size | shape |
|---|---:|---|
| A. a non-`Plain` oblique gets no possessive | 14,026 missing / 101 classes | `add_possessives` skips the oblique and its whole possessive grid |
| B. one spelling per slot | 4,359 missing / 92 classes | `GEN` 1,962, `ILL` 2,397, `PAR` 1,117, `NOM` 740 |

A gates B. The obvious B fix (delete `.take(1)` in
`src/expand/inflect.rs`) was measured as a net loss over ten classes:
missing 1,241 → 1,078 but illegal 385 → 596, because the extra spellings are
removed by demotion passes *after* the possessives are built and their
possessives survive as orphans (`valtimoom` is gone, `valtimoommemme` is not).

The recorded A/B counts predate the last possessive round. **Measure first:**
regenerate the 5-lemma class sweep and re-bucket A and B before touching code.

### 3.2 Design

**A — the demotion, not the possessive.** `docs/harness.md` records that the
`lovi` shape (`lovessa` from the weak stem, `lovilta` from the strong) comes
from `demote_contested` in `src/grammar/noun.rs`; the possessive layer then
correctly skips the non-`Plain` oblique. Fix the class/stem reading so the
singular obliques and the plural obliques are each `Plain` on their own stem,
then the possessives follow. Start with `lovi`, `ovi`, `joki`, `noki`,
`oikku`, `allergikko` and the 20-lemma sample from `TODO.md`.

**B — ordering.** Only after A:

1. Collect every `Plain` spelling per slot into a `Vec<String>` instead of
   `.take(1)`.
2. Let the demotion/retention passes for that slot run (the catalogue
   retention at `inflect.rs` and the heuristic's `demote_contested`).
3. Build the possessives from the surviving spellings, as `add_possessives`
   already does for the oblique grid (`inflect.rs::add_possessives_for`).
   Attributes follow the surviving base spelling, not the slot.
4. Check the authoritative per-class engines: for an authoritative class the
   catalogue output is cleared and the generated engine owns the grid, so the
   same ordering rule has to hold there (or the engine keeps its own
   possessive build).

Do not add a possessive for a spelling that was demoted to `Guessed`/`Repaired`
after the build; that is the orphan mechanism, not a coverage goal.

### 3.3 Steps

- [ ] Re-run `python3 tools/report_by_class.py -n 5 -j 8 --split reports/classes`
      and re-bucket A/B; record the new denominators in the plan.
- [ ] Pin the failing families as tests (`lovi` stems, `tarjoilu` genitive
      plurals, `valtimo` orphans).
- [ ] Land A in `src/grammar/noun.rs`; no possessive change in the same commit.
- [ ] Land B as the ordering change; keep the per-class table in `docs/harness.md`
      with before/after values.
- [ ] Re-run the 5-lemma sweep, `--verify-illegal`, and the 20-lemma sample.

### 3.4 Gate

- A: the `lovi`/`ovi`/`joki` families and the 20-lemma sample have no new
  `not-plain` or `illegal`; the missing count falls.
- B: at least half of the recorded B rows are recovered and `illegal` does not
  rise above the pre-change baseline on the ten measured classes.
- The full 5-lemma report keeps 0 illegal under the curated pass and does not
  regress `missing`; `not-plain` stays 0.
- `tests/catalogue.rs` and the `GAPS` staleness rules are untouched.

### 3.5 Stop condition

If B cannot be ordered without hand-editing a generated nominal engine, land A
alone, record the orphan list and leave B open. Do not hand-edit
`src/expand/nominals/`.

### 3.6 Files

`src/grammar/noun.rs`, `src/expand/inflect.rs`, `src/expand/mod.rs`,
`src/expand/nominals/` (regenerate only), `tests/catalogue.rs`,
`docs/harness.md`, `misc_md/TODO.md`.

## 4. Slice 3 — typography surfaces

### 4.1 Problem

`src/typography.rs` is landed and documented, but nothing calls it: no wasm
export, no demo toggle, no extension command. The semicolon plan left the
surface optional.

### 4.2 Design

Keep typography out of `check_syntax` and `check_grammar_style`; it is a
separate, opt-in pass.

- **wasm**: `check_typography(text) -> js_sys::Object` with the same edit shape
  the panel already applies:
  `{ kind: "typography", rule, severity: "suggestion", confidence: "certain",
  start, end, message, corrections: [{ edits: [{ start, end, replacement }] }] }`.
  No textarea state is needed; add a serializer next to `diagnostics_array`.
- **Demo (Leptos)**: a **Typografia** toggle in `SyntaxPanel` that runs the
  pass and lists/applies its diagnostics with the existing underline style.
- **Extensions**: a separate context-menu command **Tarkista typografia**
  routed through `spell-worker/spell-core.ts`; the panel maps the two rule ids
  (`typography_semicolon_spacing`, `typography_semicolon_duplicate`) to Finnish
  labels. Keep Firefox and Chromium source-identical (the `npm test`
  source-sync check).
- **Options**: one toggle, default off, so the existing commands never change
  behaviour for users who do not opt in.
- Underline: reuse the suggestion style for now; a separate colour is a
  follow-up once the command exists.

### 4.3 Steps

- [ ] Add the wasm export + serializer and a native serializer unit test.
- [ ] Add the Leptos toggle.
- [ ] Add the extension command, panel labels and options toggle; run both
      typechecks and `npm test` in each extension directory.
- [ ] Document the command in `extensions/README.md` and the typography
      section of `docs/syntax_checker.md`.

### 4.4 Gate

- The default grammar/style commands return exactly the same diagnostics as
  before (no typography rows).
- The typography command returns only `typography_*` findings, applies edits
  without shifting, and leaves URLs/code/quotations untouched.
- Both extensions pass typecheck, unit tests and the source-sync check; the
  options toggle defaults off.

### 4.5 Files

`torille-wasm/src/lib.rs`, `torille-wasm-leptos/src/components/syntax.rs`,
`extensions/{firefox,chromium}/src/content/*`,
`extensions/{firefox,chromium}/src/background/*`,
`extensions/{firefox,chromium}/src/spell-worker/*`,
`extensions/{firefox,chromium}/src/options/*`, `extensions/README.md`,
`docs/syntax_checker.md`.

## 5. Slice 4 — closed-spelling audit and residue

### 5.1 State

`src/expand/closed_spellings.rs` still has **six** rows, all the `kuulten`
family (`kuulten`, `kuultemme`, `kuulteni`, `kuultenne`, `kuultensa`,
`kuultesi`), documented as the less common postpositional instructive of
`kuulla`. The other 26 rows are gone and the engine resolves them. The
tracker's header and Progress section say "32 → 0", which is stale; the header
is corrected to 32 → 6 and the Progress list is the record of the 26 removals.

### 5.2 Design

The table stays as the genuine lexical residue; the missing piece is the
harness the tracker's Phase 0 describes.

- **Policy parameter.** Thread `Closed::{Enabled, Disabled}` through
  `check_normal` / `is_known_word_with_compound_uncached`; production passes
  `Enabled`, tests and the audit pass `Disabled` to prove the engine derives a
  surface without the table.
- **Audit tool** `tools/src/bin/audit_closed_spellings.rs`: one row per
  surface with `surface`, `base`, `base_in_sanalista`, `engine_emits`,
  `find_bases_base`, `fast_resolves_without_closed`, `proper_name`, `authority`,
  `proposed_action`.
- **Fixture.** Check in the historical 32 rows with their authority/action, so
  a deletion or a re-addition is a reviewed change.
- **Failure rules.** The audit fails when a row marked "engine cannot derive"
  starts being derived (staleness), when a base is absent and undocumented,
  when a "fast-only" row has no probe, or when `CLOSED` gains a row without a
  source comment.
- **Parity.** For every fixture surface, `check` and `check_fast` agree; the
  six live rows resolve to `kuulla`; the deleted split forms offer the
  two-word spelling.
- **Tracker.** Correct the header and Progress to 32 → 6 and record the
  `kuulten` decision (lexical, per the module doc) as closed.

### 5.3 Steps

- [ ] Fix the tracker status and the Progress arithmetic.
- [ ] Add the `Closed` policy parameter and the audit binary.
- [ ] Add the checked-in fixture and the failure rules.
- [ ] Add the full/fast parity test over every fixture surface.
- [ ] Benchmark `check_normal_fast`; the parameter must not move the keystroke
      path.

### 5.4 Gate

- The audit is deterministic, fails when a closed row is re-added or becomes
  derivable, and passes on the six live rows.
- Full and fast paths agree for every fixture surface.
- `check_fast` timing is unchanged within noise.
- No new `CLOSED` row and no new `is_nonword` entry without a source.

### 5.5 Files

`src/expand/closed_spellings.rs`, `src/expand/check_normal.rs`,
`src/suggest/known/` (the fast table lookup), `tools/src/bin/audit_closed_spellings.rs`
(new), `tests/fixtures/closed_spellings.json` (new),
`tests/binding_agrees`-style parity test, `misc_md/TODO_CLOSED_SPELLINGS.md`.

## 6. Standard gates for every slice

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
python3 tools/check_docs.py
python3 tools/test_binding_agrees_with_binary.py
python3 tools/test_reverse_lookup_recall.py
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/test_illegal_evidence.py
```

Plus:

- `page-2000` stays at 3 grammar / 14 style diagnostics.
- `syntax_diagnostics`, `syntax_eval` clean rows and the prose baseline do not
  move.
- No new `GAPS` entry and no `AUTHORITATIVE_CLASSES`/`DISPATCH` change.
- A slice that misses its gate is recorded as a measured negative result and
  is not merged.

## 7. Risks

- **The cache unit changes hit rate.** Merging line fragments makes a unit
  larger, so an edit inside a wrapped sentence re-checks both lines. Measure
  the warm-page pass; if the hit rate drops more than the parity is worth,
  keep the units and accept the miss (they are still one sentence).
- **The possessive ordering is cross-layer.** The catalogue, the heuristic and
  the generated engines each own part of the slot output. Land A first and
  keep B behind its own measurement; do not hand-edit generated files.
- **Typography is a new public surface.** Keep it behind its own toggle and out
  of the default wasm command so the diagnostic contract does not change.
- **The closed-spelling table is deliberately tiny.** The audit is for the
  policy and the residue, not a reason to add rows.

## 8. Out of scope (kept on the trackers)

- The `taloni` `mistagged` decision (3,348 rows): a harness-vs-engine policy
  question in `TODO.md`.
- The first `WordOrder` `PREFER` suggestion: deliberately held until a
  structurally impossible order exists (`TODO_GRAMMAR_STYLE.md`).
- The grouped form-table download budget and the class-level `accepts` test
  (`TODO_GRAMMAR_STYLE.md`, cold-page target).
- Neural reading disambiguation M0 (`docs/neural_disambiguation.md`).
- Any new suffix-stripper entry in `is_known_word`.
