# Review: semicolon handling against the programmatic Finnish semicolon specification

Status: **review.** Baseline `a80e616`; Phase 0–1 of the plan have since
landed (audit fixture and the comma-boundary fix). The normative text is the
programmatic specification for Finnish semicolons (FI-SEM-001 … FI-SEM-015);
the existing punctuation review is
[`REVIEW_PUNCTUATION_SPEC.md`](REVIEW_PUNCTUATION_SPEC.md) and the staged plan
is [`PLAN_PUNCTUATION_SPEC.md`](PLAN_PUNCTUATION_SPEC.md).

The short verdict: the engine already treats a top-level semicolon as a clause
boundary, and valid main-clause semicolons stay quiet. It has **no semicolon
rule**, one **correctness bug in the comma layer**, and a **list false
positive**. The next work is therefore not “add a semicolon diagnostic” first;
it is “stop the comma layer from writing a comma in front of a semicolon”, then
add a narrow suggestion-only misuse rule and a separate typography layer.

## Evidence

The corpus is small but important: the clean prose fixture and `page-2000`
contain seven semicolons, all between independent clauses or stylistic main
units. Any rule that flags a main-main semicolon would break the clean-prose
gate.

| input | current output |
|---|---|
| `Hän tuli; minä lähdin.` | quiet |
| `Hän lähti kotiin; minä jäin.` | quiet |
| `Hän tuli; mutta minä jäin.` | quiet |
| `Kun tulin kotiin; kaikki nukkuivat.` | quiet after Phase 1 (before Phase 1: `comma_missing` at `16..16`, **before** the semicolon) |
| `Hän sanoi; että hän tulee.` | quiet after Phase 1 (before Phase 1: `comma_missing` at `10..10`, **before** the semicolon) |
| `Hän tuli; koska satoi.` | quiet after Phase 1 (before Phase 1: `comma_missing` at `9..9`, **before** the semicolon) |
| `Opiskelijat; valmistautuivat kokeeseen.` | `semicolon_misuse` / `semicolon_phrase`, `Suggestion`, deletion |
| `Ostin; leipää.` | `semicolon_misuse` / `semicolon_phrase`, `Suggestion`, deletion |
| `Hän on; väsynyt.` | `semicolon_misuse` / `semicolon_phrase`, `Suggestion`, deletion |
| `Hän on ystävällinen; kohtelias ja avulias.` | `semicolon_misuse` / `semicolon_coordination`, `Suggestion`, `;` → `,` |
| `Hän lähti kotiin;` | `semicolon_misuse` / `semicolon_terminal`, `Suggestion`, `;` → `.` |
| `Ostin omenoita; banaaneja; appelsiineja.` | `semicolon_misuse` / `semicolon_simple_list`, `Suggestion`, `;` → `,` for both |
| `Tarvitsemme kolme asiaa: aikaa; rahaa; osaamista.` | `semicolon_misuse` / `semicolon_simple_list`, `Suggestion`; the old `missing_argument` is suppressed |
| `sana ; toinen` | quiet |
| `sana;toinen` | quiet |
| `Osallistujat olivat Virtanen, Maija; Laine, Pekka; ja Korhonen, Anna.` | quiet after Phase 3 (before Phase 3: `predicative_agreement` at `20..28`, rewriting `Virtanen` to `virtaset` / `virtasia`) |
| `Kokoukseen osallistuivat Virtanen, toimitusjohtaja; Laine, projektipäällikkö; Korhonen, talousjohtaja.` | quiet |
| `Matkakohteita olivat Helsinki, Suomi; Tukholma, Ruotsi; ja Oslo, Norja.` | quiet |
| `Hän tuli (nopeasti; hiljaa) kotiin.` | quiet; one clause |
| `Hän sanoi: "Tulen huomenna; minä lähden."` | quiet; quote does not split |
| `let x = 1; let y = 2;` | quiet; no typography layer |

The first three `comma_missing` rows were the critical defect: applying the
correction would have produced `Kun tulin kotiin,; kaikki nukkuivat.` The
comma layer checked for a comma or dash between the clauses but not for `;`.
Phase 1 fixed it by sharing `analysis::boundary_between`; those rows are now
quiet and the exact fixture pins them.

