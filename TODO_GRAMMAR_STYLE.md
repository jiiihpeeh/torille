# TODO — grammar, syntax and style roadmap

A roadmap for the opt-in grammar (`syntax_check`), syntax analysis (`syntax`)
and style (`style_check`) layers. The design records are
[`docs/syntax_checker.md`](docs/syntax_checker.md),
[`docs/style_checker.md`](docs/style_checker.md),
[`PLAN_SYNTAX.md`](PLAN_SYNTAX.md),
[`PLAN_SYNTAX_CHECKER.md`](PLAN_SYNTAX_CHECKER.md) and
[`PLAN_STYLE.md`](PLAN_STYLE.md).

Status on 2026-11-15: the planned phases are landed and the targeted syntax and
style suites are green. The first false-positive slice is in. This file is the
next roadmap, not a list of known red tests.

## Principles

1. **A wrong `Error` on correct prose is worse than a missing advisory rule.**
   Precision on real Finnish outranks rule count.
2. **Fix interpretation before adding rules.** A checker with hundreds of rules
   is unreliable if the morphology readings or syntactic roles are wrong.
   Prefer role disambiguation, reading consistency and confidence over another
   surface pattern.
3. **Keep the layers separate.** `syntax` understands structure, `syntax_check`
   evaluates correctness, `style_check` suggests expression. The checker must
   consume the syntax analysis, not grow a second parser beside it.
4. **Respect legitimate variation.** Word order, optional commas, colloquial
   possessive omission and copula complement case are not automatic errors.
5. **Detection and correction are different claims.** A finding can be valid
   even when no safe automatic replacement is available.
6. **Adjudicated prose is the release metric.** Constructed fixtures prove
   rules; reviewed prose proves the checker is trustworthy.

## Roadmap at a glance

| Stage | Work | Why |
|---|---|---|
| P0 | Morphological-ambiguity regression suite | Protect the interpretation fixes |
| P0 | Adjudicated Finnish prose corpus | Establish real-world precision |
| P0 | Remaining false positives | Every new one gets a regression first |
| P1 | Diagnostic contract, confidence, explainability, correction confidence | One contract before the rule expansion |
| P1 | Agreement subsystem | Highest-frequency missing errors |
| P1 | Case-government subsystem | Reuses the existing role and clause data |
| P1 | Sentence-structure subsystem | Conservative structural diagnostics |
| P1 | Morphology gap: adjective degree | Only if a correction rule needs it |
| P2 | Extra comma, duplicate word, style vocabulary | Lower-severity coverage |
| P2 | UI ignore/dismiss and combined command | Usability and rule-level suppression |
| P2 | Cold-page performance | Separate engineering target |
| P3 | Long-distance syntax roadmap | Keep visible; do not rush |

## P0 — protect the foundation

These come before any new rule family. They are where the current checker is
most likely to lose trust.

### P0-A — Morphological ambiguity and role-selection regressions

The recent fixes were all interpretation bugs, not missing rules. Make that
class of bug a permanent test category.

- [x] **Add an ambiguity fixture** — landed as
  `tests/fixtures/syntax_ambiguity.json` + `tests/syntax_ambiguity.rs`.
  Each case pins the intended interpretation and at least one competing
  interpretation that must not win.
- [x] **Require the pair for every new role rule.** A rule that says "this is
  a subject" must also say which nearby nominal is *not* the subject. Add the
  pair to the fixture in the same commit.
- [x] **Cover the readings that actually collide:**
  - noun vs verb (`tuli`, `kirjaan`, `sen`);
  - pronoun vs adverb (`sen`, `ne`);
  - preposition/postposition vs noun (`ilman`, `ennen`);
  - nominative vs genitive/accusative on the same surface (`sinut`);
  - determiner vs relative pronoun (`joka`);
  - subject vs adverbial (`joka päivä`).
- [x] **Gate the suite in the same way as the exact-output fixture.** A role
  change that silently flips a competing reading fails the suite.

Suggested cases:

| Construction | Intended reading | Competing reading to reject |
|---|---|---|
| `Joka päivä juon kahvia.` | frequency adverbial | `päivä` as subject |
| `Joka mies tietää sen.` | `mies` as subject | `joka` as determiner phrase |
| `Joka päivä on kaunis.` | `päivä` as subject | frequency adverbial |
| `Ilman rahaa en tule.` | preposition complement | `Ilman` as subject |
| `Ilman sinua minä en tule.` | `minä` as subject | `sinua` as attribute |
| `Tarvitsen sen.` | `sen` as object | `sen` as adverb |
| `Hän pitää puheen.` | total object | partitive-only rection error |
| `Autan sinut sisään.` | `sinut` as accusative object | `sinut` as nominative subject |
| `Pidän sinusta.` | elative rection | total-object silence |
| `Merkitse joka esine kirjaan.` | one clause, determiner | relative clause split |
| `Mies, joka tuli, söi.` | relative clause | determiner |

Area: `tests/fixtures/`, `tests/syntax_check.rs`, `src/syntax/roles.rs`,
`src/syntax/predicates.rs`, `src/syntax/segments.rs`.

### P0-B — Adjudicated Finnish prose corpus and release metrics

The clean gates are constructed sentences. `tests/fixtures/perf/page-2000.txt`
reports grammar diagnostics, but they are counted, not judged. Add a small
public-domain prose sample with reviewed findings and provenance.

- [x] **Choose a small sample with provenance.** Seeded with a reviewed
  Project Gutenberg excerpt via `tests/fixtures/perf/page-2000.txt` (ebook
  9082, public domain). The first slice is 381 adjudicated correct words plus
  one true-positive sentence. Grow it before treating the rate as stable.
- [x] **Adjudicate every diagnostic** as true positive, false positive, or
  deliberate/acceptable. The seed is
  `tests/fixtures/prose/adjudicated.json`: 15 clean rows, 5 known
  false-positive rows, and 1 true-positive row.
- [x] **Separate rows by severity/status:** `clean`, `errors`, and
  `known_false_positives`. A fix to a known row makes the suite fail so it can
  be moved to `clean` or `errors`.
- [x] **Add the ambiguity corpus** from P0-A to the same run.
- [x] **Keep the constructed exact-output fixtures** as the per-rule gate.
- [ ] **Grow the prose sample to at least 1,000 correct words** and keep the
  known-false-positive rate per 1,000 words as the baseline.

Track these metrics:

| Metric | Purpose |
|---|---|
| `Error` false positives per 1,000 tokens | Release blocker |
| false positives per 1,000 tokens by severity | Overall precision |
| precision and recall per `DiagnosticKind` | Which subsystem is noisy |
| percentage of sentences with ≥1 finding | How intrusive the checker is |
| correction acceptance / correction validity | Detection vs correction quality |

Release gate:

- **zero `Error` on adjudicated correct prose**;
- no regression in the warning/suggestion per-1,000-token baseline;
- every new rule lands with constructed positive/negative cases and, where it
  is a high-frequency construction, an adjudicated prose row.

Do this **before** implementing most of P1, so rule work has a real precision
ceiling.

### P0-C — Remaining false positives and reading bugs

Every new false positive found in prose goes here first and gets a regression
before the next rule lands.

- [x] **`joka`/`jokainen` as a determiner before a time noun** — landed.
  `Joka päivä juon kahvia.`, `Joka aamu juon kahvia.`,
  `Joka vuosi matkustan Lappiin.`, `Jokainen päivä menen kouluun.` used to emit
  `subject_predicate_agreement` **Error/High**. The role pass treats a
  determiner plus a closed set of temporal nouns as a frequency adverbial when
  the clause's finite reading is 1st/2nd person. `Joka mies tietää sen.` and
  `Joka päivä on kaunis.` keep their subject readings.
  Area: `src/syntax/roles.rs`, `src/syntax/segments.rs`,
  `src/syntax/predicates.rs`.

- [ ] **Verify `koko` + temporal noun**
  `Koko päivä juoksin.` currently reads `päivä` as the subject and offers
  `juoksi`. Standard Finnish more often uses `koko päivän`, so do not extend
  the frequency-determiner pass until the construction is checked against
  Kielitoimisto. If it is accepted, add it to the determiners and the ambiguity
  fixture.

