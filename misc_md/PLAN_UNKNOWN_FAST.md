# PLAN_UNKNOWN_FAST — a faster misspell first pass

Status: **Phases 0–2 landed** (2026-11-22); Phases 3–6 proposed. Baseline
`6de70e7`; the code changes are on top of `778da8a`.

The first pass that decides “is this token a misspelling?” is
`find_misspells` → `check_normal_fast` per token. It is not the suggestion
ranker and not the slow `check_fast` confirmation; it is the gate that decides
whether a red underline appears at all. On a fresh page it costs **326 ms**
for 1,959 words, and that wait is what this plan removes.

The surprising part of the measurement: **rejecting an unknown word is the
cheap half.** Of the 327 ms spent on the 1,147 unique surfaces of
`page-2000`, only **24 ms** is spent on the 35 surfaces that end up unknown;
**303 ms** is spent proving that known words are known. The expensive words
are compounds and verb forms, not nonsense strings. Optimising only the
“unknown” branch would move ~7 % of the time.

The demo’s slow `check_fast` pool is a background confirmation: it can demote
a candidate, but the underline is already drawn from the first pass, and the
browser extension has no slow pool at all. The first pass is the only gate on
whether a word is flagged.

## 1. What the first pass does today

`find_misspells` walks the tokens, consults the per-textarea `known` cache,
and on a miss calls `check_normal_fast` (`torille-wasm/src/state.rs`). The
cache makes a re-render of the same text nearly free; the cost below is the
**cold first render of a page or a reader chapter**, which is what a user
sees after opening a document.

`check_normal_fast` runs the cheap gates in `src/expand/check_normal.rs`:

1. direct lemma;
2. closed spellings, adverbs, feminine nouns;
3. clitic strip;
4. noun stem map + `emits_form` verification;
5. verb stem map + `emits_target_form` verification;
6. proper-name fallback;
7. productive derivations;
8. compound DP / short compound / fast compound.

Gates 4, 5 and 8 are not constant-time. They find a candidate stem, then
**build the candidate’s paradigm** to prove the surface is really emitted.
That is the cost.

### 1.1 Baseline of record

From `bench/baselines/reference.json`
(`check_suggest.page-2000.corpus.*`, `page-2000`, 1,959 tokens):

| metric | value |
|---|---:|
| first pass, total | **325.6 ms** |
| per token | 0.166 ms |
| p50 | **3 µs** |
| p95 | **506 µs** |
| p99 | **2.51 ms** |
| max | **16.39 ms** |
| unknown outcomes | 92 |

A fresh local run on the same fixture (1,975 tokens, one run) gives 341 ms,
p50 3 µs, p95 530 µs, p99 2.70 ms, max 17.11 ms. The distribution is the
story: half the tokens are essentially free, and a small tail of real words
pays milliseconds.

### 1.2 Where the unique-surface time goes

Measured with a one-call-per-unique-word harness over the same fixture
(1,147 unique lowercase surfaces, no repeats; one run):

| bucket | time | note |
|---|---:|---|
| all unique surfaces | **326.9 ms** | |
| the 35 unique unknown surfaces | **24.1 ms** | 7.4 % |
| known surfaces | **302.8 ms** | 92.6 % |

The slowest surfaces are all known:

| surface | time | why |
|---|---:|---|
| `lempiviikkomme` | 16.1 ms | compound + possessive |
| `perunkirjoittamista` | 15.5 ms | compound `perun` + `kirjoittamista` |
| `ollut` | 13.0 ms | irregular `olla` paradigm |
| `kapperoisiaan` | 9.2 ms | long compound/derived surface |
| `tuomioistuimen` | 6.5 ms | compound head verification |
| `läsnäolevien` | 5.8 ms | participle + compound |
| `arkihuoneessa` | 5.3 ms | compound head verification |

The unknown tail is mostly names (`kenoveeva`, `sikri`, `anni`, `kuntsi`);
they are cheap individually but there are many occurrences. The real lever is
the **known-word verification**, which the rest of this plan removes.

### 1.3 Instruction profile

`callgrind` over the 1,147 unique surfaces (4.05 G instructions total):

