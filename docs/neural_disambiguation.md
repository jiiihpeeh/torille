# Neural morphological disambiguation — architecture audit

Status: **audit only, model not implemented** · 2026-11-15 ·
scope: `morphology`, `syntax`, `syntax_check`, `style_check`, native and
WASM deployment.

This document asks one question: where can a small neural model score
competing morphological readings in torille without replacing the parser,
duplicating the lexicon, or changing existing grammar behaviour by default?

The answer, in one sentence: **add a context-aware re-ranking pass between
`analyze_morphology` and `analyze_syntax`, store its decisions in a per-analysis
sidecar, and leave `WordInfo.readings` and every existing rule untouched.**

The rest of this document explains why, what must change to preserve the
alternative readings, which corpora and runtimes are realistic, and what the
smallest useful first milestone is.

## 0. Summary of the recommendation

| Question | Recommendation |
|---|---|
| Integration point | New `disambiguate(tokens, &MorphologyAnalysis)` pass after morphology, before syntax/style |
| What it outputs | One `ReadingDecision` per token: candidate scores, preferred reading index, morphological confidence, abstain flag |
| What it must not do | Mutate `WordInfo` in the shared surface cache; drop `Tags`; assign roles, clauses, subjects, objects or predicates; lower/suppress existing diagnostics in M0 |
| Morphological vs syntactic confidence | The model ranks readings only. Role confidence and diagnostic severity remain the syntax/checker layer’s decision. |
| Candidate identity | A reading index may map to zero, one or several lemmas; use a `CandidateView` adapter with explicit `Unavailable` provenance |
| Evaluation | Candidate coverage, morphological accuracy, syntactic-role accuracy and diagnostic precision are separate gates |
| M0 experiments | Compare morphology-only, local-context and sentence-context candidate MLPs on the same candidates and labels |
| First model | Candidate-conditioned one-hidden-layer MLP over local/context features (the M0 neural baseline) |
| Later model | 1-layer BiLSTM (128 hidden) or 2-layer compact Transformer (d=128, 4 heads); compare empirically |
| Runtime | Hand-written fixed-weight inference for M0; `tract` or `rten` for ONNX in M1; do not start with `ort`, Candle or Burn |
| Training data | UD Finnish-FTB (CC BY 4.0) first; UD Finnish-TDT (CC BY-SA 4.0) with licence review; OOD/PUD for test only |
| Evaluation | Frozen candidate-level test set + UD OOD/PUD + the existing adjudicated prose corpus |
| Rollout | Compile-time/runtime feature flag, off by default; heuristic fallback on low confidence or missing candidate |
| Smallest viable milestone | M0: sidecar + feature MLP + frozen ambiguity set + no behaviour change when the model is off |

## 1. Current architecture

### 1.1 Workspace and embedded data

The relevant crates are:

| path | role |
|---|---|
| `src/` (`torille`) | morphology, syntax, grammar and style logic |
| `tools/` | CLI drivers and the external Omorfi/Voikko harness |
| `bindings/` | PyO3 extension used by the Python harness |
| `torille-wasm/` | wasm-bindgen exports, textarea state, morphology codec |
| `torille-wasm-leptos/` | Leptos demo |
| `extensions/firefox/`, `extensions/chromium/` | browser surfaces and worker pool |

The engine embeds the Kotus sanalista (`data/nykysuomensanalista.csv`) and the
paradigm catalogue. It has no runtime data files. Morphology is generated from
the lexicon and catalogue; there is no statistical model in the current
pipeline.

### 1.2 Morphology

`src/morphology/mod.rs` is the layer below syntax:

- `WordInfo` (`src/morphology/mod.rs:124`) holds the lowercased key, chosen
  `lemma`, `verb_lemma`, `nominal_lemma`, `lemmas`, **`readings: Vec<Tags>`**,
  agreed `case`/`number`, `part_of_speech`, clitic, possessive, a possible
  derivation, and the `Depth` at which the surface was recognised.
- `MorphologyCache` (`:216`) is keyed by lowercased surface and capped at 4096
  entries. It stores `Arc<WordInfo>` so a long document does not copy readings.
- `MorphologyAnalysis` (`:364`) is a token-aligned vector of
  `Option<Arc<WordInfo>>` plus sentence spans.
- `analyze_morphology` (`:379`) tokenises, resolves every word through the
  cache, applies the proper-name overlay, and applies the numeral-governed
  partitive upgrade.
- `resolve_word` (`:629`) is the shallow ladder: direct lemma, stem map,
  clitic/possessive host, connegative table, negation. `resolve_word_slow`
  (`:1109`) runs the head/tail and prefix walks only when a rule asks for the
  deep result.

The shallow/deep split matters for a model: a shallow `WordInfo` may miss a
reading that the deep walk would find. A reranker can only score candidates
that are present. Candidate recall is therefore a first-class metric, not an
implementation detail.

`WordInfo` is surface-level, not context-level. Two occurrences of `tuli` share
the same cached readings regardless of whether one is the noun “fire” and the
other the past tense of `tulla`. This is deliberate and must remain true.

### 1.3 Tags, Forms and confidence

`src/tags.rs`:

- `Tags` is a compact, `Copy` reading: optional `part_of_speech`, `case`,
  `number`, `person`, `tense`, `mood`, `voice`, `possessive`, `clitic`, plus
  `polarity`.
- `Form` (`:892`) holds a surface, `Vec<Tags>`, and a `Confidence`
  (`Guessed` < `Repaired` < `Plain`).
- `add_reading` (`:36`) collapses readings that subsume one another. This keeps
  the reading set small, but it means a form’s **source lemma and generation
  confidence are not retained per reading** once the reading is copied into
  `WordInfo`.
- `agreed` (`:57`) and `Tags::unite` (`:815`) collapse dimensions where the
  readings disagree. They are useful for display and indexing, but they are
  exactly the wrong input for a candidate reranker: a missing dimension means
  “readings disagree”, not “unknown”.
- `Tags::matches` (`:844`) treats a missing dimension as a wildcard. Corrections
  rely on this; a model must not reuse that wildcard semantics when scoring
  candidates.

### 1.4 Syntax

`src/syntax/mod.rs`:

- `TokenSyntax` (`:129`) holds `key`, `role`, `confidence`, clause index,
  governor, licensed `case`, **one chosen `lemma`**, `lemmas` (all bases), and
  **`readings` (all Tags)**.
- `SyntaxAnalysis` (`:173`) holds sentence spans, clauses, lists and
  token-aligned `TokenSyntax`.
- `analyze_syntax` (`:213`) is pure: it consumes an existing
  `MorphologyAnalysis` and never searches the lexicon.
- `analyze_syntax_with_contexts` (`:222`) copies `info.readings`,
  `info.lemmas`, `info.lemma`, and `info.verb_lemma`/`nominal_lemma` into
  `TokenSyntax`, then calls `classify` and `resolve_clause`.

The choices made here are heuristic:

- `src/syntax/roles.rs:classify` (`:18`) decides predicate/adverbial/unknown
  from the **set** of readings. It does not have a probability.
- `choose_predicate` picks the first likely finite predicate, then a weak one,
  then an infinitive, then any predicate.
- `resolve_clause` chooses subject, object, complement and modifiers with
  ordered heuristics (`nominal_has_case`, coordination, numeral complements,
  adposition phrases). It reads **all** readings, not a top-1.
- `TokenSyntax.lemma` is a single chosen lemma. Alternatives remain in
  `lemmas` and `readings`, but a rule that only looks at `lemma` sees one
  answer.

This is the seam a neural model should influence. It should not write a new
parser; it should make the existing heuristic selectors better informed.

### 1.5 Grammar and style checkers

`syntax_check` (`src/syntax_check/mod.rs`) consumes `SyntaxAnalysis`:

- `check_analysis` (`:67`) runs agreement, connegative, numeral, punctuation,
  rection, object-case and argument rules.
- Rules read `TokenSyntax.readings`, `case`, `role`, `clause`, `head`,
  `predicate`, `subject`, `object`, `complement`, and the token spans.
- `sentence_cache::check` (`src/syntax_check/sentence_cache.rs:88`) re-analyses
  only changed sentences. It runs `analyze_morphology` on the sentence, then
  `analyze_syntax_with_contexts`, then the sentence-local rules.

`style_check` (`src/style_check/mod.rs`) consumes `MorphologyAnalysis`
directly:

- `check_tokens_with_morphology` (`:72`) runs phrase tables, repetition and
  length.
- Repetition uses `WordInfo.readings` for content-word filtering and
  `WordInfo.lemma` for the repeated lemma (`src/style_check/repetition.rs:90`).
- Style never builds clauses or roles.

The key consequence: a model that changes only `TokenSyntax` will affect
grammar but not style. A model that mutates `WordInfo.lemma` would affect style
but poison the surface cache. The correct design is a shared per-analysis
decision layer that both layers may consume, with style staying on the old
heuristic until the model is evaluated.

### 1.6 Combined pass and WASM

`src/check.rs`:

- `check_all_with_cache` (`:33`) runs `analyze_morphology` once and then
  `check_all_tokens`.
- `check_all_tokens` (`:40`) runs `analyze_syntax`, `check_analysis`, and
  `style_check::check_tokens_with_morphology` from the same morphology.

WASM (`torille-wasm/src/lib.rs`):

- `analyze_syntax`, `check_syntax`, `check_style`, and their `_with_state`
  variants run stateless or through a textarea cache.
- `check_grammar_style_with_state` (`:624`) and
  `check_grammar_style_from_morphology` (`:555`) are the combined entry points.
- `TextAreaState` (`torille-wasm/src/state.rs:67`) owns `syntax_cache`
  (`MorphologyCache`) and `sentence_cache` (`SentenceCache`).
- The Firefox morphology pool (`extensions/firefox/src/spell-worker/morphology-worker.ts`)
  resolves disjoint surface groups through `morphology_chunk`; the owning
  worker seeds the rows and runs syntax/style once. Chromium’s MV3 service
  worker uses the chunked single-thread path.
