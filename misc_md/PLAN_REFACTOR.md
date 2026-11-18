# Plan: refactor the nominal class engines onto the verb side's shape

## Goal

Give `src/expand/nominals/` the machinery `src/expand/verbs/` already has: a
`Decl` that states a class's stems and endings as data, a `decline` that walks
the case grid from it, and a `Builder` whose `push` takes the caller's `Tags`
the way `VerbBuilder::push` does.

Two things fall out of that, and only the first is a refactor:

- The **1 397 hand-written `b.cell(...)` calls** collapse into per-class
  declarations. The grid is already uniform; only the plumbing is duplicated.
- **`PartOfSpeech` stops being hard-coded to `Noun`**, which is a live defect
  rather than a style question. See *The defect* below.

## What is actually there

Measured, not estimated (`nominals/`, 209 modules, 1 476 933 bytes):

| | count |
|---|---|
| modules | 209 |
| thin delegators (`class_10A` → `class_10`) | 81 |
| modules that mention `Builder` | 52 |
| modules that call `b.cell(` | 51 |
| modules that call `build_i_loans(` | 5 |
| `b.cell(` call sites | 1 397 |
| `b.comitative(` / `b.push(` / `b.cell_rare(` / `b.push_rare(` / `b.possess(` | 54 / 68 / 26 / 20 / 5 |
| `b.h("a", "ä")` call sites | 1 585 |

**The grid is uniform and the duplication is not in it.** Across the 51 modules
that call `b.cell`, there are only **three distinct case grids**, and **49 of
them are the identical 24-cell grid** — 12 cases x singular/plural, every one.
The two outliers are `class_49` (23 cells) and `class_99` (1 cell). What differs
between the 49 is the *endings*: 12 distinct sets of `b.h` harmonic pairs, which
is correct, because that is the class data. What is duplicated 49 times over is
the structure that applies them.

So the noun side writes out by hand, 24 `b.cell` calls plus the instructive and
the comitative per module, something whose shape is fixed; and the verb side
states the same kind of thing once in `Decl` and drives it from `decline`.

## The defect

`nominals::Builder::push` builds its own reading:

```rust
let tags = match poss {
    Some(p) => Tags::of_number(case, num)
        .of_pos(PartOfSpeech::Noun)
        .possessed_by(p),
    None => Tags::of_number(case, num).of_pos(PartOfSpeech::Noun),
};
```

and `expand::mod::inflect_in` never passes `pos` to
`nominals::dispatch(&class, lemma, possessives)`. So **every** form a
hand-written nominal module emits is tagged `noun`, whatever the sanalista says
the lemma is. Verified against the binding:

```
aallokas        {'noun': 171}   -- sanalista: adjektiivi, class 41*A
aakkosellinen   {'noun': 165}   -- sanalista: adjektiivi, class 38
```

**5 913** lemma-rows whose `part_of_speech` is not `substantiivi` sit in an
authoritative nominal class (143 such classes), and the largest groups are class
38 (2 963 adjectives) and class 34\*C (562). `VerbBuilder` does not have this
problem because `push(word, tags)` makes the caller supply the reading.

The second, smaller divergence turned out not to be one, and the measurement
is worth keeping. `VerbBuilder::push` refuses a form under three characters;
the noun `Builder` has no such guard and emits **16** two-character forms
across the corpus — but every one of the 16 is a lemma's own nominative
singular (`se`, `yö`, `ja`, `mä`, `sä`, `mi`, `EU`, `DJ`, `UV`, `id`, `bi`,
`ha`, `hm`, `ma`, `sa`, `so`), and several are common words. `se` has 12
forms and `yö` 165; only their nominative is two characters. A blanket
three-character guard would delete the only nominative of `yö`, `se` and
`ja`. The verb guard exists because a two-character *verb form* is always a
fragment, not because short words are; the noun side has no inflected form
under three characters at all, so the guard has nothing legitimate to
remove. Step 7 is therefore a no-op with a test pinning it, not a change.

