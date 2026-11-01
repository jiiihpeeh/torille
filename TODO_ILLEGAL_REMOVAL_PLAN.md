# Illegal-form removal plan

Status: **complete for the current noun lexicon.** The engine-side `34*C`
defect is fixed, and so are the class-50 compound defects behind the
plural-only tails (bound combining forms like `‑kulmio`, and tails that are the
nominative plural of their head, like `vanhemmat`/`vanhempi`) and the
plural-only `-t` lemmas in classes 9, 9\*, 12, 13 and 14\*A (`aktiivat` ->
`aktiivoiden`, `rintsikat` -> `rintsikoiden`). The default report applies
`data/oracle_exceptions.json` and is 0 illegal at every sample. `missing` is
counted per cell (`drop_covered_missing`): a cell is missing only when the
engine produces **none** of its confirmed spellings, which takes the
full-lexicon `missing` from 100,453 to **24,986** and the default from 73,386
to **19,787** (recall 99.3%). The raw oracle
report is **538 illegal at n=5, 2,003 at n=20, 6,271 at n=100 and 12,179 at
full lexicon**, all covered by **44 rules**; the verified run is 0 illegal at
full lexicon. The evidence file also curates the `missing` direction: the
oracle's strong-grade comparative spellings (`kuurompalta`,
`alamaisempalta`) and plural-only `-iset` singulars are not words, so they are
reported as `oracle-only` (**78 at n=5, 238 at n=20, 931 at n=100, 5,199 at full lexicon**)
rather than as recall gaps. The evidence gate `tools/test_illegal_evidence.py`
is green at n=5, n=20, n=100 and full.

The remaining plural-only work is the classes whose engines still treat a
`-t`-final lemma as a singular: 2, 3, 5\*G, 16\*H, 18 and 20 (`bileet`,
`kapiot`, `illatsut`, `häät`, `juomingit`). Class 38's `-iset` nouns are now
plural-only, and the comparative possessives are generated. Those are a
separate engine issue, not an illegal-form one.

Goal: make `illegal` a trustworthy number in both report modes.

1. **Default materialised oracle:** 0 illegal, with every remaining oracle
   disagreement represented by an explicit, externally evidenced, per-cell
   rule instead of a blanket class exemption.
2. **`--verify-illegal`:** 0 illegal on the full lexicon, not only on the
   5-lemma sample.
3. **No hidden defects:** a genuine engine error must still fail the report
   even when it lives in a class that has an oracle-error rule.

## Current state

Measured 2026-11-01 with the 5-lemma sample:

```bash
python3 tools/report_by_class.py -n 5 -j 8 \
    -o reports/noun-classes.json
python3 tools/report_by_class.py -n 5 -j 8 --verify-illegal \
    -o reports/noun-classes-verified.json
```

| mode | confirmed | missing | illegal | mistagged | unsupported |
|---|---:|---:|---:|---:|---:|
| default (SQLite oracle) | 96,200 | 8,704 | 538 | 0 | 714 |
| `--verify-illegal` | 96,738 | 8,704 | **0** | 0 | 714 |

The two modes differ only in the illegal/confirmed split: the 538 default
illegal rows are reclassified as confirmed by live Voikko and the curated
Wiktionary list. The `missing` and `unsupported` counts are identical because
`--verify-illegal` does not touch them.

### Where the 538 live

All 538 are `Plain`, so all of them are offered as corrections today.

| class | illegal | confirmed | engine form vs oracle expectation | evidence status |
|---|---:|---:|---|---|
| `51` | 141 | 628 | `omakuvitta` vs `omittakuvitta` | dual-class word; live Voikko accepts engine; needs per-lemma audit |
| `41*C` | 88 | 651 | `rattaitta` vs `rattaaitta` | plural-only `-aat`; Wiktionary confirms short form |
| `41*I` | 88 | 637 | `maltaitta` vs `maltaaitta` | plural-only `-aat`; Wiktionary confirms short form |
| `33*D` | 76 | 319 | `näkimittä` vs `näkimkemittä` | plural-only `-imet`; Wiktionary confirms short form |
| `33*J` | 76 | 627 | `antimitta` vs `antimemitta` | plural-only `-imet`; Wiktionary confirms short form |
| `34*C` | 25 | 770 | `levottomani` vs `levotonni` | **fixed in the engine**; oracle expects a non-word |
| `32*D` | 21 | 3 | `ikenittä` vs `ienittä` | irregular `ien`; Wiktionary confirms engine |
| `18` | 7 | 582 | `piihin` vs `piihan`/`piihen` | `pii` irregular; Wiktionary confirms engine |
| `49*D` | 6 | 305 | `säkeneiden` vs long `säe-` family | live Voikko accepts engine; needs a Wiktionary entry |
| `49*E` | 5 | 306 | `taipaliinsa` vs `taipaleihinsa` | `taival`; Wiktionary confirms engine |
| `41*E` | 5 | 806 | `hapaansa` vs `havaksensa` | `havas` dual-class; Wiktionary confirms engine |

