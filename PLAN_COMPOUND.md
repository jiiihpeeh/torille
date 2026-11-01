# Compound engine plan

Status: **in progress.** The decomposition and generation halves already exist
and are used by `check`, `suggest` and the harness; this plan is about making
the middle half — *which parts inflect, and in what shape* — explicit instead of
being spread over three special cases in `expand/compound.rs`.

The licence rule is absolute: **no Omorfi or Voikko code in the engine or a
binding.** Both are GPL and the engine is MIT/Apache-2.0. They are dev-time
oracles only (`tools/*.py`, GPL, never linked); their *output* may be read to
derive a rule or a curated evidence row, and every such row names its source.
No runtime model, no statistical guesser, no external data file.

## What the spec asks for, and where it already lives

| Spec concern | Where it is today |
|---|---|
| Split an orthographic word into components | `src/compound.rs::analyze` — `Analysis::{Simple,Compound,Unknown}`, recursive, hyphen-aware, genitive modifiers resolved to their lemma (`talonmies` → `talo` + `mies`) |
| Component roles | `Component { surface, lemma, role: Modifier\|Head }`, `head_index` |
| Hyphenated compounds | `HyphenRule` (`Abbreviation`, `Number`, `ProperName`, `VowelBoundary`, `PairedExpression`); a hyphen must be licensed, so `kirja-hylly` stays closed |
| Morphological generation | `expand/` — the class engines, gradation, harmony, stem alternation; never suffix-appending |
| Class-50 compounds (initial part fixed) | `expand/compound.rs::compound_paradigm` — prefix joined to each tail cell; bound forms (`‑kulmio`), plural tails (`vanhemmat`), compound-like tails (`viinimarja`) |
| Class-51 compounds (initial part inflects) | the same function — head cells × tail cells, matched on case and number |
| Lexical exceptions | the sanalista itself, `build.rs::REJECTED_LEMMAS`, `data/oracle_exceptions.json` |
| Confidence / "do not guess silently" | `Confidence::{Plain,Repaired,Guessed}`; only `Plain` is ever suggested |
| Phrase vs compound | `tokenise`/`sentence` work on word boundaries; `analyze` is deliberately one orthographic word |

So the pipeline the spec describes exists. What it does **not** have is a
declared *inflection mode* per component and a declared *compound kind*; the
planner infers both from the class marker (50 vs 51) and from whether a part is
`carries_a_case`. That inference is why the recent bugs were all one-off
patches:

- a head that is a participle (`särkynyt` in `särkynytsydän`) is not a lemma;
- a head that is a nominative plural (`isot` in `isotaivot`) is not the lemma;
- a tail that is a bound form (`‑kulmio`), a plural (`vanhemmat`), or a
  compound of its own (`viinimarja`).

Each is now handled, but each was found by a report row rather than by the
model.

## Gaps

| # | Gap | Impact | Phase |
|---|---|---|---|
| 1 | `Component` has no inflection mode; `Compound` has no kind | every new compound shape is another special case in `find_parts` | 1 |
| 2 | A classless row is a compound (56 486 sanalista rows) but is not first-class: `inflect_all` reaches it through `compound_head_forms`, and every form is `Repaired` | `check_fast`/wasm and the reverse index now see the forms (2026-11-01), but they are never suggested, and the class report cannot measure them | 2 |
| 3 | A genitive-initial modifier is resolved to its lemma but the case is not recorded | the planner cannot tell `talonmies` (frozen genitive) from `talomies` (a typo); `explain` cannot say why `talon` is unchanged | 3 |
| 4 | Coordinated compounds (`suomalais-ruotsalainen`, `teoria-käytäntö-suhde`) are recognised as hyphenated but not as *two inflecting parts* | `suomalais-ruotsalaisia` needs both halves; today only the writing rule is known | 3 |
| 5 | Classless verbs (269 rows, `aliarvioida` = `ali` + `arvioida`) have no paradigm | `inflect` refuses them; `check` falls back to compound analysis | 4 |
| 6 | `analyze` has no `Phrase` result and no ambiguity result | `iso talo` is left to the tokeniser, and a two-way split is silently resolved to the first | 5 |
| 7 | No `explain` (which component changed, and why) | the wasm demo and the CLI can print forms but not the analysis | 1 |

## Phases

### Phase 1 — say what the planner already knows

Add to `src/compound.rs`, without changing any generated form:

- `Component.inflection: ComponentInflection` with `Invariant`, `Inflect`,
  `Lexical` and `Conditional` (the spec's four modes, not a bool). A modifier
  resolved through a *case form* is `Lexical`; a modifier that is a bare lemma
  is `Invariant`; the head is `Inflect`; a coordinated first half is
  `Conditional`.
- `Compound.kind: CompoundKind` with `Ordinary`, `GenitiveInitial`,
  `Coordinated`, `Lexicalised`. Classification uses the resolved surface vs the
  lemma (genitive `talon` vs `talo`), the hyphen rule (`PairedExpression`), and
  the sanalista marker (50/51) when the row has one.
- `explain(word, builder) -> Explanation` returning each component's surface,
  lemma, mode and — where the word is an inflected form — the action taken.

The class-50/51 planner is then rewritten to read `kind`/`inflection` instead of
re-deriving them. The regression net is the existing `expand::tests` compound
tests plus the class-50/51 report: **the forms must be identical before and
after**, which is checkable by diffing `inflect` output over the 93 class-51
lemmas and a sample of class-50 rows.

### Phase 2 — ordinary compounds as first-class

A row with no class is a compound, and `compound_head_forms` already inflects it
by its head (modifier fixed, every form `Repaired`). Two things remain:

- **Promote the stated cases.** When the sanalista row is classless *and* the
  head split is the only one, the forms are as plain as any other dictionary
  word; `Repaired` was the safe default while the path was new. Promoting needs
  evidence, not a hunch: the classless rows are outside the class report, so
  the check is a sampled run of `inflect` against the live oracle
  (`tools/check_word_against_omorfi.py`), recorded in the plan before the
  change.
- **Cover the report.** Extend `report_by_class.py` with a synthetic
  `(none)` class for classless nominals so their recall is measured like every
  other class. This is the only way the 56 486 rows stop being invisible.

### Phase 3 — genitive-initial and coordinated compounds

- Record the modifier's case (`Component.case`) and mark it `Lexical`. The
  planner keeps the surface, which it already does; the gain is that `explain`
  can justify it and `check` can reject a *different* case in the same slot
  (`talonmies` yes, `talommies` no).
- For a `PairedExpression` hyphen (`suomalais-ruotsalainen`), inflect both
  halves in the same cell and join with the hyphen, but only when both halves
  are independently inflectable and the pair is lexicalised. The spec's warning
  is right: the hyphen alone is not the signal; `linja-auto` is one head, not a
  pair.
- Fixtures: the spec's own examples plus the sanalista's hyphenated rows
  (`nuori-isäntä` is class 51 and already covered).

### Phase 4 — classless verbs

`aliarvioida`, `aivopestä`, `alijäähtyä` (269 rows) are `prefix + verb`. The
tail is a verb lemma with a verb class, so the paradigm is the tail's with the
prefix fixed — the verb counterpart of `compound_head_forms`, not class 50.
Implement as a classless-verb branch in `inflect_all`, `Repaired` like the
nominal one, and measure it with a sampled `check_word_against_omorfi.py` run.

### Phase 5 — phrases and ambiguity

- `Analysis::Phrase(Vec<Component>)` for input containing whitespace, built on
  `tokenise`; `iso talo` is a phrase, not a compound. No inflection planner for
  phrases: each word inflects independently, which is what the tokeniser
  already enables.
- `Analysis::Ambiguous(Vec<Compound>)` when more than one split survives the
  same longest-prefix rule. Today the first wins silently. Returning candidates
  is the spec's "structured uncertainty"; the callers (`check`, `suggest`) can
  keep taking the first and say so.

## Testing

The repo's guard is data-driven, not a hand-written corpus: every catalogue form
of every class member (`tests/catalogue.rs`), the harness oracle, and the
`GAPS` list with staleness checks. This plan keeps that and adds:

- form-for-form diffs around each refactor (Phase 1 must be a pure refactor);
- a Wiktionary-sourced fixture for each compound shape the spec names
  (`kirjakauppa`, `viikonloppu`, `talonmies`, `suomalais-ruotsalainen`,
  `linja-auto`, `isotaivot`, `pikkuaivot`, `särkynytsydän`);
- the harness samples (n=5/20/100/full) and the evidence gate after every
  phase.

## What this plan does not do

- It does not integrate Omorfi, Voikko or UralicNLP at runtime, and does not
  copy their code. Their output is evidence, cited in
  `data/oracle_exceptions.json` or in a note here.
- It does not add a statistical or ML component. The morphology stays
  dictionary- and rule-driven.
- It does not claim the spec's accuracy targets as results. They are targets;
  the measured numbers are the harness report, and they move one class at a
  time.
