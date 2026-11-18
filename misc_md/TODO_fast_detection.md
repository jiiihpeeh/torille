# Fast-detection TODO

Words the checker fails to recognise, as reported. `check_normal --fast`
is the per-keystroke path (`find_misspells` / the extension underline);
`check_normal` (full) adds the engine gate. A real failure is
`fast=unknown` **and** `full=unknown` — the word is underlined and never
demoted. Each entry names the root cause and the cluster it belongs to.

Status: **open** · **wip** · **done**.

## Reported list

| word | fast | full | status | root cause |
|---|---|---|---|---|
| `vuotias` / `-vuotias` | unknown | unknown | open | not a lemma — only a combining form; `kaksivuotias`-type compounds miss too |
| `polyteknikon` | unknown | unknown | open | `polyteknikko` is **not in the sanalista** — data gap |
| `hämilläni` | unknown | unknown | open | adverb `hämillä` is not a lemma + possessive `-ni` |
| `filosofia` | **lemma** | lemma | done | already accepted (class 12) — not a failure |
| `virvoitushuoneeseen` | **inflected** | inflected | done | compound split already accepts it (base `huone`) |
| `kaksivuotias` | **lemma** | lemma | done | already a sanalista lemma |
| `suodattamaan` | **inflected** | inflected | done | 3rd infinitive already generated |
| `ettet` | inflected → `että` | inflected | done | fused `että` + `et` (negation table) |
| `ellet` | inflected → `ellei` | inflected | done | fused `ellei` + `et` (negation table) |
| `ellette` | inflected → `ellei` | inflected | done | fused `ellei` + `ette` (negation table) |
| `enkä` | inflected → `ei` | inflected | done | negation `en` + negative clitic `-kä` |
| `eivätkä` | inflected → `ei` | inflected | done | negation `eivät` + negative clitic `-kä` |
| `omakseen` | inflected → `oma` | inflected | done | translative `-kse-` + possessive `-en` (fast path) |
| `palkakseni` | inflected → `palkka` | inflected | done | translative `-kse-` + possessive `-ni`, weak grade |
| `ymmärtääkseni` | inflected → `ymmärtää` | inflected | done | long 1st infinitive `-kse-` + possessive `-ni` |
| `saattaakseen` | inflected → `saattaa` | inflected | done | long 1st infinitive `-kse-` + possessive `-en` |
| `minkä` | unknown | unknown | open | suppletive genitive of `mikä` — prefix walk can't reach it |
| `molemmin` | unknown | unknown | open | `molemmat` mis-filed in `16*H`; instructive missing |
| `yksinkertaisia` | inflected → `yksinkertainen` | inflected → `yksinkertainen` | done | classless `-inen` adjective; head `kertainen` is a *derived* adjective (cluster 5, `derived_inen_forms`) |
| `kernaimmin` | unknown | unknown | open | superlative of the adverb `kernaasti` (class 99) |
| `täysikavuiseksi` / `täysi-ikäiseksi` | inflected → `täysi-ikäinen` | inflected | done | classless compound; inflect-by-head (`ikäinen` 38) |
| `periaatteideni` | inflected (base `id`) | inflected → `periaate` | done | classless compound; inflect-by-head (`aate` 48*C) |
| `varuillamme` | unknown | unknown | open | adverb `varuilla` is not a lemma + possessive `-mme` |
| `tukahdetulla` | unknown | unknown | open | adessive of the passive past participle `tukahdettu` (`tukahtua`) |
| `ajatelko` | inflected → `ajatella` | inflected | done | imperative **connegative** — derived table |
| `hätäilkö` | inflected → `hätäillä` | inflected | done | imperative **connegative** — derived table |
| `syöne` | inflected → `syödä` | inflected | done | potential **connegative** — derived table |
| `kehoitat` | unknown | unknown | open | 2sg present of `kehoittaa` (class 100 verb not covered) |
| `peloittaa` | unknown | unknown | open | `peloittaa` is **not in the sanalista** — data gap |
| `mutkisteleminen` | unknown | unknown | open | `mutkistella` is **not in the sanalista** — data gap |
| `jotka` | inflected → `joka` | inflected → `joka` | done | suppletive nominative plural; noun-builder nominative fallback (cluster 2) |
| `esimerkkiään` | inflected → `esimerkki` | inflected → `esimerkki` | done | classless compound (`esi-` prefix + `merkki`); fixed by the bound-prefix modifier + oracle regen (cluster 5) |
| `päässeet` | inflected → `päästä` | inflected → `päästä` | done | past-active participle, plural nominative (cluster 10) |
| `sisälmyksiä` | unknown | unknown | open | lemma `sisälmykset` is plural-shaped; the engine emits `sisälmyksiä` (`inflect`) but the reverse search never names it (cluster 11) |
| `saamiaan` | inflected → `saada` | inflected → `saada` | done | agent participle partitive plural + possessive (cluster 10) |
| `vihkiytyneen` | inflected → `vihkiytyä` | inflected → `vihkiytyä` | done | past-active participle genitive singular (cluster 10) |
| `ryhdyttäessä` | inflected → `ryhtyä` | inflected → `ryhtyä` | done | passive 2nd-infinitive inessive (cluster 10) |
| `varttuneita` | inflected → `varttua` | inflected → `varttua` | done | past-active participle partitive plural (cluster 10) |
| `useampikin` | inflected → `usea` | inflected → `usea` | done | comparative `useampi` of `usea` + clitic `-kin`; now derived via the pronominal degree row (cluster 6) |
| `kunnostautuneille` | inflected → `kunnostautua` | inflected → `kunnostautua` | done | past-active participle allative plural (cluster 10) |
| `kukistumiseen` | inflected → `kukistua` | inflected → `kukistua` | done | 4th-infinitive illative; was mis-split to compound head `mi` (cluster 10 + false positive) |
| `yksinkertaiset` | inflected → `yksinkertainen` | inflected → `yksinkertainen` | done | classless `-inen` adjective; head `kertainen` is a *derived* adjective (cluster 5, `derived_inen_forms`) |
| `kammoksuttavasti` | unknown | unknown | open | `-sti` adverb of the participle `kammoksuttava` (`kammoksuttaa`, 53\*C) |
| `vastanneen` | inflected → `vastata` | inflected → `vastata` | done | past-active participle genitive singular (cluster 10) |
| `kuningasten` | unknown | unknown | open | genitive plural `-ten` variant of `kuningas` (41\*G); engine emits only `-iden`/`-itten` (cluster 12) |
| `rakennetut` | inflected → `rakentaa` | inflected → `rakentaa` | done | passive past participle plural (cluster 10) |
| `teereä` | unknown | unknown | not-a-bug | Voikko rejects it; the real partitive of `teeri` is `teertä` (which the engine emits) |
| `paistelivat` | inflected → `paistella` | inflected → `paistella` | done | 3pl past of the frequentative `paistella`; the lemma was missing from the sanalista **and** the Omorfi-built verb oracle, so it is now added and the oracle enriched from the engine (`build_verb_lexicon.py` engine pass, cluster 13) |
| `kovakorvaisia` | inflected → `kovakorvainen` | inflected → `kovakorvainen` | done | classless `-inen` adjective (`kova` + `korvainen`); head is a *derived* adjective (cluster 5, `derived_inen_forms`) |
| `itselleen` | inflected → `itse` | inflected → `itse` | done | allative `itselle` + contracted 3rd possessive `-en`; that shape was missing from `POSSESSIVE_SUFFIXES` |
| `nikkioeläintensä` | unknown | unknown | not-a-bug | Voikko rejects it and Omorfi only guesses (`UPOS=X`); not a real word |
| `useimmat` / `useimpia` / `useampikin` | inflected → `usea` | inflected → `usea` | done | `usea` is a **pronomini**, so `build_adj_degrees.py` (adjectives only) never emitted its degree row; including pronominals gives `usea useampi usein`, and the engine derives `useimmat`/`useimpia` from the superlative (cluster 6) |
| `juonenkulultaan` | inflected | inflected | done | compound head `kulultaan` (`kulku`/`kulu` + ablative + contracted 3rd possessive `-an`); the fast path only stripped the full `-nsa`/`-nsä`, so the split failed (cluster 15) |
| `plutokratia` (+ inflections) | lemma/inflected | lemma/inflected | done | added to the sanalista (class 12) and the oracle/stem maps regenerated, so `plutokratian`, `plutokratioita`, ... resolve (cluster 7) |
| `minkä` / `millä` | inflected → `mikä` | inflected → `mikä` | done | class 101 `mikä` was tabulated for the nominative only; the full suppletive paradigm is now in both nominal trees, and `pronoun_base` reaches it from the fast path |
| `liittoveljiensä` | inflected → `veli` | inflected → `veli` | done | genitive plural + possessive (`veljien` + `-nsä`) contracts the `-en`; the possessive stripper now peels the residual `e` and verifies the full paradigm |
| `pärisytti` | inflected → `pärisyttää` | inflected → `pärisyttää` | done | Omorfi has the word but Voikko does not, so the past 3sg was filtered out of the verb stem map; the map now includes Omorfi-only past 3sg forms and the runtime probes the surface directly |
| `Euroopan` | inflected → `eurooppa` | inflected → `euroopan` | done | continents are not countries; `Eurooppa`, `Aasia`, `Afrikka`, ... are now fetched from `P31 Q5107` and in the proper-name set |
| `todistelukykynsä` | inflected → `kyky` | inflected | done | compound split (`todistelu` + `kykynsä`) with the contracted-possessive fix |
| `todistelykykynsä` | unknown | unknown | not-a-bug | not a word Voikko or Omorfi knows; the standard form is `todistelukyky` (`todistelukykynsä`) |
| `heiluttajan` | inflected → `heiluttaja` | inflected | done | derived `-ja` agent noun (`heiluttaa` → `heiluttaja`); new `derived_agent_base` in the derivation layer |
| `jollen` | inflected → `jollei` | inflected | done | `jollei` fused with the negative auxiliaries; added to the closed connectives table |
| `huomatuimpia` | inflected → `huomatuin` | inflected | done | superlative of the participle `huomattu`; class-36 fallback for participle degrees |
| `raitiotiellä` | inflected → `tie` | inflected | done | `raitio` is a bound prefix of the classless `raitiotie`; `detect_compound` now uses the bound-prefix set |
| `raitioteiden` | inflected → `tie` | inflected → `tie` | done | same classless-compound path; the shared stem `teid` also names `tee`, so the head is disambiguated by the known compound lemma `raitiotie` (`raitio` + `tie`) |
| `harvainvallat` / `harvainvallassa` / `harvainvalloin` / `harvainvaltain` | inflected → `valta` | inflected → `valta` | done | `harvain` is a form of `harva`; `detect_compound` now accepts any fast-resolvable modifier, not only a lemma/stem. `vallassa` is also an adverb lemma, so the content-head guard defers to the compound lemma `harvainvalta` |
| `tähtientutkijat` | inflected → `tutkija` | inflected | done | `tähtien` is the genitive plural of `tähti`; the broadened modifier check reaches it, and `tutkijat` is a form of the lemma `tutkija` |
| `henkiruumiiseenne` | inflected → `ruumis` | inflected | done | illative `-seen` + possessive contracts one `n` (`ruumiiseen` + `nne` -> `ruumiiseenne`); the possessive stripper puts the `n` back and verifies the paradigm |
| `henkivartioväki` (+ inflections) | inflected → `väki` | inflected → `väki` | done | three-part compound (`henki` + `vartio` + `väki`); `detect_compound` now allows a bounded recursive split on the left half, so `henkivartio` resolves as a compound too. Inflections (`-väen`, `-väkeä`, `-väellä`, ...) split at the head |
| `lentokenttäbussi` / `kirjoituspöytälamppu` | inflected → `bussi` / `lamppu` | inflected | done | same three-part recursion (the regression test pins these) |
| `turkulainen` / `porilainen` / `albertalainen` / `missourilainen` | inflected → the derived adjective | inflected | done | productive `-lainen`/`-läinen` place adjective (`ADJ_LAINEN`); `derived_lainen_base` reconstructs the nominative from the `-lais-` stem, requires the place to be a known name/lemma, and inflects class 38. A consonant-ending base takes the epenthetic `i` (`Quebec` → `quebecilainen`, `Hessen` → `hesseniläinen`); an `-i` place often takes `-elainen` (`Lahti` → `lahtelainen`, `Rovaniemi` → `rovaniemeläinen`). The whole class-38 paradigm is covered, including comitative possessives (`laitilalaisineni`). `Sahara` → `saharalainen` and `Laitila` → `laitilalainen` are pinned by the regression test |
| US states / Canadian provinces / German states | proper name | proper name | done | first-level administrative divisions (`P31/P279* Q10864048`) with Finnish labels, plus the single-token components of multi-word names (`New`, `Hampshire`, `Rhode`, `Island`, `Nova`, `Scotia`, `Brunswick`, `Labrador`) |
| `harvainvallain` | unknown | unknown | not-a-bug | Voikko rejects it; the genitive plural is `harvainvaltain`, the instructive `harvainvalloin` |
| `metafyysiika` / `metafysiikko` | unknown | unknown | not-a-bug | Voikko rejects both; the lemmas are `metafysiikka` and `metafyysikko` |
| `minkätähden` | compound `minkä` + `tähden` | unknown (suggests `minkä tähden`) | done | `tähden` is a postposition, not a compound head; the standard spelling is the two-word phrase |
| `mikset` / `miksen` | unknown | inflected -> `miksei` | done | the fused adverb + negation family (`miksen`, `mikset`, `miksei`, `miksemme`, `miksette`, `mikseivät`); gate 1c names `miksei` before the verb stem map can claim `miksata` |
| `nostatella` | unknown | inflected -> `nostatella` | done | a frequentative derived from `nostaa`, confirmed by Voikko but absent from Kotus; the supplemental derived-verb table inflects it as class 67 |
| `jouduttaitui` | unknown | suggests `jouduttautui` | done | the derived reflexive `jouduttautua` (class 52) is in the supplemental table; `jouduttaitui` is one edit from its past 3sg |
| `talonpoikain` | unknown | inflected -> `talonpoika` | done | archaic short genitive plural (`poika` + `-in`); class 10 now emits it as a repaired form, so it is accepted but not suggested |
| `monine` / `monineen` | unknown | inflected -> `moni` | done | comitative plural and its contracted 3rd possessive; `ine` added to the fast-path noun endings and `-en` to the possessive stripper |
| `vedenpitävine` / `vedenpitävineen` | unknown | inflected -> `vedenpitävä` | done | same comitative path; the compound splits `vesi` + `pitävä` |
| `ennenkuin` | compound `ennen` + `kuin` | unknown (suggests only `ennen kuin`) | done | the standard spelling is the two-word phrase `ennen kuin`; `compound::analyze` now refuses a function-word head, so the CLI no longer accepts the fused form, and the suggestion pass offers `ennen kuin` |
| `ennenin` / `ennenein` / `enneniin` | unknown | not suggested | done | class-99 `ennen` (and the colloquial `in`) are in the indeclinable set: they emit only the bare lemma instead of fabricated i-loan forms (`ennenin`) that had leaked into suggestions |
| `todistelukykynnsä` | unknown | unknown (suggests `todistelukykynsä`) | done | double `n`; the standard form is `todistelukykynsä`. `check_fast` now falls back to the compound detector, so the correct form is recognised in the wasm state, and the doubled-letter suggestion fallback offers the correction |
| `henkiruumiseenne` | unknown | unknown | not-a-bug | single `i`; the standard form is `henkiruumiiseenne` |

