# Syntax checker

`torille::syntax_check` is the grammar and punctuation checker built on the
advisory analysis layer in [`src/syntax.rs`](../src/syntax.rs). It answers a
small, high-confidence set of questions and returns concrete corrections. It
is **not** a parser, a style checker, or part of the keystroke path.

## Contract

- `check(text)` tokenises, runs `syntax::analyze`, applies the rules, and
  returns `SyntaxCheck { analysis, diagnostics }`.
- `check_with_cache(text, cache)` is the per-textarea form; the wasm export
  `check_syntax_with_state(idx, text)` uses the same word cache.
- Every diagnostic carries:
  - a stable `DiagnosticKind` name;
  - a `Severity`: `Error`, `Warning` or `Suggestion`;
  - a `DiagnosticConfidence`: `High`, `Medium` or `Low`;
  - a byte span in the source text;
  - a stable `message` key;
  - an optional `expected` value for debugging;
  - zero or more `Correction`s, each of which is a list of `Edit`s.
- A correction is applied by replaying its edits **from the end of the text
  backwards**, so earlier offsets stay valid.

The demo panel is in `torille-wasm-leptos/src/components/syntax.rs`; it runs
the checker only when the user presses the grammar button.

## Rule catalogue

| kind | rule | severity |
|---|---|---|
| `subject_predicate_agreement` | subject and finite predicate agree in person and number | `Error`, or `Warning` when a role is weak |
| `attribute_agreement` | attributive adjective/demonstrative agrees with its head | `Error` for number, `Warning` for case |
| `verb_rection` | curated valency frame requires a case the argument lacks | `Error` with a likely predicate, otherwise `Warning` |
| `object_case` | negative clause requires a partitive object; aspect cues suggest partitive/total | `Error` for negation, `Suggestion` for aspect |
| `missing_argument` | an obligatory slot from `data/verb_arguments.csv` is absent | `Warning` |
| `extra_argument` | a closed intransitive frame has an object | `Warning` |
| `comma_missing` | subordinate clause needs a comma; `esimerkiksi` needs a parenthetical pair | `Error`, or `Suggestion` for the parenthetical |
| `comma_pair_unbalanced` | an opened relative-clause comma pair is not closed | `Error` |
| `question_mark_missing` | a clear direct question ends with `.` or no mark | `Error` |
| `unbalanced_quote` | a quotation is still open at the end of the text | `Warning` |
| `list_separator_missing` | two adjacent list items have no separator | `Error` |
| `list_separator_duplicate` | a list item is separated twice | `Error` |
| `list_comma_before_conjunction` | a comma before the final `ja`/`tai`/`sekä` | `Suggestion` |
| `list_introducer_comma` | a comma directly after a list-introducing colon | `Error` |

Reserved but not emitted: `CaseSelection`, `ClauseStructure`, `WordOrder`,
`CommaExtra`. Word order is deliberately deferred: Finnish order is
information structure, not a fixed rule.

## False-positive policy

Ship fewer rules, not more guesses.

- `Error` requires a clear cue and, for role rules, a likely role and a single
  relevant reading.
- Unknown words, unknown roles, ambiguous agreeing targets, and numeral
  phrases suppress the rule.
- A clean corpus failure is a release blocker.
- A semicolon is never an error.
- A question mark is an `Error` only when the sentence has one clause and its
  first word is interrogative or carries `-ko`/`-kö`.
- An unbalanced quote is a `Warning`; quote style and dialogue dashes are not
  corrected.
- Parenthetical commas are suggestions, limited to a closed cue list.
- A final conjunction without a comma is never an error in Finnish.

## Evaluation

`tests/fixtures/syntax_eval.json` is the aggregate corpus; the exact-output
gate is `tests/fixtures/syntax_diagnostics.json`.

Current baseline (2026-11-12):

```text
clean rows 109: 0 errors, 0 warnings, 0 suggestions
cases 50: every emitted kind precision 1.00, recall 1.00
```

Run both gates:

```bash
cargo test --release -p torille --test syntax_check --test syntax_eval
```

The evaluation test prints the per-kind table; the exact-output fixture checks
every span and correction. A new rule that adds an extra diagnostic to a case
lowers precision and must be explained in its commit, but the hard gate is
zero `Error` on clean rows.

For one sentence during development:

```bash
cargo run --release -p torille-tools --bin check_syntax -- "Koira nukkuvat."
```

## Data files

| file | content |
|---|---|
| `data/verb_rection.csv` | curated `lemma,case,frame,priority,note` valency frames |
| `data/verb_arguments.csv` | curated `lemma,slot,case,requirement,note` argument-completeness frames |
| `maps/verb_rection.bin.gz` | `(verb lemma → governed cases)` |
| `maps/verb_arguments.bin.gz` | `(verb lemma → argument frames)` |

Both tables are small and curated. Do not infer a valency frame or an
obligatory argument from a suffix or a dictionary class.

## Known limitations

- **Word order** is not checked.
- **Plural completive object suggestions** are deferred: a partitive plural is
  often homonymous (`kirjoja` belongs to both `kirja` and `kirjo`), so the
  syntax layer's chosen lemma can name a different word (`kirjot`).
- **`kuten` parenthetical pairs** are deferred: the parenthetical is the
  following phrase, not the cue word, and phrase extent is not modelled.
- **Relative clauses are linear.** The host clause is split into contiguous
  ranges; there is no `Clause.parent` or antecedent link.
- **Nested lists hold one child per item.** A second bracketed list in the
  same item is not represented yet.
- **The browser extensions do not yet expose a grammar toggle.** The Leptos
  demo panel is the current UI; the extensions remain spell-check-only.
- **No style layer.** Repetition, register and phrasing are out of scope; a
  style analyzer would be a separate opt-in module with its own kinds.