## Architecture

### Today

```
51 modules                          verbs/utility.rs
  b.cell(Case::Inessive,             Decl { 18 fields }  <- data
          Number::Singular,             |
          format!("{weak}{}", b.h(..)),  decline(d, tags) <- driver
          format!("{weak}{}", b.h(..)),
          true)
  ... 1 397 times, 49 the same grid
```

### After

```
49 modules                          nominals-refactored/utility.rs
  decline(&Decl {                      Decl { stems + harmonic endings }
      strong, weak,                      |
      ESSIVE => ("na", "nä"),           decline(d, tags) -> Builder
      INESSIVE => ("ssa", "ssä"),
      ...                                Builder::push(word, tags)
  })                                     Builder::push_rare(word, tags)
```

A noun `Decl` needs fewer fields than a verb `Decl`, because a noun's grid is
fixed where a participle's is not: stems (strong, weak, and the oblique base
each case takes), one harmonic ending pair per cell, the per-cell `double3`
flag, and the handful of cells whose base is not the stem plus its ending
(illative `stem + lastv` then `n`, genitive plural `stem + je`, translative
`stem + kse`, allative `stem + lle`, instructive, comitative). Those last ones
are the reason the verb `Decl` grew its `comitative_stem` / `ess_sg` /
`nom_base` overrides, and the noun one will want the same escape hatches.

## Wiring

`nominals-refactored/` starts as a byte-identical copy of `nominals/`. It is
declared as a **second module compiled beside the live one**, not a replacement
for it:

```rust
#[cfg(feature = "generated-nominals")]
pub mod nominals_refactored;
```

Two trees side by side means the refactor is checked by `cargo test` rather than
by a script somebody has to remember to run, and either tree can be diffed at
any moment. `expand::tests::the_two_trees_agree` compares them form for form
over every authoritative lemma. When the trees agree everywhere, the swap is a
one-line change to the `mod` declaration.

## Harness / verification

**The gate for a pure refactor is the engine's own output, not an oracle.**
A refactor that moves no form should be provable to move none, and there are
4.9M forms to be exact about:

```
tools/fingerprint_nouns.py      # per-lemma sha256 over
                                 # (word, pos, case, number, possessive, confidence)
29 316 lemmas, 4 911 677 forms, 19 s
```

Baseline captured at `e46c04d` (before any of this work):

```
ADHD  165  7cc615e7cbf53309225e119692dab3c167a206866972050d10b8abad913a4ced
ALV   165  ec54c8fb22d62eda1e69ab26d81c0ed458f06615f96bbbe3bf144d0b414ef862
```

Plus:

- `cargo test` — lib and integration, including the new equivalence test
- `python3 tools/check_nouns_with_omorfi.py` — 740 Voikko-confirmed forms, **0
  missing**, **0 generated but not offered** (the current README figures; the
  `41 generated but not offered` in `PLAN.md`'s outcome section is stale and
  should not be quoted as a baseline)
- `rustfmt --edition 2024 --config skip_children=true <file>` per touched file
  (never `mod.rs`: rustfmt follows `mod` declarations from it)

**What each gate can and cannot see.** The fingerprint reads the shipped
engine, so it sees only the live tree and all 29 316 lemmas; it cannot see the
refactored tree at all. The two-trees test sees both trees but only the fixture,
four words per class. Neither alone is enough, and the gap between them is
where the defects have been:

| defect | found by |
|---|---|
| `cell_rare` dropping the possessive tag (`a722fc6`) | equivalence test |
| class 18's `ILLATIVE` missing 5 abbreviations (`31d2a22`) | fingerprint — and only because `AMK` and `DJ` are *not* fixture words for class 18, so the two trees could and did disagree on the same commit and the equivalence test stayed green |
| classes 23/25's nominative possessives on the `i`-stem (`47f2cf9`) | equivalence test, naming all 17 fixture words the moment the live fix landed |

