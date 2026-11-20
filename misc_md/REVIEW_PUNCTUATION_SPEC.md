# Review: punctuation checker against the programmatic specification

Status: **review.** Reviewed at `6e91518` with the current `check_syntax` and
the prose/corpus gates. This is a gap analysis; the implementation order is in
[`PLAN_PUNCTUATION_SPEC.md`](PLAN_PUNCTUATION_SPEC.md). Phases 1–2
(indirect-question and vocative commas) have since landed; the matrix below
still records the original gaps for the remaining phases. The specification's own priority is high precision; the checker is
deliberately conservative, so most gaps below are **missing recall**, not wrong
corrections.

## Summary

The implementation is strongest where the specification is most syntactic and
least lexical:

- a clause analysis with main/subordinate/relative kinds;
- subordinate and relative closing commas;
- list diagnostics when the list structure is already visible;
- question-mark corrections with dash, line-wrap, correlative and rhetorical
  guards;
- decimal/date tokens that never become clause punctuation;
- confidence levels and rule-level suppression.

The largest gaps are structural:

1. **No rule for two independent main clauses** (`Menin kotiin ja hän jäi
   töihin.` is quiet). This is not just a missing suffix check: the clause
   parser currently returns **one** clause for both the different-subject and
   the shared-subject form, so there is no reliable boundary to punctuate.
2. **No indirect-question comma** (`En tiedä missä hän asuu.` is quiet).
3. **Relative clauses only get a closing comma when an opening comma already
   exists**; the opening comma is never inserted (`Mies joka tuli eilen on
   opettaja.` is quiet).
4. **Vocatives, appositions and general parentheticals are not detected**
   (`Hei Maija!`, `Helsinki Suomen pääkaupunki sijaitsee etelässä.` are quiet).
5. **No spacing/typography layer** (`sana , toinen` is quiet).
6. **Lists need existing separators**; an unseparated list is not recognized
   (`Ostin omenoita banaaneja ja päärynöitä.` is quiet).
7. **Quotations are only balanced**, not punctuated with the reporting clause;
   colons, semicolons and dashes have no dedicated rules.

## Spec-to-implementation matrix

### 1. Commas

| spec | status | evidence / current behaviour |
|---|---|---|
| 1.1 two independent main clauses | **missing** | `Menin kotiin ja hän jäi töihin.` → 0 diagnostics; parser reports one clause. |
| 1.2 shared constituent, no comma | **safe by omission** | `Menin kotiin ja söin ruokaa.` → 0; but the engine does not actually prove the shared subject, it simply has no main-main rule. |
| 1.3 main + subordinate | **partial** | `Tiedän että hän tulee.` and `Kun tulin kotiin kaikki nukkuivat.` insert the comma. Multiword and rarer openers are not all covered. |
| 1.4 subordinating conjunctions | **partial** | `is_subordinator` covers `että`, `kun`, `jos`, `vaikka`, `koska`, `jotta`, `mikäli`, `ellei`; multiword `ennen kuin` / `sen jälkeen kun` are not a single opener. |
| 1.5 indirect questions | **missing** | `En tiedä missä hän asuu.` → 0; `missä` is not treated as a clause opener for comma purposes. |
| 1.6 relative clauses | **partial** | `Koira, joka nukkuu sohvalla on minun.` gets `comma_pair_unbalanced`; `Mies joka tuli eilen on opettaja.` gets no opening comma. |
| 1.7 multiple subordinate clauses | **partial** | Each clause is checked, but the parser merge behaviour decides how many clauses it can see. |
| 1.8 coordinated subordinate clauses | **no dedicated rule** | `että ... ja että ...` is quiet when no comma is needed, which is safe but not a positive rule. |
| 1.9 lists | **partial** | List separator/duplicate/conjunction rules fire when `syntax_list` has a list. `Ostin omenoita banaaneja ja päärynöitä.` has no list separator and is quiet; semicolon lists are not supported. |
| 1.10 coordinated elements without conjunction | **missing** | `Hän oli rauhallinen ystävällinen kohtelias.` is quiet; no adjective/list coordination rule. |
| 1.11 vocative | **missing** | `Hei Maija!` and `Tule tänne Pekka.` are quiet. |
| 1.12 parentheticals | **partial** | `esimerkiksi` and `kuten` pairs are implemented; general parentheticals and appositions are not. |
| 1.13 appositions | **missing** | `Helsinki Suomen pääkaupunki sijaitsee etelässä.` is quiet. |
| 1.14 sentence-initial connectives | **correctly quiet** | `Valitettavasti en pääse mukaan.` has no forced comma. |
| 1.15 `kuin` | **good** | Comparison clauses do not get a comma (`Hän on nopeampi kuin minä.`); the `niin kauan kuin` false positive was fixed. |
| 1.16 `kuten` | **partial** | Parenthetical `kuten` pairs are checked; integrated comparison stays quiet. |
| 1.17 `paitsi`, `myös`, `saati` | **no dedicated rule** | No blanket comma, which matches the spec's warning. |
| 1.18 paired constructions | **missing** | `Mitä enemmän …, sitä …` has no rule. |
| 1.19 non-finite constructions | **safe by omission** | No comma is forced; integrated constructions stay quiet. |
| 1.20 quotations/reporting | **partial** | Quote balance only; `Hän sanoi: ”Tulen huomenna.”` is quiet, but a missing comma/colon around the reporting clause is not corrected. |
| 1.21 titles/designations | **missing** | No apposition rule. |
| 1.22 decimals/dates | **good** | `3,14` and `20.11.2026` are single tokens and never become clause punctuation. |

