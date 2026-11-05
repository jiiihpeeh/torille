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
| noun stem-map bases | 83 307 | 32 115 | — | **51 196** | 4 |
| verb stem-map bases | 10 022 | 10 033 | — | 6 | 17 |
| verb stem-map `(stem, base)` pairs | 195 803 | 237 424 | 150 704 | 45 099 | 86 720 |
| adjective comparatives | 4 859 | 5 815 | 4 859 | **0** | +956 |
| adjective superlatives | 1 934 | 5 751 | 1 934 | **0** | +3 817 |

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
| G1 classless lemmas in `Engine.inflect` | 51 139 of 51 196 lost noun bases | medium | P0 |
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
  (`G1:classless`, `G2:class-100`, `G6:lexical`).

**Exit.** `python3 tools/build/compare_engine_outputs.py` reproduces the
baseline table and fails when a new regression appears.

---

## G1 — classless lemmas must inflect through the binding

**Symptom.** An engine-only noun stem map covers 32 115 bases; the committed
hybrid map covers 83 307. All 51 196 lost bases are sanalista rows, and
51 139 of them are classless compounds (`mahonginruskea`,
`differentiaaliyhtälö`, `älykortti`, `äänestyspaikka`).

**Root cause.** `src/expand/mod.rs::inflect_all` already handles classless
rows: a compound inflects by its head (`compound_head_forms`), a classless
`-inen` adjective as class 38, a classless verb by its verb head. The binding
does not use it: `Engine::plan` (`bindings/src/lib.rs`) builds a plan from the
lemma's sanalista classes, and a classless row contributes no `Inflection`, so
`Engine.inflect` returns nothing.

**Fix.**

- In `bindings/src/lib.rs`, when the row has no usable class, fall back to
  `torille::expand::inflect_all(lemma)` and map its `Form`s to `PyForm`s (the
  same fields the normal path fills). Keep the explicit-class path unchanged.
- Expose whether a fallback happened (`PyForm.confidence` already carries the
  head's confidence) so tooling can distinguish engine-derived compound forms
  from sanalista paradigms.
- In `tools/build/build_stem_to_base_map.py`, include classless lemmas in the
  engine source (the current `classed_lemmas` filter exists only because the
  binding cannot inflect them). Keep the `hybrid` path as the oracle-backed
  comparison until G1 closes.
- Add binding tests: a classless noun compound, a classless `-inen`
  adjective, and a classless verb (`aliarvioida`).

**Files.** `bindings/src/lib.rs`, `src/expand/mod.rs` (reuse, do not
duplicate), `tools/build/build_stem_to_base_map.py`, `playground`/binding
tests.

**Exit.** Engine-only noun stem-map bases ≥ 83 300; the residual losses are
explained (see G1b); `cargo test --release --workspace` and
`python3 tools/test_binding_agrees_with_binary.py` pass; the regenerated map
passes `check_normal` and the class-report gate.

### G1b — the 57 classed losses

After classless rows, 57 bases remain: mostly `99` adverbs/interjections
(`maanantaisin`, `aiemmin`, `pitkin`, `harvoin`), plus `piilevä` (class 10),
`viti` (5*F), `puolikymmentä` (numeral class 51), and parenthesised classes
(`OK` `(18)`, `mezzopiano` `(1)`). They are not nominal paradigms and are
correctly absent from a *noun* stem map. Encode them as an explicit
allow-list with the reason, not as a silent gap.

---

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

## G7 — switch the degree generator (safe)

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

---

## Order of work

1. **G0** comparison tool (no behaviour change).
2. **G7** degree generator (safe superset).
3. **G1** classless binding fallback → regenerate the noun stem map.
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

- [ ] G0 comparison script committed and green on the baseline
- [ ] G7 degree table regenerated from the engine
- [ ] G1 classless `Engine.inflect` fallback + tests
- [ ] G1 noun stem map regenerated from the engine
- [ ] G1b 57 classed losses encoded as explained allow-list
- [ ] G2 eight class-100 rows resolved
- [ ] G3 negation/connectives emitted by the engine
- [ ] G3 verb stem map regenerated without the tool post-pass
- [ ] G4 lost `all_forms` surfaces audited; table regenerated
- [ ] G5 catalogue frozen + drift report
- [ ] G6 lexical tables documented as frozen reference data
- [ ] `tools/build/README.md` says "engine-only" without an asterisk