| inclusive cost | share | what it is |
|---|---:|---|
| `check_normal_with` | 99.8 % | the whole gate |
| `emits_form` | 75.6 % | paradigm build during stem verification |
| `inflect_in_unmarked` | 58.4 % | the build inside `emits_form` |
| `is_known_word_with_compound` | 39.9 % | compound/modifier probes |
| `collect_bases_via_verb_stem_map` | 34.7 % | verb candidate verification |
| `modifier_is_known` | 34.3 % | prefix probes inside the compound DP |
| `is_known_compound_dp` | 30.5 % | compound segmentation |
| `find_base_via_verb_stem_map` | 28.4 % | first-hit verb gate |
| `inflect_from` | 27.8 % | the engine build |
| `emits_standalone_form` | 27.3 % | compound head/modifier builds |
| `collect_bases_via_stem_map` | 17.4 % | noun candidate verification |

The shares overlap (they are inclusive), but the ranking is unambiguous:
**building paradigms to answer a boolean** is the whole cost. The compound
DP then asks the same question again for every prefix.

### 1.4 Results so far (2026-11-22)

Phases 0–2 are landed and measured on the reference machine:

| step | measurement |
|---|---:|
| state-level `find_misspells`, before Phase 1 | **247.9 ms** |
| after Phase 1 (proper-name short-circuit) | **218.6 ms** |
| after Phase 2 (noun signature accept) | **192.4 ms** |
| first-pass tool, unique surfaces, before | **318 ms** |
| first-pass tool, unique surfaces, after | **271 ms** |
| signature/naming parity | **0 mismatches** over 42,253 unique surfaces |

The state-level probe is `TextAreaState::find_misspells` on `page-2000`
after `update`, release build; it is the user-visible first pass. The
first-pass tool is the deduplicated cold gate and includes its own startup.
The parity corpus is `page-2000`, the 8 %-error and all-unknown corpora,
`homonyms`, `line-wrap`, and the 7,000 and 45,448 books. The all-unknown
corpus did **not** improve (about 799 ms): it has no known nouns to verify,
which is the branch this plan targets.

## 2. Constraints

- **The fast path must stay a subset of the full path.** No new accept may
  make `check_normal_fast` accept a surface that `check_normal` rejects. The
  full path remains the reference for parity tests; this plan changes how
  fast an answer is produced, not which answer is correct.
- **No suffix strippers.** Every accept must come from a form the engine
  actually builds, or from a table derived from those forms. A new
  `is_known_word` suffix list is not an option.
- **The stem maps and signatures are generated artefacts.** They live in
  `maps/`, are fetched/published on the orphan `maps` branch, and are
  regenerated with `gen_stem_endings` after an engine change. A plan that
  changes them must include the regeneration and the parity test.
- **The wasm is the shipping surface.** Every blob is embedded in the wasm
  module; a size increase is a download and memory cost. The wasm is
  single-threaded for the first pass; the four-worker morphology pool is for
  grammar/style, not for this gate.
- **The per-textarea cache is not the target.** Typing already re-checks only
  the changed word. This plan is about the cold first render of a page or a
  reader chapter.

## 3. Phases

### Phase 0 — Measure the first pass, not the slow check

**Landed** (2026-11-22): `perf_check_suggest --first-pass` and `--parity`;
the suite records `check_suggest.<corpus>.first_pass.*`.

Add a reproducible gate-level measurement before changing the gate.

- Extend `tools/src/bin/perf_check_suggest.rs` (or add a sibling
  `perf_check_normal`) with `--first-pass`:
  - per-word wall time for one call per unique surface;
  - known/unknown split;
  - gate shares for gates 1, 2, 3, 4, 4b, 4c0, 4c and the proper-name gate;
  - counts of stem-map candidates, verifies, hits/misses, `emits_form`
    builds, compound DP calls and `modifier_is_known` probes;
  - p50/p95/p99/max and the slowest 40 surfaces.
- Corpora: `tests/fixtures/perf/page-2000.txt`, the three benchmark books,
  an unknown-only list (names + nonsense), a typo list, and a proper-name
  list. Keep the existing corpus manifest hashes.
- Add a `--parity` mode that runs `check_normal_fast` and `check_normal` over
  every token and reports any fast-only accept.
- Record the native baseline in `bench/baselines/` (or the plan appendix)
  and add the same numbers to `tools/bench.py` as `check_suggest.first_pass.*`.

**Gate:** no outcome changes; the new tool reproduces the table in §1.1
within noise; the wasm first pass is measured once with the demo probe.

---

### Phase 1 — Do not pay for a name twice

**Landed** (2026-11-22): the proper-name short-circuit and the
cache-poisoning test.

