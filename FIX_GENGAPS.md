# FIX_GENGAPS — closing the gaps that block engine-only map generation

Plan written 2026-11-05, after comparing the committed tables with the same
tables generated from the current engine (Python binding). The goal is the
one stated in `tools/build/README.md`: a ground-up build that needs only the
repository itself — no `data/kotus_nomines.sqlite`, no `kotus_verbit.sqlite`,
no live Omorfi/Voikko.

Nothing here is a code change yet. Each gap has a measurement, a root cause,
a fix, and an exit criterion, so the plan can be worked and reviewed one item
at a time.

## Baseline measured on 2026-11-05

Engine outputs were written to `/tmp` and diffed against the committed file:

| artifact | committed | engine | shared | lost | new |
|---|---:|---:|---:|---:|---:|
| `all_forms.csv` unique surfaces | 4 856 961 | 6 441 884 | 4 844 254 | 12 707 | 1 597 630 |
| noun stem-map bases (after G1) | 32 115 | 32 115 | — | 0 | 0 |
| verb stem-map bases | 10 022 | 10 033 | — | 6 | 17 |
| verb stem-map `(stem, base)` pairs | 195 803 | 237 424 | 150 704 | 45 099 | 86 720 |
| adjective comparatives | 4 859 | 5 815 | 4 859 | **0** | +956 |
| adjective superlatives | 1 934 | 5 751 | 1 934 | **0** | +3 817 |

Before G1 the committed map was the hybrid (oracle) output: 83 307 bases and
528 629 pairs. The 51 196 "lost" bases were classless whole compounds, which
must not be map bases at all: they resolve through their compound head
(`raitiotiellä` -> `tie`, not `raitiotie`). G1 switched the map to the
engine-only 32 115 classed bases; the classless citations are reached by the
splitter instead. See G1 for the residual modifier gaps.

Reproduce with:

```bash
python3 tools/build/build_all_forms_csv.py --out /tmp/all_forms_engine.csv
python3 tools/build/build_stem_to_base_map.py --source engine \
        --out /tmp/stem_map_noun_engine.csv
```

The verb and adjective comparisons were generated from the same algorithms as
`tools/build/build_verb_stem_to_base_map.py` and
`tools/build/build_adj_degrees.py`, but those two still read sqlite/Omorfi.
Adding the comparison as a committed script is G0.

## Priority

| gap | impact | risk to fix | priority |
|---|---|---|---|
| G0 comparison script | makes every later gap measurable | low | P0 |
| G1 compound-modifier coverage | 406 classless compounds + 57 non-nominal rows | medium | P1 |
| G2 class-100 verbs | 2 lost verb bases, 8 lemmas with no paradigm | low | P1 |
| G3 negation/connective stems | 4 hard-coded verb bases | medium | P1 |
| G4 `all_forms` stale-form audit | 12 707 surfaces, fast-path correctness | medium | P2 |
| G5 catalogue/rule regeneration | keeps the oracle out of the nominal pipeline | high | P3 |
| G6 lexical exception tables | cannot be derived; freeze or implement | high | decision |

Adjacent, no gap: adjective degrees are a strict superset. The generator can
switch to the engine and be regenerated without losing a cell (G7, safe).

---

## G0 — commit the comparison as a tool

**Why.** Every number above came from throwaway scripts. Without a committed
comparison, the next person cannot tell whether an engine change moved the
gap.

**Fix.**

- Add `tools/build/compare_engine_outputs.py`:
  - regenerate `all_forms.csv`, the noun/verb stem maps and the degree table
    into a temporary directory;
  - report committed/engine/shared/lost/new counts for rows, unique surfaces
    and distinct bases;
  - exit non-zero when a *committed* surface or base that the current engine
    cannot produce is not in an explicit allow-list (the lexical tables and
    known engine gaps below).
- Keep the allow-list in the script and make every entry name its gap
  (`G1:compound-modifier`, `G2:class-100`, `G6:lexical`).

**Exit.** `python3 tools/build/compare_engine_outputs.py` reproduces the
baseline table and fails when a new regression appears.

---

## G1 — classless compounds resolve through the head, not as map bases

**First finding (corrected).** `Engine.inflect` already inflects classless
lemmas through `expand::inflect_all`: `mahonginruskea` returns 353 forms,
`älykortti` 161. An earlier draft blamed the binding for the 51 196 missing
bases; that was wrong. The builder filters classless lemmas out of the engine
source on purpose.

**Why they must stay out.** A classless compound's own surface must not be a
stem-map base, or it shadows the content head:

- `raitiotiellä` -> `raitiotie` instead of `tie`;
- `kristinuskon` -> `kristinusko` instead of `usko`;
- `kyrmyniskoja` -> `kyrmyniska` instead of `niska`.

Those are the exact assertions in
`suggest::tests::reported_compounds_name_the_content_head`,
`reported_standard_compound_and_degree_forms_are_known` and
`check_normal::tests::fast_variant_accepts_the_reported_missing_words`.
Including classless lemmas in the engine source fails all three.

**What is actually lost.** Run the engine over every lemma (classed *and*
classless) and compare bases with the committed map: 461 lost.

- **406 classless compounds** whose modifier is not a free lemma:
  `agraaripolitiikka` (`agraari` is not in the sanalista, `agraarinen` is),
  `ampuma-ase`, `ampumahiihto`, `ammatinvalinnanohjaus`.
- **57 classed rows** that are not nominal paradigms: `OK`, `aiemmin`,
  `harvoin`, `hyvin`, `ja`, `ha`, `alias`, ... (adverbs, interjections,
  postpositions; see G1b).

Everything else resolves correctly under the engine-only classed map; verified
with `check_normal --fast`:

```text
raitiotiellä   -> tie        älykortissa       -> kortti
kristinuskon   -> usko       mahonginruskeaksi -> ruskea
kyrmyniskoja   -> niska      ammatinvalinnanohjauksessa -> ohjaus
agraaripolitiikassa -> unknown
```

**Fix.** Extend the compound splitter's modifier coverage, not the map:
`compound::analyze` / `expand::compound_head_forms` should recognise a
modifier that is a derived or bound form (`agraari` from `agraarinen`,
`ampuma-` from `ampua`, hyphenated `ampuma-ala`). That closes the 406 for
every member of the pattern, and the head base is still what the map reports.

**Files.** `src/compound.rs`, `src/expand/compound.rs` (head/modifier split),
`src/expand/mod.rs` (`compound_head_forms`), with class tests in
`src/expand/mod.rs`.

**Exit.** `check_normal --fast agraaripolitiikassa` (and the `ampuma-`,
`ammatinvalinnan-` families) resolves to the content head; the engine-only
noun map is accepted with the 57 classed rows allow-listed; all three
content-head tests stay green.

**Status 2026-11-05.** The splitter now accepts five modifier patterns in
`modifier_is_known`:

- a `-inen` adjective's lexical stem, gated so an adjective case stem does
  not qualify (`agraari` -> `agraarinen`, but `irstai` does not become a
  modifier through `irstainen`/`irstas`);
- an `-eä`/`-ea` adjective's vowel stem (`jähmeä` -> `jähme-`);
- a `-kko`/`-kkö` noun from a `-kas`/`-käs` adjective (`kuivakas` ->
  `kuivakko-`);
- an agent participle's `-ma`/`-mä` stem (`ampua` -> `ampuma-`);
- a genitive that is an inflected classless compound, resolved through its
  own head (`ruumis` -> `ruumiin`, `avomeri` -> `avomeren`), restricted to
  a nominal base.

Measured effect: 406 -> **59** lost classless *map bases*, and the map is
now generated from the engine (32 115 classed bases, 244 740 pairs, no
oracle). Classless citations are no longer bases: `check_normal` resolves
them through the head, which is what the user-visible contract asks for
(`binaarijärjestelmässä` -> `järjestelmä`, not `ässä`). Two hybrid-era test
expectations moved with it (`ilkialastomina` -> `alaston`,
`savupatsasten` -> `patsas`). All 874 lib tests and the workspace suite
pass.

### G1b — the 57 classed losses

Mostly `99` adverbs/interjections (`maanantaisin`, `aiemmin`, `pitkin`,
`harvoin`), plus `piilevä` (class 10), `viti` (5*F), `puolikymmentä` (numeral
class 51), and parenthesised classes (`OK` `(18)`, `mezzopiano` `(1)`). They
are not nominal paradigms and are correctly absent from a *noun* stem map.
Encode them as an explicit allow-list with the reason.

### G1c — optional gradation and the rejected `viti` — **done**

- `nahka` is `9*(D)`. The parenthesised gradation is the standard one
  (`nahan`; Voikko and Omorfi both confirm it), but the parser's documented
  rule drops a parenthesised gradation when the unbracketed class is there
  (`8*(C)` is genuinely optional). `src/sanalista_overrides.rs` now carries
  the correction `nahka -> 9*D`, applied by both parsers; `nahanluonti`
  inflects, and a test pins `nahan`.
