# Plural-only lemmas at the engine level

Status: **implemented, core and data layers.** The lexicon now carries a
per-row plural-only bit (`data/plural_only.tsv` → `SAN3` blob), `Form` has a
`combining` role, and the lookup paths separate standalone words from compound
stems. Phase 5 (removing the local per-class `-t` filters) is a cleanup left
for after this lands; the bit already decides the answer. The wizard's
saved-entry `number=pl` remains the assertion for a word the lexicon does not
contain.

## The defect

A plural-only lemma has no standalone singular paradigm. Its citation is the
nominative plural (`housut`, `häät`, `talkoot`, `pidot`, `sakset`) or a
plural-derived nominative (`läksiäiset`). The engine currently decides this
inside the class engines with a per-class shape check:

- `class_1`, `class_6`, `class_7`, `class_9`, `class_10`, `class_13`,
  `class_17`, `class_18`, `class_41`, `class_48` and friends test
  `lemma.ends_with('t')`;
- `class_38` has a separate `-iset` rule (`läksiäiset` ← `läksiäinen`);
- `class_5`, `class_43` and `class_44` deliberately do **not** treat a final
  `-t` as plural-only: `airsoft`, `olut` and `kevät` are singular.

The checks are inconsistent:

| lemma | class | current output |
|---|---|---|
| `housut`, `kasvot`, `aivot` | 1 | plural only |
| `sakset`, `päitset` | 7 | plural only |
| `häät` | 18 | plural only |
| `läksiäiset`, `avajaiset` | 38 (`-iset`) | plural only |
| `talkoot`, `hynttyyt` | 17 | **88 singular forms** (`talkootn`, `talkootta`, …) |
| `pidot` | `1*F` | **71 singular forms** (`pidodn`, `pidota`, …) |

`check("talkootn")` and `check("pidodn")` therefore accept invented singulars,
while `check("housu")` correctly rejects one. (`talkoo` is a real sanalista
lemma of class 17; `talkoot` is the plural-only row that borrows `talkoo`'s
plural cells.) The wizard's `number=pl` cannot fix this: it only covers the
user's own entries, not the 100 k sanalista rows.

## What "plural-only" must mean

It is a property of the **standalone paradigm**, not of the stem. A plural-only
lemma still has a productive **combining stem**:

- `housu-` in `housukauppa`, `housunkannattimet`, `housukangas`;
- `hää-` in `hääkakku`, `häämatka`, `hääsunnittelija`;
- `talkoo-` in `talkooväki`, and the citation `talkoot` unchanged in
  `talkootyö`.

So the engine needs two answers from one paradigm:

1. **Standalone forms**: only the plural cells. `hää`, `housu` and `talkootn`
   are not words.
2. **Combining forms/stems**: every stem, including the singular one. `hää`,
   `housu` and `talkoo` are compound modifiers (`talkoo` is also a real lemma
   of class 17; the point is the stem is available either way).

A post-filter on `inflect_in` that drops singular forms is therefore wrong.
For `häät`, only the suppressed singular cells use `hää`; the plural cells use
`häi`. Dropping the singulars removes `hää` from the stem map and breaks
`hääkakku`, `hääsunnittelija` and the rest of the `hää-` family.

## Data model

### The per-lemma table

`data/plural_only.tsv`, committed and reviewed:

```tsv
# torille-plural-only v1
# lemma	homonymy	source	note
housut		kotus	no singular paradigm
talkoot		kotus	no singular paradigm
pidot		kotus	no singular paradigm
läksiäiset		wiktionary	-inen singular omitted
```

- The key is `(lemma, homonymy)`, not the surface alone. `pito` and `pidot`
  are different rows and only the latter is plural-only.
- `source` is one of `engine`, `kotus`, `wiktionary`, `oracle`, `manual`; `note` is free
  text without tabs. The table is the auditable evidence, not a suffix list.
- The table must be sorted, unique, and staleness-checked against the
  sanalista (a row whose lemma disappears is an error, not a silent miss).
- Classless compound rows are not automatically plural-only. `isotaivot`
  (class 51) can be in the table; `housukauppa` is not.

### The blob bit

The sanalista blob gets one bit per row. The bit is set by `build.rs` from the
committed table:

- bump the magic `SAN2` → `SAN3`;
- use the spare header word at offset 44 as `plural_off`;
- store a `ceil(row_count / 8)` bitset between the homonym table and
  `WORD_ORDER` (or after `TAIL_ORDER`, whichever keeps the offsets simple);
- `EmbeddedLemma::plural_only()` reads the bit;
- `Lemma` gains `pub plural_only: bool`, set by `to_lemmas`.

The bit is per row, not per inflection: all classes on a row describe the same
word. A homonym with a singular reading keeps its own row.

