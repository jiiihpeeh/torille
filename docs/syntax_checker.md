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
  - a stable `DiagnosticKind` name and a stable `RuleId` (the message key,
    suitable for ignore/dismiss lists);
  - a `Severity`: `Error`, `Warning` or `Suggestion`;
  - a `Confidence`: `Certain`, `Likely` or `Ambiguous`;
  - a byte span in the source text;
  - a stable `message` key;
  - an optional `expected` value for debugging;
  - an `Explanation` with the triggering `Relation` and a stable key;
  - zero or more `Correction`s, each of which is a list of `Edit`s and records
    its `CorrectionLicense` (`Inflection`, `Curated`, `Punctuation`) and its own
    `Confidence`.
- A correction is applied by replaying its edits **from the end of the text
  backwards**, so earlier offsets stay valid.

The grammar and style layers share these primitives (`src/diagnostic.rs`) and
implement the `Finding` trait; `CheckAll::findings()` exposes both streams
through it. `Confidence::Ambiguous` findings are suppressed before they reach a
caller, and an `Error` whose confidence is only `Likely` is downgraded to a
`Warning`, so a rule cannot overstate an ambiguous target.

The demo panel is in `torille-wasm-leptos/src/components/syntax.rs`; it runs
the checker only when the user presses the grammar button.

## Rule catalogue

| kind | rule | severity |
|---|---|---|
| `subject_predicate_agreement` | subject and finite predicate agree in person and number | `Error`, or `Warning` when a role is weak |
| `connegative` | the main verb after a negative auxiliary is in the connegative form | `Error` |
| `attribute_agreement` | attributive adjective/demonstrative agrees with its head | `Error` for number, `Warning` for case |
| `predicative_agreement` | a predicative adjective agrees with its subject in number | `Error` for a clear number mismatch, `Warning` when a role is weak |
| `case_selection` | a cardinal numeral governs the partitive singular, a curated adposition governs a non-matching case, or an existential subject is genitive | `Error` for a clear complement, `Warning` when a role is weak |
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

Reserved but not emitted: `ClauseStructure`, `WordOrder`, `CommaExtra`. Word
order is deliberately deferred: Finnish order is information structure, not a
fixed rule.

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
gate is `tests/fixtures/syntax_diagnostics.json`. The
`tests/fixtures/syntax_ambiguity.json` gate pins the intended reading and the
competing reading that must not win for ambiguous surfaces (`joka päivä`,
`ilman`, `sen`, `sinut`, and the determiner/relative split).
`tests/fixtures/prose/adjudicated.json` is the reviewed-prose gate: clean rows
must stay quiet, true positives must match their kinds, and known false
positives pin the current bugs so a fix has to move the row. As of 2026-11-17
the prose fixture has 66 clean rows / 1,021 words and no known false positives.

Current baseline (2026-11-15):

```text
clean rows 154: 0 errors, 0 warnings, 0 suggestions
cases 61: every emitted kind precision 1.00, recall 1.00
```

Run both gates:

```bash
cargo test --release -p torille --test syntax_check --test syntax_eval --test syntax_ambiguity --test syntax_prose
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
  input and contenteditable fields. On Firefox the background resolves
  morphology in a pool of up to four workers (the `grammarThreads` setting,
  1–4); Chromium's service worker cannot create nested workers and stays on
  the sentence-batch path. The worker converts UTF-8 byte offsets to UTF-16
  before the panel sees them. After the check the same spans
  are underlined: grammar errors orange double, warnings amber dashed and
  suggestions amber dotted. Spelling keeps its red wavy underline and style is
  blue solid; the grammar/style underlines clear on the next edit. The
  grammar colours and the grammar/style visibility toggles are configurable
  in the options page.
- **CLI:** `check_syntax` prints the analysis and diagnostics for one sentence
  or file.

## Performance

The checker is on demand and must not run in the keystroke path. Its cost is
dominated by the first morphology pass over a text, not by the rules.
`syntax_check::check_with_cache` and `torille::check_all_with_cache` share a
per-textarea `morphology::MorphologyCache`; each `WordInfo` caches the readings
plus the agreed case/number, part of speech, clitic and possessive, and the
style layer reads the same pass. The wasm combined export runs the grammar
rules through `sentence_cache::SentenceCache`: an unchanged sentence is a hash
probe, and one edited sentence re-checks in ~2.6–3.7 ms on the 1,959-word
fixture. On a long text the demo and the Firefox extension resolve morphology
in a 4-worker pool, collect the encoded rows in the owning state, and run
syntax/style once over the full document; the full page drops from ~3.0 s to
~1.4 s. Chromium's MV3 service worker cannot create nested workers, so it
keeps the sentence-batch path. The measured baseline, the cold-path pruning
and the pool are in
[`PLAN_GRAMMAR_STYLE_PERF.md`](../PLAN_GRAMMAR_STYLE_PERF.md) and
[`PLAN_PARALLEL_MORPHOLOGY.md`](../PLAN_PARALLEL_MORPHOLOGY.md).

## Data files

| file | content |
|---|---|
| `data/verb_rection.csv` | curated `lemma,case,frame,priority,note` valency frames |
| `data/verb_arguments.csv` | curated `lemma,slot,case,requirement,note` argument-completeness frames |
| `data/adposition_cases.csv` | curated `adposition,case,example` governed-case rows |
| `maps/verb_rection.bin.gz` | `(verb lemma → governed cases)` |
| `maps/verb_arguments.bin.gz` | `(verb lemma → argument frames)` |
| `maps/adposition_cases.bin.gz` | `(adposition → governed cases)` |

These tables are small and curated. Do not infer a valency frame, an
obligatory argument or an adposition's case from a suffix or a dictionary
class.

## Known limitations

- **Word order** is not checked.
- **Predicative agreement is number-only and adjective-only.** `Talot ovat
  punainen` and `Talo on punaisia` are caught; noun complements and participle
  complements are not, because their agreement/case behaviour needs a separate
  rule.
- **Connegative findings are safest in declarative clauses.** `en mene`,
  `hän ei menee` and the negative past are covered. Imperative auxiliaries
  (`älä`, `älkää`) are detected but currently get no automatic correction; the
  conditional connegative is accepted as the 3sg conditional surface, and the
  potential is left quiet.
- **Numeral government is deliberately narrow.** Only a nominative complement
  or a partitive plural is flagged; a non-nominative, non-partitive complement
  (`kaksi kolmesta`) is left alone. `yksi`, plural numerals (`kahdet`) and
  non-cardinal numerals (`toinen`) are skipped.
- **Adposition government is a curated list.** Only rows in
  `data/adposition_cases.csv` are checked; an adposition's case is lexical, so
  it is never inferred from the surface. The rule fires on a nominative or
  partitive against a genitive frame and on a nominative against a partitive
  frame. A genitive after a partitive-governing preposition is left quiet
  because it may modify a following nominal or participle
  (`ilman Jumalan tahtomatta`), and a multi-case adposition stays quiet.
- **Existential subject case detects, but does not choose the repair.** A
  sentence-final genitive common noun after an existential `on` is an error
  (`Pöydällä on kirjan.`). The engine offers both licensed forms
  (`kirja`, `kirjaa`) because boundedness and count/mass choose between them;
  the diagnostic is `Certain`, each correction `Likely`. The rule skips
  pronouns, proper names, and clause-final genitives created by a line break.
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