- `src/morphology/codec.rs` is the worker transport (`TMOR`, version 1). It
  serialises the whole `WordInfo`, including every reading.

The pool is surface-partitioned. It has no sentence context. A neural reranker
cannot live in the morphology workers; it must run on the owning thread after
the rows are merged, before syntax/style.

### 1.7 Baseline

The audit baseline is recorded in §13. Current relevant numbers from the
repository:

- native/wasm cold page: ~2.1–2.3 s native, ~3.0 s wasm single-thread,
  ~1.4 s wasm with the 4-worker pool;
- warm grammar+style rule pass: ~6.9 ms on the 1,959-word fixture;
- wasm artifact: ~31 MB raw (roughly 3–4 MB gzipped in the shipped build);
- syntax evaluation: 154 clean rows / 61 cases, all emitted kinds precision
  and recall 1.00;
- adjudicated prose seed: 381 correct words, 17 known false positives
  (44.6 per 1,000 words, tracked as a seed, not a stable rate).

## 2. Morphological reading lifecycle

This is the exact path a reading takes today, and the places where a model
could act.

```
text
  → tokenise::tokenise
  → analyze_morphology(tokens, MorphologyCache)
       → MorphologyCache::resolve / resolve_deep
            → resolve_word_fast / resolve_word_slow
            → find_bases_for_analysis
            → expand::form_readings / forms_of
            → WordInfo { readings, lemmas, case, number, lemma, ... }
       → proper-name overlay
       → numeral-governed partitive upgrade
  → MorphologyAnalysis { sentences, tokens: Vec<Option<Arc<WordInfo>>> }
  → analyze_syntax_with_contexts
       → TokenSyntax { readings, lemmas, lemma, case, role, ... }
       → classify / resolve_clause
  → syntax_check::check_analysis
  → style_check::check_tokens_with_morphology
  → diagnostics
```

### 2.1 Stage by stage

| stage | data | what is decided | what is preserved |
|---|---|---|---|
| tokenise | `Token` | word vs separator, byte spans | exact surface, case |
| surface resolution | `WordInfo` | candidate readings and preferred lemma fields | all `Tags` in `readings`, all `lemmas` |
| context upgrades | `WordInfo` | proper name, numeral partitive | readings are replaced only by a deeper resolved `WordInfo`; the token-level entry is updated |
| syntax | `TokenSyntax` | role, clause, one chosen lemma, licensed case | `readings` and `lemmas` copied; chosen lemma is one of them |
| grammar | `SyntaxDiagnostic` | correctness findings | diagnostics, corrections, spans |
| style | `StyleDiagnostic` | optional style findings | no change to morphology |

### 2.2 Where alternative readings are already preserved

- `WordInfo.readings` keeps every `Tags` reading the resolver produced.
- `WordInfo.lemmas` keeps every base lemma that produced a reading.
- `TokenSyntax.readings` and `TokenSyntax.lemmas` keep the alternatives after
  syntax has chosen a role.
- `Form.readings` keeps multiple readings for a generated surface before the
  morphology reader collapses them into `WordInfo`.
- `Tags::matches` and `Form::readings_matching` exist for callers that have
  context.

### 2.3 Where alternatives are currently collapsed or lost

These are the changes any model work must account for.

1. **`WordInfo.readings` stores only `Tags`.** `expand::form_readings` returns
   `Vec<Tags>` (`src/expand/mod.rs:601`); the `Confidence` and source lemma on
   the underlying `Form` are not retained per reading. A model that wants
   “this reading came from a `Repaired` form” or “this reading came from lemma
   X” cannot see it today.
2. **`WordInfo.lemma` / `verb_lemma` / `nominal_lemma` are singletons.** They
   are preferences, not alternatives. `TokenSyntax.lemma` is also one string.
   A model must not be asked to “pick the lemma” by overwriting these without a
   preserved candidate list.
3. **`WordInfo.case` / `number` are agreed values.** They are `None` when the
   readings disagree (`fill_case_number`, `:1228`). A candidate scorer must
   read the individual `Tags`, not the agreed summary.
4. **`add_reading` subsumes readings.** `add_reading` (`src/tags.rs:36`)
   removes a reading subsumed by a more specific one. This is correct for the
   current consumers, but it means candidate identity is not stable across
   merges unless the surviving reading is given a stable index.
5. **`agreed` / `Tags::unite` collapse dimensions.** They are for display and
   indexing; a model must use the individual readings.
6. **`MorphologyCache` is surface-keyed and shared.** Any context-dependent
   score or preferred reading stored in `WordInfo` would leak from one
   occurrence to another. This is the single most important reason for a
   sidecar decision table.
7. **`TokenSyntax` has public fields.** Adding `reading_scores` or
   `preferred_reading` directly is a public API break for callers that build
   the struct literally. Prefer a sidecar/accessor or `#[non_exhaustive]` when
   the time comes.
8. **`MorphologyAnalysis` has public fields.** Adding a decisions field is also
   a public API break. Return a separate `Disambiguation` object, or add a
   constructor/accessor rather than a public field in the first version.
9. **`morphology::codec` version 1.** If per-reading source/confidence is added
   to `WordInfo`, the worker transport must bump `VERSION` and encode the new
   fields; otherwise pool workers and the owning thread disagree about the
   candidate representation.
10. **`sentence_cache` caches diagnostics.** If a model changes a sentence’s
    decisions, the cache key must include the model identity/version (and the
    feature flag state). A model update must invalidate the sentence cache.
11. **Correction paths re-run base searches.** `rection::correction_bases`
    (`src/syntax_check/rection.rs`) and the object-case ambiguity helper can
    look up alternative lemmas again. A model’s preferred reading and the
    correction’s candidate base must come from the same candidate list, or the
    diagnostic and the correction can disagree.
12. **Style uses `WordInfo.lemma`.** It will not see a `TokenSyntax`-only
    decision. Either pass the sidecar to style later or keep style on the
    heuristic path for v1.

### 2.4 Reading identity and candidate-to-lemma provenance

A model output must identify a candidate by **token index + reading index**,
not by surface or lemma. The current `WordInfo` is shared per surface, so the
reading vector is identical for the same lowercased surface in one analysis,
but the model’s **choice** is per occurrence. The decision table must be
token-indexed.

The reading index does **not** carry a reliable lemma. `WordInfo.readings` is
a `Vec<Tags>`; `WordInfo.lemmas` is a token-level list of every base that
produced a reading. There is no per-reading source pointer. Consequences:

- one reading can be produced by more than one lemma (`tuli` as noun `tuli`
  and as a form of `tulla`);
- one lemma can produce more than one reading of the same surface;
- a surface can be a clitic/possessive/compound form whose source is not the
  chosen `lemma`;
- `TokenSyntax.lemma` is a **preference**, not proof of the reading’s source.

Do not assume that `preferred` identifies a unique lemma. Use an adapter:

```rust
pub struct CandidateView<'a> {
    pub token_index: usize,
    pub reading_index: usize,
    pub tags: Tags,
    /// Known(lemmas) may contain more than one lemma; Unavailable means the
    /// engine cannot attribute this reading without re-inflection.
    pub lemma_provenance: LemmaProvenance<'a>,
    /// WordInfo does not retain Form::confidence per reading today.
    pub source_confidence: Provenance<Confidence>,
}

pub enum LemmaProvenance<'a> {
    Known(Vec<&'a str>), // one or more; never assume len() == 1
    Unavailable { reason: &'static str },
}

pub enum Provenance<T> {
    Known(T),
    Unavailable { reason: &'static str },
}
```

**Attribution adapter.** For a single-lemma token, `TokenSyntax.lemmas` is
already a reliable one-element source. For a multi-lemma token, the Phase 1
harness re-runs `expand::form_readings` for each candidate lemma and records
which lemmas emit the exact `Tags`. If none do, `LemmaProvenance::Unavailable`
is reported; it is **not** a model error. The production path should later
store provenance during reading construction rather than re-inflecting, but
Phase 1 must not add a public API or mutate the cache.

**Matching rule.** An exact candidate match requires:

- the gold mapping status is `Exact`;
- the candidate’s supported `Tags` match the gold tags;
- the gold lemma is a member of `LemmaProvenance::Known`.

If provenance is `Unavailable`, the match can only be `Partial`, and the
report records `provenance_unavailable`. A preferred reading with ambiguous
provenance is not a licence to pick a lemma.

The stable identity key remains:

```text
(token_index, reading_index, mapping_version)
```

and never `(surface, lemma)`.

### 2.5 Morphological confidence is not syntactic confidence

The model’s confidence is **morphological**: how sure it is that reading *r*
is the intended reading of token *t*. It is not:

- a role confidence (`Subject` vs `Object` vs `Adverbial`);
- a clause-structure confidence;
- a diagnostic severity;
- permission to lower or suppress an existing grammar finding.

`classify`, `choose_predicate`, `resolve_clause`, `agreement`, `rection`,
`object_case`, `arguments` and the punctuation rules remain the owners of
syntax and correctness. The model may give those selectors a better preferred
reading; it must not emit roles or rewrite clauses.

In M0 the model’s confidence is reported separately from
`DiagnosticConfidence`. The first integration milestone must **not** use model
uncertainty to downgrade an `Error` or suppress a diagnostic. The effect of
changed reading preferences on diagnostics is measured as an experiment, not
applied as a policy.

## 3. Candidate integration points

### 3.1 Rejected points

| point | why not |
|---|---|
| Inside `MorphologyCache::resolve` | Cache key is the lowercased surface; no sentence context. A decision would be reused for every occurrence and poison later analyses. |
| Inside `resolve_word_fast` / `resolve_word_slow` | These produce candidates, not context choices. Adding context there duplicates the syntax stage and breaks the surface cache. |
| Inside `analyze_morphology` per token | The whole sentence is available, but `WordInfo` is still cached per surface; writing scores there is the same poisoning problem. A separate decision pass is safer. |
| Inside `syntax_check` rules | Each rule would need its own model call or its own selection logic. That is exactly the “second parser” the architecture is trying to avoid. |
| After diagnostics | Too late: roles, cases and corrections have already been chosen. |
| In the morphology pool workers | Workers see surface groups, not sentences. They cannot score context. |