`find_misspells` currently computes the expensive lexical `known` answer
first, then computes `proper` and ORs them. For a capitalized name in the
proper-name table, the lexical answer is not needed: `case_accepts` already
accepts a capitalized proper name, and `recognised` is true. The lexical gate
is pure duplicate work.

- In `torille-wasm/src/state.rs::find_misspells`, resolve
  `self.proper_nouns` and `proper_names::contains_capitalised(&tok.text)`
  before the `known` cache/gate.
- If the token is capitalized and `proper` is true, skip `check_normal_fast`
  and leave `known = false`; `recognised = known || proper` and
  `case_accepts(word, known, sentence_start, proper)` keep the existing
  case-only behaviour. Do **not** write the skipped result into the `known`
  cache: a lowercase occurrence must still run the lexical gate so a
  common-word reading is not lost.
- Do the same for the user’s custom word forms in the extension path:
  `isUserWord` is currently checked after `analyze_misspells`; a saved word
  should not pay the gate either.
- Add tests: `Anni` (name) accepted; `anni` (lowercase) still case-only;
  `Talo` at a sentence start accepted; `talo` accepted; a name that is also
  a common word keeps its current reading.

**Gate:** `page-2000` first pass −≥5 %; the proper-name corpus produces no
new flags; the extension tests and the case-only fixtures are unchanged.

---

### Phase 2 — Accept a noun stem without building its paradigm

**Landed** (2026-11-22): `check_normal_fast_accept` uses the noun signature
for the boolean misspell gate; the naming path still verifies. The exactness
test runs over every noun stem row in release.

The noun stem signature (`src/expand/stem_endings.rs`) records the endings
that **some base at a stem** can take. The generator derives it from the
engine’s own forms, and for nouns the entry is exact: it records an ending
only when a generated form is literally `stem + ending`.

The current gate throws that fact away: it calls `emits_form` for every
candidate base to name the base. For the boolean question, the build is
redundant.

- Add a fast entry point, e.g. `find_base_via_stem_map_unverified`, or a
  `naming: bool` parameter on the stem-map probe:
  - `naming = false` (the misspell gate): if `NOUN.allows(stem_index, ending)`
    is true, the surface is real; return the first candidate base without
    calling `emits_form`.
  - `naming = true` (morphology, `check_normal`, the CLI): keep the existing
    verification so the returned base is the one that emitted the surface.
- Start with buckets that have exactly one base. That is ~98.9 % of the noun
  stem keys (239,565 / 242,329) and removes the build with no naming
  ambiguity at all.
- Use the fast entry point in `check_normal_fast` gate 4, in
  `is_known_word_with_compound`’s noun-stem branch and in
  `modifier_is_known`, which is where the compound DP spends its time. Keep
  `collect_bases_via_stem_map` (plural) for the callers that need every base.
- Add a generated-data test: build each base’s forms once, attribute every
  form to the stem rows it matches, then for every stem and every ending in
  `ENDINGS`, `NOUN.allows(stem_index, ending)` must agree with whether
  `stem + ending` is in the union of the forms attributed to that stem. This
  is the staleness gate for the blob, not a spot check.

**Gate:** `page-2000` first pass −≥15 % beyond Phase 1; the parity corpus
shows no fast-only accepts; the typo corpus gets no new accepts; all
catalogue and harness tests are green.

---

### Phase 3 — Make the verb signature exact, then stop verifying verbs

The noun trick does not transfer to verbs yet. The verb stem map stores the
**vowel-trimmed** stem (`luken`), while a form such as `lukeneita` is
`lukene` + `ita`. The current signature records `ita` for `luken`, so it
would also allow the non-word `lukenita`; that is why the verifier must run.
The same gap is why `ollut` (`oll` + `ut`) costs 13 ms.

The fix is to record the **full remainder after the stem key**, including the
trimmed vowels:

- `lukeneita` → stem key `luken`, remainder `eita`;
- `ollut` → stem key `oll`, remainder `ut`;
- `lukenee` → stem key `luken`, remainder `ee`.

The generator (`tools/src/bin/gen_stem_endings.rs`) already walks each base’s
forms. Change it to find, for each form, the prefix whose vowel-trimmed form
is one of the base’s stem keys, and record `form[stem.len()..]` from that stem
key, instead of matching the fixed ending list. The runtime keeps the existing
ending loop but checks `allows(stem_index, &word[stem.len()..])` rather than
`allows(stem_index, ending)`. For a single-base bucket this is exact; for a
multi-base bucket the union is still exact for existence (the surface is
`stem + remainder`, which is one of the forms), and only the base **name** can
be ambiguous.

