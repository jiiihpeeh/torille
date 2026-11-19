# PLAN_GRAMMAR_SYNTAX_LAYOUT — a render-safe feature reference

Status: **implemented.** The reference is now plain-text-first: numbered
pipeline, definition lists instead of long table cells, a compact diagnostic
index plus per-family sections, and `check_docs.py` column/cell/heading/
box-art limits with an optional `pandoc` render check. This is a presentation
and information-design plan for
[`../docs/grammar_syntax.md`](../docs/grammar_syntax.md). It does not change
the linguistic model. The semantic hardening plan is
[`PLAN_GRAMMAR_SYNTAX_REFERENCE.md`](PLAN_GRAMMAR_SYNTAX_REFERENCE.md).

The pasted copy of the reference exposed a separate problem: the **source** is
valid Markdown, but the **rendered/copy-pasted artifact** is not readable.
Tables lose their row boundaries, the pipeline diagram collapses, and text from
adjacent sections interleaves. This plan makes the document robust when it is
read in a Markdown viewer, a plain-text export, or a PDF/HTML print.

## 1. Symptoms in the pasted artifact

These are the concrete failures, not cosmetic preferences:

- The pipeline renders as one line of branch glyphs:
  `text │ ├─├─├─… tokenisesentence::spans…`
- The word-class table merges rows, for example `VerbNegativeVerb`.
- The possessive section receives a stray `verb, finite or non-finite …` row
  from the word-class table.
- The sentence-layer section receives a stray `built by repairing the lemma …`
  row from the confidence table.
- The `NodeKind` section receives a stray `predicative or governed complement`
  cell from the role table.
- The diagnostic table interleaves kind, rule, severity, correction and example
  columns into one undifferentiated block.
- The API and file-map tables lose their row boundaries.

The source file does not contain those interleavings. The export path is
therefore (a) flattening HTML tables into one text stream, or (b) applying a
multi-column PDF layout. The document is also **too table-heavy**: a table with
five long prose cells is fragile even when the source is correct.

## 2. Diagnosis

| observation | conclusion |
|---|---|
| `python3 tools/check_docs.py` passes | the Markdown source is structurally valid |
| the pasted copy reorders rows across sections | the renderer flattens tables or uses multi-column layout |
| the worst blocks are 4–5-column tables with long cells | the layout asks too much of a line-based renderer |
| the pipeline uses ASCII branch art | it has no chance in a plain-text/PDF flattening pass |
| the diagnostic table is 21 rows × 5 columns | it should be prose/lists, not a mega-table |

Root cause: **document shape**, not missing facts. The rewrite should be
plain-text-first. A Markdown table is a rendering convenience, not the primary
representation.

## 3. Goals

1. Copy-paste from a browser, GitHub/GitLab, `pandoc -t plain`, or a PDF keeps
   every sentence, rule and example in a readable order.
2. No fact, rule id, severity or example is lost.
3. The document remains compact enough to be useful as a reference.
4. The Markdown source remains valid and machine-checkable.
5. The document is single-column by construction; no side-by-side blocks.

## 4. Layout rules

Apply these throughout `docs/grammar_syntax.md`:

- **One concept per line.** A rule is a bullet with its id, severity, correction
  and example, not five table cells.
- **Tables only for short enumerations.** Keep 2–3 column tables whose cells are
  terms or very short phrases (cases, clitics, feature values). Do not put
  sentences in a table.
- **No ASCII box art.** Replace branch diagrams with a numbered list or a
  single arrow line.
- **No multi-column prose.** No columns, floats, or side-by-side cards.
- **One blank line around every block** (heading, paragraph, list, table,
  fence).
- **Short lines.** Target ≤ 80–90 columns in source; long lines wrap badly in
  plain text.
- **Examples stay with their rule.** Never put an example in a separate table
  column.

## 5. Phase 0 — reproduce and choose the target renderers

1. Capture the broken pasted artifact (or the PDF/HTML it came from) as the
   baseline.
2. Render the current source with:
   ```bash
   pandoc docs/grammar_syntax.md -f gfm -t html -s -o /tmp/grammar_syntax.html
   pandoc docs/grammar_syntax.md -f gfm -t plain --wrap=none -o /tmp/grammar_syntax.txt
   ```
3. Confirm the failure mode:
   - HTML: tables are intact → the renderer is the failure;
   - plain: tables flatten → the layout is the failure.