## Spec matrix

| rule | status | evidence |
|---|---|---|
| FI-SEM-001 two closely related independent clauses | **accepted by omission** | `Hän tuli; minä lähdin.` quiet. The spec says “allow”, not “require”; quiet is correct. |
| FI-SEM-002 independent clauses without conjunction | **accepted** | `Hän lähti kotiin; minä jäin.` quiet. |
| FI-SEM-003 complex list with internal commas | **addressed (quiet)** | `syntax_list` joins the semicolon groups of one sentence into one list, each group a phrase item. All three complex rows are quiet, and the `Virtanen` agreement false positive is gone because a complement that is a list item is skipped. |
| FI-SEM-004 long complex enumeration | **accepted by omission** | No rule; this is the safe stylistic default. |
| FI-SEM-005 contrasting independent clauses | **accepted** | `Hän oli väsynyt; hän lähti kotiin.` quiet. |
| FI-SEM-006 explanatory independent clauses | **accepted** | Same shape; no rule. |
| FI-SEM-007 consecutive related statements | **accepted** | Same shape; no rule. |
| FI-SEM-008 main + subordinate | **addressed (suggestion)** | `Tiedän; että …`, `Kun …; kaikki …` and `Koira, joka …; on …` get `semicolon_subordinate`. |
| FI-SEM-009 simple list, prefer commas | **addressed (suggestion)** | `Ostin omenoita; banaaneja; appelsiineja.` and the colon-introduced `Tarvitsemme …` row get one `semicolon_simple_list` correction replacing every `;` with `,`. |
| FI-SEM-010 subject + predicate | **addressed (suggestion)** | `Opiskelijat; valmistautuivat kokeeseen.` gets `semicolon_phrase`. |
| FI-SEM-011 verb + object | **addressed (suggestion)** | `Ostin; leipää.` and `Hän on; väsynyt.` get `semicolon_phrase`. |
| FI-SEM-012 coordinated sentence elements | **addressed (suggestion)** | `Hän on ystävällinen; kohtelias ja avulias.` gets `semicolon_coordination`. Object coordination through a semicolon list is owned by the list layer: a short parallel list gets commas, a complex one keeps its hierarchy. |
| FI-SEM-013 sentence termination | **addressed (suggestion)** | `Hän lähti kotiin;` gets `semicolon_terminal`. |
| FI-SEM-014 whitespace before | **gap** | `sana ; toinen` quiet. |
| FI-SEM-015 missing whitespace after | **gap** | `sana;toinen` quiet. |

Two additional findings:

1. *Resolved in Phase 3.* The colon/semicolon simple list `Tarvitsemme kolme
   asiaa: aikaa; rahaa; osaamista.` used to get a `missing_argument` warning.
   That was the wrong layer and the wrong explanation; the sentence is a list,
   not a missing verb argument. It is now one `semicolon_simple_list`
   suggestion.
2. Code and URL protection is untested. `let x = 1; let y = 2;` and
   `https://example.com/a;b` are quiet today only because no rule looks at
   them. A typography pass would need an explicit protected-context scanner.

## Where the behaviour comes from

- **Clause splitting is already right.** `src/syntax/segments.rs`
  `top_level_semicolon` splits at a bracket-depth-zero semicolon and leaves
  semicolons inside parentheses and quotations alone. `syntax/mod.rs` has
  tests for both.
- **The comma rules do not share that boundary predicate.**
  `src/syntax_check/punctuation/comma.rs` uses `comma_between` and a local
  `dash_between`. `subordinate_comma` therefore sees no boundary at `;` and
  inserts a comma before it. The newer `indirect_question_comma` already has
  `clause_separator_between`, which does include `;`; that helper is the
  pattern the rest should share.
- **`syntax_list` is comma-only.** `top_level_commas` drives item splitting,
  `List.separators` holds comma token indices, and the explicit test
  `a_semicolon_does_not_make_one_list` pins the current policy. The AST
  consumers (`syntax/ast/lists.rs`, `syntax/ast/validate.rs`) only skip
  separator tokens, so they can tolerate semicolon entries, but the parser
  cannot produce them yet.