So the order matters: land the refactored port in the same commit as any live
fix the port would otherwise contradict, and read the fingerprint for every
commit even when the equivalence test is green.

## Sequencing — the one ordering constraint that matters

**The `pos` fix cannot live inside the byte-identical proof**, because it is
supposed to change output. So the work is in three separately-gated stages, and
each is committed on its own evidence:

| stage | change | gate |
|---|---|---|
| 1 | wire both trees, reshape `Builder` to `VerbBuilder`'s signature | fingerprint **byte-identical**; `the_two_trees_agree` green |
| 2 | add `Decl` + `decline`, port the 49 modules | fingerprint **byte-identical**; tests green |
| 3 | thread `pos` into `dispatch`, add the 3-char guard | fingerprint differs in **exactly** the rows it should |

Stage 3's diff is the interesting one, because it should be readable rather than
merely small: the expected movement is the 5 913 non-`substantiivi` lemma-rows
moving off `noun` onto their own part of speech, and the 16 two-character forms
disappearing. Any other movement is a bug, and the fingerprint makes it
impossible to miss.

## Steps

1. Declare `nominals_refactored` beside `nominals`; confirm it compiles and that
   `the_two_trees_agree` passes with the two trees identical. **Done.**
2. Add `tools/fingerprint_nouns.py` and record the baseline. Confirm the
   baseline reproduces (it must, being a pure read of current behaviour). **Done**,
   with the trap in the harness section below.
3. ~~Reshape `Builder::push` to take `Tags`~~ **Cut, see below.** It would have
   churned 52 modules' worth of call sites that step 4 then rewrites, so the
   `Tags`-taking `push` arrives with the `Decl` and the pilot, in one pass.
4. Add the noun `Decl` and `decline`, and port **one** class (`class_1`) as the
   pilot. **Done**, and it moved the design: see the risks section.
5. Port the remaining 46 modules on the shared 24-cell grid. `class_49` (23
   cells) and `class_99` (1 cell) are ported only if the `Decl` reaches them
   without an override that exists solely for them — otherwise they keep
   `b.cell` and the note says why. **Ported by family, through `83892c6`**:
   `1` + `1*A` (the pilot), `1*B`–`1*M` (the shared `gradated.rs` grid),
   `2`/`3`/`16H`/`18`, `23`/`25`, `19`/`20`/`24`, `11`/`13`, `10` (and
   `10*A`..`10*L`), `9` (and `9*A`..`9*K`), `7`, `8`, the shared
   `build_i_loans` grid — `5`'s plain half, `6` whole, and the shared half of
   `22`/`99` — `28_j` (the shared four-stem `inflect_base`, with `27`,
   `29` and `30`), and `32` (the shared two-stem `inflect_with_stems`, with
   `38`) plus `32_c` (a three-stem grid of its own), `34` (the shared
   `inflect_ton`, with `34_c`), `45` (the shared `ordinal`, with `46`), and
   `33` (the shared `inflect_graded`/`inflect_graded_nom_base`, with the
   `33_b`–`33_l` subtypes, `40` and `42`), `35_h`, `43`, `15`, `12`, `21`,
   `22`, `36`, `41`, `48` (and the `41*`/`48*` subtypes), `49` (the shared
   `inflect_with`, with `49_j`), `4A`, `99`, and `101` (the pronoun type).
   **Step 5 is complete**: every one of the 49 grids is a `Decl`, and no
   refactored class module calls `b.cell`/`b.push_rare` any more. The one
   remaining `b.push` is class 24's meri/veri partitive column, which is
   genuinely outside the `Decl`: its third person is `-nsa` where the
   harmonic column gives `-nsä` (`mertansa`), the only such cell in the
   tree. (Class 17's instructive was the other, and is now an ordinary
   `plain` entry.) The 51 modules that had the grid structure now read as
   the data they are.
