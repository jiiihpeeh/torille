# The verification harness

`torille` is judged against two external authorities, never against itself.
This is what the harness does, why it is built the way it is, and how to read
what it reports.

## The goal

The engine's output is a **list of forms a spell checker may offer as
corrections**. That framing decides the shape of the test. A coverage test asks
"does the engine produce the right forms" and a coverage test alone is not
enough, because it says nothing about the forms the engine invents. So the
harness checks both directions and treats them as equally important:

| direction | question | the bug it finds |
|---|---|---|
| **recall** | is every real form offered as `plain`? | the checker rejects a word that exists. `jokina` is a word — Voikko reads it as `joki` — and the engine produces it only as `guessed`, so it is never suggested. On the same lemma `jokinansa` and `jokinaani` are not produced at all: there is no essive plural for the engine to hang a possessive off. |
| **precision** | is every `plain` form real? | the checker tells the user to write a word that does not exist. `kevydsoratta` is offered as `plain` where the right word is `kevytsoratta`, so it is a "correction" away from correct. |

The second direction is the one that is easy to leave out and expensive to
ship, because everything about a coverage test points away from it: coverage
tests compare against a list you built, and the natural extension of that is a
bigger list, not an assertion that the list is complete.

Both scripts exit non-zero if either direction fails.

## The oracle

Two libraries, and they answer different questions. Neither alone is
authoritative.

**Omorfi** decides which forms *exist*. It is asked one cell at a time and
answers with the surface string, or with the empty string when the cell is not a
word:

```python
omorfi.generate("[WORD_ID=kissa][UPOS=NOUN][NUM=SG][CASE=NOM]")
# 'kissa'
omorfi.generate("[WORD_ID=kissa][UPOS=NOUN][NUM=PL][CASE=INS][POSS=SG1]")
# ''  -- an instructive possessive is not a word
```

The feature string wants a *complete* vector, and the **omos** names, not the
UD names. `ufeats` reports `Number` and `Poss`; the omor string wants `NUM` and
`POSS`. A feature string naming a field nobody has parses without complaint and
answers with the empty string for every cell, which reads as a quiet run that
confirms almost nothing.

**Voikko** decides which of those answers are *real*. Its dictionary is finite,
so a form Omorfi generates that Voikko does not recognise as belonging to the
lemma is not authoritative and is dropped. Voikko confirms everything Omorfi
generates, and it is the *better* of the two on possessives — it knows
`kissani`, `kissaksemme`, `taloillani`, `taloihini`, `taloani` and
`taloihimme`, all of which are real words. So it cannot be used alone as a
coverage oracle either: it will not tell you what forms exist, only which of
the ones you thought of are real.