The 34*C entry is different from the rest: it was a real engine defect. The
class engine attached the nominative possessive to the lemma (`levotonni`) and
also emitted the correct genitive-stem form (`levottomani`). The oracle
confirmed the non-word, so the correct form was reported illegal. The engine
now emits only `levottomani`; the 25 cells moved to `missing` under
`--verify-illegal` because the oracle still expects `levotonni`. See
`docs/harness.md` → Known defects → C.

## Principles

- **The oracle is evidence, not truth.** Omorfi decides what exists; Voikko
  decides what is real; Wiktionary/Kielitoimisto break ties the two oracles
  disagree on. Record the authority for every exception.
- **Never reclassify by surface alone.** A rule names the class, the cell
  shape and the form shape it covers. `levottomani` being right does not make
  every `34*C` nominative possessive right.
- **Prefer fixing the data over adding harness exceptions.** A corrected oracle
  row helps every consumer of the lexicon; a harness exception helps one
  report.
- **A blanket class exemption is a harness bug.** The current
  `WIKTIONARY_CONFIRMED_CLASSES` reclassifies every illegal row in `33*D`,
  `33*J`, `41*C` and `41*I`. That is too broad: a genuine defect in one of
  those classes would disappear. Replace it with per-cell rules.
- **A staleness check is mandatory.** Every exception must still match at least
  one row. When an engine fix or an oracle rebuild removes the disagreement,
  the exception becomes a red test, not permanent dead code.
- **Do not touch the engine for an oracle bug.** The 34*C fix belonged in
  `src/expand/nominals/class_34.rs`; the other rows do not.

## Phase 0 — done

- [x] Fix the `34*C` nominative possessive: build it on the oblique
  `-ttoma-`/`-ttömä-` stem, not the lemma.
  `src/expand/nominals/class_34.rs`.
- [x] Add the regression test
  `nominative_possessives_sit_on_the_oblique_stem` in
  `src/expand/nominals/class_34C.rs`.
- [x] Reclassify the known oracle non-word in the report tool:
  `GENITIVE_STEM_POSSESSIVE_CLASSES` in `tools/report_by_class.py`.
- [x] Update `docs/harness.md` and `TODO.md` with the new totals.

## Phase 1 — make the evidence a data file

Today the evidence lives in three places: `WIKTIONARY_CONFIRMED_CLASSES`,
`WIKTIONARY_CONFIRMED`, and the new `GENITIVE_STEM_POSSESSIVE_CLASSES`. Move it
into one committed file that both the report tool and (eventually) the oracle
builder can read.

Proposed file: `data/oracle_exceptions.json`. It is curated evidence (rule
ids, URLs and notes), not a copy of the dictionary data; add a short provenance
note beside it so it is clear what the file is and where each source came from.

```json
{
  "version": 1,
  "rules": [
    {
      "id": "34c-nom-possessive-genitive-stem",
      "classes": ["34*C"],
      "cell": {"case": "NOM", "possessive": "*"},
      "form": "oblique-stem",
      "source": "https://en.wiktionary.org/wiki/levoton",
      "note": "levottomani, not levotonni; the possessive is on the -ttoma- stem"
    },
    {
      "id": "plural-only-33d-imet",
      "classes": ["33*D"],
      "cell": {"case": "*", "number": "PL"},
      "form": "short-imet",
      "source": "https://en.wiktionary.org/wiki/n%C3%A4kimet",
      "note": "näkimissä/näkimittä, not the oracle's näkimkemittä family"
    }
  ]
}
```

Requirements:

- [x] Define the schema and the allowed `form` predicates in
  `tools/illegal_evidence.py`:
  - `lemma`: the class/lemma filter is the whole rule.
  - `also-class`: the lemma also belongs to one of `also_classes` (the
    dual-class words).
  - `oblique-stem`: the engine form begins with the class's oblique stem.
  - `shorter-than-expected`: the engine form is a proper subsequence of one of
    the oracle's expected spellings (the plural-only `-imet`/`-aat` caveat).
- [x] Add a loader in `tools/report_by_class.py` and remove the three ad-hoc
  constants.
- [x] Add `tools/test_illegal_evidence.py`:
  - run the default report on the fixed 5-lemma sample;
  - assert every `illegal` row is matched by exactly one rule;
  - assert every rule matches at least one row (staleness);
  - fail on an unknown rule id or an unmatched row.
- [x] Make `--verify-illegal` print the rule id in each reclassified row's
  `detail`, so the report itself says why a row is not a defect.

Acceptance: the default report's 538 rows are all explained by rules; a new
unexplained illegal row fails `tools/test_illegal_evidence.py`. **Met:**
`python3 tools/test_illegal_evidence.py --report reports/noun-classes.json`
prints `538 illegal rows, 17 evidence rules, OK`.

## Phase 2 — audit the remaining ten classes

Each class gets the same treatment: re-read the disagreement, check it against
an external authority, then either fix the engine or add a rule.

### 2.1 Plural-only families (`33*D`, `33*J`, `41*C`, `41*I`)

These are the largest block (328 rows). The documented caveat is that the
materialised oracle marks the longer spellings `both` and the short
Wiktionary/Kielitoimisto spellings `omorfi`.

- [ ] Re-check one representative lemma per class against Wiktionary:
  `näkimet` (`33*D`), `antimet` (`33*J`), `rattaat` (`41*C`), `maltaat`
  (`41*I`).
- [ ] Write a per-cell rule that covers the short forms only. Do **not** keep
  the current whole-class exemption.
- [ ] Add the reverse check: the engine must not start offering the long
  `-kem-`/`-aa-` spellings as `Plain`.
- [ ] If a class has a member with a genuinely different paradigm, split the
  rule by lemma and record it.

### 2.2 `51` (compound/dual-class)

141 rows, the largest single class. `omakuva` is both a class-51 word
(`omatkuvat`) and a class-10 word (`omakuvat`); the oracle's expected
`omittakuvitta` looks like a head/tail split artifact.

- [ ] List the distinct lemmas in the 141 rows. All 141 are `omakuva` in the
  current sample; `täysikuu` is the other documented dual-class word and needs
  the same check.
- [ ] Check each against Wiktionary; record the two valid declensions.
- [ ] Decide whether the engine should emit both paradigms or whether the
  report should accept the class-51 reading only.
- [ ] Add a per-lemma rule; do not exempt every class-51 word.

### 2.3 The irregular and dual-class words

- [ ] `18` `pii` / `tii` / `pe-tsai`: verify the illative against Wiktionary,
  then encode the `pii` rule.
- [ ] `32*D` `ien`: verify `iken-`/`ien-` forms; the engine's `ikenittä`
  family is the one to keep.
- [ ] `41*E` `havas`: verify both declensions and record which cells belong to
  which.
- [ ] `49*D` `säen`: add a Wiktionary citation; the current evidence is live
  Voikko only.
- [ ] `49*E` `taival`: verify the illative plural family; Wiktionary is
  already recorded.

Acceptance for phase 2: every one of the 538 default rows is covered by a
rule with a source, and the `--verify-illegal` report is still 0.

## Phase 3 — tighten the harness

- [x] Replace `WIKTIONARY_CONFIRMED_CLASSES` with the per-cell rules from
  phase 1. The four plural-only classes are no longer exempt as a block.
- [x] Replace the per-`(class, lemma)` `WIKTIONARY_CONFIRMED` entries with
  per-form or per-cell entries. The evidence rules name a cell or a lemma
  shape rather than exempting the whole lemma.
- [x] Replace `GENITIVE_STEM_POSSESSIVE_CLASSES` with the
  `34c-nom-possessive-genitive-stem` rule.
- [ ] Add `--strict-illegal`: do not reclassify anything, and print the rule
  that *would* have applied. This is the audit mode for CI and for reviewing a
  new rule.