- `viti` is `5*F`, but the row is already rejected outright in
  `src/rejected.rs`. `vitilumi`/`vitivalkea` are therefore intentionally
  unknown, not a gradation gap; no parser change is warranted.

The classless-bases loss list drops by the three `nahan*` compounds.

### G1d — numeral compounds

Seventeen of the 59 are hundreds/tens compounds: `kaksisataa`,
`kahdeksansataa`, `parikymmentä`, `puolitoistasataa`, ... The ordinary
head split cannot model them because both members inflect
(`kahdensadan`) and the head of the citation is `sataa`, not the lemma
`sata`.

**Status 2026-11-05.** `numeral_forms` now has `hundreds_forms`, the
counterpart of `tens_forms`: a numeral prefix plus `sata`, both in the
case, with the nominative reading `kaksi` + `sataa`:

```text
kaksisataa  kahdensadan  kahtasataa  kahdessasadassa
```

It also handles a classless numeral prefix (`kahdeksantoistasataa` via
`kahdeksantoista`). Special quantity prefixes (`pari`, `puolen`, `toista`,
`puolitoista`) still return no `Engine.inflect` paradigm, but their
inflected surfaces resolve through the compound gate
(`parinkymmenen` -> `kymmenen`, `puolentoistasadan` -> `sata`), so the
checker accepts them; extending the numeral layer to those is optional.

The hundreds test pins `kaksisataa`, `kahdensadan`, `kahtasataa`.

### G1e — nested modifier compounds — no map gap

The rest are compounds whose modifier is itself a compound:
`kaksikamari-`, `kaksiväri-`, `monitoimi-`, `omatoimi-`, `punakylki-`,
`konetahti-`, `jakomieli-`. With the engine-only map these are not bases;
their citations are in the lemma set and their obliques resolve through
the head (`monitoimikoneen` -> `monitoimikone`, `punakylkirastaan` ->
`rasta`). Nothing is lost at the checker level. They stay out of the
comparison allow-list because they are not map bases by design.

## G2 — class-100 verbs generate no forms

**Symptom.** Eight sanalista verbs are class `100`; `Engine.inflect` returns
0 forms for all of them and `./target/release/inflect hälyyttää` prints
`(no forms generated)`. Two of them (`hälyyttää`, `rationoida`) are the only
verb-map bases the engine drops that the post-pass does not restore.

**The eight rows.**

```text
hälyyttää   hälyytys   kehoittaa   kuullostaa
rationoida  vaakita    verytellä   veryttely
```

Most are variant/misspelled spellings of lemmas that already exist with a
real class (`hälyttää` 53*C, `kehottaa` 53*C, `kuulostaa` 53).

**Fix (choose per row, in `data/oracle_exceptions.json` style).**

1. If the row is a legitimate variant, map it to the standard lemma's class
   in `src/grammar/irregular.rs` (or a small `class 100` table) and let the
   normal engine inflect it.
2. If it is a misspelling, add it to `build.rs::REJECTED_LEMMAS` with a
   comment, and regenerate the lexicon and fast form set.
3. If it is a noun mis-tagged as a verb (`hälyytys`, `veryttely` look like
   nouns), correct the class via the same exception table.

**Files.** `src/grammar/irregular.rs`, `build.rs::REJECTED_LEMMAS`,
`data/oracle_exceptions.json` if the decision is oracle-facing.

**Exit.** Every class-100 row either inflects or is rejected; the engine-only
verb map loses no base except the connectives handled in G3; the affected
`inflect` outputs have tests.

**Status 2026-11-05 — done.**

- `src/sanalista_overrides.rs` names a class for the two rows that are real
  verbs with a neighbouring paradigm: `hälyyttää` `53*C` (like `hälyttää`)
  and `rationoida` `62` (the `-oida` class). The remap is applied by both
  the runtime parser and the build-script parser, so `parse_sanalista` and
  the embedded blob still agree.
- The other six are rejected in `src/rejected.rs`: `hälyytys`,
  `kehoittaa`, `kuullostaa`, `vaakita`, `verytellä`, `veryttely`. Voikko
  rejects all six and Omorfi marks them non-standard or `GUESS=UNKNOWN`;
  leaving them accepted would also hide the standard spelling from the
  suggester.
- `inflect hälyyttää` emits 1 254 forms, `rationoida` 1 247;
  `maps/sanalista.bin` is regenerated. A test pins both paradigms and the
  six rejections.
- The verb stem map is still the oracle output (G3); once it moves to the
  engine, these two bases are no longer lost.

---

## G3 — negation and connective stems are hand-written

