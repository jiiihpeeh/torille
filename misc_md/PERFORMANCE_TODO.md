# PERFORMANCE_TODO — check and suggest

Status: **plan, not implemented** (2026-10-29).

This is the performance plan for the two user-facing operations:

- **check** — is this word real? (`check`, `check_fast`, `check_normal`,
  `check_normal_fast`, `is_known_word*`, wasm `check`, extension `check`)
- **suggest** — what did the writer mean? (`suggest`, `suggest_detailed`,
  `suggest_with_state`, wasm `suggest_words`, extension `suggest`)

It is written against the current tree and the measured baselines below.
Correctness coverage (missing forms, wrong stems) is tracked in `TODO.md`
and `docs/harness.md`; this file only changes **how fast a correct answer is
produced**.

The guiding constraint is Finnish morphology. We should prune with facts
about inflection, not with arbitrary suffix lists: a form is a stem plus a
known case/tense/person ending, plus an optional possessive and clitic, with
consonant gradation and vowel harmony deciding the details. Those facts make
most of the current work unnecessary.

---

## TL;DR

1. **`check` generates full paradigms to answer a boolean.** A direct lemma
   (`talo`) costs ~303 ms; an ordinary inflected form (`talossa`) ~325 ms.
   `find_bases_for_check` walks the head-1 neighbourhood and inflects up to
   3000 lemmas even when the answer is known at the first probe.
2. **The suggestion pool generates a million forms and keeps about a
   hundred.** For `taloo`, `find_bases_with` builds 784 885 forms and the
   ranker keeps 92 candidates within distance 2. The generation, not the edit
   distance, is the cost. (Curated `REPORTED_CORRECTIONS` words such as
   `salamyhkää` return before this work; without that shortcut the same pool
   would build 778 160 forms and keep none.)
3. **The demo's first pass has a 1–100 ms tail per token.** A 2000-token book
   excerpt costs 10.9 s (5.5 ms/token); one real compound token,
   `avioliittoaikeita`, costs 1.25 s. The cost is stem-map verification
   (`inflect_all` per candidate) and the compound split search.
4. The fixes are structural, not micro-optimisations:
   **stop inflecting what cannot match**, **answer recognition without
   building forms**, and **split compounds at morphological boundaries**.

Suggested first week: Phase 0 + P1.1–P1.3 + P2.1–P2.2. That addresses the
bulk of the measured check and suggestion time; the page's long tail still
needs Phase 3.

---

## Progress

### 2026-11-02 — pronoun table precomputed; lemma set from packed rows

The first *unknown* word of a process still cost ~20 ms after the cache work.
It was two tables built by materialising the whole lexicon:

- `pronoun_forms()` scanned all 104k lemma rows to find the 32 class-101
  lemmas, then inflected them (395 surfaces). The table is now a committed blob
  (`maps/pronoun_forms.bin.gz`, 7.5 KB) written by `gen_derived` from
  `expand::check_normal::compute_pronoun_forms`; the runtime reads it into a map
  once. The 104k-row scan is gone.
- `lemma_set()` called `kotus::lemmas()`, which allocates an `Inflection` vector
  for every row (measured: **16.2 ms**, 104,775 rows). The set only needs the
  words, so it is now built from the packed rows
  (`sanalista().iter()` / `EmbeddedLemma::word`), which borrows the blob.

| first miss, new process | before | after |
|---|---:|---:|
| `kotus::lemmas()` materialisation | 16.2 ms | not on this path |
| `the` in the 2000-token corpus | 18–22 ms | 7–14 ms |

`expand::check_normal::tests::the_pronoun_blob_matches_compute` pins the blob to
`compute_pronoun_forms`, and the `Blob` reader moved to `expand/blob.rs` so the
derived and pronoun tables share it. The corpus steady state is unchanged
within measurement noise (the host was under load ~11 during these runs).
The wasm is the beneficiary: it pays the first miss once per page, and it has
no threads to hide it behind.

### 2026-11-02 — memoise the recognition stack

`is_known_word_with_compound` is a pure function of the word and the static
dictionary, and the compound split search asks it about overlapping substrings:
`kaksikymmenvuotiaalla` and `kaksikymmenvuotiaana` share their modifier, and a
page repeats heads across tokens. The result is now memoised per thread (both
the compound and the non-compound reading), bounded at 20 000 words; the memo
is a speed-up, not state, so dropping it when full is safe. The lookup borrows
the word as `&str`, so a hit allocates nothing.

| corpus | before `7fe9538` | after |
|---|---:|---:|
| `45448.txt` 2000 | 396–425 ms | **344 ms** |
| `45448.txt` 2000 p95 | 883 µs | **672 µs** |
| `45448.txt` 4000 | 707 ms | **609 ms** |
| `7000.txt` 2000 | 1 120–1 289 ms | **828 ms** |
| `7000.txt` 2000 p95 | 3 517 µs | **2 087 µs** |
| `7000.txt` 4000 | 2 319 ms | **1 435 ms** |

Unknown counts are again unchanged (293 / 406 / 566 / 964), and the single-word
runs stay at 0.03–0.04 s.

### 2026-11-02 — the compound verifies use the paradigm cache

The cache in `emits_form` only helped callers that called `emits_form`. The
stem-map verifies in `src/suggest/known/stems.rs` called `inflect_all` directly
and listed the whole paradigm on every ask, so the compound split search never
hit the cache: it asks the same `(surface, base)` pair once per pass, per
possessive variant and per recursion. On `valentine` the verb base `valua` was
verified 50 times, each building a 1 258-form paradigm.

The three helpers now call `emits_form`, which walks the same engines and stops
at the first match, with the paradigm cached. The cache key now includes the
inflection list as well: a lemma can carry two classes (`kungfu` is `1,21`) and
a caller may ask about one of them
(`expand::tests::the_paradigm_cache_keys_on_the_inflections`).

| word, `check_normal_fast`, new process | before | after |
|---|---:|---:|
| `valentine` | 0.32 s | **0.03 s** |
| `triestistä` | 0.23 s | **0.04 s** |
| `tapasitteko` | 0.22 s | **0.04 s** |
| `mesaanipurjeineen` | 0.17 s | **0.03 s** |
| `mehuvesi` | 0.17 s | **0.03 s** |

The corpus moves much more than the single-word runs, because the cache is
per-base and a page repeats bases:

| corpus (2000 tokens) | before | after |
|---|---:|---:|
| `45448.txt` | 1,652–1,747 ms | **396–425 ms** |
| `45448.txt` p95 | 1,325 µs | **883 µs** |
| `7000.txt` | 8,712 ms | **1,120–1,289 ms** |

Unknown counts are identical (293 / 406 / 566), so this is the same acceptance
at a fraction of the cost.