6. Thread `pos` from `inflect_in` through `dispatch` into `Builder::new`. This is
   the change that makes `aallokas` an adjective. Show the fingerprint diff and
   check it against the 5 913 rows the sanalista says are not nouns.
7. ~~Add the 3-char guard. Check the 16 two-character forms are gone and nothing
   else moved.~~ **Cut, see below.** The 16 two-character forms are real
   nominatives (`se`, `yö`, `ja`, …), so the guard would delete words. Pinned
   by a test instead.
8. `cargo test`, clippy (no new lints against the 503 baseline), `cargo doc`,
   `check_nouns_with_omorfi.py`.
9. Commit each stage separately, with its own gate numbers in the message.

### Why step 3 was cut and 4-5 interleaved

Step 3 as written was throwaway work. Reshaping `push` to take `Tags` means
editing every `b.cell` / `b.push` / `b.push_rare` call in 52 modules, and step 4
replaces every one of those call sites with a `Decl` entry. Doing 3 first pays
the diff twice and reviews nothing in between. The `Tags`-taking `push` now
arrives with the `Decl` — `nominal_tags(pos, case, number, possessive)` is the
one function that builds a reading, and it is what stage 3 threads `pos` through.

### Two legal reasons to re-baseline, not one

The risks section below originally said re-baselining is legitimate in exactly
one case, stage 3. That was wrong, and the first three ports proved it: the
equivalence test found a defect in the live tree (`a722fc6`) — `cell_rare` sent
its possessive suffixes through `push_rare`, which takes no owner, so 62 748
rare possessive forms over 11 363 lemmas were spelled as possessives and tagged
as bare. That is a real change to the output and it was committed on its own,
before any more ports, with the test that has teeth (56 of 190 without the fix,
none with it).

So: re-baseline when the change is *intended* (stage 3) or when it is a *defect
the gate found*, and in both cases in its own commit with the count of what
moved. Never to make a port pass. Seven so far: `a722fc6` (the rare
possessive), `31d2a22` (class 18's abbreviations, baseline to
`nouns_base_v3`), `47f2cf9` (the `-i` nominative, baseline to
`nouns_base_v4`), `412e958` (the `-a`/`-ä` plural word lists, baseline to
`nouns_base_v5`), `d651e05` (class 5's `-i`-plural list, baseline to
`nouns_base_v6`), `5aa2156` (the 28* family's `-ten` genitive plural,
baseline to `nouns_base_v7`), and `5dbcf10` (class 39's `-ude` list,
baseline to `nouns_base_v8`). The eighth is the *intended* one, stage 3's
`pos` fix (`ee6a654`, baseline `nouns_base_v9`), and its diff is the one
the plan predicted: 5 713 lemmas moving off `noun`, every one keeping its
form count. The 3-char guard that was to accompany it was cut -- its 16
two-character forms are real nominatives -- so stage 3 moves those 5 713
lemmas and nothing else.

The fourth is worth a note on its own kind of evidence. Classes 11 and 13
each hold a hand-picked list of the words whose confirmed plural is one
family rather than the other, and both lists named words the reference
gives *both* families -- so every word in them was offering a confirmed
form as a Repaired guess, and `algebra`, held back to the o-stem, was not
offering ten confirmed plural cases at all. Reading the reference across a
whole class settled it without a judgement call: of class 13's 168 words,
153 give both plural families, 3 give the `i`-plural alone and 10 give the
vowel-plural alone, and not one mixes them across the four forms the two
families differ in. So the lists are not a curation anyone had to guess at,
they are a readable projection of the data, and the lists now say so.

