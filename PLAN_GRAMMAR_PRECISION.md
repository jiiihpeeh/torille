# PLAN_GRAMMAR_PRECISION — closing the adjudicated false positives

Status: **18 diagnostics on the committed prose page, 13 adjudicated false
positives across nine phases; phases 0–4 landed** · 2026-11-18 · branch
`syntax` · baseline `b8b3687`

This plan closes the false positives that remain on
`tests/fixtures/perf/page-2000.txt` after the argument-selection and agreement
rounds (54 → 27). It is a **precision** plan: no new rule, no new diagnostic
kind, no change to the keystroke path. The grammar layer keeps its
false-negative-first policy — a rule that would silence a true positive is
rejected, not tuned.

Contents: [§1 where we are](#1-where-we-are) ·
[§2 the remaining rows](#2-the-remaining-rows) ·
[§3 rules of engagement](#3-rules-of-engagement) ·
[§4 the phases](#4-the-phases) · [§5 acceptance](#5-acceptance) ·
[§6 risks](#6-risks) · [§7 file map](#7-file-map)

## 1. Where we are

`page-2000` (1,959 words) reports **18** grammar diagnostics. Every one was
adjudicated by hand: 13 false positives, 1 true positive, 3 accepted, 1
excerpt artefact.

| kind | rows | false positives |
|---|---:|---:|
| `verb_rection` | 6 | 6 |
| `question_mark_missing` | 4 | 2 (1 accepted, 1 true positive) |
| `list_separator_missing` | 3 | 3 |
| `subject_predicate_agreement` | 2 | 0 (2 accepted) |
| `attribute_agreement` | 1 | 1 |
| `object_case` | 1 | 1 |
| `unbalanced_quote` | 1 | 0 (excerpt ends inside a quote) |

The workflow is a review loop:

```bash
cargo run --release -q -p torille-tools --bin dump_diagnostics
cargo run --release -q -p torille-tools --bin dump_diagnostics -- --words 1959
```

`dump_diagnostics` prints one block per finding — kind, byte span, offending
text, first correction, and the sentence around it — which is what makes an
adjudication reviewable. `docs/harness.md` is the lexicon harness; this plan
is about the grammar rule layer only.

### 1.1 The measurement invariant

Every phase is judged by the same three-way check:

1. the rows the phase names move from `known_false_positives` to `clean` in
   `tests/fixtures/prose/adjudicated.json` **when they are pinnable there**;
2. no other row on `page-2000` appears or disappears;
3. the clean corpus, the exact-output fixtures and `m0_prose_eval` stay green.

"Improvement" is never a lower count on its own: a phase that removes one row
and adds another is a failed phase and is reverted.

### 1.2 What the prose fixture can pin

The prose fixture checks each row in isolation. Of the current 13 false
positives, seven reproduce alone (seven entries in `known_false_positives`)
and six do not (five page-only rows: `paljon hyvää`, the two `lapsineen`
findings, `puute`, and two of the question-mark rows). The fixture form of
each row is in the §2 table.

A page-only row is still pinned by the plan: the phase that fixes it adds a
**purpose-built sentence** to `syntax_diagnostics.json` that exercises the same
cue, plus the page count invariant above. Phase 1 fixed the `tuuli` bearer with
its line break preserved; Phase 2 fixed the `jolloin … naisihmisen` comma pair
with an `yllään` sentence. Five page-only rows remain.

### 1.3 Two structural root causes behind several rows

1. **A newline is a sentence end.** `sentence::separator_ends_sentence`
   returns `true` for any `\n`, so a line-wrapped sentence becomes two
   sentences. Three `question_mark_missing` rows are this and nothing else:
   `Voitteko Jumalan silmien edessä` ends the line and the `?` is on the next
   one, and the `miten kauheasti …` line is the same shape. Changing the
   sentence layer is out of scope here (it is the style metric and the
   sentence-cache key); the question rule can see that the sentence continues
   past a line break and stay quiet. The same split made the Phase 1 agreement
   row: `tuuli` ends the line and was left as a one-word predicate, while
   `puhalteli` began the next sentence.
2. **A comma at a boundary is invisible when a quote sits beside it.**
   `kuolla', sanoi Kuntsi` and `valkeni, jolloin` both have the comma the
   rules want to insert; the separator scan stops at the quote or at the
   relative clause. Fixing the boundary test removes three rows without any
   new heuristic.

## 2. The remaining rows

Offsets are byte offsets in `page-2000.txt`. `fixture` is where the row is
pinned: `sentence`, `window` (a two-sentence excerpt), or `page` (page-only,
per §1.2).

| # | kind | span | context | fix | adjudication | phase | fixture |
|---:|---|---|---|---|---|---|---|
| 1 | `subject_predicate_agreement` | 8470 `tuuli` | `…kuu oli laskeutumaisillaan, tuuli puhalteli kylmästi…` | `tuulivat` | `tuuli` is the subject of `puhalteli` | 1 | **fixed** |
| 2 | `comma_missing` | 4516 | `Kun se aamu valkeni, jolloin Kenoveevan täytyi…` | `,` | the comma is already there | 2 | **fixed** |
| 3 | `comma_missing` | 9904 | `'Hänen täytyy kuolla', sanoi Kuntsi.` | `,` | the comma is already there, after a quote | 2 | **fixed** |
| 4 | `comma_pair_unbalanced` | 15040 | `…ulos, jolla oli yllään lampaan nahka…` | `,` | the relative clause ends the sentence | 2 | **fixed** |
| 5 | `comma_missing` | 3771 | `…sievästi, niin että kaikki sanoivat…` | `,` | `niin että` is a fixed conjunction | 3 | **fixed** |
| 6 | `comma_missing` | 10627 | `…hiljaista, sen kun Anni vain kertoi…` | `,` | `sen kun` is a fixed adverbial | 3 | **fixed** |
| 7 | `comma_missing` | 10654 | same sentence | `,` | same | 3 | **fixed** |
| 8 | `verb_rection` | 9153 `se` | `…katsokaa, se peitäkse pilven taa…` | `sitä` | `se` is the 3sg subject | 4 | **fixed** |
| 9 | `verb_rection` | 5105 `kaikki` | `…jolloin kaikki tänne kokoontuneet Jumalaa…` | `kaikkeen` | `kaikki` is a plural subject | 4 | **fixed** |
| 10 | `object_case` | 13182 `se` | `…kerro, niinkuin se sitten kävi.` | `sitä` | `niinkuin` opens a comparison clause | 5 | sentence |
| 11 | `verb_rection` | 11079 `lapsi` | `Hän jäi lapsi sylissä istumaan…` | `lapseen` | `lapsi sylissä` is an absolute phrase | 5 | sentence |
| 12 | `verb_rection` | 11341 `lapsi` | `Hän lähti lapsi sylissä kulkemaan…` | `lapseen` | same | 5 | sentence |
| 13 | `verb_rection` | 5952 `Päivän` | `Päivän vaietessa` | `päivään` | genitive subject of an impersonal non-finite verb | 5 | window |
| 14 | `verb_rection` | 14764 `niitä` | `…lähti niitä seuraamaan.` | `niihin` | `niitä` is the infinitive's object | 6 | sentence |
| 15 | `verb_rection` | 6583 `sinut` | `…uskon sinut … haltuun.` | `sinuun` | the illative `haltuun` satisfies the frame | 6 | sentence |
| 16 | `verb_rection` | 8210 `Jumala` | `…'Jumala, ole minulle armollinen…'` | `jumalaan` | vocative | 7 | sentence |
| 17 | `attribute_agreement` | 9821 `hyvää` | `kuinka paljon hyvää hän … osoitti` | `hyvä` | `hyvää` is the head of `paljon` | 7 | page |
| 18 | `list_separator_missing` | 11698 | `…suojassa sateelta ja tuulelta, mutta…` | `,` | not a list | 8 | page |
| 19 | `list_separator_missing` | 11707 | same | `,` | same | 8 | page |
| 20 | `list_separator_missing` | 13463 | `…puute, niin yhtenä päivänä…` | `,` | across a clause boundary | 8 | page |
| 21 | `question_mark_missing` | 9357 | `miten kauheasti puut heiluvat…` | `?` | the sentence continues on the next line | 9 | page |
| 22 | `question_mark_missing` | 9431 | `Voitteko Jumalan silmien edessä` | `?` | the `?` is on the next line | 9 | page |
| 23 | `question_mark_missing` | 9882 | `'--'Hänen täytyy` | `?` | a line-break artefact inside a quote | 9 | page |

Accepted and **not** to be "fixed":

| kind | span | why it stays |
|---|---|---|
| `question_mark_missing` | 1084 | `Muistatko--muistatko …` is a direct `-ko` question with a period (the fixture's one `errors` row) |
| `subject_predicate_agreement` | 11995, 12014 | colloquial `ne on` in dialogue; `ovat` is the prescriptive form and a defensible finding |
| `unbalanced_quote` | 15280 | the committed excerpt ends inside a quotation |

## 3. Rules of engagement

1. **One family per phase, one commit per phase.** Pinnable rows move from
   `known_false_positives` to `clean` in the same commit, so a regression is
   red instead of silent.
2. **A closed set stays closed and documented.** Where a cue is lexical (a
   fixed conjunction, a quantity adverb, a transfer verb), it is a short
   `const` next to the rule with a comment naming the examples, the way
   `ASPECTUAL_OBJECT_VERBS` and `DIRECTION_EXTENT_ADVERBS` already work.
3. **No new suffix stripper.** A false positive is a rule reading a word it
   cannot defend; the fix belongs in role assignment, agreement, segmentation
   or the rule's own candidate filter — never in `is_known_word`'s probes.
4. **No rule may silence a true positive.** Each phase adds a *positive*
   fixture for the error the rule still catches (`Menet koulu.`,
   `Haluan mennä koulu.`, `Missä sinä asut.`), not only a clean row.
5. **Prefer the structural cue.** Making the clause structure right (a
   comparison clause, an absolute phrase, a fixed conjunction) beats a lexical
   exception for one word.
6. **Measure both sides.** `dump_diagnostics` before and after; the page diff
   must show exactly the phase's spans and no others.

Gates per phase:

```bash
cargo test --release -p torille --test syntax_check --test syntax_ambiguity \
  --test syntax_prose --test syntax_eval
cargo test --release --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -q -p torille-tools --bin m0_prose_eval -- \
  --file tests/fixtures/prose/adjudicated.json
cargo run --release -q -p torille-tools --bin dump_diagnostics
```

## 4. The phases

### Phase 0 — pin the rows and ship the review tool — **landed**

- `tools/src/bin/dump_diagnostics.rs` (kind, span, context, first correction;
  `--file`, `--words`).
- 13 rows (14 findings) pinned in `known_false_positives`: eleven in one
  sentence, one with the preceding sentence (`Päivän vaietessa`), one as a
  two-sentence quote window (`kuolla', sanoi Kuntsi`), and the `sen kun` row
  with its two comma kinds. Each entry carries a `note` naming the phase.
  The suite reports **11.4 findings per 1,000 words** over 1,021 clean and 212
  known-false-positive words.
- The eight page-only rows are listed in §1.2 and §2; they are tracked by the
  page count and by the purpose-built fixture each phase adds.

Acceptance: `syntax_prose` prints the false-positive rate over the clean word
count and fails on any unpinned change.

### Phase 1 — an inherited subject keeps its number — **landed**

The original diagnosis (a bearer picked from the first finite reading) was
wrong: `agreement::bearer_of` already uses `clause.predicate`, with the
negative and compound-tense auxiliaries as the exceptions it models. The row
is a consequence of root cause 1: the page wraps `tuuli\npuhalteli`, so
`tuuli` is left alone at the end of its sentence and becomes a one-word
predicate through its `tuulla` reading.

`share_clause_roles` then let that clause inherit `tähdet` from the preceding
clause. `clause_subject_person_number` collected only subject *tokens inside
the clause*; an inherited subject is an index outside that range, so it
returned `None`, and `predicate_matches_previous_subject` treats `None` as
“do not block sharing”. The agreement rule compared the 3sg `tuuli` predicate
with the plural `tähdet`.

- `clause_subject_person_number` now includes `clause.subject` when it is not
  among the clause's own subject tokens. The inheritance check then compares
  the predicate with the subject that would actually be shared.
- A mismatching share is refused, leaving the clause with no subject and
  therefore no agreement controller: quiet, which is the false-negative-first
  outcome for a clause whose subject cannot be established.
- No change to `bearer_of`; the page count falls by exactly this row.

Acceptance: row 1 gone from the page (27 → 26); `Koira nukkuvat.`,
`Minä menee kotiin.`, `Hän antoivat kirjan.` still fire; the sentence with
its original line break is a clean row in `syntax_diagnostics.json`.

### Phase 2 — a comma already at the boundary counts — **landed**

Three findings inserted a comma where one was already written. Each had a
different reason, and all three are now gone without a new rule.

1. **The merged relative clause (row 2).** `Kun se aamu valkeni, jolloin …`
   leaves `jolloin` inside the same `Clause` as `valkeni`, so the
   merged-subordinate path (`finite.len() >= 2`) saw a second finite predicate
   and inserted a comma before it. `comma_before_relative` now checks whether a
   comma already sits before a relative pronoun or the relative adverb
   `jolloin` between the two finite words. `jolloin` is deliberately *not*
   added to the global `RELATIVE_SURFACES`: that changes clause segmentation
   and also removes the Phase 4 `kaikki` row, which this phase must leave
   alone (measured).
2. **The quote after a full stop (row 3).** The sentence layer treats `. 'Jos`
   as non-terminal because the separator ends in an apostrophe quote, so the
   `jos` clause looked like the same sentence as `Kunti.` and `subordinate_comma`
   inserted a comma before the period. The sentence layer is left alone (it is
   the style metric and the sentence-cache key); `same_sentence` now also
   rejects a pair when a terminal mark sits between the clauses
   (`terminal_between`).
3. **A postposition with a possessive suffix (row 4).** `yllään` is the
   postposition `yllä` plus the 3rd-person possessive, but it also carries a
   finite `yltää` reading. `is_finite` accepted that reading, so
   `relative_comma` took it as the relative clause's closing host and inserted
   a comma after `oli`. `is_finite` now requires `possessive.is_none()`: a
   finite verb does not take a possessive suffix.

Acceptance: rows 2–4 clean; the two probe rows move from
`known_false_positives` to `clean`; row 4's `yllään` sentence is a clean row
in `syntax_diagnostics.json`; `subordinate_comma_after_main`,
`subordinate_comma_initial` and `relative_comma_pair` still fire. Page
26 → 23, exactly these three.

### Phase 3 — fixed multiword conjunctions — **landed**

- `niin että` is the consecutive conjunction and takes no comma between its
  halves; `sen kun` is the fixed adverbial (“kept on …”). `NO_COMMA_PAIRS`
  is a closed two-entry table in `punctuation/comma.rs`, checked on both
  clause-boundary paths before a comma is required.
- The second `sen kun` finding was not a subordinator at all: `ajan` is also
  the 1sg of `ajaa`, so the merged-subordinate path took `koko ajan` as a
  second finite predicate. The merged path now skips whenever a comma already
  sits between its last two finite words, which is the same cue Phase 2 used
  for `jolloin` and removes the spurious `koko, ajan` insertion.
- `siten, että` is deliberately absent from the table: it does take a comma.

Acceptance: rows 5–7 clean; `Luulen, että hän tulee.` stays clean and
`Luulen että hän tulee.` still reports the missing comma (new exact-output
case and clean rows); `Niin että hän tuli.` is clean. Page 23 → 20, exactly
these three.

### Phase 4 — demonstrative subjects and a relative adverb — **landed**

The planned agreement test was not the mechanism for either row.

- **Row 9 (`kaikki`).** `jolloin` is a relative adverb of time, but it was not
  in `RELATIVE_SURFACES`, so the `jolloin` clause stayed merged with `Se päivä
  tulee` and `kaikki … kiittävät` was read as a post-predicate argument of
  `tulee`. Adding `jolloin` to the relative surfaces gives the clause its own
  predicate, and `kaikki` is its subject. This also keeps the Phase 2
  `comma_before_relative` fix intact.
- **Row 8 (`se`).** Two merged-clause readings produced the finding. A comma
  before a demonstrative pronoun starts a new clause (`… katsokaa, se
  peitäkse …`), so the rection argument search now skips a demonstrative
  pronoun with a comma between it and the predicate. The negative-clause
  object rule now also requires the clause's negative auxiliary to precede
  the predicate: the `ei` in `ikäänkuin ei voisi` belongs to the later verb,
  not to `katsokaa`. The two guards are independent, so the row stays quiet
  whichever rule reaches it first.

Acceptance: rows 8–9 clean; `Katsokaa se!` still reports `verb_rection`
(new exact-output case), `Menen koulu.` and `Pidän sinä.` still fire, and
`En pidä sinä.` still fires. Page 20 → 18, exactly these two.

### Phase 5 — comparison clauses, absolute phrases, non-finite subjects (rows 10–13)

The riskiest phase; ship its three parts separately.

1. **Comparison clause.** `niinkuin`/`ikäänkuin` introduce a comparison clause
   (`…kerro, niinkuin se sitten kävi`). Add them to the clause markers and the
   segmentation openers, so `se` is that clause's subject rather than the
   imperative's object.
2. **Absolute phrase.** A nominative nominal followed immediately by an
   inessive nominal or adverb, after the clause subject, is a detached phrase
   (`Hän jäi lapsi sylissä istumaan`). Mark it `Adverbial` before the object
   pass so a rection frame cannot rewrite it.
   Guard: `Hän istui tuolissa` and `Kirja pöydällä on minun` keep their
   readings.
3. **Genitive subject of an impersonal non-finite verb.** `Päivän vaietessa`
   has a genitive subject; `Kirjan luettuaan` has a preposed genitive object.
   The discriminator is the **possessive suffix on the non-finite form**
   (`luettuaan` marks its agent) and a closed list of impersonal verbs of time
   and weather (`vaieta`, `hämärtää`, `sarastaa`, `koittaa`, `valjeta`,
   `pimetä`). Only the intersection gets a genitive subject.

Acceptance: rows 10–13 clean; `Haluan mennä koulu.` fires; `Kirjan luettuaan
hän lähti.` keeps `kirjan` as the object (new fixture); `Hän istui tuolissa.`
is clean.

### Phase 6 — the infinitive's object and the transfer frame (rows 14–15)

- **The infinitive's own object.** In `lähti niitä seuraamaan` the partitive
  belongs to `seuraamaan`, not to `lähteä`. When a clause ends in an
  infinitive, a nominal immediately before it is that infinitive's argument:
  skip it in the matrix verb's rection search.
- **A satisfied frame.** `uskoa` has a belief frame (illative) and a transfer
  frame (accusative object + illative goal). In `uskon sinut … haltuun` the
  illative `haltuun` satisfies the frame, so the accusative is the object.
  If the clause already has a nominal in one of the frames' cases, the frame
  is satisfied and no diagnostic is due.

Acceptance: rows 14–15 clean; `Luotan sinä.` and `En pidä sinä.` still fire;
`Hän uskoo sinuun.` stays clean.

### Phase 7 — vocative and quantity head (rows 16–17)

- **Vocative.** A nominative nominal followed by a comma and then an
  imperative predicate is an address (`Jumala, ole minulle armollinen`). Mark
  it detached before the object pass.
- **Quantity head.** An adjective after `paljon`, `vähän`, `runsaasti`,
  `tarpeeksi` is the head of the quantity phrase (`paljon hyvää`), not an
  attribute of a following noun. Exclude it from `attribute_shape`'s
  candidates.

Acceptance: rows 16–17 clean; `Punainen autot.` and `Punaisessa auto.` still
fire; `Minä, menen kotiin.` stays `comma_extra`.

### Phase 8 — list detection over clause boundaries (rows 18–20)

`list_separator_missing` builds a list out of heterogeneous adverbials and
across a comma that ends a clause.

- A list needs at least two items of the **same kind** (same case and part of
  speech, or two clauses of the same shape). `lapsineen suojassa sateelta ja
  tuulelta` are four different obliques, not a list.
- A comma that separates two clauses (a following `mutta`/`niin`/finite verb)
  closes the list: no separator is missing inside it.
- Guard: `Ostin omenoita appelsiineja, banaaneja ja päärynöitä.` still reports
  the missing comma.

Acceptance: rows 18–20 clean; the four `list_*` and nested-list fixtures
unchanged.

### Phase 9 — a line break is not a question mark's sentence end (rows 21–23)

Three `question_mark_missing` rows are the newline-sentence-boundary artefact
of §1.3, not missing marks: the `?` is on the following line.

- `punctuation/question.rs`: when the sentence's last word is followed by a
  separator run that continues into another word (a line break, a quote, a
  dash) rather than terminal punctuation, stay quiet. `Terminal::Insert`
  already represents this shape; split it into “continues” (quiet) and “ends
  the text” (the current behaviour).
- Keep the `-ko` path: `Missä sinä asut.`, `Onko sinulla kirjaa.` and
  `Muistatko--muistatko …` still fire, because each ends at real terminal
  punctuation.
- Do not change `sentence::spans`: it is the style metric and the
  sentence-cache key, and a wider change belongs in its own plan.

Acceptance: rows 21–23 clean; the `question_mark_*` fixtures unchanged.

## 5. Acceptance

The plan is complete when:

1. every row in §2 is either clean or in §2's accepted table with a reviewed
   reason;
2. `page-2000` reports **≤ 6** diagnostics: the true positive, the two
   accepted `ne on` rows, and the excerpt artefact;
3. `known_false_positives` is empty and the printed false-positive rate is 0
   over the clean word count;
4. the clean corpus, the exact-output fixtures, `m0_prose_eval` and the
   workspace suite are green, and clippy is clean;
5. the diagnostic counts on the 20-, 96- and 1,959-word prefixes are recorded
   here.

## 6. Risks

| risk | guard |
|---|---|
| a phase silences a true positive | every phase adds a positive fixture for the error it still catches |
| the prose corpus is one author's style | cues are structural or documented closed sets, never “this book” |
| the bearer change alters a clause's reading | `subjects`/`parents`/`features` are pinned in `syntax_ambiguity.json`; the `disambiguation_noop` gate compares the full analysis |
| an absolute-phrase rule swallows a subject | the phrase needs an overt clause subject first, and the inessive must follow immediately |
| the non-finite genitive rule misfires on preposed objects | possessive-suffix + impersonal-verb intersection, with `Kirjan luettuaan` as a red-on-failure fixture |
| list detection loosens and drops real findings | the list fixtures are exact-output; a missed separator must stay `Error` |
| page-only rows regress silently | the plan's count invariant plus a purpose-built sentence per phase |

## 7. File map

| area | files |
|---|---|
| agreement bearer, subject, attributes | `src/syntax_check/agreement.rs` |
| rection argument selection | `src/syntax_check/rection.rs` |
| roles, absolute phrases, vocatives | `src/syntax/roles.rs` |
| clause segmentation, comparison clauses | `src/syntax/segments.rs` |
| boundary commas, relative comma pair | `src/syntax_check/punctuation/comma.rs` |
| clause kinds | `src/syntax_check/punctuation/clause.rs` |
| questions | `src/syntax_check/punctuation/question.rs` |
| lists | `src/syntax_check/punctuation/lists.rs`, `src/syntax_list.rs` |
| adjudicated prose | `tests/fixtures/prose/adjudicated.json`, `tests/syntax_prose.rs` |
| exact diagnostics and readings | `tests/fixtures/syntax_diagnostics.json`, `tests/fixtures/syntax_ambiguity.json` |
| review tool | `tools/src/bin/dump_diagnostics.rs` |