What is left on the fast path is now mostly one-off and coverage, not repeated
work:

- `pronoun_forms()` scans all 104k lemma rows on the first miss to find the 32
  class-101 lemmas (~20 ms once per process; the wasm pays it on its first
  unknown word). A build-time class-101 word list would remove the scan; the
  paradigms stay in the engine.
- The remaining per-unknown cost is the compound DP and the *first* verify of
  each base, which is the slot-level reverse match (a per-class ending
  inventory, so a candidate can be rejected before its paradigm is built).
  That is still a codegen project, not a micro-optimisation.

### 2026-11-02 — paradigm cache in `emits_form`

The stem-map gates verify the same base once per token, and a page has many case
forms of one lemma (`talo`, `talossa`, `talolle`, ...). Each verification
rebuilt the whole paradigm. `emits_form` now keeps the verified word set per
(lemma, reading, possessives, lexicon) in a thread-local cache, bounded at
100 000 words and cleared when full; the first call builds the set and the rest
are one probe.

| corpus (2000 tokens) | before | after |
|---|---:|---:|
| `45448.txt` | 1,869 ms | 1,652–1,747 ms |
| `45448.txt` p95 | 1,672 µs | 1,325 µs |
| `7000.txt` | 9,601 ms | 8,712 ms |

The one-off compounds do not move (`valentine` 170 ms, `mesaanipurjeineen`
70 ms): their suffixes are distinct, so nothing is reused. Cutting those needs
the slot-level reverse match — a per-class ending inventory used to reject a
candidate before its paradigm is built — which is the rest of P1.3 and a larger
design than the cache.

### 2026-11-02 — derived tables precomputed

The parallel build cut `derived()` from 85 to 26 ms on the native host, but the
wasm is one thread and still paid the whole build. The tables are now
**committed blobs**: `tools/src/bin/gen_derived.rs` runs
`expand::derived::compute()` and writes

- `maps/derived_by_form.bin.gz` — 38 211 surfaces -> lemma, 1.4 MB,
- `maps/derived_nominal.bin.gz` — 8 389 surfaces, 159 KB,
- `maps/derived_modifiers.bin.gz` — 9 stems, 163 B.

`src/expand/derived.rs` reads them with a sorted-offset blob and a binary
search (`Blob` in that file); there is no build at run time and no per-lookup
map. `base`, `forms`, `is_modifier` and `is_nominal` are unchanged in meaning;
`forms()` is now an iterator over the blob's sorted entries. The bundle grows
by ~1.6 MB.

| | before | after |
|---|---:|---:|
| `derived()` at first unknown | 85.3 ms (26 ms / 12 threads) | **0 ms** |
| `check_normal_fast("the")`, first unknown, new process | 0.12 s | **0.02 s** |

`expand::derived::tests::the_blobs_match_compute` compares every blob entry
against `compute()`, so a `RULES` or sanalista change without regenerating the
blobs fails the suite rather than drifting silently.

### 2026-11-02 — `derived()` built on the cores

The `derived()` tables (surface -> derived lemma, plus the nominal and modifier
sets) are built on the first *unknown* word of a process: `check_normal_fast`
never needs them for a known word, but an unknown token such as `the` pays them.
The sanalista parse is 28 µs; the work is inflecting the 55 flagged rows, which
make 42,532 surfaces.

Those rows are now cut into chunks and built across `worker_count()` (the wasm
shape honours `TORILLE_SINGLE_THREAD`), then merged in row order so the table
contents are identical. Two details: the chunk floor is one row, not 64 — a
floor of 64 made a single chunk and no parallelism — and the merged maps reserve
their final sizes.

| | before | after |
|---|---:|---:|
| `derived()` | 85.3 ms | 25.8 ms (12 threads) |
| `check_normal_fast("the")`, first unknown, new process | 0.12 s | 0.06 s |

The wasm is one thread, so the extension still pays ~85 ms. A precomputed blob
(the `StemMap`/`StaticSet` machinery the other tables use) is the fix there,
not threading.

### 2026-11-02 — P1.4: bound the engine fallback by shape

`find_any_base_with` and `find_bases_with_inner` verified a full paradigm for
every lemma in the head-1 bucket and for every lemma of three characters or
fewer. A lemma that cannot produce the query was still inflected: `taloo` paid
~1 000 short-lemma paradigms, `valentine` every `va` lemma.

Two bounds, both morphology, neither a suffix list:

- **Two characters.** A form is the lemma's stem plus an ending; the stem is a
  prefix of the lemma and gradation changes only its last consonant, so a
  regular form shares the lemma's first two characters. The suppletive `olla`
  forms (`on`, `ovat`) are the exception and reach the checker through the verb
  stem map, not this walk. Applied to the head-1 walk in the recognition paths
  and to the short-lemma scan in every mode (the scan is `≤3`-char lemmas,
  whose forms all share two).
- **Two characters of length.** A form is at most two characters shorter than
  its lemma (the `-da/-dä` infinitive against the stem); sampled across 1 500
  lemmas the largest shrink is 2 (`räksyttää` → `räksytä`). A lemma longer than
  `word.len() + 2` cannot make the word. Applied to the recognition head-1
  walks.

| call | before | after |
|---|---:|---:|
| `check_fast("taloo")` | 1.45 s | **170 ms** |
| `check_fast("koirra")` | 1.57 s | **260 ms** |
| `check_fast("baromeetteri")` | 417 ms | **134 ms** |
| `check_fast("salamyhkää")` | 13.4 s | **2.1 s** |
| `check_fast("silmäränpäyksen")` | 7.7 s | **1.5 s** |
| `check_fast("xyzzy")` | 65 ms | **1.8 ms** |
| `suggest("taloo")` | 1.9 s | **640 ms** |
| `check("taloo")` | 386 ms | **110 ms** |

The suggestion goldens and the workspace tests are unchanged; the unknown
counts are unchanged.

**What this does not move:** `check_normal_fast` — the demo's first pass —
because it calls `check_normal_with(..., engine_gate = false)`, so gate 5 never
runs. Its corpus stays at ~1.97 s (`45448`), and its tail is gate 4/4b/4c plus
one-off initialisation, not the engine walk. The next lever for *that* path is
the slot-level `emits_form` (the rest of P1.3) and eager initialisation, not
P1.4.

### 2026-11-02 — P1.3: `emits_form`

`inflect_all` lists a paradigm: it deduplicates with a
`HashSet<(String, Confidence, Vec<Tags>)>`, cloning every word and reading. The
stem-map gates only ask "is this one form in the paradigm", and paid that clone
for every other form.