The fifth is a different shape again: it moves no word forms at all. Class 5's
`-i`-plural list named two words of 3375 and described one (`fitness`) as
lacking `-itten`, which the reference gives. The measurement is a clean binary:
26 words confirm all three alternants, everyone else confirms none, so the
intermediate `IPlural::IdenIta` was a state the data never takes and is gone.
Baseline `nouns_base_v6` moves 25 lemmas and zero forms — every change is a
confidence, 24 promotions plus `fitness`'s `-itten`. Worth having as a
precedent: **a re-baseline can be entirely about confidence**, and the
fingerprint's per-lemma digest catches it even when its form count cannot.

The sixth is the fifth's kind again, and sharpens it: class 27 demoted the
`-ten` genitive plural for `tosi`/`täysi` behind a `ten_rare` flag, on the
belief they take only `-ien`. The reference gives `totten` and `täytten`.
The flag was the whole parameter's only user — every one of the family's 30
lemmas attests `-ten` — so it is deleted, not inverted. Baseline
`nouns_base_v7` moves 2 lemmas (`tosi`, `täysi`), 165 forms apiece before and
after. Two confidence-only re-baselines in a row now, and the point is that the
equivalence gate could not have found either: it compares the two trees to each
other, so a defect both trees share is invisible to it. What found these was
the word lists measured against the reference; what records them is the
fingerprint against the committed baseline. Keep both in the loop.

The seventh is the first that *moves forms* for the plain reason a table was
short: class 39's `-ude` list had four of the six words the reference gives the
`-ude` shape, so `nuntius` and `pangasius` were spelled with the plain `-ukse`
stem (`nuntiuksen`, which the reference does not give; it gives `nuntiuden`).
Baseline `nouns_base_v8` moves those two lemmas, 165 -> 159 forms each: the six
lost forms apiece are the unattested `-ukse` genitive, its possessives and the
`-ten` genitive plural, replaced by the `-ude` set. The audit had to split the
class string rather than match `class = '39'`, because `rosvous` is a
multi-class `39, 40` entry -- the same trap that hid class 13's `-ijä` words.

Three class-39 lemmas remain outside every list, recorded rather than forced:
`pallas` and `kallas` carry the `ll -> lt` long-`a` stem (`paltaan`,
`kaltaan`) and `pallas` is special-cased by name while `kallas` is not, which
may or may not matter because `kallas` is also `41*I` and that engine reaches
the same forms; and `poi'itus`/`rei'itys` keep an apostrophe in the lemma whose
reference forms drop it (`poiituksen`, not `poi'itukse`). None of these is a
name missing from a finite table the way the `-ude` words were: the first is a
second word of a shape the code already models (a one-line extension if the
`41*I` overlap is harmless), and the second needs a lemma-normalisation the
engine has no place for. Left for a class-39 port, where the code is rewritten
anyway.

A defect candidate that turned out *not* to be one, recorded so it is not
re-found: class 8's grid generates the vowel plural for all 109 lemmas, but
the reference gives `birdie`, `söde` and `vintage` an `i`-plural and doubles
the stem's `e` across `söde`'s and `vintage`'s paradigms (`vintageen`,
`vintagetta`, `södeet`). Three names is exactly the shape of a list, and the
temptation is to add them; but two of the three are a distinct doubled-`e`
type the grid has no shape for and spells wrong in the singular too. A list
there would fix one of three forms for one word and leave the rest wrong, so
the honest statement is that this class's type models 106 of its 109 lemmas.
Left as a measured gap in the port's doc comment.

One flag for stage 3, found while porting class 38: it calls the shared
consonant-word engine with `possessives = true`, discarding the flag its
caller passes. `build.rs` derives that flag from the part of speech, so 2962
of class 38's 3860 lemmas -- its adjectives -- currently get possessive forms
the caller asked it not to build. The live module carries a comment claiming
this is deliberate, so it is a decision and not an oversight; but it is the
same axis stage 3's `pos` work runs along, and it should be settled there
(suppress the possessives for the non-nouns, and let the nouns keep them)
rather than in a port. Recorded here so the port does not bury it.

