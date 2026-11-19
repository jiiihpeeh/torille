# PLAN_GRAMMAR_SYNTAX_REFERENCE — hardening the feature reference

Status: **landed.** Phases 0–7 are implemented in the reference, the code and
the tests; phase 8 remains gated future work. This document responds to the
review of [`../docs/grammar_syntax.md`](../docs/grammar_syntax.md) and records
the decisions that were taken while hardening it.

The review’s architectural verdict is accepted: **no rewrite**. The work was
specification hardening, clearer Finnish-specific modelling, and documentation
traceable to code and tests.

## 1. Disposition

| review point | response | phase |
|---|---|---|
| architecture separation is good | keep it; document the contract between layers | 0 |
| retain ambiguity / conservative roles | keep as a stated invariant | 0 |
| `Plain`/`Repaired`/`Guessed` is valuable | document the exact propagation and correction rule | 5 |
| AST is additive and validated | keep; document flat model vs tree authority | 0 |
| correction policy is defensible | separate inflection, curated and punctuation licenses | 0, 5 |
| 2.1 syncretism | add case-evidence model; never convert morphology silently | 1 |
| 2.2 role model is coarse | keep roles; add construction/function features | 2 |
| 2.3 clause segmentation | document precedence and uncertainty | 3 |
| 2.4 agreement rules | add explicit construction-by-construction matrix | 4 |
| 2.5 confidence semantics | formalise the four confidence layers and propagation | 5 |
| 3 inconsistencies | resolve each in the contract appendix | 0, 1, 2, 5 |
| 4A coverage matrix | add and verify it | 6 |
| 4B diagnostic lifecycle | add the lifecycle diagram | 5 |
| 4C public API guarantees | add a contract section | 6 |
| 4D performance | add measured budgets and cache semantics | 7 |
| 5 presentation | restructure, standardise terms, validate tables | 0 |
| 6 priorities | use P0–P3 below | all |
| 7 verdict | no rewrite; harden and validate | — |

### 1.1 Landing status

| phase | status |
|---|---|
| 0 editorial/terminology/table checks | done: reference restructured, TOC/terminology/pipeline authority added, `tools/check_docs.py` green |
| 1 case evidence | done: three-level model, examples, `tests/syntax_contract.rs` |
| 2 role vs construction | done: `SubjectKind`, `Construction::Necessive`, construction table; `Experiencer` gated |
| 3 segmentation precedence | done: precedence table and worked examples in the reference |
| 4 agreement matrix | done: construction-by-construction matrix and existing fixtures |
| 5 confidence/mapping/lifecycle | done: four-layer table, propagation policy, full `DiagnosticKind → RuleId` table, contract test, audited rule-specific certainty |
| 6 coverage/API | done: coverage matrix, API guarantees, `tests/syntax_contract.rs` |
| 7 evaluation/performance | done: methodology caveat and measured table |
| 8 future linguistic work | gated; not started |

The hardening added `tools/check_docs.py` and `tests/syntax_contract.rs`, and
changed `ClauseFeatures` with `subject_kind` plus `Construction::Necessive`.
The public flat role inventory was not expanded.

## 2. Invariants to preserve

These are the design decisions the hardening must not erode:

1. **Morphology, syntax, AST and diagnostics are separate responsibilities.**
2. **The flat syntax analysis is authoritative.** The AST is a derived,
   additive view; it never re-resolves a role or searches the lexicon.
3. **An unknown role is preferable to a guessed one.**
4. **Ambiguity is retained.** Disagreement is absence of an agreed feature,
   not a reason to pick one reading.
5. **Only `Plain` forms may be inflection corrections.**
6. **An `Error` needs a clear cue.** `Confidence::Ambiguous` is suppressed;
   `Error` with `Likely` is downgraded to `Warning`.
7. **Word order is information structure, not a correctness rule.** No
   general `WordOrder` diagnostic.

## 3. Phase 0 — editorial contract and terminology

### Goal

Make the reference readable and unambiguous before adding technical detail.

### Work

- Restructure `docs/grammar_syntax.md`:
  1. Scope and terminology
  2. Pipeline and authority
  3. **Morphology and inflection** (rename Part I)
  4. Syntax analysis
  5. Surface tree (AST)
  6. **Grammar checker** (keep Part III, but call it “grammar checker”)
  7. Contracts and guarantees
  8. Evaluation and performance
  9. Deliberate limits
  10. Appendices (diagnostic mapping, coverage matrix, file map)