`expand::emits_form` walks the same engines, stops at the first match, and keeps
the derived layers (`degree_forms`, `numeral_forms`) so `pahempi` is still a
form of `paha`. A classless lemma has nothing but those layers and the compound
fallbacks, so it takes the full `inflect_all` path. It is used by `lemma_emit`
(`find_bases`) and by gate 4, gate 4b and the possessive-noun verifier
(`check_normal`).

| corpus (2000 tokens) | before | after |
|---|---:|---:|
| `45448.txt` | 2,245 ms | **1,970 ms** |
| `45448.txt` p95 | 2,858 µs | **1,522 µs** |
| `7000.txt` | 10,077 ms | **9,601 ms** |
| unknown tokens | 293 / 566 | 293 / 566 (unchanged) |

`huomasi -> huomata` and `puhtaassa -> puhdas` still resolve; the answers and
the unknown count are unchanged, and
`expand::tests::emits_form_agrees_with_inflect_all` pins the function against
the full list on noun, adjective and verb forms.

Baseline note: the 2026-10-29 DP number for `45448` was 1.69 s. The morphology
commits after it (`5c3290f`, `231d284`, `65f4d4e`, `d5c4587`, `916a89d`,
`2828978`, `3dc6c17`, `20c376d`) added gates for correctness, so today's
pre-P1.3 number was 2.245 s, not 1.69 s. The new baseline is 1.97 s.

The remaining tail is gate 5 (`find_bases_with`), not the stem maps:
`valentine` 165 ms, `tapasitteko` 112 ms, `the` 109 ms, `triestistä` 102 ms on
`45448`. That is P1.4 (bound the engine fallback) and P2.4 (morphology-driven
candidate discovery), which are the next levers.

### 2026-11-02 — the reverse index build (`check`/`lookup` start-up)

`tools/src/bin/perf_index.rs` measures the build that `check` and `lookup` do
before answering the first word: total time, field sizes, and peak RSS (the
latter from `/usr/bin/time -v`). It also honours `TORILLE_SINGLE_THREAD` /
`set_single_thread`, so `--single-thread` reports the wasm-shaped build.

| | before | after |
|---|---:|---:|
| `build` (12 threads) | 29.4 s | **8.6 s** |
| `build` (1 thread) | 40.8 s | **23.3 s** |
| peak RSS | 3.24 GB | **1.86 GB** |
| `check xyzzy` wall time | 32.3 s | **9.8 s** |
| `suggestible` | 446 MB, 16.8M entries | **2 MB bitset** |
| merge phase | 24.3 s | **4.6 s** |

What was actually wrong, in the order it mattered:

1. **`add_base` scanned `more_bases`.** The list holds the second base of every
   homonym word (~233k entries), and every call did a linear `iter().any`. The
   merge makes ~233k such calls while the list grows to 233k: **27 billion
   comparisons**, most of the 24 s merge. A `HashMap<(word, base), index>` makes
   it a probe and keeps the list in insertion order.
2. **`suggestible` was a second `WordSet`.** 16.8M words, 446 MB and one hash
   insert per generated form, and nothing in the tree reads it: `suggest` walks
   `find_bases_with`, not the builder. It is now one bit per position in
   `known` (2 MB), with `is_suggestible(word)` and `suggestible_len()` as the
   API. `tests/catalogue::a_repaired_form_is_not_suggestible` and
   `expand::build::tests::a_repaired_form_is_accepted_but_not_suggestible` pin
   the plain/offered split.
3. **Every insert probed twice, and the merge three times.**
   `WordSet::insert` ran `contains` and then `insert_new` (which probed again),
   and `merge` called `position` after `insert` for the mapping. `insert_at`
   returns `(is_new, position)` and writes at the empty slot the search already
   found, so an insert is one probe. `accept` and `merge` use it.
4. **The merge copied and then freed every share's readings.** The `Vec<Tags>`
   is now *moved* for a word the merged half has not seen, and only copied for
   a word it already has. That removed most of the copy and the 17M small
   frees, and dropped peak RSS by 660 MB on its own.
5. **The merged `known` grew through the doubling chain.** At 17M words that
   rehashes the whole set twice. `reserve_words` sizes the table, `offsets`,
   `analyses` and `first_base` once from the shares' total: merge 7.8 s → 4.6 s.

Still open:

- The merge is 4.6 s of single-threaded `insert_at` over 17M words. A sharded
  merge (partition by hash, merge shards in parallel, keep a sharded lookup)
  is the next structural step.
- `analyses` is 564 MB: one `Vec<Tags>` header per word (24 bytes × 17M) plus
  the 10-byte readings. A flat arena (`Vec<Tags>` + `Vec<u32>` starts) would be
  ~250 MB, but a reading can be added to a word after later words exist, so it
  needs a batch/sort or the same sharding; it is not a drop-in change.
- `first_base` is 130 MB as `Vec<(u32, u16)>` (8 bytes each). Two parallel
  arrays would save 28 MB; not worth the churn alone.

### 2026-10-29 — first PR: recognition fast paths

Done:

- **Phase 0 (partial)** — `tools/src/bin/perf_check_suggest.rs` exists with
  `--words`, `--corpus FILE --tokens N`, `--single-thread`, `--json`,
  `--iters` and `--fast-iters`. `TORILLE_SINGLE_THREAD=1` or
  `torille::expand::set_single_thread(true)` forces the sequential walk, so
  the native probe can measure the extension's shape.
- **P1.1** — `check()` now answers direct lemmas and ordinary inflected forms
  from the cheap gates before the engine. Ambiguous homonyms still fall back.
- **P1.2** — `find_bases_for_check` runs in `recognition_only` mode: it stops
  after `direct`, after the negation/connegative tables, after phase 1, and
  after the head-1 walk as soon as a base is found. `find_bases_with` is
  unchanged.
- **P1.5 (first half)** — `check_normal_fast` no longer runs compound analysis
  when gate 1 (`is_known_word_no_compound`) already knows the word.
- **P3.4 / P5 (first half)** — `TextAreaState` now keeps a bounded lexical
  cache across renders (`known`). `find_misspells` takes `&mut self`; a
  re-render of an unchanged page is one hash probe per token. Proper-noun
  decisions stay generation-scoped and are still checked separately.
- **P3.4 (library half)** — `check_normal_fast` now has a thread-local memo
  (16 384 entries, clear-on-cap). It is a pure function of the word, so this
  also helps the *first* render of a page with repeated tokens and the
  reader/textarea panels that do not share a `TextAreaState`.
- **Gate-1 duplication** — `check_normal_with` used to compute
  `direct_base = is_known_word_no_compound(word)` at the top, which runs the
  whole non-compound gate stack a second time. It is now computed only when
  the cheaper gates have missed. `mehuvesi` dropped from ~60 ms to ~0.08 ms;
  `avioliittoaikeita`'s gate 1 from 109 ms to 0.6 ms; the 2000-token corpus
  from 3.70 s to 3.19 s (45448) and 8.58 s to 7.86 s (7000).