### 3.2 Recommended point

**After `analyze_morphology` and before `analyze_syntax` /
`style_check::check_tokens_with_morphology`.**

That pass has:

- the full token stream and sentence spans;
- the complete `MorphologyAnalysis`;
- every candidate reading, without needing to search the lexicon again;
- no clauses or roles yet, so it cannot duplicate the parser;
- a natural place to expose a sidecar that syntax and style may consume.

Call sites to wire:

| call site | current code | change |
|---|---|---|
| `check_all_with_cache` / `check_all_tokens` | `src/check.rs:33–50` | `analyze_morphology` → `disambiguate` → `analyze_syntax`/style |
| `syntax::analyze` / `analyze_with_cache` | `src/syntax/mod.rs:190–210` | add an internal `analyze_syntax_with_decisions` or an optional decision parameter |
| `syntax_check::check_with_cache` | `src/syntax_check/mod.rs:53–62` | consume the decided path |
| `SentenceCache::check` | `src/syntax_check/sentence_cache.rs:88–150` | disambiguate the sentence slice before `analyze_syntax_with_contexts` |
| `style_check::check_with_cache` | `src/style_check/mod.rs:56–80` | optional: pass decisions to repetition; otherwise keep heuristic |
| wasm `check_grammar_style_with_state` | `torille-wasm/src/lib.rs:624` | run model on owner after morphology, before sentence cache |
| wasm `check_grammar_style_from_morphology` | `torille-wasm/src/lib.rs:555` | run after seeding pooled rows |
| wasm `analyze_syntax_with_state` / `check_syntax_with_state` | `:234`, `:488` | route through the decided analysis |
| wasm `analyze_syntax` / `check_syntax` | `:222`, `:431` | stateless path uses the same internal decided analysis |
| Firefox morphology workers | `morphology-worker.ts` | no model; only surface rows |

### 3.3 Proposed seam type

Keep `MorphologyAnalysis` unchanged. Add a new, per-analysis type:

```rust
pub struct Disambiguation {
    /// One entry per input token; separators and single-candidate words are None.
    pub decisions: Vec<Option<ReadingDecision>>,
    pub model_version: u32,
}

pub struct ReadingDecision {
    /// Index into WordInfo.readings. Never a new reading.
    pub preferred: usize,
    /// One score per reading, in the same order as WordInfo.readings.
    pub scores: Vec<f32>,
    /// Margin and entropy are for abstention and diagnostic confidence.
    pub margin: f32,
    pub entropy: f32,
    /// True when the model declines to choose.
    pub abstain: bool,
}
```

`disambiguate` takes `&[Token]` and `&MorphologyAnalysis`, never `&mut
MorphologyCache`. It may return `None` for a token with one candidate, an
unknown word, or an empty reading set.

A consumer reads `decision.preferred` only where it needs a context choice;
all other code continues to see `WordInfo.readings`/`TokenSyntax.readings`.

### 3.4 How the existing selectors use the decision

Do not replace `classify` or `resolve_clause`. Add a narrow preference:

- `classify`: if there is a preferred reading, prefer its part of speech for
  the role family, but keep the full reading set for ambiguity fallback.
  The model still does not assign a role.
- `choose_predicate`: prefer a preferred finite reading over a first-reading
  heuristic; the selector, not the model, decides that a predicate exists.
- `subject_of` / object selection: prefer a preferred nominal case when the
  current reading-consistent helpers have more than one viable candidate; the
  selector still owns subject/object/complement assignment.
- `TokenSyntax.lemma`: set from the preferred reading **only when**
  `CandidateView.lemma_provenance` is `Known` with exactly one lemma. If
  provenance has several lemmas or is `Unavailable`, keep the existing chosen
  lemma and record the ambiguity. A preferred reading is not proof of a unique
  lemma.
- `syntax_check` rules: unchanged. **M0 must not lower, raise or suppress a
  diagnostic because of model confidence.** The first integration milestone
  keeps the existing severity and diagnostic emission exactly as it is. The
  effect of changed reading preferences on diagnostics is a separate
  experiment, not an activation policy.

No rule should call the model. The model is one pass, before the rules.

## 4. Proposed model input/output schema

### 4.1 Inputs

The model is a **candidate reranker**, not a tagger. It sees the sentence and
the candidate set for each token.

The input builder must use the `CandidateView` adapter from §2.4. It must not
read `TokenSyntax.lemma` as if it were the source of every reading and must not
invent a `source_confidence` that `WordInfo` does not carry.

Per token:

| field | source | notes |
|---|---|---|
| `token_index` | token stream | stable for the analysis |
| `surface` / `lower` | `Token` | casing is a signal |
| `position`, `sentence_length` | token/sentence pass | relative position |
| `prev_surface`, `next_surface` | neighbours | local context |
| `capitalized` | `Token` | name signal |
| `candidate_count` | `WordInfo.readings.len()` | skip K<=1 |
| `depth` | `WordInfo.depth` | direct vs compound vs proper name |

Per candidate:

| field | source | notes |
|---|---|---|
| `reading_index` | position in `WordInfo.readings` | output label |
| `lemma_provenance` | `CandidateView` adapter | `Known(Vec<lemma>)` or `Unavailable`; may be several lemmas |
| `part_of_speech` | `Tags` | categorical |
| `case`, `number` | `Tags` | categorical |
| `person`, `tense`, `mood`, `voice` | `Tags` | categorical |
| `polarity` | `Tags` | categorical |
| `possessive`, `clitic` | `Tags` | categorical |
| `source_confidence` | `WordInfo` today | **always Unavailable** in Phase 1; do not infer it |
| `source_depth` | resolver depth | token-level today |

### 4.2 Tensor shape

A compact first model can use:

```text
token_ids        [batch, seq_len]
char_ids         [batch, seq_len, max_char]
token_features   [batch, seq_len, F_token]
candidate_feats  [batch, seq_len, max_candidates, F_candidate]
candidate_mask   [batch, seq_len, max_candidates]
```

Output:

```text
logits           [batch, seq_len, max_candidates]
scores           [batch, seq_len, max_candidates]  # softmax over real candidates
preferred        [batch, seq_len]                  # argmax or -1 for abstain
margin           [batch, seq_len]
entropy          [batch, seq_len]
```

The model must emit `-1`/`None` for tokens with no candidate or where it
abstains. It must never emit a candidate that is not in the input list.

### 4.3 JSON shape for fixtures and debugging

```json
{
  "tokens": [
    {
      "surface": "tuli",
      "lower": "tuli",
      "sentence": 0,
      "position": 3,
      "candidates": [
        {
          "reading_index": 0,
          "lemma_provenance": {"known": ["tuli"]},
          "pos": "noun",
          "case": "nominative",
          "number": "singular"
        },
        {
          "reading_index": 1,
          "lemma_provenance": {"known": ["tulla"]},
          "pos": "verb",
          "mood": "indicative",
          "tense": "past",
          "voice": "active",
          "polarity": "affirmative"
        }
      ]
    }
  ],
  "decision": {
    "model": "m0-mlp",
    "version": 1,
    "preferred": 1,
    "scores": [0.12, 0.88],
    "margin": 0.76,
    "entropy": 0.21
  }
}
```

### 4.4 Calibration and abstention

The model should output calibrated probabilities with temperature scaling on a
held-out dev set. The consumer uses:

- `preferred` when the margin is above a threshold and the model is not
  out-of-domain;
- `abstain` when entropy is high, candidate coverage is low, the surface is
  unknown, or the model version is unsupported.

The model confidence is **morphological**. In M0 it only decides whether to
use a preferred reading or abstain. It does not change a diagnostic’s severity,
does not lower an `Error` and does not suppress a finding. That is a separate
experiment after the reading-preference effect has been measured.

The threshold is a product decision; it must be frozen with the model and
tested against the adjudicated prose corpus.

## 5. Dataset and annotation strategy

### 5.1 Candidate corpora

Sizes and licences below are from the public treebank/resource pages checked
for this audit. “Independent” means not produced by torille and not simply a
copy of its outputs.

| corpus | size | annotation | licence | use | caveats |
|---|---:|---|---|---|---|
| UD Finnish-TDT | ~202k tokens, ~15k sentences | syntax manual; morphology disambiguated from OMorfi + ML, converted to UD | CC BY-SA 4.0 | train/dev/test | Morphology is not purely manual; share-alike review needed for model weights |
| UD Finnish-FTB | ~159k words, ~18.7k sentences | manual native morphology/syntax, converted to UD | CC BY 4.0 | train/dev/test | Grammar-example domain; best permissive licence |
| UD Finnish-OOD | ~19.4k tokens, 2,122 sentences | manual native UD | CC BY-SA 4.0 | test only | Out-of-domain web/clinical/social/poetry; not for training |
| UD Finnish-PUD | ~15.8k tokens, 1,000 sentences | manual native UD | CC BY-SA 4.0 | test only | Parallel news/wiki test set |
| ArkiSyn | ~279k word tokens, ~44.6k sentences | manual morphosyntax | CC BY-ND 4.0 | evaluation only, with legal review | No derivatives; contains personal data; do not train |
| Finnish Dialect Corpus | ~1.08M words, ~70k sentences | manual morphosyntax | CC BY-ND 4.0 | dialect evaluation only, with permission | No derivatives; personal data; not standard Finnish |
| DMA | ~404k dialect clauses | morphologically coded dialect | CC BY 4.0 | robustness/evaluation | Dialect, clause-level, not UD; mapping needed |
| LAS2 | ~288k tokens, ~22k sentences | manual morphosyntax | CLARIN RES + NC + no redistribution | not for shipping | Research plan, personal data, non-commercial |
| Finnish Internet Parsebank | ~6.2B tokens | automatic morphology/syntax | research distribution; check terms | unlabelled/weak supervision only | Not gold; underlying text copyright; research-only |
| UniMorph Finnish | ~2.49M forms, 57k lemmas | lexicon paradigms | Creative Commons (check exact version) | candidate coverage, lemma embeddings | No sentence context |
| Hutmegs | ~1.4M Finnish words | morphological segmentation gold | FINTWOL component is commercial | do not use | Not free for a shipped model |
| Gutenberg / Wikipedia | large | unlabelled | public domain / CC BY-SA | unlabelled pretraining | No gold reading labels |

