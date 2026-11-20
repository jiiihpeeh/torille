# Punctuation plan: from the high-precision core to the specification

Status: **phases 0–2 landed; phases 3–7 proposed.** The baseline is `b1ac21e`; the review that motivates this
plan is [`REVIEW_PUNCTUATION_SPEC.md`](REVIEW_PUNCTUATION_SPEC.md). The checker
is deliberately precision-first: a missing comma is cheaper than a wrong one.
This plan closes the highest-value gaps in that order.

## Scope

Cover the programmatic specification's rules that the current engine does not
yet implement, without turning the checker into a lexical trigger table:

- indirect-question commas;
- vocatives;
- a separate typography/whitespace layer;
- independent main-clause commas, with a real shared-constituent check;
- relative-clause opening commas;
- unseparated and semicolon lists;
- quotation/reporting, colon, semicolon and dash rules where the clause model
  can support them.

Non-goals:

- a full dependency parser or a TurkuNLP integration;
- automatic rewriting of ambiguous clause boundaries;
- optional stylistic commas;
- normalising code, URLs, file paths or user-chosen quotation style;
- sentence-final period insertion for headings and fragments.

## Baseline and acceptance

Current evidence at `b1ac21e`:

| gate | value |
|---|---|
| clean prose | 266 rows / 6,349 words |
| true positives | 2 |
| known false positives | 0 |
| `page-2000` grammar diagnostics | 3 (two agreement, one document-end quote) |
| punctuation false positives on `page-2000` | 0 |

Every phase must keep the clean prose quiet. A new rule that flags a clean row
is either fixed or moved to `known_false_positives` only after a reviewed
ambiguity pair — the same contract as the existing grammar rules.

## Architecture decisions

### Keep the flat clause model as the source of truth

`src/syntax/mod.rs`'s `Clause` stays the linear source of truth. The new work
adds **views** rather than replacing it:

- `Coordination` (new, additive): left/right clause indices, the coordinator
  token, the boundary, the subjects, and which constituents are shared.
- `Relative` already has `parent`, `children` and `antecedent`; the opening
  comma rule reads those, not a fresh search for `joka`.
- `Typography` is a separate pass with its own diagnostics; it does not enter
  `SyntaxAnalysis`.

### Rule identity and confidence

- Reuse `CommaMissing` for the missing-comma family and give the diagnostic a
  distinct `RuleId`/message where the user needs to dismiss it separately:
  `indirect_question_comma`, `main_clause_comma`, `relative_comma_open`,
  `vocative_comma`.
- Main-clause and relative-opening commas are `Warning`/`Likely` by default.
  Finnish punctuation allows structural and stylistic variation, and the
  checker must not overstate a `ja` boundary. A clear indirect question with a
  reporting verb can be `Error`/`Certain`; a vocative is `Suggestion`/`Likely`.
- Typography findings are `Suggestion` and live behind their own API.

### No lexical trigger rules

Every rule must name the syntactic evidence it used:

- indirect question: reporting/mental predicate + interrogative opener +
  clause boundary;
- main clause: two main clauses + no shared subject/constituent + coordinator
  or juxtaposition;
- relative: relative clause with an antecedent and a missing opening comma;
- vocative: address form in a clause-initial or clause-final position;
- list: parallel items with same case/POS, not two adjacent nouns alone.

## Phases

### Phase 0 — audit harness

**Landed.** `tests/fixtures/punctuation_spec.json` records the specification
matrix with `covered`/`gap` status; `tests/punctuation_spec.rs` asserts every
covered rule id and insertion point and skips the gaps. The exact-output net
stays `tests/fixtures/syntax_diagnostics.json`.

- Add `tests/fixtures/punctuation_spec.json` with the specification's required
  positive, negative and ambiguous examples. Initially mark unimplemented
  cases as `gap` so the suite can report progress without failing.