## Clusters

1. **Negation fusions** (`ettet`, `ellet`, `ellette`, `enkä`, `eivätkä`).
   Closed class: the negation auxiliary fused with the negative clitic
   `-kä`/`-kään`, or with the conjunctions `että` / `ellei`. Table in
   `src/expand/negation.rs`, reached by both the engine and the fast
   path. **done**
2. **Suppletive pronoun forms** (`minkä`, `molemmin`, `jotka`, and the
   like). `find_bases` keys on the surface prefix, so a form that does not
   share the lemma's prefix (`minkä` vs `mikä`) is unreachable; `molemmat`
   is also mis-filed in `16*H`. Needs the pronoun paradigm to be reachable
   independent of the prefix, plus a plural-only paradigm for `molemmat`.
   **Partly done**: the suppletive *nominative plurals* (`jotka` of
   `joka`, `ketkä` of `kuka`, `mitkä` of `mikä`) do not end in `-t`, so
   the noun builder's `stem_of` dropped them; it now emits the whole form
   as a key for an unmatched nominative (a ~190-key fallback), and the
   fast path reaches it through the empty-ending probe. `minkä` (genitive)
   and `molemmin` remain open.
3. **Transitive/possessive `-kse-`** (`omakseen`, `palkakseni`,
   `ymmärtääkseni`, `saattaakseen`). The translative stem `-kse-` plus a
   possessive. **done** on the fast path (rule 1c in `is_known_word`,
   with a noun-stem-map probe for the weak grade). Still open for the two
   fixed adverbs (`hämilläni`, `varuillamme`) whose hosts are not lemmas.
