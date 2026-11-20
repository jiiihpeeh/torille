# Syntax checker

`torille::syntax_check` is the grammar and punctuation checker built on the
advisory analysis layer in [`src/syntax/`](../src/syntax/mod.rs). It answers a
small, high-confidence set of questions and returns concrete corrections. It
is **not** a parser, a style checker, or part of the keystroke path.

For the full feature inventory across morphology, syntax and every diagnostic
kind, see [`grammar_syntax.md`](grammar_syntax.md).

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
through it. `SyntaxCheck`, `StyleAnalysis` and `CheckAll` expose
`without_rules(&[RuleId])`/`ignore_rules`, so an ignore list filters by stable
rule id rather than by span. `Confidence::Ambiguous` findings are suppressed before they reach a
caller, and an `Error` whose confidence is only `Likely` is downgraded to a
`Warning`, so a rule cannot overstate an ambiguous target.

The demo panel is in `torille-wasm-leptos/src/components/syntax.rs`; it runs
the checker only when the user presses the grammar button.

## Rule catalogue

| kind | rule | severity |
|---|---|---|
| `subject_predicate_agreement` | subject and finite predicate agree in person and number; an active past participle in a compound tense agrees in number | `Error`, or `Warning` when a role is weak |
| `connegative` | the main verb after a negative auxiliary is in the connegative form | `Error` |
| `attribute_agreement` | attributive adjective/demonstrative agrees with its head | `Error` for number, `Warning` for case |
| `predicative_agreement` | a predicative adjective/participle agrees with its subject in number; a clear noun-predicate number mismatch | `Error` for a clear adjective number mismatch, `Warning` for a noun predicate or weak role |
| `possessive_agreement` | a possessive suffix matches an adjacent genitive personal pronoun | `Warning` for a wrong suffix, `Suggestion` when it is missing |
| `case_selection` | a cardinal numeral governs the partitive singular, a curated adposition governs a non-matching case, or an existential subject is genitive | `Error` for a clear complement, `Warning` when a role is weak |
| `verb_rection` | curated valency frame requires a case the argument lacks | `Error` with a likely predicate, otherwise `Warning` |
| `object_case` | negative clause requires a partitive object; a singular bare nominative object is an error; aspect cues suggest partitive/total | `Error` for negation and the bare nominative, `Suggestion` for aspect |
| `missing_argument` | an obligatory slot from `data/verb_arguments.csv` is absent | `Warning` |
| `extra_argument` | a closed intransitive frame has an object | `Warning` |
| `clause_structure` | a declarative clause has no finite predicate, or ends in a coordinator without a coordinated predicate | `Warning`, never a correction |
| `comma_missing` | subordinate clause needs a comma; `esimerkiksi` needs a parenthetical pair; a `kuin` comparison boundary does **not** need one | `Error`, or `Suggestion` for the parenthetical |
| `comma_extra` | a comma directly between a personal-pronoun subject and its finite predicate | `Warning` with a `Likely` removal |
| `comma_pair_unbalanced` | an opened relative-clause comma pair is not closed | `Error` |
| `question_mark_missing` | a clear direct question ends with `.` or no mark; a dash-interrupted or line-wrapped question gets no mark at the break, and a correlative relative or rhetorical conditional stays quiet | `Error` |
| `unbalanced_quote` | a quotation is still open at the end of the text | `Warning` |
| `list_separator_missing` | two adjacent list items have no separator | `Error` |
| `list_separator_duplicate` | a list item is separated twice | `Error` |
| `list_comma_before_conjunction` | a comma before the final `ja`/`tai`/`sekä` | `Suggestion` |
| `list_introducer_comma` | a comma directly after a list-introducing colon | `Error` |

Reserved but not emitted: `WordOrder`. The construction-aware specification is
[`docs/word_order.md`](word_order.md): Finnish order is information structure,
SVO is a preference rather than a correctness rule, and the future kind stays
suggestion-only.