- **P3.1 (partial) — DP compound segmentation.** `check_normal_fast` now
  runs a memoised dynamic program over character positions for long words
  (`segment_compound_dp` in `known.rs`): every transition is a
  `modifier_is_known` prefix, the final component is the non-compound
  detector with a content-head and negation guard, and the answer per
  position is cached. It handles arbitrary-length compounds without a
  component cap. The old split search remains as the fallback for short
  words and cases the DP declines.

  Measured: 2000-token corpus 45448 **3.19 s → 1.69 s**, max token
  **1.05 s → 0.117 s**; 7000 7.86 s → 7.38 s. The DP also rejects the
  doubled-possessive misspelling `todistelukynnsä` while accepting the
  correct `todistelukykynsä` (`todistelu + kyky + -nsä`).

Measured with `perf_check_suggest` on the reference host:

| word | before | after |
|---|---:|---:|
| `check("talo")` | 302 874 µs | **2 µs** |
| `check("talossa")` | 324 601 µs | **5–7 µs** |
| `check("puhtaan")` | 317 619 µs | **459 µs** |
| `check("korrelle")` | 333 912 µs | **146 µs** |
| `check_normal_fast("äesteli")` | compound-probe path | **9 µs**, base `äestellä` |

Cache measurement (4000-token book excerpt, native):

| render | time |
|---|---:|
| first render | 5013 ms |
| unchanged re-render | **9.8 ms** |
| one-word edit | **9.6 ms** |

Thread-local `check_normal_fast` memo, 2000-token corpus first render:

| corpus | before | after |
|---|---:|---:|
| `books/45448.txt` | 4840 ms | **3697 ms** |
| `books/7000.txt` | 10 949 ms | **8583 ms** |
| p50 (45448) | 42 µs | **3 µs** |

Deferred `direct_base` probe, 2000-token corpus:

| corpus | before | after |
|---|---:|---:|
| `books/45448.txt` | 3697 ms | **3190 ms** |
| `books/7000.txt` | 8583 ms | **7864 ms** |
| `mehuvesi` | ~60 ms | **0.08 ms** |

Still open from this PR: the `forms_generated` / `paradigms_inflected` /
`compound_splits` counters and the `lyökämme` compound-head regression.
`check("xyzzy")` is still ~64 ms; that is P1.4's bounded engine fallback, not
P1.1/P1.2.

---

## How the baselines were measured

Host: AMD Ryzen 5 4600H, 12 hardware threads, Linux, `cargo build --release`,
default workspace profile. Timings are warm (lazy statics initialised) and
single-run; treat them as orders of magnitude, not benchmarks.

- Library entry points: repeated calls on the word list below, mean wall
  time per call.
- Page simulation: the first 2000 **word tokens** of two demo books
  (`torille-wasm/demo/books/7000.txt`, `45448.txt`), tokenised with
  `torille::tokenise`, then `check_normal_fast` per token — the same call the
  demo's `find_misspells` makes.
- Candidate counts: `find_bases_with` and count `forms`, forms whose length is
  within ±2, and forms within Levenshtein 2.
- Inflection cost: `inflect_all` over 200 lemmas of each POS, with and
  without possessives.

The harness for these numbers was a scratch example. Phase 0 promotes it to
`tools/src/bin/perf_check_suggest.rs` so the numbers are reproducible with one
command.

> The wasm is **single-threaded**. Host numbers use `std::thread` and
> `available_parallelism`; the wasm's `par_first_some` / `walk_head1` run
> sequentially. Expect the wasm to be slower, and always re-measure in Chrome
> (`probe-timing.mjs`, `probe-toggle.mjs`) before claiming a win.

---

## Baseline numbers

### Entry points (µs per call, host release)

| word | kind | `check_normal_fast` | `is_known_word_fast` | `check_fast` | `check` | `suggest` |
|---|---|---:|---:|---:|---:|---:|
| `talo` | lemma | 0 | 0 | 0 | 302 874 | 337 179 |
| `talossa` | inflected | 140 | 2 | 5 | 324 601 | 357 840 |
| `puhtaan` | inflected | 486 | 477 | 473 | 317 619 | 313 043 |
| `korrelle` | inflected | 319 | 159 | 170 | 333 912 | 309 796 |
| `edistämisvaroista` | compound | 77 349 | 81 469 | 1 485 478 | 1 900 583 | 80 424 |
| `kahvikekkereitä` | compound | 13 709 | 13 682 | 13 627 | 868 624 | 12 997 |
| `henkiruumiiseenne` | possessive compound | 31 598 | 31 901 | 6 276 889 | — | 29 790 |
| `taloo` | typo | 14 | 8 | 1 454 048 | — | 1 891 905 |
| `koirra` | typo | 45 | 31 | 1 569 190 | — | 457 261 |
| `baromeetteri` | typo | 30 600 | 30 216 | 416 553 | — | 356 486 |
| `salamyhkää` | typo | 16 657 | 16 536 | 13 365 744 | — | 0 (curated) |
| `silmäränpäyksen` | transposition | 21 521 | 21 671 | 7 742 323 | — | 1 (curated) |
| `xyzzy` | nonsense | 15 | 8 | 61 545 | 59 991 | 62 226 |

`—` = not measured because the call is the same walk as `check_fast` and ran
into minutes. The `0` / `1` µs suggestion entries are the curated
`REPORTED_CORRECTIONS` early return, not a fast general path.

### Page simulation (2000 word tokens)

| corpus | unknown tokens | total | per token |
|---|---:|---:|---:|
| `books/7000.txt` | 564 | 10 949 ms | 5.48 ms |
| `books/45448.txt` | 292 | 4 789 ms | 2.40 ms |

Distribution on `7000.txt`: 412 tokens under 10 µs, 215 at 10–99 µs, 646 at
0.1–1 ms, 470 at 1–10 ms, **257 at ≥10 ms**. The tail, not the average, is
what users feel on every keystroke.

Worst tokens include real words (`avioliittoaikeita` 1.25 s, `lyökämme`
60 ms, `mehuvesi` 52 ms) as well as unknown names and archaisms.

### Candidate generation in the suggestion pool

These counts are `find_bases_with` directly — the pool `suggest` builds after
its curated/compound early exits. Words with a `REPORTED_CORRECTIONS` entry
(`salamyhkää`, `silmäränpäyksen`) return before this work; the table shows what
the generator would otherwise do.