### 5.2 Independence and label quality

- **UD Finnish-FTB** is the cleanest starting point: manual native annotation,
  permissive CC BY 4.0, but its domain is grammar examples rather than general
  prose.
- **UD Finnish-TDT** is the largest broad-coverage resource, but the TDT paper
  describes the morphological layer as automatically disambiguated from
  OMorfi using syntactic features, not fully manually annotated morphology.
  It is independent of torille but not independent of OMorfi. Treat it as
  silver-plus for fine-grained readings, not as a pure manual gold.
- **UD Finnish-OOD** and **PUD** are manual native UD; keep them as frozen
  external tests. Do not train on them.
- **ArkiSyn**, **Dialect Corpus**, **LAS2** are manually annotated but their
  licences (ND/RES/NC) make them unsuitable for model training or shipping
  without an explicit agreement. They can inform a manually adjudicated
  evaluation set only after legal review.
- **Parsebank** is automatically annotated and research-distributed. It is not
  a gold standard and should not be the first training source.
- **Hutmegs** is segmentation, not contextual disambiguation, and the Finnish
  component is commercial. Exclude.

### 5.3 Mapping UD to torille `Tags`

A candidate label must be matched against torille’s candidate set. The mapping
must be explicit and versioned:

| UD | torille |
|---|---|
| `UPOS=NOUN/ADJ/PRON/NUM/VERB/AUX/ADV/ADP/...` | `PartOfSpeech` |
| `Case=Nom/Gen/Par/Ill/Ine/Ela/Ade/Abl/All/Ess/Tra/Abe/Com/Ins/Acc` | `Case` |
| `Number=Sing/Plur` | `Number` |
| `Person=1/2/3` | `Person`; `Person=0` -> `None` |
| `Tense=Pres/Past` | `Tense` |
| `Mood=Ind/Cnd/Imp/Pot` | `Mood` |
| `Voice=Act/Pass` | `Voice` |
| `Polarity=Neg` / `Connegative=Yes` | `Polarity::Negative` |
| `Poss` / `Number[psor]` / `Person[psor]` | `Possessive` |
| `Clitic` | `Clitic` |
| `VerbForm=Fin/Inf/Part` | mood/tense/infinitive mapping; not one-to-one |
| `Degree`, `NumType`, `PronType`, `Derivation`, `PartForm`, `InfForm` | not represented in `Tags` today |

The mapper must be a separate, tested module. A relaxed match on the fields
that `Tags` actually stores is safer than inventing dimensions. Record every
unmatched gold reading as a **candidate-coverage miss**, not as a model error.

### 5.4 Candidate coverage, mapping outcomes and evaluation gates

Before training, run a coverage harness:

1. Run torille morphology/syntax on the UD sentence (the candidate set plus
   the token-level lemma list).
2. Map the UD gold reading to the `Tags` subset above using a versioned
   mapper.
3. Attribute each reading to its lemma(s) with the `CandidateView` adapter.
4. Classify the token into exactly one outcome:

| outcome | meaning | counted as model error? |
|---|---|---|
| `exact` | gold mapping is exact; one candidate matches the supported tags; gold lemma is in `LemmaProvenance::Known` | yes |
| `partial` | gold mapping is partial, or lemma provenance is unavailable; a candidate matches all supported tags | yes, but reported separately |
| `ambiguous` | more than one candidate matches, or the UD feature mapping itself is ambiguous (e.g. `Poss=Yes` without person/number) | no; separate bucket |
| `missing` | no candidate matches the supported gold fields | yes; candidate-generation/recall gap |
| `unavailable` | the gold reading needs a feature torille does not represent (`Degree`, `NumType`, `PronType`, `Derivation`, `PartForm`, `InfForm`, unsupported POS, missing lemma) | **never**; excluded from the model-error denominator |

The report must use the categories explicitly:

- **candidate coverage** = `(exact + partial) / (exact + partial + missing)`
  over available gold readings; `ambiguous` and `unavailable` are reported
  separately, not folded into misses;
- **morphological accuracy** = top-1 reading accuracy on `exact` and `partial`
  tokens, per POS/case/depth, with calibration and abstention;
- **syntactic-role accuracy** = subject/object/predicate/adverbial role
  accuracy against UD, measured only after the model changes role preferences;
  this is not implied by morphological top-1;
- **diagnostic precision/recall** = the adjudicated-prose gate, per
  `DiagnosticKind` and per severity; a candidate top-1 gain is not product
  quality if it makes the checker noisier.

Never count an `unavailable` gold reading as a model prediction error. Never
report a headline “accuracy” that mixes `exact`, `partial`, `ambiguous` and
`missing` without the denominator.

The report is grouped by:

- UD POS (`UPOS`);
- gold case (`Case` or `none`);
- morphology depth (`Depth::label()`);
- mapping status (`exact/partial/ambiguous/unavailable`);
- candidate outcome (`exact/partial/ambiguous/missing`);
- `provenance_unavailable` count.

Fix only clear candidate-generation gaps in the engine; do not add gold-only
candidates to the model input.

### 5.5 M0 experiments

The first neural milestone is not one architecture but a controlled comparison
on the same candidate set, labels and splits:

1. **Morphology-only MLP** — candidate features only; no neighbouring tokens.
2. **Local-context MLP** — candidate features plus a fixed window (for example
   two tokens left/right) and sentence position.
3. **Sentence-context MLP** — candidate features plus a sentence encoder
   (mean-pooled or a small recurrent/attention encoder).

All three use the same input candidates and the same gold labels. Compare:

- morphological top-1 accuracy on `exact`/`partial` tokens;
- calibration and abstention rate;
- syntactic-role accuracy after applying the preference;
- diagnostic precision/recall on the adjudicated prose;
- latency and model size.

Only proceed to BiLSTM/Transformer if sentence context shows a measurable gain
over the local-context MLP. If it does not, the smaller M0 model is the
answer.

### 5.6 Training and test splits

- Train on UD Finnish-FTB train (and optionally TDT train after licence review).
- Tune on FTB dev and a frozen manually adjudicated hard set.
- Test on FTB test, TDT dev/test if used, Finnish-OOD, Finnish-PUD, and the
  existing adjudicated prose corpus.
- Split by document, not sentence, to avoid near-duplicate leakage.
- Keep a final **never-touched** test set for the first release decision.

### 5.7 Manual hard-ambiguity set

UD only gives one gold reading, not a ranked candidate set. Add a small manual
set focused on the ambiguities that actually break torille:

- `tuli` (noun vs past verb), `kirjaa` (noun partitive vs verb), `sen`/`ne`
  (pronoun vs adverb), `joka` (determiner vs relative), `ilman`/`ennen`
  (preposition vs noun/adverb), `kuusi` (numeral vs spruce), `kaksi`/`kaksi`
  numeral government, `hän`/`hänen`, `ole`/`olen`/`on`, `mene`/`menen`.
- 500–1,000 tokens is enough for the first evaluation; annotate the **preferred
  reading index**, a confidence/acceptability judgement, and an `abstain`
  option.
- Double-annotate a subset; record agreement. If annotators cannot agree, the
  model should abstain, not guess.

### 5.8 Licence strategy

The engine is MIT/Apache-2.0 and the repository carefully separates GPL
oracle tools. A trained model adds a new licensing question:

- CC BY 4.0 (FTB) requires attribution; model cards/weights must carry it.
- CC BY-SA 4.0 (TDT/OOD/PUD) may make model weights an adapted work; obtain a
  legal review before shipping a model trained on it. Safer first release:
  train on FTB only, evaluate on TDT/OOD/PUD.
- CC BY-ND (ArkiSyn, Dialect Corpus) prohibits sharing adapted material; do
  not train or ship a derivative without permission.
- CLARIN RES/NC (LAS2) prohibits redistribution and commercial use; do not use
  for a shipped model.
- Research-only Parsebank is not a commercial training source.
- Do not train on OMorfi/Voikko/Kotus-generated labels; that would recreate
  the existing oracle and undermine independent evaluation.

The model artifact should live outside `maps/` unless it is treated like a
published map. A `models/` directory with a model card, source corpus list,
licence notice, checksum, and version is the clean shape. If the model is
derived from CC BY-SA data, its licence must be stated separately from the
engine’s MIT/Apache licence.

## 6. Architecture comparison

| architecture | parameters (rough) | context | data need | native latency | wasm | size | risk |
|---|---:|---|---:|---:|---|---:|---|
| current heuristic | 0 | set of readings | 0 | baseline | baseline | 0 | no probabilities |
| candidate MLP (M0) | 10k–100k | local window + candidate features | small | <1 ms / 1k tokens | negligible | <100 KB | weak long context |
| BiLSTM + candidate scorer | 0.3–2M | full sentence, sequential | 100k–1M tokens | 1–5 ms / sentence | 5–20 ms / sentence | 0.3–2 MB int8 | sequential, data-hungry |
| compact Transformer (2–4 layers, d=128) | 0.5–4M | full sentence, parallel | 200k–2M tokens | 1–5 ms / sentence | 5–25 ms / sentence | 0.5–4 MB int8 | more parameters, attention memory |
| char CNN + candidate scorer | 0.2–1M | local + morphology | 100k+ | 1–3 ms / sentence | 5–15 ms | 0.2–1 MB | weaker syntax |
| fine-tuned FinBERT/base | 100M+ | full sentence | huge | 10–50 ms/ sentence native | not viable | 100–400 MB | size, latency, licence |