The same hard-coded `true` is in six modules, not one: 36, 38, 39 (its
`pallas` path), 41, 45 and 49. Each has an unused-`possessives` clippy warning
already, which is the compiler saying the same thing. Taken together this is a
class of defect rather than six accidents -- the ordinal and `-nen` families
are largely adjectives, so the flag is false for most of their lemmas and true
for almost none, which may be *why* the authors stopped threading it. Stage
3's `pos` change is where the answer belongs, and the baseline move it needs
is then a single step rather than six.

The third one has a wrinkle worth writing down. A live fix cannot be committed
without the refactored port of the same class, because the two-trees test
compares them and would report the divergence — which is what happened, and is
the test working. So the pair travels together, and the way to keep the commit
readable is to hold back the *other* ports in that batch until the next commit.
`47f2cf9` is the fix plus the ports of 23 and 25; `c3b6d3c` is the ports of
19, 20 and 24, which move nothing. The baseline moves once, in the first.

## Risks / watch-outs

- **A `Decl` that cannot express a class is worse than a duplicated loop.** The
  pilot in step 4 exists to find that out on one class rather than 49. If the
  `Decl` needs a new escape-hatch field for the third class it ports, stop and
  reconsider the shape — that is the signal that the data is not regular, and
  the verb `Decl`'s own pile of `Option` overrides is the precedent for *how many*
  overrides are acceptable before it stops being a `Decl`. **This one
  materialized on the second class**, see below.
- **`bare` and `base` differ per cell and that is load-bearing.** `b.cell` takes
  both because the genitive's bare is weak while its possessives sit on the
  strong stem (`akun` beside `akkuni`), and the nominative plural does the same
  (`akut` beside `akkumme`). A `Decl` that keeps one stem per case silently
  loses the class-1 strong-genitive rule. **Materialized**: `Base` was first
  written as named variants (`Bare` / `Stem` / `Ending`) derived from the cell's
  own stem, which cannot express `akkuni`. The fix was to measure rather than add
  a variant — sorting all 1 373 `b.cell` calls by *how the base is spelled* gives
  four relations and no more (946 same stem same suffix, 312 same stem second
  ending, ~130 a different stem bare-ending, 3 a different stem with an ending),
  so `Base::Bare | Base::On { stem, ending }` covers every cell in the tree. Four
  named variants would have been a list of the 435 exceptions, not a rule for them.
- **The `double3` flag is per cell, not per class.** The doubled third-person
  spelling depends on whether the *base* ends in a harmonic vowel, so a cell
  whose base ends in `n` must not double. Collapsing it to one class-wide flag
  will invent forms. 895 cells set it, 390 do not, and no two classes agree where
  the line falls.
- **Harmony comes from the lemma, and the heuristics are per class.** `new`,
  `new_rare` and `new_harmony` all exist because `front(lemma)` is wrong for
  some words (loanword compounds with a front-by-convention neutral head). The
  `Decl` must not re-derive harmony from a stem, or those classes regress.
- **A `doubled` flag on `Cell` would have fitted class 1 and lied about two
  others.** Class 1 doubles the cell's own stem for the singular illative; class
  17 and class 18 bind a second stem beside it (`ill_v`, `ill_base`) because
  their doubling is chosen from the last letter. So it is `doubled_stem()`, a
  function, and the class states the doubled stem as the cell's stem. A flag
  would have forced class 17 to fake it.
- **An ending is not always a `[back, front]` pair.** Class 17's singular
  illative picks `-seen` or `-ihin` from the stem's last letter, so there is no
  pair to write down. Hence `Ending::Pair | Ending::Given`, with the
  constructors taking `impl Into<Ending>` so the 99% of cells that are pairs
  still read `["ssa", "ssa"]`.
- **`Decl` is a `Vec`, not a `[Cell; 12]`, and it must stay one.** The emission
  order is the class's own: class 1A emits its strong-grade cells first and its
  weak-grade ones after, and the alternant plurals are offered *between* the
  cells they belong beside (`pianoitten` after `pianoiden`, not at the end).
  Sorting into `DECLENSION` order would be tidier and would change what the
  engine emits, which the equivalence test compares.