- Add a small `--audit` mode or tool output that runs the fixture and prints
  implemented/gap counts per rule.
- No runtime change.

Gate: the fixture parses, the baseline counts are recorded, and no existing
test moves.

### Phase 1 — indirect-question comma

**Landed** in `src/syntax_check/punctuation/comma.rs`.

Two paths share `indirect_question_comma`:

- a wh-clause the parser already segmented (`En tiedä missä hän asuu.`) is a
  cross-clause comma;
- a wh-opener or `-ko`/`-kö` verb merged into the host clause
  (`En tiedä milloin hän tulee.`, `Hän kysyi tulemmeko mukaan.`) is inferred
  from the reporting predicate and the opener.

A relative clause is excluded by `Clause::antecedent`, so
`Hän kertoi jotain mikä yllätti minut.` stays a relative clause. A cross-clause
wh-clause is `Error`/`Certain`; the merged path is `Warning`/`Likely`.

Implement `indirect_question_comma`:

- detect a main clause with a reporting/mental predicate (`tietää`, `kysyä`,
  `ymmärtää`, `miettiä`, `muistaa`, `kuulla`, `sanoa`, `kertoa`, and the
  imperatives `kerro`, `sano`);
- find the next indirect interrogative opener (`missä`, `mistä`, `mihin`,
  `miksi`, `miten`, `kuinka`, `milloin`, `kuka`, `mikä`, `onko`, `-ko/-kö`);
- require a clause boundary and no comma/dash there;
- insert `, ` before the opener.

Positive cases: `En tiedä missä hän asuu.`, `Hän kysyi tulemmeko mukaan.`,
`Kerro mitä tapahtui.` Negative: `Missä hän on?` (direct question),
`Hän tietää kaiken.`, `Tiedän että hän tulee.` (already handled as `että`),
`Kysyin, missä hän asuu.` (already punctuated).

Gate: positives fire; direct questions and reported speech with an existing
comma stay quiet; prose suite unchanged.

### Phase 2 — vocative

**Landed** in `src/syntax_check/punctuation/vocative.rs`.

The rule covers the three positions the specification names:

- sentence-initial greeting (`Hei Maija!`, `Terve Pekka!`, `Kiitos ystäväni.`,
  `Hyvää huomenta, kaikki.`);
- leading address before a second-person imperative (`Maija tule tänne.`,
  `Pekka tule!`);
- trailing address after one (`Tule Pekka!`, `Tule tänne Pekka.`).

Guards that keep it precise:

- only a second-person imperative counts; `Herra olkoon kanssanne` and
  `Jumala johdattakoon` are third-person jussives;
- a predicate with a finite indicative homonym is not an imperative
  (`Kunta saa periä maksun`);
- an adjective homonym is not an imperative (`Kaksi pieni lapsi.`);
- a leading address must be a `Subject`/`Unknown` nominal, not an adverbial
  (`Ajoneuvolla saa ...`);
- a trailing address is a name (or `kaikki`), never a possessive object
  (`näe minun käteni`) or an adposition complement (`ilman Bathsebaa`);
- rection skips an `Unknown`-role address after an imperative, so
  `Tule Pekka!` is not rewritten as an illative complement.

The finding is `Suggestion`/`Likely`; the comma is not an error.

Implement `vocative_comma` narrowly:

- sentence-initial interjection/address (`Hei`, `Terve`, `Moi`, `Kiitos`,
  `Hyvää huomenta`) followed by a name/address and then a clause;
- clause-final address after an imperative (`Tule tänne Pekka.`);
- use the POS/morphology to require an interjection or a proper/common noun in
  the address; do not fire on `Hei` as a noun or on a list.

Positive: `Hei Maija!`, `Terve Pekka!`, `Tule tänne Pekka.` Negative:
`Hei on huudahdus.`, `Maija tulee.`, `Tule tänne nopeasti.`