### 6.1 BiLSTM

Strengths:

- data-efficient for a small treebank;
- natural fit for a sequence of tokens and a per-token candidate set;
- small parameter count at 128 hidden units;
- easy to export as a fixed graph or implement manually.

Weaknesses:

- sequential inference; wasm per-step overhead;
- long sentences need truncation/windowing;
- less parallel than a Transformer.

Feasibility: **high**. A 1-layer bidirectional LSTM with word/char embeddings
and a candidate MLP is a realistic first neural model. It fits the 1–2 MB
budget and is trainable on FTB/TDT.

### 6.2 Compact Transformer

Strengths:

- parallel over tokens; good matrix-op utilization in Rust/wasm;
- better long-distance context in principle;
- easy to export through ONNX/tract/rten;
- can share a tokenizer with the existing token stream (no new tokenizer
  needed for word-level features; char features can be a small CNN).

Weaknesses:

- more parameters for the same data;
- attention memory and shape handling in wasm;
- overfitting on a 160k–200k-token treebank unless regularized.

Feasibility: **high**, but it is the challenger, not the first model. A
2-layer, 4-head, d=128 model is small enough for wasm. Compare it to the BiLSTM
on the same frozen set; do not choose by architecture preference.

### 6.3 Why not a full tagger or FinBERT

A full tagger over a fixed tagset is the wrong abstraction: torille’s
candidates are lexicon-specific and sparse, and a fixed tagset would discard
candidate identity. Fine-tuned FinBERT is far outside the wasm size/latency
budget and would require a separate tokenizer and runtime. A candidate
reranker is the only architecture that preserves the existing parser and the
alternative readings.

## 7. Rust/WASM runtime options

| runtime | wasm | ONNX | native | binary cost | effort | recommendation |
|---|---|---|---|---|---|---|
| hand-written fixed weights | yes | no | yes | smallest | medium | **M0 first** |
| `tract` | yes (pure Rust, browser/WASI) | yes | yes | moderate; loaders can be trimmed | medium | **recommended M1** |
| `rten` | yes (wasm SIMD; `wasm_api`) | yes (`.onnx`/`.rten`) | yes | moderate | low–medium | strong alternative for Transformer |
| `candle` | yes (browser examples) | partial / own model code | yes | larger | medium | possible, not first |
| `burn` + burn-onnx | yes (ndarray/wgpu/flex) | yes, codegen | yes | larger build | medium–high | possible if typed model code is preferred |
| `ort` / ONNX Runtime | native only; `ort-web` is JS | yes | best | large C++ | high for wasm | not for the embedded bundle |
| `ort-tract` / `ort-candle` | via alternative backend | yes | yes | experimental | high | not first |

### 7.1 Recommended runtime path

1. **M0:** a small MLP with fixed f32/int8 weights implemented directly in
   Rust. No inference dependency, no model loader, no wasm size surprise.
2. **M1:** export the BiLSTM/Transformer to ONNX and use `tract` or `rten`.
   Both are pure Rust and wasm-capable; `tract` has the broader loader story,
   `rten` is a simpler small-model runtime with wasm SIMD.
3. **Do not** start with `ort`: it links a C++ runtime and its wasm path is a
   separate JS/ONNX Runtime Web stack, which does not fit torille’s
   self-contained Rust wasm worker.
4. **Do not** start with Candle/Burn unless the team wants their tensor APIs
   in the engine. They are viable but add build weight for a 1–2 MB model.

### 7.2 Packaging and workers

The current wasm module is ~31 MB raw. The extension instantiates it in the
primary spell worker and in up to four morphology workers. If the model is
embedded in that module, every worker carries it.

Options:

- **A. Embed in the main wasm** — simplest loader; multiplies model memory by
  the number of worker instances. Acceptable only for a very small M0.
- **B. Separate `neural` wasm module** — only the owning grammar/style worker
  instantiates it; morphology workers stay small. Requires a second loader and
  a wasm-bindgen boundary, but is the clean long-term shape.
- **C. Native-only first** — run M0/M1 on native tools and the demo, not in the
  extension; defer the wasm packaging decision until the accuracy is proven.

Recommendation: **C for M0, B for M1 if the model survives evaluation.**
Do not put the model in the morphology pool workers.

### 7.3 Model loading and versioning

- Model bytes should be content-addressed (hash) and versioned.
- `TextAreaState` or the model owner should hold the loaded plan once.
- `SentenceCache` keys or state generation must include the model version.
- A missing/corrupt model must fall back to the heuristic path, not fail the
  checker.
- The model card must name the training corpus, licence, version, and
  evaluation set.

## 8. Performance considerations

### 8.1 Budget

The checker is on demand, not in the keystroke path. The model can be
noticeable on the first check but should not dominate it.

| workload | current | model budget |
|---|---:|---:|
| 2,000-word page, cold wasm pool | ~1.4 s | +≤250 ms |
| 2,000-word page, warm rules | ~6.9 ms | +≤10 ms |
| one edited sentence | 2.6–3.7 ms | +≤5 ms |
| 96-word page | 1.8 ms | +≤2 ms |

These are targets, not measurements. The model must be benchmarked on the
committed `tests/fixtures/perf/page-2000.txt`.

### 8.2 Cost control

- **Only run on ambiguous tokens.** K<=1 is the majority; skip them entirely.
- **Only disambiguate sentences with a minimum ambiguity density.** A sentence
  with one ambiguous token out of 20 may still need it, but batch and cap.
- **Truncate/window to 128 tokens.** Long sentences can be split with
  overlap; the syntax layer already has sentence spans.
- **Use the sentence cache.** An unchanged sentence does not rerun the model.
  A changed sentence reruns only that sentence.
- **Batch by sentence.** One plan run per sentence, padded to a fixed max
  length; avoid per-token calls.
- **Quantize.** int8 weights, f32 activations; measure top-1 loss.
- **Avoid dynamic allocations** in the hot loop; reuse buffers and masks.
- **Use wasm SIMD.** Compile with SIMD128 and provide a scalar fallback.

### 8.3 Memory

A 1–2 MB model is small, but wasm linear memory and four worker instances are
not. The model should live in the owner only. If embedded in the main module,
measure the worker-pool memory before shipping. A separate module is the safer
default for a non-trivial model.

### 8.4 Cache interaction

- `MorphologyCache` remains surface-only.
- `Disambiguation` is per analysis; do not cache it in `MorphologyCache`.
- `SentenceCache` caches diagnostics, not decisions; decisions are recomputed
  for a changed sentence or reconstructed from a cached sentence.
- The sentence-cache key must include all of:

  ```text
  sentence text hash + byte length + quote depth
  + model_version
  + feature_schema_version
  + activation_state   // off / on (and later model weights hash)
  ```

  With activation off, the old path is used and the model fields are absent.
  With activation on, an unchanged sentence with the same tuple is a cache
  hit: **do not rerun morphology, the model, syntax or the rules for it**.
  Changing the model version, feature schema or activation state invalidates
  the relevant entries (or bumps the state generation and clears the cache).
  A cached sentence’s diagnostics are shifted by span only; the model is not
  called for it.
- A model update must clear the sentence cache and the per-textarea state
  generation. A morphology/engine change that changes candidate ids also
  invalidates the feature schema and the cache.

## 9. Risks and fallback strategy

| risk | impact | mitigation |
|---|---|---|
| CC BY-SA / ND / RES licence conflict | cannot ship | FTB-only first; legal review; model card; separate model licence |
| Gold reading not in candidate set | model cannot learn; silent bias | candidate-coverage harness; report misses; fix engine gaps separately |
| Surface-cache poisoning | context bleed across occurrences | sidecar `Disambiguation`; never write scores into `WordInfo` |
| Public API break | downstream consumers | sidecar/accessor; `#[non_exhaustive]`; do not add public fields in v1 |
| Model overconfidence | wrong grammar `Error` | calibration, margin threshold, `Error` only above high confidence |
| Domain shift | poor prose performance | OOD/PUD tests; adjudicated prose; no training on OOD/PUD |
| Wasm size/latency | slow first check | owner-only module; int8; ambiguity-only; sentence cache |
| Worker multiplication | memory | separate module; owner only; measure pool |
| Sentence cache stale | wrong diagnostics after model change | model version in cache key/state generation |
| Training on oracle outputs | inflated metrics | independent UD corpora only; no Omorfi/Voikko/Kotus labels |
| Annotation disagreement | noisy labels | double annotation; `abstain`; report agreement |
| Model drift vs engine changes | candidates change | versioned candidate schema; retrain/evaluate after morphology changes |

### 9.1 Fallback ladder

1. **Feature flag off** (default): byte-for-byte current behaviour.
2. **Model unavailable / version mismatch:** current heuristic path.
3. **No candidate or K<=1:** current heuristic path.
4. **Low margin / high entropy:** use the heuristic and record an abstention.
   Do not suppress or downgrade an existing diagnostic in M0.
5. **Unknown word / depth Unknown:** heuristic path.
6. **Model crash or load failure:** log once, disable the model for the
   session, keep the checker working.

The model is an advisor, not a gatekeeper. It may change which reading the
syntax layer prefers; it may not remove a reading or a diagnostic rule’s
evidence.

## 10. Phased implementation proposal

### Phase 0 — audit and baseline (this document)

- architecture and reading-lifecycle audit;
- corpus/licence survey;
- runtime survey;
- baseline tests recorded.

**Exit:** this document reviewed; no production behaviour changed.

### Phase 1 — data and candidate-coverage harness (implemented)

Implemented as `tools/src/ud/`:

| file | purpose |
|---|---|
| `reader.rs` | CoNLL-U reader; skips multiword/empty nodes, parses FEATS, reports line errors |
| `mapping.rs` | versioned `UD_MAPPING_VERSION = "fi-ud-tags-1"`; exact/partial/ambiguous/unavailable statuses, feature mask |
| `coverage.rs` | candidate views, lemma-provenance adapter, exact/partial/ambiguous/missing/unavailable outcome, POS/case/depth tables |
| `ambiguity.rs` | initial manual fixture format and validator |
| `../bin/ud_coverage.rs` | `ud_coverage --file corpus.conllu [--ambiguity fixture.json]` |

The candidate adapter is explicit about what the current engine cannot
provide:

- `LemmaProvenance::Known(Vec<&str>)` may contain more than one lemma; a
  single-lemma token is attributed directly, a multi-lemma token is
  re-inflected through `form_readings` to find the exact source(s);
- `LemmaProvenance::Unavailable` means the harness cannot attribute the
  reading; it is reported separately and never treated as a model error;
- `source_confidence` is `Provenance::Unavailable` because `WordInfo` does not
  retain `Form::confidence` per reading.

The mapping statuses are the ones used by the model-error denominator:

- `exact`: all claimed UD dimensions represented exactly;
- `partial`: usable, but one or more dimensions approximated or absent from
  `Tags`;
- `ambiguous`: a UD feature has more than one torille interpretation
  (`UPOS=ADP` without `AdpType`, `Case=Acc` across
  Accusative/Genitive/Nominative, `Poss=Yes` without person/number);
- `unavailable`: a required feature is not representable (`DET`, `PART`,
  `Case=Lat`, etc.).

The manual fixture format is candidate-signature based, not reading-index
based:

```json
{
  "format_version": 1,
  "mapping_version": "fi-ud-tags-1",
  "cases": [
    {
      "id": "tuli_fire_noun",
      "sentence": "Tuli sammui.",
      "token_index": 0,
      "surface": "Tuli",
      "preferred": {"lemma": "tuli", "pos": "noun", "case": "nominative", "number": "singular"},
      "acceptable": [{"lemma": "tuli", "pos": "noun", "case": "nominative", "number": "singular"}],
      "abstain_ok": false,
      "note": "fire"
    }
  ]
}
```

Seed files:

- `tests/fixtures/neural/ud_sample.conllu` — hand-authored CoNLL-U smoke
  fixture, MIT/Apache-2.0;
- `tests/fixtures/neural/manual_ambiguity.json` — six initial ambiguity
  cases, mapped to `fi-ud-tags-1`.

A smoke run on the committed sample gives 11 word tokens, 9 exact, 2 partial,
0 ambiguous, 0 missing, 0 unavailable; candidate coverage 100% over the
available gold. A downloaded UD Finnish-FTB test run (not committed) gave
13,904 word tokens:

```text
gold mapping:     exact 8,320  partial 4,320  ambiguous 276  unavailable 988
candidate outcome: exact 7,636  partial 2,554  ambiguous 4    missing 2,446
candidate coverage (exact+partial): 80.6%
```

That FTB run is a smoke observation, not a gate: FTB’s `ADP` rows generally
lack `AdpType`, so all adpositions are classified `ambiguous`; `DET`/`PART`/
`Lat` readings are `unavailable`; and multi-lemma provenance re-inflection
makes the run about 30 seconds. The headline rate must not be read as product
quality.

**Exit:** candidate coverage is implemented and measurable; no model yet.

### Phase 2 — integration seam, no model (implemented)

Implemented as `src/disambiguation.rs` plus call-site seams:

- `Disambiguation`, `ReadingDecision`, `CandidateView`, `LemmaProvenance` and
  `Provenance` live in the engine; tools consume the same adapter.
- `disambiguate(tokens, &MorphologyAnalysis)` returns
  `Disambiguation::disabled()` (no decisions, `cache_key() == 0`).
- `syntax::analyze_syntax_with_decisions` and
  `analyze_syntax_with_contexts_and_decisions` accept the sidecar but do not
  use it in Phase 2.
- `check_all_tokens_with_decisions` and `check_all_with_cache` thread the
  sidecar through the combined pass.
- `syntax_check::check_with_cache` runs morphology, the disabled
  disambiguator, then `analyze_syntax_with_decisions`.
- `SentenceCache::check_with_decisions` keys each sentence by
  `sentence text + length + quote depth + disambiguation.cache_key()`.
  `check` delegates with the disabled policy. A changed sentence recomputes
  its own `disambiguate` after its sentence morphology; an unchanged sentence
  with the same key is not reprocessed.
- The wasm `analyze_syntax_with_state`, `check_syntax_with_state`,
  `check_grammar_style_with_state` and
  `check_grammar_style_from_morphology` owner paths use the sidecar. The
  morphology pool workers still do not run it.

**Exact no-op gate.** `tests/disambiguation_noop.rs` compares baseline and
disabled-sidecar paths over the committed syntax corpus, a set of focused
ambiguous cases, and the 2,000-word prose fixture. It compares:

- every `TokenSyntax` field: `key`, `role`, `confidence`, `clause`, `head`,
  `case`, `lemma`, `lemmas`, `readings`;
- every `Clause` field: ranges, predicate, subject, object, complement,
  negative, existential, parent/children, antecedent;
- every diagnostic: kind, severity, confidence, span, message, expected;
- every correction: edit order, spans, replacement, score, note;
- list structures and sentence spans;
- every `StyleDiagnostic`;
- the public combined `CheckAll` output;
- the stateless, cache and sentence-cache paths.

Any difference is a failed Phase 2. The gate is green.

**Exit:** the sidecar is wired without changing a single diagnostic, clause,
role, span, correction or style output; no model is active.

### Phase 3 — M0 feature MLP experiments (implemented)

Implemented:

- `tools/src/ud/m0.rs` and `tools/src/bin/ud_m0_features.rs` emit one JSONL
  row per ambiguous token with all candidates, the gold label index, local
  context, sentence context and morphology depth;
- `tools/neural/train_m0.py` trains one-hidden-layer candidate scorers with
  numpy only;
- `tools/neural/m0_report.json` records the experiment.

Data: UD Finnish-FTB (CC BY 4.0), `fi-ud-tags-1`.

| split | sentences | ambiguous tokens | candidates |
|---|---:|---:|---:|
| train | 14,981 | 16,738 | 41,537 |
| dev | 1,875 | 2,119 | 5,187 |
| test | 1,867 | 2,101 | 5,290 |

Model: 512 hashed features, 32 hidden units, three seeds (13, 17, 29).

| variant | context | dev top-1 | test top-1 mean | test range |
|---|---|---:|---:|---:|
| first candidate | — | — | 0.5074 | — |
| `morph` | candidate morphology + surface | 0.883–0.887 | **0.8886** | 0.8820–0.8924 |
| `local` | morph + prev/next + position | 0.900–0.902 | 0.8864 | 0.8801–0.8934 |
| `sentence` | local + hashed sentence bag | 0.881–0.883 | 0.8774 | 0.8739–0.8829 |
| `tags` | candidate tags + surface | 0.877–0.882 | 0.8862 | 0.8848–0.8872 |
| `stack` | tags + reading index, candidate count, depth, neighbour morphology | 0.911–0.916 | **0.9027** | 0.9015–0.9043 |

**Decision:** do not proceed to BiLSTM/Transformer on context gain alone.
Local context improves dev but not consistently on test; sentence context is
worse. The morphology-only MLP is the earlier candidate baseline.

Two runtime-feasible variants avoid per-reading lemma provenance:

- `tags` (`--variants tags --hash fnv`) is the tags-only model. Seed 29 was
  exported to `models/m0-tags-v1.bin` (65,812 bytes) and is embedded with the
  `neural-m0` Cargo feature.
- `stack` adds deterministic reading order, candidate count, depth and the
  first reading of the previous/next word. Seed 29 was exported to
  `models/m0-stack-v1.bin` (65,812 bytes) and is embedded with the
  `neural-m0-stack` Cargo feature.

The stack variant is the better candidate ranker: three-seed test mean 0.9027
against tags 0.8862 and the 0.5074 first-candidate baseline. The model card is
`models/README.md`.

`tools/neural/combine_m0.py` tests the other obvious combination: multiply the
tags model by a prior derived from the engine's reading order. It does not help.
The model alone reaches 0.8867 test top-1; the best dev-selected prior weight
reaches 0.8829, a product (`w=1`) 0.8734, and a square root (`w=0.5`) 0.8772.
Deterministic evidence is useful as **features** (the stack variant), not as a
second uncalibrated probability multiplied into the model.

### Phase 3b — M0 runtime/product evaluation (implemented; feature-gated, not activated)

`src/syntax/roles.rs` now has `classify_with_preference`. The first version let
the preferred reading win outright: preferred finite -> `Predicate`, preferred
adverb/adverbial case -> `Adverbial`, preferred nominal -> `Unknown`, all with
`Likely` confidence. That version produced the first product measurement:

| metric | baseline | aggressive M0 |
|---|---:|---:|
| candidate top-1 | 0.5074 | 0.8867 |
| role accuracy, all mapped tokens | 0.6643 | 0.6683 |
| role accuracy, tokens with a decision | 0.6697 | 0.6899 |
| adjudicated clean rows with a diagnostic | 0 | **1** |
| adjudicated true positives with exact kinds | 1/1 | 1/1 |
| adjudicated known-FP rows unchanged | 10/10 | 8/10 |
| page-2000 grammar diagnostics | 67 | 70 |

The clean regression was an `Error` `object_case` on `kuollut` in
`Tein sen keveällä sydämellä, sillä ethän ole kuollut.` The model correctly
preferred `Tein` as the finite predicate, but the sentence is still one clause,
so the post-predicate nominative selection took `kuollut` as a subject and the
negated-clause rule treated it as an object. On page-2000, 144 roles changed
and wrong preferences cascaded into new diagnostics.

The current implementation is the **weak-hint consumer**:

- the baseline role stays authoritative when it is `Likely`;
- an adverbial baseline always stays baseline, protecting the deliberate
  adverb/pronoun-object policy (`Tarvitsen sen`);
- only a `Weak` predicate may be re-read, as a nominal (`Unknown`) or as an
  adverbial, and the refined role is always `Weak`;
- a preferred nominal stays `Unknown`, so the subject/object passes still
  decide from context;
- a preference can never create a second confident predicate.

`Disambiguation::disabled()` keeps the exact Phase 2 no-op. The evaluation
tools are `tools/src/bin/ud_m0_eval.rs` (UD top-1 and role accuracy) and
`tools/src/bin/m0_prose_eval.rs` (adjudicated prose and full-page diagnostics).

UD Finnish-FTB test, weak-hint model on versus baseline:

| metric | baseline | tags | stack |
|---|---:|---:|---:|
| candidate top-1 | 0.5074 | 0.8867 | **0.9043** |
| role accuracy, all mapped tokens | 0.6643 | 0.6661 | **0.6694** |
| role accuracy, tokens with a decision | 0.6697 | 0.6803 | **0.6885** |
| role changes | — | 147 | 127 |

The stack model changes 127 roles: 71 improve, 26 worsen, 30 are neutral. The
extra deterministic features therefore improve both candidate ranking and the
role consumer, not just top-1.

Adjudicated prose (`tests/fixtures/prose/adjudicated.json`):

| group | baseline | tags | stack |
|---|---:|---:|---:|
| clean rows with a diagnostic | 0 | 0 | 0 |
| true positives with exact kinds | 1/1 | 1/1 | 1/1 |
| known false-positive rows unchanged | 10/10 | 10/10 | 10/10 |

On the 2,000-word `tests/fixtures/perf/page-2000.txt`, diagnostics move from
67 to 72 for tags and 67 to 71 for stack, with 66 and 44 role changes
respectively. The stack increase is spread across existing kinds:
`attribute_agreement` 12 -> 15, `object_case` 4 -> 6, `comma_missing` 13 -> 14,
with `subject_predicate_agreement` 9 -> 8 and `verb_rection` 21 -> 20. The page
is a performance fixture, not adjudicated clean prose, but the net increase is
still a failed smoke gate.

**Decision:** do not activate M0 in any default path. The weak-hint consumer
removes the adjudicated clean-prose regression and the stack features improve
UD candidate ranking and role accuracy, but adjudicated prose is unchanged and
the page-2000 diagnostic count still rises. The features stay off,
`Disambiguation` stays disabled unless a feature is compiled in, and no
diagnostic severity is lowered or suppressed.

The next step is not a larger model and not a looser consumer. It is
role/clause hardening: clause splitting at `sillä`/coordinated finite clauses,
predicate selection when a clause has more than one finite candidate, the
negated-clause object rule treating subjects and predicate complements as
objects, and adverb/nominal interaction. Those are grammar-checker bugs with
or without a model. M1/BiLSTM/Transformer remains blocked.

**Exit:** the product evaluation is complete. No model is active by default, no
diagnostics changed in the default build, and the Phase 2 no-op gate stays
green.

### Phase 4 — M1 BiLSTM / compact Transformer (blocked by M0 product evaluation)

Do not start this phase from architecture preference. It requires either:

- a demonstrated context gain over the morph M0 on a frozen test set, or
- a product evaluation showing that the M0 runtime integration improves role
  and diagnostic quality enough to justify activation.

The Phase 3b weak-hint consumer kept adjudicated prose non-negative but still
increased the page-2000 diagnostic count. Start M1 only after role/clause
hardening has made the broader smoke metrics non-negative; a larger model is
not the next lever.

If that happens:

- 1-layer BiLSTM (128 hidden) or 2-layer Transformer (d=128, 4 heads);
- word/char embeddings + candidate scorer;
- export ONNX; compare `tract` and `rten`;
- int8 quantization; calibrate; benchmark native and wasm;
- choose one architecture on measured accuracy/latency, not preference.

**Exit:** a model that is better than M0 by a meaningful margin and fits the
budget; otherwise keep M0 or stop.

### Phase 5 — WASM packaging and rollout

- owner-only module or feature-gated build;
- model version in sentence-cache key;
- options/UI flag and fallback telemetry;
- extension/demo e2e; memory measurement;
- staged rollout with a kill switch.

**Exit:** cold/warm budgets met; no regression on the prose corpus; rollback
verified.

### Phase 6 — maintenance

- retrain on new adjudicated data;
- review candidate coverage after morphology changes;
- keep a frozen test set;
- publish model card/licence with each version.

## 11. Architectural recommendation

**Integrate at the morphology→syntax boundary with a per-analysis sidecar.**

- Call `disambiguate(tokens, &MorphologyAnalysis)` once after
  `analyze_morphology`.
- Store `ReadingDecision` per token, not in `WordInfo`.
- Keep `WordInfo.readings`, `TokenSyntax.readings`, `TokenSyntax.lemmas` and
  all existing rules intact.
- Let `classify_with_preference`, `choose_predicate`, subject/object selection
  and `TokenSyntax.lemma` consult `preferred` only when the model is
  confident. The selectors still assign roles; the model never does. Phase 3b's
  measured result is that the current greedy selectors amplify a wrong
  preference, so this path is evaluated but not activated.
- `TokenSyntax.lemma` changes only when `CandidateView` provenance is known
  and unique. A preferred reading with several lemmas or unavailable
  provenance leaves the existing lemma in place.
- Keep `style_check` on the heuristic lemma for v1; pass the sidecar later
  only after measuring the divergence.
- M0 does not lower, raise or suppress diagnostics. Diagnostic-severity
  effects are evaluated separately after reading-preference effects are known.

**Model:** start with the three M0 candidate MLP variants (morphology-only,
local-context, sentence-context). Only advance to BiLSTM/Transformer if
sentence context shows a measurable gain over local context.

**Runtime:** fixed-weight Rust for M0; `tract` or `rten` for M1; owner-only
wasm module for M1. Avoid ONNX Runtime/Candle/Burn for the first milestone.

**Data:** FTB (CC BY 4.0) first; TDT (CC BY-SA 4.0) only after licence review;
OOD/PUD as frozen tests; ArkiSyn/Dialect/LAS2/Parsebank not for training or
shipping without explicit permission. Never train on Omorfi/Voikko/torille
outputs.

## 12. Smallest viable first implementation milestone

**M0 — a candidate-preference MLP behind a flag, with no wasm behaviour
change.**

Deliverables:

1. A UD→`Tags` mapper with a versioned mapping status and explicit
   `unavailable`/`ambiguous`/`partial` outcomes, plus candidate coverage on
   FTB dev/train.
2. A `CandidateView` adapter with `LemmaProvenance::{Known(Vec<lemma>), Unavailable}`
   and explicit unavailable `source_confidence`.
3. A frozen 500-token hard-ambiguity set with adjudicated preferred readings.
4. A `Disambiguation`/`ReadingDecision` sidecar and a no-op `disambiguate`
   pass wired through `check_all_with_cache`, `SentenceCache`, and the wasm
   owner path. With the flag off, all analysis and diagnostic output must be
   identical.
5. Three controlled M0 experiments (morphology-only, local-context,
   sentence-context) trained offline, exported as fixed weights, and run in
   Rust natively.
6. Evaluation separated into candidate coverage, morphological accuracy,
   syntactic-role accuracy and diagnostic precision. Do not use candidate
   top-1 as a substitute for product quality.
7. A model card with corpus/licence/version.

Exit criteria:

- no diagnostic, clause, role, span, correction or style change with the
  model disabled (Phase 2 exact no-op gate);
- no `Error` regression on the adjudicated clean prose;
- a measurable morphological gain on the frozen hard set, reported separately
  from candidate coverage and role accuracy;
- M0 shows whether local/sentence context beats morphology-only; if not, stop
  before BiLSTM/Transformer;
- model <100 KB, native inference <1 ms per 1,000 tokens;
- every alternative reading still present in `WordInfo`/`TokenSyntax`;
- no automatic lowering/suppression of existing diagnostics;
- a clear decision to proceed to BiLSTM/Transformer or stop.

This milestone is deliberately not “train a neural parser”. It proves the
seam, the labels, the candidate coverage and the evaluation before any wasm
size or runtime risk is taken.

## 13. Baseline tests recorded for this audit

Run on 2026-11-15, before any code change for this plan.

### Full engine

```bash
cargo test --release -p torille
```

Result: **1051 library tests + 142 integration/doc tests = 1193 passed,
0 failed**. Integration groups: candidate generation 4, catalogue 26,
hyphenation 9, style_check 16, style_eval 2, syntax_ambiguity 1, syntax_check
43, syntax_corpus 34, syntax_eval 2, syntax_prose 3, wiktionary 1, doctest 1.

### Morphology

```bash
cargo test --release -p torille --lib morphology::
```

Result: **17 passed, 0 failed** (including codec round-trip and seeded-cache
equivalence).

### Syntax/style gates

```bash
cargo test --release -p torille \
  --test syntax_check --test syntax_eval \
  --test syntax_ambiguity --test syntax_prose
```

Result: **43 + 2 + 1 + 3 passed, 0 failed**.

Baseline contents:

- syntax evaluation: **154 clean rows / 61 cases**, every emitted kind
  precision 1.00, recall 1.00;
- adjudicated prose seed: **15 clean rows (186 words) + 5 known
  false-positive rows (195 words) = 381 correct words; 17 known errors;
  44.6 per 1,000 words** (seed size, not a stable rate);
- exact-output and ambiguity fixtures unchanged.

### Workspace

```bash
cargo test --release --workspace
```

Result: **50 test binaries, 1256 tests passed, 0 failed**.

### Lint

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Result: clean.

### Relevant performance baseline from the repository

- 1,959-word page, cold: ~2.1–2.3 s native / ~3.0 s wasm single-thread /
  ~1.4 s wasm with 4 morphology workers;