### 2–3. Marks, typography and whitespace

| spec | status | evidence |
|---|---|---|
| periods | **no insertion rule** | Headings/fragments stay quiet; no sentence-final period correction. This is intentional (few false positives, low recall). |
| question marks | **good** | `Missä sinä asut.` → `?`; dash, line-wrap, correlative relative and rhetorical conditional are guarded; sentence cache parity tested. |
| exclamation marks | **no rule** | `Tule tänne.` is not converted to `!`; that is the correct conservative choice. |
| colon | **partial** | List-introducer comma is checked; colon insertion for explanations/quotations is not. |
| semicolon | **no rule** | `Hän lähti kotiin; minä jäin.` is accepted and not rewritten. |
| dash | **tokenized, no rule** | A dash no longer causes a question-mark insertion; no dash-spacing or clause-boundary rule. |
| parentheses | **no spacing rule** | Parenthetical text is parsed; no `asia(lisätieto)` spacing diagnostic. |
| quotation marks | **balance only** | `»…»`/`”…”` depth is tracked at document end; style is not normalized. |
| ellipsis | **no rule** | `...`/`…` are tokenized as separators; no normalization. |
| whitespace/typography | **missing** | `sana , toinen` → 0 diagnostics. The spec puts this in a separate formatting layer; the checker has no such layer. |

### 4–6. Architecture, confidence, tests

| requirement | status |
|---|---|
| Unicode-aware tokenization | **yes**; `ä`/`ö`/`å`, quotes, dashes, numbers are tokenised. |
| morphology / clause analysis | **yes**, heuristic; no dependency parser. `ClauseKind` is the basis of punctuation. |
| rule priority and confidence | **yes**; `Severity`, `Confidence`, `CorrectionLicense`, `Ambiguous` suppression and `rank.rs`. |
| decimal/date protection | **yes**; they are single tokens. |
| confidence-based correction | **yes**, but many rules are `Certain`/`Error`; the safer future default is `Likely`/suggestion for structural guesses. |
| positive/negative/boundary tests | **good for implemented rules**; missing for main-main, indirect questions, vocatives, appositions, spacing, semicolon lists. |
| prose regression net | **yes**; 266 clean rows / 6,349 words, 2 true positives, 0 known false positives. |

## Why the critical gap is structural

`Menin kotiin ja hän jäi töihin.` currently produces:

```text
clauses: 1  tokens: 12  diagnostics: 0
```

The same one-clause result appears for `Menin kotiin ja söin ruokaa.` There is
no `Clause` boundary for the comma rule to use, so adding a lexical `ja` rule
would have to decide shared-vs-independent subject itself. That is exactly the
kind of word-trigger rule the specification warns against. The right fix is a
coordination split in the clause analysis (or a dedicated coordination
analysis) that records:

- the two finite predicates;
- each side's subject;
- whether a constituent is shared;
- whether the coordinator is `ja`, `mutta`, `vaan`, `sillä`, `joten`, `tai`,
  `vai` or `sekä`;
- the boundary before the second clause.

Only then can FI-COM-001 and FI-COM-008 be implemented without a lexical
comma-before-`ja` rule. The same analysis would help the relative-opening rule
and the `joka`/`mikä` distinction.

## Recommended next slice

Ordered by precision-to-risk, not by spec order:

1. **Indirect-question comma** (1.5). `En tiedä missä hän asuu.` already has
   two clauses; add the interrogative opener to the subordinate boundary
   logic. Low risk, high value.
2. **Vocative** (1.11), narrow form: sentence-initial interjection or
   address + proper/common noun before a finite predicate
   (`Hei Maija!`, `Tule tänne, Pekka.`), `Suggestion`/`Likely`, not `Error`.
3. **Spacing layer** (section 3). A separate `typography` pass, not a
   syntax rule: no space before `, . ; :`, one after; preserve decimals/dates
   and code/URLs. It should be suggestion-only and opt-in.
4. **Main-main coordination** (1.1/1.2). Requires the coordination split
   above. Implement `Likely`/`Suggestion` first, with the shared-subject
   guard; never a lexical `ja` rule.
5. **Relative opening comma** (1.6). Only after the relative clause can be
   identified independently of an existing opening comma; treat restrictive
   vs non-restrictive as `Likely`.
6. **Lists** (1.9). Extend `syntax_list` to recognize unseparated parallel
   items and semicolon lists; do not infer a list from two adjacent nouns
   alone.
7. **Quotation/reporting** (1.20) and **colon/semicolon/dash** (2.4–2.6)
   once the clause/quote model can say which side owns the terminal mark.

The two currently remaining `page-2000` rows are `subject_predicate_agreement`,
not punctuation; they are a separate precision target.

## Test additions for the next slice

Every new rule needs four cases, as the spec asks:

- positive: the missing comma/vocative/spacing is corrected;
- negative: shared constituent, integrated comparison, decimal/date, code/URL;
- boundary: same trigger in another role (`kuin` comparative vs clause,
  `joka` determiner vs relative, `ja` shared vs independent);
- ambiguous: preserve the text (e.g. a main-main boundary whose subjects are
  not both resolvable).

The prose corpus stays the regression net; a new rule that adds a row to the
clean set must be reviewed rather than pinned as a known false positive.
