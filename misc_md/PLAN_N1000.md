# Full-lexicon noun sweep — systematic closure plan

Status: **implemented (2026-11-16).** The `n=20`, `n=100` and `n=1000` samples
are clean on the correctness axes: `n=1000` is 2,318,849 confirmed / 0 missing
/ 0 illegal / 0 not-plain / 304 oracle-only / 76,116 unsupported. The plan below
is the record of how the sweep was closed.

Baseline commit: `e34081f` (`fix(nominals): start the n=1000 sweep`).
Baseline maps: `07af70f`.

## Authority

The Finnish entry at

```
https://en.wiktionary.org/wiki/<word>#Finnish
```

is the **highest authority** for every noun form in this plan. It is available
to this workflow: the raw page is fetched with
`https://en.wiktionary.org/w/index.php?title=<word>&action=raw`, and a
declension template is expanded with
`https://en.wiktionary.org/w/api.php?action=expandtemplates&format=json&prop=wikitext`.
The template expansion is the practical way to read the whole paradigm, because
the page itself transcludes the table.

It is a development-time oracle only. Nothing in the engine or a binding may
make a network call, and no GPL Omorfi/Voikko code enters the engine.

The rules that follow from that authority:

1. **Wiktionary overrules the materialised oracle whenever they disagree.** A
   form the oracle marks `ok='both'` is still wrong if Wiktionary's Finnish
   entry does not give it in that cell.
2. **Two Wiktionary declensions are two real paradigms.** `parka`, `säkä`,
   `sini`, `palvi`, `ori`, `kysta`, `lyyra`, `koala`, `nahka`, `veto`, `hepo`
   and `puoliyö` all have more than one accepted table. The engine may emit
   both; the report must not call one of them a missing form merely because the
   oracle kept only the other.
3. **Do not add an unsourced form.** If Wiktionary lacks an entry (some
   abbreviations), Kotus/the materialised oracle is the fallback and the rule
   says so. If Wiktionary gives a form as rare, the form is accepted but is
   `Repaired`, never a plain suggestion.
4. **Every oracle disagreement gets a narrow evidence row** in
   `data/oracle_exceptions.json`, with the Wiktionary URL in `source` and a note
   that names the alternation. A rule discovered only at `n=1000` carries
   `"sample": 1000`, so the `n=5` gate does not call it stale.
5. **A class fix beats a lemma patch.** A lemma rule is for a genuinely lexical
   fact (an irregular stem, a colon abbreviation, a two-paradigm word), not for
   a suffix that should fall out of the class engine.

## Baseline

| sample | confirmed | missing | illegal | not-plain | oracle-only | unsupported |
|---|---:|---:|---:|---:|---:|---:|
| `n=20` | 226,862 | 0 | 0 | 0 | 106 | 12,995 |
| `n=100` | 679,381 | 0 | 0 | 0 | 268 | 30,008 |
| `n=1000` first sweep | 2,314,088 | 1,464 | 2,091 | 112 | 268 | 75,963 |

The correctness axes are `missing`, `illegal` and `not_plain`. `unsupported`
and `oracle-only` are coverage, not correctness: they say the oracle has no
`ok='both'` answer for the cell, not that the engine is wrong.

### Current cluster snapshot

Measured after the first batch (`n=1000`, `-j 1`, failures only):

| class | confirmed | missing | illegal | not-plain | dominant lemmas |
|---|---:|---:|---:|---:|---|
| 1 | 111,494 | 202 | 246 | 3 | `veto` 202, `hepo` 202, `menu` 44 |
| 5 | 110,405 | 76 | 287 | 0 | `kuori` 142, `irtovuori` 82, `kaihi`/`orhi` 114 |
| 9 | 91,485 | 120 | 285 | 0 | `parka` 202, `lyyra` 76, `kysta` 76, `koala` 19, `vikunja` 32 |
| 9*D | 49,802 | 50 | 50 | 0 | `nahka` 100 |
| 10 | 140,104 | 393 | 259 | 0 | `PR` 222, `sora-r` 222, `säkä` 202 |
| 48 | 46,324 | 163 | 114 | 0 | `kiiru` 157, `ori` 120 |
| 50 | 78,628 | 304 | 738 | 97 | `kuivamuona` 129, `raitisilma` 111, `kevytsukeltaja` 82, `omahoitaja` 76, `vapaamatkustaja` 70, `puoliyö` 65 |

