# TODO

A running list of gaps and known issues in the torille morphology engine,
ordered by user-facing impact. Update as items are done.

The closed-spelling table cleanup has its own checklist:
[`TODO_CLOSED_SPELLINGS.md`](TODO_CLOSED_SPELLINGS.md).

## High priority

Ordered by user-facing impact, which is not the same as gap size: an engine that
offers a non-word has a user-visible bug, and an engine that is merely silent
does not. The precision sweep is closed (see the `34*C` entry below); the first
open item is now the prose sample.

**Current full 5-lemma noun report (2026-11-01):** 96,908 confirmed, 1,041
missing, 244 oracle-only, 4,041 not-plain, 0 illegal, 0 mistagged, 646
unsupported; recall 98.9%.
The raw oracle (`--raw-illegal`) is 96,239 confirmed, 595 illegal and 1,285
missing, all covered by `data/oracle_exceptions.json`. With `--verify-illegal`
(live Voikko plus the curated rules) the report is: **96,908 confirmed, 0
illegal**, 4,041 not plain, 0 mistagged, and 646 unsupported. The 25 `34*C`
cells that used to be illegal now show as `missing`: the oracle expects
`levotonni`, which is not a word, while the engine emits the genitive-stem
`levottomani` that en.wiktionary confirms. The older per-item counts below
predate the current class-engine round and are kept only where the item is
still open.

- [x] **Stop offering whole classes built on the wrong stem**
  Done for the current sample: the default report and `--verify-illegal` both
  report **0 illegal**. The last genuine defect was `34*C`, where the class
  engine attached the nominative possessive to the lemma (`levotonni`) while
  also emitting the correct genitive-stem form (`levottomani`); the oracle
  confirmed the former, so the latter was reported illegal. The possessive now
  sits on the oblique stem (`src/expand/nominals/class_34.rs`), so `levotonni`
  is not emitted, and the same 25 cells show as `missing` because the oracle
  still expects the non-word. en.wiktionary's `levoton` paradigm gives `levottomani`,
  `levottomasi`, `levottomamme`, `levottomanne`, `levottomansa`.
  The remaining default-oracle rows are the documented caveats; the plan for
  removing them from the default report is `TODO_ILLEGAL_REMOVAL_PLAN.md`:

  | class | illegal | confirmed | note |
  |---|---|---|---|
  | `51` | 141 | 628 | compound oracle disagrees about head/tail order |
  | `41*C` / `41*I` | 88 / 88 | 651 / 637 | plural-only `-aat` rows; oracle caveat |
  | `33*D` / `33*J` | 76 / 76 | 319 / 627 | plural-only `-imet` rows; oracle caveat |
  | `32*D` | 21 | 3 | rare `ien` paradigm |
  | `18` | 7 | 582 | `pii` illative/nominative mismatch |
  | `41*E` | 5 | 806 | `havas` dual-class stem |

  The `33*D`/`33*J` and `41*C`/`41*I` rows are the documented plural-only
  caveat: `data/kotus_nomines.sqlite` marks the longer spellings `both` and the
  short, Wiktionary/Kielitoimisto spellings `omorfi`. For `näkimet`, en.wiktionary
  gives `näkimissä`, `näkimittä`, `näkimiltä` and the other short forms, and
  rejects the oracle's `näkimkemittä` family. The same page gives `antimet`'s
  possessive column as `antimieni`, `antimistani`, `antimissani`, not
  `antimemieni`. Wiktionary also gives `rattaat` as `rattaissa`/`rattaitta`
  (not `rattaaissa`/`rattaaitta`), and `omakuva` has two valid declensions
  (type 51 `omatkuvat` and type 10 `omakuvat`). Verify against Wiktionary
  before changing those classes. `51` needs the same external check; its
  expected forms look like oracle artifacts rather than engine errors.

  Recent rounds removed the singular translative possessive columns that hung
  off the wrong stem in classes `24`, `31`, and `36` (`kuorikseen` for
  `kuorekseen`, `haaksikseen` for `haahdekseen`, `vanhimmikseen` for
  `vanhimmakseen`). They also put the class-5 native twin's nominative and
  genitive possessives on the e-stem (`vyyhteni`, `kuoreni`, `appeni`), which
  clears `5*B`, `5*F`, `7*B`, `7*F`, `24`, `26`, `31`, and `36` in the sampled
  sweep. Most of what remains is oracle disagreement: live Voikko accepts the
  engine's `33*D`/`33*J`, `41*C`/`41*I`, `49*D`/`49*E`, `51`, and `32*D` forms
  and rejects the oracle's expected spellings, so do not change those classes
  against SQLite alone.

  An `n=20` sweep (2,896 lemmas) found 343 illegal rows across 7 classes; the
  evidence file covers the whole default `n=20` report (2,076 rows), and the
  verified run is 0 illegal. An `n=100` sweep found 6,424 raw illegal rows
  across 30 classes; the evidence file covers all of them, and the default
  `n=100` report is 0 illegal (868,695 confirmed, 7,781 missing, 1,908
  oracle-only, 7,611 unsupported). The full-lexicon (`n=1000`) raw report has 12,332 illegal rows; the
  evidence file covers all of them with 46 rules, so the default full report is
  0 illegal (2,905,135 confirmed, 17,881 missing, 6,176 oracle-only, 25,475
  unsupported). The evidence file now curates the `missing` direction too: the
  oracle's strong-grade comparative spellings (`kuurompalta` for `kuurommalta`,
  `alamaisempalta` for `alamaisemmalta`) are not words, so they are
  `oracle-only` rather than recall gaps. The class-50 compound engine now
  inflects bound heads (`‑kulmio` -> `suorakulmion`) and plural-only tails
  (`vanhemmat` -> `isovanhempien`), so those lemmas no longer fall through to
  the class-50 model; see `src/expand/compound.rs`. The plural-only `-t` engine
  now declines classes 9, 9\*, 12, 13 and 14\*A from the dropped base
  (`aktiivat` -> `aktiivoiden`, `rintsikat` -> `rintsikoiden`,
  `menkat` -> `menkkojen`). Classes 2, 3, 5\*G, 16\*H, 18, 20 and 38 still
  treat a `-t`-final lemma as a singular; that is the remaining engine issue.
  A class-51 head is no longer required to be a lemma in the bare nominative:
  a participle head (`särkynyt` -> `särkyneensydämen`) and a head in its
  nominative plural (`isot` in `isotaivot`, `pitkät` in `pitkäthousut`) both
  inflect, and a plural head makes the compound plural-only. A row with no
  class at all is a compound and now inflects through `compound_head_forms`
  (`pikkuaivot` -> `pikkuaivojen`, `aamuhartaus` -> `aamuhartauden`), as
  `Repaired`; `PLAN_COMPOUND.md` has the phases that make those rows
  first-class. The remaining work is in `TODO_ILLEGAL_REMOVAL_PLAN.md`.

  Evidence: `docs/harness.md` → Known defects → C.

- [ ] **Recognise the prose sample's real forms and stop the bad suggestions**
  A running-text sample turned up a cluster of false negatives and three
  suggestions that replace a reported real form with something else. These are
  not words to whitelist: each points at a compound, a derivation, or a
  verb/noun form the engine does not reach yet.

  First slice fixed `turhamielisyyksissäni`, `kevytmielisyydestä`,
  `molemmin`, `kosteahkoa`, `hoilaus`/`hoilausta`,
  `suonenvedontapaisesti`, `kiulullinen`, `salaperäisemmäksi`,
  `kahvikekkereitä` (`kekkerit` is now plural-only),
  `limonadipullojen`, `halveksivasti`, `valittavasti`, `loitommalla` and
  `ympyröinti`; the old `kekkeritn` is rejected and not suggested.

  Still open, and confirmed by Voikko/Omorfi: `kiilaili` (verb derivation),
  `kyynelettömin` (privative `-tön` and its possessive/instructive forms),
  and `liikahtelua` (deverbal action noun). Still unconfirmed — check these
  before treating them as gates: `saunoittaa`, `hurjistelu`, `narrineen`
  (probably `narreineen`), `herpaisevan`, `harjautuneet` and `ilmeenny`.

  Wrong suggestion direction still open: `harjautuneet` → `harjaantuneet`
  and `ilmeenny` → `ilmesty`. `ympäröinnyt` was a misspelling of
  `ympyröinti`, which is now recognised.

  Confirm each against `tools/check_word_against_omorfi.py` / Voikko before
  turning the list into a gate. The likely owners are the verb-derivation
  and deverbal-noun classes (`kiilaili`, `liikahtelua`), the privative
  adjective rule (`kyynelettömin`), and compound expansion for any remaining
  compound. Do not add another suffix strip to `is_known_word`; make
  `find_bases_with` and `inflect_all` reach the form so the whole class
  gains it.

- [ ] **An oblique form that is not `Plain` gets no possessive**
  **14026 missing forms across 101 classes**, the largest gap left. Most of it
  is not a possessive bug: `add_possessives` hangs a possessive off every
  `Plain` oblique and skips the rest, so an oblique the engine is unsure of
  takes its whole possessive grid with it. Of the 1659 (lemma, case) pairs with
  a gap, 1092 have a bare oblique that is produced but not `Plain`, 495 have one
  that is `Plain` but on the wrong stem (see the item above, `allergiko` vs
  `allergikko`), and 72 have no bare form at all (`joki` has no essive).

  The `demote_contested` rule added in an earlier round is what does this, so
  the fix is in `src/grammar/noun.rs`, not in the possessive code. It gates the
  next item.

  Evidence: `docs/harness.md` → Known defects → A.

- [ ] **Offer a possessive off every plain spelling of a slot, not the first**
  **4359 missing forms across 92 classes** (GEN 1962/82, ILL 2397/86, PAR
  1117/60, NOM 740/54). The slot loop takes the first `Plain` spelling for a
  slot and hangs the possessive off that alone; when a class legitimately has
  more than one, the rest are real words that are never offered —
  `tarjoiluitteni` and `tarjoiluideni` are both wanted and only `tarjoilujeni`
  is offered.

  Deleting `.take(1)` was tried and **measured to be a net loss** (ten classes:
  missing 1241 → 1078, illegal 385 → 596). The cause is that some plain slot
  spellings are removed by the demotion passes *after* the possessives are
  built, and their possessives survive as orphans — `valtimoom` never appears in
  the output but `valtimoommemme` does. So this is an ordering change, not a
  filter: build the possessives after the passes that can remove their base,
  which is why the previous item gates it.

  Evidence: `docs/harness.md` → Known defects → B.

- [ ] **`taloni` has no case**
  **3348 `mistagged` across 103 classes.** `taloni` is genuinely two words:
  torille gives it a nominative singular and a genitive singular reading,
  `Form::union` keeps a field only where the readings agree, so the case is
  absent. Whether that is a defect or a harness that is too strict is a
  question about `acceptable`, not about the engine — decide which before
  touching it. The count grew with the possessive work because there are more
  possessives now to be mistagged.

- [x] **Build the possessive paradigm for every case**
  Done. The diagnosis was that one question — *which cells get a possessive* —
  was being answered by two code paths that disagreed: a gated block in the
  slot loop (`Slot::Nom | Slot::Gen | Slot::Ill`, the three singulars) and an
  ungated one over `oblique_forms`. The gate now covers all seven slots that
  take a possessive, the plural nominative excepted (`talotani` is not a word),
  and the ungated site no longer produces instructive or comitative
  possessives (`taloini`, `taloineeni`). Three rules had to come from the oracle
  rather than from theory: the **partitive takes the suffix as it stands**
  (`taloa` → `taloani`; the partitive ending is not displaced, and the first
  attempt that stripped the trailing `a` was wrong in 32 of 186 sampled
  lemmas), the **translative spells its elided `i` out as an `e`** (`taloksi` →
  `talokseni`, 3380 forms over 117 classes), and the **third person has two
  spellings and Finnish uses both** (`taloaan` and `taloansa`), so both are
  emitted. A fourth judgement call: dropping a case `n` before the suffix
  (`talon` → `taloni`) is `Plain`, not `Repaired` — only a case ending goes, the
  stem is untouched — and that is why `talooni` is now suggestible.

  Full 146-class sweep: confirmed 32613 → 49685, missing 38225 → 21807, not
  plain 2402 → **0**, unsupported 2512 → 1341, illegal 15390 → 15431,
  mistagged 2377 → 3348. Recall 46.0% → **69.5%**, `illegal` rate 47.2% →
  31.1%. `talo` is now 0 missing, 0 illegal, 0 not-plain.

  Evidence: `docs/harness.md` → Known defects.

- [x] **Promote correct oblique cases to suggestible**
  Done. `src/grammar/noun.rs` judges every oblique form twice before marking it
  `Plain`: the stem reading has to be one this lemma has (`readings_for`, which
  drops a reading whose `suffix` or shape the lemma lacks, and drops the
  lemma-as-stem when the class has a real rule), and the ending has to be the one
  that stem licenses (`Licence`). A case that two certain readings compete for is
  demoted to `Guessed` (`demote_contested`), and so is the prolative, whose forms
  are a fact about the word and not about its class. `talossa` and `vastauksella`
  are offered; `ovessa`, `paperissa` and `talotse` are accepted as words and never
  suggested.

- [x] **Describe the three stems of class 40**
  Done. Class 40 is `with_essive`: `kalleude` in the genitive and weak cases,
  `kalleute` in the essive, and the plural `kalleuks` already read off the
  catalogue. The `-ys` words (`ystävyys` → `ystävyyde`/`ystävyyte`) are covered
  too.