4. **Connegatives as standalone words** (`syöne`, `ajatelko`,
   `hätäilkö`). The engine emits the connegative only inside multiword
   strings (`en syöne`, `älkää ajatelko`), never bare, and the oracle --
   like the class-76 test -- attests the indicative ones bare (`syö`,
   `syönyt`) but the potential/imperative only inside the phrases.
   **done**: `tools/oracle/build_verb_lexicon.py` derives the bare forms
   from the attested phrases, records them in the DB
   (`polarity='connegative'`) and in `data/verb_connegatives.csv`, and
   `build.rs` embeds them for the fast path and the engine. The engine
   paradigm is unchanged (class_76 still pins that it never emits a bare
   `tienne`); the table is a reverse-lookup accept list. The compound
   detector no longer lets a negation form or a connegative be a head.
5. **Classless compounds with a classed head** (`täysikavuiseksi`,
   `periaatteideni`, `yksinkertaisia`, `yksinkertaiset`).
   The sanalista gives a compound no class, so the engine made no forms;
   the head (`aate`, `ikäinen`) is classed and the whole inflects as the
   head. **wip**: `expand::compound_head_forms` now does that, constrained
   -- the modifier must be a word, a **known inflected form** (the
   genitive singular is the common case: `luonnon-`, `kansan-`, `maan-`),
   a combining form (a noun stem-map key) or a **bound prefix** (`esi-`,
   `epä-`, `pika-`); the head a nominal lemma. The full engine resolves
   `periaatteideni` -> `periaate`, `täysi-ikäiseksi` -> `täysi-ikäinen`,
   and `esimerkin` / `esimerkkiä` / `esimerkkiään` -> `esimerkki`,
   `pikajunan` -> `pikajuna`.
   A head that is a *derived* `-inen` adjective (`korvainen` of `korva`,
   `kertainen` of `kerta`), not a lemma, is handled by a second pass,
   `expand::derived_inen_forms`: a classless word ending in `inen`
   (`kovakorvainen`, `yksinkertainen`, and the `-llinen`/`-lainen`/
   `-mainen` adjectives) inflects as the `-inen` class (38), giving
   `kovakorvaisia` and `yksinkertaisia`/`yksinkertaiset`.
   The fast path needs the stem map, which means enriching the noun
   oracle with the classless compounds (`build_nomine_lexicon.py --engine`); that
   run is done, so `esimerkkiään`, `kovakorvaisia`, `yksinkertaisia` and
   `yksinkertaiset` now resolve on both paths. **done** for these. A
   classless word whose modifier is neither a word, an inflected form, a
   stem nor a listed bound prefix still gets no forms -- the `PREFIXES`
   set in `expand/mod.rs` is deliberately small.