These are finding counts, not lemma counts. The table is a prioritisation aid;
Phase 0 changes the denominator, so it is re-measured before any engine work.

## Regression ledger

The sweep is not only new coverage; one recent change is a live regression.

| id | change | symptom | status |
|---|---|---|---|
| R1 | class-10 agent-tail exception became a three-lemma allow-list (`e34081f`) | `kevytsukeltaja` 82, `omahoitaja` 76 and `vapaamatkustaja` 70 illegal: normal compounds now keep the `-oi-` plural, and the one `-oi-` compound keeps the `-i-` plural | **open — Phase 1** |
| R2 | class-4*A weak essive was demoted for the whole class | 1,610 `not_plain` rows in words like `aallokko` | fixed: kept only for `eksentrikko`, `koirakko`, `loogikko`, `poliitikko`, `suolakko` |
| R3 | class-2 `koordinaatisto` was on the `palvelu` i-plural | `koordinaatistoiden` etc. | fixed: delegates to class 1 `valo` |
| R4 | class-20 `-hyn` was repaired for every `-yy` word | `fondyyhyn`, `revyyhyn` not plain; `miljööhyn` was over-promoted | fixed: only `fondyy`, `revyy`, `nugaa` keep their confirmed illative |

The rule for every later batch: after a class change, re-run that class at
`n=1000` **and** the `n=20`/`n=100` samples before committing. R1 is the proof
that a small sample cannot see an allow-list mistake.

## What the failures actually are

They fall into four families, and the order below follows the family, not the
class number.

### A. Report artifacts around homonyms

A sanalista row can carry several classes (`50, 51`, `9, 12, (10)`, `48, 5`),
and a lemma can be several homonym rows (`veto` is `1*F`, `1*F`, `1`). The
report currently has two problems:

- `Harness.classes_of` returns the raw class string, and
  `len(set(classes_of(lemma)))` therefore sees `"50, 51"` as one class. The
  report then inflects the word with no class filter and compares the union
  against one class's rows.
- When it does force a class, the oracle rows for the homonyms are often
  identical (`sini` 5 and 7 both have `sinien`/`sinittä`), so forcing one class
  produces a missing row for a form the other class legitimately owns.

This is the largest single source of noise: roughly 54% of both axes in the
first sweep came from lemmas with more than one class token. Most of the
`missing` rows are artifacts of forcing one class; many of the `illegal` rows
are real alternate paradigms that the oracle kept out, so they stay visible
until Phase 5 sources them. The report still has to be fixed before engine
work, or every later measurement mixes the two.

### B. Real engine gaps

- class 10 has no colon/abbreviation mode (`PR`, `sora-r`);
- class 48 assumes the lemma ends in `-e` (`kiiru` is a `hame` with stem vowel
  `u`, `ori` with stem vowel `i`);
- class 50's compound policy has the agent-tail regression above;
- class 50 has no "the modifier also inflects" output for `puoliyö`, although
  Wiktionary gives both `pieni-suo` and `suo` tables;
- `vikunja` is filed as class 9 but Wiktionary declines it as `kulkija` (class
  12);
- `nahka` is `9*(D)` with both a graded and an ungraded table, and the current
  override keeps only the graded one.

### C. Oracle-only and rare alternates

These are not engine bugs. They need evidence rows, not new forms:

- `kuivamuona` and `raitisilma` are fixed-modifier compounds in Wiktionary;
  the oracle's inflected-modifier spellings are the alternative, not a recall
  gap;
- `silkkivuori`, `sini`, `palvi`, `kaihi`, `orhi` are class-5 `risti` words;
  Wiktionary's plural abessive is `-eittä` (`silkkivuoreitta`, `sineittä`),
  while the oracle sometimes keeps the `-ittä` spelling;