- [x] **Build the plural inessive and elative on the plural stem**
  Done. The plural stem is derived from the class (`plural_stems`): for the
  fixed-stem classes it is the fixed stem with its theme vowel off (`sisimmä` →
  `sisimm`), and for the a-to-o classes it is the a-to-o of the singular stem
  (`omena` → `omeno`, `solakka` → `solako`/`solakko`). `vastauksissa`,
  `sisimmissä`, `omenoissa` are all offered.

- [x] **Wire the case and verb models into the generator, or remove them**
  Done. The standalone `case.rs` `CaseRule` machinery and `src/verb.rs`
  (`VerbStem`, `VerbOverride`, `analyze_verb`) were deleted; the generator is
  driven by `noun_cases()`/`oblique()` and the `verb` table.

## Verb classes

The verb side is being rewritten class by class in `src/expand/verbs/`, one
hand-written module per Kotus type, the way `src/expand/nominals/` does the
nouns; each module works out the stems its type gives and `utility::paradigm`
applies the endings every verb shares. `tools/audit_verbs.py` measures a class
against `data/kotus_verbit.sqlite`, and the target per class is `0 invented,
0 missing` against the `both` oracle (`union` is the looser real-precision
claim, since `both` is empty for rare words).

**The numbered range is finished: 52 to 78 is every numbered verb type the
reference has**, and each has a hand-written module reaching it. `kotus-tyypit.txt`
ends at 78 and no `base.class` carries a number below 52, so the remaining verb
work is the variants and the flags rather than another number: the `*`-classes,
the two-reading lemmas, and classes `99` and `100`, which are the sanalista's
*taipumaton tai vaillinaisesti taipuva* marker and not paradigms (see the
`hälyyttää` entry, which is the one verb the engine cannot inflect at all).

- [x] **Classes 52 and 53 (type 1), with gradation**
  Done. `52` (`puhua`, `sanoa`) and `53` (`aakkostaa`, `muistaa`) share one
  paradigm and differ only in the stem and the passive theme; the gradation code
  the sanalista writes after the class (`52*A`, `53*C`) is threaded through
  `expand::verbs::dispatch` and applied in the fixed cell pattern type 1 gradates
  in. Over all 5084 lemmas of `52*`/`53*` the audit is **0 invented and 0
  missing against `both`** apart from the artifacts listed below.

- [x] **Class 54 (the `huutaa` type)**
  Done. Its present is class 53's — the infinitive less its final vowel, the
  third person doubling the theme vowel and the passive retheming the weak grade
  to `e` — and what it adds is a past of its own: the stem less its theme vowel
  with that final consonant replaced by `s` (`huuta` -> `huus`, `rakenta` ->
  `rakens`, `askelta` -> `askels`), which every past cell takes unchanged,
  gradation and all. `Stems::with_single_past` is what says so. All 327 lemmas
  of `54*` audit at **0 invented and 0 missing against `both`**; the one
  disagreement, `örveltää`, is Omorfi's — Kotus and Wiktionary make `54*I`
  `lt` -> `ll` (`örvellän`, `örvelletty`) and Omorfi writes `örveldä`,
  `örveldetty`.

- [x] **Class 55 (the `soutaa` type)**
  Done. Its present is class 53's — the infinitive less its final vowel, the
  third person doubling the theme vowel and the passive retheming the weak grade
  to `e` — and its past is the only one of the type-1 classes with *two* stems,
  both correct: class 53's weak-grade past (`soudin`, `souti`, `soutivat`) beside
  class 54's `s` past (`sousin`, `sousi`, `sousivat`). The module emits the union
  of the two paradigms and merges the shared cells, since they agree on
  everything but the past. All 7 lemmas of `55*` audit at **0 invented and 0
  missing against `both`**; as in class 54 the only `union`-only form is the
  `-matontaan` cell, an artifact shared by every class.

- [x] **Class 56 (the `kaivaa` type)**
  Done. Its present is class 53's — the infinitive less its final vowel, the
  third person doubling the theme vowel and the passive retheming the weak grade
  to `e` — and its past is the one thing that differs: rather than dropping the
  theme vowel the way class 53 does, it reads it as `o`/`ö` (`kaiva` -> `kaivo`,
  `auta` -> `auto`, `anta` -> `anto`). The third person takes the strong grade
  and the other persons the weak one, so `auttaa` has `auttoi` beside `autoin`.
  `Stems::with_o_past` and `Stems::o_past_stem_of` are what say so. All 50 lemmas
  of `56*` audit at **0 invented and 0 missing against `both`**; the only
  `union`-only form is again the `-matontaan` cell.

- [x] **Class 57 (the `saartaa` type)**
  Done. Its present, passive and participles are class 56's, but its past is
  class 56's `o`-past *and* class 54's `s`-past, both of them correct — Kotus
  names both on one line (`saarsi saartoi`) and the reference carries all twelve
  cells. So `saartaa` has `saartoi` beside `saarroin` and `saarsi` beside
  `saarsin`, and `kaataa` (`57*F`) has `kaatoi`, `kaadoin` and `kaasi`,
  `kaasin`. The module is class 55's union again, over class 56's past instead of
  class 53's. Gradation (`57*F` `t`→`d`, `57*K` `rt`→`rr`) shows in the present
  and the passive only. All 3 lemmas of `57*` audit at **0 invented and 0 missing
  against `both`**; the only `union`-only form is again the `-matontaan` cell.

- [x] **Class 58 (the `laskea` type)**
  Done. Its present, passive and participles are class 53's unchanged — the stem
  is the infinitive less its theme vowel and the third person repeats the stem's
  own `e` (`laskee` beside `lasken`) — and the past differs in *when* it drops a
  vowel. Class 53 drops the infinitive's theme, which for it is the stem's last
  letter (`aakkosta` -> `aakkost`); class 58 drops the stem's own `e` (`laske` ->
  `lask`, `kylpe` -> `kylp`). The two operations commute in class 53, where the
  gradation site sits before the theme, but not here, so the past's weak grade
  is the *past stem* weakened rather than the theme dropped from the present's:
  `hakea` (`58*D`) drops its `e` first (`hak`) and gradates after (`ha`), giving
  `hain` beside `haki`, where the other order would give the non-word `hagin`.
  `Stems::with_e_past` and `Stems::e_past_stem_of` are what say so. The same
  vowel is also what the conditional and the second infinitive attach to, so both
  run on the `i`-stem rather than the theme: `laskisi` and not `laskeisi`,
  `laskiessa` and `laskien` and not `laskeessa`, `laskeen`. That is
  `Stems::cond_stem` and `Stems::inf2_stem`, which every other type sets to the
  present stem and the present stem plus `e`. Gradation is the usual run: `58*D`
  drops the `k` (`hagen`), `58*E` (`kylpeä`) weakens `p` to `v` (`kylven`),
  `58*F` (`kutea`) `t` to `d` (`kuden`), `58*G` (`tunkea`) `nk` to `ng` (`tungen`)
  and `58*L` (`kulkea`) `k` to `j` (`kuljen`); the 13 plain `58` members gradate
  nothing, so the catalogue model word `laskea` needs no code. All 32 lemmas of
  `58*` audit at **0 invented and 0 missing against `both`**; the only
  `union`-only form is again the `-matontaan` cell.

- [x] **Class 59 (the `tuntea` type)**
  Done. Class 58 and class 54 at once: the stem is the infinitive less its theme
  vowel (`tunte`), the third person repeats the stem's own `e` (`tuntee`), and
  the past is the stem less that `e` with its final consonant turned to `s`
  (`tunt` -> `tuns`), which is `Stems::e_s_past_stem_of`. The past does not
  gradate, as in class 54, so every past cell takes `tuns` alike: `tunsin` and
  `tunsi` side by side with no weak grade between them. `Stems::with_e_s_past`
  and `Stems::e_themed` are what say so, the latter being the constructor classes
  58 and 59 now share: both are the type whose theme is the stem's own `e`, so
  both move the conditional and the second infinitive onto the `i`-stem —
  `tuntisi`, `tuntiessa`, `tuntien`, where class 53's `muistaisi` and
  `muisteessa` hang off the theme. The `i`-stem is the stem less the `e` and not
  the past stem, which in class 59 has also turned its consonant to `s`.
  The class has one member and it gradates: `tuntea` is `59*J`, `nt` weakened to
  `nn`, which is what makes the weak grade `tunne` and so the passive theme
  `tunnetaan` rather than `tuntetaan` — the gradation supplies the `e` that class
  53 has to retheme in. It is also the catalogue's model word under the plain
  `59` and nothing in its shape says which consonant weakens, so `missing_code`
  names it. All 1 lemma of `59*` audits at **0 invented and 0 missing against
  `both`**; as everywhere, the only `union`-only form is `tuntematontaan`.

- [x] **Class 60 (the `lähteä` type)**
  Done. Its present, passive, participles, conditional and second infinitive are
  class 58's unchanged — the theme is the stem's own `e`, so the third person
  repeats it (`lähtee`) and the conditional and second infinitive follow the
  `i`-stem (`lähtisi`, `lähtiessä`, `lähtien`) — and its past has *two* stems,
  both correct. Kotus names them on one line (`lähti (läksi)`) and the
  reference carries all twelve past cells, so the module is the union of the two
  paradigms, as class 57's is: the class-58 `i`-past that gradates (`lähti`,
  `lähtivät` beside `lähdin`, `lähdit`) together with the class-59 `s`-past that
  does not (`läksi`, `läksivät`, `läksin`, `läksit`). The merge keeps the shared
  cells once and adds only the `s`-past's own words. The parenthetical here is
  not the alternant *class* reading `parse_inflections` drops — `lähteä` has
  the one class `60*F` — but two past stems of that one type, as in class 57.
  The second of the two is lexical rather than derivable: class 59's rule turns
  a stem's final consonant to `s`, which would need `ht` → `ks` to give `läks`
  and the `k` has no source in the stem, so `s_past_stem` gives it per lemma,
  the way the grammars call `mennä` `meni`, `tulla` `tuli` and `lähteä` `läksi`
  irregular. The class has one member, `lähteä` (`60*F`, `t` → `d`), which is also
  the catalogue's model word under the plain `60`, so `missing_code` names it.
  The gradation reaches the present, the passive and the `i`-past's weak cells
  but not the `s`-past, whose stem has no `t` left to weaken. All 1 lemma of
  `60*` audits at **0 invented and 0 missing against `both`**; as everywhere,
  the only `union`-only form is `lähtemattontaan`.

- [x] **Class 61 (the `sallia` type)**
  Done. Class 61 is the `-i` type, and one thing about it is enough to make it a
  type of its own: the theme vowel is *kept*. In classes 52 and 53 the
  infinitive's ending `aa` is a doubled theme, so the stem ends in the theme and
  the past drops it (`aakkosta` → `aakkost`); here the ending is a single `ia`,
  the stem is `salli`, and it *is* the past stem too — `salli` + `i` gives
  `salli`, because the ending's `i` merges with the one the stem already ends in.
  That is `paradigm`'s `absorb_i`, and it is the whole difference:
  `Stems::with_past` is handed the stem twice. Five of the six present and past
  cells therefore come out as the same word in both tenses, and the reference
  gives no other present or past cell for this type either; the one place the
  tenses part company is the third person singular, `sallii` against `salli`, and
  the third person present is also the one place a `-i` verb repeats its stem's
  last letter. The passive has no `e` of its own to retheme into, because the
  stem's `i` is already the passive's: `sallitaan` is `salli` + `t` + `aan` where
  class 53 gives `aakkostetaan` and class 58 `lasketaan`, so `retheme` finds no
  `a`/`ä` and leaves the weak grade standing. The conditional and the second
  infinitive stay on the theme, as in class 52, since a stem ending in `i` takes
  their endings: `sallisi` is `salli` + `isi` with the `i`s merged, `salliessa`
  and `sallien` are `sallie` + `ssa` and `sallie` + `n`. Only classes 58 and 59,
  whose theme is the stem's own `e`, have to move those two onto the `i`-stem.
  Gradation is the usual type-1 run and reaches the same cells: `61*A` (`kukkia`)
  `kk` → `k` (`kukin`), `61*B` (`oppia`) `pp` → `p` (`opin`), `61*C` (`miettiä`)
  `tt` → `t` (`mietin`), `61*D` (`poikia`) the `k` deleted (`poiin`), `61*E`
  (`sopia`) `p` → `v` (`sovin`), `61*F` (`lehtiä`) `t` → `d` (`lehdin`), `61*G`
  (`hönkiä`) `nk` → `ng` (`höngin`), `61*H` (`empiä`) `mp` → `mm` (`emmin`),
  `61*J` (`sontia`) `nt` → `nn` (`sonnin`) and `61*L` (`hylkiä`) `k` → `j`
  (`hyljin`); the catalogue model word `sallia` is plain `61` and needs no code,
  so there is no `missing_code` table. All 413 lemmas of `61*` audit at **0
  invented and 0 missing against `both`**; the only `union`-only form per lemma
  is again the `-matontaan` cell (`säikkymätöntään` for `säikkyä` is that cell
  plus Omorfi's front/back harmony slip).

- [x] **Class 62 (the `voida` type)**
  Done. Class 62 is class 61 with the `d` left in the infinitive's ending, so it
  keeps the theme vowel the same way and is built the same way: the stem is the
  lemma less the whole `da`/`dä` (`voi`, `aateloi`, `dieselöi`), and it is the
  past stem too, `Stems::with_past` handed the stem twice, so `paradigm`'s
  `absorb_i` merges the past ending's `i` with the stem's own. Every member ends
  in `ida`/`idä`, so the `i` of that ending is always the stem's last letter and
  there is no member to tell apart; the class has no gradation code at all.
  Four cells are where it is not class 61, and three of them are the `t` the
  passive adds to the theme. Every other type-1 theme ends in a vowel, so the
  cluster is geminated (`sanot`/`sanott`, `sallit`/`sallitt`) and that is
  `PassiveCons::Geminated`; class 62 writes one consonant each way instead and
  gives the `d` to the present — `voidaan` where class 61 has `sallitaan`, and
  `voitiin`, `voitava`, `voitu` where class 61 has `sallitt` throughout. Hence
  `PassiveCons::Deranked` and `Stems::p_cons`. The past passive participle then
  deranks in the oblique as well: `voitu` is the nominative and `voidu` the
  oblique stem (`voidun`, `voidussa`), where class 61 halves its own doubled `t`
  instead (`sallittu`/`sallitun`), so `tu_participle` takes the flag. The second
  infinitive runs on the `d` too — `voidessa` and `voiden` are `void` + `essa`
  and `void` + `en`, beside class 61's `salliessa` and `sallien` — which is
  `Stems::inf2_stem` again, the stem plus `de` rather than plus `e`, and needs
  nothing new. The fourth cell is the third person present, and it is not about
  the `t`: class 61 repeats the stem's last letter (`sallii`) because its `-ia`
  infinitive's `i` is a *second* `i`, but here the infinitive's `i` and the
  stem's `i` are one `i`, so the third person is the bare stem (`voi`, `aateloi`,
  and `nai` for `naida`, `ui` for `uida` if it were long enough to be written).
  That is `Stems::pres3sg`, empty for this class and the stem's own last letter
  everywhere else. With it, *all six* present and past cells are the same words
  — `voin`, `voit`, `voi`, `voimme`, `voitte`, `voivat` each a present and a past
  form — and the reference gives no other present or past cell for the type
  either, so unlike class 61 there is no cell where the two tenses part company.
  All 735 lemmas of `62` audit at **0 invented**, and at **0 missing against
  `both`** bar the one form noted below. The `union`-only forms are the
  `-matontaan` cell every class shares, plus one lemma's worth of Omorfi: for
  `pikkelöidä` Omorfi conjugates the auxiliary-like `pikkelöitse`
  (`pikkelöitsee`, `ei pikkelöitse`), which Kotus does not, and it is the only
  class-62 lemma with more than the one `union`-only cell.