**Symptom.** The engine-only verb map is missing `ei`, `ellei`, `että` and
`jollei`. The builder currently appends them as a hard-coded post-pass and
reads the fusions from a sqlite table (`connectives`), bypassing the
inflection engine.

**Root cause.** The negation verb is a closed paradigm the engine does not
emit as single-word forms; the fusions live in `data/verb_connectives.csv`
(`maps/connectives.bin`).

**Fix.**

- Make the engine emit the finite negation forms (`en`, `et`, `ei`, `emme`,
  `ette`, `eivät`) through the same `inflect_all` surface, tagged as the
  negation verb. The existing `src/expand/negation.rs` already owns the
  connective table; extend it rather than teaching the stem-map builder.
- The builder then consumes engine output only; the sqlite `connectives` read
  disappears.
- Keep `data/verb_connectives.csv` as the source for the fusions (it is a
  committed lexical table, not an oracle dependency).

**Files.** `src/expand/negation.rs`, `src/expand/verbs/`, the binding,
`tools/build/build_verb_stem_to_base_map.py`.

**Exit.** Engine-only verb map contains the negation and fusion bases without
a tool-side hard-coded list; the `check_normal` gate accepts the finite forms.

---

## G4 — audit the 12 707 lost `all_forms` surfaces

**Symptom.** The engine keeps 4 844 254 of the committed 4 856 961 surfaces
and adds 1 597 630. Losses include `aatelitonmme`, `aatelitonni`,
`aallokkasten`, `aavaksimerikseen` — possessive and old stem shapes.

**Two possible causes**, and the fix differs:

1. **Stale committed rows.** The old CSV predates a stem/possessive fix;
   the engine is right to drop them. Accept, with evidence.
2. **New engine gap.** A real form disappeared (e.g. a possessive
   nominative that used to be generated). Fix the class engine.

**Fix.**

- Add an audit step to `compare_engine_outputs.py`: for every lost surface,
  check it with Voikko (and Omorfi for existence) using the existing
  `tools/check_word_against_omorfi.py` machinery, and bucket the losses into
  `confirmed` / `unknown` / `illegal` (the harness vocabulary).
- A confirmed loss is a bug; open a class-level fix, never a per-word
  exception.
- An illegal/unknown loss is accepted and added to the comparison allow-list.
- Document the tag differences separately: many "new" rows are the same
  surface with `number=plural` now filled. Report unique surfaces first, tags
  second.

**Exit.** Zero Voikko-confirmed lost surfaces; the regenerated
`data/all_forms.csv` is committed with the audit report; `cargo test` and the
harness gates pass.

---

## G5 — catalogue and class rules are still oracle-seeded

**Symptom.** `build_catalogue_csv.py` and `extract_class_rules.py` read
`data/kotus_nomines.sqlite`/`kotus_verbit.sqlite`. The catalogue is the
engine's own model table, so regenerating it from the engine is circular, but
it is also the last oracle dependency in the nominal build.

**Options, in increasing ambition.**

1. **Freeze the catalogue.** Keep `data/catalogue.csv` as a committed
   reference (like the lexical tables), with provenance. The engine already
   refines it through `tools/src/bin/build_catalogue.rs`
   (`build_catalogue` binary), which is engine-only. Ground-up builds use the
   committed CSV; no sqlite.
2. **Self-check, not self-generate.** Run the engine over each class model
   word and assert the 8 catalogue slots; report drift into `reports/` but do
   not rewrite the table. This catches a class engine disagreeing with the
   catalogue without inventing a new authority.
3. **Engine-generated catalogue.** Emit each slot from the class engine and
   treat the output as the source. Only worth doing after the harness accepts
   the engine output for every class; otherwise a defect becomes data.

**Recommendation.** Do (1) now and implement (2) as the guard. Treat (3) as a
separate project once the harness baselines are stable.

**Exit.** `build_catalogue_csv.py`/`extract_class_rules.py` are documented as
oracle reference tools (not part of the ground-up build), and
`build_catalogue` + the drift report are the engine-side gate.

---

## G6 — lexical exception tables cannot be derived

These tables exist because the engine deliberately does not produce the
forms. Regenerating them from the engine would erase the facts:

| table | why it is lexical |
|---|---|
| `maps/oi_plural_lemmas.bin` | class-10 lemmas whose `ok='both'` paradigm has `-oi-` plurals; `alkaja` is the same class without one (`AGENTS.md`) |
| class-6 `-iden/-itten/-ita` mask | per-lemma exception to the class-6 plural |
| class-15 long partitive set | per-lemma exception |
| `data/adverbit.csv` | `-ksi` adverbs with possessive forms Omorfi does not generate and the engine does not derive |
| `data/proper_name_forms.csv` | Wikidata names + Omorfi's singular cases; the engine has no name-class assignment |
| `data/proper_names.csv` | Wikidata source table |