- [x] Make `--verify-illegal` fail if live Voikko is unavailable, rather than
  silently falling back to the materialised oracle. A verified run that
  silently did not verify is worse than no run.
- [x] Until phase 5, keep the default mode raw: it is the oracle's opinion,
  not the curated one. The curated rules apply only when the report is asked
  for them. Phase 5 flips the default once the evidence file is complete and
  the staleness test is green.

Acceptance: `--strict-illegal` lists exactly the 538 rows and their rule ids;
no rule is a whole-class exemption. The default report applying the rules is
phase 5.

## Phase 4 — widen the sweep

The current 0 illegal is a 5-lemma-per-class result. It cannot see a defect in
an unsampled lemma.

- [x] Run `-n 20` per class with `--verify-illegal`; triage every new illegal
  row. The run found **343 illegal across 7 classes** (2,896 lemmas checked;
  289,704 confirmed). The evidence file now covers the whole default n=20
  report (2,003 rows) and the verified run is 0 illegal.
- [x] Run `-n 1000` (or all lemmas) per class with `--verify-illegal` and
  `--raw-illegal`; the full raw report is covered by the 40 rules and the
  verified pass is 0 illegal.
- [x] For every new class/lemma, either fix the engine or add an evidenced
  rule. The class-50 compound engine was fixed rather than whitelisted:
  bound combining-form tails (`‑kulmio` -> `suorakulmion`) and plural-only
  tails (`vanhemmat` -> `isovanhempien`) now inflect through the head's own
  paradigm, pinned by two tests in `src/expand/mod.rs`. The n=20 additions are:
  | class | rows | rule | source |
  |---|---:|---|---|
  | `51` | 1,091 | `compound-51-fixed-prefix` + `lemma-51-raskassarja` + `lemma-51-omakuva-nom-poss` | Wiktionary `omakuva`/`raskassarja`, live Voikko |
  | `6` | 95 | `plural-only-6-short-imet` | Wiktionary `simmarit` |
  | `48` | 88 | `plural-only-48-short-eet` | Wiktionary `rääppeet` |
  | `7` | 76 | `lemma-7-suitset` | Wiktionary `suitset` |
  | `33*C` | 76 | `plural-only-33c-short-imet` | Wiktionary `kiittimet` |
  | `18` | 7 | `lemma-18-koi` | Wiktionary `koi` |
  | `50` | 10 | `compound-50-prefix-fixed` | live Voikko |
  | `5*D` / `7*D` | 10 | `lemma-5d-7d-laki` | live Voikko |
  | `48*E` | 1 | `lemma-48e-live` | fi.wiktionary `live` |
  | `9*B` | 1 | `lemma-9b-hipat-nom-pl` | Wiktionary `hipat` |
- [ ] Record the full-lexicon counts in `docs/harness.md`; do not replace the
  5-lemma baseline until the full run is reproducible.
- [ ] Check the `unsupported` rows at the same time. They are not `illegal`,
  but an unbacked `Plain` form is the same user-facing risk. Track them
  separately; do not conflate the two.

The rules discovered at n=20 are marked `"sample": 20`, so the 5-lemma
evidence gate does not call them stale. The class-51 block is structural:
`fixed-prefix` finds the compound modifier (`kuiva` in `kuivakakku`, `oma` in
`omakuva`, `hieno` in `hienosokeri`) and accepts the engine form when the
oracle has inflected that modifier into the middle of the word. `raskassarja`
is the one class-51 compound whose modifier has legitimate genitive-plural
variants (`raskasten`/`raskaiden`/`raskaitten`), so it keeps a lemma rule.

`hipat` was the example of the plural-only lemma gap, and it is now fixed in the
engine: class 9*B declines the dropped-`t` base, strong-graded back to `hippa`,
so `hipat` reaches `hippoja` and `hippojen`. The evidence rule is the
class-scoped `plural-only-t-stems`.

The default n=20 report's 2,003 illegal rows are all covered by the evidence
file; `tools/test_illegal_evidence.py --report reports/noun-classes-n20.json
-n 20` passes. The raw n=20 report is 287,763 confirmed, 3,534 missing, 2,003
illegal and 2,334 unsupported; the default report reclassifies 238 of the
missing rows as `oracle-only` (289,970 confirmed, 3,296 missing, 13,810
not plain, 2,130 unsupported).