- Rename “grammar engine” to **morphology engine** wherever the section means
  inflection/recognition. Keep “grammar checker” for `syntax_check`.
- Qualify confidence by layer: `FormConfidence`, `RoleConfidence`,
  diagnostic `Confidence`, correction `Confidence`.
- Add a table of contents and stable heading anchors.
- Add an architecture diagram showing morphology → syntax → AST → checker and
  marking the flat analysis as authoritative and the tree as derived.
- Make every table a real Markdown table with a header and matching column
  count. Add a `tools/check_docs.py` gate that checks:
  - every Markdown table row has the same number of cells as its header;
  - every relative link resolves;
  - no duplicate heading anchors;
  - no orphan code fence.
- Decide the canonical home of the rule catalogue:
  - `docs/syntax_checker.md` remains the canonical rule/severity/limitation
    page;
  - `docs/grammar_syntax.md` carries the feature map and the
    `DiagnosticKind → RuleId` appendix, linking to the checker page.
  - Do not maintain two prose descriptions of the same rule.

### Documentation map

| question | authoritative document |
|---|---|
| flat syntax analysis model | `misc_md/PLAN_SYNTAX.md` |
| AST design and phases | `misc_md/PLAN_SYNTAX_AST.md` |
| remaining AST phases | `misc_md/PLAN_SYNTAX_AST_REST.md` |
| checker rules and limitations | `docs/syntax_checker.md` |
| feature reference / contracts | `docs/grammar_syntax.md` |
| word order | `docs/word_order.md` |
| grammar/syntax fix history | `misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md` |

The AST plan is authoritative for the tree. `PLAN_SYNTAX.md` is authoritative
for the flat role layer, not for the tree. Fix the introduction’s references to
say exactly that.

### Gate

- `python3 tools/check_docs.py` green;
- link check green;
- no semantic claim changes in this phase; only editorial/structural moves.

## 4. Phase 1 — case evidence and syncretism

### Goal

Never let a surface form’s ambiguity silently become a syntactic claim.

### Model

Document three distinct levels:

| level | field | meaning |
|---|---|---|
| morphological reading | `Tags.case` on each reading | a case the form can express |
| agreed morphology | `TokenSyntax.case` | the case all selected readings agree on, or `None` |
| syntactic function | `Clause.object` + `Clause.features.object_case` | the function the clause resolved, using the agreed reading when available |

The rule is: **morphology proposes; syntax selects; neither rewrites the
other.** A nominative/accusative syncretic surface can be an object, but the
token’s `case` stays what the readings say. `ObjectCase::Accusative` is produced
only from a real `Case::Accusative` reading, not from word order.

### Work

- Add a “Case evidence” table to the reference:
  - `näen pojan` — total singular object; genitive/accusative reading;
  - `näen poikaa` — partitive object;
  - `poika tuli` — nominative subject;
  - `pojan tulo` — genitive modifier;
  - `Pöydällä on kirja` — nominative existential subject;
  - `Pöydällä on kirjaa` — partitive existential subject.
- Explain `Case::Accusative` as a tag whose surface is often shared with
  nominative or genitive; it is not a separate written case.
- Add tests pinning `TokenSyntax.case`, `SyntacticRole`, and
  `ClauseFeatures.object_case` for those examples.
- Audit the `Case::Accusative` comments in `src/tags.rs` and
  `src/syntax/features.rs`; fix any wording that implies the surface is
  unambiguous.

### Gate

No rule behaviour change; new tests are documentation fixtures for the
contract. `cargo test --release --workspace`.

## 5. Phase 2 — roles versus construction features

### Goal

Keep `SyntacticRole` small and stable while modelling Finnish constructions
explicitly.

### Decision

`SyntacticRole` is a **syntactic function**, not a construction. Do not add
`ExistentialSubject`, `NecessiveSubject`, `ExperiencerObject` and similar
variants. Add construction/function features instead:

```rust
enum SubjectKind {
    Nominative,
    Partitive,
    GenitiveNecessive,
    Existential,
    Omitted,
}
```

`ClauseFeatures` gains `subject_kind`; `Construction` gains `Necessive` and
`Experiencer` only if the existing cues can support them without guessing.
Otherwise the first release documents `Necessive` as an ordinary construction
with a genitive subject and leaves `Experiencer` out.

### Construction table

