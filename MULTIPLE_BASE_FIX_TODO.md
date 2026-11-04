# Multiple-base fix plan

Status: **planning.** This file scopes the work; it does not change the engine.

The licence rule is absolute: **no Omorfi or Voikko code in the engine or a
binding.** Both are GPL and the engine is MIT/Apache-2.0. Their *output* may be
read at development time to derive a rule or a curated evidence row, but no
runtime model, no statistical guesser and no bulk table of oracle answers goes
into the crate. See `AGENTS.md`, `PLAN_COMPOUND.md` and `docs/harness.md`.

## TL;DR

A word having several base forms is usually **correct**: Finnish has real
homonymy, and a listed adjective and its source verb really do share participle
forms. Of the 15,315 reference pairs whose first base is not the reference's
lemma, **13,151 have a first base that is itself double-confirmed (`both`) for
the surface** and another **1,613 have a first base one oracle confirms**. Only
**551 of those** have a first base neither oracle confirms, and some of those
are oracle gaps rather than engine bugs. The 751 not-found pairs are a separate
recall problem.

So the fix is not "return one base". It is:

1. **Measure the right thing.** Replace "first base is exactly this pair's
   lemma" with "first base is *a* confirmed base of this surface", reported
   next to the exact figure. The current 97.49% understates the ranking because
   it counts genuine ambiguity as an error.
2. **Audit the 551 unconfirmed first bases.** Fix the class/stem rules that
   invent forms; add an oracle exception only where the form is real and the
   oracle is missing it.
3. **Expose the analysis.** Return every base with its part of speech, reading
   kind and confidence, and let the caller filter by POS or context. Do not
   hide a real verb analysis to make a nominal-only gate look better.
4. **Only then improve exact-first ranking**, with defensible signals
   (confidence, underived over derived) rather than a POS rule or an
   oracle-derived preference table.

## What "more than one base form" means

`lookup word` answers "which lemmas is this surface a form of?". A surface can
be:

- a form of two unrelated nouns (`aava` "open sea" / `aapa` "bog");
- a form of a noun and a verb (`paloin` <- `palko`, `palo`, `pala`, `paloa`,
  `palaa`);
- a form of a listed adjective and of the verb it is derived from
  (`aavistamattoman` <- `aavistamaton` and `aavistaa`);
- a form of two variant lemmas (`abstraktinen` / `abstrakti`, `fani` / `fan`).

All four are real. The reverse index's contract is to report all of them;
`bases_of` must not filter. The open question is only **which one to name
first**, and that is a ranking question, not a recognition question.

## Baseline (2026-11-03)

Reference: `data/kotus_nomines.sqlite`, `ok='both'`, `possessive=''`,
`lemma<>form`, **640,873 pairs**.

| metric | count | share |
|---|---:|---:|
| reference pairs | 640,873 | 100% |
| right base among the answers | 640,122 | 99.88% |
| first base exactly the reference lemma (ranked) | 624,807 | 97.49% |
| first base exactly the reference lemma (sanalista order) | 620,603 | 96.84% |
| words with more than one base | 37,073 | 5.78% |
| distinct ambiguous surfaces | 32,430 | — |
| not found at all | 751 | 0.12% |

The 751 not-found pairs are a separate recall problem (class 18 abbreviations,
class 99, `10*(D)`, `5*F`, ...); they are tracked in `TODO.md` and are **out of
scope** here.

### The 15,315 found-but-not-first pairs

For every one of these, the reference lemma is among the answers but the
ranked first base is a different lemma. Checking that first base against the
nominal and verb oracles gives:

| first base's status | pairs | reading |
|---|---:|---|
| `both` in the nominal oracle | 4,759 | a different **confirmed** nominal lemma |
| `both` in the verb oracle | 8,392 | a different **confirmed** verb lemma |
| `omorfi` in the nominal oracle | 781 | confirmed by one authority |
| `omorfi` in the verb oracle | 832 | confirmed by one authority |
| `engine` in the nominal oracle | 465 | unconfirmed |
| absent from both | 86 | unconfirmed |
| **total** | **15,315** | |

Measured by re-running the 15,315 wrong-first rows from `reports/recall4.tsv`
through the full index and joining the first base against both oracles
(`kotus_nomines.sqlite` and `kotus_verbit.sqlite`); Phase 0 makes this
reproducible as a checked-in tool.

Grouped by what the first base is:

| first base | pairs |
|---|---:|
| confirmed alternative (nominal + verb `both`) | **13,151** |
| one-authority alternative (`omorfi` only) | **1,613** |
| unconfirmed (`engine` or absent) | **551** |

That gives three different "first-base" scores.

Over all 640,873 pairs:

| score | count | share |
|---|---:|---:|
| exact reference lemma (today's figure) | 624,807 | 97.49% |
| first base is **a** `both`-confirmed base of the surface | 638,058 | **99.56%** |
| first base is confirmed by at least one oracle | 639,699 | **99.82%** |
| first base is unconfirmed | 1,174 | **0.18%** |

The 1,174 unconfirmed rows split into **551 found-but-not-first** (the ranking
audit list) and **623 not-found** rows; 596 of the not-found rows have no first
base at all. Over the 640,122 found pairs the figures are 97.61%, 99.66%,
99.91% and **551 (0.086%)**. The plan's audit target is the 551, not the 623
recall misses.

### The unconfirmed 551

The verb oracle (`data/kotus_verbit.sqlite`) has `ok IN ('both','omorfi')`
only: there is no `engine` tier on that side, so a verb winner is either
double-confirmed, one-authority, or absent.

The 465 nominal `engine` winners cluster on short/loan/variant lemmas:

| winner | pairs | winner | pairs |
|---|---:|---|---:|
| `kat` | 30 | `karhi` | 24 |
| `gin` | 24 | `led` | 24 |
| `chat` | 17 | `kimrööki` | 17 |
| `taido` | 17 | `gnu` | 14 |
| `hipat` | 13 | `lieri` | 13 |
| `sani` | 13 | `siskokset` | 13 |
| `kokkarit` | 12 | `puola` | 11 |
| `ori` | 10 | `raukki` | 8 |

The 86 absent winners cluster on a few lexemes:

| winner | pairs | note |
|---|---:|---|
| `peljätä` | 25 | dialectal variant of `pelätä`; the verb oracle may not have it |
| `stop` | 17 | loan/interjection; check the sanalista row and class |
| `turta` | 11 | check whether the form is really a `turta` form |
| `hepo` | 8 | colloquial `hepo`; the nominal oracle may not list it |
| `oieta` | 3 | class `72*D, 74*D`; `audit_verbs.py` reports many invented forms |
| `raueta` | 3 | alternate-class verb; same audit |
| `tyvetä` | 3 | alternate-class verb; same audit |
| others | 16 | `valastaa`, `hakoa`, `huumeilla`, `kivetä`, `murteistaa`, ... |

These are **candidates**, not 551 confirmed bugs. Each must be classified as
(a) wrong class/stem, (b) oracle gap, or (c) real but unconfirmed.

### Why the exact figure is misleading

`aavistamattoman` is the clean example:

```
lookup aavistamattoman
  aavistaa      [53]    verb, genitive singular active
  aavistamaton  [34*C]  adjective, genitive singular
```

Both are real: `kotus_verbit.sqlite` has `aavistamattoman` under `aavistaa`
with `ok='both'`, and `kotus_nomines.sqlite` has it under `aavistamaton` with
`ok='both'`. The nominal-only gate treats the verb answer as a miss because
the reference has no verb row for that pair. It is not a miss; it is ambiguity.

`check_normal aavistamattoman` currently returns `aavistaa`, the verb reading.
That is defensible, but it is only one of the two; the CLI should show both or
take `--pos`, and the first base should follow the documented policy rather
than whichever lemma the walk reached first.

The same holds for the `-va` and `-nut` families: `elävä`/`elää`,
`ahdistunut`/`ahdistua`, `herännyt`/`herätä`. Of the 9,310 first bases the
nominal reference cannot see, 9,224 are confirmed by the verb oracle. Ranking
nominals over verbs would make the nominal-only number look better while
making the engine's answer worse for a verb query.

For scale: a blanket nominal-first rule could raise exact-first by at most
about 1.45 points (9,310 / 640,873), to roughly 98.9%, but it would put a
nominal first for surfaces where the verb reading is double-confirmed. That is
the wrong trade unless the caller supplied context or asked for a POS.

## Design

### One analysis record

Add a public `BaseAnalysis` (name provisional) returned by

- `FormBuilder::analyses_of(word) -> Vec<BaseAnalysis>`
- `FormBuilder::ranked_analyses_of(word, &ClassPrior) -> Vec<BaseAnalysis>`

with at least:

```rust
pub struct BaseAnalysis<'a> {
    pub lemma: &'a str,
    pub classes: &'a [crate::kotus::Inflection],
    pub part_of_speech: &'a [PartOfSpeech],
    pub kind: ReadingKind,          // Nominal | FiniteVerb | Participle | Other
    pub confidence: Confidence,     // Plain | Repaired | Guessed
    pub cases: u16,                 // the existing case mask
}
```

`bases_of` and `ranked_bases_of` stay as thin projections, so callers that only
want names do not change. `CheckOutcome::Inflected` keeps every base; the CLI
can print them all.

### One ranking key

`rank` in `src/expand/build.rs` and `best_base` currently use different keys:
`rank` sorts by `ClassPrior` only, while `best_base` adds common-prefix length.
`lookup` uses the first and compound analysis uses the second, so they can
disagree about the first base.

`ClassPrior` is a class-size prior, not a frequency prior. A productive
derivation class can be huge (`38` `-inen`, `34*C` `-maton`, `10` `-va`), so it
can outrank the underived base. The derivation tie-break and the confidence
tier are the cheap corrections; a frequency prior is Phase 4.

Extract one key and use it in both:

1. **Confidence**: `Plain` > `Repaired` > `Guessed`. A guessed form must not
   outrank a plain one when both explain the surface.
2. **Reading kind** (open decision 1). The recommended default is:
   - a nominal non-participle reading outranks a participle-only reading
     (listed adjective vs verb participle);
   - a finite verb reading keeps the current class-prior order against a
     nominal, because `paloin` is more likely a verb than a noun in running
     text and the verb oracle confirms it;
   - unknown/indeclinable last.

   This needs `ReadingKind` recorded per base occurrence. If open decision 1
   chooses nominal-first across the board, the same key can express it;
   measure both.
3. **Derivation**: if base B has a source stem equal to base A under
   `crate::derivation::reverse`, prefer A. This is intended to fix `aamu` over
   `aamuinen` and `aateli` over `aatelinen` without a frequency table, but it
   is not universal: `abstraktinen`/`abstrakti` is a doublet where the derived
   form is the reference. Measure it before keeping it. It must not cross part
   of speech: `aavistamaton` is derived from `aavistaa` but is the preferred
   nominal reading, and rule 2 already covers that.
4. **Class prior**: the existing `ClassPrior` score.
5. **Coverage**: common-prefix length, then the index/sanalista order.

The sort must remain **stable** and must **reorder, never filter**. The recall
gate's `len_ok` column is the guard.

### One measurement

Extend `lookup_recall` to print the ranked first base, not only a boolean:

```text
form  lemma  found  nbases  plain_top1  ranked_top1  len_ok  first_base
```

Then `tools/report_multiple_bases.py` can join `first_base` against both
oracles and report:

- exact first (the current figure);
- first base is a `both`-confirmed base of the surface;
- first base is confirmed by at least one oracle;
- first base is unconfirmed (the audit list);
- `len_ok` and recall.

Do not put bulk oracle data in the engine or in a checked-in preference
table. The join is a dev-time Python tool, GPL-side only, and its output stays
in `reports/` (gitignored).

## Phases

### Phase 0 — make ambiguity visible  *(done 2026-11-03)*

- [x] Add `first_base` to `tools/src/bin/lookup_recall.rs` output and update
      the parser in `tools/test_reverse_lookup_recall.py`.
- [x] Add `tools/report_multiple_bases.py`; it reads the TSV and both SQLite
      oracles, prints the table from this file, and writes
      `reports/wrong_first.tsv` (15,315 rows) and
      `reports/unconfirmed_first.tsv` (1,174 rows, of which 551 found).
- [x] Add `lookup --explain` (and accept `--all`, which is the default) so a
      report row can be reproduced by hand with the first base marked and the
      part of speech beside each base.
- [x] Record the baseline in the tool's module docs; do not add a hard floor
      for the exact figure until Phase 2 is measured.

Acceptance met: one command regenerates the 15,315-row table and the 551-row
audit list; no engine output changed.

```bash
python3 tools/test_reverse_lookup_recall.py reports/recall5.tsv
python3 tools/report_multiple_bases.py reports/recall5.tsv
```

### Phase 1 — audit the 551 unconfirmed first bases  *(2-3 days)*

- [ ] Generate `reports/unconfirmed_first.tsv`:
      `form, reference lemma, first base, first base status (nominal/verb),
      class, POS, reading kind`.
- [ ] Triage each row into:
      **A** wrong class/stem -> fix the owning engine;
      **B** oracle gap -> add a curated `data/oracle_exceptions.json` entry or
      document the gap;
      **C** real variant/one-off -> leave, with a comment in the report.
- [ ] Verb suspects: run `python3 tools/audit_verbs.py <lemma>` for `oieta`,
      `raueta`, `tyvetä`, `peljätä`, `kivetä`, `murteistaa`, `huumeilla`,
      `nihkeillä`, `hakoa`. Fix `invented` forms at the class module
      (`src/expand/verbs/class_*.rs`, `src/grammar/verb.rs`), never by adding a
      suffix to `is_known_word`.
- [ ] Nominal suspects: run
      `python3 tools/report_by_class.py -c <class> --split reports/classes`
      for the classes behind `kat`, `gin`, `led`, `chat`, `taival`, `ahti`,
      `altisti`, `anakoluutti`, `hailea`/`haili`, `kapea`/`kapi`. Fix wrong
      stems at the class, not in the reverse index.
- [x] An interjection is a word, not a nominal paradigm: `stop` is class 5 in
      the sanalista but the noun is `stoppi`, so `FormBuilder` now records the
      citation and skips the class-5 forms. This removed 17 rows from the
      audit list without losing recall.
- [x] The nine dual 72/74 verbs keep only the reading the verb oracle
      confirms: class 72 for `oieta`, `raueta`, `isota`, `sietä`, `sueta`;
      class 74 for `aueta`, `tyvetä`, `iljetä`, `juljeta`. `audit_verbs.py`
      now reports 0 invented forms for all nine; this removed 9 audit rows.
- [x] `anakoluutti` was routed through a non-graded exception, but the
      reference confirms the weak `5*C` obliques (`anakoluutin`,
      `anakoluutissa`, `anakoluutista`) and marks the strong ones only
      `engine`. Removing the exception fixed the audit row and 17 missing
      reference pairs; the `5*C` report at `-n 100` is now 0 illegal, 0 missing,
      0 not-plain, 0 mistagged.
- [x] `vietnam` is class 6 but Wiktionary gives Kotus type 5 and the oracle
      confirms `vietnamien`/`vietnameja`, not `vietnameiden`/`vietnameitten`/
      `vietnameita`. The alternants are now rare for this word rather than
      plain; the class-6 report's `vietnam` rows are gone (the class still has
      other unconfirmed alternants, e.g. `sähköpunktuuri` and `neuroni`).