### n=100 slice

The `-n 100` run found **6,271 raw illegal rows** across 30 classes (861,260
confirmed); the verified run had 1,356 before the new rules. The extra raw rows
are the case-less possessive readings live Voikko confirms. The evidence file
now covers the whole raw n=100 report:

- class 51 compounds: `fixed-prefix` for the modifier-in-the-middle spellings,
  `case-less` for the NOM possessive readings, and the
  `raskassarja`/`vierasmies` modifier-genitive variants;
- classes 41, 1, 41*A, 33, 33*F: short plural-only forms;
- classes 7, 39, 6, 48, 33*C: plural-only `-t` stems;
- class 18 `-i` loanwords: illative `-hin` and NOM genitive readings;
- classes 9, 9*A, 12, 13, 50: plural-only NOM readings;
- classes 9, 9*A, 9*B, 9*F, 12, 13, 14*A: `plural-only-t-stems`, the whole
  plural of a `-t`-final pluralia tantum (`aktiivat`, `rintsikat`, `menkat`);
- class 50 compounds: case-less NOM possessive, and the plural-only tail rule
  (`omakustannukset` is in this sample).

The default n=100 report is 868,334 confirmed, 9,687 missing, 931
oracle-only, 27,552 not plain, 0 illegal, 7,611 unsupported.

### Full lexicon

The `-n 1000` pass checked 2,891,112 raw confirmed forms and found **12,179
raw illegal** across 30 classes; the verified run had 1,274. The evidence file
now covers the whole raw report with 40 rules:

- class 39 and 10 plural-only `-t` stems: `shorter` (the engine uses the stem,
  the oracle concatenates the nominative);
- classes 9, 9*A, 10, 12, 13, 14*A: `case-less` NOM readings (instructive
  plural / partitive possessive);
- classes 9, 9*A, 9*B, 9*F, 12, 13, 14*A: `plural-only-t-stems`, every plural
  cell of a `-t`-final pluralia tantum, where the engine declines the dropped
  base (`aktiivoiden`, `rintsikoiden`, `menkkojen`) and the oracle builds the
  plural from the nominative plural (`aktiivatiden`, `rintsikatiden`);
- class 50: `fixed-prefix` for modifier-inflected prefixes (`raskasvety`,
  `raitisilma`, `pitkäperjantai`), `plural-only-50-short` for `isoaivot`,
  `isovanhemmat`, `normaaliolot`, `omakustannukset` and `täysisisarukset`, and
  lemma rules for the head variants (`suorakulmio`, `suoraommel`,
  `vapaataival`, `mustatorvisieni`);
- class 5 NOM case-less compounds (`silkkivuori`, `tikkivuori`, `villiviini`);
- class 48 `kiiru`;
- class 18 `-i` loanwords: `sei`, `lei`, `paksoi`, ...;
- class 10 partitive plural possessives (`pässinpökkimät`, `bänät`,
  `oltavat`).

The raw full report is 2,891,112 confirmed, 24,986 missing, 12,179 illegal,
26,958 unsupported; all 12,179 are covered, and the default full report is
**2,904,774 confirmed, 19,787 missing, 5,199 oracle-only, 72,540 not plain,
0 illegal, 25,475 unsupported**. The verified run's 1,274 illegal
rows are also all covered, so `--verify-illegal` is 0 illegal at full lexicon.


### The `missing` side

`missing` is the recall direction: the oracle confirmed a form and the engine
does not produce it. The raw number is dominated by **alternative spellings**:
the oracle confirms several spellings for one cell (`kevytsinkoilta` and
`kevytsingoilta`, `ommel` and `ompele`, `säen` and `säkene`), and the engine
produces one of them. Counting each spelling separately made **81%** of the
5-lemma `missing` rows alternative spellings of a cell the engine already
covers.