### The form role

`Form` gains one field:

```rust
pub struct Form {
    pub word: String,
    pub readings: Vec<Tags>,
    pub confidence: Confidence,
    /// A form that is real as a compound part but not as a standalone word.
    pub combining: bool,
}
```

- `Form::new` and the four existing struct literals set it false.
- `Form::combining()` is the accessor; no `Tags` or `Confidence` change.
- `inflect_in` marks a form `combining` when the lemma's row says plural-only
  **and** the form's union is not plural. It does not drop the form.
- `inflect_all`'s derived layers (`-inen`, degrees, compound heads) are
  separate lexemes; only the base class paradigm is marked.

The existing `Confidence` axis answers "how was this built"; `combining`
answers "where may this surface be used". Overloading `Confidence::Repaired`
would make accepted-and-not-suggestible forms indistinguishable from
compound-only forms, so it is a separate field.

## Consumers

### Inflection and stems

- `inflect_in`, `inflect_paradigms`: full paradigm, forms marked.
- `inflect_all`: full paradigm plus derived layers; only the base forms are
  marked.
- `gen_stem_map_noun` / `gen_stem_map_verb`: unchanged; they consume the full
  paradigm, so `hää`, `housu` and `talkoo` remain stem-map keys.
- `gen_all_forms`: decide explicitly. The standalone form list should exclude
  combining forms; if the harness wants the full list, write it as a separate
  `combining_forms` column/file rather than silently reintroducing the bug.
- `gen_form_table` (branch `form-table`): a row needs the same `combining` bit,
  or the table must omit the combining split. `find_bases` must not return a
  combining row as a standalone base.

### `FormBuilder`

`FormBuilder::add` currently cites every lemma with a nominative singular
reading. For a plural-only row it must cite the nominative plural instead.
It also stores every generated surface in `known` and `analyses`.

Add a separate role:

- `analyses`, `bases_of`, `readings_of`: the union, so compounds can resolve
  `hää`, `housu`, `talkoo`;
- `standalone_bases_of`, `standalone_readings_of`: exclude forms flagged
  `combining`;
- `known` remains the union set for compound compatibility; add a
  `combining` bitset so `is_standalone` is a cheap check;
- `suggestible` is set only for standalone plain forms.

`check`, `suggest` and `check_normal` use the standalone methods. `compound`
keeps the union.

### `find_bases` and `check_normal`

- `find_bases_with` returns `SourcedForm`; a combining form is kept in the
  result but `find_bases`/`find_any_base_with` must expose a standalone-only
  variant. Gate 5 of `check_normal` uses the standalone variant.
- The stem-map probes (`find_base_via_stem_map`, `find_base_via_verb_stem_map`)
  verify with `emits_form`; that predicate must distinguish standalone from
  combining. A compound path may still ask the combining question.
- The form table, when merged, must carry the same distinction.
- `check_normal` gate 1 (`is_known_word`) must not report `talkoo` or `hää` as
  a known word; the citation and compound paths are the only ways in.

### Wizard

Once the bit exists:

- `Report` (or the wasm report object) exposes `recorded_plural_only`;
- the UI preselects **vain monikko** for a known plural-only lemma;
- the saved-entry `number=pl` remains the assertion for a word the lexicon does
  not contain. The two use the same `Number` filter, not two mechanisms.

## Sourcing the table

Do not derive the table from `class + ends_with('t')` in the build. The class
engines already tell us which shapes they intend, and the differences above
show why the shape is not enough. Use a generator plus review:

1. **Candidates**: every sanalista row ending in `-t` whose class currently
   strips the `-t` citation (class 1, 6, 7, 9, 10, 13, 17, 18, 41, 48, …), and
   every class-38 `-iset` row. This is a mechanical first list.
2. **Contradictions**: query `data/kotus_nomines.sqlite` for `ok='both'`
   singular forms. A candidate with a confirmed singular is not plural-only
   (or is a homonym and needs its own row). The oracle has only `engine` rows
   for `housut`/`häät`/`talkoot`, so it can reject a candidate but cannot
   confirm one; that is fine.
3. **Curated additions**: `talkoot`, `hynttyyt`, `pidot` and any other class
   17/`1*F` gaps the current engines miss, with the Kotus/Wiktionary evidence
   in the `source`/`note` columns. `PLAN_N1000.md` already names Wiktionary as
   the final authority for this kind of lexical fact.
4. **Review**: the committed diff is the review artifact. The table is small
   (the `-t` rows are ~1,260, but most are classless compounds or class 5
   loans; the classed candidates are a few hundred), and every row carries its
   evidence.