Gate: positive cases insert a comma; the negative cases and the prose suite
stay quiet. The rule is suggestion-only.

### Phase 3 — typography and whitespace

Add a separate `src/typography.rs` (or `src/syntax_check/typography.rs`) with
`check(text) -> Vec<TypographyDiagnostic>`:

- no space before `, . ; : ! ?`;
- one space after a comma/period/colon/semicolon in ordinary prose;
- no space inside parentheses or Finnish quotation marks;
- preserve decimal commas (`3,14`), dates (`20.11.2026`), times, ranges,
  abbreviations (`esim.`), URLs, code and repeated punctuation;
- do not add/remove spaces inside a quotation the user chose;
- edit list and byte offsets, suggestion-only.

Decide whether the demo shows typography findings in the same panel or a
separate toggle. The API is independent either way.

Gate: a new typography fixture with protected contexts; no syntax diagnostic
changes; `page-2000` unchanged or improved.

### Phase 4 — coordination split and main-clause comma

This is the structural phase.

1. Extend `src/syntax/segments.rs::segment_clause_words` to split at a
   coordinator when the right side has a subject and a finite predicate:
   - `ja` followed by a pronoun/name subject and then a finite verb
     (`Menin kotiin ja hän jäi töihin.`);
   - `mutta`, `vaan`, `sillä`, `joten`, `tai`, `vai`, `sekä` with a finite
     predicate on the right;
   - keep `Sami ja Liisa söivät.` and `Sami söi puuron ja joi maidon.` as
     they are, with the existing shared-subject test.
2. Add `Coordination` to `SyntaxAnalysis`:
   - left/right clause indices, coordinator, boundary token;
   - left/right subject and predicate;
   - shared subject/predicate/object flags after `share_clause_roles`.
3. Add `main_clause_comma`:
   - adjacent main clauses in one sentence, no comma/dash at the boundary;
   - if the subjects are not shared and there is no shared object/predicate,
     insert a comma before the coordinator (or at the juxtaposition boundary);
   - if the subjects are shared, stay quiet;
   - if one subject is omitted and the other is explicit, treat the boundary
     as non-shared only when the predicates' person/number disagree or the
     explicit subject is a different nominal.

The rule is `Warning`/`Likely`, not `Error`: `ja` can join closely connected
clauses without a comma in edited prose. `mutta`/`vaan` can be `Likely` with a
higher score.

Positive: `Menin kotiin ja hän jäi töihin.`, `Satoi mutta lähdimme ulos.`,
`Hän soitti minä en vastannut.` (juxtaposition, once segmentation supports it).
Negative: `Menin kotiin ja söin ruokaa.`, `Sami söi puuron ja joi maidon.`,
`Ostin leipää ja maitoa.`

Gate: all existing `src/syntax/mod.rs` clause tests and `tests/syntax_check`
fixtures stay green; page-2000 does not gain a main-clause false positive; the
shared-subject cases are explicit tests.

### Phase 5 — relative opening comma

- Use `Clause.antecedent`/`children` to find a relative clause whose antecedent
  has no opening comma.
- Insert a comma before the relative pronoun and keep the existing closing
  comma rule; use `relative_comma_open` as a `Suggestion`/`Likely` pair.
- Keep restrictive/determiner `joka` quiet (`joka päivä`, `joka esine`).
- Resolve the `Mies joka tuli eilen on opettaja.` case: the relative clause
  must be split from the host before the opening comma can be inserted; if the
  parser still merges it, record the limitation and do not guess.

Gate: the `relative_comma_pair` fixture still fires; the opening is inserted
for a clear non-restrictive relative; determiners stay quiet.

### Phase 6 — lists and semicolons

- Extend `syntax_list` to recognize unseparated parallel items after a verb or
  a colon: same case/POS, three or more, and no conjunction-only pair.
- Add semicolon-separated complex list support and the spec's
  `Virtanen, Maija; Laine, Pekka; ja Korhonen, Anna.` shape.