| construction | subject/possessor | agreement behaviour |
|---|---|---|
| ordinary | nominative subject | subject–predicate person/number |
| existential | postverbal nominative/partitive | singular predicate; no agreement error |
| passive | no syntactic subject | no subject agreement; participle behaviour separate |
| copular | nominative subject | complement agreement in number only |
| possessive | adessive possessor + `on` | the possessed nominal is the subject |
| necessive | genitive subject + `täytyy`/`pitää`/`on` | 3sg predicate; no nominative subject agreement |
| experiencer | case-marked experiencer + predicate | rule-specific; do not reuse subject agreement |

### Work

- Add `SubjectKind`/`Construction` definitions to the reference, with the
  “role is function” rule explicit.
- Add a table mapping each construction to the rules that read it.
- Add tests for existential, passive, necessive, possessive and predicative
  clauses; pin the current `Ordinary`/`Existential`/`Passive`/`Copular`/
  `Possessive` values.
- Do not change `SyntacticRole` in this phase.

### Gate

Existing word-order fixtures and `word_order_allow` unchanged. If a feature is
not backed by a cue, document it as absent rather than adding it.

## 6. Phase 3 — clause segmentation precedence

### Goal

Document why a sentence is split where it is, and what happens when cues
compete.

### Precedence table

The actual order in `src/syntax/segments.rs` is the specification. Document it
as:

| priority | cue | effect |
|---|---|---|
| 1 | top-level semicolon | clause split |
| 2 | `sillä` clause opener | clause split |
| 3 | relative pronoun / relative opener | clause split |
| 4 | comparison opener with a following predicate | clause split |
| 5 | subordinator (`että`/`vaikka` guarded) | clause split |
| 6 | comma + capitalised word before a finite predicate | split at the capitalised word |
| 7 | comma + personal pronoun + finite predicate | split at the pronoun |
| 8 | coordinating conjunction before a finite predicate | split at the conjunction |
| 9 | infinitive after a finite predicate | split at the infinitive |
| 10 | comma + finite predicate after a finite predicate | split at the predicate |
| 11 | no reliable cue | **no split** |

Rules that override: `että` after `sekä` is not a clause opener; `vaikka` is
only a subordinator with a following predicate; a bracket-only sentence
boundary is merged for clauses but not for the raw case spans.

### Worked examples

Add a table with the resulting clause arrays:

- `Tiedän, että hän tulee.` — main + subordinate;
- `Mies, joka tuli, söi.` — host segment, relative, continuation;
- `Kun tulin kotiin, söin.` — subordinate + main;
- `Hän sanoi, ettei tule.` — main + negative subordinate;
- `Sami ja Liisa söivät.` — one clause (coordinated subject);
- `Sami söi ja joi.` — two coordinated clauses;
- `(Tämä on uusi virke.) Toinen virke.` — two sentences;
- `omena (pieni, makea) ja banaani` — one clause, bracket boundary merged.

### Uncertainty

State explicitly: when two cues disagree, the later cue is not applied; when no
cue is reliable, the run stays one clause and the role resolver returns
`Unknown` rather than inventing a boundary. Clause tests in
`tests/fixtures/syntax_cases.json` and `syntax_ambiguity.json` are the oracle.

### Gate

Add/adjust no rule behaviour; document the existing precedence and add a table
test for the examples.

## 7. Phase 4 — agreement and construction matrix

### Goal

Make the agreement rules explicit and separable.

### Matrix

| rule | controller / bearer | construction | correction |
|---|---|---|---|
| subject–predicate | subject person/number; finite main verb or `olla` auxiliary | ordinary, copular | inflection |
| negative auxiliary | negative auxiliary carries person/number; main verb is connegative | negative | connegative selection; 2sg imperative only |
| compound tense | active past participle agrees in number with subject | ordinary/copular | inflection |
| predicate complement | adjective/participle number; noun predicate only clear number mismatch | copular | inflection/curated |
| attribute | adjective/ordinal/demonstrative agrees with its nominal head | any | inflection |
| possessive | possessive suffix matches adjacent genitive pronoun | any | none |
| passive | no subject agreement; passive participle is not a subject-agreement target | passive | none |
| existential | singular predicate; postverbal nominative/partitive subject | existential | none |
| necessive | genitive subject; 3sg predicate | necessive | none |
| experiencer | case-marked experiencer; no subject–predicate reuse | experiencer | rule-specific |

