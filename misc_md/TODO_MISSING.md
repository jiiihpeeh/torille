# TODO: missing / mis-analysed words

Words collected from reading. The list was re-checked against the release
build after the fixes below:

```bash
./target/release/check_normal --fast WORD   # first-pass / wasm demo path
./target/release/check_normal WORD          # full path
```

## Status

All but one are now accepted on both paths (and most name the right base).
`liektisevä` is intentionally still `unknown`; it is a misspelling, and
`suggest` now offers `liekehtivä`. The one remaining surface looks like a typo
for a form the engine already knows.

### Fixed

| word | now resolves to | fix |
|---|---|---|
| `uimapaikalleen` | `paikka` | `uima-` is a bound compound modifier; compound head preference |
| `ilkialastomina` | `ilkialaston` | class 34 no longer geminates base `-ton` |
| `liplatteli` | `liplatella` | new `FREQ_TTAA_ELLA` derivation (`67*C`) |
| `alastomia` | `alaston` | class 34 single-`t` oblique |
| `kristinuskon` | `usko` | `kristin-` bound modifier + classless compound head |
| `jalkineidentekotaidonkin` | accepted (base `olla` is still a diagnostic head) | compound split preference |
| `töräytytteli` | `töräytytellä` | `CAUS_YTTAA` + `FREQ_TTAA_ELLA` chain |
| `aikaansaamiseksi`, `aikaansaamisen` | `aikaansaaminen` | `ACTION_MINEN_DA` |
| `päivänpaahtamia` | accepted (base `ahtaa`) | `ACTION_MA` flag on `paahtaa`; derived nominal heads |
| `tuuppauksia` | `tuuppaus` | `ACTION_US_TA_STRONG` |
| `kaihotonta`, `lasittomat`, `varjottomaan` | `kaihoton`, `lasiton`, `varjoton` | `PRIV_TON` uses the weak stem |
| `puvuton` | `puvuton` | `PRIV_TON` weak grade (`puku` -> `puvu`) |
| `eteemme`, `huostaansa`, `keskenämme` | `eteen`, `huosta`, `kesken` | closed lexical supplement |
| `ohimennessään` | `ohi` + `mennä` | closed lexical supplement (head `mennä`) |
| `esiintumistavasta` | accepted (base `vasta`) | compound head preference |
| `uupuneemmaksi` | `uupua` | closed lexical supplement |
| `pikilanganpyörittäjä` | `pyörittäjä` | three-part compound recursion |
| `hoputtelemaan` | `hoputella` | `FREQ_TTAA_ELLA` |
| `savupatsasten` | `savupatsas` | class 41 `-sten` genitive plural |
| `valjaisiin` | `valjaat` | class 41 plural-only lemmas |
| `ryntääjät` | `ryntääjä` | `AGENT_JA_ATA_STRONG` |
| `livahtelivat` | `livahtella` | `FREQ_ELLA` flag on `livahtaa` |
| `vihollispuolen` | `puoli` | `vihollis-` bound modifier; form-set compound parts |
| `enemmittä` | `enempi` | lexical supplement (abessive of the irregular degree) |
| `kotiportilleen` | `portti` | contracted `-en` possessive on compounds |
| `hörppäillä` | `hörppäillä` | `FREQ_ILLA_STRONG` |
| `nielemistaidon` | accepted (base `taido`) | `ACTION_MINEN_LLA` + `nielemis-` modifier |
| `ikeniinsä` | `ien` | `ien`/`iken-` irregular paradigm |
| `parhaat` | `paras` | suppletive `paras` irregular paradigm |
| `antimia` | `antimet` | class 33 plural-only `-imet` |
| `käräjiä` | `käräjät` | class 10 plural-only `-t` lemmas |
| `käräjätuvissa` | `tupa` | plural-only modifier `käräjä-` |
| `toimeenpanijat` | `panija` | `AGENT_JA_NNA` flag on `panna` |
| `lasittomat`, `varjottomaan` | `lasiton`, `varjoton` | `PRIV_TON` |
| `kyyhötteli` | `kyyhötellä` | `FREQ_TTAA_ELLA` |
| `jauhoitus` | `jauhoitus` | `VERB_ITTAA` + `ACTION_US_TTAA` chain |
| `puupihteihin` | `pihdit` | lexical supplement for the plural-only `pihdit` |
| `teatterikuolemaa` | `kuolema` | compound split now prefers a direct lemma modifier |
| `kyynelettömät` | `kyyneletön` | `PRIV_TON_E` |
| `saunoitti` | `saunoittaa` | `CAUS_ITTAA` |
| `talttumuksella` | `talttumus` | `ACTION_MUS` |
| `itsekunkin` | `itse` | closed lexical supplement |
| `vankkurien` | `vankkurit` | lexical supplement for the stale snapshot |
| `liektisevä` | suggest `liekehtivä` | reported-correction table (not accepted as a word) |
| `kimotusta` | `kimotus` | added verb `kimottaa` + `ACTION_US_TTAA` |
| `elttaantuneen`, `elttaantuminen` | `elttaantua` | added verb `elttaantua` + `ACTION_MINEN` |
| `palmikoitsivat` | `palmikoitsia` | `FREQ_OITSIA` derivation on `palmikoida` |
| `katajankaarekselle` | `kaares` | added the old noun `kaares` (class 39); `katajankaari` added as the current lemma |
| `puolitorkuksiin` | `torkkua` | `torkuksiin` is a tabulated adverb of `torkkua` |

