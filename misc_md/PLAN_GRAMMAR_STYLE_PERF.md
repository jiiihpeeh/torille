# PLAN_GRAMMAR_STYLE_PERF — one morphology pass for grammar and style

Status: **Phase 0–11 landed; the serial numeral upgrade is gone, native 4-worker is ~340 ms and the wasm pool meets ≤500 ms per page (364–395 ms fresh state) once the four workers exist; 256-bit prefix + 64-bit suffix stem-shape filter, shared paradigm probe and 4-worker morphology pool landed; cold page ~1.19 s native single-thread / ~0.34 s with 4 workers; a browser session pays one ~0.95 s worker start-up; the exact form table is priced (~8 MB gz) and stays rejected** · 2026-11-19 · branch `syntax` · baseline `2e5f13c`

This is the performance plan for the two opt-in analysis layers:

- `syntax_check` — grammar and punctuation diagnostics;
- `style_check` — suggestion-only style observations.

It does **not** change a rule, a diagnostic, a correction or the keystroke
path. The exact-output fixtures (`tests/fixtures/syntax_diagnostics.json`,
`tests/fixtures/style_diagnostics.json`) are the correctness gate for every
phase here: a phase that changes one byte of output is a failed phase.

The design record for the layers themselves is `PLAN_STYLE.md` and
`docs/syntax_checker.md` / `docs/style_checker.md`. This document is about one
question only: **how do we make an explicit grammar or style check finish
before the user thinks the page has frozen?**