`Number::Collective` is not a plural. The subject-agreement bearer currently
returns no subject for a collective reading, so a collective subject never
drives a subject–predicate comparison. Document and test that; do not silently
treat collective as plural. Middle voice is a morphological reading; no syntax
rule currently branches on it, so document it as recognised but not
syntactically modelled.

### Work

- Add the matrix with one positive and one clean example per row.
- Add a paragraph distinguishing **agreement bearer**, **controller** and
  **connegative selection**; they are not interchangeable.
- Add fixtures for:
  - `Me olemme lukenut.` / `Minä olen lukeneet.`;
  - `En mene.` / `En menen.` / `Älä menet.`;
  - `Pöydällä on kirjat.` / `Pöydällä on kirja.`;
  - `Talo on rakennettu.`;
  - `Minun täytyy mennä.`;
  - `Minulla on kirja.`;
- Pin the existing `Clause.negative`, `passive`, `existential` flags in the
  same fixtures.

### Gate

Exact-output fixtures; no new false positives on the prose/page corpus.

## 8. Phase 5 — confidence, lifecycle and diagnostic mapping

### 8.1 Confidence layers

Document four layers, never one unqualified “confidence”:

| layer | values | owner |
|---|---|---|
| form | `Plain`, `Repaired`, `Guessed` | morphology generation |
| role | `Likely`, `Weak` | syntax |
| diagnostic | `Certain`, `Likely`, `Ambiguous` | checker |
| correction | `Certain`, `Likely`, `Ambiguous` | correction |

### 8.2 Propagation policy

| input evidence | diagnostic ceiling | correction |
|---|---|---|
| single relevant reading + strong role | `Certain` | `Plain` form only |
| single relevant reading + weak role | `Likely` | `Plain`, likely confidence |
| multiple competing readings | `Ambiguous` → suppressed | none |
| repaired form | warning-level only if the rule uses it | **never** an inflection suggestion |
| guessed form | suppressed for correction | never |
| curated table row | `Certain`/`Likely` as the table says | `Curated` |
| mechanical punctuation | `Certain` for a clear boundary | `Punctuation` |

A rule may have a stronger lexical cue than the role confidence. The attribute
number rule is the documented exception: an unambiguous attribute shape and an
unambiguous head number can be `Certain`/`Error` even when the modifier role is
`Weak`. Every new rule must state which evidence supplies its `Certain`.

This is the policy the review asks for. It must be checked against the code,
not merely written down.

**Licenses are separate from confidence.** The `Plain` restriction applies to
**inflection** corrections: the replacement must be a generated `Plain` form.
A curated correction (`CorrectionLicense::Curated`) comes from a reviewed table
(adposition/verb frame) and is not a generated form; a punctuation correction
is a mechanical edit. State this explicitly so “Plain only” is not read as a ban
on curated or punctuation corrections.

### 8.3 Audit work

- Add a table `DiagnosticKind → RuleId → severity → correction license` (see
  Appendix A) to the reference.
- Add a test that every emitted `message`/`RuleId` appears in the table. The
  message key is the rule id; a missing key fails the docs/test contract.
- Audit rules that set `Certain` directly:
  - `check_attributes` sets `Certain` for a number mismatch;
  - punctuation rules use `Certain` for clear boundaries;
  - agreement rules use `Confidence::from_role`.
  Audit a weak role: `rank::resolve` downgrades `Error`/`Likely`; the attribute
  rule uses unambiguous morphology as its stronger cue and is documented as a
  rule-specific exception.
- Audit correction confidence versus diagnostic confidence. A correction must
  not be more certain than the diagnostic that licensed it. Curated table
  corrections may be `Certain` only when the table row is exact.
- `WordInfo` carries readings, not per-reading `Form.confidence`. The chosen
  policy is **Option B**: diagnostics may use repaired readings while
  corrections never do. The reference states this explicitly. If an audit finds
  a repaired reading causing a false `Error`, add a `form_confidence` field to
  `WordInfo` and cap it (Option A).
- Add the diagnostic lifecycle diagram:

```text
text → tokenise → morphology → syntax → rule evaluation
     → ambiguity filter → severity assignment → corrections → rank
```

### Gate

- mapping test green;
- confidence audit tests green;
- prose/page fixtures unchanged unless a rule is deliberately capped;
- document the chosen `Repaired` policy.

## 9. Phase 6 — coverage matrix and public API contract

### 9.1 Coverage matrix

Add a verified matrix:

| feature | recognition | syntax integration | checking | correction |
|---|---|---|---|---|
| nominal inflection | yes | yes | case/rection/object | yes, confidence-dependent |
| verb inflection | yes | yes | agreement/connegative | yes, confidence-dependent |
| possessive suffixes | yes | partial | possessive agreement | no |
| clitics | yes | question detection | question marks | limited |
| compounds | yes | partial | limited | no |
| derivations | yes | partial | style/spell separate | no |
| numerals | yes | numeral phrases | numeral case | yes |
| proper names | yes | subject/object | limited | no |
| relative clauses | yes | host/AST | punctuation | limited |
| lists | yes | list arena/AST | punctuation | yes |
| parentheticals/quotes | yes | AST | punctuation | no |
| word order | features | constituent view | reserved | no |

Each row must name the code path and a test. “Partial” must say what is
missing, not just “partial”.

### 9.2 Public API guarantees

Add a contract section:

- **Spans** are half-open UTF-8 byte ranges `start..end`.
- **Token indices** are zero-based and refer to the full token array, including
  separators. `SyntaxAnalysis.tokens` has one entry per token; word leaves are
  a subset.
- **Clause/list indices** are zero-based; absent values are `Option`.
- **Clause token ranges** include separators between the first and last word;
  endpoint tokens are words.
- **Diagnostic order** is deterministic: severity, diagnostic confidence,
  `span.start`, `span.end`, kind; one diagnostic per span; corrections sorted by
  score then note.
- **Cache reuse** produces the same analysis/diagnostics; only `Depth`/cache
  counters may differ.
- **Stable:** `DiagnosticKind` names and `RuleId` strings; correction
  licenses; byte-span semantics.
- **0.x unstable:** AST node kinds/fields, `Clause` fields and JSON shapes;
  additive changes are documented per phase.

### Gate

Add tests for token/span invariants, deterministic ordering and cache parity.
Update the wasm docs with the same contract.

## 10. Phase 7 — evaluation methodology and performance

### Evaluation

Replace “precision/recall 1.00” with a definition:

- corpus: `syntax_eval` 154 clean / 60 cases; `syntax_diagnostics` 75 exact
  cases / 272 clean; prose 79 clean / 1,233 words;
- adjudication: who reviewed, what counts as a true/false positive;
- what 1.00 means: **fixture-level** exactness on this curated set, not general
  Finnish accuracy;
- known gap: a larger independently adjudicated corpus is future work;
- report results by kind and by construction, not only aggregate.

### Performance

Add a measured table:

| workload | measurement |
|---|---|
| warm grammar rules, 1,959 words | ~1.4 ms |
| warm style | ~2.5–3 ms |
| warm grammar + style | ~7–9 ms |
| one edited sentence, native | 2.6–3.7 ms |
| cold 1,959-word page, release wasm single-thread | ~3.0 s |
| cold 1,959-word page, 4-worker | ~1.4 s |
| cold-page target | ≤500 ms, still open |

Document cache semantics: `MorphologyCache` is per textarea and bounded;
`SentenceCache` is keyed by sentence text; there is no AST cache; the AST is
on-demand. State the 104k-lemma / ~3–4 MB compressed wasm context and the
practical document size caveat.

## 11. Phase 8 — future linguistic work (gated)

Only after the contract phases:

- richer grammatical-function model (`SubjectKind`, `ObjectKind`, construction
  features) beyond the phase 2 minimum;
- construction-specific handling for experiencer/necessive if the cues are
  backed by fixtures;
- coordination AST and nested/mixed lists only when a consumer needs them;
- word-order suggestions with conservative confidence and `ALLOW` nets;
- broader evaluation corpus.

Do not use these to expand `SyntacticRole` before the construction-feature
model is settled.

## 12. Priority order

| priority | work | phase |
|---|---|---|
| P0 | restructure/terminology/table checks | 0 |
| P0 | case evidence and syncretism | 1 |
| P0 | role versus construction | 2 |
| P0 | segmentation precedence | 3 |
| P0 | confidence propagation + DiagnosticKind/RuleId mapping | 5 |
| P1 | agreement/construction matrix | 4 |
| P1 | coverage matrix + API contract | 6 |
| P1 | evaluation methodology | 7 |
| P2 | performance/cache documentation | 7 |
| P3 | richer function model, coordination, word order | 8 |

## 13. Acceptance

The hardening is done when:

- every table in `docs/grammar_syntax.md` validates and every link resolves;
- Part I is “Morphology and inflection” and Part III is “Grammar checker”;
- a reader can answer, for any diagnostic: kind, rule id, severity, license,
  correction policy, source module and fixture;