The corollary matters when reading a report. A cell neither oracle answers is
a cell nobody can judge, and the harness says so (`unsupported`) rather than
guessing — see [the finding kinds](#the-finding-kinds).

What survives both is the word's actual paradigm, and that set is what torille
is compared against.

That same two-library verdict is materialised once for the whole noun-like
lexicon in `data/kotus_nomines.sqlite` — see [the noun
lexicon](lexicon.md). `ok='both'` there *is* this confirmed set, so the noun
half of the harness can read the reference with a query instead of a live
Omorfi/Voikko load. The live path below stays for verbs and for words the
lexicon does not cover.

**One cell can have two right answers.** Omorfi answers the partitive possessive
of `talo` as `taloaan/taloansa` and the translative as `talokseen/taloksensa`.
Voikko accepts both spellings, and so does Finnish: `taloaan` and `taloani` are
both words. The harness keeps both, which is the right call for a spell checker
— the user may have meant either. It is also why a report row's `expected` is
sometimes a `/`-separated list, and why the report's unit is the **form** and
not the cell: `kissani` answers both NOM/SG/SG1 and GEN/SG/SG1, so a row per
cell would count one correct word twice and make every rate wrong.

## The grid

The grid is the whole paradigm, **possessives included**:

```python
noun_cells(ALL_CASES, ["SG", "PL"])        # 13 x 2 x 6 = 156 cells
noun_cells(ALL_CASES, ["SG", "PL"], [""]) # 13 x 2 x 1 =  26 cells, with --no-possessives
```

The possessive is a cell dimension rather than decoration on a form, because
for this consumer it is exactly that. `kissani` is a word; if the grid stops at
the bare case/number cells then a harness checking 26 cells examines 25 of the
121 `plain` forms `kissa` offers and calls the word clean. An early version of
this harness exempted possessives for precisely that reason and was quietly
checking a quarter of the output.

Verb cells are (tense, mood, person, number) instead, from
`VERB_TENSE_MOODS`.

## Scope

torille emits forms the grid does not contain. Counting any of them as a
precision failure would fail every word for the wrong reason:

* for verbs, the negative (`ei laske`), the potential, the perfect compound;
* for nouns, the comitative (`kissoineen`), which is a productive
  `instructive` + `abessive` pair rather than a case;
* the clitics (`kis-sako`, `kissahan`), which are a productive suffix.

`Harness.in_scope` decides whether a reading is inside the grid at all, and it
is **permissive**: a field the reading does not carry is a wildcard, not a
mismatch. That matters more than it sounds. `kissani` is tagged
`noun singular poss 1sg` with **no case**, because the possessive nominative
and the possessive genitive are the same word. A strict scope test would skip
it — and the one bug the possessive grid was added to find is exactly
`kissaksini`, so a scope test that skips `kissani` is a scope test that cannot
see the class of defect it was written for.

`Harness.acceptable` is the strict one, and it judges a *tag* rather than a
scope: every field of the reading has to be present and match. That is what
makes `kissani` a reported mis-tag rather than an unasked-about form. 58 of 96
mis-tags are exactly a missing case tag.

## The finding kinds

They are kept apart, because they mean different things and conflating them
hides the interesting one.

| kind | direction | meaning |
|---|---|---|
| `confirmed` | — | a confirmed form torille offers as `plain`, correctly tagged |
| `missing` | recall | a confirmed form torille does not produce at all |
| `not_plain` | recall | a confirmed form it produces but does not offer as `plain` |
| `mistagged` | tag | a confirmed form offered as `plain` but filed under the wrong cell |
| `illegal` | precision | a form it offers as `plain` where the oracle confirmed a **different** word for that cell |
| `unsupported` | precision | a form it offers as `plain` that **neither** oracle vouches for, in either direction |

The two oracles disagree about coverage, and the disagreement is not symmetric.
Omorfi is the coverage authority and is sometimes simply missing a cell: it has
no abessive plural for `eukonkanto`, though the word is there and Voikko reads
`eukonkannotta` as a form of it. When Voikko says a form is real and no cell
generated it, the finding is a **gap in the grid**, not in the engine, and the
row is reported as `confirmed` with a note.

So the precision direction asks its questions in this order, and the order is
the whole content of it:

1. **Does Voikko read this as a form of the lemma?** Then it is one, whatever
   Omorfi has for the cell. The finding is about Omorfi, not about torille.
2. **Did the oracle confirm a *different* word for the cell this reading claims?**
   Then the engine is proved wrong, and the row names the string that should
   have come out. `illegal`.
3. **Otherwise nothing vouches either way.** `unsupported`.

The distinction between the last two is the difference between a bug report and
a guess. `allergikkooihin` against the confirmed `allergikoihin` is `illegal`:
the oracle said so, the engine is wrong, and the fix is known. `buffetnsa` is
`unsupported`: Omorfi generates it for `buffet` and Voikko does not recognise
it, so neither authority has an opinion, and the most that can be said is that
the engine offers a form nobody will vouch for. Conflating them overstated
class 10 by more than a third, and a lemma Omorfi had never heard of at all —
`iso` — was reported as 50-odd engine defects, which put 3596 false rows at the
top of the triage table.

Both still fail the class. An unbacked `plain` suggestion is a suggestion the
checker would offer, and the consumer of `inflect` cannot tell the difference
between a proven-wrong form and an unverifiable one.

Recall is deliberately **not** tag-scoped. Scoping it turned an
offered-but-mistagged form into a false "missing", which is worse than a
duplicate: the form exists in the output, the harness just said the engine
never made it. Only the precision direction is scoped.

## The two silent failures

Both of these cost more time to find than any other problem in the harness, and
both are worth knowing about before changing anything here.

### The possessive is spelled differently on the two sides

`POSS=SG1` in the omor string, `poss 1sg` in torille's reading. Only the
third-person one, `3`, is the same in both. Using torille's spelling yields an
omor string that parses, answers empty for every cell, and reports **63
confirmed forms instead of 156 with no error anywhere**. The 88 that survived
were exactly the third-person possessives.

`POSSESSIVE_NAMES_MAP` in `tools/omorfi_harness.py` is the only place the two
spellings meet.

### `instructive` is both a case and a mood

`verb instructive plural past instructive active` — a case *and* a mood, in one
tag list. Two recovery strategies were tried against the `inflect` binary's
printed output and both are wrong in a way that produces no error:

* **positional parsing** breaks on absent fields. `ei tehdä` has no mood, so
  `passive` and `negative` get shifted into the wrong fields and every later
  field is off by one;
* **vocabulary lookup alone** breaks on `instructive`, which is in the case
  vocabulary and the mood vocabulary and so is ambiguous.

Only an **ordered greedy walk** that skips absent fields gets all 5300 forms
right. This is why the binding exists: it returns named fields, so the question
never arises. The fallback walk is only there for when the binding is not
built, and `tools/test_binding_agrees_with_binary.py` pins the two together.

## The tools

All of them live in `tools/` and share `omorfi_harness.py`.

```
python3 tools/check_word_against_omorfi.py kissa [talo ...]
```
The strict per-word test, both directions, exits non-zero on any failure.
`--pos noun|verb`, `--cases GEN,PAR`, `--numbers sg|pl|both`,
`--no-possessives`, `--lenient`, `--strict-tags`, `--quiet`.

```
python3 tools/run_wordlist.py [LEMMA ...] [--file F] [--sanalista] [--random N]
```
The same test over a wordlist. The selection is the point: a full wordlist is
tens of thousands of lemmas and the interesting part is a slice of it.
`--list` prints what would run and stops, so a selection can be checked before
spending the time. `-j` checks that many words at once.

```
python3 tools/report_by_class.py [-n 5] [-c 9,9B,27] [--bare] [--split DIR]
```
Per inflection class, as a JSON array. The unit is the class as torille reads
it, so `9` and `9*B` stay separate — the `*B` is a consonant gradation, and
lumping them would report every `kauppa` form as a defect of class 9. Exits 1
if any class has a defect.

```
python3 tools/summarize_class_report.py reports/noun-classes.json
```
Triage. Groups findings on their shape rather than their word, so
`allergikkooihin` → `allergikoihin` is one defect across a hundred classes
rather than a hundred defects, and the before/after columns are the two strings
to choose between in the code.

```
python3 tools/test_binding_agrees_with_binary.py
```
21 lemmas, ~5300 forms, the same word, confidence and reading through the
binding and through the binary. Needs the binding built.

Two older scripts remain as cheap coverage smoke tests and are not part of the
strict check: `check_nouns_with_omorfi.py` (740 confirmed, 0 missing, 41
generated but not offered) and `check_verbs_with_voikko.py` (933 plain, 0
unrecognised). `check_nouns_with_voikko.py` is the noun counterpart and is
**not a gate** — it exits 1, and did so identically before the possessive work:
171 core-case forms unrecognised, all bare, none carrying a possessive, the same
per-lemma counts at both commits. They are the pre-existing fixed-stem and
gradation defects below.

## Skips

A wordlist run is thousands of lemmas and no single awkward word may end it, so
several conditions make a lemma a skip rather than a finding. Each is recorded
with its reason.

| skip | why |
|---|---|
| `no class in the sanalista` | a compound or an indeclinable has no paradigm of its own. This is the exact test — not a heuristic. |
| `compound` | kept because it catches 120 compounds the class test does not, such as `hepokatti` and `makrilli`. A heuristic: Finnish joins with a linking consonant, so no split of `aamunsarastus` puts a listed word on either side of the `n`. |
| `not inflectable` | the sanalista classes it but `inflect` raises. `aamunsarastus` did this until `has_class` was added. |
| `no confirmed form` | **Omorfi does not know the word.** `iso` is in the Kotus sanalista and inflects perfectly, but it is not in Omorfi's dictionary, so every cell comes back empty. There is no oracle here to disagree with, and without this skip the precision pass reads every `plain` form torille offers as a defect — a whole lemma's worth of findings that say only "Omorfi has never heard of it". Four classes were reporting nothing else. |

## Cost

Both oracle halves are loaded **once per process** and every call is
serialised on a lock, which is what makes a wordlist run affordable.

* **Omorfi** costs about 7.5s to load and a generate call is 15µs. It is loaded
  once and shared; an earlier harness built one per thread, so `-j 8` paid the
  7.5s eight times over. That, not the morphology, was most of a wordlist run.
* **Voikko** is a C++ object with no promise of concurrent use, so the same
  applies.
* **torille** is reached through the extension module, which holds the Kotus
  data in one place instead of reparsing the catalogue and the sanalista for
  every lemma: 0.22ms per word against the binary's 54ms. Without the binding
  the harness falls back to the `inflect` binary — correct, and about 245×
  slower. `TORILLE_NO_EXTENSION=1` forces the fallback, so the two paths can be
  compared rather than assumed to agree.

A 40-word `-j 8` run takes 11.2s, of which 7.5s is the one-time Omorfi load.

## Known defects

Found by this harness. Regenerate the evidence with:

```
python3 tools/report_by_class.py -n 5 -j 8 --split reports/classes
python3 tools/summarize_class_report.py reports/noun-classes.json
```

The numbers are from a 5-lemma sample of each of the 146 noun classes.
Current totals: **96,216 confirmed, 8,679 missing, 539 illegal, 0 mistagged,
722 unsupported, 0 not plain.** Recall — confirmed over confirmed plus missing
— is **91.7%**, and the `illegal` rate is **0.6%** of confirmed. Ranking the
classes by `illegal` count would be useless here (the largest classes are
simply large), so the class table below is ranked by *rate*.

**Every example was checked against Voikko by hand before it was written down**,
and two candidates that looked real in the report were not and are absent here:
`jakajaa` is the correct partitive singular of `jakaja` and the engine gets it
right, and `eukonkannon` is the correct genitive of `eukonkanto` — Omorfi is the
one that says `eukonkandon` there, and Voikko sides with the engine.

### Fixed in the possessive round

The first sweep found the possessive paradigm half-built, and the diagnosis was
that **one question — which cells get a possessive — was being answered by two
code paths that disagreed.** The gated site in the slot loop took three of the
eight catalogue slots (`Slot::Nom | Slot::Gen | Slot::Ill`, all three singular)
and the ungated site over `oblique_forms` covered whatever `oblique()` happens
to build. Neither served `Slot::Part`, `Slot::PlPart`, `Slot::PlGen` or
`Slot::PlIll`, which is why no class had fewer than 18 of its 26 cells. Four
things fell out of settling it:

* **Coverage.** All seven slots that take a possessive now do, the plural
  nominative excepted — the possessive hangs off the nominative or the genitive
  singular, the two spellings coincide there, and `talotani` is not a word.
* **The partitive takes the suffix as it stands.** `lutkutusta` is
  `lutkutustani` and `taloa` is `taloani`; the partitive ending sits in front of
  the suffix rather than being displaced by it. This was not what the first
  attempt assumed — it stripped the trailing `a` — and the oracle settled it
  within one sample.
* **The translative spells its elided `i` out as an `e`.** `taloksi` is
  `talokseni`, not the `taloksni` the elision would leave. 3380 forms over 117
  classes were wrong here and all 3380 are now right.
* **The third person has two spellings and Finnish uses both.** A doubled final
  vowel and the bare `-nsa`: `taloa` is both `taloaan` and `taloansa`, and every
  oblique case has the same pair. Only one was built.

And two subtractions, from the same ungated site: the **instructive and the
comitative take no possessive at all** — `taloin` is a way of being and
`taloineen` is a company, and neither belongs to anyone, so `taloini` and
`taloineeni` are not words and 1190 forms went away. That also removed the
`kissoinsa`/`taloinsa` family, which was the instructive possessive misread as
the third person.

One judgement call is worth recording because it is the one change that touches
the suggestion set rather than only the coverage. Dropping a case `n` before the
suffix (`talon` → `taloni`, `taloihin` → `taloihini`) was marked `Repaired`, on
the reasoning that the lemma had been altered to fit the ending. It had not: the
stem is untouched and only the case ending goes, which is what `Repaired` — a
gradation applied, a vowel epenthesised, an alternation read off the genitive —
is not. `talooni`, one of the commonest possessives in the language, was
unsuggestible because of it.

| | before | after |
|---|---|---|
| confirmed | 32613 | **49685** |
| missing | 38225 | **21807** |
| not plain | 2402 | **0** |
| unsupported | 2512 | **1341** |
| illegal | 15390 | 15431 |
| mistagged | 2377 | 3348 |

`illegal` moved 41 against 17072 gained, which is the price of the third
person's doubled vowel: Voikko's lexicon does not have every instance of it, so
the doubling shape produces a small number of forms it will not confirm
(`kalaaan` is not a word, `kissaan` is). The `-nsa` shape covers 272 of 329
third-person cells on its own and the doubling covers 276, and neither alone
covers the union.

### A. An oblique form that is not `Plain` gets no possessive

The four open defects are lettered in order of **gap size**, which is not the
order of user impact — C is the only one that makes the engine *wrong* rather
than incomplete, and `TODO.md` puts it first for that reason. A gates B.

**14026 missing forms across 101 classes** — the largest remaining gap, and most
of it is not a possessive bug at all. `add_possessives` hangs a possessive off
every `Plain` oblique and skips the rest, so an oblique the engine is unsure of
takes its whole possessive grid with it. The sample splits cleanly:

| of 1659 (lemma, case) pairs with a missing oblique possessive | |
|---|---|
| the bare oblique plural is produced but not `Plain` | 1092 |
| the bare oblique plural is `Plain` but built on the wrong stem | 495 |
| no bare oblique plural at all | 72 |

```
oikku    ABE/PL   bare oikuitta is guessed, so oikuittaan is never built
lovi     ABL/SG   bare loelta is guessed, so loveltani is never built
noki     ESS/PL   no essive plural at all, so nokinansa is never built
```

`lovi` is the interesting one: its plural obliques are `Plain` and its singular
ones are not, and the correct Finnish has both on different stems (`lovessa` from
the weak `love`, `lovilta` from the strong `lovi`). The `demote_contested` rule
that a previous round added is doing this, so the fix is in
`src/grammar/noun.rs` and not in the possessive code.

The 495 is the plural-stem defect below, seen from the other side: `allergikko`
has `allergikkoina` `Plain` as its essive plural and Omorfi wants
`allergikoinaan` as well, because the class also has a devoiced `allergiko`
plural stem.

### B. One spelling per slot, and which one

**4359 missing forms across 92 classes**: GEN 1962 over 82 classes, ILL 2397
over 86, PAR 1117 over 60, NOM 740 over 54. The slot loop takes the *first*
`Plain` spelling the alignment produced for the slot (`.take(1)`) and hangs the
possessive off that alone. When a class legitimately has more than one, the
others are real words that are never offered:

```
tarjoilu   GEN/PL   plain: tarjoiluiden, tarjoiluitten, tarjoilujen
                    offered: tarjoilujeni only
                    wanted: tarjoiluitteni, tarjoiluideni, tarjoilujeni
```

Deleting `.take(1)` was tried and **measured to be a net loss**: over ten
classes (1, 2, 5, 9, 10, 13, 27, 33, 40, 41) missing fell 1241 → 1078 but
illegal rose 385 → 596. The cause is specific, and it is why the obvious fix
does not work. Some plain slot spellings are removed by the demotion passes
*after* the possessives are built, and the possessives survive the removal as
orphans. `valtimo`'s illative slot holds `valtimoon` together with `valtimoo` and
`valtimoom`; the last two never appear in the output at all, but with the filter
gone `valtimoommemme` does, and it is a non-word. The fix is therefore an
ordering change — build the possessives after the passes that can remove their
base — not a filter on the spellings. The per-class trade is not uniform either:
`33` and `41` gain (missing 30 → 15 and 164 → 149) with no precision cost at
all, while class `1` loses 100 `illegal` and gains nothing, which is why an
average over the ten says more about class 1 than about the idea.

### C. Whole classes built on the wrong stem

**539 `illegal` forms in the current sample**, down from 15,431 after the
class-engine rounds. Every one is still `Plain`, which is what makes this the
item with the worst user impact: these are offered as corrections. The
remaining defects are clustered:

| rate | class | confirmed | illegal | note |
|---|---|---|---|---|
| 23.8% | `33*D` | 319 | 76 | plural-only `-imet`, oracle caveat |
| 22.5% | `51` | 628 | 141 | compound oracle disagrees on head/tail |
| 13.8% | `41*I` | 637 | 88 | plural-only `-aat`, oracle caveat |
| 13.5% | `41*C` | 651 | 88 | plural-only `-aat`, oracle caveat |
| 12.1% | `33*J` | 627 | 76 | plural-only `-imet`, oracle caveat |
| 3.1% | `34*C` | 795 | 25 | nominative possessive oracle |
| 1.2% | `18` | 582 | 7 | `pii` illative/nominative mismatch |
| 0.6% | `41*E` | 806 | 5 | `havas` dual-class stem |

The plural-only `33*D`/`33*J` and `41*C`/`41*I` rows are the documented caveat:
the SQLite oracle marks the longer spellings `both` and the short
Wiktionary/Kielitoimisto spellings `omorfi` (`antimet` -> `antimissa`/`antimitta`
not `antimemissa`; `valjaat` -> `valjaissa`/`valjaitta` not `valjaaissa`). Do not
"fix" those against the oracle. `34*C` and `51` need the same external check;
their expected forms look like oracle artifacts rather than engine errors
(`liittoutumattomani` is the normal possessive, not `liittoutumatonni`).

Recent rounds removed the singular translative possessive columns that hung off
the wrong stem in classes `24`, `31`, and `36` — `kuorikseen` for
`kuorekseen`, `haaksikseen` for `haahdekseen`, `vanhimmikseen` for
`vanhimmakseen`. They also put the class-5 native twin's nominative and
genitive possessives on the e-stem (`vyyhteni`, `kuoreni`, `appeni`), which
clears `5*B`, `5*F`, `7*B`, `7*F`, `24`, `26`, `31`, and `36` in the sampled
sweep. Most of what remains is oracle disagreement: live Voikko accepts the
engine's `33*D`/`33*J`, `41*C`/`41*I`, `49*D`/`49*E`, `51`, and `32*D` forms
and rejects the oracle's expected spellings, so do not change those classes
against SQLite alone.

Class 27 (`käsi`) is no longer in the `illegal` table: the fixed-stem round and
the class-27 engine removed the sampled `käittä`/`käettä` claims. Its remaining
gap is coverage, not precision.

### D. `taloni` has no case

**3348 `mistagged` across 103 classes**, and it grew with the possessive work
because more possessives exist to be mistagged. `taloni` genuinely is two words:
torille gives it both a nominative singular and a genitive singular reading,
`Form::union` keeps a field only where the readings agree, and the case is
therefore absent. The harness's strict `acceptable` requires every field to
match, so these are reported — which is what the harness documents it is for,
but whether a case-less reading of a form that has two cases is a defect or a
report that is too strict is a question about the harness, not the engine.

### Bare case gaps

**1565 missing forms across 65 classes**, and small enough to list by case:
ELA 165, INE 165, ABL 164, ADE 163, ALL 160, TRA 160, ABE 157 (51–55 classes
each), ESS 125, GEN 113, ILL 61, and then INS 51, PAR 42, NOM 39. The essive
gaps split about evenly between the two numbers (64 singular, 61 plural), so
this is not one missing cell: `joki` has `jokena` but no essive plural, and
`jokinansa` and `jokinaani` are missing because of it.

Ranked by class the leaders are `49*J` (108 forms), `9*D` (93), `51` (92) and
`50` (83) — the same classes as defect C, which is the point: a class built on
the wrong stem misses cases *and* offers wrong ones, and fixing the stem fixes
both counts at once.

An ordering constraint is worth stating, because getting it wrong wastes a fix:
**A gates B**. B needs the possessive construction moved after the demotion
passes, and A is those passes.

## Setup

```
pip install pyomorfi voikko-libvoikko
(cd bindings && env -u CONDA_PREFIX maturin develop --release)
```

`env -u CONDA_PREFIX` is needed because maturin refuses to run when both
`VIRTUAL_ENV` and `CONDA_PREFIX` are set.

The binding is **optional**. `tools/omorfi_harness.py` imports `Engine` lazily
inside `_load_engine()` rather than at module scope, because a top-level import
of a module that may not be built would take the binary fallback down with it.
Everything above works without it, slowly.