- [x] **Class 63 (the `saada` type)**
  Done. Class 63 is the `-ada` type, and it is class 62's `-ida` with the theme
  vowel still doubled where class 62 has it single: `saada` is `saa` + `da`
  beside `voida`'s `voi` + `da`, `jäädä` is `jää` + `dä` beside `dieselöidä`'s
  `dieselöi` + `dä`. That doubled vowel is the whole difference between the
  present and the past, and the only reason the type is a type of its own: the
  present stem is the lemma less its `da`/`dä` (`saa`, `jää`, `myy`) and the past
  stem is that less its final vowel (`sa`, `jä`, `my`), which needs its own
  helper — `Stems::past_stem_of` drops an `a`/`ä` but not the `y` that harmony
  leaves behind in the front vowel. Everywhere else the type is class 62's cell
  for cell, and the
  reference agrees: the passive deranks its `t` to `d` and takes the `d` in the
  present (`saadaan`, `saatiin`, `saatava`, `saatu`, `saadun` — all beside class
  62's `voidaan`, `voitiin`, `voitava`, `voitu`, `voidun`), so it reuses
  `PassiveCons::Deranked`; the third person present is the bare stem (`saa`),
  so `pres3sg` is empty as it is for class 62; and the second infinitive runs on
  the `d` (`saadessa`, `saaden`), so `inf2_stem` is the stem plus `de` as in
  class 62. One cell is its own: the **conditional** follows the *past* stem,
  which is what makes `saisi` (`sa` + `isi`) where class 53's `muistaisi` is
  `muist` + `isi` off the present stem, so `cond_stem` is the past stem; the
  potential and the imperative keep the present stem (`saanee`, `saakoon`) and so
  do not part company, as they do in classes 58 and 59. The passive theme is the
  one thing class 61 and class 62 got by accident and this type has to say: class
  53 rethemes a stem-final `a`/`ä` to `e` (`muistetaan`, `sanotaan`), and
  because this type's stem *ends* in a doubled `a`/`ä` the rule reaches it and
  gives the non-word `saetaan`, so `p_theme` is the weak stem as it stands.
  Three members and no gradation code — `saada`, `jäädä`, `myydä` are all plain
  `63` — so there is no `missing_code` table. All 3 lemmas of `63` audit at **0
  invented and 0 missing against `both`**, and the single `union`-only form per
  lemma is the `-matontaan` cell every class shares: the first class with nothing
  else outstanding.

- [x] **Class 64 (the `juoda` type)**
  Done. Class 64 is the `-oda`/`-ödä` type, and it is class 63's `saada` with a
  connecting vowel standing between the root and the ending's final vowel:
  `juoda` is `jo` + `u` + `o` + `da` beside `saada`'s `saa` + `da`, and `lyödä`
  is `lö` + `y` + `ö` + `dä` beside `jäädä`'s `jää` + `dä`. The present stem is
  the lemma less its `da`/`dä` (`juo`, `luo`, `lyö`, `suo`, `syö`, `tuo`, `vie`)
  and its tail is always two vowels; the past stem is the root plus that tail's
  second vowel alone, so the class needs its own `past_stem_of` to drop a
  penultimate vowel. The connector varies with the harmony — a `u` where the stem
  is back (`juo`, `luo`, `suo`, `tuo`), a `y` or an `i` where it is front
  (`lyö`, `syö`, and `viedä`, whose tail is `ie` rather than `yö`) — which is why
  the rule is stated as dropping a penultimate vowel rather than a fixed letter.
  That is the whole difference between the two tenses: `joi` beside `juon`,
  `löi` beside `lyön`, `vei` beside `vien`. Everything else is class 63's cell
  for cell and the reference agrees: the passive deranks its `t` to a `d`
  (`juodaan`, `juotiin`, `juotava`, `juotu` with its oblique `juodun`), so it
  reuses `PassiveCons::Deranked`; the third person present is the bare stem, so
  `pres3sg` is empty; the second infinitive runs on the `d` (`juodessa`,
  `juoden`); and the conditional follows the past stem (`joisi`, `löisi`,
  `veisi`), with the potential and the imperative keeping the present one
  (`juonee`, `juokoon`). The passive theme needs no retheming here, and that is
  worth recording as a difference from class 63 even though the outcome is the
  same: class 53 reads a stem-final `a`/`ä` as the passive's `e` (`muistetaan`),
  which class 63's doubled `a`/`ä` stem has to switch off explicitly to avoid
  `saetaan`, whereas this type's stems end in `o`, `ö` or `e`, so `retheme`
  never fires and `p_theme` is left as `with_past` set it. Seven members and no
  gradation code — `juoda`, `luoda`, `lyödä`, `suoda`, `syödä`, `tuoda`,
  `viedä` are all plain `64` — so there is no `missing_code` table. All 7 lemmas
  of `64` audit at **0 invented and 0 missing against `both`**, and the single
  `union`-only form per lemma is again the `-matontaan` cell.

- [x] **Class 65 (the `käydä` type)**
  Done. Class 65 is the `-ydä` type, and it is class 64's `juoda` with the
  connecting vowel gradating rather than merely standing: `käydä` is `kä` + `y`
  + `dä` where `juoda` is `jo` + `u` + `o` + `da`, and the past reads that `y` as
  `v` — `kävin`, `kävi`, `kävivät` beside the present's `käyn`, `käy`, `käyvät`.
  So the present stem is the lemma less its `dä` (`käy`) and the past stem is
  that with the stem-final `y` read as `v` (`käv`). The alternation is the
  class's own and has to be written out: every `Gradation` code is a pair of
  *consonants* (`kk <-> k`, `nt <-> nn`, `k <-> v`) and this is a vowel giving
  way to a consonant, so no code in `Gradation::pair` describes it; the sanalista
  also writes class 65 plain, with nothing to read it off. Everything else is
  class 63's and class 64's cell for cell and the reference agrees: the passive
  deranks its `t` to a `d` (`käydään`, `käytiin`, `käytävä`, `käyty` with its
  oblique `käydyn`), so it reuses `PassiveCons::Deranked`; the third person
  present is the bare stem, so `pres3sg` is empty; the second infinitive runs on
  the `d` (`käydessä`, `käyden`); and the conditional follows the past stem
  (`kävisi` is `käv` + `isi`), with the potential and the imperative keeping the
  present one (`käynee`, `käyköön`). The passive theme needs no override here,
  as at class 64: `retheme` fires on a stem-final `a`/`ä` and this stem ends in
  `y`, so the weak grade is the passive theme as it stands — the same position
  class 63 has to override explicitly and class 64 reaches by accident. One
  member, `käydä`, plain `65`, so no `missing_code` table — and worth saying
  plainly that every rule here is read off the reference for a single word: the
  class is a type of its own because the sanalista lists it, not because the
  data shows a second member. All 1 lemma of `65` audits at **0 invented and 0
  missing against `both`**, and the single `union`-only form is again the
  `-matontaan` cell.

- [x] **Class 66 (the `rohkaista` type)**
  Done, and it is the first verb type whose stems are not type 1's at all, so it
  took three pieces of new machinery in `utility.rs` and a module that hands
  `Stems` six overrides. The infinitive is consonant-final, so the present stem
  is the lemma less its `ta`/`tä` with the theme put back (`rohkaise` + `n` is
  `rohkaisen`, and the third person adds the one `e` that `pres3sg_of` already
  reads off the stem) and the past stem is that less the theme (`rohkaisi`).
  What differs from every class above it is which stem the *non-finite* and
  *mood* cells run on. The potential and the imperative's plurals take the **past**
  stem, not the present one (`rohkaissee`, `rohkaiskaa`, where the present stem
  would give `rohkaisekaa`), while the imperative's second person singular stays
  on the present (`rohkaise`) — so the imperative alone straddles the two stems
  and needs its own `Stems::imp_stem`. The infinitives' own consonant fronts the
  potential's `-e` and the past active participle's `-u` alike: `rohkais` + `s`
  is both `rohkaisse` and `rohkaissut`, where type 1 has `sanone`/`sanonut` off
  an `n`. Those are one fact, not two — the class's ending brings its consonant
  with it and the same consonant fronts both cells — so they travel as one
  `utility::Alt`, set once on `Stems::pot` and `Stems::past_part` and defaulting
  to type 1's shape (`Alt::n`) in the other constructors. Hoisting the negated
  past's nominative up to the top of `paradigm` let it quote `past_part` whole
  (`en rohkaissut`) instead of rebuilding `-nut` off the stem, which was the one
  form every lemma of the class was missing. The passive keeps the ending's own
  `t` in the present and drops the gemination in the past, and does not derank
  its `-tu` (`rohkaistaan`, `rohkaistiin`, `rohkaistu` with its oblique
  `rohkaistun`) — a shape neither class 62's nor class 63's passive has, hence
  `PassiveCons::SingleT` as its third variant.

  The gradation runs **backwards** from type 1's, which is why the module reads
  it off the lemma rather than handing `Stems` a code. Type 1's sanalista lemma
  shows the strong grade and the alternation surfaces elsewhere; class 66's shows
  the *weak* one (`häväistä`, `rangaista`) and the finite present, the past, the
  conditional, the agent, the present active participle and the third and fourth
  infinitives all take the strong grade (`häpäisen`, `häpäisi`, `häpäisisi`,
  `häpäisemään`), while the potential, the imperative's plurals, the whole
  passive, the second infinitive and the past participle take the weak one
  (`häväissee`, `häväiskää`, `häväistään`, `häväistessä`, `häväissyt`). So
  `Gradation::strongen` is applied to the lemma's stem to get the strong grade,
  `with_past` is handed that with `gradation: None` — which leaves `Stems::weak`
  equal to the stem, exactly where this type wants the present's four `e`-less
  persons, the imperative's second person and the negated present — and the five
  cells that want the weak grade are then named one by one. Only `E` and `G`
  occur among the 272 lemmas (on `häväistä`, `vavista`, `rangaista`) and both are
  consonant pairs in `Gradation::pair`, so `strongen` covers them and no
  `missing_code` table is needed.

  All 272 lemmas audit at **0 missing against `both`**, and 271 of the 272 at 0
  invented too. The exception is `vavista`, which is worth its own note because it
  is the oracle disagreeing with itself rather than a rule being wrong: Kotus
  spells that lemma's agent `vapisma`, on the *past* stem, where all 271 other
  lemmas have it on the present one (`rohkaisema`) — and the same verb's other
  entry, `vapista`, spells it `vapisema` like the rest, with the present stem,
  and takes `vapisemaan`/`vapisminen`/`vapismaton` from that stem as well. Kotus
  marks all 162 of `vavista`'s agent rows `omorfi`-only where `vapista`'s 162 are
  `both`, so the reference declines to vouch for the `vapisma` spelling. The
  module follows the entry that is vouched for; the cost is that `vavista`'s
  agent declension, 597 forms, counts as invented against `union`. Chasing it
  would mean a per-lemma override for a spelling the oracle itself rejects.

- [x] **Class 67 (the `tulla` type)**
  Done, and it is the first type where the two stems are the *same* stem in two
  grades with the endings sitting straight on them (`tulen` on `tule`, `tulin` on
  `tul`), so the module is six `Stems` overrides and no stem arithmetic beyond the
  theme. What the type's row says in one line is

  ```text
  tulla / tulen / tulin / tulisi / tullee / tulkaa / tullut / tullaan
  ```

  and five things in it are this type's alone.

  **One consonant, and the class fronts it in four cells.** The potential, the
  past active participle, the second infinitive's connective and the past passive
  participle's oblique all put a consonant in front where type 1 has a bare
  vowel: `tul` + `l` + `ee` is `tullee` against `sano` + `nee`, `tul` + `l` +
  `ut` is `tullut` against `sanonut`, `tul` + `l` + `e` + `ssa` is `tullessa`,
  and the `-tu` participle's oblique is `tullu` (`tullun`, `tullussa`) where the
  nominative `tultu` gives no hint of the `l` at all. The first two are the
  `utility::Alt` class 66 introduced, with the lead spelled; the second
  infinitive is `Stems::inf2_stem`; the participle oblique is the one thing no
  `Decl` could carry, so `tu_participle` was split into `tu_participle` and
  `tu_participle_on(nom, oblique)` and `Stems::p_tu_oblique` was added to hand the
  stem over (`None` in all three constructors, so classes 52 to 66 are
  byte-identical).

  **The passive adds nothing in the present.** Its theme already ends in a
  consonant and the ending gives that consonant back where every other type adds
  a `t` of its own: `tullaan` and `aaltoillaan` beside `tultiin` and
  `aaltoiltiin`. `PassiveCons` grew `ThemeEnd` for it, which is the fourth variant
  and the first that is not a fixed letter — `present` takes the theme now and
  reads the letter off its last character, so the same variant covers `tullaan`
  and, as below, `pannaan`.

  **Which stem each cell runs on is the reverse of class 66's.** The finite
  present, the past, the conditional, the imperative's second person and the
  agent take the grade the lemma *does not* show; the potential, the imperative's
  plurals, the whole passive, the second infinitive and the participles take the
  one it does (`tulkaa`, not `tulekaa`; `tullessa`, not `tulessa`). So the module
  hands `Stems` no gradation code at all — `with_past(..., None)` leaves
  `Stems::weak` equal to the stem, which is exactly where this type wants the
  present's four `e`-less persons — and computes the other grade itself with
  `Gradation::strongen` from the lemma's own grade, which works whichever way
  round the sanalista wrote it (`nakella` weak, `ommella` strong). Two codes need
  more: `D` drops its consonant rather than changing it, so `jaella` is the only
  member with it and its strong grade is written out; and `C` is coded on 398
  members, of which `gradation_of` drops it for two — `köpötellä` and
  `narskutella`, whose every `both` row is on the lemma's own single `t` and not
  one of the 1294 rows either of them has shows the `tt` the code calls for.
  Nothing in the lemma tells them apart from `määritellä`, so the two names are
  the whole of the exception.

  **The class is four types, and one letter tells them apart.** Every member's
  infinitive ends in `<stem><consonant><a|ä>`, and the consonant in front of the
  `a`/`ä` is the one the four cells above front. `tulla` gives `tul` and `l`,
  `panna` and `mennä` give `pan`/`men` and `n`, `purra` and `pierrä` give
  `pur`/`pier` and `r`. The `-nna` and `-rra` types are the `panna` and `purra`
  types, which the sanalista files under 67, and they are the same type with a
  different consonant: where `tul` already ends in `l` the infinitive writes it
  once and the endings have nothing to add, where `pan` does not end in `n` they
  supply it (`pannen` beside `panen`, `purtu` beside `pannut`, `pannaan` beside
  `tullaan`). That is `base_of`, which returns the stem and the letter together,
  and it is the only thing in the module that has to know the difference.

  1378 of the 1379 lemmas audit at **0 invented and 0 missing against `both`**,
  and the one left is not this type's — the sanalista's own index files it here.
  `panna` is a noun in it as well (*kaksi pannaa*), so the engine inflects it in
  both parts of speech: its 1182 verb forms are exact, and its 152 noun forms
  have no rows in a table about verbs at all, which is the artifact classes 52's
  and 53's counts are made of. `olla` is the irregular, and its two unreachable
  corners are written out in `fix_olla`: `on` beside `olen` and `ovat` beside
  `olevat`, which no present stem produces at once, and the auxiliary `lie`'s own
  potential (`lienen`, `lienee`, `lienevät`, `lie`), which the reference files
  under `olla`. Its potential runs through `lie`, and its two lexically absent
  corners — the 5th infinitive's possessives (`olemaisillamme`) and the past
  active participle's comitative (`olleine`) — are gaps in Omorfi's own lexicon,
  not the engine's, and the reference has no rows for them. The `union`-only cell
  is again the `-matontaan` one.

- [x] **Class 68 (the `tupakoida` type)**
  Done, and it is the first class that is *another class's* type: the two type
  rows say the same eight cells on two lemmas,

  ```text
  voida     / voin     / voi     / voisi     / voicee     / voikoon     / voinut / voitiin
  tupakoida / tupakoin / tupakoi / tupakoisi / tupakoinee / tupakoikoon / tupakoinut / tupakoitiin
  ```

  so the main paradigm is class 62's rule unchanged, and `Stems::ida` was
  lifted out of `class_62::inflect` to say so once for both (class 62's own
  output is byte-identical). The split between the classes is lexical and the
  rule cannot see it: `otsikoida` and `aateloida` are class 62, `tupakoida`
  and `haravoida` are class 68, and all four inflect alike — between them the
  two classes hold every `-ida`/`-idä` verb in the language.

  **The only thing this type has that class 62's does not is a second
  paradigm.** The type row's parentheses — `tupakoitsen`, `tupakoitsi`,
  `tupakoitsisi` — are the conjunctive present, and `expand`'s own catalogue
  test requires all three. The whole alternant is one base with three letters
  on it: `tupakoit` (the main stem with the type's own `t`, the one the passive
  writes `-aa` against and the `-ttu` participle writes `u` against) plus `se`,
  `si` and `sisi`, which is how the type's own `-ita` verbs are built
  (`valitse`/`valitsi`/`valitsisi` on `valit`). Two of the three stems are
  `i`-final and one is not, and that is the alternant's whole grammar: on
  `tupakoitse` the present is the plain type-1 set, and on `tupakoitsi` and
  `tupakoitsisi` `absorb_i` fires as it does in the main past.

  It is written out cell by cell rather than handed to `paradigm`, because
  `paradigm` builds every cell a type has and the reference has no row for the
  potential, the passive, the imperative's plurals, the participles or the
  second and fourth infinitives on that stem. Thirty-one words: the present
  and its negative, the imperative's second person, the past, the conditional,
  the third infinitive's six cases and the fifth infinitive with its
  possessives.

  The reference's own pronominal negatives (`en tupakoitse` beside `et`, `ei`,
  `emme`, `ette`, `eivät`) are not written, as they are for no verb in the
  engine: the bare negative stem is pushed and the pronoun supplied from
  outside. That word is pushed twice, once as the negative present and once as
  the imperative's second person, which is what the reference's two rows for
  `tupakoitse` are.

  51 lemmas audit at **0 invented and 0 missing against `both`**, and 345 rows
  over `union` with 62 words over it, in three pieces. 294 of the rows are the
  pronominal negatives just described, six for each of the 49 members that
  have the alternant. 51 are the `-matontaan` row every verb class in the
  project has (`tupakoimatontaan`, `emännöimätöntään`) — `omorfi`-only,
  mis-tagged (`polarity` `affirmative` under `participle = negative`,
  `possessive` `third person` on a partitive) and not a Finnish word, the
  `-maton` participle with the genitive's `t` in it. The 62 words over are the
  alternant for `pakkeloida` and `vatuloida`, the two members with no `ts` row
  anywhere in the reference; nothing in the sanalista separates them from the
  other 49 (all 52 members are plain `68`, no gradation code), so suppressing
  it for them would mean a hard-coded list of two words. Class 62's 670
  `-oid-` members have the same fact against them and it is not emitted there
  either. 1 862 rows covered against 62 words over is the trade.

  Before the module, the type engine reached the three catalogue alternants by
  generating some eighty `tse`-shaped non-words per lemma to find three of
  them (`tupakoitsen`, `tupakitsen`, `tupakoitsitsen`…), which is most of what
  this module removes.