6. **Adverb degrees** (`kernaimmin`).
   `kernaasti` is class 99; the comparative `kernaammin` and superlative
   `kernaimmin` are not derived. Mirrors the adjective-degree table.
   **Partly done**: the adjective table now also samples pronominals, so the
   quantifier `usea` gets its Omorfi degrees (`useampi`, `usein`), which
   derives `useimmat` / `useimpia` / `useampikin`. `kernaimmin` remains open.
7. **Data gaps** (`polyteknikon`, `peloittaa`, `mutkisteleminen`,
   `vuotias`). `polyteknikko`, `peloittaa`, `mutkistella`
   are absent from the sanalista (Voikko and Omorfi know them);
   `vuotias` exists only as the combining form `-vuotias`. Not fixable in
   the engine. **`plutokratia` done**: added to the sanalista as class 12
   (with the inflections of `demokratia`) and the binding, noun oracle,
   stem map and blobs regenerated.
8. **Participles in oblique cases** (engine: `tukahdetulla`; fast path:
   `päässeet`, `saamiaan`, `vihkiytyneen`, `ryhdyttäessä`, `varttuneita`,
   `kunnostautuneille`, `vastanneen`, `rakennetut`, `kukistumiseen`).
   Two distinct gaps sharing a root: the participle declines as a
   nominal.
   *Engine*: the passive past participle `tukahdettu` is generated, but not
   its adessive `tukahdetulla`; the engine stops at the nominative. Needs
   the participle to decline.
   *Fast path* (cluster 10): the engine already generates all these forms
   (`plain`), but the verb stem map carries only the bare nominative
   participle, so the fast path cannot peel a nominal case ending to reach
   a past-active oblique (`vihkiytyneen`), an agent participle + possessive
   (`saamiaan`) or a deverbal `-minen` case (`kukistumiseen`); it then
   mis-splits some as compound heads (`kukistumiseen` -> `mi`,
   `lukeneen` -> `kuka`). **done**: see cluster 10.