### Remaining

These are not fixed. They are kept here because the reported surface is either
not a standard form or has no lexicon base to derive from.

| word | note |
|---|---|
| `säveleet` | standard nominative plural is `sävelet` (already accepted); `säveleet` looks like a typo. |

## Later reports

These were reported after the first batch and are also fixed:

| word | reading | fix |
|---|---|---|
| `synnyinseudullaan` | `synnyin` + `seutu` | `synnyin` bound modifier; contracted `-an` possessive on compounds |
| `muistotonta` | `muisto` + `-ton` | `PRIV_TON` flag on `muisto` |
| `sammalpäisten` | `sammal` + `päinen` | `-inen` base now recognises long-vowel bases (`pää` -> `päinen`) |
| `pengermittäin` | `pengertää` | tabulated adverb |
| `parhaan` | `paras` | fast-path supplement for the irregular paradigm |
| `länkisääreksi` | `länki` + `sääri` | `länki` bound modifier |
| `purkautumismuotoja` | `purkautumis` + `muoto` | `ACTION_MINEN` flag on `purkautua` |
| `toiviomatkan`, `toivioretkellä` | `toivio` + `matka`/`retki` | `toivio` bound modifier |
| `rakastettavaisuus`, `rakastettavaisuutta` | `rakastettava` + `-isuus` | new `ABSTRACT_ISUUS` rule |
| `Väinämöinen` | proper name | added to the proper-name tables |
| `saksilla` | `sakset` | class 7 plural-only `-kset` lemmas |
| `tungeskeleva`, `tungeskella` | `tungeksia` | new `FREQ_SKELLA_I` rule |
| `hölynpölyä` | `hölyn` + `pöly` | `hölyn` bound modifier |
| `mitattomien` | `mitta` + `-ton` | `PRIV_TON` on `mitta` |
| `mittaamattomien` | `mittaamaton` | fast-path supplement |
| `eteensä`, `tapasin`, `haltuunsa` | `eteen`, `tavata`, `haltuun` | fast-path/adverb supplements |
| `karitonta` | `kari` + `-ton` | `PRIV_TON` on `kari` |
| `siunatumpia`, `sointuvinta` | `siunata`, `sointua` | participle-degree supplements |
| `suurkaanin` | `suurkaani` | added `kaani`/`suurkaani` and the `suur-` modifier |
| `helmettömiä` | `helmi` + `-tön` | new `PRIV_TON_I` rule |
| `yllinkyllin` | idiom | fast-path supplement |
| `tilkitsijän`, `tilkitsijä` | `tilkitä` | new `AGENT_JA_TSI` rule |
| `viittovan` | `viittoa` | fast-path supplement |
| `erkanivat` | `erkaantua` | fast-path supplement |
| `tuontuosta`, `tuontuostakin` | `tuosta` | tabulated fused adverb |
| `raamatunlauseet` | `raamattu` + `lause` | lowercase `raamattu` lemma + compound fast-path supplement |
| `silloinkun` | `silloin` | fused adverb supplement |
| `juhlatamineitaan` | `juhla` + `tamineet` | class-48 plural-only lemmas |
| `vanhanaikanen` | suggest `vanhanaikainen` | reported-correction table |
| `lehdetön` | `lehti` + `-tön` | `PRIV_TON_I` now weakens first (`lehti` -> `lehde`) |
| `kahdenkesken` | `kahden` + `kesken` | tabulated fused adverb (`kesken` is the head) |
| `salamyhkää` | suggest `salamyhkäistä` | reported-correction table |
| `lopettamaisillaan` | `lopettaa` | fast-path supplement |
| `naksahtelun` | `naksahtelu` | `FREQ_ELLA` + `ACTION_ILU` chain |
| `äesteli` | `äestellä` | `FREQ_ELLA` on `äestää` |
| `aivotus`, `aivotusten` | `aivotus` | new noun lemma |
| `säilikön`, `säilikkö` | `säilikkö` | new noun lemma and fast supplement |
| `tikkaiden`, `tikkaita`, `tikasten` | `tikkaat` | graded 41* plural-only lemmas + fast supplements |
| `edistyneemmät` | `edistyä` | participle-comparative supplement |
| `solvaajalle` | `solvaaja` | `AGENT_JA_ATA` on `solvata` |
| `tiuskasi` | `tiuskata` | new verb lemma |
| `osaavampi` | `osata` | participle-comparative supplement |
| `ajattelemisaikaa` | `ajattelemis` + `aika` | `ACTION_MINEN_LLA` now takes the strong grade |
| `narraaja` | `narraaja` | `AGENT_JA_ATA` on `narrata` |
| `palpattelisit` | `palpatella` | new verb lemma + fast supplement |
| `lehditettynä` | `lehdittää` | `VERB_TTAA_WEAK` on `lehti` |
| `mairittelija` | `mairittelija` | `AGENT_JA_ELLA` on `mairitella` (strong grade) |
| `simasuu` | `sima` + `suu` | short-compound fast supplement |
| `ruikuttelevaan` | `ruikutella` | `FREQ_TTAA_ELLA` on `ruikuttaa` |
| `maiskauttelee` | suggest `maiskuttelee` | reported-correction table |