- [x] `lymfa` is listed `9, 10`; Wiktionary gives Kotus type 9/kala and the
      oracle confirms `lymfoitta`/`lymfoissa` while `lymfitta` is only
      `engine`. The class-10 reading is suppressed for this lemma, the same
      way the dual 72/74 verbs are. The class-9 report's `lymfa` rows are gone;
      its remaining `taata` rows are a noun/verb attribution question, not a
      class-9 stem bug.
- [x] `karhi` has two declensions: type 5 (`karhit`, `karheja`) and type 7
      (`karhet`, `karhen`, `karhissa`). The oracle confirms the type-7 plural
      obliques and the type-5 nominative/partitive plural, but the type-5
      plural obliques (`karheissa`, `karheita`, `karheiden`, ...) are only
      `engine`. Those forms stay accepted as `Repaired`, so `karhea`/`karhe`
      wins the plain reading. This removed all 18 `karhi` audit rows.
- [ ] For each row that stays, record the disposition in the commit message
      (and, if the set is stable, a short table in this file). Do not add the
      whole oracle to the engine.

Acceptance: every row in the original 551 has a disposition; A rows lower the
count by fixing forms, and B/C rows are documented rather than hidden. The
interjection, confidence, dual-class, `5*C`, derivation and `karhi` fixes
already take it to 310 found-but-not-first. A fixed form must not regress
recall (the reference pair may still be found through another base).