- [x] **Prepositions and their complements** — landed.
  `Ilman rahaa en tule.`, `Ilman sinua en tule.`, `Ilman sinua minä en tule.`
  and `Ennen minua hän tuli.` are clean. `mark_adpositions` handles
  `Preposition` (including the lexical `adverbi, prepositio` row for `ennen`)
  and marks the following nominal as its complement. Nominal case selection
  requires one reading to be both nominal and in the case, so a preposition's
  nominative reading cannot make it a subject.
  Area: `src/syntax/roles.rs`, `src/syntax/predicates.rs`.

- [x] **Rection must not treat a partial valency table as exhaustive** —
  stopped with an aspectual/total-object guard.
  `Hän pitää puheen.`, `Hän pitää kokouksen.`, `Hän pitää kirjan.`,
  `Katson elokuvan.` and `Autan sinut sisään.` are clean; `Pidän sinusta.`,
  `Katson elokuvaa.` and `Luotan sinuun.` stay clean.
  `ASPECTUAL_OBJECT_VERBS` in `src/syntax_check/rection.rs` is a stopgap. The
  proper follow-up is a `frame kind` column in `data/verb_rection.csv`
  distinguishing lexical government from aspectual partitive/total
  alternation. Do not add an “anything” frame; that silences the verb.

- [x] **`sen`/`ne` pronoun objects read as adverbs** — landed.
  `Tarvitsen sen.` and `Käytän sen.` used to trigger `missing_argument`.
  `classify` now lets a pronoun genitive/accusative object reading win over the
  adverb homonym; object selection takes the genitive/accusative case.
  Area: `src/syntax/roles.rs` (`classify`), `arguments.rs`.

- [x] **`joka` + noun must not split a relative clause** — landed.
  `Merkitse joka esine kirjaan.` is one clause: `esine` is the object and
  `kirjaan` is not read as a predicate. `joka` is a determiner when followed by
  a nominative noun and not preceded by a comma.
  Area: `src/syntax/segments.rs`, `src/syntax/predicates.rs`.

Prose-corpus findings now pinned as `known_false_positives` in
`tests/fixtures/prose/adjudicated.json`:

- [ ] **Apposition and subject phrase:** `Me suuremmat istumme ...` produces
  `attribute_agreement`, `subject_predicate_agreement` and `object_case`.
  The pronoun is the subject head; `suuremmat` is an apposition.
- [ ] **Experiencer and total-object verbs:** `ettei häntä pelottaisi`,
  `he eivät uupuneet sitä kuuntelemasta`, and `Jumala hellytti pyövelien
  sydämet` are read as partitive-object errors. `pelottaa` is an experiencer
  verb, `häntä` is the pronoun (not `häntä` “tail”), and `sydämet` is a total
  object in an affirmative clause.
- [ ] **Coordination scope:** `Kenoveeva kehräsi ja lauloi, Sikri soitti ...`
  reports plural agreement on three singular predicates. Each coordinated
  clause keeps its own subject.
- [ ] **Modifier vs head:** `Nämä hurskaan miehen sanat` reports
  `attribute_agreement` on `Nämä`. The genitive `hurskaan miehen` modifies
  `sanat`; `Nämä` agrees with the head, not with the genitive.
- [ ] **Postposition and adverbial:** `rakkaan puolisonsa jälkeen` reports an
  `object_case` error on the genitive governed by `jälkeen`;
  `koko päivänä` is read as an essive attribute and corrected to `kokona`.
- [ ] **Punctuation boundaries:** sentence-initial `Ja kun ...` asks for a
  comma after `Ja`; a final relative clause (`..., joka ei ...`) is reported
  as an unbalanced comma pair; `--kun ...` and the `jos` conditional ask for a
  second comma. All are pinned false positives.

## P1 — shared diagnostic contract, confidence and explainability

This is cross-cutting and should land before the rule expansion. It is the
place to stop future false positives by construction rather than by patching
one word at a time.

### P1-A — One diagnostic contract for all three layers

The exact Rust shape can stay close to today's `SyntaxDiagnostic`, but every
layer should follow the same contract:

```rust
Diagnostic {
    rule_id: RuleId,          // stable kind/name, usable for ignore/dismiss
    severity: Severity,       // Error / Warning / Suggestion
    confidence: Confidence,   // Certain / Likely / Ambiguous
    span: Span,               // the actual problematic construction
    message: MessageKey,      // stable, localisable
    explanation: Option<Explanation>,
    suggestions: Vec<Suggestion>, // optional; empty is valid
}
```