| word | neighbours | forms generated | length ±2 | Levenshtein ≤2 |
|---|---:|---:|---:|---:|
| `taloo` | 3 162 | 784 885 | 3 242 | 92 |
| `koirra` | 3 254 | 878 455 | 13 107 | 64 |
| `baromeetteri` | 802 | 179 339 | 69 056 | 0 |
| `salamyhkää` | 3 142 | 778 160 | 119 296 | 0 |
| `silmäränpäyksen` | 4 784 | 1 231 063 | 703 655 | 1 |
| `talossa` (already known) | 3 162 | 784 886 | 18 133 | 125 |

For `taloo`, **0.01%** of generated forms are candidates. For the
`baromeetteri` / `salamyhkää` cases, the answer within distance 2 does not
exist at all, yet the engine still builds the full neighbourhood.

### Cost of one lemma's paradigm

| part of speech | possessives | forms per lemma | time per lemma |
|---|---|---:|---:|
| noun | off | 27.5 | 179 µs |
| noun | on | 152.4 | 327 µs |
| verb | either | 1 155 | 1 127 µs |

The head-1 walk covers up to 3000 lemmas. A few thousand lemmas × one verb
paradigm each is the ~1M forms above.

---

## What the numbers say

1. **`find_bases_with` is a suggestion-pool generator, not a recogniser.**
   `check()` uses it to answer a yes/no question. The report it returns
   (`direct`, `bases`, `neighbours`, `forms`) is exactly what the ranker
   needs and exactly what the checker does not.
2. **The head-1 walk runs even when the answer is already known.** For
   `talossa`, phase 1 finds `talo`, then the walk still inflects 3000 lemmas
   to collect neighbours. For `check` that work is pure waste.
3. **A boolean is answered by building a `Vec<Form>`.** Gate 4/4b in
   `check_normal` and the fallback in `check_fast` call `inflect_all` and then
   scan for the surface. For a stem-map hit only the matching slot(s) need
   checking; we should not build the other ~1 154 forms.
4. **Compound detection explores character boundaries, not morphology.**
   Every split calls the full non-compound gates on the right half, then
   recurses on the left. A single real compound can cost 1.25 s.
5. **Most of `suggest`'s work is provably irrelevant.** Any candidate form
   within edit distance `d` has length within `d` of the input. Passing that
   window into generation is a sound, morphology-independent bound; with
   Finnish's finite suffix inventory it becomes much tighter still.
6. **The extension/demo callers already know when a word is known.** They
   should not pay for `suggest` on a correct word, and the worker should
   remember the answer for a token across renders.
7. **Filtering candidate *lemmas* by length is not enough.** A quick
   experiment adding `lemma_len <= input_len + 3` to the head-1 selection
   changed `suggest("taloo")` by under 2%. The cost is the paradigm volume per
   lemma; the filter has to apply to the generated forms and slots, not to the
   lemma list.

---

## Morphological invariants we are allowed to prune with

These are the rules that make the plan safe. Each task must state which
invariant it relies on.

- **Form = stem + suffix.** A Finnish form is a stem with at most one
  inflectional suffix chain: case + number, or tense/mood + person, plus an
  optional possessive suffix and an optional clitic. There is no circumfix and
  no infix (gradation is a stem alternation, not an inserted morpheme).
  Therefore `|form| - |stem|` is bounded by the longest suffix in the class.
- **Gradation is a finite consonant alternation**, not an arbitrary edit:
  `kk↔k`, `pp↔p`, `tt↔t`, `k↔∅`, `p↔v`, `t↔d`, etc. `gradation.rs` already
  has the pairs. Candidate generation should apply these, not all 30
  substitutions.
- **Vowel harmony selects one of two ending sets** (back/front) from the
  stem. The engine already does this; reverse matching should use it to halve
  the ending candidates rather than accept both.
- **Possessive and clitic suffixes are outermost.** If the surface does not
  end near one of them (within the edit budget), no possessive/clitic form can
  be a candidate. This is the cheapest large cut in the suggestion pool.
- **Compound head is rightmost and inflects; the modifier usually does not.**
  The modifier is typically a nominative or genitive singular, or a bound stem
  (`esi-`, `epä-`, `-minen` → `-mis-`). A split at a non-morphological
  boundary is not worth testing.
- **Derived stems are rules**: `-minen` → `-mis-`, `-inen` → `-is-`,
  `-nen` → `-s-`, `-lainen`/`-läinen`, `-llinen`. Use `derived.rs` and
  `derived_forms.rs`; do not add them to a suffix-stripper in
  `is_known_word`.
- **The embedded form set is a subset.** `all_forms.bin` stores bare lemmas and
  genitive singulars (~280k), not every form. "Not in the form set" does not
  mean "not a word"; the stem maps and the paradigm are the fallback.
- **The full nominal form list exists at build time.** `data/all_forms.csv`
  has 4 909 427 rows. Any index proposal must be measured against that size
  and the wasm budget.

---

## Plan

### Phase 0 — measurement harness and gates (1–2 days)

- [x] Promote the scratch probe to `tools/src/bin/perf_check_suggest.rs`.
      Modes:
      - `--words` (the table above)
      - `--corpus torille-wasm/demo/books/*.txt --tokens 2000`
      - `--single-thread` (disable the thread pools the way wasm does)
      - `--json` for the CI gate
      Report p50/p95/max, not just the mean.
- [x] Add `check`, `check_normal_fast`, `check_fast`, `is_known_word_fast`
      cases to `benches/torille.rs`.
- [x] Add `tools/test_performance.py`: run the corpus probe and compare with a
      recorded baseline (`--record`), with optional `--budget` ceilings. The
      baseline is `reports/perf-baseline.json` (machine-local, gitignored).
- [ ] Add counters/tests (behind a test helper, not production API):
      `forms_generated`, `paradigms_inflected`, `compound_splits` per call.
      These make the structural wins visible and stop them regressing.
- [x] Record the baseline JSON in `reports/` (gitignored, regenerated by
      `python3 tools/test_performance.py --record`).

Acceptance: one command reproduces the tables above on the reference machine;
`--single-thread` reports the wasm-relevant order of magnitude.

---

### Phase 1 — recognition: answer yes/no without paradigms (3–5 days)

**P1.1 `check()` fast path** (`src/suggest/mod.rs`)

- [x] Probe the direct lemma (`sanalista().by_word`) first → `Lemma`.
- [x] Probe `is_known_word_no_compound` → `Inflected(base)`.
- [x] Keep the hyphenated-spelling rule before either.
- [x] Only then call the engine (`find_bases_for_check`), and only when the
      cheap gates could not answer (ambiguous homonyms, rare forms).
- [x] Preserve `CheckOutcome` semantics: if the fast path yields one
      unambiguous base, classify as today; if homonyms disagree, fall back.

Expected (and measured, see Progress): `check("talo")` 303 ms → ~2 µs;
`check("talossa")` 325 ms → ~5–7 µs. `check` is what the binding and tools
call; the demo uses `check_fast`/`check_normal_fast`.