The features that spec needs are already computed on `Clause::features`
(`clause_type`, `construction`, `subject_kind`, `question_type`,
`subject_position`, `object_case`, `focus_type`, `topic_position`,
`register`). They are exposed through the analysis only — no rule reads them
yet, and the `ALLOW` regression net (`tests/word_order_allow.rs`) proves the
sentences a naive order rule would flag stay clean.

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
positives pin the current bugs so a fix has to move the row. As of 2026-11-19
the prose fixture has 266 clean rows / 6,349 words, 2 true positives and 0 known
false positives; `page-2000` is at 5 diagnostics.

Current baseline (2026-11-19):

```text
syntax_eval clean rows 154: 0 errors, 0 warnings, 0 suggestions
syntax_eval cases 60: every emitted kind precision 1.00, recall 1.00
syntax_diagnostics: 75 exact-output cases, 272 clean rows
prose: 266 clean rows / 6,349 words, 2 true positives, 0 known false positives
page-2000: 5 diagnostics
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
  **Tarkista kielioppi**, or **Tarkista kielioppi ja tyyli** for the combined
  pass. The check is opt-in and runs on one input at a time; each finding can
  be dismissed by its stable rule id, and the options page clears the list.
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
[`../misc_md/PLAN_GRAMMAR_STYLE_PERF.md`](../misc_md/PLAN_GRAMMAR_STYLE_PERF.md) and
[`../misc_md/PLAN_PARALLEL_MORPHOLOGY.md`](../misc_md/PLAN_PARALLEL_MORPHOLOGY.md).

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

- **Word order** is not checked. The clause features that a future rule needs
  exist (`Clause::features`), and every example in
  [`docs/word_order.md`](word_order.md) is pinned clean, but no rule reads them.
- **Predicative agreement is number-only.** `Talot ovat punainen` and
  `Talo on punaisia` are caught for adjectives and active participles. A noun
  predicate has a semantic partitive/nominative choice, so the separate noun
  rule only reports a clear number mismatch: a nominative singular predicate
  with a plural subject (`He ovat opettaja.`) or a nominative plural predicate
  with a singular subject. Partitive predicates (`He ovat opettajia.`,
  `Ne ovat vettä.`) stay quiet, and the correction offers both licensed plural
  forms with a `Likely` correction confidence.
- **Adjective degree is a tag, not a separate rule.** `Tags.degree` marks
  comparative and superlative forms, and `attribute_agreement` /
  `predicative_agreement` compare them like any adjective. Degree derivations
  are `Repaired`, so `Punaisempi autot ...` is diagnostic-only: the checker
  names the expected number but does not offer a `Repaired` rewrite.
- **Inverted order puts the subject after its verb.** A finite predicate
  followed immediately by a nominative personal pronoun has that pronoun as
  its subject: `Sen sanoi hän.`, `Kysyi hän sitten.`. Anything before the
  predicate is then not the subject — in a quotation it is a vocative
  (`..., Heintsi', sanoi hän sille toiselle`), so the quoted pronoun is not
  diagnosed either.
- **An infinitive clause agrees through the verb that governs it.**
  `alkoi tulla ... puute` takes the number of `alkoi`, while `Haluan mennä
  koulu` still fails: a 1st-person verb cannot take the noun `koulu` as its
  subject, so the nominative there is the rection error.
- **A 1st/2nd-person predicate takes only a pronoun as its subject.** A
  nominal beside it is an apposition to the dropped pronoun
  (`Me suuremmat istumme`), and a line break can leave the apposition alone in
  its clause (`suuremmat istumme`), so person is not compared against an
  adjective.
- **`vaikka` opens a clause only when a predicate follows.** `kerron vaikka
  tästä` is one clause and takes no comma; `vaikka hän ei kuuntelisi` is a
  subordinate clause. The other subordinators keep their unconditional split,
  because their clause can end the line.
- **The agreement bearer must be a clear finite form.** A surface that is
  both a finite form and an instructive or infinitive is not diagnosed:
  `ihastellen` is the potential 1sg of `ihastella` *and* its instructive, and
  the instructive is what a text means (`sinä rannassa, ihastellen kalojani`).
  The same guard keeps `elää` (`antaa hänen elää`) and `ihmetellen` quiet.
  `kaikki` likewise never decides the number: it takes a singular verb as
  “everything” and a plural one as “all of them”.
