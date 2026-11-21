# PLAN_PROBE_PERF — targeted membership and negative evidence for cold morphology

Status: **Proposed.** Builds on Phase 12 of
[`PLAN_GRAMMAR_STYLE_PERF.md`](PLAN_GRAMMAR_STYLE_PERF.md) at baseline
`2a4b6c6`: `page-2000` cold morphology is **1,017 ms single-thread /
293 ms on four native workers**, and the pass still spends most of its time
proving that a candidate base cannot generate the target surface.

This plan does **not** change morphology coverage, a reading, a suggestion or a
diagnostic. Every phase has a parity gate against the current engine.

## 1. The measured problem

The cold pass is not short of caching or parallelism. It is doing too much work
per question. On `page-2000`:

| metric | value |
|---|---:|
| unique surfaces | 1,146 |
| paradigm builds | 1,984 |
| failed builds | 908 (46 %) |
| forms visited | 1.38 M |
| average forms per build | 693 |
| verb stem-map visits | 813 k (602 builds, 271 failed, 320 ms) |
| noun stem-map visits | 62 k (740 builds, 123 failed, 130 ms) |
| other visits (cheap gate/deep) | 469 k (599 builds, 486 failed, 220 ms) |

The central inefficiency is:

```text
candidate base + target surface
    → build ~693 forms
    → search the list for the target
```

For a failed candidate the answer is "no", and almost all of the 693 forms were
irrelevant. A per-stem ending signature can often answer "no" before the
inflector starts; a targeted membership path can answer without enumerating the
whole paradigm.

## 2. What is already true

Do not redo these. They were measured or implemented in Phase 12:

- **`emits_form` already separates the probe from the word set.** It caches the
  full `HashSet<String>` plus one `(surface, readings)` probe, and
  `form_readings` reuses that probe. Removing the full set and walking only for
  membership was measured and is **about 2× slower**: successful verifies are
  repeated for later case forms of the same lemma.
- **Unique-surface dispatch already exists.** `analyze_morphology_parallel`
  groups unique surfaces by first four characters, one `MorphologyCache` per
  worker, then merges. The wasm pool's `requestUniqueSurfaces` +
  `runMorphologyPool` does the same for the browser. The remaining parallel
  questions are worker balance and payload cost, not duplicate resolution.
- **The front-coded key search now seeks restart blocks**, and the stem loops
  no longer do `contains` + `bases` (two searches per hit).
- **The paradigm key ignores the `LexiconIndex`** for classed non-50/51
  paradigms, so the morphology and suggestion paths share one build.
- **The exact form table is rejected on size:** 3.59 MB gzip / 22.2 MB raw for
  the grouped ending+readings table, ~10 % of the cold pass. Do not revive it
  as-is. The point of Phase 3 below is a much smaller projection of the same
  evidence.

## 3. Goals and non-goals

### Goals

1. Reduce the number of expensive paradigm builds before they start.
2. Make a membership check cheaper than a full paradigm enumeration, starting
   with the verb classes that dominate the 813 k visits.
3. Remove duplicated probe scans between `resolve_word_fast` and
   `check_normal_fast` without changing either answer.
4. Recover the hash/allocation/format overhead only after the build count is
   under control.
5. Keep all gains measurable on four corpora, not one clean page.

### Non-goals

- No hand-written suffix stripper, no change to `is_known_word` coverage, no
  new `GAPS` exception.
- No merging of the `form-table` branch and no new download budget without an
  explicit decision.
- No class-engine rewrite. Targeted generation starts as a filter over the
  existing engines and only becomes a direct generator for the classes where
  the measurements justify it.
- No change to the suggestion ranking, the keystroke path or the diagnostic
  contract.

## 4. Baseline and corpora

Keep four corpora. The first three can be generated deterministically from
`tests/fixtures/perf/page-2000.txt`:

| corpus | purpose |
|---|---|
| `page-2000`, clean | the shipping page |
| 8 % of words corrupted | realistic typos |
| every word made unknown (`…x`) | worst-case probe path |
| homonym-heavy text | guards reading coverage while optimising |

The homonym corpus is new and mandatory. It must include at least:

```text
tuli (noun / past of tulla), pieniä (adjective partitive / verb),
kuusi (spruce / six), paloin, sata (hundred / to rain),
kuningas, itse, mikä, joka, kuka, talo / talous, vuori (mountain / lining)
```

Add it as `tests/fixtures/perf/homonyms.txt` and pin the expected lemma set in
a test. Optimisations that drop a verb reading from a noun form can look good on
the clean page and still be wrong here.

Add a deterministic generator, not ad-hoc Python:

```bash
python3 tools/gen_perf_corpora.py --source tests/fixtures/perf/page-2000.txt \
    --out tests/fixtures/perf/
```

It writes the corrupted and all-unknown variants with a fixed seed, plus a
manifest of the substitutions. `reports/` stays the output directory for
measurements.

## 5. Phase 0 — instrumentation (first)

The class-level answer decides whether targeted generation is worth building.
Do not start Phase 4 without it.

Add a probe-statistics surface, compiled out or nearly free when disabled:

```rust
// src/expand/probe_stats.rs
pub(crate) struct ProbeStats {
    pub builds: u64,
    pub failed: u64,
    pub forms: u64,
    pub by_class: [ClassStats; 102],
}
```

Collect at the two places that know the answer: `emits_form`'s cache-miss
branch (builds, failures, forms visited) and `inflect_in_unmarked` (the class
and gradation), keyed by `inflection.class` plus a caller tag (`noun stem`,
`verb stem`, `cheap gate`, `head walk`). Expose it through
`perf_grammar_style --probe-stats`; keep it behind an env flag
(`TORILLE_PROBE_STATS=1`) or a feature so the shipping wasm pays nothing.

The report must answer:

| question | metric |
|---|---|
| Which classes dominate form visits? | forms and builds per class, sorted by total |
| Are failures concentrated? | failed builds per class |
| Is the verb path one class or many? | top-3 share of verb forms |
| Is verification repeated? | verifies per `(base, surface)` |
| How many candidates per surface? | candidate bases per stem-map call |
| Where do allocations go? | counting allocator in the native perf tool |

Also count **duplicate candidate verifications**: the same `(map, stem, base,
surface)` reached twice in one resolution. The paradigm cache makes the second
build cheap, but the scan and hash lookup are still visible.

Add a counting global allocator to `perf_grammar_style` for native runs. It is a
tool-only concern; do not put it in the engine.

**Gate:** the report exists and is reproducible on all four corpora. If the top
three verb classes account for less than ~50 % of verb form visits, Phase 4
starts as a general builder filter, not a direct generator.

## 6. Phase 1 — faster internal hasher (low risk)

SipHash is 8–12 % of the instruction profile. The hot maps are all internal
caches whose iteration order is never used for output:

| map | file |
|---|---|
| `Paradigm.words: HashSet<String>` | `src/expand/mod.rs` |
| `KW_MEMO` | `src/suggest/known/mod.rs` |
| `FAST_CACHE` | `src/expand/check_normal.rs` |
| `modifier_is_known` memo | `src/expand/derived_forms/mod.rs` |
| `MorphologyCache.words` | `src/morphology/mod.rs` (measure separately) |

Benchmark three candidates:

1. an in-crate Fx-style hasher (`hash = (hash.rotate_left(5) ^ word) * K`);
2. the same with a per-process random seed, so untrusted text cannot force
   collisions;
3. `ahash` only if the first two are insufficient and the dependency is
   accepted. Do not add a dependency by default.

Confine the change to these aliases. Do not change `suggest`'s public types or
any structure whose iteration order can reach a diagnostic.

**Gate:** ≥3 % instruction reduction on `page-2000`, no reading change on the
homonym corpus, and a documented collision-resistance choice. Abort if the
seeded variant erases the gain.

## 7. Phase 2 — shared probe context (medium)

`resolve_word_fast` exhausts the noun/verb stem maps; when it fails,
`resolve_cheap_fallback` calls `check_normal_fast`, which runs the same stem
loops again. The paradigm cache absorbs the repeat builds, but not the candidate
scan.

Introduce a per-surface context, not a global cache:

```rust
struct StemProbe {
    noun: Option<StemOutcome>, // Exhausted | Found(Vec<String>)
    verb: Option<StemOutcome>,
    possessives: bool,
}
```

Thread it through a new internal `check_normal_with_context(word, ..., probe)`
while keeping `check_normal_fast`/`check_normal` as the public wrappers that
create an empty context. In gates 4 and 4b, skip only when the context says the
same surface, POS, ending list and possessive flag were already searched and
found nothing. A found result would not have reached the fallback; handle only
the exhausted case at first.

Do **not** skip compound checks. `resolve_compound_surface` is a targeted head
verify, while `check_normal_fast`'s gate 4c is the full compound DP; they are
not equivalent.

**Gate:** an outcome-parity test over the clean, typo, unknown and homonym
corpora (`check_normal_fast` with and without the context), plus a count showing
the duplicate stem scans are gone. Abort if the gain is under 2 % and the
complexity is more than the local skip.

## 8. Phase 3 — negative evidence (highest value, independent)

Add a **per-stem ending signature** before `emits_form`:

```text
candidate base
    → might_emit_row (prefix/suffix mask)
    → ending signature test          ← new
    → build the paradigm only if plausible
```

The signature is keyed by the stem the probe already looked up:

```text
stem -> sorted ending ids from the probe's own ending table
```

It is a **necessary condition**, never an exact test: it is the union over every
base at that stem of endings some generated form can produce. A bit clear means
no base at that stem can spell `stem + ending`; a bit set falls through to the
existing verify. It cannot reject a real form if it is generated from the same
forms the verifier uses.

Because 99.5 % of noun stem keys have exactly one base, this per-stem projection
is nearly as strong as the rejected `(stem, ending, base)` table, at a fraction
of the size.

### 8.1 Prototype first, no format change

Build a sidecar `maps/stem_endings.bin.gz` with the same front-coded key region
as the stem map and a variable-length sorted ending-id list per stem. Generate
it from the engine forms and the runtime ending lists (`ENDINGS`,
`VERB_ENDINGS`, `POSSESSIVE_SUFFIXES`), not from a hand-written table. For every
generated form and every runtime ending that strips to a stem in the map, mark
the bit; include the vowel-trimmed candidate the verb probe also tries.

Measure three encodings on the captured probe log:

| encoding | expected size | rejection |
|---|---:|---|
| `u128` exact bitset | ~16 B/stem | exact |
| 64-bit Bloom of ending ids | ~8 B/stem | approximate |
| sorted varint ending ids, front-coded | ~5 B/stem | exact |

The prototype must answer, on the four corpora:

- how many failed verifies it rejects;
- how many successful verifies it would reject (**must be zero**);
- blob size at the best gzip level used by `build.rs`;
- lookup cost per stem.

**Gate:** reject ≥30 % of failed verifies, ≤1.5 MB gzip, zero false rejects
against the catalogue forms and the probe log. Abort below 25 % rejection or
above 1.5 MB.

If the sidecar wins, fold the same ids into the stem-map blob as `MST3` so one
search returns both the bases and the signature. Do not ship both a sidecar and
a second lookup if folding is straightforward.

### 8.2 Validation

- A unit test that the signature accepts every `(stem, ending)` the probe's
  `emits_form` accepts, over the catalogue and the noun/verb fixtures.
- A test that a deliberately wrong bit is never *required*: remove a bit and
  confirm the test fails.
- `emits_form` remains the oracle; the signature is an optimisation only.

Do not start with an exact `(base, surface)` negative cache. It has low reuse in
a fresh pass and adds hashing to the path being made cheaper.

## 9. Phase 4 — targeted membership (the long lever)

After Phase 0, the target is the top verb classes, not every class.

### 9.1 API and contract

```rust
pub fn emits_target_form(
    lemma: &str,
    inflections: &[Inflection],
    cat: &Catalogues,
    lexicon: Option<&LexiconIndex>,
    pos: PartOfSpeech,
    possessives: bool,
    target: &str,
    ending: &str,
) -> bool
```

