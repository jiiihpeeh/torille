# Semicolon plan: from a scanning boundary to a precision-first rule

Status: **proposed.** Baseline `a80e616`. The review that motivates this plan
is [`REVIEW_SEMICOLON.md`](REVIEW_SEMICOLON.md); the broader punctuation plan is
[`PLAN_PUNCTUATION_SPEC.md`](PLAN_PUNCTUATION_SPEC.md).

The semicolon is the opposite of the comma: a comma is often grammatically
required, while a semicolon is usually a stylistic choice between two valid
marks. The implementation therefore starts by **protecting valid semicolons**
and only then adds suggestions for the structural cases the specification
forbids.

## Scope

Implement the programmatic specification's semicolon rules that the engine can
support with its current clause/list analysis:

- a valid semicolon between independent clauses stays quiet;
- a semicolon at a main/subordinate boundary is corrected to a comma;
- a semicolon inside a subject/predicate, verb/object or verb/complement
  phrase is removed;
- a sentence-final semicolon becomes a period;
- semicolon-separated complex lists are modelled and accepted;
- semicolon spacing is handled by a separate typography layer;
- code, URLs and technical notation are protected from typography edits.

Non-goals:

- automatic conversion of a valid comma to a semicolon;
- automatic insertion of semicolons into an ordinary simple list;
- inferring a semantic relationship (`contrast`, `consequence`, `explanation`)
  from context;
- correcting quotation/colon/reporting-clause punctuation here (that stays in
  the quote phase);
- touching semicolons inside code, URLs or inline code.

## Baseline and acceptance

Current evidence at `a80e616`:

| gate | value |
|---|---|
| clean prose | 266 rows / 6,349 words |
| true positives | 2 |
| known false positives | 0 |
| `page-2000` grammar diagnostics | 3 (two agreement, one document-end quote) |
| semicolons in the clean corpus | 7, all main-main/stylistic |
| valid main-main semicolon diagnostics | 0 |

Every phase must keep the seven corpus semicolons quiet. A new rule that flags
one of them is a regression, not a stylistic improvement.

## Architecture decisions

### The semicolon is a clause boundary, not a sentence boundary

`src/syntax/segments.rs::top_level_semicolon` already splits clauses at a
bracket-depth-zero semicolon and leaves parenthetical/quoted semicolons alone.
That stays. The semicolon never ends a sentence; the sentence layer keeps the
whole span, and the clause layer sees two clauses.

### One shared boundary predicate

`comma.rs` currently has `comma_between` plus a local `dash_between`.
`subordinate_comma` and `relative_comma` do not see `;`, so they insert a comma
before it. Add a shared helper in `src/syntax_check/analysis.rs`:

```rust
pub(super) fn boundary_between(tokens: &[Token], left: usize, right: usize) -> bool
```

It returns true for a comma, semicolon, colon or dash in the separator run.
The comma rules use it to decide “a boundary already exists”; the new
semicolon rule uses the same scan to find the mark and its byte span.

This is deliberately a *boundary* predicate, not a “punctuation is correct”
predicate: it only stops a second mark being inserted.

### One new diagnostic kind

Add `DiagnosticKind::SemicolonMisuse` with distinct rule ids:

- `semicolon_subordinate` — replace `;` with `,`;
- `semicolon_coordination` — replace `;` with `,` between coordinated
  sentence elements;
- `semicolon_phrase` — delete `;` (and the following space when present);
- `semicolon_terminal` — replace `;` with `.`.

All are `Suggestion`/`Likely`; the correction is licensed by
`CorrectionLicense::Punctuation`. A valid main-main semicolon emits nothing.

A future `SemicolonMissing` is reserved but not implemented until the list
hierarchy is reliable. Do not add it in this slice.

### List model

`syntax_list::List` keeps `separators: Vec<usize>` for AST compatibility and
gains a parallel separator kind:

```rust
pub enum ListSeparator { Comma, Semicolon }
pub separator_kinds: Vec<ListSeparator>
```

`separators` then holds every top-level separator token (comma or semicolon),
so the AST and validation layers keep skipping them generically. Existing
consumers that need comma positions keep using `comma_positions`; new consumers
read `separator_kinds`.

The parser splits on top-level semicolons first when a clause contains one;
each group may contain internal commas. The resulting items are phrases, and
the list is `Nominal` when every group is nominal and has no finite predicate.
If there is no top-level semicolon, the existing comma parser is unchanged.

The old test `a_semicolon_does_not_make_one_list` is an intentional policy
change: `Ostin omenoita; appelsiineja ja banaaneja.` should now be a list with
semicolon separators. The test is updated to assert that shape, and the new
behaviour is documented.

### List diagnostics and agreement

- `lists.rs` runs comma-only checks (`duplicate`, `comma before conjunction`,
  `introducer comma`) only when the separator at that position is a comma.
- A semicolon-separated simple list of three or more short, parallel items
  gets one `Suggestion` correction that replaces each `;` with `,`. Do not add
  `ja` and do not change an item's internal commas.
- A complex list with an internal comma in at least one item stays quiet: the
  semicolon hierarchy is the correct reading.