- **A clause divider is a comma plus a subject that runs into its verb.** A
  comma followed by a personal pronoun and then a finite predicate starts a
  new clause (`kun häneltä kysyttiin, hän vastasi`), the pronoun counterpart
  of the existing capitalised-name rule (`..., Sikri soitti`). A vocative
  (`sinä, Heintsi', sanoi ...`) is not one, because the pronoun is followed by
  a comma rather than its verb.
- **Only a real attribute is compared.** In a clause with a finite predicate
  only a `Modifier`-role token is an agreement candidate; a pronoun in an
  oblique case (`minulle`, `niille`), an object or a complement is not. Among
  the pronouns only the demonstratives can be attributive, so
  `Siellä uusiintui minulle ruumis ja sielu.` and `panna niille raha-arvo`
  stay quiet. A predicate-less fragment keeps the shape-based reading
  (`Punaisessa auto.`), because there is no role to rely on.
- **Compound-tense agreement covers the active past participle only.**
  `Me olemme lukenut.` gives `lukeneet`; `Minä olen lukeneet.` gives
  `lukenut`. Passive compound tenses (`Talo on rakennettu`,
  `Talot on rakennettu`) deliberately stay quiet because the passive
  participle does not agree with the subject. An adjective homonym
  (`väsynyt`) is resolved through the verb lemma before a correction is
  offered.
- **Connegative findings are safest in declarative clauses.** `en mene`,
  `hän ei menee` and the negative past are covered. The 2sg imperative
  (`älä menet.`) now offers `mene`; the plural/third-person imperative
  connegatives (`älkää menkö`, `älköön menkö`) are detected but stay
  uncorrected because the engine emits those surfaces as `Repaired`. The
  conditional connegative is accepted as the 3sg conditional surface, and the
  potential is left quiet.
- **Numeral government is deliberately narrow.** Only a nominative complement
  or a partitive plural is flagged; a non-nominative, non-partitive complement
  (`kaksi kolmesta`) is left alone. `yksi`, plural numerals (`kahdet`) and
  non-cardinal numerals (`toinen`) are skipped. The morphology follows the same
  line: only a **nominative** cardinal triggers the partitive upgrade that
  looks past a direct lemma (`kaksi lasta` finds the `lapsi` partitive), so
  `kahtena päivänä` and `kolmesta miehestä` resolve shallowly and the deep
  walk is not run after an inflected numeral.
- **Adposition government is a curated list.** Only rows in
  `data/adposition_cases.csv` are checked; an adposition's case is lexical, so
  it is never inferred from the surface. The rule fires on a nominative or
  partitive against a genitive frame and on a nominative against a partitive
  frame. A genitive after a partitive-governing preposition is left quiet
  because it may modify a following nominal or participle
  (`ilman Jumalan tahtomatta`), and a multi-case adposition stays quiet.
- **Rection targets a genuine argument, not a subject or an attribute.** A
  `Subject`-role token is treated as the verb's argument only when it cannot be
  the subject: the clause is existential, it has coordinated subjects, or the
  finite predicate does not agree with it (person and number for a pronoun,
  3rd person and number for a nominal). An adjective directly after a nominal
  is that nominal's attribute (`jotain erinomaista`), and a participle
  complement belongs to `olla` only. So `Sitten auttoi kreivi komean ratsun.`,
  `Rinteellä tuli hirvittävä susi.` and `Minun on täytynyt pitää
  perunkirjoitus.` stay quiet, while `Menen koulu.`, `Pidän sinä.` and
  `Luotan sinä.` still fire: a 1st-person verb cannot take a noun or a
  2nd-person pronoun as its subject.
- **Attributes are placed before roles.** An adjective, participle or
  demonstrative determiner immediately before a nominative noun is that
  noun's attribute (`Milloin tulee se kesy hirvi?` names `hirvi` as the
  subject, not `se` or `kesy`). A nominalised adjective with no head (`Nuoret
  tulivat.`) is left alone, and a cardinal numeral is left to the numeral
  rule. A postverbal coordinated subject is a *noun* phrase: an adjective
  joined by `ja` is a coordinated complement, so `oli hän yhtä viaton ja
  herttainen kuin kaunis` keeps its singular subject.