A new `tools/oracle/build_plural_only.py` should write the candidate table
from the oracle/sanalista; a human decides the exceptions. Do not run the
oracle at build time.

## What landed

The first implementation is deliberately the smallest complete slice:

- `data/plural_only.tsv` has 388 rows: the `-t` citations whose class engine
  currently emits only plural forms (`source=engine`), plus three reviewed
  class-17 / `1*F` gaps (`talkoot`, `hynttyyt`, `pidot`, `source=manual`). The
  oracle's `ok='both'` singular rows are a contradiction check; the table is
  the authority. `tools/oracle/build_plural_only.py` regenerates the candidate
  list.
- `SAN3` sanalista blob: one bit per row in a section addressed by the spare
  header word at offset 44. `EmbeddedLemma::plural_only()` and
  `Lemma::plural_only` expose it; `parse_sanalista` consults the same committed
  table through `src/plural_only.rs`.
- `Form.combining` marks a form that is real as a compound part but not as a
  word. `inflect_in_raw` marks the class forms, and `walk_generated` marks the
  classless/compound/derived layers.
- `FormBuilder` keeps `known` as the union for compounds and adds a
  `standalone` bitset; `standalone_bases_of`/`standalone_readings_of` are the
  check-facing methods. The citation reading is nominative plural for a
  plural-only row.
- `find_bases`, `lemma_emit`, the stem-map probes and the `check` tool use
  `emits_standalone_form`, so a combining singular never becomes a standalone
  base. `compound` keeps the union.
- A plural-only `-t` citation with a singular lemma row resolves through it:
  `pidot` borrows `pito`'s plural cells, and `talkoot` borrows `talkoo`'s.
  `hynttyyt`, which has no singular row, keeps its class-17 cells with the
  singulars marked combining.
- `gen_all_forms` excludes combining forms, so the fast form set no longer
  contains `pidodn`/`talkootn`. `gen_stem_map_noun` keeps them, so `hää`,
  `housu` and `talkoo` remain stems. `data/stem_to_base_map.csv`,
  `maps/all_forms.bin.gz` and `maps/stem_map_noun.bin.gz` were regenerated.
- The wizard report exposes `recorded_plural_only`; a known plural-only lemma
  defaults to the `gen_pl` pool and the UI preselects **vain monikko**.

`talkoo` is a real sanalista lemma (class 17) as well as the combining stem, so
it is not in the table; `talkootn` is. `pito` and `pidot` are separate rows,
and only `pidot` is marked.

Phase 5 (removing the local class filters) is not part of this slice. The
local filters are redundant with the bit, not contradictory; removing them is
a follow-up cleanup with no user-facing answer to gain.

## Measured results

The answer matrix after the change:

| input | result |
|---|---|
| `housu`, `hää`, `hynttyy`, `talkootn`, `pidodn` | unknown (the last two also get the right suggestions) |
| `housut`, `häät`, `talkoot`, `pidot`, `hynttyyt` | known |
| `housukauppa`, `hääkakku`, `hääsuunnittelija`, `talkooväki`, `talkootyö` | known |
| `talkoo`, `pito` | known (their own sanalista rows) |
| `kevät`, `olut`, `airsoft` | known with their singulars |

Gates:

- `cargo test --release --workspace` and clippy clean;
- binding agreement: all 23 lemmas identical;
- reverse-lookup recall: 97.59 % first base, 5.81 % multi-base;
- `compare_engine_outputs.py --strict`: `all_forms` 6,475,804 rows,
  `stem_map_noun` 245,184 pairs, all shared, 0 lost/new; the pre-existing
  `stem_map_verb` lost=4/new=4 remains;
- `test_illegal_evidence.py`: 463 illegal / 8 missing / 3,711 unsupported,
  every illegal row explained, 128 rules live;
- wizard core/page smoke and both extension `npm test` suites pass;
- maps republished to both remotes at `b65e190`.

## Phases

| phase | deliverable | gate |
|---|---|---|
| 0 | Audit every class's `-t`/`-iset` behaviour; agree the TSV schema and the blob layout. No code. | The audit table is in this plan; the candidate list is reproducible. |
| 1 | `data/plural_only.tsv`, `SAN3` blob bit, `Lemma::plural_only`, `EmbeddedLemma::plural_only`. No behaviour change. | Blob round-trip test; `cargo test`; `compare_engine_outputs --strict` unchanged; blob size recorded. |
| 2 | `Form.combining`; `inflect_in` marks the suppressed singulars; no filtering yet. | The mark is visible and no existing test moves; `Form` constructors updated. |
| 3 | Standalone vs combining in `FormBuilder`, `find_bases`, `check_normal`; citation reading changed. | `check("talkootn")`/`check("hää")` unknown; `check("talkoot")`/`check("häät")` known; existing tests green. |
| 4 | Apply the table; fix class 17 and `1*F`; regenerate stem maps, `all_forms`, and the form table (when merged). | `talkoot` emits no standalone singular; `hääkakku`, `housukauppa`, `hääsunnittelija`, `talkooväki`, `talkootyö` stay known; stem map has `hää -> häät`, `housu -> housut`, `talkoo -> talkoot`. |
| 5 | Remove the local per-class `-t` **filters** (not the stem construction); the bit is authoritative. | Each class's tests pass; no class-specific shape check remains where the bit decides. |
| 6 | Wizard preselect, docs, `data/README.md`, `maps/README.md`, harness baselines. | Full gates below. |