Rules:

- stable rule identifiers — ignore by **rule**, not by text span;
- span points at the responsible word or construction;
- severity and confidence are independent;
- suggestions are optional;
- an explanation is available without a correction;
- `style_check` never emits `Error` or `Warning`.

### P1-B — Structural confidence and suppression

Introduce a small number of explicit structural states rather than numerical
probabilities:

- **Certain** — one clear reading; an `Error` is allowed.
- **Likely** — one preferred reading with a known competitor; use `Warning` or
  `Suggestion`, or suppress for high-risk rules.
- **Ambiguous** — competing readings; suppress the diagnostic.

An agreement error should require an unambiguous subject–predicate relation.
An unknown word, an ambiguous role, a numeral phrase, a coordinated subject
whose person cannot be resolved, or a missing comma that changes the parse
should suppress rather than guess.

- [ ] Add the confidence states and route every existing rule through them.
- [ ] Pin suppression cases in the ambiguity fixture.
- [ ] Do not emit `Error` from a rule whose preconditions are only `Likely`.

### P1-C — Detection confidence vs correction confidence

A diagnostic can be valid even when no safe rewrite exists.

- [ ] Make correction confidence explicit, or make corrections fully optional
  per diagnostic.
- [ ] Keep curated replacements for style and safe case/form changes.
- [ ] Do not invent a word, a subject or an argument just to fill a suggestion.
- [ ] For numeral/adposition/object rules, detect confidently and propose only
  forms licensed by the inflection engine and the curated frame.

Example: `Kaksi lapsi tuli.` can be detected confidently, but a suggestion is
only safe when the engine can name the partitive singular without ambiguity.

### P1-D — Explainability

Every finding should be able to say:

- which word or phrase is considered wrong;
- which grammatical relation triggered it (subject–predicate, attribute–head,
  verb frame, numeral head–complement, adposition–complement, punctuation);
- what the expected category is (case, number, person, form);
- why the proposed correction is licensed by the analysis.

`expected` and `message` are useful for debugging, but the UI needs a stable
explanation separate from the correction. Add explanation keys before the
first new rule family.

## P1 — grammar rule subsystems

Once P0 and the contract are in place, add rules one subsystem at a time. A
subsystem lands with its ambiguity cases, constructed positives/negatives, and
prose-corpus review.

### Agreement

- [x] **Predicative adjective agreement** — landed for number.
  `Talot ovat punainen.` and `Talo on punaisia.` now emit
  `predicative_agreement`; `Talot ovat punaisia.`, `Talot ovat punaiset.` and
  `Talo on punainen.` are clean. The rule is number-only and adjective-only:
  `Talot ovat punaisia` (partitive plural) and `Talot ovat punaiset`
  (nominative plural) are both legitimate, so case is not a correctness claim.
  Area: `src/syntax_check/agreement.rs`; `Clause.complement`.

- [ ] **Predicative noun/participle complements**
  `Lapset ovat väsynyt.` and `He ovat opettaja.` are still silent:
  `väsynyt` is read as a participle, and the noun complement's
  partitive/nominative choice is semantic. Add only after the adjective rule
  has survived the prose corpus; do not fold it into the number-only rule.
  Area: `src/syntax_check/agreement.rs`.

- [x] **Connegative main verb after the negative auxiliary** — landed.
  `En menen.`, `Hän ei menee.` and the negative past (`En menin.` →
  `mennyt`) now emit `connegative`. `En mene.`, `Hän ei mene.`, `En ole.`,
  `En mennyt.`, `En tullut.`, `Älä mene.`, `Älkää menkö.` and `En voi mennä.`
  are clean. The rule checks only the verb adjacent to the auxiliary, accepts
  a negative reading (present connegative or past participle), and treats the
  3sg conditional as the conditional connegative. Imperative findings are
  label-only; the potential is left quiet.
  Area: `src/syntax_check/connegative.rs`, `src/syntax_check/correction.rs`,
  `src/syntax/predicates.rs`.