- **Existential subject case detects, but does not choose the repair.** A
  sentence-final genitive common noun after an existential `on` is an error
  (`Pöydällä on kirjan.`). The engine offers both licensed forms
  (`kirja`, `kirjaa`) because boundedness and count/mass choose between them;
  the diagnostic is `Certain`, each correction `Likely`. The rule skips
  pronouns, proper names, and clause-final genitives created by a line break.
- **Bare nominative objects are a narrow rule.** The total object is genitive
  singular or nominative plural, so only a singular common-noun object can be
  the error. The rule requires an overt preverbal subject, a postverbal
  clause-final object, no possessive and no competing nominal case; it skips
  imperative, passive, necessive and comparison constructions. `Näen kirjat.`,
  `Syö puuro!`, `Syötiin puuro.` and `Minun täytyy lukea kirja.` stay quiet.
- **Extra commas are detected only in the pronoun-subject slot.** A comma
  directly between `minä`/`sinä`/`hän`/`me`/`te`/`he` and its finite predicate
  is a `Warning`, with a `Likely` removal. Appositions, parentheticals
  (`Minä, yksin, tulen.`), noun-subject addresses (`Liisa, tule tänne.`) and
  any intervening word stay quiet; a deliberate pause keeps this below
  `Error`.
- **Possessive agreement is warning/suggestion only.** `Minun kirjasi on uusi.`
  warns and `Minun kirja on uusi.` suggests, but neither is an `Error` and
  neither carries a correction: colloquial Finnish drops the suffix. A suffix
  with no expressed pronoun (`Kirjani on uusi.`) is correct and stays quiet; a
  genitive pronoun before a necessive verb is the verb's subject, not a
  possessor.
- **Clause-structure warnings are deliberately narrow.** A missing predicate
  is reported only for a sentence-final subordinate clause with a
  personal-pronoun subject and a predicative word but no finite verb
  (`Koska minä väsynyt.`). Incomplete coordination is reported only when a
  clause with a predicate ends in a coordinator at a declarative period
  (`Hän tuli ja.`). Questions, headings, comma-continued clauses and
  period-less fragments stay quiet; neither warning carries a correction.
- **Ambiguous partitive plurals stay quiet.** A plural completive suggestion
  is offered when every noun base names the same total form (`autoja →
  autot`); a homonymous surface such as `kirjoja` (`kirja`/`kirjo`) stays
  quiet instead of naming the wrong word.
- **Relative clauses are linked but not reparsed.** A relative clause carries
  `parent`, `children` and an `antecedent` token link, while the linear clause
  ranges stay the source of truth. The closing comma is checked from the
  linear clause pair when the parser swallowed the host predicate; a rule
  still cannot ask for the host as one uninterrupted span.
- **The extension grammar check is context-menu only.** There is no per-site
  toggle or keystroke trigger yet; the context menu is the opt-in.
- **No grammar/style mixing.** Repetition, register and phrasing are a
  separate opt-in layer, [`docs/style_checker.md`](style_checker.md); the
  grammar checker does not emit those kinds.

## Remaining work

The grammar/syntax fix plan is landed through the user-facing phases:
predicate-less nominal phrases, missing relative closing commas, the 2sg
imperative connegative correction, and interrogative/host constituent views.
The conditional/potential connegative is closed as won’t-fix, and the optional
`Clause.sentence` / flat `ClauseKind` cleanup remains deferred without a
consumer. The full record is
[`../misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md`](../misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md).

A clause-by-clause comparison with the programmatic Finnish punctuation
specification — including the missing main-clause, indirect-question, vocative,
apposition and typography layers — is in
[`../misc_md/REVIEW_PUNCTUATION_SPEC.md`](../misc_md/REVIEW_PUNCTUATION_SPEC.md).
The staged implementation plan is
[`../misc_md/PLAN_PUNCTUATION_SPEC.md`](../misc_md/PLAN_PUNCTUATION_SPEC.md).