- case evidence, construction behaviour, segmentation precedence and agreement
  are each documented with examples and tests;
- confidence propagation is explicit and the `Repaired` policy is decided;
- API spans/indices/order/cache guarantees are stated and tested;
- evaluation says “fixture-level on a curated corpus”, not general 100%;
- performance numbers and the open cold-page target are documented;
- no architectural rewrite and no new dependency.

## Appendix A — DiagnosticKind → RuleId map

This is the complete inventory to verify against the code and put in the
reference. `RuleId` is the message key; a kind may have more than one.

| DiagnosticKind | RuleId(s) | Typical severity | Correction |
|---|---|---|---|
| `SubjectPredicateAgreement` | `subject_predicate_agreement`, `participle_agreement` | Error / Warning (weak role) | Inflection |
| `Connegative` | `connegative` | Error / Warning | Inflection, 2sg imperative only |
| `AttributeAgreement` | `attribute_agreement` | Error (number) / Warning (case) | Inflection |
| `PredicativeAgreement` | `predicative_agreement`, `predicative_noun_agreement` | Error / Warning | Inflection/curated |
| `PossessiveAgreement` | `possessive_agreement`, `possessive_missing` | Warning / Suggestion | none |
| `VerbRection` | `verb_rection` | Error / Warning | Inflection/curated |
| `CaseSelection` | `numeral_case`, `adposition_case`, `existential_subject_case` | Error / Warning | Inflection/curated |
| `ObjectCase` | `object_case`, `bare_nominative_object` | Error / Suggestion | Inflection |
| `MissingArgument` | `missing_argument` | Warning | none |
| `ExtraArgument` | `extra_argument` | Warning | none |
| `ClauseStructure` | `incomplete_coordination`, `missing_predicate` | Warning | none |
| `WordOrder` | **none emitted** | — | — |
| `CommaMissing` | `comma_missing`, `parenthetical_comma` | Error / Suggestion | Punctuation |
| `CommaExtra` | `comma_extra` | Warning | Punctuation |
| `CommaPairUnbalanced` | `comma_pair_unbalanced` | Error | Punctuation |
| `QuestionMarkMissing` | `question_mark_missing` | Error | Punctuation |
| `UnbalancedQuote` | `unbalanced_quote` | Warning | none |
| `ListSeparatorMissing` | `list_separator_missing` | Error | Punctuation |
| `ListSeparatorDuplicate` | `list_separator_duplicate` | Error | Punctuation |
| `ListCommaBeforeConjunction` | `list_comma_before_conjunction` | Suggestion | Punctuation |
| `ListIntroducerComma` | `list_introducer_comma` | Error | Punctuation |

## Appendix B — terminology

| avoid | use |
|---|---|
| grammar engine (for inflection) | morphology engine |
| grammar (for both layers) | morphology and inflection vs grammar checker |
| confidence (unqualified) | form / role / diagnostic / correction confidence |
| parser | flat syntax analysis / role resolver |
| “100% accurate” | fixture-level exactness on the named corpus |
| “word order is correct” | `PREFER`/`ALLOW` information-structure decision |

## Appendix C — decisions

1. **Repaired forms in diagnostics.** Option B is chosen for now:
   diagnostics consume readings; correction generation filters for `Plain`; a
   repaired reading may support a warning but never an inflection suggestion.
   If a repaired reading causes a false `Error`, add `form_confidence` to
   `WordInfo` and cap it (Option A).
2. **`SubjectKind` scope.** Implemented as nominative, partitive,
   genitive-necessive, existential, omitted and other. Experiencer remains
   gated behind a verified cue.
3. **`ClauseKind::Coordination` vs `NodeKind::Coordination`.** Both remain
   reserved and unemitted; the reference documents the distinction. Remove
   `ClauseKind::Coordination` only if no consumer needs a clause-level variant
   before 1.0.
4. **WordOrder API.** `DiagnosticKind::WordOrder` exists but no rule emits it.
   It stays a reserved enum member; a rule needs a suggestion-only
   `PREFER`/`ALLOW` fixture pair.
5. **Collective number.** `Number::Collective` is skipped by subject
   agreement (`agreement.rs` returns no subject); the reference says this
   explicitly rather than treating collective as plural.
6. **Middle voice.** It is a morphological reading; no syntax rule currently
   branches on it. The reference says it is recognised but not syntactically
   modelled.