**P1.2 `find_bases_for_check` stops at the first answer**
(`src/expand/find_bases.rs`)

- [x] Return immediately when `direct` is non-empty.
- [x] Return when phase 1 produced a base; do not run head-1, prefix-2/3 or
      the short-lemma scan. Those phases exist to *collect neighbours* for the
      ranker, not to recognise. The head-1 walk also stops at its first base.
- [ ] Pass `possessives` only if the surface ends in a possessive suffix
      (morphology, not a guess).
- [x] Add a recognition mode that never allocates `forms`/`neighbours`.

Expected: `check("talossa")` and every ordinary inflected form drop from
~300 ms to sub-millisecond without touching `suggest`.

**P1.3 `emits_form` — reverse verification instead of forward generation**
(`src/expand/mod.rs`, `src/expand/inflect.rs`, gate 4/4b in
`src/expand/check_normal.rs`)

- [x] Add `emits_form(lemma, inflections, cat, pos, word) -> bool`. First cut:
      walks the same engines and stops at the first match, skipping
      `inflect_all`'s dedup clone; the degree and numeral layers stay.
      The slot-level reverse match (ending + stem + gradation) is still open —
      it is the next step after P1.4.
- [x] Make `lemma_emit` in `find_bases.rs` and the stem-map verifiers in
      `check_normal.rs` use it.
- [x] Keep `inflect_all` unchanged for callers that need the paradigm
      (`inflect` tool, tests).

Expected: gate 4/4b verification (`huomasi` 40 ms, `valentine` 150 ms) stops
allocating a `Vec<Generated>` per candidate; 5–50× on the 1–10 ms tail.
This is the most important recognition change after P1.2.

**P1.4 Bound `check_fast`'s engine fallback** (`src/suggest/state.rs`)

- [x] First cut: bound the walks by morphology — a lemma shares the word's
      first two characters, and is at most two characters longer (see the
      Progress entry). `check_fast` 5–40× on the typo corpus.
- [ ] The surface-shape gate: a word that cannot be a Finnish form (no
      case/personal ending, not a bare lemma, not a compound) should not run
      the walk at all.
- [ ] Feed the morphology-derived ending/stem candidates (P2.4) instead of the
      full head/tail/prefix walk.
- [ ] Keep a documented recall escape hatch and measure it against
      `tools/test_reverse_lookup_recall.py` (once its database is clean).

Expected: `check_fast("taloo")` 1.45 s → <20 ms; `check_fast("salamyhkää")`
13.4 s → <100 ms; real rare forms still recognised.

**P1.5 Skip compound analysis when gate 1 already knows the word**
(`src/expand/check_normal.rs`)

- [x] Gate 4c on `direct_base.is_none()`, exactly as the short probe is
      gated today. The `äesteli` regression showed why: a known verb form was
      being renamed to the compound head `teli` once short compounds were
      allowed into the walk. A word the cheap gates already know must not be
      re-analysed as a compound.
- [ ] Add the regression for `lyökämme`: it currently names `käki` through the
      compound fallback instead of `lyödä`; the compound path must verify
      that the head really explains the head surface.

Expected: real but rare/irregular forms no longer pay the compound probe;
correct base names for `äesteli`, `lyökämme`.

---

### Phase 2 — suggestions: generate only what can be a candidate (1–2 weeks)

**P2.1 Pass the edit budget into generation** (`src/suggest/candidates.rs`,
`src/expand/find_bases.rs`, `src/expand/mod.rs`)

- [ ] Add a `find_bases_near(..., max_distance)` path for `suggest` (or extend
      `find_bases_with_form_filter`), leaving `find_bases_with` complete for the tools. The near path applies the length window implied by
      `max_distance`: only surfaces with
      `|len(form) - len(input)| <= max_distance` can survive.
- [ ] Add a filtered generation API (`inflect_all_filtered` / `FormSink`) so
      a rejected form is not turned into a `Form`/`SourcedForm` with cloned
      readings.
- [ ] Apply the window before the dedup `HashSet<(String, Confidence,
      Vec<Tags>)>` clone in `inflect_all`.
- [ ] Re-measure: `taloo` keeps 3 242 of 784 885 forms by length alone.

This is the safest big win: length is a mathematical consequence of edit
distance, not a heuristic.

**P2.2 Gate possessives and clitics by the surface**

- [ ] If the input does not end within `max_distance` of any possessive
      suffix (`nsa nsä nne mme ni si en an än`), do not generate possessive
      forms. Noun paradigms shrink 152 → 27.5 forms/lemma.
- [ ] Same for clitics (`-kin -kaan -kään -ko -kö -han -hän -pa -pä`).
- [ ] Keep the contracted `-en`/illative cases from `known.rs`; they are the
      reason the surface test must be table-based, not a single suffix.
- [ ] When the surface does end in a possessive, strip it once and work on
      the host; do not generate possessive forms for every candidate lemma.

**P2.3 Slot-level skipping** (the engine side of P2.1)

- [ ] In the catalogue-driven path (`expand/inflect.rs`), skip a slot when
      `stem_len + ending_len` cannot land in the window, before building the
      string and tags. Gradation can change length by at most one; widen the
      window by one for gradating stems.
- [ ] Thread the same rule through the generated per-class engines via
      `tools/oracle/extract_class_rules.py` (do **not** hand-edit
      `src/expand/nominals/` or `src/expand/verbs/`).
- [ ] The handwritten verb engines get the same predicate at their slot
      emission points.

Expected: the 1M-form pool collapses to the low tens of thousands; most
queries then spend their time in ranking, not generation.

**P2.4 Reverse candidate generation (the structural fix)**

If P2.1–P2.3 still miss the budget, replace the head-1 candidate pool with a
stem+ending search:

- [ ] Build an ending inventory from the slot table and the stem maps:
      every noun case ending, every verb tense/person ending, possessive and
      clitic, in both harmony variants.
- [ ] For the input, try the endings that its suffix is within `max_distance`
      of; strip and probe `STEM_BUCKETS_NOUN` / `STEM_BUCKETS_VERB` (already
      compile-time maps) to get candidate lemmas.
- [ ] Allow stem typos by probing the stem with the gradation alternates and
      a small edit neighbourhood (distance 1), not the whole alphabet.
- [ ] For each candidate lemma, build only the one or two target forms for
      the matched ending, compare with `levenshtein`, and stop.
- [ ] Fall back to the current head/tail/prefix walk when no ending/stem hit
      exists (very short words, typos in the ending, unknown derivations).
- [ ] Rank with the existing distance/keyboard/lemma-prior rules.