The remaining 310 triage as:

| disposition | rows | groups |
|---|---:|---|
| real variant / oracle gap | 226 | `kat`, `gin`, `led`, `chat`, `kimrööki`, `taido`, `gnu`, `raukki`, `puola`, `ori`, `lieri`, `sani`, `live`, `hepo`, `peljätä` |
| plural-only lemma | 39 | `hipat`, `kokkarit`, `siskokset` |
| residual derivation | 4 | `omakustanteinen`, `vahakankainen` |
| one-off | 41 | `kalvakas`, `kymmen`, `tee`, `valastaa`, `ketään`, ... |

The 226 variants are real words the oracle does not list (or lists only under a
different lemma); suppressing them would reject real words. The plural-only
lemmas are real but their reference base is the singular lemma. The four
residual derivations need consonant-stem alternations (`omakustanne` ->
`omakustanteinen`, `vahakangas` -> `vahakankainen`), which the current
`-inen`/`-nen` rule does not reach. The one-offs are lexical decisions, not
class rules.

### Phase 2 — rank with confidence and derivation  *(done 2026-11-03)*

- [x] Store the form's `Confidence` with each base occurrence. The top two bits
      of the base id in `first_base`/`more_bases` carry it, so the arrays do not
      grow; `base_id` masks it and a merge keeps the stronger value.