- `agreement.rs::check_complements` skips a complement that is a list item.
  This is the minimal fix for the observed `Virtanen` false positive and is
  independent of the full semicolon list model.

### Typography is separate

Add `src/typography.rs` (or `src/syntax_check/typography.rs`) with its own
`TypographyDiagnostic` and `check(text)`. It does not enter
`SyntaxAnalysis` and is not part of the default grammar call.

Rules:

- remove whitespace before `;`;
- insert one space after `;` unless it is end-of-line, end-of-text or before a
  closing quote/bracket;
- collapse a duplicated `;`;
- never touch a semicolon inside a fenced code block, inline code, a URL, a
  numeric/technical expression or a quoted passage the user chose.

The protected-context scan is a small state machine over the raw text:
fenced code (```), inline code (`...`), `http://`/`https://`/`www.` runs, and
decimal/date/time runs. The existing tokeniser has no code-span model, so this
must live in the typography pass rather than in `Token`.

## Phases

### Phase 0 — audit fixture

- Add `tests/fixtures/semicolon_spec.json` with FI-SEM-001 … FI-SEM-015 and
  `covered`/`gap` status.
- Add `tests/semicolon_spec.rs` to assert the covered rule id/span and skip the
  gaps, mirroring `tests/punctuation_spec.rs`.
- Record the seven clean-corpus semicolons as clean rows.

Gate: fixture parses; no runtime change; baseline counts recorded.

### Phase 1 — stop the comma-before-semicolon defect

- Add `analysis::boundary_between` and use it in `subordinate_comma`,
  `relative_comma`, `extra_comma` and the list/comma rules that ask “is there
  already a boundary?”.
- Keep `clause_separator_between` (the indirect-question helper) or fold it
  into the shared predicate.

Tests:

```text
Kun tulin kotiin; kaikki nukkuivat.   # no comma before ;
Hän sanoi; että hän tulee.            # no comma before ;
Hän tuli; koska satoi.                # no comma before ;
Kun tulin kotiin, kaikki nukkuivat.   # existing comma behaviour unchanged
Hän tuli; minä lähdin.                # clean main-main semicolon
```

Gate: no `comma_missing` at or before a semicolon; the exact fixture and clean
prose stay green.

### Phase 2 — structural semicolon misuse

Add `SemicolonMisuse` and the four narrow rules.

`semicolon_subordinate`:

- a clause boundary where the previous clause is `Subordinate`/`Relative` or
  the next clause is `Subordinate`/`Relative`;
- the `;` is in the separator run between the two clauses;
- correction replaces `;` with `,`.

Tests: `Kun tulin kotiin; kaikki nukkuivat.`, `Hän sanoi; että hän tulee.`,
`Hän tuli; koska satoi.`, `Koira, joka nukkuu; on minun.` Negative:
`Hän tuli; minä lähdin.`, `Hän tuli; mutta minä jäin.` (the corpus has a
semicolon before `eivätkä`; do not flag main-main coordinators).

`semicolon_coordination`:

- the segment after `;` has no finite predicate and no overt subject, contains
  a coordinator (`ja`/`sekä`/`tai`), and its coordinated adjectives/nominals
  continue the previous clause's complement or object role;
- correction replaces `;` with `,`.

Test: `Hän on ystävällinen; kohtelias ja avulias.` → `Hän on ystävällinen,
kohtelias ja avulias.` Negative: `Hän on opettaja; minä lääkäri.` (elliptical
main clause with a subject; stay quiet).

`semicolon_phrase`:

- the segment after `;` has no finite predicate and no overt subject, and its
  first nominal is governed by the previous clause's predicate as an object or
  complement;
- or the segment before `;` has no finite predicate and is a bare subject
  nominal while the next segment has the predicate;
- correction deletes `;` and the following single space.

Tests: `Ostin; leipää.` → `Ostin leipää.`, `Hän on; väsynyt.` → `Hän on
väsynyt.`, `Opiskelijat; valmistautuivat kokeeseen.` → `Opiskelijat
valmistautuivat kokeeseen.` Negatives: `Hän on opettaja; minä lääkäri.`
(elliptical main clause with a subject), `Hän tuli; nopeasti.` (stylistic
afterthought: stay quiet), `Hän tuli; minä lähdin.`

`semicolon_terminal`:

- a top-level semicolon followed by no word before the sentence/paragraph end;
- correction replaces `;` with `.`.

Test: `Hän lähti kotiin;` → `Hän lähti kotiin.`

Gate: these are the only new suggestions; clean prose and `page-2000` stay
quiet; no `Error`.

### Phase 3 — semicolon-aware lists and the agreement guard

- Extend `syntax_list` with `ListSeparator`/`separator_kinds` and the
  semicolon-first split.
- Update `a_semicolon_does_not_make_one_list` to assert the list shape.
- Keep `Kokoukseen …; …; …` and `Matkakohteita …; …; ja …` quiet.
- Fix `Osallistujat olivat Virtanen, Maija; Laine, Pekka; ja Korhonen, Anna.`
  by skipping list-item complements in `agreement.rs`.
- Add a narrow simple-list suggestion: three or more short semicolon-separated
  items, no internal commas, same case/POS, after a verb or colon → replace
  each `;` with `,`. Negative: a complex list with an internal comma; a list
  whose items are long clauses; a main-main semicolon pair.

Gate: the complex-list rows are quiet; the simple-list row gets a suggestion;
`page-2000` and the clean prose stay unchanged.

### Phase 4 — typography

- Add the separate typography API and its protected-context scanner.
- Semicolon rules: no space before, one after, collapse duplicates.
- Do not emit these through `syntax_check` by default; the demo/extension can
  show them behind a separate toggle or a separate panel section.
- Add a typography fixture with:
  `sana ; toinen` → `sana; toinen`,
  `sana;toinen` → `sana; toinen`,
  `sana;;toinen` → `sana; toinen`,
  `https://example.com/a;b` unchanged,
  `` `let x = 1; let y = 2;` `` unchanged,
  `3,14` and `20.11.2026` unchanged.

Gate: no grammar diagnostic changes; `page-2000` unchanged; code/URL rows are
untouched.

### Phase 5 — deferred: missing semicolons

Do not implement until Phase 3 and a reviewed complex-list corpus exist.

- Suggest semicolons only when a list has more than one item, at least one item
  contains an internal comma, and every item boundary is unambiguous.
- Emit one suggestion with a complete edit set, never a per-comma rewrite.
- Keep the “do not replace all list commas” rule from the specification.

This phase is recorded so the audit fixture can mark FI-SEM-003 as a future
recommendation rather than a silent gap.

## Tests

For every new rule, the four cases the specification asks for:

1. **positive** — the forbidden semicolon is corrected;
2. **negative** — a valid main-main semicolon stays quiet;
3. **boundary** — the same surface in another role (`;` inside parentheses,
   inside a quote, before a coordinator in edited prose);
4. **ambiguous** — the engine preserves the text (elliptical clause, long
   stylistic enumeration).

Add the rows to `tests/fixtures/syntax_diagnostics.json` for exact output and to
`tests/fixtures/semicolon_spec.json` for the audit matrix. The prose suite and
`page-2000` remain the corpus-level gates.

## Risks

- **The corpus uses semicolons stylistically.** All seven clean rows are
  main-main; a rule that flags a semicolon before `eivätkä`, `nyt` or a
  literary main clause is a false positive. Keep the main-main path silent.
- **Complex lists are semantic.** `Virtanen, Maija; …` needs a hierarchy the
  comma parser does not have. Model it, but do not auto-insert missing
  semicolons.
- **The comma bug is separate.** Fixing `boundary_between` must not change any
  existing comma diagnostic except the three `,;` rows; run the exact fixture
  and prose gates.
- **Typography can corrupt code.** The protected-context scanner is mandatory
  before any spacing edit is offered.
- **Old policy conflict.** `PLAN_SYNTAX_CHECKER.md` says “never suggest
  replacing a comma with a semicolon or vice versa”. This plan keeps the
  no-style-upgrade half and allows a *forbidden* semicolon to be corrected to a
  comma/period, suggestion-only. Update that note when Phase 2 lands.
- **`missing_argument` on the colon list.** The current `Tarvitsemme kolme
  asiaa: aikaa; rahaa; osaamista.` warning is the wrong layer. Phase 3 should
  suppress it when the segment is a semicolon list, or Phase 2 should replace
  it with the semicolon suggestion; do not leave both.

## Files

- `src/syntax_check/analysis.rs` — shared `boundary_between`;
- `src/syntax_check/punctuation/comma.rs` — use the shared boundary;
- `src/syntax_check/punctuation/semicolon.rs` (new) — misuse rules;
- `src/syntax_check/punctuation/mod.rs` — rule wiring;
- `src/syntax_check/diagnostic.rs` — `SemicolonMisuse` kind;
- `src/syntax_check/agreement.rs` — list-item complement guard;
- `src/syntax_list.rs` — `ListSeparator`, `separator_kinds`, semicolon split;
- `src/syntax_check/punctuation/lists.rs` — separator-aware list diagnostics;
- `src/typography.rs` (new) — spacing and protected contexts;
- `tests/fixtures/semicolon_spec.json` (new),
  `tests/fixtures/syntax_diagnostics.json`,
  `tests/semicolon_spec.rs` (new), `tests/syntax_check.rs`,
  `tests/syntax_prose.rs`;
- `docs/syntax_checker.md`, `docs/grammar_syntax.md`,
  `misc_md/PLAN_SYNTAX_CHECKER.md` (policy note),
  `misc_md/REVIEW_SEMICOLON.md`.

## Recommended first commit

Phase 0 + Phase 1 together: the audit fixture and the boundary fix. That closes
the `,;` defect without adding a new diagnostic, and it is the change most
likely to affect the existing gates.

Phase 2 is a separate commit. Phase 3 changes `syntax_list`, so it must be
separate from the boundary fix; if a prose row moves, the diff is attributable.

Phase 4 is independent of Phases 1–3 and can land in either order once the
protected-context scanner is tested.