The target and the ending are both known: the stem-map probe stripped the
ending to reach the stem. The function answers only membership; it must not
populate the full-paradigm cache unless the existing `emits_form` path is the
fallback.

`emits_form` stays as the correctness oracle and the fallback for:

- classless lemmas and compounds;
- an ending outside the class's table;
- a target that the derived layers (`degree_forms`, `numeral_forms`,
  `compound_head_forms`) could produce.

### 9.2 Escalation

1. **Builder filter.** Thread a `Target` through `inflect_in_raw`/the class
   builders so `push` only stores the matching form. This saves the `Vec`,
   `Form` and `HashSet` allocations but still formats every candidate string.
   It is mechanical and safe; measure it first.
2. **Reading filter.** Extend the ending table with a `FormFeatures` mask
   (case/number/mood/tense/person) and skip cells before their strings are
   formatted. This is where the `format!` and `malloc` costs go away. The mask
   is derived from the existing tags, not guessed.
3. **Direct cell generation.** For the top classes only, compute the requested
   cell directly (`decline_target`) and compare. Keep the full walk for
   ambiguous or derived targets.

The class list for step 3 comes from the Phase 0 report. Do not write a
class-specific generator before the report says it pays.

### 9.3 Parity gates

- For every catalogue form of every class member and every fetched Wiktionary
  paradigm, `emits_target_form` agrees with `inflect_all`-based membership.
- The existing `tests/catalogue.rs` gaps/staleness rules are untouched.
- A fallback test: an unsupported class, a classless compound, a degree form and
  a possessive all still take the full walk.
- Diagnostics on `page-2000` are byte-identical.

**Gate:** the top classes' form visits fall by ≥50 %, overall cold morphology is
under **0.80 s** single-thread, and the parity suite is green. If step 1 alone
gives <5 %, skip to step 2; if the class report is flat, stop after Phase 3.

## 10. Phase 5 — allocation and formatting (after builds)

These are deliberately later: they shave a few percent while Phase 3/4 remove
whole builds.

### 10.1 Lowercase without allocating

`analyze_morphology`, `resolve_surfaces` and `analyze_morphology_parallel`
allocate `token.text.to_lowercase()` for every word, although most Finnish
tokens are already lowercase. Use:

```rust
let lower: Cow<'_, str> = if token.text.chars().any(char::is_uppercase) {
    Cow::Owned(token.text.to_lowercase())
} else {
    Cow::Borrowed(&token.text)
};
```

The cache still allocates on the first miss; the hit path stops allocating.
Measure with the counting allocator from Phase 0.

### 10.2 `format!` in the probe path

Audit and replace the hot concatenations with a reusable scratch `String`:

- `compound_head_from_lemma` / `classless_compound_whole`
  (`format!("{left}{base}")` per candidate);
- `hyphenated_lexeme` (`format!("{}-{}", ..)` per character boundary);
- `modifier_is_known` (`{bare}\u{2011}`, `{bare}t`, `{stem}minen`,
  `{stem}nen`, `{bare}nen`, `{bare}{tail}`, `{stem}{adj}`);
- `resolve_compound_surface` (`format!("{prefix}{base}")`).

A single scratch buffer in a per-call/per-worker context is enough. Do not share
it across workers.

### 10.3 Scratch vectors

Only after the above: reuse the candidate/form vectors where the builder owns
them, with a per-worker scratch context. This touches generated nominal code, so
it needs the generator-side change, not a hand edit.

**Gate:** ≥5 % instruction reduction, no reading change.

## 11. Phase 6 — parallel/pool (only if measured)

The unique-surface dispatch is already in place. Measure before changing it:

- unique surfaces per worker and across workers (should be exactly one worker per
  surface);
- wall time per worker (max/mean) on `page-2000`;
- payload bytes and decode/seed time;
- duplicate `WordInfo` cloning.

If a worker is consistently the straggler, partition by **estimated cost**
(unknown/compound-prone surfaces, long words, class-50/51) instead of the first
four characters, or let the coordinator assign dynamic batches. Keep the
proper-name overlay and numeral upgrade on the owning thread; they are
token/capitalisation-dependent and must not move to a worker.