This is the most Finnish-specific change and the one that scales: the size of
the candidate set depends on the number of suffixes and the edit budget, not
on the 104k-lemma lexicon.

**P2.5 Ranking and allocation**

- [ ] Replace the full `sort_by` of all candidates with a bounded top-N
      selection (`select_nth_unstable` or a `BinaryHeap` of `max_n`).
- [ ] Deduplicate by surface before cloning readings; only the surviving
      top-N need `Suggestion` metadata.
- [ ] Reuse buffers for forms/candidates where the API allows it.
- [ ] Keep the deterministic order: distance asc → existing score desc →
      word asc.

**P2.6 Caller-level early exits**

- [ ] In the wasm/extension callers, do not call `suggest` for a word that
      `check_normal_fast` / `check` accepted. Return an empty list.
- [ ] Do not run `suggest` when `max_distance == 0` and the word is not
      already in the form set.
- [ ] Keep the curated `REPORTED_CORRECTIONS` first (it is already O(1)).

Expected for Phase 2: `suggest("taloo")` 1.9 s → <50 ms; `koirra`
0.46 s → <50 ms; `baromeetteri` 0.36 s → <100 ms; no query over 500 ms on the
typo corpus.

---

### Phase 3 — compound analysis (3–5 days)

**P3.0 Dynamic-programming compound segmentation (done, first cut)**

- [x] Memoise over character positions: `solve(pos)` asks whether the
      remainder can be a compound ending in a valid final form.
- [x] Transitions are dictionary modifiers (`modifier_is_known`), not every
      character boundary; longest modifier first.
- [x] Final components use the non-compound detector plus a content-head
      check; the negation auxiliary and bare connegative are rejected on the
      surface.
- [x] Keep the old split search as a fallback for short words and cases the
      DP declines.
- [ ] Use the lexicon prefix index / FST to enumerate transitions instead of
      probing every prefix. The current DP is O(n²) `modifier_is_known`
      probes; they are cheap, but a trie/FST would make transitions
      proportional to dictionary matches.
- [ ] Extend the DP to return the full component list (for the hyphenation
      adapter and the suggestion metadata), not just the head.

**P3.1 Split at morphological boundaries, not every character**
(`src/suggest/known.rs::detect_compound_pass`)

- [ ] Enumerate head candidates from the ending table: strip a case/personal
      ending from the right, probe the stem maps for the head lemma, then
      resolve the modifier with the lexicon prefix index
      (`LexiconIndex::heads_of`), not with `modifier_is_known` on arbitrary
      substrings.
- [ ] Test the bare-nominative head (ending length 0) as well.
- [ ] Verify that the head lemma actually emits the head surface with
      `emits_form` (P1.3); this fixes the `lyökämme` → `käki` mis-analysis
      (the base is `lyödä`).
- [ ] Bound the number of head endings by the surface length and the ending
      inventory; no O(n²) split fan-out.
- [ ] Do not merely reorder the current left/head loop: a left-first variant
      was tried on 2026-10-29 and reverted (see Risks).
- [ ] Keep the same-modifier `-lainen` / hyphen rules (they are spelling, not
      inflection).

**P3.2 Possessive stripping**

- [ ] Choose the longest matching possessive suffix once instead of looping
      over all nine and running a full shallow detect per match.
- [ ] Use the contracted-illative rule already documented in
      `known.rs`; keep `-en` only when the host ends in a case that takes it.

**P3.3 Do not run compound analysis on an already-known word**

- [ ] P1.5 in `check_normal_fast`.
- [ ] In `is_known_word_fast`, the same ordering already holds
      (`is_known_word_no_compound` first); keep it.
- [ ] In `suggest`, the compound short-circuit is already first; keep it.

**P3.4 Cache decisions**

- [x] The per-render `decision` map in `torille-wasm/src/state.rs` already
      deduplicated repeats. The lexical answer is now persistent in
      `TextAreaState` (bounded by `KNOWN_CACHE_LIMIT`, clear-on-cap), and
      proper-noun decisions stay generation-scoped.
- [ ] The extension worker already has bounded check/suggest caches; align
      their sizes with the measured hit rates.

Expected: `edistämisvaroista` 77 ms → <2 ms; `kahvikekkereitä` 14 ms →
<1 ms; the page's ≥10 ms tail disappears. This is what makes typing feel
immediate on a long textarea.

---

### Phase 4 — build-time form membership index (optional, decision-gated)

Only start this if Phase 2 still misses the budgets after P2.3/P2.4.

The idea: stop searching by leveraging `data/all_forms.csv` (4 909 427 nominal
rows, 218 MB source) and its generated compact form.

- [ ] Regenerate `data/all_forms.csv` from the engine (`--source engine`) and
      clear the stale rows noted in `TODO.md`; the index is only as good as
      its input.
- [ ] Decide the surface subset: bare lemmas + genitive singular (current
      280k, 0.5 MB) is too small for suggestions; all **plain** forms
      (no possessive, no clitic) is the useful target.
- [ ] Evaluate a compact membership structure:
      - Bloom filter: ~6 MB at 1% false positives, ~1.2 MB at 5%; false
        positives are fine because survivors are re-verified with
        `emits_form`.
      - MPHF + fingerprint: ~5–8 MB, no false negatives.
      - Front-coded sorted set / FST: exact, but likely 20–40 MB.
- [ ] Size gate: the wasm bundle is 17.7 MB today. Anything over ~3 MB needs
      a feature flag or a host-only path.
- [ ] Query path: enumerate plausible edits of the input (restricted by
      gradation, harmony, keyboard adjacency and doubled letters), probe
      membership, keep hits, verify the top-N with `emits_form`, rank.
- [ ] Do not enumerate the full 30-letter alphabet at distance 2; restrict to
      Finnish letters and typo classes first, then expand only if no hit.

This trades memory for a query cost independent of the lexicon. It is the
long-term answer if the morphology-driven generator is not enough, but it is
not the first move.

---

### Phase 5 — wasm, extensions and caching (2–3 days, parallel)

- [x] Add a persistent token→outcome cache in `TextAreaState` (P3.4), bounded
      and clear-on-cap; proper-noun decisions are not cached globally.
- [ ] Make the demo's render incremental: diff the token stream and only
      check new/changed tokens. The current `find_misspells` rechecks every
      token on every debounced edit.
- [ ] Batch the wasm boundary: one `check_words` / `analyze_misspells` call
      per render, not one call per word.
- [ ] Reuse output buffers (`Vec<Metric>`/`js_sys::Array` construction) on the
      wasm side where the API permits; allocation is a large part of the
      single-thread cost.
- [ ] Keep `warm_caches` and the suggestion pool, but measure them with
      `probe-timing.mjs` after each phase.