## What changed

- **Class 34** now keeps the single `t` of the base type (`alaston` ->
  `alastoman`, `alastomia`). The geminating `-ttoma` stem belongs to `34*C`
  only.
- **Class 41** gains the `-sten` genitive plural (`patsas` -> `patsasten`) and
  plural-only lemmas (`valjaat`). **Class 10** gains plural-only `-t` lemmas
  (`käräjät`). **Class 33** gains plural-only `-imet` lemmas (`antimet`).
- **`paras`** and **`ien`** are hardcoded irregular paradigms: both are
  suppletive stems the class model cannot derive.
- **Added lemmas:** `kimottaa` (53*C), `elttaantua` (52*J), `kaares` (39) and
  the current compound `katajankaari`. The adverb table now carries
  `torkuksiin`/`torkuksissa`/`torkuksista` -> `torkkua`, and the compound
  detector consults the adverb table for its members.
- **Derivation table** gains `FREQ_OITSIA`, `FREQ_TTAA_ELLA`, `FREQ_ILLA_STRONG`,
  `CAUS_ITTAA`, `CAUS_YTTAA`, `ACTION_MUS`, `ACTION_MINEN_DA`,
  `ACTION_MINEN_LLA`, `ACTION_US_TA_STRONG`, `AGENT_JA_ATA_STRONG`,
  `AGENT_JA_NNA`, `VERB_ITTAA`, `PRIV_TON_E`; `PRIV_TON` now builds on the
  weak stem. The flags live on the base rows in
  `data/nykysuomensanalista.csv`, as the table documents.
- **Compound detection** now prefers a split whose modifier is a real word,
  falls back to compound/bound modifiers, accepts derived nominal heads, and
  uses the form subset for compound parts. It no longer treats an illative
  `-en` as a contracted possessive.
- **Fast-path lexical supplement** covers the class forms that the checked-in
  form/stem snapshots predate (the snapshots still contain the old class-34
  `-ttoma` forms).
- `data/catalogue.csv` was regenerated after the class-34 fix, so the class-34
  template is `alaston`/`alastoman` now rather than the old `alasttoman`.