- **`lists.rs` is comma-specific.** It finds duplicate separators and missing
  separators with `comma_positions`. A semicolon list is not a list to this
  layer, so the comma inside `Virtanen, Maija` is parsed as a two-item list
  and the first item becomes the clause complement.
- **`agreement.rs` does not consult lists.** `check_complements` sees the
  singular `Virtanen` as a predicate complement of the plural subject
  `Osallistujat` and offers `virtaset`/`virtasia`. This is the concrete false
  positive to remove.
- **There is no typography layer.** `syntax_check` has no spacing diagnostics
  and no protected-context model, which is why `sana ; toinen` and
  `sana;toinen` are both quiet.

## Policy decisions

The old punctuation plan says “A semicolon is never an error” and “Do not
suggest replacing a comma with a semicolon or vice versa; Finnish usage is
stylistic.” That remains right for **valid** semicolons. The new specification
adds cases where a semicolon is structurally forbidden, so the policy is
refined, not reversed:

1. **A semicolon itself is never an `Error`.** Valid main-main semicolons stay
   quiet, including the seven corpus rows and semicolons before a coordinating
   conjunction in edited prose.
2. **Never convert a valid comma to a semicolon.** The engine may suggest
   replacing a semicolon with a comma when the comma is grammatically
   required, but it must not “upgrade” a comma for style.
3. **Forbidden semicolons are suggestions.** A semicolon between a main and a
   subordinate clause, between a subject and predicate, between a verb and its
   object/complement, or at sentence end gets a `Suggestion`/`Likely`
   correction (`;` → `,`, deletion, or `;` → `.`). None of these is an
   `Error`.
4. **Complex lists are accepted before they are corrected.** The first
   deliverable is to model semicolon groups and stop the agreement false
   positive. Missing-semicolon insertion is deferred until the list hierarchy
   is reliable; the specification itself says not to replace all list commas
   automatically.
5. **Spacing is a separate layer.** No whitespace around `;` is a
   `typo`/`typography` finding, not a grammar diagnostic, and it must not
   touch code, URLs or technical notation.

## What is already correct

- `Hän tuli; minä lähdin.` is a clean main-main semicolon and must stay clean.
- A semicolon inside `(...)` or a quotation does not split the host clause.
- `Hän tuli (nopeasti; hiljaa) kotiin.` is one clause and stays quiet.
- The main-main corpus rows (`Peura …; nykyään …`, `… pois; heidän täytyi …`,
  `… teettämä; nyt …`, `… nähnytkään; hän meni …`, `… koettaa; eivätkä …`,
  `Bathseba, kylmene; kylmä minäkin olen.`) stay quiet. They are the regression
  net for any new semicolon rule.

## Recommended next slice

1. **Fix the comma boundary bug first.** *Done in Phase 1:* one shared
   `boundary_between` now covers `subordinate_comma`, the relative closing
   comma and the indirect-question rule, and the three failing sentences are
   exact tests.
2. **Add a narrow `SemicolonMisuse` suggestion.** *Done in Phase 2:* four
   `Suggestion`/`Likely` rules cover main+subordinate (`;` → `,`), complement
   coordination (`;` → `,`), subject/predicate or verb/object/complement
   (`;` deleted), and sentence-final (`;` → `.`).
3. **Make the list layer semicolon-aware.** *Done in Phase 3:* semicolon
   groups are one list, the complex-list examples stay quiet, the `Virtanen`
   agreement false positive is gone and a short parallel list gets commas.
4. **Add the typography pass.** No space before `;`, one space after, duplicate
   semicolons; protect code, URLs and numeric/technical expressions.
5. **Defer missing-semicolon insertion.** It needs the list hierarchy and a
   reviewed corpus; the specification treats it as a recommendation, not a
   mandatory correction.

The detailed phases, files, tests and gates are in
[`PLAN_SEMICOLON.md`](PLAN_SEMICOLON.md).