- **`class_49` and `class_99` are the odd two.** 23 cells and 1 cell against a
  shared 24. Port them only if the port is honest; do not pad one to 24 or trim
  one to 23 to make the shape tidier.
- **The 81 thin delegators must not be touched.** They are 4 bytes of `inflect`
  and they are the reason the diff stays reviewable.
- **Do not paper over a class to make the fingerprint pass.** The fingerprint
  records current behaviour, including behaviour that is wrong. If a port
  changes a form, the right response is to find out why, not to re-baseline.
  Re-baselining is legitimate in exactly two cases, both in their own commit
  with the count of what moved: stage 3, where changing the output is the task,
  and a defect the gate found. See the sequencing section for the one that
  already happened.
- **Bind `sg` and `pl`, and count rustfmt's line explosions.** The first port of
  class 17 came out 88% *longer* than the code it replaced, all of it rustfmt
  splitting calls that crossed 100 columns: `Number::Singular` is 15 characters
  where `sg` is 2, and a call that overflows becomes seven lines. Per-module
  line counts are worth measuring after formatting, not before.

## Outcome

**Done.** All three stages landed, each gated on its own evidence, plus seven
defect re-baselines the gates found along the way.

### Stage 1 — wire both trees, reshape the builder
`458cbec`..`c10635f`. `nominals-refactored/` compiled beside `nominals/` and the
`the_two_nominal_trees_agree_form_for_form` gate added (206 classes, 832 pairs,
1664 cases). `Decl`/`decline`/`Entry`/`Cell`/`Base` added and class 1 ported as
the pilot.

### Stage 2 — the 49 grids are `Decl`s
Fifty-odd commits, `a92f640`..`83892c6` plus `baf4bf8`. Every one of the 49
grids now reads as data driven by `decline`, so a class states its stems and its
harmonic endings rather than 24 `b.cell` calls. `b.cell`/`b.push` call sites in
the class modules went from **1 397 to 4**, and the 4 are one loop: class 24's
meri/veri partitive column, whose third person is `-nsa` where the harmonic
column gives `-nsä` (`mertansa`), the only such cell in the tree. The shared
engines that ported families in one step: `gradated.rs` (1*B–1*M),
`build_i_loans` (5/6/22/99), `inflect_base` (27–30), `inflect_with_stems`
(32/38), `inflect_ton` (34/34*C), `ordinal` (45/46), `inflect_graded`/`_nom_base`
(33 and 40 classes including 39/40/42), `a_stem_inflect` (41 and its subtypes),
`e_stem_inflect` (48 and its subtypes), `inflect_with` (49/49*J),
`superl_inflect` (36/37).

The refactored tree is **not smaller** than the live one (1 517 464 bytes against
1 503 128), and it should not be made to look so: the win is that the same grid
is stated once, and the port carries documentation and tests the live tree does
not. What shrank is the *duplication*, not the byte count.

### Stage 3 — `pos`, and the guard that was not added
`ee6a654` and `f42dd56`.

Every nominal class built its reading as `nominal_tags(PartOfSpeech::Noun, ...)`
and `inflect_in` never handed the dictionary's `pos` across, so every class form
was tagged `noun`. The fix applies `Form::of_pos(pos)` at the `inflect_in`
boundary, the one place both trees share, which is also what lets the swap be a
one-line change without the defect returning. Baseline `nouns_base_v9`: **5 713
lemmas** move, every one keeping its form count exactly and changing only the
part-of-speech tag. `aallokas` and `aakkosellinen` are adjectives, `se` a
pronoun, `kaksi` a numeral.

The 3-char guard was **cut**: measured, all 16 two-character forms are a lemma's
own nominative (`se`, `yö`, `ja`, `mä`, `sä`, `EU`, ...) and several are common
words, so the guard would delete them. A test pins that they stay. The verb
guard exists because a two-character *verb form* is a fragment, not because
short words are.