- [x] **Class 69 (the `valita` type)**
  Done, and it is class 68 seen from the main paradigm's side. The base is
  `valit` — the lemma less its `ta`/`tä`, with the type's own `t` in front
  (`vali` + `t`) — and the same three letters class 68's alternant uses go on
  it, only here they are the paradigm rather than a second one: `se` for the
  present (`valitsen`), `si` for the past (`valitsin`) and the conditional's
  own `isi` on the base's `s` (`valits` + `isi` = `valitsisi`, where that `s`
  is `si` with the ending's `i` taken). Class 68's conditional is
  `tupakoitsisi` because its `sisi` stem is `i`-final and `absorb_i` swallows
  the ending's `i`; this type's `valits` is not, so the ending stands whole and
  the two words differ in their fourth letter.

  All 48 members are plain `69` with no gradation code and all end in
  `ta`/`tä`, so there is nothing to tell apart. Four cells are the type's own
  and each is a separate `Stems` field: the second infinitive and the
  imperative's plurals run on the base, not the present stem (`valitessa`,
  `valitkaa`), while the imperative's second person is the stem (`valitse`);
  the passive's theme is the base and its own `t` serves as the passive's
  consonant, so the present adds nothing (`valitaan` beside `valittiin` and
  `valittava`) — a new `PassiveCons::ThemeOwn`, where class 67's `ThemeEnd`
  gives the theme's own consonant back (`tullaan`) and every other type adds
  one (`sanotaan`); and the potential and past active participle are the
  lemma's own stem with the `n` doubled in front of the ending
  (`valin`+`n`+`nee`, `valin`+`n`+`ut`), the one cell on neither the base nor
  the present stem.

  **46 of the 48 members audit at 0 invented and 0 missing against `both`**,
  leaving one `union` row apiece uncovered: the `...imatontaan` mis-tag every
  verb class here has (`valitsematontaan`). The other two are the class's only
  two-reading members, both of which the reference pools into one form set no
  single class's rule can match. `keritä` is homonym 1 `75*D` and homonym 2
  `69`: all 369 of its class-69 `both` rows are generated, the 491 on the other
  reading's `kerki` stem are class 75*D's, and the 723 words over are the rest
  of class 69's paradigm (the reference files most of this lexeme's rows under
  the other reading, so the engine cannot see them). `kaita` is homonym 1 the
  adjective `9*F` and homonym 2 `69`: no missing row at all, and none of its
  151 words over is on a class-69 stem — they are the nominal path's own (the
  verb's 1st infinitive declined as a noun, `kaitaa`/`kaitana`; its 4th,
  `kaitoihin`; the adjective's stem, `kaidoin`).

- [x] **Class 70 (the `juosta` type)**
  Done, and it is the second class of the `-sta` shape. The lemma is
  consonant-final once its `ta`/`tä` is off (`juos`, `pies`, `syös`), and that
  is the weak grade; the strong grade is the same stem with a `k` in front of
  its `s` (`juoks`). The present, past and conditional run on the strong grade
  (`juoksen`, `juoksi`, `juoksisi`), the potential, the imperative's plurals,
  the passive and the past active participle on the weak one (`juossee`,
  `juoskaa`, `juostaan`, `juossut`), and the third and fourth infinitives, the
  present participle, the agent and the negative participle on the strong one
  (`juoksema`, `juokseva`). The imperative alone straddles both: 2sg is the
  present stem (`juokse`), the plurals are the weak base (`juoskaa`).

  This is exactly class 66's stem set, so it was lifted into `Stems::sta`,
  which takes the strong grade as an argument because the two classes reach it
  differently. Class 66 reads a gradation code backwards (`häväis` ->
  `häpäis`); class 70 has no code at all — the `k` is the type's own, and
  `strong_of` writes it. Class 66's output is byte-identical after the
  extraction (597/0/876, unchanged), and `class_70::inflect` is one line.

  All three members audit at **0 missing against `both`**, and each leaves the
  `...imatontaan` row every class here has. `juosta` is otherwise exact. The
  other two carry a second, `t`-less `-tu` participle in the plural oblique
  that omorfi invented (`piesyissä`, `syösyiksi`) where the type's participle
  has the `t` (`piesty`, `piestyn`, `piestyä`, all `both`) — the `t`-less
  variant is `omorfi`-only, is in no `both` row, and is not a Finnish word, so
  the module writes the `t`-ful one the reference lacks instead: 101 rows each
  way, 202 words over for 3 114 rows covered. Before the module the default
  engine missed 2 502 `both` rows over the three.