A prototype over the whole lexicon (all 32,119 noun bases and 10,038 verb
bases) gives:

- **noun** — 125,913 stems with a signature, 476 patterns, 4,516 distinct
  remainders, average set 33.3, largest set 515, **0.33 MB raw**.
- **verb** — 349,680 stems with a signature, 501 patterns, 61,601 distinct
  remainders, average set 98.7, largest set 5,050, **2.18 MB raw**.

The raw estimate is 2.5 MB before gzip; the current blobs are 100 KB gzip
total. The gzip result is the decision point: the exact form table was
rejected at 3.15 MB gzip grouped / 8.1 MB flat, so this must stay well under
that. Gate at **≤1.5 MB gzip for both maps** and measure the wasm size.
Measure the runtime probe too: the remainder dictionary grows from 69 entries
to 61,601, and every lookup hashes the remainder. If that offset eats the
win, try a 64-bit remainder hash with the verifier as the collision fallback.

- Generate the full-remainder blobs with `gen_stem_endings`, publish them on
  the `maps` branch, and keep them in `REQUIRED`. The `ENDP` version must
  bump: the remainder dictionary outgrows `u8` ids, while the pattern ids
  stay `u16` (501 patterns).
- Use the exact signature for the boolean path (single-base and multi-base):
  no `emits_target_form` call on a signature hit. Keep verification for
  `naming = true`.
- Add a parity test: build each base’s forms once, attribute them to the stem
  rows they match, then for every stem row and every ending, the signature
  must be true iff the verifier finds the surface. Add a false-accept corpus
  of constructed non-forms (`lukenita`, `ollt`, `talossa` with the wrong
  harmony) and the existing typo list.

**Gate:** `page-2000` first pass −≥25 % beyond Phase 2; max ≤5 ms; the maps
are ≤1.5 MB gzip; no fast-only accept; diagnostics and readings unchanged.

---

### Phase 4 — Stop asking the compound DP the same question twice

After Phases 2–3, remeasure `modifier_is_known` and `is_known_compound_dp`.
If they are still hot:

- Make `modifier_is_known` use the signature-backed fast gate instead of
  falling through to `is_known_word_fast`, which runs the compound detector
  again for a prefix. A modifier only needs the non-compound gates plus a
  bounded recursive split for genuinely compound modifiers; the current
  fallback can re-enter the whole compound machinery.
- Add a per-call `CompoundProbe` cache for prefix decisions. The
  `modifier_is_known` memo is thread-local and helps repeats, but the first
  occurrence of every prefix still pays.
- For a long word, count how often `is_known_compound_fast` or
  `is_known_short_compound` finds a split after the DP has already missed.
  If the counter is near zero, skip the duplicate probes for long words and
  keep the DP as the single segmenter.
- Do **not** replace the DP with a greedy split; the DP’s arbitrary-length
  and boundary guarantees are the reason it exists.

**Gate:** `modifier_is_known` calls into `is_known_word_fast` drop ≥80 %;
compound acceptance is identical on `page-2000` and the classless-compound
report; `page-2000` first pass −≥10 % beyond Phase 3.

---

### Phase 5 — Make the wait invisible (perceived latency)

The engine phases reduce the total. This phase reduces the **time to the
first underline**, which is what a reader notices.

- The first pass currently splits into 2,000-word windows and awaits every
  window before firing `onMisspellCandidate`. Stream the windows: fire the
  candidates for each 250–400-word window as soon as it returns, then
  continue. The total work is unchanged; the first underline lands after
  ~40 ms instead of ~325 ms.
- Keep the byte offsets, the surgical diff and the cancellation check
  working across windows. A candidate from an early window may be demoted by
  a later slow check; that is already the contract.
- In the Gutenberg reader, prefetch the next chapter’s first pass when the
  current render has settled (idle callback). A chapter flip is then warm.
- Report both metrics in the benchmark: total first-pass time and
  time-to-first-underline.

**Gate:** time-to-first-underline ≤50 ms on `page-2000`; total time does not
regress; no duplicate candidates; reader probe and extension tests green.

---

### Phase 6 — Lock the result in

- Add the first-pass metrics to `tools/bench.py` and
  `docs/benchmarking.md`: total, p50/p95/p99/max, known/unknown split,
  proper-name share, time-to-first-underline.
- Add a `check_normal_fast` vs `check_normal` parity corpus over every token
  in the four benchmark books plus a deterministic sample of catalogue
  forms. A fast-only accept fails the suite.
