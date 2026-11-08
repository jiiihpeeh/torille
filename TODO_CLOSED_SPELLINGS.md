# TODO: shrink `closed_spellings::CLOSED`

Status: **residue reached (32 → 6 rows)** · owner: — · created: 2026-11-08

The closed-spelling table in `src/expand/closed_spellings.rs` has 32
`(surface, base)` pairs. Most are not closed spellings: they are ordinary
inflection, productive derivation, compound analysis, proper-name data, or
plain misspellings. The table also serves the fast per-keystroke path, so a row
can only go when **both** the full engine and `check_normal_fast` resolve the
surface without it.

Goal: reduce 32 rows to the genuine residue (roughly 3–8), each with an
authority source and a test proving the engine still cannot derive it.

Related context: `PLAN_FAST_PATH_REMOVAL.md` (the table's origin and rules),
`AGENTS.md` (“No shortcuts in the inflection detection”).

## Ground rules

- [ ] No new suffix-stripper entries in `is_known_word`; fix the class,
      derivation, compound, or data layer instead.
- [ ] A row may stay only if it is a fused spelling or lexical variant that
      morphology cannot derive **and** Kotus/Kielitoimiston sanakirja accepts.
- [ ] Every row needs a source comment (Kotus, Kielikello, KS, Wiktionary) and
      a staleness test: if the engine later derives it, the row fails.
- [ ] The table is consulted only after the cheap gates; it must not shadow a
      regular paradigm base.
- [ ] Do not hand-edit generated files; regenerate `nominals/`, stems, blobs,
      and maps through their generators.
- [ ] Keep `CLOSED` sorted; `lookup` is a binary search.

## Current audit findings

Read-only probes on the current tree (`inflect(base)`, `look_base`,
`check_normal`, sanalista rows):

- [ ] Record in the audit fixture:
  - `enemmittä`, `keskenämme`, `seuduitta`, `suurkaanin` are already emitted by
    `inflect(base)` and found by `find_bases`; only the fast path needs work.
  - `honkanen`, `selma`, `unhotus` are **not** sanalista lemmas.
  - `honkasien` and `selmallakaan` are lowercase proper-name forms; the checker
    deliberately rejects lowercase names
    (`proper_names::contains_capitalised`).
  - Kotus/Kielitoimiston ohjepankki accepts `ylhäältä päin ~ ylhäältäpäin`
    (and the adverb/particle + `päin` rule generally).
  - Kotus writes these as two words: `edeltä käsin`, `itse kunkin`,
    `sen tähden` family, `sitten kun`, `yhtä kaikki`.
  - `äkkipikaa` is a genuine one-word adverb (Wiktionary).


## Progress (2026-11-08)

Removed from `CLOSED` (32 → 6 rows):

- [x] `edeltäkäsin`, `itsekunkin`, `niinkauan`, `senjälkeen`, `silloinkun`,
      `yhtäkaikki` — Kotus two-word spellings; moved to `is_nonword`.
- [x] `honkasien`, `selmallakaan` — lowercase proper-name forms; the name gate
      only accepts capitalised forms.
- [x] `kehottavasti`, `koneentapaisesti` — derived through `-sti`; the fast
      path now probes `derived_sti_base` before the compound DP split.
- [x] `nyrpistyksessä` — `-us`/`-ys` action noun oblique `-kse-` stem is
      reconstructed; the source verb may take `-aa`/`-ää` as well as
      `-ta`/`-tä`.
- [x] `perehtyneempi` — the comparative nominal is built from the `-nee-`
      stem; the fast path probes `derived_comparative_base` before the DP
      split.
- [x] `seuduitta` — plural abessive `itta`/`ittä` added to `ENDINGS` and the
      stem-key generator.
- [x] `kävellytti` — the double-`l` spelling is not the causative; the correct
      `kävelytti` (one `l`) is already a form of `kävelyttää` on both paths.
      Voikko confirms `kävelytti`, and the row is deleted, not replaced.
- [x] `kuulten`, `kuulteni`, `kuultesi`, `kuultemme`, `kuultenne`,
      `kuultensa` — en.wiktionary's `kuulla` paradigm gives only the bare
      instructive `kuullen`, which the class-67 engine already generates.
- [x] `ventovieraita` — `ventovieras` is `vento` + `vieras`; `vento` has no
      lemma row, so the compound layer now accepts it as an opaque bound
      modifier.
- [x] `enemmittä`, `loitommalla` — generated reverse-degree map
      (`maps/degree_forms.bin.gz`); `enempi` is covered as a comparative
      lemma, `loitto` by a curated `adj_degrees_extra.csv` row.
- [x] `hiljempaa` — curated `data/adverb_degrees.csv`, emitted by the engine
      and included in the reverse-degree map.
- [x] `keskenämme` — generated possessive-adverb map
      (`maps/adverb_possessive.bin.gz`) from the class-99 paradigms.
- [x] `suurkaanin` — fast compound naming now returns the whole classless
      compound lemma (`suurkaani`, `raitiotie`) instead of its head.

Kept after verification:

- `tienpuoleen` — Voikko analyses it as `tienpuoli`; restored to `CLOSED`.
- `pormestarinna` — historical `-nna` feminine (wife of a titled man), a
  different system from productive `-tar`/`-tär`; left in place until the
  lexical family table/lemma lands (see the `-nna` section below).

## Final residue

The six rows that remain are either genuine accepted fused spellings or
lexical forms whose base has no sanalista row:

- `alhaaltapäin`, `ylhäältäpäin` — Kotus allows adverb/particle + `päin`
  written together.
- `äkkipikaa` — lexical adverb (`suddenly`), not a productive suffix.
- `pormestarinna` — historical `-nna` feminine, lexical; awaiting the
  `-nna` family table (see the feminine section below).
- `tienpuoleen` — base `tienpuoli` (compound, Voikko-confirmed) has no
  sanalista row.
- `unhotusta` — base `unhotus` (action noun, Voikko-confirmed) has no
  sanalista row.

Everything else was deleted by the phases above or moved to
`is_nonword`/the derived/fast maps.

## Row disposition

Legend: **A** delete as nonword/mis-spelling · **B** engine/derivation/data
fix, then delete · **C** fast-path probe, then delete (engine already derives) ·
**D** keep as genuine closed spelling.

| surface | base | disposition | action |
|---|---|---|---|
| `alhaaltapäin` | `alhaalta` | D | keep; verify exact Kotus entry |
| `edeltäkäsin` | `edeltä` | A | Kotus: `edeltä käsin` |
| `enemmittä` | `enempi` | C | fast degree probe |
| `hiljempaa` | `hiljaa` | B | adverb-degree data/rule + fast probe |
| `honkasien` | `honkanen` | A | lowercase surname; delete |
| `itsekunkin` | `itse` | A | Kotus: `itse kunkin` |
| `kehottavasti` | `kehottaa` | B | fix `-sti` / participle derivation |
| `keskenämme` | `keskenään` | C | fast adverb-possessive probe |
| `koneentapaisesti` | `tapaisesti` | B | compound adjective + `-sti` |
| `kuultemme` | `kuulten` | B/D | decide as one family |
| `kuulten` | `kuulten` | B/D | decide as one family |
| `kuulteni` | `kuulten` | B/D | decide as one family |
| `kuultenne` | `kuulten` | B/D | decide as one family |
| `kuultensa` | `kuulten` | B/D | decide as one family |
| `kuultesi` | `kuulten` | B/D | decide as one family |
| `kävellytti` | `kävellä` | A/B | verify; likely typo/non-form |
| `loitommalla` | `loitto` | B | degree data/rule + fast probe |
| `niinkauan` | `niin` | A | verify Kotus; two words |
| `nyrpistyksessä` | `nyrpistää` | B | fix `-us` action noun `-kse-` |
| `perehtyneempi` | `perehtyä` | B | fix comparative reconstruction |
| `pormestarinna` | `pormestari` | B/D | `-nna` family or lexical lemma |
| `selmallakaan` | `selma` | A | lowercase name; delete |
| `senjälkeen` | `sen` | A | Kotus: `sen jälkeen` |
| `seuduitta` | `seutu` | C | abessive plural ending/stem key |
| `silloinkun` | `silloin` | A | Kotus: `sitten kun`; two words |
| `suurkaanin` | `suurkaani` | C | fast classless-compound probe |
| `tienpuoleen` | `tie` | A | verify; noun + `puoleen` |
| `unhotusta` | `unhotus` | A/B | verify; delete or add class-39 lemma |
| `ventovieraita` | `vieras` | B | compound modifier `vento` |
| `yhtäkaikki` | `kaikki` | A | Kotus: `yhtä kaikki` |
| `ylhäältäpäin` | `ylhäältä` | D | keep; Kotus allows both |
| `äkkipikaa` | `pikaa` | D | keep; genuine adverb |

## Phase 0 — instrument before deleting

- [ ] Refactor the closed lookup into a policy parameter:
      `check_normal_with(word, engine_gate, closed: Closed)` and
      `is_known_word_with_compound_uncached(word, allow_compound, closed: Closed)`.
      Production passes `Closed::Enabled`; tests/audit pass
      `Closed::Disabled`.
- [ ] Add `tools/src/bin/audit_closed_spellings.rs` printing:
      `surface`, `base`, `base_in_sanalista`, `engine_emits`,
      `find_bases_base`, `fast_resolves_without_closed`, `proper_name`,
      `authority`, `proposed_action`.
- [ ] Add a checked-in expected-action fixture for all 32 rows.
- [ ] Make the audit fail when:
  - [ ] a row marked “engine cannot derive” starts being derived;
  - [ ] a base is absent from the lexicon and not documented as a proper
        name/adverb;
  - [ ] a row marked “fast-only” lacks the probe that replaces it.
- [ ] Record the baseline: `check_normal` / `check_normal_fast` outcomes,
      suggestions for rejected forms, and `check_fast` timings.
- [ ] Add a full/fast parity test for every surface in the table.

## Phase 1 — delete invalid rows (no engine work)

Move to `is_nonword`, with the split spelling offered by the suggester:

- [x] `edeltäkäsin` → `edeltä käsin`
- [x] `itsekunkin` → `itse kunkin`
- [x] `niinkauan` → `niin kauan` (verify Kotus first)
- [x] `senjälkeen` → `sen jälkeen`
- [x] `silloinkun` → `silloin kun`
- [ ] `tienpuoleen` → `tien puoleen` (verify Kotus first)
- [x] `yhtäkaikki` → `yhtä kaikki`

Delete outright:

- [x] `honkasien` — lowercase surname; pin the rejection.
- [x] `selmallakaan` — lowercase name; ensure `Selmallakaan` (capitalised)
      still resolves.
- [ ] `unhotusta` — verify; if `unhotus` is real, add a class-39 lemma
      instead.
- [x] `kävellytti` — the double-`l` spelling is a misspelling; `kävelytti`
      (one `l`) is already derived. Deleted and pinned.

Tests:

- [ ] `check_normal_fast` and the full path both return `Unknown`.
- [ ] The suggestion list contains the split spelling for each deleted fused
      form.
- [ ] No lowercase proper name is accepted.

## Phase 2 — cover the four engine-derived rows on the fast path

- [ ] `enemmittä` — add a reverse adjective-degree probe. Prefer a generated
      `degree form -> positive lemma` table (inverse of
      `data/adj_degrees.csv` + productive class-16/36 rules) consulted from
      `cheap_derived_base`; avoid a hand-written `-mmittä` suffix rule.
- [ ] `keskenämme` — add adverb-possessive coverage (data row in
      `data/adverbit.csv`/its generator, or a small `adverb_possessive_base`
      that strips the possessive and consults the adverb table).
- [x] `seuduitta` — plural abessive `itta`/`ittä` added to `ENDINGS` and
      `tools/src/stem_key.rs`; `data/stem_to_base_map.csv` regenerated.
- [ ] `suurkaanin` — teach the fast compound path the classless-compound case:
      known modifier (`suur`) + known head noun (`kaani`) + head paradigm
      emits the surface.
- [ ] Remove the four rows only after both paths resolve them.
- [ ] Add fast/full parity tests for all four.
- [ ] Benchmark `check_normal_fast`; the probes must stay bounded.

## Phase 3 — fix engine/derivation gaps

Work one family at a time; each fix gets a family test, not just the reported
word.

### `-sti` from participles and compound adjectives

- [x] `kehottavasti`: the fast path now probes `derived_sti_base` before the
      compound DP; the existing participle path names `kehottaa`.
- [x] `koneentapaisesti`: the compound-adjective branch names
      `koneentapainen`; both paths agree.
- [ ] Add tests for other participles and compound adjectives.
- [ ] Remove both rows.

### `-us` action nouns with an oblique `-kse-` stem

- [x] `nyrpistyksessä`: `derived_action_us_base` reconstructs the `-kse-`
      stem and accepts `-aa`/`-ää` source verbs.
- [ ] Add tests for `edistys`/`päätös`/`yritys`-type families so the fix
      generalizes.
- [ ] Remove the row.

### Participle comparative

- [x] `perehtyneempi`: `derived_comparative_base` builds the nominal from the
      `-nee-` stem and returns `perehtynyt`; the fast path probes it before the
      DP split.
- [ ] Add tests for `-nee-` participles and ordinary vowel-stem adjectives.
- [ ] Remove the row.

### Degree data/rules

- [ ] `loitommalla`: extend `derived_comparative_base` to oblique comparative
      cases (strip case ending, map weak `-mma-`/`-mpa-` to the class-16
      `-mpi` nominative, verify), **or** add `loitto` to
      `data/adj_degrees.csv` from Omorfi and expose it through the degree
      reverse probe. Verify class 16 emits `loitompi` → `loitommalla`.
- [ ] `hiljempaa`: add an adverb-degree mechanism (`hiljaa` →
      `hiljempaa`/`hiljemmin`) as data or a small productive rule, with a fast
      probe. Keep it out of `ADJ_DEGREES`.
- [ ] Remove both rows.

### Feminine `-nna` (lexical, not a suffix rule)

`-nna`/`-nnä` is a separate historical system from productive `-tar`/`-tär`:
it usually names the **wife of a titled man** (`pormestarinna`,
`tohtorinna`, `rovastinna`, `ruustinna`, `monarkinna`); `keisarinna` is the
one that also denotes a woman holding the office. These are old-fashioned and
lexical. Do not add an `-nna` suffix stripper to `is_known_word`.

- [ ] Audit the family against the sanalista and en.wiktionary/Kotus:
      `keisarinna` is already a lemma; check `tohtorinna`, `rovastinna`,
      `ruustinna`, `monarkinna`.
- [ ] Prefer a small curated lexical table (`data/feminine_nouns.csv`,
      form -> masculine base) embedded as a form map, or sanalista rows where
      the authority lists them. Keep `pormestarinna -> pormestari`.
- [ ] If a table is added, generate/embed it like `adverbit`/the closed
      tables and probe it in the fast path; do not generalise to other `-nna`
      words.
- [ ] Remove the `pormestarinna` closed row once the table/lemma lands.

### Compound with an opaque modifier

- [x] `ventovieraita`: `vento` added to the opaque-modifier list in
      `modifier_is_known`; both paths resolve the form.
- [x] Test the `ventovieras` compound form.
- [x] Remove the row.

### `kuulten` family (six rows)

- [ ] Decide authority first: is `kuulten` a productive non-finite form of
      `kuulla`/`kuultaa`, or a lexical/archaic adverb?
- [ ] If productive: implement the instructive + possessive form in the verb
      engine/non-finite possessive layer; one fix removes six rows. Test
      `kuulla`, `kuultaa`, and another class-67 verb.
- [ ] If lexical: add a defective `kuulten` entry with its possessive forms
      (data-backed), or keep the six rows as the genuine closed family.
- [ ] Do not add six suffix rules.

## Phase 4 — shrink the accepted residue

Likely residue: `alhaaltapäin`, `ylhäältäpäin`, `äkkipikaa`, plus any kept
`pormestarinna` decision.

- [ ] Add a source comment to every surviving row.
- [ ] Add the staleness test (`inflect(base)` must not emit the surface).
- [ ] Decide whether to keep the code table or move to
      `data/closed_spellings.csv`; if a CSV is added, document its producer.
- [ ] Consider a general adverb/particle + `päin` rule for
      `alhaaltapäin`/`ylhäältäpäin`. Only if it does not accept noun + `päin`
      (`kotiin päin` must stay separate).
- [ ] Keep `äkkipikaa` as a lexical adverb unless an authority rejects it.

## Phase 5 — regenerate and gate

Data changes that need regeneration:

- [ ] `data/nykysuomensanalista.csv` and `src/sanalista_overrides.rs` for any
      added lemma (`unhotus`, `perehtynyt`, `pormestarinna`, `kävellyttää`,
      `ventovieras`).
- [ ] `data/adj_degrees.csv` for `loitto`/participle degrees.
- [ ] `data/adverbit.csv` for `hiljaa`/`keskenään` families.
- [ ] `data/stem_to_base_map.csv` and `data/verb_stem_to_base_map.csv` after
      `ENDINGS`/stem-key changes.
- [ ] `data/proper_name_forms.csv` only if a name form is genuinely missing;
      do not accept lowercase names.

Commands:

```bash
cargo run --release -p torille-tools --bin gen_all_forms
cargo run --release -p torille-tools --bin gen_adj_degrees
cargo run --release -p torille-tools --bin gen_stem_map_noun
cargo run --release -p torille-tools --bin gen_stem_map_verb
cargo run --release -p torille-tools --bin gen_derived
cargo run -p torille-maps -- publish   # only after the blobs are final
```

Gates:

- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --release --workspace`
- [ ] `python3 tools/test_binding_agrees_with_binary.py`
- [ ] `python3 tools/test_reverse_lookup_recall.py`
- [ ] `python3 tools/build/compare_engine_outputs.py --strict`
- [ ] `python3 tools/test_illegal_evidence.py`

Additional checks:

- [ ] `check_normal_fast`/`check_fast` benchmark before and after.
- [ ] Hyphenation corpus tests; `hyphenation/compounds.rs` consults
      `closed_spellings::base`, so the residue is part of its known-head set.
- [ ] Suggestion tests: every deleted fused form offers the split spelling.
- [ ] Full/fast parity for every surface in the table.
- [ ] Blob-consistency tests (`the_blobs_match_compute`, pronoun/derived blobs).

## Open decisions

- [ ] `edeltäkäsin`, `niinkauan`, `tienpuoleen`: confirm Kotus and reject as
      two-word spellings.
- [ ] `koneentapaisesti`: confirm the one-word compound reading.
- [ ] `kävellytti`: typo of `kävelytti`, form of `kävellyttää`, or invalid?
- [ ] `kuulten` family: productive non-finite or lexical/archaic?
- [ ] `pormestarinna`: `-nna` family or single lexical item?
- [ ] `unhotusta`: archaic but valid (add lemma) or invalid (delete)?
- [ ] `alhaaltapäin`: confirm the exact Kotus entry for the `alhaalta` case.

## Success criteria

- [ ] `CLOSED` is ≤ 8 rows, each with an authority comment and a staleness
      test.
- [ ] No row remains that `inflect(base)` or a fixed derivation/compound layer
      can produce.
- [ ] `check_normal` and `check_normal_fast` agree on every row and every
      removed surface.
- [ ] Deleted fused spellings are rejected and their split form is suggested.
- [ ] Lowercase proper names stay rejected; capitalised forms still work.
- [ ] No regression in `check_fast` latency, the harness gates, hyphenation, or
      suggestion ranking.

## Infrastructure plan for the last five rows

These five surfaces need shared fast-path infrastructure rather than another
closed row. The design keeps generation in the engine, stores only a reverse
map, and leaves the fast path bounded.

### 1. A generated reverse-degree map

Covers `enemmittä` (`enempi`), `loitommalla` (`loitto`) and, with the adverb
table, `hiljempaa` (`hiljaa`).

- [ ] Add a compute function in the library, e.g.
      `expand::degrees::compute_degree_forms() -> HashMap<String, &'static str>`.
      For every classed adjective lemma:
  - take the comparative and superlative nominatives from `ADJ_DEGREES` plus
    the productive `-inen`/`-kas`/`-nut`/`-ttu` fallbacks;
  - inflect the comparative as class `16*H` and the superlative as class `36`;
  - map each generated **case form** to the positive lemma.
- [ ] Include comparative lemmas themselves: `enempi` is class `16*H` and is
      its own lemma, so map all of its paradigm forms to `enempi`. This is the
      `enemmittä` case; it is not in `ADJ_DEGREES`.
- [ ] Write the table from `gen_derived` as `maps/degree_forms.bin.gz` using
      the existing `DER1` writer (`torille_tools::der1::write_table`), the
      same pattern as `pronoun_forms.bin.gz`.
- [ ] Add the blob to the `REQUIRED`/decompress loop in `build.rs` and a
      `the_blob_matches_compute` test in the module that owns the map.
- [ ] Add a runtime `degree_base(word)` probe (binary search over the blob)
      and call it in `cheap_derived_base` **after** the noun/verb stem maps
      and **before** gate 4c's compound DP. That ordering keeps ordinary
      noun/verb readings first and prevents the DP split from naming a
      fragment.
- [ ] Also consult the map from `find_through_possessive`, so a possessive
      on a degree form (`loitommallani`) resolves without storing the whole
      possessive column.
- [ ] Size check: measure the blob and the wasm. If it exceeds the agreed
      budget, switch from *all case forms* to a **stem map**: store the weak
      stem of each degree cell (`enemmi`, `loitomma`, ...) and strip
      `ENDINGS` before probing, as the noun/verb maps do. Start with the form
      map; it is simpler and unambiguous.

**Data gap for `loitto`:** en.wiktionary gives comparative `loitompi`, no
superlative. Add `loitto` to `data/adj_degrees.csv`, but note that
`gen_adj_degrees` regenerates the file from the engine and would drop a
hand-added row. Add a small `data/adj_degrees_extra.csv` (merged by the
generator) or an explicit override list before adding the row. Then the engine
emits `loitommalla` from class 16 and the reverse map contains it.

### 2. Adverb degree data

Covers `hiljempaa` (`hiljaa`).

- [ ] Add `data/adverb_degrees.csv` with
      `lemma<TAB>comparative<TAB>superlative` and a source comment. Start with
      `hiljaa<TAB>hiljempaa<TAB>` (en.wiktionary: comparative `hiljempaa`, no
      superlative).
- [ ] Emit it as a forward `StemMap` (like `ADJ_DEGREES`) and teach
      `degree_forms` an adverb branch that emits the listed comparative as a
      `Repaired` form of the adverb. This makes the full engine recognise it,
      not only the fast probe.
- [ ] Reuse the reverse-degree blob from item 1 for the fast path: the
      compute function reads the adverb table too and maps `hiljempaa` to
      `hiljaa`.
- [ ] Treat the table as curated data (authority-checked), not as something
      `gen_adj_degrees` can infer.

### 3. Generated adverb-possessive map

Covers `keskenämme` (`keskenään`).

- [ ] Add `expand::check_normal::compute_adverb_possessive_forms()`, next to
      `compute_pronoun_forms`. Inflect every adverb lemma whose paradigm emits
      possessive forms (class 99: `keskenään` -> `keskenämme`, `keskenäni`,
      ...) and map each possessive surface to its lemma.
- [ ] Write `maps/adverb_possessive.bin.gz` from `gen_derived` (DER1
      form -> lemma), add it to the `build.rs` loop and a compute-vs-blob
      test.
- [ ] Add `adverb_possessive_base(word)` and probe it in the fast path before
      the generic possessive stripper. The generic stripper lands on
      `keskenä`, which is not a lemma, so a dedicated form map is the
      bounded answer.
- [ ] Keep the map small: only forms whose `tags.possessive` is set and whose
      part of speech is adverb.

### 4. Full compound lemma naming in the fast path

Covers `suurkaanin` (`suurkaani`; fast currently says `kaani`).

- [ ] In the compound naming step (`is_known_compound_with` and the DP path),
      after a split `left + right` resolves `right` to head lemma `H`, form
      the candidate `L = left + H`. If `L` is a classless compound lemma in
      the sanalista, return `L` instead of `H`.
- [ ] Guard it: require the reconstructed lemma to exist, to be nominal, and
      to have no inflection class (the Kotus classless-compound convention).
      A regular compound (`taloilta` -> `ilta`) must still name `ilta`.
- [ ] Reuse the reconstruction in both the general split and
      `is_known_compound_dp`; the DP path is the one that currently names
      `kaani` for long words.
- [ ] Test `suurkaanin` on both paths, plus negative cases (`taloilta`,
      `kirjahyllyssä`) to pin the head naming.
- [ ] No new blob: this is a cheap suffix/prefix check on the existing lexicon
      hash.

### 5. Shared plumbing

- [ ] New blobs use the existing `DER1` reader/writer and `build.rs`
      decompression loop; do not add a second table format.
- [ ] Put the runtime probes in `src/expand/degrees.rs` (degree forms) and
      `src/expand/check_normal.rs` (adverb possessive), with the compute
      functions public for `gen_derived` and the blob tests.
- [ ] Regeneration order:
      `gen_adj_degrees`, the adverb-degree/extra data, `gen_derived`,
      `cargo build`, then `cargo run -p torille-maps -- publish`.
- [ ] Gates: `cargo test --release --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`, the Python
      harness gates, and a `check_fast` benchmark. Also record the wasm size
      before/after (the reverse map is the only size risk).
- [ ] After each item lands, delete the row and add a full/fast parity test.
