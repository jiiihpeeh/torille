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

### Phase 1 — say what the analyzer already knows  *(done 2026-11-02)*

`src/compound.rs` now carries the distribution model, and no generated form
changed:

- `Component.inflection: ComponentInflection` with `Invariant`, `Inflect`,
  `Lexical` and `Conditional` — the spec's four modes, not a bool. A modifier
  resolved through a case form is `Lexical`; a bare lemma modifier is
  `Invariant`; the head is `Inflect`; the first part of a class-51 or
  coordinated compound is `Conditional`.
- `Component.case` records the case a frozen modifier is in (`talon` is
  genitive), so `talonmies` can be told from a typo.
- `Compound.kind: CompoundKind` with `Ordinary`, `GenitiveInitial`,
  `BothParts`, `Lexicalised`. `BothParts` covers class 51 (`nuoripari`) and
  the coordinated hyphen pair (`suomalais-ruotsalainen`); the hyphen is one
  route to it, not the definition.
- `explain(word, builder) -> Explanation` returns each component's surface,
  lemma, mode, case and `Action` (`Unchanged`, `Inflected`, `Frozen`).
  `check` prints it: `talonmiehen: compound talo (frozen genitive) + mies
  (inflected) [genitive-initial]`.

Two decomposition defects surfaced while writing the tests and are fixed:

- the recursive split scanned boundaries left to right and returned the first
  success, so a short prefix that is a word of its own won: `nuoriparin` was
  `nuo` + `ripa` + `ry`, `isotaivot` was `iso` + `taivo`, `särkynytsydän` was
  `särky` + `nyt` + `sydän`. It now scans longest-modifier first.
- the base named for a head was whichever the index reached first, which is
  the sanalista's order. `FormBuilder::best_base` ranks by `ClassPrior` and
  then by how much of the surface the base covers, so `parin` is `pari` before
  the golf term `par`.

**Not done, deliberately:** `expand/compound.rs` was not rewritten to consume
`kind`/`inflection`. It starts from a sanalista row and a class marker and
works in `Lemma` rows and case cells; the analyzer starts from an orthographic
word and works in strings. They meet at `check`/`explain`, and rebuilding the
planner on the analyzer is Phase 2's job, when classless compounds become
first-class. Forcing it now would be a refactor with no behaviour to show.

The regression net was the existing `expand::tests` compound tests plus the
class-50/51 report; the forms are unchanged. `check` is the new consumer, and
`a_genitive_modifier_is_recorded_as_frozen`,
`a_class_51_word_inflects_its_first_part`,
`a_coordinated_pair_inflects_both_parts_without_a_class`,
`explain_separates_a_frozen_modifier_from_an_inflected_head` and
`the_best_base_prefers_the_reading_that_covers_the_surface` pin the model.

**Known limit:** `kind` is exact for the citation form. For an inflected form
of a class-51 compound whose head is a plural (`pitkienhousujen`), the citation
is reconstructed from the lemmas (`pitkähousut`) and the plural `-t` is lost,
so it reads as `genitive-initial`. The inflector is unaffected; this is an
`explain` limitation until the ambiguity work in Phase 5.

### Phase 2 — ordinary compounds as first-class

A row with no class is a compound, and `compound_head_forms` already inflects it
by its head (modifier fixed, every form `Repaired`). **Covered (2026-11-02):**
`report_by_class.py --classless` audits the rows under a synthetic `(none)`
class, so the 56 486 rows are no longer invisible. The measurement on a
100-lemma sample (96 checked, 4 skipped):

| status | count | what |
|---|---:|---|
| `not_plain` | 14,286 | the engine produces every confirmed form, as `Repaired` |
| `missing` | 442 | three lemmas with no engine paradigm |
| `illegal` | 0 | not measurable while the forms are not plain |

So the split is right for 97% of the confirmed cells: the forms match the
oracle, they are just not offered. The 442 missing are three shapes, not a
general failure:

- `kotimaanmatka` — the modifier is the genitive of a *classless* compound
  (`kotimaa`), and the reverse index holds only its citation form, so
  `kotimaan` is not a known form and the split fails. Fixing it means indexing
  classless forms (or resolving the modifier recursively), which is the
  reverse-index decision below.
- `leijupakastus` — the head is a derived noun (`pakastaa` + `-us`), and
  `compound_head_forms` needs a head lemma.
- `kos-salaatti` — a hyphen the ordinary split does not handle.

**Promotion was tried and is blocked.** Changing `compound_head_forms` to
`Plain` turned the 14,286 into `confirmed` but first added **246 `illegal`**
and 586 `unsupported`. 241 of the 246 were one lemma, `happiradikaali`, and
they were adjective **degree** forms (`happiradikaaleimmittaan`) on a compound
that is a *noun*: `inflect_all` added the head's degrees whatever part of
speech the caller asked under, and the compound inherited them.

**Fixed (2026-11-02):** `compound_head_forms` now runs the head's paradigms
only (`inflect_paradigms`) for a nominal compound, and `inflect_all` for an
adjective compound, so `happiradikaali` has 179 forms and no degree, while
`tummanpunainen` keeps `tummanpunaisempi`. Pinned by
`a_noun_compound_does_not_take_the_heads_degrees`. Re-measured with `Plain`,
the sample is **14,234 `confirmed`, 442 `missing`, 5 `illegal`**.

The five are all the engine's over-complete `-in` genitive plural
(`heinäväkein`, `lankkipoikain`, `pyöränjälkein`, `terveysoppein`,
`keuhkoputkein`). Wiktionary gives `väki` as `väkien` with no `-in` variant, so
these are not rare forms the oracle missed; they are the engine's variant
generator. Promoting before that is pruned would make the engine offer
non-words as corrections, so the forms stay `Repaired`.

The order that unblocks promotion:

1. ~~stop `inflect_all` adding degree forms when the caller asks for a noun~~
   (done);
2. prune the `-in` genitive plural variant where the class does not license it
   (the remaining blocker; five rows in the n=100 sample);
3. index classless forms, or resolve a classless modifier recursively, for the
   nested compounds (`kotimaanmatka`), and handle derived heads
   (`leijupakastus`) and the odd hyphen (`kos-salaatti`) for the 442 missing;
4. then promote and re-run `--classless` at n=100 and full; the target is
   `illegal` at 0 with `missing` no worse.

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