9. **Class-100 verbs** (`kehoitat`).
   `kehoittaa` is class 100; its present forms (`kehoitat`) are not
   generated. Needs class 100 wired into the verb engine.
10. **Non-finite oblique cases on the fast path** (`päässeet`,
    `saamiaan`, `vihkiytyneen`, `ryhdyttäessä`, `varttuneita`,
    `kunnostautuneille`, `vastanneen`, `rakennetut`, `kukistumiseen`).
    The engine emits every one of these (`inflect_all` -> `plain`); the
    fast path missed them because the verb stem map
    (`tools/build/build_verb_stem_to_base_map.py`) emitted only the bare
    nominative participle (`lukenut`), never its oblique stems
    (`lukenee`, `lukenei`) or the `-minen` case stems (`kukistumi`), and
    gate 4b (`find_base_via_verb_stem_map`) stripped only verb endings,
    not the nominal case endings the participle/infinitive actually
    takes. Fix: the builder emits the non-finite **case-stem** (the form
    minus its nominal case ending), and gate 4b also strips the nominal
    `ENDINGS` list. The verify step (re-inflect the base) keeps it honest.
    This also closes the false positives: `lukeneen` resolved to `kuka`
    and `kukistumiseen` to `mi` before.
11. **Plural-shaped lemmas** (`sisälmyksiä`). `sisälmykset` is a class-39
    lemma whose citation form is already the plural nominative. The engine
    *does* emit `sisälmyksiä` (`inflect sisälmykset` -> partitive plural
    `plain`), but `find_any_base_with` does not reach `sisälmykset` from
    `sisälmyksiä`, so the full check reports `unknown` too -- a
    reverse-search gap, not a form-generation gap. Needs the reverse
    search to consider a lemma whose head is plural-shaped.