- [x] Extend the rank key with the confidence tier; `Plain` > `Repaired` >
      `Guessed`, then the class prior. `best_base` and `rank` now use the same
      key.
- [x] The suffix-only derivation tie-break was tried first (`aamu` over
      `aamuinen`). It also flipped `abstrakti`/`abstraktinen` and other
      doublets, net exact -8, so it was reverted. The kept rule is narrower:
      an adjective derived from a **noun** that is also a base loses to that
      noun (`aamu`, `huume`, `aateli`), while adjective/adjective doublets
      keep the class prior. Both `-inen` and the `-nen` segmentation (as in
      `emali` + `nen` -> `emalinen`) are read. Homonym rows union their parts
      of speech for this rule while the class prior keeps the first row's
      class (`toiste` is adverb 99 and noun 48; the noun reading must survive).
      Net exact +4, confirmed +4, unconfirmed 334 -> 328, no regression.
- [x] Use one rank key in `rank` and `best_base`; the tests pin the plain-over-
      repaired order and the merge.
- [x] Measure exact-first, confirmed-first and unconfirmed-first before and
      after. Keep the change only if confirmed-first does not fall.

Measured after the interjection, confidence, dual 72/74 and `5*C` fixes
(2026-11-03):

| metric | before | after |
|---|---:|---:|
| exact first | 624,807 / 97.49% | 625,051 / **97.53%** |
| first base confirmed (all rows) | 638,058 / 99.56% | 638,319 / **99.60%** |
| first base confirmed (found rows) | 637,958 / 99.66% | 638,219 / **99.70%** |
| first base one-authority (all) | 639,699 / 99.82% | 639,957 / **99.86%** |
| unconfirmed found-but-not-first | 551 | **310** |
| ambiguous pairs | 37,073 / 5.78% | 37,042 / 5.78% |

The narrow rule is net positive: `aamu`/`aamuinen` and the other noun ->
adjective pairs now name the noun, while `abstrakti`/`abstraktinen` keeps the
class prior. Do **not** expect the exact figure to approach 99.9%: the
confirmed alternatives are genuine double analyses.

Acceptance: `len_ok` is 100%; recall is unchanged; the new `confirmed-first`
metric does not regress; `best_base` and `ranked_bases_of` agree.

### Phase 3 — expose ambiguity in the check paths  *(1-2 days)*

`lookup` builds the full index and ranks. `check` collects the phase-1 bases
but returns them in index (sanalista) order, and `check_normal` uses
`find_any_base_with`, which short-circuits on the first match. Neither applies
the rank key:

- [ ] Rank `BasesReport::bases` before `check::classify` turns it into
      `CheckOutcome::Inflected`; the first base should be the same one
      `lookup` names. The recognition-only report has classes but not the
      per-form readings, so either switch the homonym fallback to
      `find_bases_with` (measure the cost) or rank on classes/POS only.
- [ ] Add `find_best_base_with` for `check_normal` (or a bounded variant) that
      collects the phase-1 head/tail matches (median bucket 3) and applies the
      same rank key, then falls back to the current first-match behaviour for
      the expensive prefix walks. Keep the per-keystroke cost bounded; measure
      against `tools/test_performance.py`.
- [ ] Make `check_normal` and `check` agree with `lookup` on the first base for
      the same word, or document why they cannot (the fast path has no index).
- [ ] Add `check_normal --all` so a caller can see every base; keep
      `check_fast` single-answer for the wasm.
- [ ] Tests: `aavistamattoman`, `ahdistuneen`, `heränneen`, `paloin`,
      `aamuista`, `vireissä`.

Acceptance: the CLI reports the same first base as `lookup` for the fixture
words; the wasm fast path stays within its performance budget.

### Phase 4 — optional frequency or context  *(separate decision)*

Only if exact-first ranking must improve beyond what morphology can justify:

- **Frequency prior**: rank by log lemma frequency within the same reading
  kind/confidence tier. Requires a permissively licensed frequency list
  (MIT/CC-compatible, attributed) and a precomputed blob, like
  `data/precomputed/oi_plural_lemmas.bin`. Do not derive it from the GPL
  oracle.
- **Context resolver**: use `sentence`/`syntax` to choose between a nominal
  and a verb reading from the clause. This is the linguistically correct fix
  for `paloin`/`palo` and `aavistamattoman`/`aavistaa`; it is a separate plan
  and not needed for the reverse-lookup gate.
- **Curated preference list**: a small hand-checked table of genuine doublets
  (`aava`/`aapa`, `taival`/`taipale`) is acceptable only as a last resort. It
  must be MIT, sourced, and kept small; it must not become a bulk oracle
  dump.

## Tests and gates

- `cargo test --release --workspace`; `cargo clippy --all-targets -- -D warnings`.
- `python3 tools/test_reverse_lookup_recall.py reports/recall7.tsv`:
  recall floor stays **99.8%**; `len_ok` must stay 100%.
- New metric script: exact, confirmed-first, one-authority-first,
  unconfirmed-first; record the baseline in the script docstring and add a
  floor only after Phase 2.
- `python3 tools/test_binding_agrees_with_binary.py` after any `BaseAnalysis`
  or `CheckOutcome` change.
- `python3 tools/audit_verbs.py` for every verb lemma touched.
- `python3 tools/report_by_class.py` for every nominal class touched.
- Unit tests:
  - `rank` and `best_base` agree;
  - a `Plain` form outranks a `Guessed` form of the same surface;
  - `aamu` outranks `aamuinen` on `aamuista` (derivation tie-break);
  - `aavistamattoman` keeps both `aavistaa` and `aavistamaton`, and the first
    base follows the documented policy;
  - `paloin` keeps all five bases;
  - ranking never changes `bases_of` membership.

## Acceptance

- `first_base` agrees with `lookup` for the fixture words.
- `confirmed_first` >= **99.60%** over all rows (99.70% over found rows) and
  `one_authority_first` >= **99.86%** (99.95% over found rows); no regression.
- exact-first >= **97.53%**; further gains must come from derivation,
  frequency or context, not from a POS rule.
- `unconfirmed_first` <= **310** found-but-not-first, with every remaining row
  disposed of in the report; the 606 not-found rows are recall work.
- ambiguity rate stays around **5.8%**; it is not a target. A drop is allowed
  only when an analysis is proven spurious.
- recall >= **99.88%**; `len_ok` = 100%; workspace tests and clippy green.

## Non-goals

- Do not remove real analyses to make `nbases` smaller.
- Do not rank nominals over verbs by default to satisfy the nominal-only gate.
  The verb oracle confirms 9,224 of the 9,310 first bases the nominal
  reference cannot see.
- Do not copy the oracle's preferred base per surface into the engine.
- Do not add suffix shortcuts to `is_known_word` to make one word rank better;
  fix the class or the rank key.
- Do not add a frequency table or a context model inside this plan without a
  separate decision.

## Open decisions

1. **What is the first-base contract?** Exact reference lemma (today), any
   confirmed base (recommended for the validity metric), or a POS-filtered
   answer (`lookup --pos noun`)? The recommendation is to report both exact
   and confirmed, and let `lookup --pos` answer the filtered question.
2. **Do we add a verb half to the recall gate?** Recommended: a sampled verb
   gate using `data/kotus_verbit.sqlite` (7.1M `both` rows), because 60% of the
   exact misses are confirmed verb analyses. Without it, the ranking is judged
   against a reference that cannot see half the language.
3. **Is a frequency list acceptable?** If yes, Phase 4; if no, genuine
   nominal-nominal ambiguity stays and exact-first is capped by it.
4. **Should `check_normal` return all bases instead of one?** Recommended for
   the CLI (`--all`); keep the wasm fast path one answer.

## Related work

- Recall misses (`atk:hon`, `tuhaksi`, `ainutta`, ...): `TODO.md`,
  `reports/recall4.tsv`.
- Class engines: `src/expand/nominals/`, `src/expand/verbs/`,
  `src/grammar/verb.rs`.
- Harness: `docs/harness.md`, `tools/report_by_class.py`,
  `tools/audit_verbs.py`, `tools/test_reverse_lookup_recall.py`.
- Compound ambiguity: `PLAN_COMPOUND.md` Phase 5; this plan does not change
  compound parsing.