- [x] **Class 71 (the `nähdä` type)**
  Done, and it is the irregular one: two lemmas, `nähdä` and `tehdä`, and the
  lemma's `hd` turns into three different consonants, so no one stem carries
  the paradigm. The present is strong `näke` / weak `näe` — the first and
  second persons take the weak grade (`näen`, `näet`, `näemme`, `näette`) and
  the third the strong (`näkee`, `näkevät`). That is `k` <-> `∅`, the
  sanalista's gradation `D`; the type writes it into itself rather than into
  the lemmas' rows (neither `nähdä` nor `tehdä` carries a code), so `inflect`
  hands `Stems::with_past` `Gradation::D` and the weak present falls out for
  free. The past is the strong stem with `i` and keeps the `k` only in the
  third persons (`näki` beside `näin`), and the conditional is the `k` stem
  bare with the ordinary `-isi` (`näkisi`), so it keeps the `k` throughout.

  Everything else runs on the lemma's own `näh`/`teh`: the potential and past
  active participle (`nähnee`, `nähnyt`), the imperative's plurals
  (`nähkää`), the passive (`nähdään`, `nähtiin`) and the second infinitive
  (`nähden`, `nähdessä`). The passive is `PassiveCons::Deranked`, the same
  shape class 62's `-ida` has — `d` in the present, a single `t` in the past
  and the moods, and the `-tu` participle deranking again in the oblique
  (`nähty` beside `nähdyn`). The third and fourth infinitives, the present
  participle, the agent and the negative participle take the strong present
  stem (`näkemä`, `näkeminen`, `näkevä`, `näkemätön`), and the imperative is
  the only cell on both stems, 2sg being the weak present (`näe`).

  Both lemmas audit at **0 invented and 0 missing against `both`** over all 926
  of their forms, each leaving only the `...imatontaan` row every class here
  has (`näkemätöntään`, `tekemättömään`).

  A `k`-gradation read as `D` is worth remembering: the next class whose type
  row shows both grades of a `k` and whose lemmas carry no code is the same
  trick.

- [x] **Class 72 (the `vanheta` type)**
  Done. The type is a **`-ne-` present on the strong grade and nothing else on
  the weak one**: `vanheta / vanhenen / vanheni / vanhenisi / vanhennee /
  vanhetkoon / vanhennut / vanhettiin`. The weak base is the lemma less its
  `ta`/`tä` (`vanhe`, `pae`, `ede`, `huono`, `lämme`) — the sanalista writes
  every member in that grade, so the strong one is the module's work — and the
  strong base is that base's own gradation (`pake`, `ete`, `lämpe`). The present
  is the strong base with `ne` (`vanhenen`, `pakenen`, `etenen`) and the past is
  the same with `n` (`vanheni`, `pakeni`), one letter apart, so the conditional
  follows the *past* stem and not the present's (`vanhenisi` is `vanhen` +
  `isi`, not `vanhene` + `isi`) — this type's whole point, and the reason it is
  not class 53's `a`-theme with a gradation code: the vowel the ending hangs on
  has been replaced by a consonant rather than weakened.

  Everything else runs on the weak base with the class's own `n` as the leading
  consonant, the same [`Alt`] shape classes 66, 67 and 69 share: the potential
  and the past active participle take the doubled `n` in front of the ending
  (`pae` + `n` + `n` + `ee` = `paennee`, `vanhe` + `n` + `n` + `ut` =
  `vanhennut`), the imperative's plurals take the type's own `t` (`paetkaa`,
  `vanhetkoon`) while its second person is the present stem (`vanhene`), the
  passive is the ordinary geminated `t` on the base (`vanhetaan` beside
  `vanhettiin`) and the second infinitive the base's `te` (`vanhetessa`). The
  third and fourth infinitives, the present participle, the agent and the
  negative participle take the *present* stem, as in every type.

  **`D` is the one gradation code no other class needed.** The sanalista's code
  carries nearly every gradating member — `A` `kk <-> k`, `B` `pp <-> p`, `C`
  `tt <-> t`, `E` `p <-> v`, `F` `t <-> d`, `H` `mp <-> mm`, `J` `nt <-> nn`,
  `L` `k <-> j` — and `Gradation::strongen` does all of them, because in each the
  base's last weak consonant stands for the strong one. `D` (`k <-> nothing`) is
  the exception and the class's commonest gradation code (17 of its 149
  lemma/class rows, ahead of `A`'s 13): the base shows the *dropped* grade, so
  there is no weak consonant to find and the `k` has to be written back in front
  of the base's final vowel by hand (`pae` -> `pake`, `kye` -> `kyke`, `ulo` ->
  `ulko`). Every other class's strong grade adds to a consonant that is already
  written, which is why this is the only place in the verb side where the code
  has to be matched as a literal `4`.

  **133 of the 146 lemmas audit at 0 invented and 0 missing against `both`**,
  over 103 058 reference rows — every cell the type gives. Each leaves exactly
  one `union` row uncovered, the `...imatontaan` row every verb class here has
  (`vanhenematontaan`, `levenemätöntään`): omorfi-only, mis-tagged, and not a
  Finnish word, the `-mätön` participle in the illative with the genitive's `t`
  in it.

  The other 13 are accounted for, and none of them is the module's. Six are
  absent from the lexicon altogether — `erkanee`, `karkenee`, `korkenee`,
  `mustenee`, `paranee`, `ulkonee` are the `-nee` members the sanalista files
  under `72` and which have no rows in either source, so `invented=1255` counts
  our own paradigm against nothing. Seven are the class's two-reading members,
  whose reference form set is the union of both readings' and so cannot be
  matched by one class's rule; **six of the seven carry class 74 as the second
  reading** (`aueta`, `oieta`, `raueta` as `72*D, 74*D` in a single row,
  `isota`, `sietä`, `tyvetä` as two homonyms), and class 74 has no module yet,
  so what these leave over is the old type engine's class-74 attempt built on the
  *weak* base — `oie`, `raue`, `iso`, `sie`, `aue` — every form of which
  libvoikko rejects. Each is 0 missing against `both`. `aueta` is the one that
  also misses rows, and its 538 missing are all class 74's cells that Kotus
  files and libvoikko confirms (`aukean`, `aukeaa`, `aukesi`, `aukeaisi`,
  `aukeama`, `aukeaminen`, `aukeava`) — they arrive when class 74 is written.
  `isota`'s 35 are the mirror image: libvoikko accepts all 29 of its single-word
  forms, so they are real words, but neither source lists them for `isota`.
  (`isota` and `tyvetä` are worth re-reading per reading rather than summed, as
  the note below asks: the reference gives the two homonyms of a lemma
  byte-identical form sets — 1255 forms under each for `isota`, 1267 for
  `tyvetä` — so there is nothing to split, and the split has to happen on our
  side instead.)

  The seventh, `seljetä`, is plain `72*L` and needs no second reading to explain
  it. Its reference entry is half empty: **456 `both` rows against the class
  median of 982**, holding only the passive, the second infinitive, the
  imperative's third person and the perfect. All 456 are generated, so the 669
  words over are ours against a reference that never listed them. What the
  reference does list on the other side is omorfi's, and it is incoherent: it
  reads the strong stem `selke` and then gives it a type-1 present (`selkeän`,
  `selkeämme`) *and* a class-74 past (`selkesi`) *and* two spellings of the
  conditional (`selkeisin` and `selkeäisin`) *and* a third infinitive on a
  different stem from its own first (`selkeämän` beside `seljetä`). `selkesi` is
  not a word — libvoikko rejects it, and it is the one form in the set that
  would settle the reading — and 635 of omorfi's 663 single-word forms are
  rejected with it. The module is right in every cell, including the two that
  look hardest: `seljennyt` is the front-vowel `-nnyt` and not `-nyt`, and
  `seljentää`/`seljeni` are absent because the `n` is the type's own.

  **Auditing this turned up a real bug in the old engine, in the two places
  where it harmonises a `-tä` participle.** `grammar::verb.rs` builds the
  `-ta/-tä` participle as `stem` + a suffix that fronts from `nnut`, and the
  front branch of that suffix was `"nyt"` — one `n` short. So every front-vowel
  `-tä` verb the old engine touches got `hypänyt` beside the correct
  `hypännyt`, and because a hand-written module is authoritative and clears the
  engine's output, the class was masking it: it only leaked through two-reading
  lemmas (`sietä`) and the unwritten classes. Kotus-Verbit's own rows settle the
  value outright — over the whole 9834-verb lexicon, 219 front `-tä` verbs have
  `stem+nnyt` and 819 back ones `stem+nnut`, and `stem+nyt` has **zero** `both`
  rows anywhere. The same mistake stood next door in the potential, whose ending
  was being harmonised to `nnyt`/`nyt`; a potential is never `-nyt` (`jyllänee`,
  `jyllännee` beside `sala`, `salannee`), so that ending is now flat `nnee`, with
  only type 6 taking the bare `nee` — which is what its own comment already said
  it was for. Together: **225 lemmas no longer emit the one-`n` participle**, and
  `sietä` drops from 73 invented to 35 and `tyvetä` from 707 to 669. Every other
  number in the class audit is byte-identical across the change and the 353 lib
  tests pass.

- [x] **Class 73 (the `salata` type)**
  Done, and it is class 72 seen from the other side: the same lemma-less base,
  the same doubled `n` in front of the potential and the past active
  participle, the same `t` of the infinitive doubling into `salatkaa` and
  `salattiin`, and the same `te` of the second infinitive (`salatessa`,
  `salaten`). The type row is
  `salata / salaan / salasi / salaisi / salannee / salatkoon / salannut /
  salattiin`, and what makes it its own type is one thing: **the whole finite
  mood runs on the strong grade**, where classes 52, 53 and 56 put the weak grade
  in front of the vowel and keep the strong one only for the third person
  (`haukko`/`haukkoa`, `auttoi`/`autoin`).

  That is not a stylistic difference but the reason the class exists. Those types
  gradate in the *middle* of the stem, so the weak grade is a shorter, lighter
  variant of the same stem and can carry the first and second persons on its own.
  Class 73 gradates at the *end* of the base, so the weak grade is not a variant
  of anything — `pak` + `kaan` is not a Finnish word — and the paradigm has no
  weak present and no weak past to speak of. Hence the module works both grades
  out by hand and hands `Stems` a `gradation` of `None`, so `Stems::weak` has
  nothing left to weaken and every person of every finite mood takes the same
  stem. The present is the strong base with the infinitive's own theme vowel on
  it (`salaa`, `pakkaa`, `lepää`), which is why the third person's
  `pres3sg` is **empty** — the stem already ends in the doubled theme, so the
  letter to repeat is already there (`salaa`, not `salaaa` as type 1's `sanao`
  beside `sanon`), the same arrangement class 62's `-ida` gets for its own
  reason. The past is that base with `s` in front of the endings' own `i`
  (`salasi` is `salas` + `i`, where type 1 drops the theme instead, `sanoi`),
  and the conditional is the bare base with the ordinary `-isi` (`salaisi`), so
  the past and the conditional are one letter apart and both are strong.

  **`strong_base_of` is now `utility`'s, not class 72's.** Classes 72, 73 and 74
  all have a lemma-less base, all three write it in the weak grade, all three
  read the present off the strong one, and `D` is the commonest code in all
  three — so the helper that inserts the dropped `k` before the base's last
  *vowel* is shared rather than copied. The site is the vowel and not the last
  character, which is what lets it also serve class 67's consonant-final `jael`
  (`jakel`); class 72's audit is byte-identical across the move.

  **947 of the class's 962 lemmas audit at 0 invented and 0 missing against
  `both`**, over 655 560 of the class's 660 912 reference rows, and each leaves
  exactly one `union` row uncovered — the `...amatontaan` row every class here
  has (`salaamatontaan`, `lepäämätöntään`), the `-mätön` participle in the
  illative with the genitive's `t` in it. Before the module the class generated
  101 forms a lemma against a 1 289-form reference, and the missing rows over it
  were **596 803**; they are now 1 093.

  The other 15 are the data's rather than the module's, in four groups. **Seven
  have no Kotus-Verbit rows at all** (`digata`, `feikata`, `krampata`, `shakata`,
  `tiltata`, `torpata`, `trendata`), so their 668 invented are a full paradigm
  against an empty reference — the class-72 group's verdict applies unchanged.
  **Four have a half-empty entry** (`blogata`, `hyljätä`, `peljätä` at 499 rows
  and `pokata`, `reitata` at 456, against the class median of 981), holding only
  the weak-base cells; three are covered in full and their 625–668 invented are
  ours against a reference that never listed them. `hyljätä` and `peljätä` are
  the class's two `L` members and worth a look, because `L` is `k <-> j` and
  they come out exactly as the type says: `hylkään`, `hylkäsi` on the strong and
  `hyljännee`, `hyljänneet`, `hyljätköön`, `hyljättiin` on the weak.

  **`taata` is two words** — verb `73*D` and noun class 9 — so its reference set
  is the union of the two, which is covered to within the one `union` row above;
  the 152 invented are the noun's own forms. This is the "read the audit per
  row" note in action, and it is also the reason `taata` looks like the class's
  worst lemma on `invented` and its best on `missing|union`.

  **Two have a gradation the sanalista does not record.** `lobata` and
  `blogata` are filed as plain `73`, but Kotus-Verbit gradates them `bb <-> b` and
  `gg <-> g` — `lobbaa`, `lobbasi`, `lobbaisi` and `bloggaa`, `bloggasi` on the
  strong, against `lobatkaa`, `lobannut` and `blogatkaa`, `blogannut` on the
  weak, which is class 73's own strong/weak split exactly. libvoikko accepts all
  four and rejects `lobaa` and `blogaa`, so the reading is right and the data is
  thin. Neither consonant has a code: the thirteen are `kk`, `pp`, `tt`, `k`,
  `p`, `t`, `nk`, `mp`, `lt`, `nt`, `rt`, `k`, `k`, and there is no `b` or `g`
  among them. This is the same trade class 68 declined for its `-oid-` members —
  reproducing the grade means a hard-coded list of two words — and the same
  trade class 72's six `-nee` members cost.

  **`tavata` is the fifteenth, and the one where the sanalista is wrong rather
  than thin — so it is worth reading twice.** It is filed `73*E`, `73` and `73*E`
  over three homonym rows, but **none of the three reference entries gradates**:
  all 981 `both` rows of each are the ungradated `tavaa`, `tavasi`, `tavaisi`,
  `tavannee`, `tavannut`, and all 525 missing rows are of them. The module does
  the right thing with the code it is handed and gives `tapaa`. The binding's
  `dedup_graded` then drops the plain `73` row that would have supplied the
  ungradated half — correctly by its own rule, since a code and its bare class
  are meant to be one declension and not two, but wrongly *here*, because
  `tavata` does not gradate and the two rows are two different words. Worth
  knowing that `dedup_graded` cannot tell the two cases apart: it reads the
  class list, and the class list does not say whether `73*E` and `73` on the
  same spelling are one reading or two. Left alone deliberately — changing it
  would move every class's audit, and `tavata` is three verbs out of 9 834.