### The defect re-baselines
Seven, each found by a gate and committed on its own:
`a722fc6` (a rare cell's possessives were tagged bare -- 62 748 forms),
`31d2a22` (class 18's abbreviation illatives), `47f2cf9` (the `-i` nominative
possessives), `412e958` (classes 11/13's `-a`/`-ä` plural word lists),
`d651e05` (class 5's `-i`-plural list, and `IPlural::IdenIta` deleted),
`5aa2156` (the 28* family's `-ten` genitive plural, `ten_rare` deleted),
`5dbcf10` (class 39's `-ude` list, `nuntius`/`pangasius` added). Five of the
seven moved no form count at all -- three were confidence-only -- which is why
the fingerprint, not the equivalence test, is the instrument that finds them.

### The numbers
| | at `e46c04d` | at `3199fad` |
|---|---|---|
| forms | 4 911 677 | 4 911 574 |
| `b.cell`/`b.push` sites | 1 397 | 4 |
| live nominal modules | 209 | 152 |
| equivalence cases | -- | 574 lib + 26 integration; green over 1 434 |
| Omorfi check | -- | 740 confirmed, 0 missing, 0 not offered |

The equivalence gate is green with both trees agreeing form for form. The swap
is now the one-line `#[path]` change the plan promised.

### The dead-module cleanup
`3199fad` removed 57 generated modules named by sanitising raw sanalista class
values (`(1)` -> `class__1_.rs`, `10, (11)` -> `class_10___11_.rs`). They were
the temporary dump of `extract_class_rules.py --rust`, and they are dead:
`parse_inflections` drops the parenthesised part and keeps only the first
gradation letter, so `Inflection::to_string()` can only yield a class number
with an optional single `*<letter>` -- `(1)` parses to nothing, `10, (11)` to
class 10, `9*(D)` to class 9. Of the dispatch table's 206 keys, 58 were
unreachable; 57 of their modules were these files. Removing them left the
fingerprint byte-identical (`nouns_base_v10` == v9), which is the proof they
never ran. The generator's `--rust` default moved to `target/class-rules` so it
cannot recreate them in `src/`.

Seven generated modules remain -- `class_49D`..`class_49H`,
`class_50`, `class_51` -- all reachable and none with a `__` name. Replacing
them with hand-written engines would change output (the generated rules carry
no possessives) and is a separate, re-baselined step, not part of this cleanup.

A second naming pass (`1fc768e`) settled the gradation-subclass convention.
The tree had two -- `class_9B` (number + uppercase) for 58 modules and
`class_49_j` (number + `_` + lowercase) for 43 -- and the second was the
sanalista's `*` leaking into a filename. All 43 are now `class_28J`,
`class_33B`, `class_41A`, `class_48A`, `class_49J` and kin, in both trees and
at every reference. Baseline `nouns_base_v11` is byte-identical to v10.
### Retirement (2026-10-24)

The remaining 53 unswapped classes were swapped, the old `nominals/` per-class
files were deleted, and the refactored tree was moved into
`src/expand/nominals/`. `AUTHORITATIVE_CLASSES` / `is_authoritative` (which only
the live tree carried) were copied into the surviving `mod.rs`, so the
authoritative/additive split is unchanged.

Removed with the duplicate tree:

- the 99 `#[path]` swap declarations and the `nominals_refactored` module,
- `expand::tests::the_two_nominal_trees_agree_form_for_form` (it would now
  compare each file with itself),
- the second-tree loop in
  `a_word_without_possessives_gets_no_possessive_column`, which now asserts the
  contract against the single tree.

Verification: `cargo check -p torille --all-targets` and `cargo clippy -p
torille --all-targets -- -D warnings` are clean; 680 lib tests and the 26
`tests/catalogue.rs` cases pass.
