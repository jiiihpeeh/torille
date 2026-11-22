# PLAN_FOLLOWUPS — the next four slices

Status: **slices 1, 3 and 4 landed 2026-11-22; slice 2 measured (A/B already
closed).** Baseline
`ccad6f4` (2026-11-21); the sentence-cache rows were re-verified on
2026-11-22. The semicolon plan (0–5) is closed, and the
probe-performance plan is measured through Phase 6 (5–6 recorded as no
action), so these are the first items from the standing trackers rather than
an open phase. Each slice is independent, lands as its own commit, and carries
the standard gate list in §6.

Order of work:

1. **Sentence-cache parity** — a shipping path returns diagnostics the other
   does not. Correctness first.
2. **Possessive coverage** — the largest user-visible morphology gap (A gates
   B; see §3).
3. **Typography surfaces** — the pass exists but no UI can reach it.
4. **Closed-spelling audit and residue** — small hygiene, independent of 1–3.

## 1. Why these four

- **`TODO_GRAMMAR_STYLE.md` P2** — the sentence-cache path and the
  whole-document pass disagreed on `page-2000` by 3 diagnostics (slice 1,
  fixed).
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
quote. The three extras are sentence-path artefacts, not new findings:

| rule at byte | cache slice | why the whole pass is right |
|---|---|---|
| `existential_subject_case` 236 | `Siellä oli laulujen\n` | the sentence continues `ja kanteleen maa.`; `laulujen` is not sentence-final |
| `comma_missing` 711 | `kun ajattelin niitä arvoja, …\n` | the clause is preceded by `hauskuudekseni--`; the dash is the boundary |
| `question_mark_missing` 9357 | `miten kauheasti … värisevät.\n` | the line continues the comma of `kuule,`; `miten` does not open a question |

Simulating the fix confirms the reading: the 711 and 9357 slices are quiet as
soon as the **previous word** is prepended, and the 236 fragment is quiet as
soon as it is joined with its continuation.

### 2.2 Root cause (verified)

`src/sentence.rs::separator_ends_sentence` treats every newline as a boundary
by design: `spans` is the line/paragraph layer the case and spelling paths
want. `SentenceCache::check` slices at `morphology.sentences`, and the spans
partition the text with the **leading separators belonging to the previous
span**. A cache unit therefore has two defects:

- **It starts at the first word**, so a rule that looks one word back sees
  nothing. `question.rs::preceded_by_comma` (9357) and
  `comma.rs::subordinate_comma`'s `marked` closure, which calls
  `previous_word` and `boundary_between` (711), both need that word.
- **It ends at the newline**, so a rule that asks "is this the end of the
  sentence or the text?" sees the slice end as the text end.
  `existential.rs::sentence_ends` walks the tokens after the last word and
  returns true when it finds none (236).

The whole-document pass sees the previous word and the continuation, so it
stays quiet. Note both passes split the *clause* at the newline; the defect is
the context around the unit, not the clause segmentation itself.

### 2.3 Design

Keep the line-based `sentence::spans` for the case/spelling layers. Add a
grammar unit in `sentence_cache.rs`:

```rust
struct GrammarUnit {
    /// The sentence range(s) whose diagnostics the unit owns.
    core: Range<usize>,
    /// The text to analyse: the previous word, its separators and the core.
    slice: Range<usize>,
}
```