- [ ] Check the extension's caches (`spell-core.ts`) against the new timings;
      a 30 ms check is still visible in a popup, a 1 ms one is not.

---

## Budgets and gates

Proposed budgets (host release, 2000 word tokens, warm). Relax ×2 for wasm
until the single-thread probe says otherwise.

| path | p50 | p95 | max |
|---|---:|---:|---:|
| `check_normal_fast` per token (corpus) | 50 µs | 500 µs | 2 ms |
| `is_known_word_fast` / extension `check` | 20 µs | 1 ms | 5 ms |
| `check` (library) | 100 µs | 1 ms | 5 ms |
| `suggest` on the typo corpus | 20 ms | 100 ms | 500 ms |
| 2000-token page, first pass | 100 ms | 250 ms | 500 ms |

Do **not** turn these into brittle CI thresholds before the phases land.
Phase 0 should measure variance; the blocking gate is the absolute budget,
the advisory gate is the regression ratio.

Correctness gates that must stay green:

- `cargo test --release --workspace`
- `tests/catalogue.rs` (every catalogue form of every class member)
- `tests/candidate_generation.rs` golden list (update only with a deliberate
  review, and regenerate the corpus in the same commit — see AGENTS)
- `python3 tools/test_binding_agrees_with_binary.py`
- `python3 tools/test_reverse_lookup_recall.py` (currently blocked by a stale
  `kotus_nomines.sqlite`; fix the database first, then use it as the recall
  guard for P1.4/P2.4)
- `node torille-wasm/demo/probe-timing.mjs`, `probe-toggle.mjs`
- extension e2e (`extensions/chromium/e2e/run.mjs`)

---

## Sequencing and expected impact

| order | task | files | expected win (measured basis) | risk |
|---|---|---|---|---|
| 1 | P1.1 `check` fast path | `src/suggest/mod.rs` | 300 ms → ~2 µs (lemma), ~5–7 µs (inflected) — done | low |
| 2 | P1.2 recognition early exit | `src/expand/find_bases.rs` | ~300 ms → <1 ms for known forms | low |
| 3 | P1.5 skip compound on known | `src/expand/check_normal.rs` | removes 10–100 ms tail; fixes wrong bases | low |
| 4 | P1.3 `emits_form` | `src/expand/*`, `check_normal.rs` | 5–50× on gate 4/4b; page p99 | medium |
| 5 | P2.1 length window | `suggest/candidates.rs`, `find_bases.rs`, `expand/mod.rs` | 785k → 3.2k forms for `taloo` (theoretical 250×) | medium |
| 6 | P2.2 possessive/clitic gate | same + `known.rs` | nouns 152 → 27.5 forms/lemma | low |
| 7 | P3.1 morphological compound split | `suggest/known.rs` | `edistämisvaroista` 77 ms → <2 ms | medium |
| 8 | P2.3 slot-level skip | `expand/inflect.rs`, codegen | the rest of the generation cost | medium-high |
| 9 | P2.4 reverse generator | new `suggest/reverse.rs` | suggests independent of lexicon size | high |
| 10 | P2.5 top-N/allocations | `suggest/candidates.rs` | 1.2–2× ranking | low |
| 11 | P1.4 `check_fast` bound | `suggest/state.rs` | 1.5 s → <20 ms on typos | medium (recall) |
| 12 | P5 caching/incremental | `torille-wasm/state.rs`, demo | 10–100× on repeated renders | low |

First PR suggestion: Phase 0 + P1.1 + P1.2 + P1.5. They are local, low-risk,
and already give a usable `check`.

Second PR: P1.3 + P2.1 + P2.2 + P3.2. These turn the page and suggestion
numbers around without changing the candidate set.

Third PR: P2.3/P2.4/P3.1. Larger, but this is where the structural win lives.

---

## License and source rule

Everything here is the repository's own `MIT OR Apache-2.0` code. Do not copy
Voikko, HFST or any GPL oracle/harness source into the engine; those projects
are references for behaviour, not a source of code. The DP and the trie/FST
ideas are implemented from the existing lexicon indexes and morphology tables
in this tree.

## Risks and rules

- **Do not add suffix strippers to `is_known_word` / `known.rs` as a shortcut.**
  The AGENTS rule stands: a missing form is an inflection-engine bug. This plan
  adds reverse *analysis* of real slot endings, not a list of guessed suffixes.
- **Keep the plain/offered split.** Repaired and guessed forms are accepted,
  never suggested; `tests/catalogue.rs` pins this. Performance work must not
  widen the suggestible set to make a benchmark look good.
- **Do not hand-edit generated per-class files.** Change
  `tools/oracle/extract_class_rules.py` or `grammar/` and regenerate.
- **Length windows need a gradation margin.** A strong/weak pair can differ by
  one character; widen by one at each end.
- **Wasm is single-threaded.** A host win from `walk_head1`'s threads may not
  exist in the extension. Always run the browser probes.
- **Suggestion goldens may move.** Regenerate `tests/candidate_generation.rs`
  when the ranking changes; do not hand-edit it.
- **Memory.** The 1M-form blow-up is also a wasm heap problem (`Vec<Form>`,
  cloned readings). Cutting it reduces peak memory and OOM risk, not just
  latency.
- **A left-first compound split is not enough.** Reordering
  `detect_compound_pass` to check the modifier before the head helped
  `edistämisvaroista` (77 → 26 ms) but regressed other long compounds:
  `mesaanipurjeineen` went from ~50 ms to 3.2 s, and the 2000-token corpus from
  4.8 s to 16.4 s. The deferred nested-compound recursion was the cause. P3.1
  must derive the head from the ending inventory; it cannot be a loop
  reordering.
- **Skipping head-1 is not enough either.** A two-stage `suggest` (cheap
  phases first; head-1 only when no candidate was closer than the budget) was
  tried on 2026-10-29 and reverted: it improved `koirra` (457 → 161 ms) but
  changed the golden candidate list for `koiraa` and left `taloo` at ~1.75 s
  because the prefix-2 walk, not head-1, dominates. P2.4's morphology-driven
  candidate discovery is the fix; the phase ordering alone is not.

---

## Open questions

1. Can `emits_form` reuse the class engines' slot emission without a second
   implementation? The generated tree already knows every stem/ending pair;
   the ideal is one function used by both generation and verification.
2. Is the stem-map route complete enough for suggestion candidates, or does
   the fallback head-1 walk still carry a meaningful recall share? Measure on
   a sample of real typos before removing it.
3. What is the actual wasm budget (size and memory) for a form index? If the
   extension can afford 3 MB, a Bloom filter changes the design; if not,
   Phase 2.4 is the end state.
4. Should `check` expose a `--fast`/`CheckMode` so callers can choose between
   the cheap gates and the full engine walk, rather than us guessing?