4. Decide the primary reading modes: GitHub/GitLab Markdown, `pandoc` HTML and
   plain text. If PDF is required, use a single-column CSS (`table {
   page-break-inside: avoid; }`, `body { max-width: 60em; }`) and verify it.
5. Record the chosen renderers in the document header.

**Gate:** the reproduction shows whether the source or the export is at fault;
no content is changed in this phase.

## 6. Phase 1 — pipeline and headings

Replace the branch-art pipeline with a numbered sequence:

```text
1. tokenise — word/separator runs with byte spans
2. sentence::spans — sentences, paragraphs, quote depth
3. morphology::analyze — readings, lemmas, case/number, depth
4. syntax::analyze_syntax — clauses, roles, features, lists
5. syntax::ast::build — Document/Sentence/Clause/Phrase/List/…
6. syntax_check — diagnostics, explanations, corrections
```

This preserves the same information and survives every line-based renderer.
Apply the same rule to any other diagram: no `├`, `└`, `│` outside a code
block; prefer a numbered list or a single `→` chain.

**Gate:** the plain-text export has one stage per line.

## 7. Phase 2 — convert fragile tables

### 7.1 Word classes

Replace the three-column table with a definition list:

```text
- **Noun** — common or proper noun. `talo`, `Sami`.
- **Adjective** — adjective, including degrees. `punainen`, `punaisempi`.
- **Verb** — verb, finite or non-finite. `sanoa`, `sanoo`, `sanottu`.
- **NegativeVerb** — the negation auxiliary. `en`, `ei`, `älä`.
...
```

This is the section where the pasted artifact merged `Verb` and
`NegativeVerb`; a list cannot merge them.

### 7.2 Case, clitics, degree

Keep a **two- or three-column table** only where every cell is a term or a short
phrase:

| case | example |
|---|---|
| nominative | `talo` |
| genitive | `talon` |

The current case table is acceptable if cell text stays short. Move the long
notes into a following bullet list per case group (core cases, local cases,
rare cases). Do not put a full sentence in a cell.

### 7.3 Morphology API

Replace the API table with a list:

```text
- `morphology::analyze_morphology(tokens, cache)` — one `WordInfo` per word.
- `morphology::MorphologyCache` — per-text cache; bounded.
...
```

Long purpose text in a table is the main cause of the five-column spread.

### 7.4 Clause features and coverage

The clause-feature table is short and should stay. The agreement and coverage
matrices should become one subsection per construction:

```text
#### Ordinary
- Controller: nominative subject.
- Agreement: subject person/number.
- Correction: inflection.
- Example: `Sami söi puuron.`
```

A matrix is readable on screen but not in plain text; lists are safer.

**Gate:** no table has more than three columns or a cell longer than roughly 90
characters.

## 8. Phase 3 — rewrite the diagnostic catalogue

The 21-row × 5-column table is the single most fragile block. Replace it with
one subsection per family and one bullet per kind:

```text
### Agreement

- `subject_predicate_agreement` — Error, or Warning for a weak role.
  Correction: inflection. `Koira nukkuvat.` → `nukkuu`.
- `connegative` — Error/Warning. Correction: inflection, 2sg imperative only.
  `En menen.` → `mene`.
- `attribute_agreement` — Error for number, Warning for case.
  Correction: inflection. `Punainen autot.` → `punaiset`.
...
```

Keep a separate compact **index** near the top of Part III:

| kind | rule id | severity |
|---|---|---|
| `subject_predicate_agreement` | `subject_predicate_agreement` | Error/Warning |
| `connegative` | `connegative` | Error/Warning |
...

Three columns, one short id per row. The full rule/example text is in the
family subsections. This preserves lookup and removes the mega-table.

The stable `DiagnosticKind → RuleId → severity → correction` contract can stay
as a **four-column table** only if the cells remain short; otherwise use a
definition list per kind. The contract test remains the source of truth.

**Gate:** every one of the 21 kinds appears exactly once in the index and once
in a family subsection; `tests/syntax_contract.rs` still passes.

## 9. Phase 4 — tooling

Extend `tools/check_docs.py`:

- maximum table columns (default 3; allow 4 only for the diagnostic index);
- maximum cell length (warn at 90, error at 140 characters);
- reject box-drawing characters (`├`, `└`, `│`, `─`) outside fenced code;
- check every heading is followed by a blank line;
- check every fenced block is balanced (already implemented);
- check every relative link (already implemented).

Add a render check:

```bash
pandoc docs/grammar_syntax.md -f gfm -t plain --wrap=none \
  -o /tmp/grammar_syntax.txt
python3 tools/check_docs.py docs/grammar_syntax.md
```

The plain export is a review artifact, not a committed second copy. Do not
commit generated HTML/PDF/plain files; commit the Markdown source.

If a PDF is required, add a small `docs/grammar_syntax.css` with a single-column
layout and `table { page-break-inside: avoid; }`, and render with
`pandoc --css`. Keep the CSS out of the Markdown content.

## 10. Phase 5 — content preservation check

Before and after the rewrite, compare:

- all 21 `DiagnosticKind` names;
- every `RuleId` in the contract table;
- every severity/correction claim;
- every code path and test reference;
- all relative links;
- every example sentence.

A simple script can extract backticked identifiers from the old/new files and
diff the sets; the exact-output and contract tests remain the executable check.

## 11. Recommended order

1. **Pipeline** and heading spacing.
2. **Word classes / API** tables → lists.
3. **Diagnostic mega-table** → index + family subsections.
4. **Agreement/coverage matrices** → construction subsections.
5. **`check_docs.py` limits** and `pandoc` plain/html render check.
6. **Content diff** and final link/table validation.

## 12. Acceptance

- Copying the Markdown source into a plain-text field yields ordered lines, not
  interleaved table cells.
- `pandoc -f gfm -t plain` output has one rule/example per line and no merged
  rows.
- No table has >3 columns (the diagnostic index is the documented exception) or
  a cell over the length limit.
- Every rule, example and code path from the current document is present.
- `python3 tools/check_docs.py` passes.
- `tests/syntax_contract.rs` and `cargo test --release --workspace` pass.
- The document is still single-source Markdown; no generated HTML/PDF/plain
  copy is committed.

## 13. Non-goals

- No change to the morphology, syntax or checker model.
- No new diagnostics, no new roles and no new AST nodes.
- No hand-editing of a broken PDF/plain paste as the source of truth.
- No dual maintenance of a Markdown and a separate "plain" reference; the
  Markdown must itself be plain-text-friendly.

## 14. Risks

| risk | mitigation |
|---|---|
| lists become less scannable than tables | keep a compact index table and short family subsections |
| the rewrite drops a rule/example | extract identifier sets before/after; contract test |
| the renderer is actually a PDF exporter | add the single-column CSS and verify with `pandoc` |
| wide tables return | `check_docs.py` column/cell limits |
| duplication with `syntax_checker.md` | keep the checker rule catalogue canonical there; this document indexes and links |

## 15. Relationship to the semantic plan

`PLAN_GRAMMAR_SYNTAX_REFERENCE.md` answers *what* the reference must say
(case evidence, construction features, confidence, API contracts). This plan
answers *how* it must be laid out so those claims survive a real reader. Do the
layout phase first only where it blocks review; do not reopen the semantic
decisions while moving text.

## 16. Implementation record

Landed on 2026-11-19:

- `docs/grammar_syntax.md` was rewritten in place:
  - the branch-art pipeline is a numbered six-stage list;
  - the terminology, word-class, case, API, role, feature, family, confidence,
    data, performance, guarantee, coverage and file-map tables are definition
    lists;
  - the 21-row diagnostic mega-table is a three-column index plus family
    sections (agreement; rection/case/objects; arguments/structure;
    punctuation/lists/questions/quotes; reserved);
  - the only remaining table is the diagnostic index.
- `tools/check_docs.py` now enforces:
  - at most three table columns;
  - table-cell warning at 90 characters and failure at 140;
  - no box-drawing characters outside a fence;
  - a blank line before and after every heading;
  - the existing row-count, link, anchor and fence checks;
  - an opt-in `--render` that runs `pandoc -f gfm -t plain` and rejects
    box-drawing or over-wide lines in the export.
- Content preservation: the 496 inline-code identifiers in the old source were
  diffed against the new source; only the standalone `ListKind` span was
  re-added, and all diagnostic kinds, rule ids, examples and links remain.
- Verification:
  - `python3 tools/check_docs.py` and `python3 tools/check_docs.py --render`
    pass;
  - the pandoc plain export keeps one pipeline stage per line and one
    diagnostic rule per bullet;
  - `tests/syntax_contract.rs` is unchanged and still the executable contract.

A single-column print CSS was not added: the document now renders safely as
plain text, and a CSS sheet is only needed if a PDF becomes a required
artifact. Do not commit generated HTML/PDF/plain copies.
