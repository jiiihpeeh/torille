# PLAN_GRAMMAR_STYLE_PERF — one morphology pass for grammar and style

Status: **Phase 0–1, 2 (cold path), 3 (warm rule pass), 4 (sentence cache) and 5 (chunking) landed; Phase 6/7 next** · 2026-11-14 · branch `syntax` · baseline `2e5f13c`

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
  with `head_inflections`. No new prefix list is added.
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
- **Shallow closed-class fix** — a pronoun base named by the cheap gate
  (`jossa` → `joka`) is verified across every sanalista row, so relative
  pronoun forms no longer fall to `Unknown`. The attribute rule now skips
  relative-pronoun lemmas, which removes three false `attribute_agreement`
  findings on the perf fixture (71 → 69 grammar diagnostics). The syntax
  corpus's `two_tuli` case is green again.

Measured on the committed fixture, single-thread (wasm shape):

| workload | before | now |
|---|---:|---:|
| 20-word cold | 3.0 s | 0.14 s |
| 96-word cold | 11.5 s | 0.76 s |
| 1,959-word cold morphology | ~120 s (1,722-word baseline) | 2.5 s |
| 1,959-word warm rule pass | 13.9 ms | 1.4 ms |
| 1,959-word warm style | 6–13 ms | 2.5 ms |
| 1,959-word warm grammar+style | ~33–41 ms | 6.9 ms |
| 17-word bound-form fixture, cold | 620 ms | 79 ms |

The full 1,959-word page is 3.08 s in the release wasm (Phase 7), and the
chunked worker shows first progress at ~160 ms (Phase 5).

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
3.08 s for the whole page, with first progress at ~160 ms.

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

### Phase 2 — cold path (3–5 days) — in progress

- §4.1 shallow/deep split and bounded walks.
- §4.2 base-source ladder: direct, stem map, closed spelling, clitic,
  possessive, compound.
- §4.3 stem-shape filter prototype and blob; generator + staleness test.
- §4.4 clitic host reuse.
- §4.6 cache metrics and two-generation eviction.

Acceptance: ≥95% of page tokens resolve without the head-1 walk; cold 20-word
≤ 100 ms (wasm shape); cold 2,000-word page ≤ 3 s; `lasta`/`lapsi` has an
explicit test; the 20-word fixture's diagnostics are unchanged.

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

### Phase 5 — wasm and worker (3 days) — partial

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
first progress **~160 ms**, final **69 grammar / 14 style** (identical to the
whole-text wasm check), and an edit during the run cancels to 0 diagnostics.
The longest batch is still ~200 ms, so the 50 ms per-task target is not met;
the extension workers, word budget and reader enablement remain.

Acceptance: no worker task over 50 ms; an edit during a check cancels within
one batch; spelling still answers while a grammar check runs; the demo and
both extensions build.

### Phase 6 — optional reverse index (3 days, decision gate)

- §4.5 coverage/size experiment.
- Ship only if it meets the 95% / 3 MB gate and improves the cold page to
  ≤ 500 ms; otherwise document the decision and stop.

Acceptance: either a `maps/syntax_forms.bin.gz` with a generator and coverage
report, or a written “not shipped” decision with the measured numbers.

### Phase 7 — reader enablement and docs (1 day)

- If a 2,000-word page checks in ≤ 500 ms, or the chunked progress UI is in
  place, enable the grammar/style command on the reader page; otherwise keep
  the reader spelling-only and document why.
- Update `docs/syntax_checker.md`, `docs/style_checker.md`, `README.md` and
  this plan's status.
- Bump the crate/demo and extension versions only if a surface changes.

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