- **Merge line fragments.** Consecutive spans are one unit while the trailing
  separator of the **previous span** carries no terminal mark, colon or closing
  quote/bracket — a pure newline, comma or dialogue dash does not end a unit.
  The boundary text lives at the end of the previous span (`span.end` is the
  next span's start), so the test scans that span's tokens after its last
  word, not the gap. A hard boundary keeps the split. Inside the unit,
  `sentence::spans` still splits at newlines exactly as the whole pass does;
  only the analysis window is larger.
- **Include the previous word and its separators.** `slice.start` is the start
  of the previous unit's **last word** (0 for the first unit), not its end.
  The word is required: `previous_word` finds nothing when the slice starts at
  the separator run, which is why 711 still fires with separators alone.
  Consecutive slices then overlap on one word plus the separators.
- **Seed the quote depth at the slice start, and key on it.** The leading word
  may sit inside a different quote state than the core; pass the depth at
  `slice.start` to `analyze_with_quote_depth` so the analyzer reaches the core
  at the whole-pass depth. The cache key uses the same value.
- **Filter after the check.** Store diagnostics relative to `slice.start` and
  keep the ones whose `span.start` is at or after `core.start`. To avoid a
  double report of a zero-width edit exactly on a boundary: a non-final unit
  keeps `span.start < core.end`, while the final unit keeps an end-of-text
  insertion with `span.end <= core.end`. The current unit owns an insertion at
  its own first word.
- **Shift by `slice.start`**, not by the core start, because the cached
  diagnostics are relative to the slice that was analysed.
- **Keep the counters honest.** `SentenceCacheStats` (and the perf tool's
  labels) currently count sentences; the entries are grammar units now, so the
  doc comments and the report label change with them. The 2,000-entry cap and
  the hit/miss accounting stay.
- Document-global rules stay where they are: the unbalanced-quote warning is
  still added once from the full token stream.

Adjudicate the three rows while landing: the grammar reading says all three
are sentence-path false positives (`laulujen ja kanteleen maa` is a legitimate
nominative existential subject; the dash marks the `kun` boundary; the comma
marks the `miten` continuation). Pin them as quiet in the unit tests, not as
whole-pass regressions.

A scratch simulation of this design on `page-2000` groups 309 spans into 130
units (max 9 fragments) and reproduces the whole-pass diagnostics exactly, so
the parity gate is reachable with this shape rather than with paragraph-sized
units.

### 2.4 Landed

Landed 2026-11-22 as `grammar_units` + `GrammarUnit` in
`src/syntax_check/sentence_cache.rs`. On `page-2000` the 309 spans become
**130 units** (max 9 fragments); the cache returns the same 3 diagnostics as
the whole-document pass, the all-hit pass is **0.80 ms** and a one-sentence
edit **0.93 ms** (`perf_grammar_style --words 0 --repeat 5
--single-thread --json`). The three old findings are still reproducible on
the bare line fragments, which pins the mechanism. Tests:
`tests/sentence_cache.rs` (line-wrap fixture + page parity),
`tests/fixtures/perf/line-wrap.txt`, and the extended unit list in
`the_sentence_cache_matches_the_whole_text_pass`.

### 2.5 Steps

- [x] Add `grammar_units` with merging, the previous-word slice and the
      slice-start quote depth.
- [x] Switch `SentenceCache::check_with_decisions` to the units, the
      `core` filter and the `slice.start` shift; keep `sentence::spans`
      untouched.
- [x] Add `tests/fixtures/perf/line-wrap.txt` with the three contexts (two
      preceding/following sentences each) and compare the cache path with the
      whole pass.
- [x] Pin each row both ways: the bare slice still fires, and the unit with
      the previous word / continuation is quiet.
- [x] Add the three strings to the existing
      `the_sentence_cache_matches_the_whole_text_pass` list.
- [x] Add a normal `page-2000` parity test in `tests/sentence_cache.rs`;
      the suite already runs the page in `disambiguation_noop`, so the cost is
      precedented.

### 2.6 Gate

- Cache and whole pass are byte-identical on `line-wrap.txt` and `page-2000`.
- `page-2000` stays at 3 diagnostics, prose and exact fixtures unchanged.
- A zero-width edit exactly on a unit boundary is reported once, and a
  sentence that starts after a quoted previous word keeps the whole-pass quote
  depth.
- The warm post-seed grammar pass is not slower than the recorded ~72 ms (the
  cache still hits unchanged units; a moved newline inside a unit is a miss).
- `cargo test --release --workspace`, clippy and the perf harness are green.

**Met on 2026-11-22:** `page-2000` cache == whole pass (3 diagnostics), the
line-wrap fixture is quiet, `sentence_cache_warm_ms` 0.81 and the edit pass
1.04 ms (`--words 0 --repeat 5 --single-thread`).

### 2.7 Stop condition

If merging changes a rule's output on the clean corpus, the merged unit is
wrong for that rule: keep the previous-word slice but do not merge, and record
the rows that still diverge (the 236 row is the known one).

### 2.8 Files

`src/syntax_check/sentence_cache.rs`, `src/sentence.rs` (read-only),
`tests/fixtures/perf/line-wrap.txt` (new), `tests/sentence_cache.rs` (new),
`tools/src/bin/perf_grammar_style.rs` (labels/comments) and
`docs/syntax_checker.md`.

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

A is not one mechanism. The recorded split of the 1,659 `(lemma, case)` pairs
with a missing oblique possessive is:

| bucket | pairs |
|---|---:|
| the bare oblique is produced but not `Plain` (the `demote_contested` gate) | 1,092 |
| the bare oblique is `Plain` on the wrong stem (the plural-stem defect) | 495 |
| no bare oblique at all | 72 |

The first bucket is slice A's target; the second is the class/stem defect the
harness records separately; the third is an engine gap with no form to attach
to. Scope A to the first bucket unless the measurement shows the same fix
covers the others.

The recorded A/B counts predate the last possessive round. **Measure first:**
regenerate the 5-lemma class sweep and re-bucket A and B before touching code.

### 3.2 Measured 2026-11-22 — A and B are already closed

No code change was needed. A fresh live sweep (`report_by_class.py -n 5
--live`, then `-n 20 --live`) has:

| sample | confirmed | missing / not-plain / illegal |
|---|---:|---|
| n=5, 514 lemmas | 79,830 | 140 / 6 / 0 |
| n=20, 1,505 lemmas | 228,245 | 140 / 6 / 0 |

Every missing/not-plain row is the same two class-stem cells: `41*E havas`
(139 missing + 6 not-plain) and `48*E live` (1 missing). There is **no A or B
row** at either size. The recorded examples now hold:

- `lovi`/`ovi`/`joki` all have their confirmed forms `Plain`
  (`check_word_against_omorfi.py` and the 20-lemma sample);
- `tarjoilu` offers `tarjoiluitteni`, `tarjoiluideni` and `tarjoilujeni`.

The remaining `41*E`/`48*E` residue is a class-stem defect (the abessive and
the oblique stem), not the possessive mechanism; it is recorded under
§8 instead of widening this slice. The `14,026`/`4,359` figures in
`docs/harness.md` predate the possessive rounds.

### 3.3 Design

**A — the demotion, not the possessive.** `docs/harness.md` records that the
`lovi` shape (`lovessa` from the weak stem, `lovilta` from the strong) comes
from `demote_contested` in `src/grammar/noun.rs`; the possessive layer then
correctly skips the non-`Plain` oblique. Adjust that rule so the singular
obliques and the plural obliques are each `Plain` on their own stem, then the
possessives follow. Start with `lovi`, `ovi`, `joki`, `noki`, `oikku`,
`allergikko` and the 20-lemma sample from `TODO.md`.

**B — ordering.** Only after A:

1. Collect every `Plain` spelling per slot into a `Vec<String>` instead of
   `.take(1)`.
2. Let the demotion pass for the slot run. For the catalogue path that is the
   `alignment_plain`/`oblique_plain` demotion at the end of `inflect_from`
   (the loop that turns a slid reading into `Guessed`), not the heuristic's
   `demote_contested`, which owns the case layer in `grammar/noun.rs`.
3. Build the possessives from the surviving spellings, as `add_possessives`
   already does for the oblique grid (`inflect.rs::add_possessives_for`).
   Attributes follow the surviving base spelling, not the slot.
4. Check the authoritative classes: `inflect_in_unmarked` clears the catalogue
   output and the generated `nominals` engine owns the grid there. If the
   missing rows come from a generated class, the fix belongs in the class rule
   source (`rules/`, `tools/oracle/extract_class_rules.py`) and is regenerated;
   do not hand-edit `src/expand/nominals/`.

Do not add a possessive for a spelling that was demoted to `Guessed`/`Repaired`
after the build; that is the orphan mechanism, not a coverage goal.

### 3.4 Steps

- [x] Re-run `python3 tools/report_by_class.py -n 5 --live` and `-n 20 --live`;
      re-bucket A/B (both empty).
- [x] Pin the failing families as tests: the fresh sweep and the existing
      `lovi`/`tarjoilu` checks cover them; no new unit test is needed for a
      measured no-op.
- [x] Land A in `src/grammar/noun.rs`: not needed, the `lovi`/`ovi`/`joki`
      gate passes.
- [x] Land B as the ordering change: not needed, `tarjoilu` offers all three
      genitive plurals.
- [x] Re-run the 5-lemma sweep, `--verify-illegal`, and the 20-lemma sample.

### 3.5 Gate

- A: the `lovi`/`ovi`/`joki` families and the 20-lemma sample have no new
  `not-plain` or `illegal`; the missing count falls.
- B: at least half of the recorded B rows are recovered and `illegal` does not
  rise above the pre-change baseline on the ten measured classes.
- The full 5-lemma report keeps 0 illegal under the curated pass and does not
  regress `missing`; `not-plain` does not rise above the current 4,041.
- `tests/catalogue.rs` and the `GAPS` staleness rules are untouched.

### 3.6 Stop condition

If B cannot be ordered without hand-editing a generated nominal engine, land A
alone, record the orphan list and leave B open. Do not hand-edit
`src/expand/nominals/`.

### 3.7 Files

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

- **wasm**: `check_typography(text) -> js_sys::Object` using the **same field
  names** as `diagnostic_object`: `kind: "typography"`, `rule_id` (the
  typography rule), `severity: "suggestion"`, `confidence: "certain"`,
  `start`, `end`, `message`, and `corrections[].edits[]` with
  `start`/`end`/`replacement`. Add a `typography_diagnostics_array` next to
  the existing serializers rather than inventing a second shape. No textarea
  state is needed.
- **Demo (Leptos)**: a **Typografia** toggle in `SyntaxPanel`. The crate already
  depends on `torille`, so it can call `torille::typography::check` directly.
- **Extensions**: a separate context-menu command **Tarkista typografia**
  routed through `spell-worker/spell-core.ts`; the panel maps
  `syntaxKindLabel("typography")` to `typografia` and can show the specific
  `rule_id` (`typography_semicolon_spacing`, `typography_semicolon_duplicate`)
  in the message. Keep Firefox and Chromium source-identical (the `npm test`
  source-sync check).
- **Options**: one toggle, default off, so the existing commands never change
  behaviour for users who do not opt in.
- Underline: reuse the suggestion style for now; a separate colour is a
  follow-up once the command exists.

### 4.3 Landed

Landed 2026-11-22:

- `check_typography` in `torille-wasm` returns the same diagnostic shape
  (`kind: "typography"`, stable `rule_id`, one edit per correction).
- The Leptos demo has a **Typografia** panel that lists and applies the edits.
- Both extensions have the **Tarkista typografia** context-menu command,
  routed through the worker/direct host to `bg.check_typography`; the panel
  maps `kind: "typography"` to `typografia`.
- The options toggle was not added: the command is already opt-in and the
  default grammar/style commands are untouched, which is the gate that
  matters. A settings toggle can wrap the command later without changing the
  wasm API.

Both extensions pass `npm run typecheck` and `npm test` (57 tests each); the
Leptos/wasm crates build.

### 4.4 Steps

- [x] Add the wasm export + serializer; the field shape is pinned by the
      TypeScript `RawDiagnostic` contract.
- [x] Add the Leptos panel.
- [x] Add the extension command and panel label; run both typechecks and
      `npm test` in each extension directory. The options toggle is deferred
      (the command is opt-in).
- [x] Document the command in `extensions/README.md` and the typography
      section of `docs/syntax_checker.md`.

### 4.5 Gate

- The default grammar/style commands return exactly the same diagnostics as
  before (no typography rows).
- The typography command returns only `typography_*` findings, applies edits
  without shifting, and leaves URLs/code/quotations untouched.
- Both extensions pass typecheck, unit tests and the source-sync check; the
  options toggle defaults off.

### 4.6 Files

`torille-wasm/src/lib.rs`, `torille-wasm-leptos/src/components/syntax.rs`,
`extensions/{firefox,chromium}/src/content/*`,
`extensions/{firefox,chromium}/src/background/*`,
`extensions/{firefox,chromium}/src/spell-worker/*` (including
`torille-wasm-glue.d.ts`), `extensions/{firefox,chromium}/src/globals.d.ts`,
`extensions/{firefox,chromium}/src/options/*`, `extensions/README.md`,
`docs/syntax_checker.md`.

## 5. Slice 4 — closed-spelling audit and residue

### 5.1 State

`src/expand/closed_spellings.rs` still has **six** rows, all the `kuulten`
family (`kuulten`, `kuultemme`, `kuulteni`, `kuultenne`, `kuultensa`,
`kuultesi`), documented as the less common postpositional instructive of
`kuulla`. The other 26 rows are gone: some are engine-derived now, the rest
are rejected as `is_nonword` (with a split-spelling suggestion) or pinned as
misspellings. The tracker said "32 → 0" before this plan; it now records
32 → 6, and its Progress list is the record of the 26 removals.

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
- **Fixture.** Check in the historical 32 rows with `authority`,
  `proposed_action` and `kind` (`derived`, `nonword`, `fast-only`, `lexical`),
  so a deletion or a re-addition is a reviewed change.
- **Failure rules.** The audit fails when a `derived` row stops resolving
  (regression), when a `lexical` row starts resolving without `CLOSED`
  (staleness), when a base is absent and undocumented, when a `fast-only` row
  has no probe, or when `CLOSED` gains a row that has no fixture row carrying
  an authority.
- **Parity.** For every fixture surface, `check` and `check_fast` agree; the
  six live rows resolve to `kuulla`; the rows marked `nonword` offer their
  two-word spelling and the `derived` rows resolve without `CLOSED`.
- **Tracker.** The header now records 32 → 6 and the Progress list the 26
  removals; record the `kuulten` decision (lexical, per the module doc) as
  closed.

### 5.3 Landed

Landed 2026-11-22 as `Closed::{Enabled, Disabled}` in
`src/expand/closed_spellings.rs`, threaded through `check_normal_with` (public
`check_normal_with_policy`) and the `is_known_word` stack. The audit tool
`tools/src/bin/audit_closed_spellings.rs` prints one row per historical
surface; `tests/fixtures/closed_spellings.json` carries the 32 rows with
`kind`/authority/action, and `tests/closed_spellings.rs` enforces the
invariants (full/fast parity, closed rows lexical, derived rows deriving,
nonword rows rejected). Current audit: 32 rows, all invariants hold; the six
`kuulten` rows are the only live `CLOSED` entries.

### 5.4 Steps

- [x] Fix the tracker status and the Progress arithmetic.
- [x] Add the `Closed` policy parameter and the audit binary.
- [x] Add the checked-in fixture and the failure rules.
- [x] Add the full/fast parity test over every fixture surface.
- [x] Benchmark `check_normal_fast`; the parameter does not move the keystroke
      path (production passes a constant `Enabled`).

### 5.5 Gate

- The audit is deterministic, fails when a closed row is re-added or becomes
  derivable, and passes on the six live rows.
- Full and fast paths agree for every fixture surface.
- `check_fast` timing is unchanged within noise.
- No new `CLOSED` row and no new `is_nonword` entry without a source.

### 5.6 Files

`src/expand/closed_spellings.rs`, `src/expand/check_normal.rs`,
`src/suggest/known/mod.rs` (`is_known_word_with_compound_uncached`),
`src/suggest/state.rs` (`check_fast`),
`tools/src/bin/audit_closed_spellings.rs` (new),
`tests/fixtures/closed_spellings.json` (new),
`tests/closed_spellings.rs` (new parity test),
`misc_md/TODO_CLOSED_SPELLINGS.md`.

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
  larger; on `page-2000` the 309 spans become 130 units, up to 9 fragments, so
  an edit inside a wrapped block re-checks the whole block. Measure the warm
  page; if the hit rate drops more than the parity is worth, keep the units
  and accept the miss.
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