- `kovametalli` and the other `-metalli` compounds confirm only
  `kovametallien` in Wiktionary; the oracle's `kovametalleiden`/`-itten` are
  rare alternates, so the engine's `Repaired` confidence is correct;
- `piano`, `jargon`, `fitness` and the other class-1/5 i-plural loans have rare
  `-iden`/`-itten`/`-ita` alternants that must stay non-plain.

### D. Tail/compound cell mismatch

The remaining class-50 rows are tail/whole-word cell disagreements
(`lyhytnumero`, `harmaakaihi`, `valmisvaate`, `kevytsinko`, `raakanahka`).
Some are tail-class bugs; some, like `lyhytnumero`, are the oracle putting a
plural stem into a singular cell. Each one is settled by the compound's own
Wiktionary template before a compound exception is written.

## Phase 0 — make the report measure the lemma, not one homonym

**Goal:** remove the report artifact before touching the engine. No generated
form changes.

Changes:

1. Add a `class_tokens(lemma)` helper to `tools/omorfi_harness.py`:
   split on commas, drop parenthesised alternatives beside an unbracketed
   class (`9, 12, (10)` -> `9`, `12`; `50, 51` -> `50`, `51`), keep the
   gradation (`1*F`, `9*D`, `48*(E)` -> `48` or `48*E` according to the
   parser's rule).
2. Replace `multi_class = len(set(classes_of(lemma))) > 1` with a real
   per-lemma class list, and compare at the **lemma level**:
   - engine forms: for each token in `class_tokens(lemma)`, call
     `inflect(lemma, luokka=token)` and merge the results. `inflect(lemma)` with
     no class chooses one row, so it is not enough (`veto` would lose either
     `vedoitta` or `vetoitta`). A classless lemma still calls `inflect(lemma)`;
   - expected forms: the union of the `ok='both'` rows for all homonyms;
   - `missing` is raised only when **no** class engine produces the expected
     form. A form another class owns is not a recall gap for this class;
   - `illegal` is still raised for an engine form that is in **none** of the
     expected rows. That is deliberate: it exposes an unsourced alternate
     (`isokierto`'s class-51 forms, `veto`'s ungraded `vetoitta`) instead of
     hiding it. Phase 5 either sources it or removes it;
   - findings are still bucketed under the reported class for triage, and the
     JSON gains `homonym_classes`.
3. Add `--class-strict` to `tools/report_by_class.py`. It restores the old
   force-one-class behaviour for debugging a single paradigm; it is not the
   default and it is not what the gate runs.
4. Add a small test (new `tools/test_homonym_audit.py`, or a unit test beside
   the harness) covering:
   - `isokierto` (`50, 51`): class 50 must not report `isokiertojen` as
     missing; the class-51 forms remain a separate illegal/override question;
   - `koala` (`9, 12, (10)`): the class-9 report must accept `koalojen`;
     class-12's `koaloiden` remains a real alternate to source or demote;
   - `veto` (`1*F`, `1*F`, `1`): the class-1 report must accept `vedoitta`,
     and `vetoitta` remains the ungraded alternate to source;
   - `ori` (`48, 5`): `oriiden` and `oreitta` are both in scope; the class-5
     and class-48 sets are not each other's missing rows.

**Checkpoint:** rerun the seven cluster classes plus `n=20` and `n=100`. The
expected result is a large drop in `missing` with `illegal` still showing the
real alternates; there must be **no new `not_plain`**. If a class-specific
defect disappears entirely, confirm it still shows under `--class-strict`; if
it does not, the lemma-level comparison is masking it and the class needs a
targeted test before Phase 1.

## Phase 1 — stop the class-50 agent-tail regression

**Goal:** close R1 and make the `-i-`/`-oi-` choice a policy, not a list of
three words.

Current code in `src/expand/compound.rs`:

```rust
let i_plural_tail = matches!(
    lemma,
    "kultainennoutaja" | "tarkka-ampuja" | "kylmävaraaja"
);
if i_plural_tail && plural && tail.word.starts_with(&oi_prefix) {
    continue;
}
```

That is backwards for every other agent compound: `kevytsukeltaja` and
`omahoitaja` keep the `-oi-` plural and are reported illegal, while
`vapaamatkustaja` keeps the `-i-` plural and is reported illegal in the other
direction.

Replace it with an explicit policy:

```rust
enum AgentPlural { NormalI, Oi }

let agent_plural = if lemma == "vapaamatkustaja" {
    AgentPlural::Oi
} else {
    AgentPlural::NormalI
};
```

- `NormalI`: skip the `-oi-` alternants, exactly as the current `true` branch
  does. This is the default for every class-10 `-ja`/`-jä` tail.
- `Oi`: skip the `-i-` plural obliques, keep the nominative plural and the
  `-oi-` obliques. `vapaamatkustaja` is the one page that needs this.

Tests in `src/expand/mod.rs`:

- `kevytsukeltaja` -> `kevytsukeltajitta`, not `kevytsukeltajoitta`;
- `omahoitaja` -> `omahoitajilta`, not `omahoitajoilta`;
- `kultainennoutaja`, `tarkka-ampuja`, `kylmävaraaja` keep their existing
  normal forms;
- `vapaamatkustaja` -> `vapaamatkustajoitta`, not `vapaamatkustajitta`.

If a later sweep finds another `-oi-` compound, it joins the `Oi` list with its
Wiktionary source; the default stays `NormalI`.

## Phase 2 — close the fixed-modifier compound rows

**Goal:** make the report distinguish "the oracle prefers the inflecting
modifier" from "the engine cannot produce a real form".

`data/oracle_exceptions.json` already has `compound-50-fixed-prefix`, but it
only explains `illegal` rows. The missing side is the same fact seen from the
other direction: `kuivamuona` and `raitisilma` are fixed-modifier compounds in
Wiktionary, while the oracle expects `kuivittamuonitta` /
`raittiittailmoitta`.

Changes:

1. Extend the `fixed-prefix` predicate to `missing`:
   - `_compound_prefix(lemma)` gives the modifier;
   - if the expected form does not start with the modifier, the modifier was
     inflected and the engine's fixed form is the compound reading;
   - if it does start with the modifier but the head starts later than
     `len(prefix)`, the modifier has added material and the same conclusion
     holds.
2. Add `"applies_to": ["illegal", "missing"]` to the class-50 rule (and the
   class-51 rule, which has the same shape).
3. Keep the engine's fixed output for `kuivamuona` and `raitisilma`; do not
   invent the oracle's inflected forms.
4. Treat `puoliyö` separately, because Wiktionary gives **both** declensions:
   `pieni-suo` (`puolittaöittä`) and `suo` (`puoliöittä`). The engine already
   produces the `suo` set. Add the `pieni-suo` set from the class-51-style
   inflecting modifier, keep both, and let `fixed-prefix` explain the fixed
   set against the oracle. The two sets are not duplicates; the first is the
   modifier-inflected paradigm and the second is the lexicalised compound.

**Checkpoint:** class 50 `missing` should drop by ~240; `puoliyö` should have
no missing rows; the fixed set may still show `illegal` rows until the
`fixed-prefix` rule is applied in the default report.

## Phase 3 — class-10 colon abbreviations

**Goal:** `PR` and `sora-r` (444 rows) get the colon paradigm, not `PRR…`.

The oracle's `ok='both'` grid for `PR` is the specification:

```
PR, PR:n, PR:ä/PR:ää, PR:ään,
PR:t, PR:ien, PR:iä, PR:ihin/PR:iin,
PR:issä, PR:istä, PR:illä, PR:iltä, PR:ille,
PR:inä, PR:iksi, PR:ittä, PR:in
```

`sora-r` is the same grid with `sora-r` before the colon. The colon is an
orthographic boundary: the stem is `PR` / `sora-r`, the ending follows the
colon, and the nominative singular is the only cell without a colon.

Implementation:

- add a colon branch to `src/expand/nominals/class_10.rs` (or a small
  `colon.rs` helper called by it); build the class-10 `koira` grid on the part
  before the colon and splice `:` before every ending;
- preserve the two partitive spellings (`PR:ä`, `PR:ää`) and the two plural
  illatives (`PR:ihin`, `PR:iin`);
- do not treat `:` as part of the stem for harmony, gradation or the
  possessive column.

Tests: every `ok='both'` form above, plus `PR:ttänsä` / `PR:ttään` and
`sora-r:ttänsä` / `sora-r:ttään`. The `PR:äään` row is Omorfi-only and stays
out of the plain set.

Wiktionary is unlikely to have a Finnish entry for these abbreviations; the
note in any evidence row says the colon spelling comes from the Kotus/oracle
grid, and the rule is still narrow to the two lemmas.

## Phase 4 — class-48 stem-vowel override

**Goal:** `kiiru` (157 rows) and `ori` (120 rows) decline as `hame` with a
non-`e` stem vowel.

Wiktionary:

```
kiiru  {{fi-decl-hame|kiir|||a|stem_vowel=u}}
       kiiru, kiiruun, kiirutta, kiiruut,
       kiiruiden/kiiruitten, kiiruita, kiiruitta
ori    {{fi-decl-hame|or|||a|stem_vowel=i}}
       ori, oriin, oritta, oriit,
       oriiden/oriitten, oriita, oriitta
```

The current `e_stem_inflect` builds `long = short + last_char`, which is only
right for the `-e` family: `kiiru` becomes `kiiree-` and `ori` becomes `oree-`.

Change the shared engine to take the stem pair:

```rust
fn e_stem_inflect(lemma, possessives, short: &str, long: &str)
```

and in `class_48::inflect`:

- default `-e` words: `short = lemma without final e + e` (unchanged), `long =
  short + short.last()`;
- `kiiru`: `short = "kiiru"`, `long = "kiiruu"`;
- `ori`: `short = "ori"`, `long = "orii"`.

The existing cells already put the long stem in the singular and nominative
plural, and the short stem in the genitive/partitive/locative plurals, so only
the pair changes. Add tests for the exact forms above, and keep `aave`, `aihe`,
`viive`, `tamineet` unchanged.

`ori` also has a class-5 `risti` row. After Phase 0 the report audits the union;
the class-48 `hame` forms and class-5 `risti` forms are different real
paradigms, so both stay. No evidence is needed for the class-5 set if it is
already in the oracle.

## Phase 5 — twin paradigms, optional gradation and class-5 `risti`

**Goal:** the words Wiktionary gives twice are emitted twice, with the oracle's
omissions explained rather than hidden.

### 5a. `nahka` (`9*(D)`)

`src/sanalista_overrides.rs` deliberately reads `9*(D)` as `9*D` because the
standard singular is `nahan` (also `nahkan`). Wiktionary has both tables:

- `kala|nah|k`: `nahan`, `nahkojen`, `nahoissa`, `nahoitta`;
- `kala|nahk`: `nahkan`, `nahkojen`, `nahkoissa`, `nahkoitta`.

The engine currently emits only the graded table, so the oracle's ungraded
`nahkoitta` is missing and the graded `nahoitta` is illegal. Emit both tables
for this lemma: the ungraded set plain (oracle-backed) and the graded set plain
too (Wiktionary-backed), with a narrow `stems: ["naho"]` evidence rule for the
graded weak plural. Do **not** demote `nahoitta` to `Repaired` just to silence
the row; it is a standard form.

### 5b. `säkä`, `parka`, `veto`, `hepo`

After Phase 0 the engine union contains both the graded and the ungraded
paradigm. The oracle keeps only the graded-looking cells:

- `säkä` `10*D`: `säittä`; engine also has ungraded `säkittä`;
- `parka` `9*D`: `paroitta`; engine also has ungraded `parkoitta`;
- `veto` `1*F`: `vedoitta`; engine also has ungraded `vetoitta`;
- `hepo` `1*E`: `hevoitta`; engine also has ungraded `hepoitta`.

Add one `stems` evidence rule per lemma (or one `also-class` rule if the whole
alternate set is known) with the Wiktionary URL. The ungraded forms are not
`Repaired` guesses: they are the second Wiktionary table, so accepting them as
plain is correct.

### 5c. `kysta`, `lyyra`, `koala`

Wiktionary gives both tables:

- `kysta` and `lyyra`: `kala` (class 9) and `koira` (class 10);
- `koala`: `kala` (class 9) and `katiska` (class 12).

After Phase 0 the engine union contains both (`kystoitta`/`kystitta`,
`lyyroitta`/`lyyritta`, `koaloitta`/`koaloiden`). The oracle's `ok='both'`
rows kept only the `kala` forms for the class-9 report. Add `stems` evidence
rules for the `koira`/`katiska` set: `kysti`, `lyyri`, `koaloi`. Do not remove
the second table; it is standard Finnish.

### 5d. class-5 `risti` and the `5`/`7` homonyms

`silkkivuori` is `risti|silkkivuor`; `sini`, `palvi`, `kaihi` and `orhi` are
`risti` words, and `sini`/`palvi` also have a class-7 `ovi` row. Wiktionary's
plural abessive for `risti` is `-eittä`:

```
silkkivuori  silkkivuoritta (sg), silkkivuoreitta (pl)
sini          sinittä (sg), sineittä (pl)      [risti]
              sinettä (sg), sinittä (pl)       [ovi]
```

The engine's class-5 `e`-stem plural is therefore right, and the oracle's
`-ittä` plural is the `ovi`/singular spelling. Add narrow `stems` rules for the
`-eittä` set. For `sini`/`palvi`, use `also-class` only after checking the full
set: the paired `ovi` spellings are real, but a broad rule would also excuse a
genuinely wrong class-5 form. Do not change the class-5 grid to make one oracle
row pass; that would break every loanword in the class.

`kuori` and `irtovuori` (`5` + `26`) are the same family; Phase 0's union
should remove their artificial rows. If `--class-strict` still shows a
class-26 defect, fix class 26, not class 5.

### 5e. `vikunja`

The sanalista files it as class 9, but Wiktionary is
`{{fi-decl-kulkija|vikunj|a}}` (`vikunjoiden`, `vikunjoitten`, `vikunjoita`,
`vikunjoitta`), i.e. class 12 (`kulkija`), not class 9 (`kala`) and not class
10 (`koira`). Add it to `sanalista_overrides.rs` as `("vikunja", "12")` — this
is a lexical class correction, not a suffix rule. Add a test that `vikunjoita`
is plain and `vikunjoja` is not the class-9 reading.

### 5f. `isokierto` and the `50, 51` rows

`isokierto` is filed as `50, 51`, but Wiktionary gives one `valo` table
(`isokierron`, `isokiertojen`) with the modifier fixed. The class-51
modifier-inflected forms (`isojenkiertojen`) are therefore the unsourced
alternate, not a second standard paradigm. Demote the class-51 set to
`Repaired` (or override the lemma to class 50 if the class-51 bucket should
not audit it at all) and add a test that `isokiertojen` is plain while
`isojenkiertojen` is not. This is the general shape for any `50, 51` row whose
Wiktionary page shows only one table.

### 5g. `menu`, `piano`, `jargon`, `fitness`

Phase 0 handles `menu` (`1, 21`) by union. The i-plural loans are different:
`piano`'s `pianoiden`/`pianoitten`/`pianoita` and the class-5 `-iden`/`-itten`
sets are rare alternants. If Wiktionary gives them as rare, leave them
`Repaired` and add an `applies_to: ["not_plain"]` evidence rule. If Wiktionary
gives them as ordinary, promote them in the class engine. Do not promote a
form merely because the oracle says `ok='both'`.

To support that, extend `illegal_evidence.py` and its gate to accept
`"not_plain"` in `applies_to`, and have `check_class` reclassify a matched
`not_plain` as `confirmed` with a detail that says the engine deliberately
withholds it.

## Phase 6 — class-50 tail and confidence residuals

After Phases 0–5 the remaining class-50 failures should be small and named:

| lemma(s) | rows | diagnosis | action |
|---|---:|---|---|
| `lyhytnumero` | 57 | Wiktionary declines the whole word as `palvelu` (class 2): singular `lyhytnumerotta`, plural `lyhytnumeroitta`. The engine's singular is right; the oracle puts the plural tail into the singular cells. | narrow `cell` + `stems` evidence rule; do not change class 50 |
| `harmaakaihi` | 57 | `kaihi` is `5` (`risti`) + `7` (`ovi`); the engine's `harmaakaiheitta` is the risti plural, the oracle's `harmaakaihitta` is the ovi plural | Phase 0 union plus a `stems` evidence rule for the risti set |
| `valmisvaate` | 14 | `vaate` is class 48 `hame` with t:tt; Wiktionary's ablative plural is `valmisvaatteilta`, the oracle's `valmisvaateilta` is the short form | verify the ablative cell, then evidence if the engine is right |
| `kevytsinko` | 14 | Wiktionary's `valo` template (`kevytsi|nk|ng`) puts the plural locals on the weak `singo-` stem; the oracle keeps the strong `sinko-` in a weak cell | evidence; class 1*G is already the right grid |
| `raakanahka` | 100 | `nahka` tail | covered by 5a |
| `kovametalli`, `raskasmetalli`, `jalokoralli`, `raakametalli`, `kevytmetalli`, `kevytjukurtti`, `villiyrtti` | 97 | the tail's rare `-iden`/`-itten` alternants are `Repaired`; the oracle confirms them | `not_plain` evidence; keep `Repaired` |

The order matters: fetch the compound's own Wiktionary template first. Where
Wiktionary declines the whole compound as one class (`lyhytnumero` is
`palvelu`), the oracle is the one that is wrong, and the fix is evidence, not a
new compound branch.

## Phase 7 — final sweep, maps and remotes

When the targeted reports are clean:

1. Full sweep:
   `python3 tools/report_by_class.py -n 1000 -j 1 --only-failures -o /tmp/report1000-final.json -q`
   (always `-j 1`; SQLite is not thread-safe).
2. Regression samples:
   `python3 tools/report_by_class.py -n 20 -j 1 ...` and `-n 100 -j 1 ...`
   must stay at 0 missing / 0 illegal / 0 not-plain.
3. Evidence gate:
   `python3 tools/report_by_class.py -n 5 -j 1 --raw-illegal -o /tmp/report5-final.json -q`
   then `python3 tools/test_illegal_evidence.py --report /tmp/report5-final.json -n 5 -j 1`.
4. Full tests and lint:
   `cargo test --release --workspace`
   `cargo clippy --workspace --all-targets -- -D warnings`
5. Regenerate the engine-derived blobs and compare:
   `cargo run --release -p torille-tools --bin gen_all_forms`
   `cargo run --release -p torille-tools --bin gen_stem_map_noun`
   `cargo build -p torille`
   `python3 tools/build/compare_engine_outputs.py --strict`
6. Publish `maps` and mirror both remotes:
   `cargo run -p torille-maps -- publish`
   then force-push the new `maps` commit to `github`.
7. Commit one phase at a time; put the before/after `n=1000` counts in the
   commit body. `syntax` and `maps` must end on the same commit on both
   GitLab `origin` and GitHub `github`.

## Per-batch validation

Run this after every phase, before regenerating maps:

```bash
cargo test --release -p torille --lib <module>
(cd bindings && env -u CONDA_PREFIX maturin develop --release)
python3 tools/report_by_class.py -c '<touched classes>' -n 1000 -j 1 \
    --only-failures -o /tmp/report1000-<phase>.json -q
```

Then, before the commit:

```bash
cargo test --release --workspace
cargo clippy --workspace --all-targets -- -D warnings
python3 tools/report_by_class.py -n 20 -j 1 -o /tmp/report20-<phase>.json -q
python3 tools/report_by_class.py -n 5  -j 1 --raw-illegal \
    -o /tmp/report5-<phase>.json -q
python3 tools/test_illegal_evidence.py --report /tmp/report5-<phase>.json -n 5 -j 1
```

A phase is not done until the targeted class is clean **or** every remaining
row is explained by a rule that names its Wiktionary source.

## Definition of done

- `n=1000`: `missing = 0`, `illegal = 0`, `not_plain = 0` on the default
  report. `unsupported` and `oracle-only` may remain; they are coverage.
- `n=20` and `n=100` remain 0/0/0.
- Every default-report `illegal` row matches exactly one evidence rule; every
  rule in `data/oracle_exceptions.json` is still live at `n=5` or its declared
  `sample`.
- Every new form is traceable to a class engine or a Wiktionary page. No
  `is_known_word` suffix strip was added to make a word pass.
- `cargo test --release --workspace` green, clippy clean, strict engine-output
  comparison pass.
- `maps` regenerated and published; `syntax` and `maps` identical on both
  remotes.

## Risks and invariants

- **Evidence rules can hide defects.** The gate requires exactly one match, so
  keep each rule's `lemmas`/`cell`/`stems` narrow. `also-class` is a last
  resort, not a blanket exemption.
- **Union auditing can mask a class bug.** `--class-strict` is the debugging
  counterweight; every class whose rows disappear under union still needs its
  own class test (the `tests/catalogue.rs` data-driven net is the guard).
- **Two paradigms must not become a form explosion.** `puoliyö`, `nahka` and
  the `-metalli` family are curated lemma lists, not a general "run every class
  twice" switch.
- **Do not hand-edit generated `class_50.rs`.** Class 50 is generated by
  `tools/oracle/extract_class_rules.py --rust`; compound policy belongs in
  `src/expand/compound.rs` and the tail class engines.
- **Do not run `cargo fmt` workspace-wide.** `src/expand/mod.rs` and
  `src/expand/build.rs` are hand-formatted; format only the files touched.
- **`report_by_class.py` must stay `-j 1`.** Parallel class workers corrupt
  the SQLite reader; the `-j 8` failures are not engine failures.


## Outcome

Phases 0–6 landed together:

- the report now splits comma-separated class fields, unions the lemma's
  per-class paradigms, and keeps `--class-strict` for single-paradigm
  debugging;
- class 10 has the colon grid (`PR`, `sora-r`, and the `CD-ROM` colon forms
  used by class 5);
- class 48 has the `hame` stem-vowel override (`kiiru`, `ori`);
- class 5/9/10/26 twin paradigms are emitted and sourced by evidence rows;
- class 50's agent tails default to the normal `-i-` plural, with
  `vapaamatkustaja` the `-oi-` exception, and `puoliyö`, `isokierto`,
  `nuoripari` and the fixed-prefix compounds have explicit policy;
- `vikunja` is class 12 (`kulkija`), `nahka` emits both `9*(D)` tables, and
  `CD-ROM` keeps both the colon and closed spellings.

The final gates are `cargo test --release --workspace` (55 binaries, 1,315
tests), clippy, the `n=5` evidence gate (119 rules), the `n=20`/`n=100`
samples, and `tools/build/compare_engine_outputs.py --strict`.


### Exhaustive follow-up

`n=1000` was a sample: eight noun classes have more than 1,000 lemmas. The
exhaustive pass over `1, 2, 5, 6, 10, 12, 39, 40` audited 18,148 lemmas and
2,049,728 forms. It found only the class-5 `vuori` compounds, `uros`/
`merirosvous`, and the remaining class-5 i-plural promotions; all are closed.
The smaller classes were already fully covered by `n=1000`.


### Adjective follow-up

`report_by_class.py` gained `--pos adjective --bare` and now honours the
requested cell grid in the materialised-oracle path. The adjective `n=1000`
sample is 76,440 confirmed / 0 missing / 0 illegal / 0 not-plain. It exposed
the `34*A` geminating `-ton` adjective (`seksitön`) and the adjective
superlative `perin` (`sisin`/36), both now sourced and tested.