- [x] **Class 74 (the `katketa` type)**
  Done, and it is **class 73 with two fields different**. Same lemma-less base,
  same doubled `n` in front of the potential and the past active participle
  (`katkennee`, `katkennut`), same `t` of the infinitive on the weak base
  (`katketkaa`, `katketessa`), same `te` of the second infinitive, same
  `gradation: None` with both grades worked out by hand, and the same strong base
  throughout the finite mood for the reason class 73's entry gave. The type row
  is
  `katketa / katkean / katkesi / katkeaisi (katkeisi) / katkennee / katketkoon /
  katkennut / katkettiin`, and the two cells that are not class 73's are both
  about the theme vowel.

  **The third person keeps the theme; class 73 drops it.** Same present stem in
  both (`katkea` = `salaa` = `pakkaa`), but class 74's third person repeats it
  (`katkeaa`) where class 73's is the stem unchanged (`salaa`). That is
  `paradigm`'s ordinary `pres3sg` rule — the stem's own last letter — so class 74
  needs *no* field where class 73 empties one, and every other person hangs its
  ending on the same stem in both. **The conditional follows the present rather
  than the base** (`katkeaisi` = `katkea` + `isi`, where class 73's is the bare
  base, `salaisi`), which is what puts the theme into the mood.

  **The type row's parentheses are a whole second set of six cells, and this is
  the note the class-74 entry is really about.** Class 74 is one of only three
  verb types in `kotus-tyypit.txt` to carry a parenthesised alternant (with 60 and
  68), and class 73's entry's advice to read them turned out to be right in a
  stronger form than expected: it is not a spare form, it is six more cells.
  **142 of the class's 180 members have both spellings of all six persons** (99
  of them with both as `both` rows, 43 with the alternant as omorfi's alone), and
  the remaining 38 have no finite conditional in the reference at all. The
  alternant is the **strong base** bare — `katkeisi`, `hikoisi`, `kokoisi`,
  `herkeisi`, `häpeisi`, `ilkeisi` — one letter short of the primary. It also
  reaches the negative auxiliary, which takes it beside the primary in all six
  phrases (`en katkeaisi` and `en katkeisi`); that was worth six rows a member
  and was the whole of the audit's residual for 130 of them.

  Since the alternant is a whole second set of cells rather than a spare form,
  and since only one type so far wants it, it is a `Stems` field
  (`cond_alt: Option<String>`, defaulted to `None` in all three constructors)
  rather than a loop copied into the module — `paradigm` runs the six conditional
  cells once per stem, and `compound::Parts` grew one matching
  `cond_conneg_alt` so the negative phrases follow the same single source. Class
  68's much larger `-tse` alternant is still written out cell by cell; if a
  fourth type turns up with a two-stem mood, it should move here too.

  **The passive needed no new field, and the one this entry added has since been
  removed.** Class 74's writes a single `t` in the present and `tt` everywhere
  else (`katketaan`, `keretään`, `kaikotaan` beside `katkettiin`,
  `katkettaisiin`, `katkettaneen`, `katkettakoon`), with `-ttava`/`-ttu`
  geminating as in every other type. That is not a cell where class 74 parts
  company with class 73 — class 73 writes the same shape (`salataan` beside
  `salattiin`, not `salattaan` as this entry used to say). It is
  `PassiveCons::Geminated` like almost every other type, and the `SplitT` variant
  added for it in the same commit was a second name for a behaviour that already
  existed: it returned `t` and `tt` from exactly the two methods `Geminated`
  already answered. Writing class 75, which needs the same passive, is what made
  the duplication visible; the variant is gone and class 74's numbers are
  unchanged to the row (171 clean, 143 040 `both`, 4110 invented, 538 missing,
  before and after). In every type in the language the passive present and the
  passive past disagree on the number of `t`s; what differs between types is the
  *present's* consonant, not the past's. 144 of the class's 180 members have a
  passive present row at all and all 144 write the single `t`; the other 36 have
  none.

  **171 of the class's 180 lemmas audit at 0 invented and 0 missing against
  `both`**, over 142 502 of the class's 143 040 reference rows. Before the module
  it was 18 of 180 clean with 128 961 missing rows; the missing rows over the
  class are now 538, and they are all `kimmota`'s. Every clean lemma leaves
  exactly one `union` row uncovered — the `...amatontaan` row
  (`katkematontaan`, `kerkämätöntään`), the `-mätön` participle in the illative
  with the genitive's `t` in it, which every class here has and no class covers.

  The other nine are the data's rather than the module's, in three groups.
  **Five have a reference entry that is class 72's, or empty.** `isota` and
  `tyvetä` are two-reading members (`72` and `74`) whose two homonym rows carry
  byte-identical form sets, so nothing class-74-shaped is reachable from them;
  `kivetä` is filed plain `74` but its entry holds 456 `both` rows of class 72's
  shapes (`kipenee`, `kipeni`, `kipenisi`); `sietä` and `sueta` have no `both`
  row at all, only omorfi's. Their 669–681 invented are ours against a reference
  that never listed class 74, and `missing` is 0 for all five. Same shape as
  class 72's `seljetä` and class 73's four half-empty entries.

  **`kimmota` is a parser gap, and it is the only verb in the whole sanalista
  where one bites.** It is filed `74*(H)`: the gradation code is *parenthesised*,
  which is the sanalista's way of marking a code as optional, so `parse_single`
  reads `74*` and drops it. But Kotus-Verbit's 994 `both` rows are all on the `H`
  grade — `kimpoaa`, `kimpoan`, `kimposi`, `kimpoaisi`/`kimpoisi` in the finite
  mood, against `kimmo` for the potential, the imperative's plurals, the second
  infinitive and the passive (`kimmonnee`, `kimmotkaa`, `kimmotessa`,
  `kimmoamaton`) — which is class 74's strong/weak split exactly. The drop is
  right everywhere else: `halvata 73*(E)` and `kevetä 72*(E)` are the sanalista's
  other parenthesised verb codes, both audit at 0/0 with the code dropped, and
  neither reference gradates. libvoikko accepts both grades for `kimmota`, so
  only the reference rows decide, and they all say `kimpo`. So the reading is
  right and the *sanalista* is odd; hard-coding `kimmota` would be a word list
  for a data gap, the same trade class 68 and class 72 declined. Left alone
  deliberately.

  **Three have omorfi's alternant on the wrong grade, and libvoikko sides with
  us.** `heretä`, `keretä` and `pietä` are all `D` front-vowel members, and
  omorfi gives their alternant on the *weak* base (`hereisi`, `kereisi`,
  `pieisi`) where every other member has it on the strong (`herkeisi`,
  `kerkeisi`, `pikeisi`). Each is 12 invented against 12 `union`-only rows (six
  cells plus six negative phrases). libvoikko rejects all three of omorfi's and
  accepts all three of ours, and the 141 other members' alternants are on the
  strong base, so this is omorfi erring on the three front `D` members rather
  than a rule about front harmony. Reproducing it would mean special-casing the
  grade on the front `D` members, which is the `dedup_graded` trade class 73's
  `tavata` entry declined.

  **The note class 75 earned: two types can be one paradigm apart.** Class 75 is
  class 74 with the parenthetical gone and nothing else changed, which is worth
  knowing before reading a type row as a description of a whole verb: the cells a
  Kotus row prints are a *sample*, and a type number can differ from its
  neighbour's for a reason that shows up in exactly one of them. The corollary
  for the work is that a new module in this family should be diffed against its
  neighbour's stem rules before it is designed, not after — class 75's whole
  implementation was a copy of class 74's with one field left at its default, and
  the way to know that is to put the two type rows side by side, not to design
  from the present tense's shape. The `D` members are the part of class 75 worth
  checking the neighbour for: `keritä` and `siitä` lose the `k` in the weak grade
  rather than having it, which is the same `D` code as class 73's and class 74's
  `keretä` and means the finite mood's stem is *not* predictable from the lemma's
  own spelling.

- [x] **Class 75 (the `selvitä` type)**
  Done, and it is **class 74's type with no alternant** — the shortest possible
  difference between two Kotus types, and worth writing down as such. The two
  rows are

  ```text
  74  katketa / katkean / katkesi / katkeaisi (katkeisi) / katkennee / katketkoon / katkennut / katkettiin
  75  selvitä / selviän / selvisi / selviäisi / selvinnee / selvitköön / selvinnyt / selvittiin
  ```

  and cell for cell they agree everywhere except that parenthetical. So the
  module is class 74's stem rules with `cond_alt` left `None`, and the one place
  it had to say something new is the `D` members: for `keritä` and `siitä` the
  `k` is the *weak* grade's loss rather than its own, so `strong_base_of` puts it
  back in front of the base's last vowel and the finite mood runs on `kerki` and
  `siki` (`kerkiän`, `sikiän`) while the weak cells stay on `keri` and `sii`
  (`kerinnee`, `siinnyt`). Nothing was needed beyond `Stems::with_past` and
  `strong_base_of`, and no field was added.

  **`pres3sg` is the one field to get right here, and it is a copy-paste
  hazard.** Class 73 empties it because its present stem has already written the
  theme twice (`salaa` is the whole third person); classes 74 and 75 leave it at
  its default, the present stem's own last letter, which is the theme, so
  `selviä` + `ä` is `selviää`. Copying class 73's line into class 75 loses
  exactly the 38 present-third-person rows the class has, one per lemma, and
  nothing else — which is what happened the first time, and what the module's own
  test caught. If a sixth `-tä` type turns up, the third person's field is the
  thing to check first.

  **46 of the class's 48 lemmas audit at 0 invented and 0 missing against
  `both`**, over all 36 806 of the class's reference rows — before the module it
  was 5 of 48 clean with 32 964 missing. Invented rose from 1144 to 1294, which
  is the module's own doing and the right trade: the old engine was producing a
  smaller, wrong paradigm, and the extra 150 are in the two lemmas below. Every
  clean lemma leaves exactly one `union` row uncovered, the `...amatontaan` row
  (`selviämätöntään`) that every class here has and no class covers.

  The two residuals are both reference defects, and neither is a word list away.
  **`keritä` is the `isota` defect again, and this time the tag is the class-69
  one.** It files two homonym rows, `75*D` and `69`, and they are byte-identical
  — 1255 forms each, all 1255 shared, none differing — while the reference holds
  not one `keritse`-shaped row for it. So the data tabulates one paradigm and
  labels the second copy with a type it does not use; the module answers the
  `75*D` reading and the `69` reading's 669 forms are what the audit counts as
  invented. Class 72's `isota` and `tyvetä` are the same defect with the tag the
  other way round. **`ryöpytä` is omorfi being inconsistent with itself on one
  lemma.** It is the class's only `B` member and its entry is thin (500 `both`
  rows against the class median of 982): Kotus-Verbit holds the bare forms of the
  non-finite nominals, omorfi holds their possessives, and the six bare forms both
  agree on all carry the theme (`ryöppyämän`, `ryöppyämällä`, `ryöppyämässä`) —
  after which omorfi drops that theme from the very same series' possessive
  forms, writing `ryöppymien` where the theme gives `ryöppyämien`. All 625
  invented forms are that one disagreement.

