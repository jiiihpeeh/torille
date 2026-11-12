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
clean rows 112: 0 errors, 0 warnings, 0 suggestions
cases 52: every emitted kind precision 1.00, recall 1.00
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

## Surfaces

- **Leptos demo:** the `SyntaxPanel` runs `syntax_check::check` on a button
  press, lists diagnostics in rank order and applies corrections.
- **Firefox / Chromium extensions:** right-click a text field and choose
  **Tarkista kielioppi**. The check is opt-in and runs on one input at a time;
  a floating panel lists diagnostics and applies corrections to textarea,
  input and contenteditable fields. The extension calls
  `check_syntax_with_state` in the wasm worker, which converts UTF-8 byte
  offsets to UTF-16 before the panel sees them.
- **CLI:** `check_syntax` prints the analysis and diagnostics for one sentence
  or file.

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
- **Ambiguous partitive plurals stay quiet.** A plural completive suggestion
  is offered when every noun base names the same total form (`autoja →
  autot`); a homonymous surface such as `kirjoja` (`kirja`/`kirjo`) stays
  quiet instead of naming the wrong word.
- **Relative clauses are linked but not reparsed.** A relative clause carries
  `parent`, `children` and an `antecedent` token link, while the linear clause
  ranges stay the source of truth. Comma diagnostics still work on the linear
  ranges; a rule cannot yet ask for the host as one uninterrupted span.
- **The extension grammar check is context-menu only.** There is no per-site
  toggle or keystroke trigger yet; the context menu is the opt-in.
- **No grammar/style mixing.** Repetition, register and phrasing are a
  separate opt-in layer, [`docs/style_checker.md`](style_checker.md); the
  grammar checker does not emit those kinds.