12. **`-ten` genitive plural** (`kuningasten`). `kuningas` is 41\*G; the
    engine emits `kuninkaiden` / `kuninkaitten` but not the `-ten` form
    (`kuningasten`), which Omorfi/Voikko and the old literary texts use.
13. **Frequentative verbs** (`paistelivat`, `notkistelevat`). `paistella`
    and `notkistella` are frequentatives of `paistaa` / `notkistaa`
    (`-ele-`); Voikko knows them, Omorfi does not, and neither was in the
    sanalista. **done** for these two: the lemmas are added as class 67,
    and `tools/oracle/build_verb_lexicon.py --engine` adds the finite forms of every
    sanalista verb the Omorfi-built verb oracle lacks (274 lemmas, 8 004
    rows) with `ok='both'`, so the verb stem map carries their finite
    stems (`paisteli` → `paistella`) and the fast path resolves them.
    A productive VERB→VERB frequentative derivation rule is still not
    implemented; a *new* frequentative absent from the sanalista would
    still need its lemma added.

## Batch 2026-10-19 (large text dump)

Fast-path results, with Omorfi+Voikko verdicts.

**Now detected** (fixed this round): `rutistuneen` → `rutistua`
(Omorfi-only participle, see cluster 10 / verb oracle), `lakiperäisessä`,
`rohkeakatseisten`, `frakkipukuisen`, `vastenmielisiä` /
`vastenmielisellä` → `vastenmielinen` and `henkiruumiisenne` /
`henkiruumiisia` → `henkiruumiinen` (derived `-inen` head harmony,
cluster 14), `olivat`, `useimmat` / `useimpia` / `useampikin` → `usea`
(pronominal degrees, cluster 6), `juonenkulultaan` (contracted possessive
compound head, cluster 15), and `plutokratia` with all its inflections
(data + oracle + stem maps, cluster 7), plus `paistelivat` /
`notkistelevat` → `paistella` / `notkistella` (frequentative lemmas and
engine-derived verb forms, cluster 13).