- [ ] **Compound-tense participle agreement**
  `Me olemme lukenut.`, `Minä olen lukeneet.` and `Hän on lukeneet.` are
  silent. The auxiliary agrees; the past participle's number is not compared
  with the subject.
  Area: `src/syntax_check/agreement.rs`.

- [ ] **Possessive suffix person/number**
  `Minun kirjasi on uusi.`, `Hänen kirjani on uusi.` and `Minun kirja on uusi.`
  are silent. Morphology already records `possessive`.
  Emit `Warning` or `Suggestion`, never `Error`: colloquial Finnish drops the
  suffix and the written norm has exceptions.
  Area: `src/syntax_check/agreement.rs`.

### Case government

- [x] **Numeral government** — landed.
  `Kaksi lapsi tuli.`, `Kaksi lapsia tuli.` and `Ostin kaksi kirja.` now emit
  `case_selection` with the partitive singular. `Kaksi lasta tuli.`,
  `Kaksi miestä tuli.`, `Yksi lapsi tuli.`, `Kahdet kengät.`,
  `Kaksi pientä lasta.`, `Kaksi kolmesta.`, `Sata autoa.`,
  `Kaksikymmentä lasta.` and `Ostin kaksi kirjaa.` are clean. The rule is
  narrow: only a nominative complement or partitive plural is flagged; `yksi`,
  plural numerals, `toinen` and other non-cardinals are skipped. The adjective
  guard also removes the old `Kaksi pientä lasta` attribute false positive.
  Area: `src/syntax_check/numeral.rs`, `src/syntax_check/agreement.rs`.

- [ ] **Adposition case government**
  `Pöytä alla on kissa.` (should be `Pöydän alla`) is silent. Add a small
  `data/adposition_cases.csv` and use the reserved `CaseSelection` kind.
  Do not infer the case from the surface; share only the case-selection
  infrastructure with numeral government, not the linguistic assumptions.
  Area: `src/syntax_check/`, `src/expand/`, `data/`.

- [ ] **Existential subject case**
  `Pöydällä on kirjan.` is silent. An existential's subject is nominative or
  partitive, never genitive. The clause already has `existential`.
  Area: `src/syntax_check/`.

- [ ] **Bare nominative object**
  `Minä näen kirja.` is silent; the syntax layer already marks `kirja` as an
  object with a nominative case. Emit an `ObjectCase` error only when the
  object is a bare nominative with no possessive and the clause is not
  imperative, passive or necessive. The guards already exist.
  Area: `src/syntax_check/object_case.rs`.

### Sentence structure

- [ ] **Missing finite predicate**
  `Koska minä väsynyt.` (missing `olen`) is silent. Start with a conservative
  “subordinator or coordination with no finite predicate” rule.
  `Hän tuli ja.` is the coordination variant. Do not flag headings, captions
  or intentional fragments.
  Area: `src/syntax_check/punctuation/clause.rs`, `src/syntax_check/`.

- [ ] **Incomplete coordination**
  `Hän tuli ja.` may share the missing-predicate rule or get its own
  `ClauseStructure` finding. Keep it `Warning` at most.

- [ ] **Extra comma**
  `Minä, menen kotiin.` is silent; `CommaExtra` is declared but never emitted.
  Avoid appositives, parentheticals and deliberate pauses before firing.
  This is P2 punctuation work, not a prerequisite for the agreement rules.
  Area: `src/syntax_check/punctuation/comma.rs`.

### Morphology

- [ ] **Adjective degree**
  `Punaisempi autot ovat hienoja.` is silent because `Tags` has no degree;
  `attribute_agreement` deliberately skips `expand::degree_base` matches.
  Add a degree dimension only when a correction rule needs it.
  Recorded decision: `PLAN_SYNTAX_CHECKER.md` §14 item 4.

## Recommended implementation order

The rule order is deliberately interleaved across subsystems so each step
reuses the previous one and the prose corpus grows with it.