`drop_covered_missing` in `tools/omorfi_harness.py` now counts per cell: a
cell is `missing` only if the engine produces **none** of its confirmed
spellings. At the 5-lemma sample that takes `missing` from 6,808 to **1,709**
(recall 93.4% -> 98.3%), and it is what stops the class-50/51 compound rows
(`kevytsinkoilta` against the engine's `kevytsingoilta`) counting as gaps.

What remains is two kinds, and the evidence file handles the ones the oracle
gets wrong:

- **Comparative strong-grade in weak cells.** `-mpi` takes the weak `-mm-`
  grade in the open-syllable cases (`kuuromman`, `kuurommalta`,
  `alamaisemmalta`), and the oracle sometimes keeps the strong `-mpa-` there
  (`kuurompalta`, `alamaisempalta`), which is not a word. Where the cell has
  no engine form at all, `comparative-strong-grade-in-weak-cells` reclassifies
  the row as `oracle-only`.
- **Plural-only `-iset` singulars.** `avajaiset`, `läksiäiset`, `kuukautiset`
  are plural-only (Wiktionary marks them `{{monikollinen}}` with `yks=-`), so
  the engine keeps only the plural cells; the oracle still claims the
  singulars, and `plural-only-38-iset` reclassifies them.

The rest is real: the class-50/51 compound cells, the remaining degree cells,
and the ordinary class-1/6/10 gaps. The comparative possessives
(`alamaisempattaan`, `olemattomampaaan`) are no longer among them:
`degree_forms` now generates them as `Repaired`.

The `unsupported` direction has its own case. The class-18 `-i` loanwords
(`pii`, `koi`, `täi`, `bi`, `sei`, ...) take the `maa`-type singular obliques
(`piitä`, `piissä`, `piiltä`, `piiksi`, `koita`, `täitä`), and Wiktionary
lists them; Omorfi generates only the plural, so the engine's correct singular
forms had neither oracle and were reported `unsupported`. The rule
`class-18-i-singular` reclassifies the singular cells of those lemmas as
`confirmed`: **1,483 rows** at full lexicon.

### The two class-50 engine fixes

`src/expand/compound.rs` now handles two tail shapes it used to miss, and both
remove garbage forms rather than hiding oracle rows:

- **Bound combining forms.** The sanalista spells a bound head with a leading
  U+2011 (`‑kulmio`, `‑jussi`, `‑mielisyys`). `suorakulmio`, `hullujussi` and
  `matalamielisyys` were not split, fell through to the class-50 model, and came
  back with stems like `suorakulmdin`. The tail is now looked up with the
  hyphen prepended and the hyphen is stripped from every generated form
  (`suorakulmion`, `hullujussin`, `matalamielisyyden`).
- **Plural-only tails.** `isovanhemmat` is `iso` + `vanhemmat`, and `vanhemmat`
  is the nominative plural of `vanhempi`; `omakustannukset`, `normaaliolot` and
  `täysisisarukset` are the same shape. The tail index matches spellings, so it
  never found these and the words fell through to the class-50 model
  (`isovanhemmaeihin`, `omakusdannukseikseen`). A bounded neighbourhood probe
  over the lemmas sharing the surface's leading characters resolves the base,
  and the compound keeps only the base's plural cells (`isovanhempien`,
  `omakustannuksitta`, `normaalioloja`, `täysisisarusten`).

Both are pinned by `a_bound_combining_form_can_be_a_compound_tail` and
`a_plural_only_compound_tail_inflects_from_its_base` in `src/expand/mod.rs`.

A third shape is a **compound-like tail**: `mustaviinimarja` is `musta` +
`viini` + `marja`, and the sanalista lists the middle tail `viinimarja` as a
compound-like row with no inflection of its own, so the class-51 split found no
tail and the word produced nothing at all. The tail is now resolved as its own
class-50 compound (`viini` fixed, `marja` inflecting), and the class-51 first
part inflects as well: `mustanviinimarjan`, `mustaaviinimarjaa`,
`mustassaviinimarjassa`, `mustiinviinimarjoihin`. The acceptance is limited to
a lemma that is class 51 alone, so the dual-class words (`isomaksaruoho` is
`50, 51`) keep their correct class-50 reading instead of gaining a second
paradigm. Pinned by `a_compound_like_tail_is_split_recursively`.

The class-22 loanwords had a harmony bug in the apostrophe illative.
Wiktionary gives `parfait'hen` (front) but `nougat'han` (back), and the harmony
is lexical rather than derivable from the spelling (`parfait` ends in a neutral
`i` after a back `a`). The apostrophe paradigm emitted a fixed `'hen`, so
`nougat'han` was not a word the engine knew. Both apostrophe spellings are now
emitted, like the external `nougathan`/`nougathen` pair, and `check` accepts
`nougat'han` and `parfait'hen`. This does not move the harness counts (the
apostrophe forms are `repaired`).