- Keep the current separator/duplicate/conjunction rules.

Gate: `Ostin omenoita banaaneja ja päärynöitä.` gains the missing comma;
`Ostin leipää ja maitoa.` and `kaksi poikaa ja tyttö` stay quiet; nested lists
and internal commas remain.

### Phase 7 — quotation, colon, semicolon and dash

The semicolon slice now has a dedicated review and plan:
[`REVIEW_SEMICOLON.md`](REVIEW_SEMICOLON.md) and
[`PLAN_SEMICOLON.md`](PLAN_SEMICOLON.md). It starts with the comma boundary
bug (`,;`), then adds suggestion-only structural misuse and a separate
typography pass.

Only once the quote/clause model can say which side owns the terminal mark:

- reporting clause + quotation (`Hän sanoi: ”Tulen huomenna.”`);
- no second terminal period outside a quotation;
- colon introducing a list/explanation/quotation;
- semicolon between independent units;
- dash spacing and interruption boundaries, preserving `–`/`—`.

These are mostly formatting/typography decisions; keep them separate from
clause commas and do not auto-convert `.` to `!`.

## Tests

For every rule, add the four cases the specification asks for:

1. **positive** — the missing punctuation is inserted;
2. **negative** — the same surface in a shared/integrated construction stays
   quiet;
3. **boundary** — the trigger in another grammatical role (`ja` shared vs
   independent, `kuin` comparison vs clause, `joka` determiner vs relative);
4. **ambiguous** — the engine preserves the text (`Confidence::Ambiguous` or
   no diagnostic).

Add the spec's test table to `tests/fixtures/punctuation_spec.json`; keep the
existing `tests/fixtures/syntax_diagnostics.json` cases as the regression net.
The prose suite (`tests/syntax_prose.rs`) and `page-2000` remain the
corpus-level gates.

## Risks

- **Coordination split changes roles.** Splitting `ja hän ...` can change
  agreement and object assignment. Run the full syntax, prose and page-2000
  gates after Phase 4; add the shared-subject test before the rule.
- **Main-clause comma is optional in edited Finnish.** Use `Likely`, not
  `Error`; never add a comma before `ja` from the word alone.
- **Relative clauses are ambiguous.** Restrictive relatives take no comma;
  the opening rule must be suggestion-only and respect `joka` determiners.
- **Typography is not grammar.** Keep it out of `check` by default; otherwise
  a space rule can mask a real syntax finding or flood the panel.
- **Lists are semantic.** Two adjacent nouns are not a list; require parallel
  case/POS and a list context.
- **The spec and Kotus differ on optional commas.** The rule's confidence and
  correction license must say which authority it follows; ambiguous cases stay
  quiet.

## Files

- `src/syntax/segments.rs`, `src/syntax/mod.rs` — coordination split/view;
- `src/syntax_check/punctuation/comma.rs` — indirect question, main clause,
  relative opening;
- `src/syntax_check/punctuation/vocative.rs` (new);
- `src/typography.rs` (new) and its fixtures;
- `src/syntax_list.rs` — unseparated/semicolon lists;
- `src/syntax_check/punctuation/mod.rs` — rule wiring;
- `tests/fixtures/punctuation_spec.json` (new),
  `tests/fixtures/syntax_diagnostics.json`, `tests/syntax_check.rs`,
  `tests/syntax_prose.rs`;
- `docs/syntax_checker.md`, `docs/grammar_syntax.md`,
  `misc_md/REVIEW_PUNCTUATION_SPEC.md`.

## Recommended first commit

Phase 1 + Phase 2 together: indirect questions and vocatives are independent of
the coordination split, low-risk, and give immediate coverage. Keep them in
one commit with the new fixture rows and the prose/page-2000 gates.

Phase 4 should be a separate commit: it changes clause segmentation, and the
diff must be attributable if a prose row moves.