- 1,959-word page, warm grammar+style: ~6.9 ms;
- 96-word page warm: ~1.8 ms;
- wasm artifact: ~31 MB raw (roughly 3–4 MB gzipped).

These are the numbers a model must not regress.

## 14. Phase 1 and Phase 2 implementation verification

### Phase 1 — data and coverage harness

Implemented without touching production grammar code, public APIs or model
training:

| path | change |
|---|---|
| `tools/src/ud/reader.rs` | CoNLL-U reader |
| `tools/src/ud/mapping.rs` | versioned UD→`Tags` mapping |
| `tools/src/ud/coverage.rs` | candidate/lemma adapter and coverage report |
| `tools/src/ud/ambiguity.rs` | manual ambiguity fixture format and validator |
| `tools/src/bin/ud_coverage.rs` | CLI |
| `tests/fixtures/neural/ud_sample.conllu` | hand-authored smoke corpus |
| `tests/fixtures/neural/manual_ambiguity.json` | six initial ambiguity cases |
| `tools/Cargo.toml`, `tools/src/lib.rs` | module and test dependencies |

### Commands and results

```bash
cargo test --release -p torille-tools --lib ud::
```

Result: **21 passed, 0 failed** (reader, mapping, provenance, coverage and
fixture validation).

```bash
cargo test --release -p torille-tools
```

Result: **21 passed, 0 failed** (the other tool binaries contain no unit tests).

```bash
cargo test --release --workspace
```

Result: **51 test binaries, 1,277 tests passed, 0 failed**.

```bash
cargo test --release -p torille \
  --test syntax_check --test syntax_eval \
  --test syntax_ambiguity --test syntax_prose \
  --test style_check --test style_eval
```

Result: **43 + 2 + 1 + 3 + 16 + 2 passed, 0 failed**. No existing syntax,
style or prose output changed.

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Result: clean.

### Smoke report

```bash
cargo run --release -p torille-tools --bin ud_coverage -- \
  --file tests/fixtures/neural/ud_sample.conllu \
  --ambiguity tests/fixtures/neural/manual_ambiguity.json
```

Result on the committed sample:

```text
word tokens: 11
gold mapping:      exact=9 partial=2 ambiguous=0 unavailable=0
candidate outcome: exact=9 partial=2 ambiguous=0 missing=0
candidate coverage (exact+partial): 100.0%
provenance unavailable: 0
```

The report includes POS, case and depth tables. `--mapping-version` prints
`fi-ud-tags-1`.

A downloaded, uncommitted UD Finnish-FTB test run was also used as a smoke
observation:

```text
word tokens: 13,904
gold mapping:      exact 8,320  partial 4,320  ambiguous 276  unavailable 988
candidate outcome: exact 7,636  partial 2,554  ambiguous 4    missing 2,446
candidate coverage over available gold: 80.6%
```

That number is not a product-quality gate. FTB `ADP` rows generally lack
`AdpType`, so all adpositions become `ambiguous`; `DET`/`PART`/`Lat` readings
are `unavailable`; and multi-lemma provenance re-inflection makes the run
about 30 seconds. It is a coverage smoke test, not a model result.

No model was trained or run; no production grammar behaviour or public API was
modified by Phase 1.

### Phase 2 — exact no-op sidecar

Implemented:

| path | change |
|---|---|
| `src/disambiguation.rs` | `Disambiguation`, `ReadingDecision`, `CandidateView`, `LemmaProvenance`, `Provenance`, disabled `disambiguate` |
| `src/syntax/mod.rs` | `analyze_syntax_with_decisions`, context-aware internal seam |
| `src/check.rs` | `check_all_tokens_with_decisions`; combined cache path runs the disabled disambiguator |
| `src/syntax_check/mod.rs` | cache path runs morphology → disabled disambiguator → syntax |
| `src/syntax_check/sentence_cache.rs` | `check_with_decisions`; disambiguation key joins the sentence hash |
| `torille-wasm/src/lib.rs` | owner state/combined paths use the sidecar; pool workers do not |
| `tests/disambiguation_noop.rs` | exact no-op gate |

```bash
cargo test --release -p torille --test disambiguation_noop
```

Result: **3 passed, 0 failed**. The gate compares baseline and disabled-sidecar
`TokenSyntax`, clauses, diagnostics, spans, corrections, lists, style output,
combined `CheckAll` and sentence-cache output over the syntax corpus and the
2,000-word prose fixture.

```bash
cargo test --release --workspace
```

Result: **52 test binaries, 1,282 tests passed, 0 failed**.

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Result: clean.

Phase 2 does not use a decision to change any role, clause, severity or
diagnostic. No model is active; `Disambiguation::disabled().cache_key()` is 0.

### Phase 3 — M0 experiment verification

Feature extraction:

```bash
cargo run --release -p torille-tools --bin ud_m0_features -- \
  --train /tmp/fi_ftb-ud-train.conllu \
  --dev /tmp/fi_ftb-ud-dev.conllu \
  --test /tmp/fi_ftb-ud-test.conllu \
  --out /tmp/m0
```

Result: 16,738 / 2,119 / 2,101 ambiguous train/dev/test tokens and 41,537 /
5,187 / 5,290 candidates.

Training:

```bash
python3 tools/neural/train_m0.py --data /tmp/m0 --out /tmp/m0-run \
  --hidden 32 --dim 512
```

Result (three seeds in `tools/neural/m0_report.json`):

| variant | dev top-1 | test top-1 mean |
|---|---:|---:|
| morph | 0.883–0.887 | **0.8886** |
| local | 0.900–0.902 | 0.8864 |
| sentence | 0.881–0.883 | 0.8774 |
| first candidate | — | 0.5074 |

The experiment supports stopping before BiLSTM/Transformer: local context
improves dev but not test, and sentence context is worse.

Tags-only runtime training and export:

```bash
python3 tools/neural/train_m0.py --data /tmp/m0 --out /tmp/m0-tags-29 \
  --hidden 32 --dim 512 --hash fnv --variants tags --seed 29
python3 tools/neural/export_m0.py /tmp/m0-tags-29/m0-tags.npz models/m0-tags-v1.bin
```

Result: 65,812-byte artifact, three-seed test mean 0.8862. The artifact is
embedded only with the `neural-m0` feature.

Runtime and product evaluation:

```bash
cargo run --release -p torille-tools --features neural-m0-stack --bin ud_m0_eval \
  -- --file /tmp/fi_ftb-ud-test.conllu
cargo run --release -p torille-tools --features neural-m0-stack --bin m0_prose_eval \
  -- --file tests/fixtures/prose/adjudicated.json
cargo run --release -p torille-tools --features neural-m0-stack --bin m0_prose_eval \
  -- --text tests/fixtures/perf/page-2000.txt
```

Results (weak-hint stack consumer):

| metric | baseline | M0 stack |
|---|---:|---:|
| UD candidate top-1 | 0.5074 | 0.9043 |
| UD role accuracy, all mapped tokens | 0.6643 | 0.6694 |
| UD role accuracy, decided tokens | 0.6697 | 0.6885 |
| adjudicated clean rows with a diagnostic | 0 | 0 |
| adjudicated true positives with exact kinds | 1/1 | 1/1 |
| adjudicated known-FP rows unchanged | 10/10 | 10/10 |
| page-2000 grammar diagnostics | 67 | 71 |
| page-2000 role changes | — | 44 |

The weak-hint consumer removes the earlier adjudicated clean regression and
keeps adjudicated prose at baseline, but the page-2000 smoke still increases
diagnostics. The model is not activated by default and the default build
remains an exact no-op.

```bash
cargo test --release --workspace
```

Result: **55 test binaries, 1,287 tests passed, 0 failed** (four new role
preference tests).

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Result: clean.

## 15. References

- UD Finnish-TDT: <https://universaldependencies.org/treebanks/fi_tdt/>
- UD Finnish-FTB: <https://universaldependencies.org/treebanks/fi_ftb/>
- UD Finnish-OOD: <https://universaldependencies.org/treebanks/fi_ood/>
- UD Finnish-PUD: <https://universaldependencies.org/treebanks/fi_pud/>
- UD licensing overview: <https://universaldependencies.org/contributing/licensing.html>
- TDT paper (morphology from OMorfi + ML): <https://link.springer.com/article/10.1007/s10579-013-9244-1>
- ArkiSyn (CC BY-ND): <https://www.utu.fi/en/university/faculty-of-humanities/school-of-languages-and-translation-studies/research/utu-digilang/arkisyn-database-of-finnish-conversational-discourse>
- Finnish Dialect Corpus (CC BY-ND): <https://www.kielipankki.fi/lic/la-murre-eng/>
- DMA (CC BY 4.0): <https://www.kielipankki.fi/corpora/dma/>
- LAS2 (CLARIN RES/NC): <https://www.kielipankki.fi/lic/las2/>
- Finnish Internet Parsebank: <https://turkunlp.org/finnish_nlp.html>
- UniMorph: <https://unimorph.github.io/>
- Hutmegs (commercial FINTWOL component): <http://users.ics.aalto.fi/mcreutz/papers/Creutz04tr.pdf>
- Hoya Quecedo et al., neural disambiguation of lemma/POS: <https://aclanthology.org/2020.lrec-1.439.pdf>
- Silfverberg & Hulden, data-driven Finnish morphology: <https://aclanthology.org/W18-0209.pdf>
- `tract`: <https://github.com/sonos/tract>
- `rten`: <https://docs.rs/rten/latest/rten/>
- Candle: <https://github.com/huggingface/candle>
- Burn ONNX import: <https://burn.dev/burn-book/import/onnx-model.html>
- `ort` backends: <https://ort.pyke.io/backends>

---

*Architecture audit, Phase 1/2 tooling, Phase 3 M0 experiments and the
feature-gated Phase 3b runtime/product evaluation. No production grammar
behaviour or diagnostic severity changed in the default build; the M0 model is
compiled only with `--features neural-m0` and is not activated.*
