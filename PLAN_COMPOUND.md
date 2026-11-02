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

### Phase 2 — ordinary compounds as first-class  *(done 2026-11-02)*

`report_by_class.py --classless` audits the rows under a synthetic `(none)`
class, so the 56 486 rows are no longer invisible. The first measurement on a
100-lemma sample (96 checked, 4 skipped) was 14,286 `not_plain` and 442
`missing`: the engine produced every confirmed form, but as `Repaired`.

Promotion to `Plain` was blocked twice, and the two fixes are different in kind:

- **The head's degrees.** `happiradikaali` is a noun, and its head `radikaali`
  is an adjective, so the compound inherited `happiradikaaleimmittaan`. Fixed
  by `inflect_paradigms`: a nominal compound takes the head's paradigms only,
  an adjective compound still takes the degrees (`tummanpunaisempi`).
- **The `-in` genitive plural.** The engine emits `heinäväkein` beside
  `heinäväkien`, and Wiktionary gives `väki` with no `-in` variant. The fix is
  not to prune the variant but to stop the prefix re-stamping the head's
  confidence: `compound_head_forms` gives each form the head's own confidence,
  so a plain head form makes a plain compound form and `väkein` stays
  `Repaired`. That is the right rule anyway — the modifier adds a prefix, not a
  repair.

Re-measured with the promotion in place:

| status | count | what |
|---|---:|---|
| `confirmed` | 14,407 | every oracle-confirmed cell of the sample |
| `missing` | 0 | — |
| `not_plain` | 269 | the head's repaired forms (`tahdinlyöntein`) |
| `illegal` | 0 | no plain form the oracle does not confirm |
| `unsupported` | 591 | plain forms whose cell the oracle has no row for |

`aamuhartaus` is 160 plain forms; `pikkuaivot` 89 plain + 3 repaired (its
`-in` variants); `happiradikaali` 160 plain + 19 repaired. The forms are plain
in `inflect` and the binding, but the reverse index still holds only the
citation form for a classless row, so `check_fast`/`suggest` do not see them —
the same limit as compound analysis in the fast path.

The three lemmas that were missing had three different causes, and all are
fixed:

- `kotimaanmatka` — the modifier is the genitive of a *classless* compound
  (`kotimaa`), which the compile-time form set does not hold.
  `classless_modifier_is_known` inflects the classless base and matches the
  surface. Pinned by `a_classless_modifier_resolves_through_its_own_compound`.
- `leijupakastus`, `kos-salaatti` — the modifier is a verb stem (`leiju-` of
  `leijua`, `kos-` of `kosia`), and `modifier_is_known` deliberately accepts
  only nominal stems. `verb_stem_is_a_modifier` checks the verb stem map and
  requires the base to be a verb lemma, which keeps a noun that happens to share
  the spelling out. The same probe is used on the verb-head path
  (`riippuliitää`).

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

### Phase 4 — classless verbs  *(done 2026-11-02)*

`aliarvioida`, `aivopestä`, `alijäähtyä` (269 rows) are `prefix + verb`. The
tail is a verb lemma with a verb class, so the paradigm is the tail's with the
prefix fixed — the verb counterpart of `compound_head_forms`, not class 50.

`compound_verb_forms` does that: it splits the word, keeps a head whose classes
are verb classes, and runs `inflect_paradigms` on the head. Two details matter:

- the negative is two words (`ei arvioi`), and the modifier belongs on the main
  verb (`ei aliarvioi`, never `aliei arvioi`);
- the modifier may be a case form the compile-time subset does not hold
  (`aikaan` is the illative of `aika`), so `verb_modifier_is_known` falls back
  to inflecting a lemma that is a prefix of the modifier. That widening is used
  only for a verb head, where the head constraint keeps it off nominal
  compounds.

All 269 inflect. Two modifier shapes needed more than the prefix probe:

- `puhtaaksikirjoittaa`, `puhtaaksimuurata` — `puhtaaksi` is the translative of
  `puhdas`, and the `d` -> `t` gradation means no lemma is a prefix of the
  surface. `verb_modifier_is_known` falls back to the engine's reverse lookup
  (`find_bases_with`), which handles gradation.
- `riippuliitää` — `riippu` is the stem of `riippua`, and `modifier_is_known`
  deliberately accepts only nominal stems. The verb path checks the verb stem
  map, where the head is a verb and the widening cannot reach a nominal
  compound.

Pinned by `a_classless_verb_inflects_by_its_verb_head`.

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