| Order | Work | Reason |
|---|---|---|
| 0 | Ambiguity regression suite | Protect the interpretation fixes |
| 0 | Adjudicated prose corpus | Establish real-world precision |
| 1 | Diagnostic contract + structural confidence | Stop future false positives by construction |
| 2 | Predicative complement agreement | High-frequency, reuses `Clause.complement` |
| 3 | Connegative main verb | Finnish-specific and tightly scoped |
| 4 | Numeral government | High-frequency, constrained |
| 5 | Adposition case government | Natural extension of the preposition work |
| 6 | Existential subject case | Reuses the existing clause flags |
| 7 | Compound-tense participle agreement | Extends the agreement subsystem |
| 8 | Bare nominative object | Important; needs the imperative/passive/necessive guards |
| 9 | Clause structure | Conservative missing-predicate/incomplete-coordination rule |
| 10 | Possessive suffix | `Warning`/`Suggestion` only |
| 11 | Extra comma | P2 punctuation coverage |
| 12 | Duplicate words | Low-risk style suggestion |
| 13 | Style vocabulary expansion | Incremental table growth |
| 14 | UI ignore/dismiss + combined command | Rule-level suppression and usability |
| 15 | Cold-page performance | Separate engineering objective |

## P2 — punctuation, style and surfaces

### Conservative duplicate-word detection

Adjacent repetition is not always accidental:

- `Se oli hyvin, hyvin vaikeaa.`
- `Ei, ei se ollut niin.`
- `Hän sanoi, että että tämä oli väärin.` may be a transcription artifact, but
  context matters.

Implement `style_duplicate_word` as a **Suggestion** only. Start with adjacent
identical surfaces and no punctuation between them. Skip when a comma or dash
separates the pair, and keep an explicit exception list for rhetorical
reduplication. Do not conflate repetition with bad style.

Area: `src/style_check/repetition.rs`.

### Other P2 items

- [ ] **Grow the style tables carefully**
  `data/style_words.csv` has 6 entries and `data/style_phrases.csv` has 8.
  Common fillers such as `periaatteessa`, `tosiasiassa`, `joka tapauksessa`,
  `niin kuin` and `ikään kuin` are not matched. Add reviewed rows only; keep
  the exact-match contract and measure on the prose corpus.

- [ ] **Dead reserved kinds**
  `CaseSelection`, `ClauseStructure` and `CommaExtra` are never emitted;
  `WordOrder` is deliberately reserved. The first three correspond to the P1
  rules above. Do not remove them before those rules land.

- [ ] **Ignore/dismiss and a combined command**
  Findings can be corrected but not ignored. Ignore by stable rule ID, not by
  span. The extensions expose **Tarkista kielioppi** and **Tarkista tyyli** as
  separate commands even though the wasm export returns both in one pass.
  Add a combined action if the UX is in scope.

- [ ] **Cold page target**
  `PLAN_GRAMMAR_STYLE_PERF.md` still records the ≤500 ms cold-page target as
  open: ~1.4 s wasm with the 4-worker pool, ~3 s single-thread. Performance is
  separate from rule correctness.

## P3 — future syntax roadmap

Keep these visible; do not implement them before the local agreement and case
work is trustworthy. The current analysis is deliberately local and advisory.

- relative clauses with complex, coordinated or distant antecedents;
- coordination with shared subjects and long-distance agreement;
- embedded clauses and nested subordination;
- infinitive and participial constructions as clause participants;
- long-distance agreement relationships;
- a non-contiguous host span for interrupted relatives;
- `ClauseKind` promotion, `Clause.sentence`, and antecedent-aware list roles —
  only when a second consumer needs them.

Area: `src/syntax/`, `src/syntax_list.rs`, `PLAN_SYNTAX.md` §“Remaining
analysis refinements”.

## Follow-up plan — neural reading disambiguation

- [ ] **Audit the neural reranker integration.** The architecture audit is
  [`docs/neural_disambiguation.md`](docs/neural_disambiguation.md). It proposes a
  per-analysis `Disambiguation` sidecar between `analyze_morphology` and
  `analyze_syntax`, preserving every existing `WordInfo.readings` candidate.
  The smallest viable milestone is an M0 candidate-preference MLP behind a
  feature flag, evaluated on UD Finnish-FTB and a frozen hard-ambiguity set;
  the BiLSTM/compact-Transformer choice comes only after that baseline.
  No model is implemented yet.

## Deliberate boundaries