**Real, still unknown — by cluster:**

- Pronoun suppletion (cluster 2): `minkä`, `millä` (adessive/genitive of
  `mikä`; the paradigm is unreachable by prefix).
- Negation fusion (cluster 1): `jollen` (`jollei` + `en`; the connectives
  table has no `jollen` row).
- Degrees (cluster 6): `huomatuimpia` (`huomattu` comparative),
  `viimeisimmät` (plural of the superlative lemma `viimeisin`).
- `-sti` of a participle: `leppyvästi` (`leppyä` → participle → `-sti`).
- Derived `-inen` **harmony** (cluster 14): **done** for
  `vastenmielisiä` (compound `-inen` head now takes the head's harmony)
  and `henkiruumiisenne` (`henki` + `ruumiinen`).
- Compound with a modifier that is a non-lemma combining form (cluster 5
  data): `raitiotiellä` / `raitioteiden` (`raitio`), `sisällissotaa`
  (`sisällis` of `sisällinen`), `luokkasiveysopistaan` (3+ parts).
- Compound with a possessive/fused head (cluster 15): `liittoveljiensä`,
  `todistelukykyynsä` (fast mis-resolves both to `sä`),
  `vierasmiehiltään`, `sydänsiteillään` (heads resolve, the compound does
  not), `ruokotöillään`, `henkivartioväkeä` (3-part),
  `hallitsijattaria` (`-tar` feminine derivation).
- Frequentative (cluster 13): **done** for `notkistelevat` /
  `paistelivat` — `notkistella` and `paistella` are sanalista lemmas now,
  and the engine-derived verb oracle carries their finite forms.

**Not a bug (oracle rejects):** `pelaitti` (both reject), `naukujaiset`
(Voikko rejects; fast *wrongly accepts* it as `jainen` — a false
positive to investigate), `läsnäollessaan` / `huomatuimassa` (Voikko
rejects), `perusjuuriaanvalituneen` (two words run together).

## 2026-10-21 batch

Checked with the existing harness after rebuilding the binding:

```
python3 tools/check_word_against_omorfi.py -p noun tie suo työ vyö yö
python3 tools/report_by_class.py -c 19
```

`report_by_class.py -c 19` found a class-wide defect the single-word fix
would have missed: the singular possessive was built on the short plural
stem (`teni` for `tieni`, `soni` for `suoni`). Fixed in both nominal trees;
class 19 now passes all 159 confirmed forms for each of `tie suo työ vyö yö`.

Also fixed en route:

* `compound::analyze` sliced at a byte index and panicked on `mikä`/`minkä`
  (multi-byte `ä`); splits now check `is_char_boundary`.
* `negation_base` rejected every word not starting with `e`; the `jollei`
  fusions start with `j`.