**Decision needed.**

- **Freeze (recommended for the release).** Keep the committed files and
  their GPL reference generators under `tools/oracle/`; mark them "reference
  data, not part of a ground-up build" in `tools/build/README.md`.
- **Derive.** Requires engine work: a class-10 `-oi-` plural rule, an
  adverbial possessive rule, and a name-class heuristic. Each is a feature
  with its own harness lane; none is a cleanup.

**Exit.** The README and comparison allow-list name every frozen table and
its provenance, so "engine-only" is a precise claim: *all non-lexical tables
are engine-generated; the lexical tables are committed reference data.*

---

## G7 — switch the degree generator (safe) — **done**

Not a gap. Engine output is a strict superset:

- comparatives: 4 859 shared, 0 lost, +956;
- superlatives: 1 934 shared, 0 lost, +3 817.

**Fix.** Replace `build_adj_degrees.py`'s Omorfi/Voikko body with the engine
loop already prototyped:

```python
nom = [f.word for f in engine.inflect(lemma)
       if f.case == "nominative" and f.number == "singular"]
comparative = next(w for w in nom if w.endswith("mpi") and w != lemma)
superlative = next(w for w in nom if w.endswith("in") and w not in (lemma, comparative))
```

Keep the adjective + pronoun lemma set (the `usea` note). Regenerate
`data/adj_degrees.csv`, `maps/adj_degrees.bin` and the tests.

**Exit.** `data/adj_degrees.csv` contains every previously committed cell plus
the productive degrees; `cargo test --release --workspace` passes; the
harness shows no new `illegal` rows.

**Status 2026-11-05.** Generator rewritten on the binding, table regenerated
(5 819 rows, 0 comparatives and 0 superlatives lost, +956 / +3 817), blob
rebuilt, 873 lib tests green.

---

## Order of work

1. **G0** comparison tool (no behaviour change).
2. **G7** degree generator (safe superset).
3. **G1** compound-modifier coverage (do not put classless bases in the map).
4. **G2** class-100 rows → regenerate the verb map without its two losses.
5. **G3** negation/connectives through the engine → drop the tool post-pass.
6. **G4** `all_forms` audit → regenerate the fast form set.
7. **G5/G6** document and freeze the remaining reference tables.

After G1–G4 the only non-engine inputs are the frozen lexical tables, and
`tools/build/` can drop its sqlite/Omorfi dependencies except the reference
generators, which move to `tools/oracle/`.

## Guardrails

- Every regenerated table is compared with the committed one first; no
  committed data is overwritten until the loss list is explained.
- A missing form is fixed in the inflection engine (`src/expand/`,
  `src/grammar/`), never by a new suffix stripper in `src/suggest/known/`.
- A confirmed lost surface is a bug; an unconfirmed one is removed with the
  audit evidence.
- Regenerate the precomputed blobs after a CSV change and run:
  `cargo test --release --workspace`,
  `python3 tools/test_binding_agrees_with_binary.py`,
  `python3 tools/test_reverse_lookup_recall.py`,
  `python3 tools/report_by_class.py` (or the `show_illegals.py` refresh).
- Oracle-derived generators stay `GPL-3.0-or-later`; nothing from them is
  copied into the engine or a binding.

## Checklist

- [x] G0 comparison script committed and green on the baseline
- [x] G7 degree table regenerated from the engine
- [x] G1 compound-modifier coverage (`agraari`, `ampuma-`, `ruumiin`,
      `avomeren`); 406 -> 59 losses, 874 tests green
- [x] G1c `9*(D)` parenthesised gradation (`nahka` override); `viti` stays rejected
- [x] G1d numeral hundreds (`hundreds_forms`); special prefixes resolve via the head
- [x] G1e nested modifiers resolve through the head (no map gap)
- [ ] G1b 57 classed losses encoded as explained allow-list
- [x] G2 eight class-100 rows resolved (2 remapped, 6 rejected)
- [ ] G3 negation/connectives emitted by the engine
- [ ] G3 verb stem map regenerated without the tool post-pass
- [ ] G4 lost `all_forms` surfaces audited; table regenerated
- [ ] G5 catalogue frozen + drift report
- [ ] G6 lexical tables documented as frozen reference data
- [ ] `tools/build/README.md` says "engine-only" without an asterisk