- Word order is not checked; Finnish order is information structure.
- The style layer never emits `Error` or `Warning`.
- Grammar and style are on-demand and never run in the keystroke path.
- No numerical probabilities; structural confidence states only.
- No automatic rewrite without a licensed correction.
- Relative clauses are linked additively; a non-contiguous host span is not
  modelled.
- The checker consumes `syntax`; it must not build a second parser.

## Verification

Current gates:

```bash
cargo test --release -p torille --test syntax_check --test syntax_eval --test syntax_ambiguity --test syntax_prose --test style_check --test style_eval
cargo test --release -p torille
cargo clippy -p torille --all-targets -- -D warnings
./target/release/check_syntax "Joka päivä juon kahvia."
./target/release/check_syntax "Ilman rahaa en tule."
./target/release/check_style "Tämä on on hyvä."
```

Run the prose suite with `-- --nocapture` to print the adjudicated word count
and the known false-positive rate. The ambiguity gate prints each
intended/competing interpretation pair that fails.

## Work log

- 2026-11-15: audit written; landed the first P0 slice:
  determiner `joka`/`jokainen` + temporal noun, preposition complements,
  reading-consistent nominal case selection, the rection total-object guard,
  and the `sen`/`ne` pronoun-object override. Added clean fixture rows and
  unit tests for the role changes.
  `cargo test --release -p torille` and `cargo clippy -p torille --all-targets
  -- -D warnings` are green.
- 2026-11-15: revised this roadmap after review. Added P0 ambiguity and
  adjudicated-prose gates, a shared diagnostic/confidence contract,
  detection-vs-correction confidence, explainability, subsystem grouping,
  conservative duplicate-word policy, and a future long-distance syntax list.
- 2026-11-15: landed P0-A. Added `tests/fixtures/syntax_ambiguity.json` and
  `tests/syntax_ambiguity.rs`; the suite pins the intended role, the rejected
  competing role, and the clean diagnostic list for thirteen ambiguous
  constructions.
- 2026-11-15: landed the P0-B seed. Added
  `tests/fixtures/prose/adjudicated.json` and `tests/syntax_prose.rs`:
  15 clean rows, 5 known-false-positive rows and 1 true-positive row
  (381 adjudicated correct words, 17 known false-positive errors, 44.6 per
  1,000 words at this seed size). The suite fails if a clean row stops being
  quiet or a known row changes, so a fix must move the row rather than leave a
  stale baseline.
  Moved adjudicated prose before the P1 rule expansion.
- 2026-11-15: landed the first P1 rule, predicative adjective agreement.
  `Talot ovat punainen.` and `Talo on punaisia.` emit
  `predicative_agreement` with adjective number corrections; the clean corpus
  adds `Talot ovat punaisia.`, `Talot ovat punaiset.`, `Talo on punainen.`,
  `Lapset ovat väsyneitä.`, `Hän on väsynyt.` and `He ovat opettajia.`.
  Added exact-output and ambiguity cases, updated the UI labels and the rule
  catalogue. The prose corpus recorded no new false positives.
- 2026-11-15: landed the connegative rule. `En menen.` and `Hän ei menee.`
  now emit `connegative` with `mene`; `En menin.` offers `mennyt`. Clean rows
  cover `En mene.`, `Hän ei mene.`, `En ole.`, `En mennyt.`, `En tullut.`,
  `Älä mene.`, `Älkää menkö.` and `En voi mennä.` The rule required two prose
  corrections during development: it now checks only the verb adjacent to the
  auxiliary (so `Ei, sillä ...` is not an auxiliary) and accepts the 3sg
  conditional as the conditional connegative (`ettei häntä pelottaisi`).
- 2026-11-15: landed numeral government. `Kaksi lapsi tuli.`,
  `Kaksi lapsia tuli.` and `Ostin kaksi kirja.` now emit `case_selection`
  with the partitive singular. Clean rows cover `Kaksi lasta tuli.`,
  `Yksi lapsi tuli.`, `Kahdet kengät.`, `Kaksi pientä lasta.`,
  `Kaksi kolmesta.`, `Sata autoa.` and `Kaksikymmentä lasta.`. The same work
  added a numeral-phrase guard to `attribute_agreement`, removing the
  `Kaksi pientä lasta` false positive.