### The plural-only `-t` engine fix

`aktiivat` is the nominative plural of `aktiiva` (Kotus type 13/katiska:
`aktiivoiden`, `aktiivoitten`, `aktiivojen`, `aktiivoita`), and the sanalista
lists only the plural. The class-13 engine took that lemma as the nominative
singular, so it built `aktiivatn`, `aktiivata`, `aktiivatiden` and
`aktiivatja` from the plural nominative. The same shape held for `perskat`,
`menkat`, `hipat`, `passiivat` and `rintsikat`.

The fix follows the rule classes 1, 10 and 17 already used: a `-t`-final lemma
declines from the dropped-`t` base, keeps only the plural cells, and the
nominative plural the base produces is the lemma itself. Where the class
gradates, the dropped form is the *weak* grade (`menka`, `hipa`, `rintsika`),
so `Gradation::strongen` turns it back to the strong stem (`menkka`, `hippa`,
`rintsikka`) before the engine runs. `plural_only` in
`src/expand/nominals/utility.rs` is the shared filter.

This fixed the classes `9`, `9*A`, `9*B`, `9*F`, `12`, `13` and `14*A`.
Classes `2`, `3`, `5*G`, `16*H`, `18`, `20` and `38` still treat a `-t`-final
lemma as a singular (`bileet` -> `bileetn`, `avajaiset` -> `avajaisen`,
`juomingit` -> `juomingitn`); those are the remaining engine work.

Pinned by `a_plural_only_lemma_declines_from_the_dropped_stem` in
`class_9.rs`, `class_12.rs` and `class_13.rs`.

Acceptance: `--verify-illegal` reports 0 illegal on the full lexicon, and the
default report's illegal rows are all covered by the evidence file.

## Phase 5 — default gate and CI

- [x] Make `report_by_class.py` apply `data/oracle_exceptions.json` in the
  default mode, so the gate's illegal count is 0 without `--verify-illegal`.
  `--raw-illegal` keeps the oracle's raw rows for the evidence gate.
- [x] Keep `--verify-illegal` as the independent live-Voikko recheck of the
  curated rules.
- [x] Add `tools/test_illegal_evidence.py` to the pre-merge gate (`AGENTS.md`).
- [x] Add a short `docs/harness.md` section describing the evidence file and
  the difference between `illegal`, `oracle-disagreement` and `unsupported`.
- [x] Update `TODO.md` when the default count reaches 0.

The default report now applies the evidence: n=5 is **96,738 confirmed, 8,704
missing, 0 illegal**; n=20 is **289,976 confirmed, 20,933 missing, 0 illegal**.
`--raw-illegal` still gives the oracle's 538 and 2,003 rows, and the evidence
gate runs on those.

Acceptance: both modes report 0 illegal; the evidence test is green; a
deliberately introduced wrong form still fails the default report.

## Non-goals

- Reducing `missing` or `unsupported`. Those are separate plans; the 34*C fix
  moved 25 cells from `illegal` to `missing`, which is the correct trade but
  not a recall improvement.
- Changing the `Plain`/`Repaired`/`Guessed` policy. `illegal` is about forms
  already offered as `Plain`.
- Whitelisting a surface form without a class, cell and source.
- Rebuilding the 657 MB oracle as the first step. The evidence file is cheaper
  and reversible; correcting the oracle at build time can follow once the
  rules are stable.

## Verification commands

```bash
# The two report modes.
python3 tools/report_by_class.py -n 5 -j 8 -o reports/noun-classes.json
python3 tools/report_by_class.py -n 5 -j 8 --verify-illegal \
    -o reports/noun-classes-verified.json

# Triage.
python3 tools/summarize_class_report.py reports/noun-classes.json

# One class at a time.
python3 tools/report_by_class.py -c '34*C' -n 5 -j 8 --verify-illegal

# The evidence gate (phase 1).
python3 tools/test_illegal_evidence.py

# The engine gates.
cargo test --release --workspace
cargo clippy --all-targets -- -D warnings
```

Note: `tools/test_reverse_lookup_recall.py` currently fails on a stale
reference database built with the old `--engine` default (3,075,081 pairs
against a 626,201 clean baseline). Rebuild it with `--no-engine` before using
it as a gate; it is unrelated to the illegal-form work.