Phase 4 is the only phase that changes what `check` accepts. Keep it in one
commit with the regenerated maps so the diff is attributable.

## Tests

- **Blob**: a synthetic lexicon with a plural-only row and a singular homonym;
  the bitset round-trips, and `Lemma::plural_only` is per row.
- **Marking**: `hynttyyt` class 17 (no singular lemma row) has plural forms
  plus combining singulars; `talkoot`/`pidot` resolve through `talkoo`/`pito`
  and emit no singular cells; `housut` has no standalone singular; `airsoft`,
  `olut`, `kevät` have no combining forms.
- **Standalone**: `hää`, `housu`, `hynttyy`, `talkootn` and `pidodn` are
  unknown; `housut`, `häät`, `talkoot`, `pidot` and `hynttyyt` are known.
  `talkoo` and `pito` are real lemmas of their own.
- **Compounds**: `housukauppa`, `housunkannattimet`, `hääkakku`,
  `hääsunnittelija`, `talkooväki`, `talkootyö` known; the singular stem is
  still a stem-map key.
- **Stem map**: `hää -> häät`, `housu -> housut`, `talkoo -> talkoot`.
- **Wizard**: a known plural-only lemma preselects `number=pl`; a novel
  plural-only entry still uses the saved flag.
- **Gates**: `cargo test --release --workspace`, clippy, `check_docs --render`,
  the binding agreement, reverse-lookup recall, `compare_engine_outputs
  --strict`, `test_illegal_evidence.py`, and the wizard smokes.
- **Baselines**: record the before/after `illegal` rate and `known` word count.
  Removing invented singulars is expected to reduce the known set; rank by
  rate, not count, and document the move in `docs/harness.md`.

## Risks and open questions

- **False positives in the table.** A word with a rare but real singular would
  lose it. The evidence columns and the oracle contradiction check are the
  guard; the review diff is the decision.
- **Homonyms.** The bit is per row, so `pito`/`pidot` can disagree. A table
  keyed only by surface would not.
- **Compound over-correction.** Do not filter `inflect_in`; keep the full
  paradigm and mark. The `hää` case is the regression test.
- **`FormBuilder`'s union.** `bases_of` is used by both check and compounds;
  the plan adds standalone methods rather than changing the existing answer.
- **Form table branch.** `form-table` is not on `master`; rebase it after this
  lands and add the row bit. Until then, the branch's table must not be
  regenerated from a filtered engine.
- **Harness baselines.** This is a correctness fix, not a performance change;
  a shifted `illegal` or recall number must be explained by the removed forms,
  not waved through.
- **Possessives.** A possessive form may be numberless (`maitoni`). Decide
  whether it belongs to the standalone set or the combining set; test both a
  plural-only and a singular-only lemma before choosing.
- **Class 51/50.** `isotaivot` is already plural-only through its class;
  whether the table should also name it is a consistency question, not a bug.
- **Wasm size.** The bitset is ~13 KB raw and compresses to almost nothing;
  this is not a download decision like the form table.

## Files

- `data/plural_only.tsv` (new), `data/README.md`;
- `build/csv_builders/sanalista.rs`, `build.rs`;
- `src/kotus.rs` (`Lemma`, `EmbeddedLemma`, blob reader);
- `src/tags.rs` (`Form.combining`);
- `src/expand/mod.rs` (`inflect_in` marking), `src/expand/build.rs`
  (standalone/combining sets), `src/expand/find_bases.rs`;
- `src/expand/nominals/` (remove the local filters in phase 5; keep the stem
  construction);
- `tools/src/bin/gen_stem_map_noun.rs`, `gen_all_forms.rs`,
  `gen_form_table.rs` (branch), `tools/oracle/build_plural_only.py`;
- `src/class_wizard.rs`, `torille-wasm/src/lib.rs` (wizard preselect);
- `misc_md/PLAN_WORD_WIZARD.md` (link the engine bit), `docs/lexicon.md`,
  `docs/harness.md`, `AGENTS.md`.