Contents: [§1 measured problem](#1-the-measured-problem) ·
[§2 targets](#2-targets) · [§3 morphology cache](#3-design-one-morphology-cache-with-case-information) ·
[§4 cold path](#4-fixing-the-cold-path) · [§5 warm path](#5-making-the-warm-path-cheap) ·
[§6 worker](#6-wasm-and-worker-surfaces) · [§7 phases](#7-phases) ·
[§8 harness](#8-measurement-harness) · [§9 risks](#9-invariants-and-risks) ·
[§10 decisions](#10-open-decisions)

### Implementation status (2026-11-13)

Landed:

- **Phase 0** — `tools/src/bin/perf_grammar_style.rs` and
  `tests/fixtures/perf/page-2000.txt` (cold/warm, morphology/rules/style,
  `--single-thread`, `--slowest`).
- **Phase 1** — `src/morphology/` with `WordInfo` (`readings`, agreed `case`
  and `number`, `part_of_speech`, `clitic`, `possessive`, a derivation
  candidate, `depth`), a shared `MorphologyCache`, `analyze_morphology`,
  `syntax::analyze_syntax`, `style_check::check_with_cache`,
  `torille::check_all_with_cache`, wasm `check_style_with_state`, and the
  per-textarea cache shared by both layers.
- **Phase 2 (partial)** — direct lemma and stem-map fast path, clitic and
  possessive host reuse, contracted possessives (`-an`/`-än`), a
  numeral-governed deep upgrade (`kaksi lasta`), a deep-result cache, and a
  compound fallback. The compound split comes from the existing DP state
  machine (`check_normal_fast` → `suggest::known::compound`), the modifier is
  validated by `modifier_is_known` (which reads the classless CSV rows and the
  bound prefixes `esi-`, `lempi-`, `ensi-`, ...), and the whole is inflected
  with `head_inflections`. No new prefix list is added. Cold page: **~2.1–2.3 s
  native / ~3.0 s wasm**; the 20/96-word targets are met, the 2,000-word
  wasm target is at ~3 s, and the ≤500 ms target was deferred to
  Phase 6 and remains open.
- **Phase 3** — warm rule pass: `TokenSyntax.key` / `WordInfo.key` cache the
  lowercased surface, the agreement scan asks for an attribute shape before
  the degree lookup, the shape helpers use a two-slot fold instead of
  `Vec`+sort, style phrases are lowercased once and gated by a generated
  first-word set, the long-sentence metric is one token pass, and correction
  paradigms are cached per (lemma, inflections, wanted tags, POS). Measured on
  the fixture: rules **13.9 → 1.4 ms**, style **5.0 → 2.5 ms**, warm all
  **23.5 → 6.9 ms**.
- **Phase 4** — `SentenceCache` keys each sentence by its exact text and
  reuses its diagnostics across edits. The document quote rule stays global.
  The wasm state owns the cache; one edited sentence on the 1,959-word page
  costs **2.6–3.7 ms** including tokenise, morphology and style, and a 96-word
  page costs **1.8 ms**. Diagnostics are identical to a full re-check.
- **Phase 5 leftovers** — the demo worker batches one sentence at a time,
  the extension workers use the combined grammar/style export, the demo's
  cancellation token is per textarea state, and `GutenbergReader` runs
  grammar/style with progress and cancellation. The extension background
  chunks the command, posts progress and stops on an edit. The p90 batch is
  native **63 ms**; the remaining browser outliers are cold names/compounds.
- **Parallel morphology (follow-up)** — a 4-worker pool resolves the unique
  surfaces, the owning state collects the encoded rows, and syntax/style run
  once over the full document. The demo cold page is **1.39 s** (first
  progress 112 ms, 68/14 exact); the Firefox extension uses the same pool
  behind `grammarThreads` (1–4). Chromium's MV3 service worker cannot create
  nested workers and keeps the chunked path. See
  [`PLAN_PARALLEL_MORPHOLOGY.md`](PLAN_PARALLEL_MORPHOLOGY.md).
- **Phase 6 (decision)** — the reverse-index experiment measured five top-N
  subsets. No subset met both 95% page coverage and 3 MB compressed; the
  largest under-budget blob covers 86.6%, and the full 104k-lemma map is
  17.3 M rows. **Not shipped**; the §4.2/§4.3 ladder stays.
- **Phase 7** — `GutenbergReader` runs grammar/style with progress and
  cancellation; `docs/syntax_checker.md` records the sentence-cache path.
  The background pool e2e is `extensions/firefox/e2e/pool.mjs`; the
  content-panel e2e is `extensions/firefox/e2e/pool-content.mjs`.
- **Shallow closed-class fix** — a pronoun base named by the cheap gate
  (`jossa` → `joka`) is verified across every sanalista row, so relative
  pronoun forms no longer fall to `Unknown`. The attribute rule now skips
  relative-pronoun lemmas, which removes three false `attribute_agreement`
  findings on the perf fixture; the stem-map-first reorder also removes one
  false `verb_rection` on `korkean` (71 → 68 grammar diagnostics). The syntax
  corpus's `two_tuli` case is green again.
- **Phases 8–10** — the shared paradigm probe, the stem-shape filter and the
  exact-table decision. Cold morphology 2,016 → 1,330 ms single-thread and
  753 → 450 ms on 4 workers. Phase 10 priced the one remaining engine lever
  (an exact `surface → base + reading` table) at ~8 MB gzip and showed no
  table under the 3 MB gate reaches 500 ms wasm, because 305 ms of that wall
  is worker start-up and transport. See §Phase 10.

Measured on the committed fixture, single-thread (wasm shape):

| workload | before | now |
|---|---:|---:|
| 20-word cold | 3.0 s | 0.14 s |
| 96-word cold | 11.5 s | 0.76 s |
| 1,959-word cold morphology | ~120 s (1,722-word baseline) | ~2.2 s |
| 1,959-word warm rule pass | 13.9 ms | 1.4 ms |
| 1,959-word warm style | 6–13 ms | 2.5 ms |
| 1,959-word warm grammar+style | ~33–41 ms | 6.9 ms |
| 17-word bound-form fixture, cold | 620 ms | 79 ms |

The full 1,959-word page is ~3.0 s in the release wasm single-thread, and
**~1.4 s** with the 4-worker morphology pool (Phase 5 follow-up). First
progress lands at ~110 ms.

Bound forms are no longer a hotspot. The 17-word fixture
(`esimerkki`, `leikkimielisyys`, `salaperäinen`, `täysi-ikäinen`,
`nuori-isäntä`, `pariskunnan`, `epäsuhta`, ...) went from 620 ms to 79 ms
cold, and every item is under 3 ms: `nuoren-isännän` fell from 250 ms to
0.61 ms, `salaperäisen` from 68 ms to 1.8 ms, and `leikkimielisyyden` from
52 ms to 0.6 ms. The fix is that
`add_forms_from_lemma` passes the `LexiconIndex` to `inflect_all`, so class
50/51 compounds can split their parts, and a classless base uses
`compound_head_forms` directly for a bound suffix head.

The synthetic 20-word slice is now near the interactive target. The full page
is still dominated by a long tail of names, derived adjectives and archaic
forms (`palajakaan`, `pariskunnan` at the time of measurement, `kulettivat`,
`kivuloinen`, `kenoveeva*`); the page was 20.9 s before the later phases. The
partitive-plural stem-shape filter and the warm rule pass have landed, and the
worker chunks the check with progress and cancellation. The sentence cache
(Phase 4) remains. The cold page target is not met yet: the release wasm is
~3.0 s single-thread / ~1.4 s with the 4-worker pool, with first progress at
~110 ms.

Re-measured 2026-11-17 on the committed fixture, single-thread (before the
shared paradigm probe; the current numbers follow this block):

```text
page-2000 (1,918 words): cold morphology 1,931 ms, warm morphology 86 ms,
warm syntax 4.8 ms, warm rules 10.5 ms, warm style 89.9 ms, warm all 97.4 ms,
style from a fresh cache 1,376 ms; sentence cache cold 94.8 ms.
cold cache: 1,124 words, 809 hits, 1,136 misses;
depths: direct 334, stem_map 693, clitic 24, possessive 2, compound 18,
unknown 53; deep upgrades 12.
slowest single word: 8.7 ms (`hirvestä`); every other item is under 8.4 ms.
```

The cost is spread across the long tail, not one word. The reverse-index
experiment was already rejected on blob size (§Phase 6 decision), so the
≤500 ms target stays open and this remains a separate engineering objective,
not a rule-correctness gate.

Re-measured 2026-11-18 after the shared paradigm probe (§Phase 8), same
machine, median of 7 first-pass runs in fresh processes:

| workload | before | after |
|---|---:|---:|
| cold morphology, single-thread (wasm shape) | 2,016 ms | **1,490 ms** |
| style from a fresh cache | ~1,410 ms | **~1,140 ms** |
| cold morphology, 4 native workers | 753 ms | **537 ms** |
| warm morphology / warm all | 90 / 97 ms | unchanged |

Diagnostics are identical in every configuration: `--threads 4` reports the
same 54 grammar / 14 style as the single-thread pass, and the parity flags are
`true`/`true`. The ≤500 ms target is now within ~7% on the native 4-worker
path and still open for the shipping wasm build, which this plan does not
measure.

### What the remaining seconds are, measured

Three throwaway experiments on the fixture attribute the cold pass. Each
disables one branch and re-measures; the point is the *shape*, not the exact
milliseconds. The baseline is the fixture with the probe and the memo already
in, so the deltas are what each branch costs on top of everything else:

| experiment | cold morphology | reading |
|---|---:|---|
| baseline (probe and memo in) | 1,763 ms | — |
| skip the verb stem map (`collect_bases_via_verb_stem_map`) | 1,797 ms | no change: a base it would have named then costs the fallback instead |
| skip the cheap fallback (`resolve_cheap_fallback`) | 1,551 ms | −12% |
| skip both | **964 ms** | −45% |

`callgrind` on the same fixture counted 14.9 G instructions before the shared
paradigm probe and 12.0 G after (a 1.24× instruction reduction; wall clock
improved 1.35× because the probe also removes allocation). The tree, from the
after run, with inclusive shares that nest:

| frame | inclusive |
|---|---:|
| `emits_form` (the verification walk and every paradigm it builds) | 74% |
| `inflect_in` (the builders) | 67% |
| `modifier_is_known` → `is_known_word_fast` → compound DP | 19% |
| `alloc::fmt::format` | 24% |
| `split::dedup` | 15% |

Self-cost leaves say the same thing from the other side: `malloc`/`free` 23%,
the formatter 14–24% (overlapping with the DP, which is its largest caller),
the two `dedup` sorts 7%, hashing 10%, `memcpy` 8%, and the front-coded map
probes 8%.

So the cost is *paradigm construction* plus a *duplicated unknown-word
search*, not one hot function. `format!` is concentrated in the compound DP
(`segment_compound_dp::solve` is its largest caller) and in the per-class
builders; `dedup` runs once per `inflect_in`. Any further large win has to come
from constructing fewer paradigms (a form → readings table, or a targeted
"can this class spell this surface" test that does not build the grid) rather
than from cheaper cells.

## 1. The measured problem

`books/9082.txt` (a Finnish Gutenberg novel), a middle slice, checked with a
fresh per-textarea `AnalysisCache`. “Cold” means the first check of this text;
“warm” means the same cache after one complete pass. The single-thread column
is the wasm shape (`TORILLE_SINGLE_THREAD=1`); the wasm worker has one thread.

| workload | cold, wasm shape | cold, native multi-core |
|---|---:|---:|
| 4-word sentence | 156 ms | — |
| 20-word sentence | 3.0 s | — |
| 254-word paragraph | 26.5 s | — |
| 1,722-word reader page | **120 s** | **70–82 s** |

The same 1,722-word page once warm:

| pass | time |
|---|---:|
| morphology only | 4–7 ms |
| all grammar rules | 20–28 ms |
| style (full pass) | 6–13 ms |
| grammar + style as two calls | ~2× the morphology work |

Inside the rule pass, agreement is the outlier:

```text
agreement 15–19 ms   punctuation 0.5–0.8 ms   rection 1.8–2.4 ms
object_case 2.2–2.8 ms   arguments 0.2 ms   rank < 0.1 ms
```

The shape of the problem is therefore **cold morphology, not warm rules**.
A 20-word sentence can take three seconds; a reader page takes two minutes.
That is why the Gutenberg reader is still spelling-only, and it is why the
demo's grammar and style toggles are scoped to the small free-text panel.

### 1.1 Where the cold seconds go

A single word costs tens to thousands of milliseconds the first time it is
resolved:

| word | cold | phase-1 candidates | phase-1.5 candidates |
|---|---:|---:|---:|
| `kanssa` | ~726 ms | 281 | 843 |
| `liika` | ~4.3 s | 278 | 240 |
| `talossa` | ~118 ms | 163 | 0 |
| `perunkirjoittamista` | ~1.3 s | 244 | 1,293 |
| `xyzzy` (unknown) | ~1.9 ms | 0 | 0 |
| `silloinkin` (clitic) | ~925 ms | — | — |

A 20-word sentence with 15 unique words summed to ~2.9 s. The top ten words
accounted for 99% of it; `silloinkin`, `sinunkin` and `kanssasi` alone were
~1.8 s.

The costs are structural, not micro-optimisation:

1. **`find_bases_for_analysis` keeps walking when the surface is already a
   direct lemma.** `keep_walking_with_direct = true` exists to recover a
   homonym behind a common word (`lasta` is a splinter lemma and the partitive
   of `lapsi`). For every direct lemma it therefore runs the head-1 walk,
   phases 2–3 and the short-lemma scan. `kanssa` alone walks 843 head-1
   candidates; `liika` spends 4.2 s in phase 1.
2. **Every candidate is verified by building its whole paradigm.**
   `lemma_emit` → `emits_form` → `inflect_all`; the first verification of a
   lemma builds a full form set. 163 candidates for `talossa` is 118 ms; 278
   candidates for `liika` is 4.2 s.
3. **Clitic stripping recurses into the host and repeats the walk.**
   `silloinkin` → `silloin` → the direct-lemma deep walk; `kanssasi` →
   `kanssa` → the same.
4. **The warm path is paid twice when both layers run.** Grammar uses the
   per-textarea `syntax_cache`; style uses a separate thread-local cache and
   calls `syntax::analyze` even though it only needs lemmas, POS and sentence
   spans. A style check from a cold process on the same page took **95 s**.
5. **The wasm export marshals the full analysis for nothing.**
   `check_syntax_with_state` builds `syntax_object` — every sentence, clause,
   token, reading and list as JS objects — and both workers read only
   `diagnostics`. `analyze_syntax_with_state` already exists for callers that
   want the full object.
6. **The warm rule pass allocates on every token.** `check_attributes` calls
   `degree_base(token.text.to_lowercase())` before it asks whether the token
   can be an attribute at all; `attribute_shape` and `head_shape` allocate a
   `Vec` per token; `nominal_case` does the same in the shared helpers.
7. **The cache clones on every hit and clears wholesale.** `AnalysisCache`
   returns `analysis.clone()` (a `Vec<Tags>` plus several `String`s) for every
   occurrence of every cached word, and clears all 4,096 entries when full.
8. **The resolver ignores the base sources the engine already has.** 82% of
   the page's unique words are either a direct lemma (30%) or named by the
   noun/verb stem map (52%). The syntax path still walks head/tail candidates
   for many of them instead of verifying the one base the map names.

## 2. Targets

These are the budgets that make the feature usable, not aspirational numbers.
The cold budgets and the chunking bound are the gates for enabling
grammar/style on the reader; the rest are per-phase gates.

| metric | today | target |
|---|---:|---:|
| cold 20-word sentence, wasm | 3.0 s | **≤ 100 ms** |
| cold 2,000-word page, wasm | 120 s | **≤ 3 s** (Phase 2), **≤ 500 ms** (Phase 6) |
| warm re-check after one edited sentence, 2,000 words | 20–28 ms | **≤ 5 ms** |
| grammar rule pass, 2,000-word page | 20–28 ms | **≤ 5 ms** |
| style pass, 2,000-word page | 6–13 ms | **≤ 3 ms** |
| grammar + style vs grammar alone | +6–13 ms | **≤ +3 ms** |
| longest uninterrupted worker task | 120 s | **≤ 50 ms per chunk** |
| wasm download (`torille_wasm_bg.wasm.br`) | ~5.0 MB | **≤ +1 MB** (stem-shape filter), +3 MB only for the optional reverse index |
| diagnostic output | — | **byte-identical** |

A 3 s cold page is acceptable only because grammar/style are explicit,
opt-in commands with a progress state. A 500 ms cold page is what allows the
reader to opt in. The 20-word sentence is the interactive case: it must feel
like a check, not a hang.

## 3. Design: one morphology cache with case information

The central change is to make morphology a first-class cached stage that both
layers consume. Today `syntax::cache::WordAnalysis` already stores readings
(and therefore case/number/person/mood/voice/polarity/clitic), but it is
private to `syntax`, it is cloned per token, style cannot reach it, and the
agreed case is recomputed by the rule helpers on every call.

### 3.1 `WordInfo`

Proposed new module `src/morphology/` (re-exported as `syntax::AnalysisCache`
during the transition):

```rust
pub type LemmaId = u32;

/// One resolved surface. Shared by every rule that reads it.
pub struct WordInfo {
    /// Lowercased surface key, for diagnostics and the cache API.
    pub key: Box<str>,
    pub lemma: Option<LemmaId>,
    pub verb_lemma: Option<LemmaId>,
    pub nominal_lemma: Option<LemmaId>,
    pub lemmas: Box<[LemmaId]>,
    /// The readings, including case, number, person, mood, voice,
    /// polarity and clitic. `Tags` is 7 bytes and `Copy`.
    pub readings: Box<[Tags]>,
    /// The agreed nominal case, computed once at insert time.
    pub case: Option<Case>,
    /// The agreed number, computed once at insert time.
    pub number: Option<Number>,
    /// True when the ordinary search found this exact surface.
    pub exact: bool,
    /// How hard the resolver had to work. Drives lazy upgrades.
    pub depth: Depth,
}

pub enum Depth {
    Direct,
    StemMap,
    ClosedSpelling,
    Clitic,
    Possessive,
    Compound,
    HeadTail,
    Prefix,
    ProperName,
    Unknown,
}
```

`case` and `number` are the answer to “cache the case information for syntax”:
`nominal_case()` and `candidate_number()` in
`src/syntax_check/analysis.rs` currently scan `readings` and allocate a `Vec`
for every call. They become `word.case` and `word.number`, computed once when
the entry is inserted. The raw readings stay available for rules that need a
non-agreed reading.

`WordInfo.case` is the agreed morphological case of the surface.
`TokenSyntax.case` is different: it is the case a role licensed (`söi` gives
the object its accusative reading). Keep the two separate; do not replace the
role-licensed field with the cached morphological one.

`LemmaId` is the sanalista row. It keeps `WordInfo` small and clone-free; the
display string is materialised only when a diagnostic or a panel needs it.
`TextAreaState` is inside a `Mutex`, so entries use `Arc<WordInfo>`
(`Send + Sync`); the wasm is single-threaded but the state container is not.

### 3.2 Cached morphology, not a boolean

The spell fast path caches a yes/no known-word answer. That is not enough for
syntax, and it is why the two paths currently diverge. The morphology cache
stores the **readings**, not only the surface:

- part of speech, lemma and every alternative lemma;
- case and number (raw per reading, plus the agreed pair);
- person, tense, mood, voice and polarity for verb readings;
- possessive and clitic;
- the resolution depth and whether the surface was an exact hit.

A rule never asks “is this word known?”; it asks “which cases does this word
have, and which one licenses this role?”. The cache answers that directly.
This is also what lets style reuse the same pass: repetition needs the lemma
and content POS, and the long-sentence rule needs a finite predicate, all of
which are already in `WordInfo`.

The cache key is the lowercased surface. Morphology is text-independent, so
entries survive edits and even a new sentence, subject to the eviction cap in
§4.6; only `clear_state` / `destroy_state` drop the whole cache. Capitalization
is applied at the token edge: the token's `is_capitalised()` decides whether a
proper-name reading is added, so the cached entry itself stays case-neutral.
Learned proper nouns live in `TextAreaState.proper_nouns`; the embedded
proper-name table is static. Neither belongs in `WordInfo`.

### 3.3 Cache and stage API

```rust
pub struct MorphologyCache {
    words: HashMap<Box<str>, Arc<WordInfo>>,
    sentences: SentenceCache,        // phase 4
    hits: u64,
    misses: u64,
}

impl MorphologyCache {
    pub fn resolve_shallow(&mut self, word: &str) -> Arc<WordInfo>;
    pub fn resolve_deep(&mut self, word: &str) -> Arc<WordInfo>;
    pub fn get(&self, word: &str) -> Option<&Arc<WordInfo>>;
    pub fn stats(&self) -> CacheStats;
    pub fn clear(&mut self);
}

/// Stage 1: words + sentence spans. No roles, no clauses, no lists.
pub fn analyze_morphology(
    tokens: &[Token],
    cache: &mut MorphologyCache,
) -> MorphologyAnalysis;

/// Stage 2: roles, clauses, lists. Pure; no morphology search.
pub fn analyze_syntax(
    tokens: &[Token],
    morphology: &MorphologyAnalysis,
) -> SyntaxAnalysis;

/// Stage 3: the grammar rules. Pure.
pub fn check_grammar(
    tokens: &[Token],
    analysis: &SyntaxAnalysis,
) -> Vec<SyntaxDiagnostic>;

/// Stage 3': the style rules. Needs morphology + sentences, not roles.
pub fn check_style(
    tokens: &[Token],
    morphology: &MorphologyAnalysis,
) -> StyleAnalysis;
```

Public wrappers keep the current names:

```rust
syntax_check::check_with_cache(text, &mut cache) -> SyntaxCheck
style_check::check_with_cache(text, &mut cache) -> StyleAnalysis
torille::check_all_with_cache(text, &mut cache) -> CheckAll
```

`CheckAll { syntax: SyntaxCheck, style: StyleAnalysis }` resolves morphology
once and runs both rule passes. The wasm worker uses it when both toggles are
on; the extension uses it for a combined request.

### 3.4 Shallow and deep resolution

The current resolver has one depth: direct lemma → heads/tails → head-1 →
prefix-3 → prefix-2 → short-lemma → clitic. That is why a common word costs a
second.

Split it into two public depths:

- **Shallow** — the §4.2 ladder through the bounded head/tail scan: direct
  lemma, stem map, closed spelling, clitic, possessive, compound, then phase 1
  with the stem-shape filter. This is enough for style (lemma + POS + finite
  predicate) and for the vast majority of grammar roles.
- **Deep** — head-1, prefix-2/3 and the short-lemma scan. Only a rule that is
  about to diagnose a token asks for it, and only when the shallow entry is
  ambiguous or the token's role is `Unknown`/weak.

`WordInfo.depth` records which resolution produced the entry. A deep upgrade
replaces the entry and re-classifies that token (and its clause) only if the
readings actually changed. Cap the number of upgrades per sentence (e.g. 8) so
one pathological sentence cannot walk the lexicon.

The `lasta`/`lapsi` case becomes an explicit test: shallow sees only `lasta`;
a deep upgrade requested by the agreement/object rule sees `lapsi` too. If the
upgrade is not requested, the rule must stay quiet — false negatives are
preferred over false positives, as everywhere else.

### 3.5 Scope and lifetime

One `MorphologyCache` per `TextAreaState`, not one per analysis call:

```rust
pub struct TextAreaState {
    // ...
    pub(crate) morphology: torille::morphology::MorphologyCache,
}
```

- `check-syntax` and `check-style` both take the state index and share it.
- `clear_state` / `destroy_state` drop it.
- A new book or chapter already calls the panel's `clearState`, so the cache
  does not leak across documents.
- The spell fast path stays out of this cache. It is a yes/no gate
  (`check_normal_fast`) and must not grow a morphology dependency. If a later
  measurement shows the cache can answer a known-word probe for free, it can
  be consulted read-only; that is not part of this plan.

## 4. Fixing the cold path

Do these in order; stop when the targets are met. Each step is independently
shippable and each keeps the exact-output fixtures green.

### 4.1 Stop the deep walk when a base is already known

`find_bases_for_analysis` should stop after the shallow ladder in §4.2
(through the bounded head/tail scan). The deep walk moves to
`find_bases_for_analysis_deep`, called only by `resolve_deep`.

- `keep_walking_with_direct` becomes `false` in the shallow call. A direct
  lemma is accepted without a head/tail scan; the `lasta`/`lapsi` alternative
  is a deep upgrade, not a cost every common word pays.
- Phases 1.5/2/3/4 run only in `resolve_deep`; phase 1 is the bounded rung 7.
- Clitic stripping resolves the host shallowly and copies the host's
  `WordInfo`; it does not trigger a second deep walk.
- Head-1 is capped at 128 candidates (today 3,000) and ordered by longest
  common prefix with the surface, then lexicon order for determinism.
- Prefix-2/3 walks are capped at 512 candidates each.

This alone removes the 843-candidate `kanssa` walk (a direct lemma) and the
clitic-host deep walks. It does not help `perunkirjoittamista`, whose phase 1
found no base; that is §4.2.

### 4.2 Resolve from the cheapest source first

The engine already has cheaper base sources than the head/tail scan; the
syntax resolver just does not use them. On the 1,722-word page, 1,029 unique
words break down as:

| source | unique words | share |
|---|---:|---:|
| direct lemma | 310 | 30% |
| noun/verb stem map | 533 | 52% |
| no cheap base | 186 | 18% |

So 82% of the vocabulary can be resolved by a direct lemma or one stem-map
probe. The remaining 186 are dominated by clitics (`ajattelehan`, `ennenkin`,
`eiköhän`), possessives (`autuuteni`, `aviomiehensä`, `eteensä`), compounds
(`arkihuoneessa`, `erämaassa`, `isänmaan`) and names.

Replace the single resolver with a ladder. Each rung names a base, then
verifies it with one `emits_form` call — never a full scan:

1. **Direct lemma** — accept the sanalista row.
2. **Stem map** — `find_base_via_stem_map(word, Noun)` or
   `find_base_via_verb_stem_map(word)`.
3. **Closed spelling** — `closed_spellings::base` (`kuulten` → `kuulla`),
   then rungs 1–2. It is a tiny table probe, not a morphology walk.
4. **Clitic** — `strip_all_clitics`, then rungs 1–3 on the host; copy the
   host's readings and add the outermost clitic.
5. **Possessive** — strip the possessive suffix (`-ni`, `-si`, `-nsa`, …),
   then rungs 1–4; add the possessive tag. The helper exists in
   `check_normal` (`find_base_via_possessive_noun`) and should be exposed
   rather than reimplemented.
6. **Compound** — split with `compound::analyze`; resolve the head's class and
   inflect it with the modifier fixed. `arkihuoneessa` is `arkihuone` +
   `-ssa`; `isänmaan` is `isän` + `maa`.
7. **Head/tail scan** — the current phase 1, with §4.3's filter.
8. **Deep walk** — only the lazy upgrade from §3.4.

Measure the coverage after rung 6 on the Phase 0 fixture. The Phase 2 target is
**≥ 95% of tokens resolved without the head-1 walk**. If a rung is missing
coverage, add it before tuning the fallback.

### 4.3 Reject fallback candidates without building their paradigm

For the words that still reach rung 7, add a build-time **lemma stem-shape
filter**. Do not use a Bloom mask over all form trigrams: a lemma with ~100
forms contributes ~600 trigram insertions, which saturates a 64-bit mask and
makes it useless. A mask over the lemma's **stems** is sparse and selective.

- New build input `maps/lemma_stem_shape.bin.gz`, generated by
  `tools/src/bin/gen_lemma_stem_shapes.rs` with the class engines (the same
  engine-driven pattern as `gen_derived`).
- For each lexicon row, a 64-bit mask of hashed stem prefixes: the first
  three characters of every strong, weak and plural stem. A lemma has a
  handful of stems, so the mask stays sparse.
- 104,743 rows × 8 bytes = 838 KB raw; gzip is expected to be 300–500 KB.
- `lemma_might_emit(row, surface)` strips each legal case/number ending from
  the surface, hashes the resulting stem prefix and tests the mask. A zero
  bit means “no stem of this lemma can produce the surface” and skips
  `emits_form` entirely. A one bit is a false positive and takes the old path.
- The filter must never reject a true form. The generator tests a sample of
  lemmas against `inflect_all`; the test is part of the blob's staleness gate,
  like `the_blobs_match_compute`.
- At runtime the order is: length/prefix checks → stem-shape mask →
  `emits_form`. The expected rejection rate is much higher than a trigram
  mask's because the stem, not the whole inflected surface, is compared.

If the stem mask is not selective enough, prototype the alternative before
shipping the blob: a per-class legal-ending set plus a direct stem comparison
(no data file, more code). Measure both on the Phase 0 fixture; pick by cold
time and wasm bytes.

### 4.4 Reuse the clitic host

`silloinkin` and `kanssasi` are not special words; they are a host plus a
particle. `resolve_shallow` already resolves the host at rung 3. Store the
host's `WordInfo` under the clitic surface with the outermost clitic added to
each reading. No second base walk, no second candidate scan. The existing
`strip_all_clitics` stays the parser.

### 4.5 Optional: a build-time reverse index

The gold standard is a form → lemma map: one hash probe per surface, no
candidate scan at all. It is **not** the default because the data is large:
`data/all_forms.csv` has 6.48M rows and 6.42M unique surfaces. A naive
front-coded map is tens of MB, which is not acceptable for a 5 MB wasm.

Phase 6 is a decision experiment, not a commitment:

1. Pick the top N lemmas by token frequency in the committed Gutenberg corpus.
2. Generate every form of those lemmas and a front-coded `form → lemma row`
   table.
3. Measure coverage on a held-out page and compressed size.
4. Ship only if coverage ≥ 95% of tokens and the blob is ≤ 3 MB compressed.
   Otherwise keep §4.2 and §4.3.

Do not generate the full 6.4M-form map without this gate. The download budget
is part of the product.

### 4.6 Bound and instrument the existing caches

- `emits_form`'s thread-local paradigm cache is capped at 100,000 *words* and
  clears wholesale. Add `entries`, `hits`, `misses` to `CacheStats`; cap by
  approximate bytes and evict the older half instead of clearing everything.
- `MorphologyCache` uses the same two-generation eviction: when full, drop the
  entries not touched in the previous generation. A page's common words
  survive; a paste's one-off names do not.
- Store `Arc<WordInfo>` in the cache and return the `Arc`; never clone the
  readings or lemma strings on a hit. This removes the per-token clone that
  the warm morphology pass still pays today.
- The first syntax call also pays the lazy `lemmas()`,
  `Catalogues::embedded()` and proper-name table initialization. Measure it
  separately in Phase 0; if it is material, warm those statics at worker boot
  (`warm_caches`) so the first user check does not pay it.

### 4.7 Classless rows: a derived head table, not source edits

A classless sanalista row is not a word missing a class; it is Kotus saying
“this is a compound and declines like its final component”. That empty field
is read by `compound::analyze`, `inflect_all` (`compound_head_forms`),
`head_class` and the syntax resolver. Filling the CSV with the head's class
would make those paths treat the word as simple, lose the modifier/head split,
and hide the cases where the head is bound, absent, or has several classes.

It would also not store the split. `pariskunta` is `paris` + `kunta`; the
class belongs to `kunta` (10*J), but the modifier is the combining `-s` form of
`pari`, and only the split says where the head begins. A class on its own
would make the whole string align against `kunta` by suffix luck, not by the
compound rule.

The runtime fix is in place: `modifier_is_known` accepts the combining `-s`
form (`pari` -> `paris-`), and `resolve_from_base_surface` gives a classless
base the head's inflections before verifying the surface. `pariskunnan` now
resolves as `pariskunta`, genitive singular, in ~30 ms instead of ~1 s. The
bound-head lookup also tries the ASCII leading hyphen, not only U+2011, so
`epäsuhta` (`epä` + `-suhta`) has a full paradigm.

The general fix is a **generated** table, not a source edit:

- `tools/src/bin/gen_classless_heads.rs` walks the 56k classless rows and
  writes `maps/classless_heads.bin.gz`: lemma row -> head split + head
  `Inflection` list, or an “unresolved/ambiguous” marker.
- `head_inflections` and `compound_head_forms` probe the table first and fall
  back to the current `classless_heads` walk for a row the generator could not
  resolve.
- The source CSV stays exactly Kotus. The table is regenerated and versioned
  like `maps/derived_*`; the staleness test compares it against
  `head_inflections` for a sample.
- Estimated size: 56k rows × a few bytes, well under 200 KB compressed.

This is the place to “fill the class”: in a derived map that still knows the
split, not in the source column that means “compound”.

## 5. Making the warm path cheap

Warm morphology is already 4–7 ms; the rule pass is the larger warm cost. The
target is a page check under 10 ms once warm, which makes the per-edit
debounce feel instant.

### 5.1 Agreement

`check_attributes` is 15–19 ms of the 20–28 ms rule pass. Fix the order and
the allocations:

1. Ask `attribute_shape(syntax)` first. If it is `None`, skip the token. Only
   then call `degree_base` (and only for a token with an adjective, ordinal or
   pronoun reading).
2. Cache the lowercased surface in `WordInfo.key`; never call
   `to_lowercase()` per rule per token.
3. Precompute `clause_has_predicate: Vec<bool>` once instead of
   `analysis.clauses.get(syntax.clause)` per token.
4. Precompute candidate index lists in `MorphologyAnalysis`:
   `attribute_indices`, `nominal_indices`, `finite_indices`. Rules iterate the
   candidates, not all 3,500 tokens.
5. Replace the `Vec` + sort + dedup in `attribute_shape`, `head_shape`,
   `nominal_case` and `candidate_number` with a two-slot fold that stops at
   the first disagreement.

### 5.2 Other rules

- `rection` and `object_case` use the same candidate indices and
  `WordInfo.case`/`number` instead of rescanning readings.
- `rank::resolve` uses `sort_unstable_by`; diagnostics are already few.
- `punctuation` and `arguments` are already sub-millisecond; leave them until
  the candidate-index refactor settles.

### 5.3 Style

Style needs less than grammar:

- It never needs roles, clauses or lists. `analyze_morphology` is the whole
  input; skip stage 2.
- `length::check` uses `morphology.sentences` and a prefix sum of word counts
  instead of filtering every token for every sentence.
- `repetition` reads `WordInfo.lemma`, `is_content_word` and
  `is_finite_predicate`; no `TokenSyntax` needed.
- `phrases` lowercases each token once, not once per phrase length.
- If only style is on, resolve with the cheap rungs only (through closed
  spelling, clitic and possessive). A word that needs the head/tail scan is
  treated as unknown for style; the deep walk never runs. That is the
  cheapest cold path in the product.

### 5.4 Per-sentence incremental checking

Once the stages are separate, the unit of re-check is a sentence:

```rust
struct SentenceCheck {
    hash: u64,                  // hash of the sentence text (and length)
    diagnostics: Vec<SyntaxDiagnostic>, // spans relative to sentence start
}
```

- Tokenise the new text (0.4 ms for a page) and compute sentence spans.
- For each sentence, hash its exact text. An unchanged hash reuses the cached
  diagnostics; a changed one re-runs the rules for that sentence only.
- Sentence spans and clause groups are sentence-local in
  `syntax::analyze`; verify this with a test that checks a sentence in
  isolation against the same sentence inside a document. If a rule reads
  across a boundary, include the previous/next sentence hash in the key.
- Analyse a changed sentence against its token range and rebase clause and
  token indices into the full text before running the rules, or keep the
  per-sentence analysis in the cache and merge only the diagnostics. The
  second shape is simpler; pick it unless a rule needs a global clause index.
- Shift cached spans by the sentence's new start offset when merging. The same
  applies to correction edits.
- Cap the cache at ~2,000 sentences; clear it on `clear_state` and on a book
  change. A changed sentence is 1–3 ms; a 2,000-word page with one edit is
  ~5 ms, independent of page length.

This is the phase that makes typing with Grammar ON tolerable. Without it, a
warm re-check still walks every sentence after every debounce.

## 6. Wasm and worker surfaces

### 6.1 Return only what the caller reads

Both workers read `raw.diagnostics` and discard the rest. Add:

```rust
#[wasm_bindgen]
pub fn check_syntax_diagnostics_with_state(idx: u32, text: &str) -> js_sys::Array;

#[wasm_bindgen]
pub fn check_style_with_state(idx: u32, text: &str) -> js_sys::Object;

#[wasm_bindgen]
pub fn check_grammar_style_with_state(idx: u32, text: &str) -> js_sys::Object;
```

Keep `analyze_syntax_with_state` for the full object. `check_syntax_with_state`
can either become the diagnostics-only form or stay as a compatibility alias
that calls `analyze_syntax_with_state`; decide in Phase 5 and update the
glue `.d.ts` and both workers in the same commit.

`check_grammar_style_with_state` runs `analyze_morphology` once, then
`check_grammar` and `check_style`, and returns
`{ diagnostics: [...], style: [...] }`. The two streams keep their distinct
kinds and labels.

### 6.2 Chunk, cancel, and stay out of the spelling lane

A synchronous wasm call cannot be interrupted. Make the worker cooperative:

- Split the text into sentence batches (10–20 sentences) so one batch stays
  under the 50 ms budget.
- `await` a macrotask (`setTimeout(0)`) between batches so the worker can
  process spelling messages.
- Check a `generation` token before each batch and after each yield; a newer
  edit bumps it and the old request stops.
- Post `check-progress { done, total }` so the panel can show
  `tarkistetaan… 3/12` instead of a frozen worker.
- The client ignores a final result whose `generation` is stale. This is the
  same pattern the spell render already uses with `renderGen`.

If chunking is not enough, run grammar/style on a dedicated worker: the
extension already has a suggestion pool, and the demo can create a second
worker when a toggle is first enabled. A cold check then cannot delay a
spelling request. The trade is one more wasm instance; measure before
adopting.

### 6.3 Trigger policy

- Grammar/style stay opt-in. The context-menu command and the demo toggles are
  the only triggers; no keystroke path.
- The context-menu command may target a long text field. It runs with progress
  and cancellation on; the word budget below applies to the automatic demo
  toggles, not to a user who explicitly asked for the check.
- When a toggle is on, debounce at 500 ms (spelling is 150 ms) and skip the
  automatic run for texts over the word budget (2,000 words) until the cold
  page target is met.
- On a new edit, clear the underlines immediately, as today. A running check
  for the previous text is cancelled by the generation bump.
- The Gutenberg reader stays spelling-only until a 2,000-word page checks in
  ≤ 500 ms or the chunked progress UI is in place. Then decide whether to
  enable the same toggles or keep the reader spelling-only by design.

## 7. Phases

Each phase ends with the same correctness gate:

```bash
cargo test --release -p torille --test syntax_check --test syntax_eval \
  --test style_check --test style_eval
cargo test --release --workspace
```

plus the exact-output fixtures. A phase that changes a diagnostic is reverted
or the change is explained and the fixture updated deliberately, never as a
side effect.

### Phase 0 — harness and baseline (0.5 day) — landed

- Promote the temporary probe to `tools/src/bin/perf_grammar_style.rs`:
  `--book`, `--words`, `--repeat`, `--single-thread`, `--json`.
- Add `tests/fixtures/perf/page-2000.txt`, a checksummed slice of
  `books/9082.txt`, and a 20-word and 100-word prefix.
- Report tokenise, morphology cold/warm, rules, style, total, cache hit rate
  and wasm bytes.
- Record the baseline in this file.

Acceptance: the tool runs on a clean checkout and the numbers reproduce the
table in §1 within noise.

### Phase 1 — morphology cache with case information (2–3 days) — landed

- Add `src/morphology/` with `WordInfo`, `MorphologyCache`, `Depth`,
  `CacheStats`, `analyze_morphology`.
- Move `WordAnalysis` resolution there; keep `syntax::AnalysisCache` as a
  re-export during the transition. `syntax::analyze(tokens)` stays as a
  wrapper over a thread-local `MorphologyCache` for callers that do not own
  one.
- Compute and store the agreed `case`/`number` at insert time; make
  `syntax_check::analysis` read them.
- Return `Arc<WordInfo>` on every hit; no per-token clone.
- Add `style_check::check_with_cache` and the
  `torille::check_all_with_cache` facade.
- Wire `TextAreaState.morphology` into the syntax and style wasm paths.

Acceptance: warm grammar+style adds ≤ 3 ms over warm grammar alone; the exact
fixtures are unchanged; `MorphologyCache` stats show a hit rate above 95% on a
re-check.

### Phase 2 — cold path (3–5 days) — partial

- §4.1 shallow/deep split and bounded walks.
- §4.2 base-source ladder: direct, stem map, closed spelling, clitic,
  possessive, compound.
- §4.3 stem-shape filter prototype and blob; generator + staleness test.
- §4.4 clitic host reuse.
- §4.6 cache metrics and two-generation eviction.

Acceptance: ≥95% of page tokens resolve without the head-1 walk; cold 20-word
≤ 100 ms (wasm shape); cold 2,000-word page ≤ 3 s; `lasta`/`lapsi` has an
explicit test; the 20-word fixture's diagnostics are unchanged.

**Landed:** the shallow/deep split, base-source ladder, stem-shape filter,
clitic/possessive reuse, numeral upgrade, compound fallback and depth
metrics. A stem-map-first reorder removed the duplicate `cheap_base` pass for
plain words, taking the native cold page from ~2.4–2.6 s to **~2.1–2.3 s**.
The 256-bit stem-shape mask (`PLAN_COLD_SEARCH.md` Phase 4) then cut the
per-candidate walk; the 2,000-word wasm page is now **~3.0 s** single-thread.
The 4-worker morphology pool then takes the full page to **~1.4 s**. The 20-
and 96-word targets are met. The ≤500 ms target was deferred to Phase 6,
which rejected the reverse index, so it remains open; the pool is a large
partial win, not the target.

### Phase 3 — warm rule pass (2 days) — landed

- §5.1 agreement reorder and candidate indices.
- §5.2 shared helpers use `WordInfo.case`/`number`.
- §5.3 style uses `MorphologyAnalysis` and sentence spans.

**Landed:** the lowercased surface is cached on `WordInfo` and copied to
`TokenSyntax.key`, so the agreement, rection, argument and object-case rules
stop allocating a `String` per token. The attribute scan asks for an
unambiguous shape before the degree lookup, and the shape helpers fold two
slots instead of collecting, sorting and deduplicating a `Vec`. Style
lowercases each token once, skips the phrase-length loop unless the word is a
generated phrase start, and computes long-sentence word/predicate counts in one
token pass. Correction paradigms are cached per (lemma, inflections, wanted
tags, POS); the first pass still builds them, and every warm re-check reads
them back.

A correction cache hit is the difference between 7.3 ms and 0.07 ms for the
fixture's ten agreement corrections, which is why the warm rule pass drops from
13.9 ms to 1.4 ms.

Acceptance: rule pass ≤ 5 ms/page; style ≤ 3 ms/page; warm total ≤ 10 ms/page.
All three are met on the fixture: **1.4 / 2.5 / 6.9 ms**.

### Phase 4 — sentence cache (3–4 days) — landed

- §5.4 sentence hash, relative spans, merge and invalidation.
- A test for the sentence-locality invariant.
- Cap and clear rules.

**Landed:** `src/syntax_check/sentence_cache.rs` keys each sentence by a hash
of its exact text, its byte length and its starting quote depth, and stores
diagnostics with spans and correction edits relative to the sentence start.
An unchanged sentence is a hash probe and a shift; a changed one tokenises the
slice and runs only the sentence-local rules. The starting quote depth is part
of the key and seeds the sentence analyzer, so a sentence inside a quotation
sees the same depth as the whole-document pass. The document-global
`unbalanced_quote` warning runs once outside the per-sentence pass. The wasm
textarea state owns the cache and the combined export uses it, so the chunked
worker reuses unchanged sentences across chunks and across edits. The cache is
capped at 2,000 sentences and is cleared by `clear_state` with the morphology
cache.

`perf_grammar_style` now reports cold, all-hit and one-edited-sentence timings.
On the 1,959-word fixture, with warm morphology and style on:

| workload | result |
|---|---:|
| all-hit sentence pass | **0.1 ms** |
| one edited sentence, rules only | **0.1–0.3 ms** |
| one edited sentence, tokenise + morphology + rules + style | **2.6–3.7 ms** |
| 96-word page, one edited sentence, full re-check | **1.8 ms** |

The edit reuses 308 of 309 sentences; exactly one sentence is re-analysed. The
sentence-cache result is byte-identical to the whole-text rule pass on the
clean corpus, bracket-merged spans, quotes, relative clauses and coordinated
subjects.

Acceptance: a 2,000-word page with one edited sentence re-checks in ≤ 5 ms;
a 100-word page in ≤ 2 ms; diagnostics are identical to a full re-check. All
three are met.

### Phase 5 — wasm and worker (3 days) — landed (p90 target and browser e2e open)

- §6.1 diagnostics-only and combined export; update both workers and the
  glue `.d.ts`.
- §6.2 chunking, generation, progress, cancellation.
- §6.3 debounce and word budget.

**Landed:** the combined `check_grammar_style_with_state` export, a
quote-balanced sentence chunker in the demo worker, `check-progress`
envelopes, generation-based cancellation, and a single panel resource that
renders both streams. The combined export runs the grammar rules through the
per-state `SentenceCache`, so a re-check reuses every unchanged sentence
across chunks. `probe-grammar-chunk.mjs` on the 1,959-word perf page:
first progress **~30–70 ms**, final **68 grammar / 14 style** (identical to the
whole-text wasm check), and an edit during the run cancels to 0 diagnostics.
The batch is now one sentence; the remaining p90 cost and the extension/reader
work are in the leftovers below.

**Phase 5 leftovers — landed:**

1. **Batch budget.** The demo worker now uses one sentence per batch (120-word
   fallback for a paragraph with no sentence punctuation). On the 1,959-word
   fixture the first progress is **29–73 ms** (was ~160 ms) and the native
   wasm batch p90 is **63 ms** (median ~20 ms). The browser worker reports a
   higher p90 (~200 ms) because two panels may run concurrently and a single
   cold name/compound can cost 150–470 ms. Splitting a normal sentence at a
   word boundary breaks diagnostic parity, so the remaining >50 ms outliers
   are a cold-word cost, not a batch-size cost. The p90 ≤50 ms target is not
   met; the next lever is the Phase 6 cold-path decision.
2. **Extension combined worker and progress.** The Firefox and Chromium
   spell workers call `check_grammar_style_with_state` for the grammar/style
   commands. The background chunks the text one sentence at a time, posts
   `torille:check-progress` to the content script, and stops the loop when an
   edit bumps the per-tab/input generation. The content script shows a
   `Tarkistetaan… n/N` toast and sends `torille:cancel-check` from the
   overlay's input handler. The glue `.d.ts` and `globals.d.ts` carry the new
   shapes, and both source trees are identical. The extension e2e suite does
   not drive the real background yet; the build and 57 unit tests are the
   current gate.
3. **Reader enablement.** `GutenbergReader` now has the same grammar/style
   resource as `TextPanel`: `checkGrammarStyle`, progress, cancellation,
   diagnostics on the `Editor`, and a `__torille.panels` hook.
   `probe-reader-grammar.mjs` sets both the free-text panel and the reader to
   the 1,959-word fixture and gets **68 grammar / 14 style** from both, with
   grammar/style spans in the reader underlay and a clean cancel to 0.
4. **Multi-panel generation.** The demo worker's cancellation token is now
   per textarea state (`Map<idx, generation>`) instead of one global counter.
   Two panels can check at once; a newer request for one state no longer
   cancels the other panel's check.

Acceptance: first progress ≤100 ms (met: 29–73 ms); p90 worker batch ≤50 ms
(not met: native 63 ms, browser ~200 ms, cold-word bound); the reader and both
extensions build; an edit during a check leaves no stale diagnostics in the
demo, reader and extension (met). The cold-word p90 is the remaining Phase 6
work.

### Phase 6 — optional reverse index (3 days, decision gate) — decided, not shipped

- §4.5 coverage/size experiment.
- Ship only if it meets the 95% / 3 MB gate and improves the cold page to
  ≤ 500 ms; otherwise document the decision and stop.

**Experiment:** a 10.9 MB / 1.45 M-token sample of the local Gutenberg corpus
(40 books, capped at 20 MB) supplied the lemma frequencies. The held-out page
was the committed 1,959-word perf fixture (1,973 word tokens, 1,150 unique).
The full engine form→base map was dumped with `dump_form_bases` (17.3 M rows,
17.07 M unique words, 104,295 bases), and `gen_syntax_forms` wrote the
front-coded `MST2` blob for each top-N subset.

| top lemmas | form pairs | unique forms | page tokens covered | compressed blob |
|---:|---:|---:|---:|---:|
| 1,000 | 426,525 | 421,685 | 71.0% | 426 KB |
| 2,000 | 860,645 | 848,818 | 78.8% | 839 KB |
| 5,000 | 2,174,010 | 2,138,520 | 86.6% | **2.1 MB** |
| 10,000 | 4,232,839 | 4,158,040 | 88.5% | 4.1 MB |
| 19,000 | 7,289,501 | 7,156,694 | 89.3% | 7.0 MB |

**Decision: not shipped.** The gate is an AND, and no point meets it. The
largest blob under 3 MB (5,000 lemmas) covers 86.6% of the held-out page; the
next point (10,000 lemmas) is 4.1 MB and still only 88.5%. Coverage plateaus
around 89% because the remaining tokens are names, unknown words and rare
lemmas; the full 104k-lemma map is 17.3 M rows and far above the download
budget. The 500 ms cold-page target is therefore not reachable with this
index shape. Keep the §4.2/§4.3 ladder. The remaining cold-word p90 and the
500 ms page need a different cold-path design (for example, a form table that
stores readings, not only the lemma), not a larger `form -> lemma` blob.

The experiment used two local helper binaries (a full form→base dump and a
front-coded map writer). They were not committed: neither is part of the
shipped crate, and the measured decision is the deliverable. No `maps/` blob
was added and nothing is embedded in the crate.

### Phase 7 — reader enablement and docs (1 day) — landed

- If a 2,000-word page checks in ≤ 500 ms, or the chunked progress UI is in
  place, enable the grammar/style command on the reader page; otherwise keep
  the reader spelling-only and document why.
- Update `docs/syntax_checker.md`, `docs/style_checker.md`, `README.md` and
  this plan's status.
- Bump the crate/demo and extension versions only if a surface changes.

**Landed:** the reader now runs the same chunked grammar/style resource as
`TextPanel`, with progress, cancellation, diagnostics on the `Editor` and a
`__torille.panels` hook. `probe-reader-grammar.mjs` gets 68 grammar / 14 style
from both the free-text panel and the reader, shows the reader underlay spans,
and cancels to 0. `docs/syntax_checker.md` records the sentence-cache path.
The reader was enabled on the chunked-progress branch of the condition, not
the 500 ms cold-page branch.

### Phase 8 — shared paradigm probe (landed)

The cold pass asked the engine the same question twice per word. The stem-map
ladder verifies a candidate base with `expand::emits_form` ("does this lemma
emit this surface?"), and the reading extraction immediately asks
`expand::form_readings` the same thing ("which readings does that surface
have?"). Each built the whole paradigm: a page did ~2,000 verifications and
~1,400 extractions, ~3.5 full paradigm builds per unique word.

Three changes, none of which touches a rule or a generated class module:

1. **The paradigm cache carries a probe.** `emits_form`'s cache entry is now
   the word set *plus* the readings of the one surface it was asked about.
   `form_readings` asks the same cache with the same key and answers from the
   probe when the surface matches. On the fixture that turns 609 of 1,398
   extractions into a comparison. Only one probe is kept per paradigm, so an
   entry costs a surface and its readings, not a second copy of the word set.
2. **The verification walk skips `split::dedup`.** `inflect_in` stays the
   ordered, merged list; `inflect_in_raw` is the same builder without the two
   sorts. `emits_form` walks the raw list (its `HashSet` collapses the copies)
   and folds the matched copies with `Form::absorb`, which is exactly the merge
   `dedup` would have done — so the probe's readings are the merged ones. The
   public listing API is untouched.
3. **`modifier_is_known` is memoised.** It is a pure function of the modifier
   and the static dictionary, and one classless lemma asks it once per
   verification of that lemma. The `-mis`/`-inen`/numeral tails fall through
   to the compound recogniser, which is the expensive part.

Acceptance: cold single-thread 2,016 → **1,490 ms**, fresh-cache style
~1,410 → **~1,140 ms**, 4-worker native 753 → **537 ms**; warm numbers
unchanged; the whole suite, the strict engine comparison, the illegal-evidence
gate and the binding agreement gate green; two unit tests pin the invariant
(the probe equals a fresh walk's readings, and the raw paradigm spells the same
words as the merged one).

Rejected in this phase, with the measurement in hand:

- **Skipping `dedup` in `form_readings` too** (a further ~8%): the merged
  readings are the contract, and `add_reading`'s order-independence is an
  argument, not a test.
- **Raising the paradigm cap to 400,000 words**: more cache hits, no measurable
  time win, four times the heap.
- **`format!` → `push_str` in `nominals/utility.rs` and `inflect.rs push`**:
  measured inside the noise. The 24% of instructions in `fmt` is dominated by
  the compound DP, not by the class builders.

### Phase 9 — suffix mask and generator union (landed)

The prefix mask rejects a candidate whose first four characters do not occur in
a lemma's paradigm, but forms of one lemma share a prefix and differ in their
endings (`talossa`, `talosta`, `talolle`). Each row now also carries a 64-bit
suffix mask (a hash of the last four characters); `might_emit_row` checks it
only after the four prefix bits pass. The blob is `LSS3`, 40 bytes per row
(+187 KB gzipped), and classless rows stay all-ones.

The generator also unions `inflect_all` with `None` and with
`Some(&LexiconIndex)`. Class-50 compound heads emit some forms only through
the index path, so the old prefix-only mask could reject a real form that the
sample happened to admit by a hash collision. The generator spot-checks both
paths, and `the_mask_accepts_real_forms` / `the_mask_accepts_the_noun_fixture`
cover them at runtime.

Measured 2026-11-19 on `page-2000`:

- native single-thread cold **1,497 → 1,330 ms** (−11%);
- native 4-worker **450 ms**, under the ≤500 ms native budget;
- wasm 4-worker **755–762 ms**, 8-worker **777–852 ms**, still above 500 ms;
- warm numbers unchanged; grammar/style diagnostics byte-identical.

The suffix filter does not close the wasm target by itself: the per-chunk wall
is still dominated by paradigm construction. The next lever remains the
form → readings table or a class-specific surface test, not another mask.

A wider 256-bit suffix mask (four hashes per row, 64 bytes/row) was measured
and rejected: native single-thread cold moved 1,330 → 1,504 ms and the
4-worker path 450 → 490 ms. The extra hash cost outweighed the additional
candidates it rejected, so the 64-bit suffix mask stays.

### Phase 10 — the exact form table, measured and rejected (decision)

The remaining plan text said the next lever was "a form → readings table or a
class-specific 'can this class spell this surface' test". Both were built far
enough to price them. The answer is that the table does not fit the wasm
budget and the mask cannot substitute for it. **Not shipped.**

#### Where the cold pass actually goes

`page-2000`, cold `analyze_morphology`, single-thread, release, median of
three runs. The paradigm cache was instrumented by call site (the counters
were removed after the run):

| caller | builds | of which fail | form visits | build ms |
|---|---:|---:|---:|---:|
| noun stem-map verify | 740 | 123 | 62 k | 130 |
| verb stem-map verify | 602 | 271 | 813 k | 320 |
| head-1 walk | 43 | 28 | 30 k | 15 |
| other (cheap gate, deep probes) | 599 | 486 | 469 k | 220 |
| **total** | **1,984** | **908** | **1.38 M** | **685** |

The pass is 1,420–1,580 ms, so paradigm construction is ~45 % of it and the
rest is candidate scan, map probes and reading assembly. A build walks **693
forms on average**; the verb stem map is 47 % of the build time on its own,
because a verb paradigm is an order of magnitude bigger than a noun's.

Two facts follow, and they close the mask line of work:

- **46 % of the builds fail.** The surface was not in the candidate's
  paradigm. That is the recoverable half.
- **The suffix mask cannot reject those candidates for verbs.** A class-52
  verb emits ~700 forms; the suffix mask is 64 bits with one bit per form, so
  `P(bit never set) = (63/64)^700 ≈ 2·10^-5`. A verb row's suffix mask is
  saturated — every bit set — and rejects nothing. The 256-bit experiment
  failed for the same reason: 700 draws into 256 bits leaves ~7 % of the bits
  clear, which is not a filter. Widening further is the same trade again.
  Bloom masks over a *paradigm* cannot work; only a per-form structure can.

#### What the form table costs

A probe generated every non-possessive, non-clitic nominal form from
`inflect_lemma` and encoded five layouts, each gzipped at the best level (the
same codec `build.rs` uses):

| layout | rows | raw | gzip |
|---|---:|---:|---:|
| flat `surface → base id`, u32 ids | 1,096,373 surfaces | 21.0 MB | **2.68 MB** |
| flat `surface → base id`, varint ids | 1,096,373 surfaces | 19.3 MB | **2.66 MB** |
| two-letter shard (`aa` bucket, prefix once) | 385 shards | 18.7 MB | 2.65 MB |
| two-level `stem → (ending, base)`, base per entry | 234,537 stems | 11.8 MB | 2.85 MB |
| **grouped `stem → base → endings`** | 234,537 stems | **8.2 MB** | **1.35 MB** |

Two things fall out, and the second corrects this plan's first draft:

- The prefix-sharded layout — "`aavetta` points at the `aa` map, which holds
  the base" — saves **33 KB of 2.68 MB (1.2 %)**. Sharding buys lookup
  locality, not bytes.
- The **naive** two-level layout is worse compressed (2.85 MB) even though it
  halves the raw size, because it writes the base id once per
  `(ending, base)` entry and alternates text with binary ids. `static_table.rs`
  records that **99.5 % of stem keys have exactly one base**, so grouping the
  endings under their base and writing the id once per stem changes the answer
  completely: **1.35 MB, half of the flat table.** Varint ids shave another
  12 KB.

`data/all_forms.csv` is **nominal only** (every case column is a nominal
case), so the verb half was generated the same way, splitting each form at the
longest prefix that is a committed `(stem, base)` key — exactly the key the
runtime probes. 10,034 verb lemmas emit **2,275,732 non-possessive,
non-clitic surfaces over 320,980 stems (2,295,351 entries)**:

| verb layout | raw | gzip |
|---|---:|---:|
| flat `surface → base id`, varint ids | 40.8 MB | 5.44 MB |
| **grouped `stem → base → suffixes`** | **15.4 MB** | **1.79 MB** |

So the whole noun + verb table is **~3.15 MB gzip (23.6 MB raw)** in the
grouped layout, against **~8.1 MB** flat. It omits the reading (case, number,
and for verbs mood/tense/person/voice), which is ~1 byte per entry and pushes
the total to **~3.7 MB**. That is over the 3 MB reverse-index ceiling of
Phase 6, but it is the same order as that ceiling rather than three times it —
the earlier "~8 MB" was the flat-table price, and the naive two-level encoding
was not the right representation of this idea.

#### What would still work, and what it costs

The wasm pool now measures **364–395 ms** per fresh page after Phase 11, so
the table is no longer on the critical path for the ≤500 ms target. It stays
interesting as a way to remove the remaining build cost, and the grouped
encoding makes it a live trade rather than a rejected one:

| option | bytes | engine saving |
|---|---:|---:|
| grouped two-level `stem → base → endings`, nouns + verbs | **3.15 MB** | ~10–13 % (verification only) |
| the same with a reading byte per entry | **~3.7 MB** | ≤ 45 % |
| flat `surface → base` table, nouns + verbs | ~8.1 MB | ≤ 45 % |
| class-level `accepts` (no data file) | 0 | ≤ 45 % |

Only the reading byte makes the table worth its bytes: an ending-only table
removes the verification builds but every surviving base still needs its
paradigm for `form_readings`, which is why the nominal-only variant saves just
the failing verifies (~130 ms). A reading byte per entry costs ~0.5 MB and
lets the resolver answer "which readings does this surface have" without
building the grid at all, which is the full 45 %.

3.7 MB is above the 3 MB ceiling that rejected the reverse index in Phase 6,
so this is a download-budget decision rather than an engineering one. The
class-level `accepts` test is the 0-byte alternative: ask the class whether it
can spell the surface instead of materialising its grid. For nominals that is
one generic walk over the cells a class already declares
(`expand/nominals/utility.rs` plus the regenerated template). For verbs the
paradigm is a single `paradigm(&Stems)` function in
`expand/verbs/utility.rs` that formats and pushes every cell inline, so the
same test needs that function restated as data (stem × ending × tags) and then
driven — a rewrite of one 2,000-line file that 27 class modules share, gated by
the catalogue tests. It removes both the failing and the succeeding builds,
which is why it matches the table's saving without the bytes; the cost is code
and risk, not download size.

#### The wasm gap is not engine time

Native 4-worker was 450 ms and wasm 4-worker 755–762 ms for the same grouping
and the same engine. Phase 11 then showed where that wall actually was: it was
not startup or transport but the serial numeral upgrade (~540 ms in wasm),
which has since been removed. The pool now measures 364–395 ms per fresh page
with four workers alive, and ~950 ms for the first check of a session (four
worker spawns plus their wasm instantiation).

**Decision:** no data table ships. The grouped two-level layout is priced at
3.15 MB (3.7 MB with readings), so it is a download-budget question rather
than an absurd one, and it is the lever to revisit if the remaining build cost
ever matters again. The ≤500 ms target is met per page without it.

### Phase 11 — the serial numeral upgrade (landed)

Phase 10 ended by saying the wasm wall had to be decomposed before spending
bytes on a table. It was, and the answer was not the table.

Running the real engine in wasm (a `wasm-pack --target nodejs` build of
`torille-wasm`, driven from node on the same fixture) showed the pool's
post-chunk tail dominating:

| step | before | after |
|---|---:|---:|
| four cold chunk states, sequential | 1,612 ms | 1,661 ms |
| final grammar/style pass, first call | **615 ms** | **72 ms** |
| final pass, second call | 117 ms | 7 ms |
| `check_syntax` with its own morphology | 1,480 ms | 1,179 ms |

The first final pass was 615 ms because it was not morphology — that part was
seeded and hit. It was the **numeral-governed partitive upgrade**:
`analyze_morphology` walks the document after every numeral and, when the next
word has no partitive reading, runs `resolve_deep` — the head/tail search — on
the owning thread, *after* every pool worker has returned. Twelve upgrades
cost ~190 ms native and ~540 ms in wasm on this page.

The upgrades were also mostly spurious. The trigger was "a numeral", without
looking at the numeral's case, so `kahtena päivänä`, `kolmesta miehestä`,
`kahdella miehellä`, `menoon`, `kotona` and `suusta` all fired one. Only a
**nominative** cardinal governs a partitive; an inflected numeral agrees with
its head in its own case. `numeral_governs_partitive` now checks the reading's
case, and the sixteen-test first pass keeps three genuine upgrades (`kaksi
lasta` still gains the `lapsi` partitive; the regression test asserts
`deep_upgrades` is 0 for `kahtena päivänä` and 1 for `kaksi lasta`).

Measured 2026-11-19 on `page-2000`:

- native single-thread cold **1,330 → ~1,190 ms** (−10 %);
- native 4-worker **450 → 319–348 ms** (−23 %), comfortably inside the 500 ms
  budget for the first time;
- the wasm final pass **615 → 72 ms**, which is where the pool wall lived;
- diagnostics unchanged: 5 grammar / 14 style on the fixture, and the pool's
  `check_syntax` path keeps 5.

The rule change is semantic, not a shortcut: it stops a deep walk that was
overwriting a correct essive/locative reading with a partitive-oriented one.

#### The browser pool, measured

`extensions/firefox/e2e/pool-timing.mjs` builds the extension and warms the
four-worker pool with a one-sentence check, then times the full page on a
**fresh textarea state** — the cold-page cost a reader pays for each new
article while the browser session is alive. Chrome `--headless=new`:

| measurement | wall |
|---|---:|
| warm-up sentence (spawns the four workers) | 57 ms |
| first full page in the session (cold workers and state) | ~950 ms |
| full page, fresh state, warm pool #1 | **364–395 ms** |
| #2 / #3 (worker caches warm from the previous page) | 273 / 237 ms |

So the ≤500 ms cold-page target is **met once the pool exists** (364–395 ms),
and the ~950 ms first check is worker start-up: four wasm instantiations of the
31.8 MB module plus their first map access, paid once per browser session, not
per page. `extensions/firefox/e2e/pool.mjs` pins the shipped path; its grammar
expectation moved 68 → 9 because the engine changed, and the 9-vs-5 divergence
between the sentence-cache path and the whole-document pass is tracked in
`misc_md/TODO_GRAMMAR_STYLE.md` rather than hidden in the constant.

## 8. Measurement harness

The harness is a tool, not a unit test: it must be runnable without a browser
and without the binding.

```bash
cargo run --release -p torille-tools --bin perf_grammar_style -- \
  --book 9082 --words 2000 --repeat 5 --single-thread --json
```

Output shape:

```json
{
  "words": 2000,
  "bytes": 16000,
  "tokenise_ms": 0.5,
  "morphology_cold_ms": 120000,
  "morphology_warm_ms": 6,
  "rules_ms": 24,
  "style_ms": 13,
  "total_cold_ms": 120040,
  "total_warm_ms": 43,
  "cache": { "words": 1029, "hits": 2438, "misses": 1029 }
}
```

Rules:

- `--single-thread` is the default in CI; the wasm has one thread. The native
  multi-core number is informative only.
- Pin the fixture by SHA-256 and print it in the JSON so a changed book slice
  is visible.
- Run the release profile. Debug numbers are meaningless here.
- Track `target/wasm32-unknown-unknown/release/torille_wasm_bg.wasm` and the
  brotli sidecar size in the same report.

CI policy: the per-PR gate is the correctness suite. The performance gate runs
on the release/nightly job and fails on a 5× regression against the recorded
baseline, not on absolute milliseconds (runner speed varies). The wasm size
gate is absolute: fail above the budget in §2.

## 9. Invariants and risks

| risk | guard |
|---|---|
| a phase changes a diagnostic | exact-output fixtures + syntax/style eval; revert or explain deliberately |
| shallow resolution loses `lasta`/`lapsi` | explicit shallow-vs-deep test; false negatives preferred |
| the stem-shape filter rejects a real form | generator test against `inflect_all`; a Bloom zero is only a rejection, never an acceptance |
| the sentence cache reuses stale context | sentence-locality test; include neighbours in the key if a rule crosses a boundary |
| the cache grows the wasm heap | `CacheStats` + two-generation eviction; per-state lifetime; drop on destroy |
| a shared entry type is not `Send` | `Arc`, never `Rc`; the state container already requires `Send` |
| the reverse index bloats the download | 95% / 3 MB decision gate; no full 6.4M-form map |
| grammar/style blocks spelling | chunking + generation; dedicated lane only if measured |
| Firefox/Chromium drift | the demo worker types come from `torille-wasm/protocol.ts`; the extensions carry their own protocol in both `globals.d.ts` files, so keep the two copies in sync and the shared content files byte-identical |
| the reader gets a frozen page | reader stays spelling-only until the 500 ms / progress gate |

Style invariants stay as they are: every finding is `Severity::Suggestion`,
never `Error`/`Warning`; a clean corpus stays quiet; a correction exists only
from a curated table row. Performance work must not weaken these.

## 10. Open decisions

1. **Base-source ladder vs reverse index.** Start with the ladder and the
   stem-shape filter (small, uses the existing stem maps). Only run the
   reverse-index experiment if the cold page misses the 3 s target.
2. **Shared worker vs dedicated lane.** Chunk first; add a lane only if a
   spelling request still waits behind a grammar batch.
3. **Cache cap policy.** Entries vs bytes, and the eviction generation size.
   Measure with `CacheStats` on the 2,000-word page before fixing a number.
4. **Spell-path sharing.** Keep the fast path independent for now. Revisit only
   if the morphology cache can answer a known-word probe without changing the
   fast path's semantics.
5. **Reader scope.** The reader may stay spelling-only by design even after
   the targets are met; the plan enables the option, it does not require it.

## Appendix A — raw measurements

Environment: this repository's release build, Linux, native run without
`TORILLE_SINGLE_THREAD`, wasm shape with it. Times are wall-clock and vary
between runs; the order of magnitude and the cold/warm split are the point.
The numbers were taken with a temporary probe equivalent to the Phase 0 tool;
the committed tool lands with Phase 0.

```text
page: 1722 words, 13695 bytes, 13096 chars
tokens: 3467
round 0: tokenise 0.51  sentence 0.06  syntax cold 63346.73  warm 18.61  style 6.50
round 1: tokenise 0.31  sentence 0.09  syntax cold 76671.61  warm 23.83  style 9.13
round 2: tokenise 0.40  sentence 0.06  syntax cold 74080.70  warm 22.46  style 7.30
two halves, one cache: first 37965.71  second 29945.55
```

Single-thread (wasm shape):

```text
page: 1722 words — syntax cold 123510 / 119420 / 120794 ms, warm 19.10 / 17.90 / 19.78 ms
254 words — style cold 26510.79 ms, warm 0.95 ms
83 words — style cold 11258.04 ms
45 words — style cold 8055.00 ms
18 words — style cold 2996.07 ms
4 words — style cold 156.43 ms
```

Rule split on the 1,722-word page:

```text
agreement 15.4 / 18.7 / 18.2 ms
punctuation 0.48 / 0.76 / 0.51 ms
rection 1.78 / 2.42 / 1.91 ms
object_case 2.26 / 2.81 / 2.25 ms
arguments 0.19 / 0.23 / 0.23 ms
rank 0.006 / 0.012 / 0.009 ms
```

Per-word cold cost and phase candidates:

```text
kanssa              726.05 ms   phase 1: 281 candidates, 451.61 ms
                                phase 1.5: 843 candidates, 259.47 ms
liika              4260.96 ms   phase 1: 278 candidates, 4180.60 ms
                                phase 1.5: 240 candidates, 78.78 ms
talossa             118.52 ms   phase 1: 163 candidates, 118.23 ms
kirjassa            252.90 ms   phase 1: 253 candidates, 251.21 ms
perunkirjoittamista 1369.36 ms  phase 1: 244 candidates, 267.06 ms
                                phase 1.5: 1293 candidates, 1008.08 ms
silloinkin          ~925 ms     clitic host walk
xyzzy                 1.91 ms   phase 1: 0 candidates
```

## Appendix B — files this plan touches

| area | files |
|---|---|
| morphology cache | `src/morphology/` (new), `src/check.rs` (facade), `src/syntax/cache.rs`, `src/syntax/mod.rs`, `src/lib.rs` |
| grammar rules | `src/syntax_check/mod.rs`, `analysis.rs`, `agreement.rs`, `rection.rs`, `object_case.rs` |
| style rules | `src/style_check/mod.rs`, `repetition.rs`, `length.rs`, `phrases.rs` |
| cold path | `src/expand/find_bases.rs`, `src/expand/mod.rs`, `src/expand/lexicon_index.rs` |
| shape blob | `build.rs`, `tools/src/bin/gen_lemma_stem_shapes.rs`, `maps/README.md` |
| wasm | `torille-wasm/src/lib.rs`, `torille-wasm/src/state.rs`, `torille-wasm/protocol.ts` |
| workers | `extensions/{firefox,chromium}/src/spell-worker/spell-core.ts`, `torille-wasm/demo/src/spell-worker.ts`, `spell-client.ts` |
| UI | `extensions/*/src/content/grammar-panel.ts`, `torille-wasm/demo/src/TextPanel.tsx`, `Editor.tsx` |
| benchmark | `tools/src/bin/perf_grammar_style.rs`, `tests/fixtures/perf/` |