- [x] **Class 76**
  **Both members audit at 0 invented and 0 missing against `both`**, 1211
  reference rows each and 2422 in all, where before the module the class generated
  nothing at all. That is the first class in this family to come out clean end to
  end: class 72 invents 12 261 rows, class 74 invents 4110 and misses 538, class
  75 invents 1294. The one row per lemma still uncovered is the `...amatontaan`
  one (`tietämätöntään`) that every class here has and no class covers.

  **It is the odd one out of the five in three ways, and each of them is a place
  where copying the sibling would have been wrong.** The sanalista writes both its
  members in the *strong* grade (`tietää` has the `F` pair's `t`) where classes
  72 to 75 are written in the weak one and have to be strengthened by hand — so
  `Stems::weak` finds the gradation on its own and the module needs no
  `strong_base_of` at all. Its theme is the ending's *consonant* as well as its
  vowel (`tie` + `tä` = `tietä`, where class 75 has `selvi` + `ä`). And its
  ending is one letter longer, `-aa`/`-ää` rather than `-a`/`-ä`, which is why it
  cannot use the shared `ta_base`: `tietää`'s `tä` is not its last two letters,
  so stripping those leaves `tiet` and not `tie`. Its own `base_of` strips three
  characters (`taa`/`tää`), and the shared `ta_base` — now lifted out of the four
  copies that had it, as classes 72 to 76 all did — is the two-character one.

  **Which cells take which grade is reversed from every other type, and the shared
  `paradigm` already does the right thing.** The third person is on the theme stem
  (`tietää`, `tietävät`) and everyone else on the weak one (`tiedän`, `tiedät`,
  `tiedämme`, `tiedätte`, `tiedä!`, `en tiedä`), which is the same rule as
  classes 52, 53 and 56 read the other way round. `pres3sg` therefore stays at its
  default here — the theme stem's own last letter, which is the theme — where
  class 73 empties it. That is the third distinct answer in the family and the
  reason to check the field rather than assume it.

  **The type row parenthesises nothing but space-separates two alternants, and
  they are two different cells: the potential (`taitanee tainnee`) and the past
  active participle (`tainnut taitanut`).** Both are the same fact — the theme
  stem's spelling beside the bare base's — so one `Stems::alt` carries the
  alternant of both and `paradigm` runs each once per stem, the same trick as
  class 74's `cond_alt` one mood lower. The alternant does not stop at the mood
  boundary: the participle's whole declension is built twice and so is every
  compound phrase quoting it (`on tiennyt` beside `on tietänyt`, `eivät ole
  tienneet` beside `eivät ole tietäneet`, `lienee tiennyt` beside `lienee
  tietänyt`), so `compound::Parts` grew `done_sg_alt`/`done_pl_alt` and
  `pot_conneg_alt` alongside them. The imperative's connegative is still single,
  since no type spells that twice.

  **One stem is handed over by hand, and it is the past.** The past is the bare
  base plus the ending's `s` and does not gradate (`tiesin`, `tiesi`,
  `tiesivät`), where `with_past` would have taken the weak grade of the present
  stem less its theme vowel — `tied`, which is not a Finnish stem. The passive
  theme, by contrast, is `Stems::passive_theme_of` at its default (`tiedä` →
  `tiede`) and needs nothing said about it.

  **And one thing is genuinely the type's own: the potential's `Alt::lead` is
  `nn`, not the `n` every other type takes.** That is `tiennen` rather than
  `tienen`, and it sits beside the participle in the very same row, which takes
  the single `n` that gives `tiennyt`. Read the two `Alt`s against each other
  rather than assuming the type shares the family's leading consonant.

  Also worth carrying forward: `cargo fmt` at the workspace root formats **all**
  members including `bindings`, which is edition 2021 and rejects the let-chains
  the root crate is written in, and it takes file arguments after `--` that it
  does not honour either — it reformatted 149 files. Use `rustfmt --edition 2024
  --check <file>` on the files you touched and nothing else.

  **And `rustfmt` on its own is not narrower than that: it follows `mod`
  declarations.** Passing `src/expand/verbs/mod.rs` reformats the whole
  `expand::verbs` tree, so class 73 and 74's tests came back rewritten one item
  per line and `class_67.rs` lost a blank line. The flag is
  `--config skip_children=true`, and there is no `--skip-children` on rustfmt
  1.8. So the safe invocation is
  `rustfmt --edition 2024 --check --config skip_children=true <file>`, which is
  worth pasting rather than reconstructing: the churn is pure formatting, so it
  survives a `git diff` review without being noticed and then lands as unrelated
  noise in a class's commit.

- [x] **Class 77**
  **Done. `src/expand/verbs/class_77.rs`, 0 invented and 0 missing on all three
  lemmas** (`kumajaa`, `heläjää`, `vipajaa`: 1 `both` row each, 654 `union`,
  653 generated, and the single residual is the project-wide `...amatontaan`
  mis-tag class 76 also leaves).

  **It is the first module that does not call `utility::paradigm`, and the reason
  is the type, not a preference.** The type row reads *vaillinaisesti taipuvia
  verbejä; vain joitakin kolmannen persoonan muotoja* and gives three cells —
  `kumajaa`, `kumaji`, `kumajaisi` — but the reference is the inventory, and it
  holds 654 forms: the present's and the past's two third persons, the
  conditional's **six** persons (so "only some third-person forms" is about the
  indicative and says nothing about the conditional), the six `en kumajaisi`
  phrases, the third, fourth and fifth infinitives, and three participle nouns.
  `paradigm` emits the whole type-1 paradigm, and here almost all of it would be
  invented: four present persons, four past persons, the imperative, the
  potential, the passive, the second infinitive, the `-nut`/`-ttu`/`-ttava`
  participles, and every other compound phrase. **So check the type row's cell
  count against the reference before reaching for `paradigm`: a type whose row is
  a sample rather than a paradigm needs the cells written out, and the note on the
  row is the authority on which.**

  What carried over from the shared machinery is everything below `paradigm`:
  `VerbBuilder`, `va_participle`, `maton_participle`, `minen_infinitive` and
  `VerbBuilder::possess` build the four noun declensions between them, and their
  differences come free — `va_participle`'s `gen_pl_possess` is why `kumajava`
  has 128 possessive forms where `kumajamaton` has 139. Two helpers had to be
  lifted out to reach it: `participle_tags` is now `pub`, and the conditional
  connegative loop moved out of `compound::emit` into
  `compound::conditional_connegatives`, which `emit` still calls — a type whose
  only negative is the conditional needs that phrase and none of the rest.

  Three small morphological points, each of which a `-ta` habit would get wrong:
  the present stem is the lemma less **one** letter (`kumaja`), because the theme
  is the ending's `a` and the third person repeats it; the past drops the theme
  vowel off that stem (`kumaji`), which is the shared `Stems::past_stem` rule and
  not a new one; and `PCP=AGENT` is the A-infinitive `kumajama` under another
  name, the same string as the third infinitive's stem, so the two share it.