**Gate:** the wasm pool's fresh-state page does not regress, and the native
four-worker path improves or the imbalance metric shows why not. Abort if the
change only helps a synthetic corpus.

## 12. Benchmarking additions

Extend `perf_grammar_style` (or add a companion tool) with:

| metric | why |
|---|---|
| builds per unique surface | candidate explosion |
| failed builds / total | negative-evidence opportunity |
| forms per successful verify | targeted-generation floor |
| forms per failed verify | wasted work |
| candidate bases per surface | stem-map selectivity |
| duplicate verifies per surface | probe-context opportunity |
| allocations per word | lowercase/scratch work |
| forms/time per class | where targeted generation pays |
| worker max/mean time | pool balance |

Keep the four corpora and report the **median of at least 7 fresh-process
runs**. The shared box is noisy; use `taskset` for native A/B runs and
`callgrind` instruction counts as the stable secondary signal. Do not quote a
single run.

## 13. Gates, acceptance and stop conditions

Before every landing:

```bash
cargo test --release --workspace
cargo clippy --workspace --all-targets -- -D warnings
python3 tools/check_docs.py --render
python3 tools/test_binding_agrees_with_binary.py
python3 tools/test_reverse_lookup_recall.py
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/test_illegal_evidence.py
```

Plus:

- `dump_diagnostics` on `page-2000` = 3, and `one_cache_serves_both_layers`
  still passes.
- `syntax_diagnostics` exact cases, `syntax_eval` clean rows, and the
  homonym-corpus lemma set are unchanged.
- No new `GAPS` entry and no change to `AUTHORITATIVE_CLASSES`/`DISPATCH`.

| phase | accept when | stop when |
|---|---|---|
| 1 hasher | ≥3 % Ir, no reading change | seeded variant removes the gain |
| 2 probe context | duplicate scans gone, outcomes identical | <2 % and complex |
| 3 ending signature | ≥30 % failed verifies rejected, ≤1.5 MB gz, 0 false rejects | <25 % or >1.5 MB |
| 4 targeted membership | top classes −50 % visits, cold <0.80 s, parity green | top-3 <50 % of visits or parity fails |
| 5 allocations | ≥5 % Ir, no reading change | only noise |
| 6 pool | wasm pool not worse, native worker improved | synthetic-only gain |

Targets are hypotheses. A phase that misses its gate is a measured negative
result and is recorded in the plan; it is not merged.

## 14. First week

1. Add the probe-statistics module and the four-corpus generator.
2. Capture the class table on all four corpora. Sort by total forms/time, not
   call count.
3. Land the fast hasher experiment in isolation.
4. Prototype the per-stem ending signature from the captured probe log and the
   engine forms; report size and rejection before touching `build.rs`.
5. Implement the shared probe context only for the exhausted-stem case.

Then decide between Phase 3 consolidation (`MST3`) and Phase 4 step 1 based on
the measured rejection rate. The likely order is: hasher → probe context →
signature → targeted verb generation.

## 15. Files this plan touches

| area | files |
|---|---|
| resolver/probe context | `src/morphology/mod.rs`, `src/expand/check_normal.rs` |
| paradigm membership | `src/expand/mod.rs`, `src/expand/inflect.rs`, `src/expand/nominals/`, `src/expand/verbs/` |
| stem maps / signature | `build.rs`, `build/map_builders/front_coded.rs`, `src/expand/static_table.rs`, `src/map_reader.rs`, `tools/src/bin/gen_stem_map_noun.rs`, `tools/src/bin/gen_stem_map_verb.rs` |
| hashers | `src/fast_hash.rs` (new), the four cache modules above |
| compound/format | `src/suggest/known/compound.rs`, `src/suggest/known/stems.rs`, `src/expand/derived_forms/mod.rs` |
| perf tooling | `tools/src/bin/perf_grammar_style.rs`, `tools/gen_perf_corpora.py` (new), `tests/fixtures/perf/homonyms.txt` (new) |
| wasm pool | `torille-wasm/src/lib.rs`, `torille-wasm/demo/src/spell-client.ts`, `extensions/*/src/spell-worker/` |