- Add a wasm size budget for the signature blobs and the module.
- Update `README.md`’s performance table and `docs/grammar_syntax.md` only
  after the numbers are measured; do not quote the targets as results.

**Gate:** the full pre-landing suite is green:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
python3 tools/test_binding_agrees_with_binary.py
python3 tools/test_reverse_lookup_recall.py
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/test_illegal_evidence.py
python3 tools/check_docs.py --render
(cd extensions/firefox && npm run typecheck && npm test)
(cd extensions/chromium && npm run typecheck && npm test)
```

plus `dump_diagnostics` on `page-2000` = 3, byte-identical diagnostics, and
the ordinary `tests/catalogue.rs` gaps.

## 4. Targets and stop conditions

| metric | now | target after Phases 1–4 |
|---|---:|---:|
| native first pass, `page-2000` | 325.6 ms | **≤180 ms** (stretch ≤150 ms) |
| p95 | 506 µs | **≤200 µs** |
| p99 | 2.51 ms | **≤1 ms** |
| max | 16.4 ms | **≤5 ms** |
| time to first underline | ~325 ms | **≤50 ms** (Phase 5) |
| wasm blob growth | 100 KB signatures | **≤1.5 MB gzip** |

The targets are hypotheses. A phase that misses its gate is a measured
negative result and is recorded, not merged:

- Phase 1 <5 % → keep the change if it is clean, but do not count it as the
  main lever.
- Phase 2 <10 % or any fast-only accept → revert the accept path; keep the
  measurement and the test.
- Phase 3 >1.5 MB gzip or any parity failure → stop at Phase 2; do not ship a
  larger table.
- Phase 4 <5 % → drop it; the signatures already removed the builds.
- Phase 5 does not change total time; it is a UX metric, not a throughput
  claim.

## 5. Risks

- **Signature drift.** The blobs are generated from the engine. A class or
  derivation change without regenerating them can accept a form the engine no
  longer emits. The parity test over every stem-map row is mandatory, and
  `gen_stem_endings` must stay in the same commit as the engine change.
- **Naming vs acceptance.** The fast path may return a base from a multi-base
  bucket that is not the base that emitted the surface. That is fine for a
  boolean misspell flag, not fine for morphology. The `naming` flag keeps the
  two callers separate.
- **Proper-name case behaviour.** Skipping the lexical gate for a capitalized
  name must not change a lowercase name from case-only to accepted. The rule
  is “skip only when the token is capitalized and `proper` is true”; the
  lowercase path still runs the gate.
- **Compound acceptance.** The DP is the most delicate part of the checker;
  every Phase 4 change needs the classless-compound report and the
  `page-2000` diagnostic parity, not just a timing win.
- **Blob size.** 2.5 MB raw is an estimate from a prototype; the gzip result
  and the wasm module size decide Phase 3. If the blob is too large, ship
  Phase 2 and the compound probe work only.

## 6. What not to do

- Do not ship the reverse index to the wasm. It was measured and rejected on
  size and coverage (86.6 % at 2.1 MB; 95 % would exceed 3 MB).
- Do not build the exact form table. It is 3.15 MB gzip grouped / 8.1 MB
  flat, and it is the thing the full-remainder signature is designed to avoid.
- Do not add suffix lists to `is_known_word`. The signature is derived from
  the engine’s forms; a hand-written suffix is the bypass the repository
  explicitly forbids.
- Do not move the first pass to the four-worker morphology pool in this
  plan. The pool’s state is per worker and the first pass needs the learned
  proper nouns and the per-textarea cache; the engine phases are the cheaper
  route. Revisit only if Phases 1–4 miss the target.
- Do not change `check_normal`’s acceptance to make the fast path match it.
  The full path is the oracle for this work.

## 7. First week

1. Land Phase 0: the first-pass tool, the known/unknown split and the gate
   counters. Record the baseline in the plan or `bench/`.
2. Land Phase 1: the proper-name/user-word short-circuit and its tests.
3. Prototype Phase 2 on the noun stem map and run the exactness test over
   every row. If it is clean, use it in `check_normal_fast` and
   `modifier_is_known`.
4. Generate the Phase 3 full-remainder blobs in a scratch branch, measure the
   gzip size and run the parity test before touching the shipping gate.
5. Re-measure `page-2000` and the wasm first pass; only then decide whether
   Phase 4 and Phase 5 are needed.