- [x] **Class 78 — class 77 with the past taken away**
  Done, and it closes the numbered range: **52 to 78 is every numbered verb type
  in the reference**, so `src/expand/verbs/` now has a hand-written module for
  each. `kotus-tyypit.txt` ends at 78 and `base.class` carries no number below
  52, so what is left is not another number.

  The evidence agreed cell for cell on all 32 members (`helskää`, `hohkaa`,
  `huiskaa`, `jyskää`, `kaikaa`, `kalkkaa`, `kapsaa`, `kilkkaa`, `kolkkaa`,
  `kopsaa`, `loksaa`, `läpsää`, `moikaa`, `mäikää`, `mäiskää`, `naksaa`,
  `napsaa`, `niksaa`, `paukkaa`, `puhkaa`, `pärskää`, `raikaa`, `raksaa`,
  `rapsaa`, `roikaa`, `ruskaa`, `ryskää`, `räikää`, `sykkää`, `telmää`,
  `tuikkaa`, `ähkää`): **652** `union` forms each against class 77's **654**,
  the 21 reading families match one for one, none gradating, none given a second
  reading, and **not one of the 32 has a past form at all**. The two cells are
  `[TENSE=PAST][PERS=SG3]` and `[TENSE=PAST][PERS=PL3]`, which is the whole
  difference. So `class_77.rs` was refactored into `fn forms(lemma, past: bool)`,
  with `inflect` = `forms(lemma, true)` and a new `inflect_without_past` =
  `forms(lemma, false)`, and `class_78.rs` is a three-line module that delegates.
  `stem_of` went `pub` for class 78's tests to reach its members' stems. **0
  invented, 0 missing on all 32**, each leaving the one project-wide
  `...matontaan` / `...mätöntään` residual (11 front members, 21 back), and all
  875 lemmas of classes 72 to 77 byte-identical to their saved baselines.

  **What this turn adds to the reading-a-row discipline: the type row already
  carries the difference, if you read it as a row rather than as a word list.**
  Class 77's row is three cells and class 78's is two, and the one that is gone
  is `kumaji` — the past. That is the whole finding visible in `kotus-tyypit.txt`
  before the reference is consulted at all, and it took the 652-against-654
  count across all 32 members to confirm rather than the sample of three cells
  the row offers. The companion trap is that the row's note is identical for
  both (`vain joitakin kolmannen persoonan muotoja`, "only some third-person
  forms"), so the note cannot be what distinguishes them; and the `both`
  coverage is patchy in the opposite direction from what that would suggest —
  28 of class 78's 32 members have no `both` row at all, `hohkaa` has 13,
  `helskää` none, where class 77's `kumajaa` has one. A thin `both` set is this
  type's normal state, not evidence of anything, so it is the count that
  settles a case like this and never the `both` set.

  **And the milestone's own caveat, since "52 to 78 is the whole range" is not
  the same as "every verb is covered".** Three `base.class` values sit outside
  it: `99` and `100` are the sanalista's `Sana on taipumaton tai vaillinaisesti
  taipuva` marker rather than a paradigm, and `dispatch` has no arm for either.
  Two lemmas have one of those as their **only** class — `hälyyttää` (1255
  `union` forms) and `rationoida` (1248) — and the reference inflects both with a
  full 133-family paradigm, so the engine returns **0 forms** for each, all 2503
  of them uncovered. The third, `parata`, also reads `99` but *also* reads `72`,
  and is clean as class 72 (`1254/1255`, the usual residual), so the two-sided
  case is the harmless one. See the new entry below.

  **The notes carried forward from class 77's turn, plus the ones it earned
  above.** Check the
  sanalista's index for lemmas the class's own type
  would not cover before designing the module — the `-nna` and `-rra` verbs under
  67 are a different type each, and both turned out to be the same type with a
  different consonant. Read the type row's *parentheses*, not just its cells:
  class 68 and class 60 and class 74 are the three verb types in
  `kotus-tyypit.txt` that carry a parenthesised alternant, and the alternant is a
  whole second paradigm rather than a spare form — class 74's is six more
  conditional cells *plus* the six negative phrases on the same stem, which is why
  it is a `Stems` field and not a copy of the mood loop. **Class 76 adds the
  other notation to watch for: space-separated alternants, two of them, on two
  different cells.** Watch the two-reading members (a verb whose sanalista row
  lists a second class, or which is also a noun or adjective): their reference
  form set is the union of both readings', so their audit numbers are the type's,
  not the module's, and should be read per row rather than summed — but note that
  the split cannot be done on the reference's side, because the two homonyms of a
  lemma carry byte-identical form sets (1255 forms under each for `isota`, 1267
  for `tyvetä`), so it has to be done on ours. And when a class's audit leaves a
  residual that is `omorfi`-only and no `both` row has it, check whether it is a
  real word before reproducing it: several classes now share one shape
  (`Stems::ida` for 62/68, `Stems::ita` for 69, `Stems::sta` for 66/70), and the
  shared helper is usually what the next class needs rather than a fresh copy.

  **The note class 74 earned: an alternant is a second set of cells, so the
  question to ask of a new one is *which* cells it covers.**
  Class 74's reached the six finite conditional cells and the six negative phrases
  on the same stem, and stopped at the mood boundary, which is what made it one
  `Stems` field plus one `compound::Parts` field. Class 60's and class 68's do not
  stay inside one mood, which is why class 68's is written out cell by cell and
  class 74's is not. So: a second *mood* wants a `Stems` field, a second
  *paradigm* does not. Class 75 turned out to carry none, which is what makes it
  class 74's type with one field at its default. Class 76 settled the case for
  the space-separated notation — its alternant is a whole second spelling of the
  participle, declension and compound phrases included, so `compound::Parts` needed
  two more fields rather than none.

  **And the note class 73 earned, which is about where the gradation sits rather
  than which cells take it.** Classes 52, 53 and 56 give the weak grade to the
  first and second persons and the strong one to the third, and class 73 does the
  opposite in every cell — because their gradation sits in the *middle* of the
  stem, where the weak grade is a lighter variant of the same stem, while
  class 73's sits at the *end* of the base, where the weak grade is not a
  variant of anything (`pak` + `kaan` is not a word). So before copying a
  `-ta/-tä` type's weak/strong pattern across, check where in the stem the code
  lands: the pattern is a consequence of the site and not a choice. When the
  whole paradigm is on the strong grade, hand `Stems` a `gradation` of `None`
  with both grades worked out by hand — that is what makes `Stems::weak` return
  the strong stem, and it is cheaper than overriding the three fields that read
  it (`stem`, `past_weak`, and whatever else `weak()` reaches). Class 74 confirms
  it twice over: its conditional *and* its conditional alternant are both strong,
  while its potential, participle, imperative plurals, second infinitive and
  passive are all weak, exactly as class 73's are.

  **And the note class 76 earned, which is about what the sanalista chose to write
  rather than what the type needs.** Three things about this family are decided by
  the *lemma* and not by the type, and each of them would have been a wrong turn
  if carried over from the sibling class. Which grade the sanalista wrote decides
  the direction of the whole module: 72 to 75 are written in the weak grade and
  must be strengthened by `strong_base_of`, while class 76 is written in the
  strong one, so `Stems::weak` finds the `F` pair on its own and the module needs
  no strengthening at all. The *length* of the ending decides which base helper
  applies, and it is not the same helper: classes 72 to 75 end in `-ta`/`-tä` and
  take the shared two-character `ta_base`, class 76 ends in `-aa`/`-ää` and needs
  its own three-character strip, because `tietää`'s `tä` is not the last two
  letters of the word. And whether the theme is the vowel alone or the consonant
  too decides what the present stem is made of. So: read the type row's ending
  before reaching for the previous class's helper, and check which grade the lemma
  is in before assuming the module has to strengthen anything.

  **The two notes that class 72 spent, both of which are about the old engine
  rather than about the class.** Its residual is currently the old type
  engine's class-74 attempt, and it is recognisable: it builds class 74 on the
  **weak** base instead of the strong one, so it emits `oiean`/`oiesi`/`oieama`
  where `oikean`/`oikesi`/`oikeama` are the words (`aueta`, `oieta`, `raueta`,
  `isota`, `sietä`, `tyvetä` — 35 forms each, all rejected by libvoikko). Both
  defects below are in `grammar::verb`; one is fixed and one is not, and class
  74's module has now been written over the top of the second:

  * the potential is built on the weak stem, not the strong one — `keretä` gives
    `erennee` where the reference has `kerennee` (`erinnee` for `keritä`,
    `jehjennee` for `kehjetä`). 25 lemmas in classes 73 to 75 are affected and
    libvoikko rejects the wrong form in every case. **Still open** — class 74's
    module now covers `heretä`, `keretä` and `pietä`, so what is left of the 25
    is classes 73 and 75, and class 75's module is the reason to write it rather
    than a separate fix;
  * a front-vowel `-tä` verb's participle lost an `n` (`hypänyt` for
    `hypännyt`) and its potential was given a `-nyt` ending (`jyllännyt` for
    `jyllännee`). Both are fixed in `grammar::verb`; 225 lemmas no longer emit
    the one-`n` participle.

- [ ] **`hälyyttää` and `rationoida` — class `100`, and `dispatch` has no arm for it**
  The only two verbs the engine cannot inflect at all. `base.class` for each is
  `100` and nothing else: the sanalista writes `100` where `kotus-tyypit.txt`
  explains `99 = Sana on taipumaton tai vaillinaisesti taipuva`, so the flag
  says "this word is uninflected or irregularly inflected" rather than naming a
  type, and `verbs::dispatch` has no `100 =>` arm. The audit therefore reads
  `class=100 both=0 union=1255 got=0 missing|union=1255` for `hälyyttää` and
  `union=1248 missing|union=1248` for `rationoida` — **2503 forms uncovered in
  all, and the only place left in the verb side where the engine emits nothing
  for a verb at all.**

  The sanalista's flag is not the reference's view, though, so this is a real
  gap rather than a right answer: inflected, both have **133 reading families**
  and a full paradigm's worth of forms, and the families are the ordinary ones
  (`hälyytetään`, `hälyytellyt`, `älköön olko hälyytetty`, `eivät olleet
  hälyyttäneet`). `hälyyttää` has exactly the 1255 forms and 133 families of
  class 72's `parata`. So the sanalista declines to classify a verb that
  Kotus-Verbit classifies perfectly well, and the two-sided case is the
  harmless one: `parata` also reads `99` but *also* reads `72`, and is clean as
  class 72 (`1254/1255`, the usual `...matontaan` residual).

  What to do with it is a judgement call, and it should be made deliberately
  rather than by adding an arm to make a number move. The two candidates are to
  route `100` to whichever of 72/74 the lemma's shape fits — `hälyyttää` is a
  `-ttää` verb and `rationoida` a `-ida` one, so they may well not be the same
  type, and 1255 against 1248 says they are not; or to leave the flag meaning
  what it says and accept two uninflected words. Routing by shape means deciding
  that the sanalista's own class column is unreliable for two rows out of a
  lexicon, which may be right and should then be written down rather than left
  implicit in a `match` arm. Either way: **read the flag before designing the
  arm**, because `100` looks like a paradigm and is not one.

- [ ] **`uida` — `ui` is two letters and `VerbBuilder::push` drops it**
  The 735th and last lemma of class `62` has the only stem of the class short
  enough to matter: `uida` is `ui` + `da`, so its third person present, its
  imperative second person and its past third person are all the two-letter word
  `ui`, which Kotus confirms in all three cells. `VerbBuilder::push` and
  `push_rare` refuse anything shorter than three letters, so the one form is
  missing against `both`. The guard is load-bearing and predates the class work:
  dropping it to two letters costs nothing on classes 52 to 62 (the audit is
  unchanged to the form) but starts emitting consonant-plus-`n` fragments such as
  `jn` (`juoda`), `sn` (`saada`) and `ln` (`luoda`) from the classes past 62,
  which `grammar::verb` still drives and which mangle the stem of a one-syllable
  verb before they reach a `push`. Revisit when those classes are hand-written
  and the fragments go away; until then one real word is the cheaper price.

- [ ] **`vuotaa` and `sortaa` — the reference uses the alternate class**
  The sanalista gives them `53*F, (54*F)` and `53*K, (54*K)`, and both reference
  generators inflect them as 54: `vuodan` in the present but `vuosin` in the
  past, which is 54's `t` -> `s` past and not 53's `vuodin`. The engine keeps
  only the first reading, because `parse_inflections` drops what the sanalista
  wraps in parentheses (an alternant, per its own doc, as in `alpi`'s
  `7*E, (5)`), so class 53 inflects the whole paradigm and the 6 past forms are
  the only ones `both` still marks missing. Including parenthesised readings
  wholesale would add the alternant's forms for every dual-class verb (`avata`,
  `iljetä`, `laasta`), so the two stay an artifact of the sanalista disagreeing
  with its own reference.

- [ ] **`viittoa` — one lemma, 597 forms the reference does not have**
  The sanalista's `52*C`, but the reference builds its `-va`, `-ma` and `-maton`
  on the *passive* theme (`viitotva`, `viitotma`) rather than on the strong stem,
  and lists no infinitives at all. Its own analysis is incoherent — three stems
  across one paradigm — so there is no rule to copy and no table entry worth
  writing for a word that is not Finnish. `ajettua`, its class-mate, is regular.

- [ ] **The reference's apostrophe is dropped, we keep it**
  `liu'uttaa`, `poi'ittaa` and `rei'ittaa` (U+2019) are 1188 forms each, all of
  them ours: both generators normalise the glottal-stop marker away, so the
  reference has `liuutettu` where we have `liu'utettu`. Reproducing a
  normalising bug is not what the engine is for; the noun side strips the
  marker on the way in (`docs/lexicon.md`), which is where the disagreement
  comes from.

- [ ] **21 lemmas where Omorfi does not gradate**
  `kurmottaa`, `mesettää`, `panikoitua`, `raskautua`, `voimaantua` and 16 more:
  the union has the ungradated paradigm, `both` has the gradated one, and the
  class code says gradated. `poutaantua`, `sohvaantua` and `voimaantua` are the
  sharper case — Omorfi gradates them `t` -> `d` (`poutaandu`) where the class's
  `J` says `nt` -> `nn` (`poutaannu`). We follow the class.

- [ ] **`*matontaan`, one form per verb**
  `puhumatontaan` is Omorfi parsing the negative participle's translative as if
  it were an essive. Deliberately not generated; it is the one `missing|union`
  left on every verb.

## Medium priority

- [ ] **Wire the syntax checker into the browser extensions**
  The engine, Leptos panel with edit application, docs and evaluation baseline
  are landed. The last surface is an opt-in, debounced grammar call in the
  Firefox and Chromium extensions; they remain spell-check-only for now.
  Deferred edge cases: plural completive object suggestions and `kuten`
  parenthetical phrase extent. Word order stays deliberately deferred. The
  phased plan and the ordered remaining index are in
  [`PLAN_SYNTAX_CHECKER.md`](PLAN_SYNTAX_CHECKER.md) §16; deferred analysis
  refinements are in [`PLAN_SYNTAX.md`](PLAN_SYNTAX.md).

- [ ] **Fill the bare case gaps**
  **1565 missing forms across 65 classes**, small enough to list by case: ELA
  165, INE 165, ABL 164, ADE 163, ALL 160, TRA 160, ABE 157 (51–55 classes
  each), ESS 125, GEN 113, ILL 61, then INS 51, PAR 42, NOM 39. The essive gaps
  split about evenly between the two numbers (64 singular, 61 plural), so it is
  not one missing cell — `joki` has `jokena` but no essive plural, and
  `jokinansa` is missing because of it.

  By class the leaders are `49*J` (108 forms), `9*D` (93), `51` (92) and `50`
  (83), which are the same classes as the precision item above: a class built on
  the wrong stem misses cases *and* offers wrong ones, so fixing the stem moves
  both counts.

- [ ] **Implement `analyze_case` reverse analysis** (spec §31 Phase 10, §27)
  `analyze_verb` exists but there is no `analyze_case` / `NominalAnalysis`
  (lemma, pos, number, case, stem, grade). Deferred when the declarative
  `CaseRule` model was added.

- [x] **Fix the `nainen` genitive bug**
  Done. The class 38 stem rule is `drop_sub_add(2,'n','s',"e")`, so the stem is
  `naise` and the genitive `naisen`; the obliques (`naisena`, `naiseksi`,
  `naisetta`) are reached and the class's GAPS retired.

- [x] **Clean up the dead `ppc` binding**
  Done. The verb tail no longer reads the conditional slot; the dead binding is
  gone.

## Low priority

- [ ] **Fill or remove the placeholder modules**
  `src/tokenize.rs` and `src/suggest.rs` are one-line placeholders but are
  declared `pub mod` in `src/lib.rs`. Implement tokenization / spelling
  suggestions, or remove them from the crate.

- [x] **Implement the irregular case rules in `case.rs`**
  Done. The stub `CaseRule` model was deleted; the catalogue-driven generator
  owns the partitive, illative, comitative and instructive through
  `noun_cases()`/`oblique()`.

- [x] **Reduce spurious low-confidence guesses**
  Done. The `inflect` tool prints plain/repaired/guessed as separate sections,
  the catalogue's parenthesised (rare) alternants are demoted from plain, and
  the model-word alignment cuts are restricted to its licensed stems, so the
  `joki → joia`-style noise is gone from the plain paradigm.

- [x] **Fix the clippy warnings**
  Done. Clippy is clean across all targets (was ~17 style warnings; the unused
  `case`/`verb` dead code is gone and the rest were fixed as the work above
  landed).

## Infrastructure

- [x] **Stop `suggest` and `check_fast` from walking the whole head-1 run for known compounds**
  Done. `check_fast` and `suggest_detailed` now run the compound-only
  detector before the engine, and `suggest` returns no candidates when the
  input is already a correctly spelled compound. The detector is shallow
  (one split) and retries on a possessive host, so it stays cheap:
  `suggest("kahvikekkereitä")` went from ~30 s to ~13 ms,
  `limonadipullojen` from ~42 s to ~10 ms,
  `turhamielisyyksissäni` from ~90 s to ~3 ms, and
  `henkiruumiiseenne` from ~25 s to ~13 ms. The engine fallback that covers
  heads missing from the old stem snapshot is now limited to a known-word
  left and a head of at most 12 characters; otherwise a non-compound such as
  `salaperäisemmäksi` spends seconds probing `mmäksi`/`emmäksi`. That word
  still gets its own degree suggestions (~0.3 s). `linjaauto` and
  `suomalaisruotsalainen` deliberately miss the short-circuit because they
  must still be offered their hyphenated spelling.

- [ ] **Replace `LEMMA_SET` with a build-time index over `WORD_ORDER`**
  `lemma_set()` currently allocates a ~1.6 MB `HashSet<&str>` of ~100 k lemmas
  on first use. A custom build-time open-addressed / minimal-perfect hash over
  indices into the existing `WORD_ORDER` would remove the allocation and
  startup cost without duplicating the strings. Do **not** use a plain
  `phf::Set` literal: the form/stem tables already document why that duplicated
  every key and was replaced with front-coded blobs.

- [ ] **Clean the stale oracle artefacts out of `all_forms.csv`**
  The generated file still carries old engine mistakes such as `molemmaaa`
  from `molemmat`; `build/map_builders/front_coded.rs` filters only the known plural-only
  `...tn` rows. Find where the stale rows enter the oracle, regenerate or filter
  them at the source, and add a regression so a bad engine snapshot cannot
  silently become part of the fast form set again.

- [ ] **Speed up `lookup` / `check` tools**
  They build the full reverse index over ~100k lemmas and do not finish in
  ten minutes for a single word, so neither is usable as it stands. Add
  incremental indexing, caching, or parallelism. Until then the binding's
  `Engine.lookup` is the practical way to ask what class a lemma has, and the
  harness never needs the reverse index at all.

- [ ] **Make the formatting gate meaningful**
  `cargo fmt --check` still fails on pre-existing files (`build.rs`,
  `src/case.rs`, `src/expand/build.rs`, and others), so a red result does not
  distinguish a new formatting mistake from old noise. Either clean the tree
  once or make the gate check only hand-written files with
  `rustfmt --config skip_children=true`; never run root `cargo fmt` blindly
  (see the formatting notes in the class-73 section).

- [x] **Write a README**
  Done. `README.md` covers the goal, the workspace layout, the data files, the
  confidence model, the four CLI tools, the PyO3 binding, the tests and the
  gates. `docs/harness.md` covers the verification harness in detail.

- [ ] **Expand test coverage**
  `cargo test` runs 144 unit tests in the library and 26 in
  `tests/catalogue.rs`. Still missing: property/round-trip tests over the
  1388-form catalogue check, and targeted tests for the fixed bugs (potential
  strong stem, plural genitive, genitive possessive). The third kind of test —
  the Omorfi + Voikko harness in `tools/` — checks the library against
  external authorities rather than against itself, and is where a new class
  defect should be looked for first; see `docs/harness.md`.

## Known issues to verify later

- `ummi`/`joki`-type gradation interplay across plural genitive, possessive,
  and partitive plural warrants a wider audit against the catalogue.
- The `GAPS` table in `tests/catalogue.rs` is now empty: every Wiktionary form
  of every model word is offered or generated. If a class's stems change, the
  test re-opens the holes automatically.