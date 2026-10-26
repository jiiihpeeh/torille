# Training plan v5: the data-scaling experiment

## What changed since v4

Ten corrections from the inspection review. The two
biggest:

1. **The headline experiment is now A vs A, not D vs
   the 3k baseline.** The strongest version of the
   central hypothesis ("the architecture was never the
   problem; data was") is a clean A-on-3k vs A-on-50k
   comparison. If `A_scaled ≥ A_3k + 0.20` on
   `test_unseen_lemma`, we've basically proven it
   without needing trigrams, edit ops, or position. A
   result like `A_scaled = 0.72` is **stronger
   evidence** than `D_scaled = 0.69`, because A is a
   smaller, more honest baseline.

2. **The A/B/C/D ablations physically remove feature
   dimensions, not zero them.** The v4 design had
   "zero out the slots but keep the slice layout
   constant" (so the x matrix dimensions don't shift),
   then separately said "n_in is honestly smaller". Those
   are contradictory. The fix: each ablation has its
   own `n_in`. A = 145, B = 273, C = 277, D = 365.
   Each model is genuinely smaller, not "D with zeros".

The other eight:

3. **Lemma concentration is reported, not just lemma
   count.** A 100k-row corpus with 1,000 lemmas can
   still effectively be a 14-lemma corpus if 80% of
   rows come from 50 lemmas. Hard gate: **no single
   lemma contributes > 1% of training rows**. If the
   natural generator is too skewed, we sample
   lemma-balanced.

4. **Corruption distribution is compared to v1, not
   gated at 40%.** Replace "no strategy above 40%" with
   "flag any strategy whose proportion changes by > 10
   percentage points vs `data/train/train.jsonl`".

5. **Synthetic-positive filter rule is sharpened.**
   The v4 rule "remove rows where any candidate has
   `source='synthetic_positive'`" is too conservative —
   it throws away rows where the engine found the
   correct answer but happened to also surface a
   synthetic distractor. The cleaner rule: **remove
   rows where `candidates[correct_idx].source ==
   "synthetic_positive"`** — i.e. only when the
   positive itself is a fabrication.

6. **Oracle reporting is decomposed.** The combined
   "oracle_ed", "oracle_source" etc. become a
   per-feature report:
   - `correct_is_min_edit_distance` (binary, per row)
   - `correct_is_unique_min_edit_distance` (correct is
     the unique minimum, not just tied for it)
   - `correct_source` (the actual source label)
   - `correct_is_DirectLemma`
   - `correct_is_GeneratedForm`
   - `correct_is_only_candidate_with_correct_lemma`
   - **summary:** "fraction where deterministic
     candidate features alone uniquely identify the
     answer"

7. **A "minimum edit distance" baseline is added to
   the ablation table.** It's a 5-line Python
   function: `pick the candidate with lowest
   edit_distance; break ties by source priority;
   break further ties by lemma match`. This is the
   floor — if the neural model can't beat "pick the
   closest edit-distance candidate", it's learned
   nothing.

8. **Permutation diagnostic for D.** Evaluate D on
   `data/rerank_typo_v2/test_unseen_lemma.jsonl`
   twice: once with the original candidate order,
   once with the candidate order randomly permuted.
   Because the model has no positional feature, the
   top-1 should be identical (up to floating-point
   noise). This is an empirical demonstration of the
   "no positional leak" argument, in addition to the
   source-code inspection.

9. **The "D > A by 0.05" gate is removed.** The v4
   plan called for it as a hard acceptance criterion.
   The new priority order in the report is:

   1. **Did scaling solve generalisation?** (A_3k vs
      A_scaled on test_unseen_lemma)
   2. **How close does the neural model get to
      deterministic ranking?** (best A/B/C/D vs
      deterministic ranker)
   3. **Which features add measurable value?** (B, C,
      D deltas relative to A on the same data)

   Feature additions are interesting but secondary to
   the data hypothesis.

10. **Decision tree restructured** around the
    A-vs-A headline result, with three clear exits:
    - `A ≥ 0.65`: success; compare B/C/D for marginal
      value
    - `A < 0.65` but `D ≥ 0.65`: features mattered
    - All `< 0.60`: don't touch the architecture;
      inspect oracle, candidate generation,
      corruption distribution first

## Central experiment (re-stated for emphasis)

> **Can `A`, the minimal char+edit model
> (chars + bigrams + edit_distance, ~15k params,
> `n_in = 145`), generalise from 14 lemmas to 1,000+
> unseen lemmas?**

The rest of the plan is in service of answering that
question cleanly.

## Phases

### Phase 0 — Slice-3.5 baseline (DONE)

- [x] 38k-param architecture
- [x] Boundary markers, fixed hash formulas, `_UNK_GLYPH`
- [x] `_STRICT_PARITY = False` documented
- [x] `parity_test.py` golden fixture
- [x] Fixed `forward_all` numpy broadcasting bug

### Phase 1 — Scale data, with full diagnostics

**Deliverable:** a `data/rerank_typo_v2/` directory with
the five split files preserved raw, plus
`train.filtered.jsonl` (with only the precise
synthetic-positive rows removed) and a written
`REPORT.md` containing all the diagnostics below.

#### Step 1 — Generate larger raw corpus

```bash
cargo run --release --bin train_data -- \
  --output data/train_v2 \
  --max-rows 200000 \
  --seed 0
```

Wall time: 3-5 min.

#### Step 2 — Pre-rerank diagnostic (BEFORE running the engine)

Per-split report:

```
split                     rows    unique_lemmas   noun%   verb%
train                     ...     ...             ...     ...
dev                       ...
test_unseen_lemma         ...
test_unseen_inflection    ...
test_unseen_corruption    ...
```

Per-split distribution diagnostics (for `train` at minimum):

```
unique lemmas
median rows/lemma
p90 rows/lemma
p99 rows/lemma
max rows/lemma
fraction from top 10 lemmas
fraction from top 50 lemmas
fraction from top 100 lemmas
```

**Hard gates (both must pass to proceed to Step 3):**

- [ ] `train` has ≥ 1,000 unique lemmas
- [ ] **No single lemma contributes > 1% of training rows**
      (i.e. max rows/lemma / total ≤ 0.01)

If the max-lemma gate fails, we apply lemma-balanced
sampling: sample at most `total / 100` rows per lemma
to enforce the 1% cap. This is a one-line SQL change
in `train_data` (`GROUP BY lemma` and a cap) or a
post-hoc Python filter on `data/train_v2/`.

#### Step 3 — Compare corruption distribution to v1

```
corruption_type           v1 (33k) %    v2 (200k) %    delta_pp
transpose_adjacent        ...           ...            ...
delete_random             ...           ...            ...
insert_random             ...           ...            ...
substitute_random         ...           ...            ...
double_consonant_drop     ...           ...            ...
vowel_swap                ...           ...            ...
swap_random               ...           ...            ...
common_finnish_misspelling ...          ...            ...
```

**Gate:** flag (don't fail) any strategy whose
proportion changes by > 10 percentage points. Inspect
each flag manually. If the change is sampling
artefact, regenerate; if it's a real shift (e.g. the
typo model now produces more "real-looking" errors),
document it and proceed.

#### Step 4 — Run the engine pass

```bash
cargo run --release --bin rerank_data -- \
  --input data/train_v2 \
  --output data/rerank_typo_v2 \
  --max-rows 0
```

The output preserves the existing JSONL schema. Every
row is written, including `synthetic_positive` rows.

**Acceptance for Step 4:**
- [ ] All five split files exist
- [ ] `train.jsonl` has ≥ 50,000 rows
- [ ] No `correct_idx == -1`

#### Step 5 — Filter synthetic-positive rows (sharpened rule)

```python
# Rule: remove rows where the POSITIVE is fabricated.
# Synthetic-positive candidates that are merely distractors
# (the engine found the right answer plus also some
# fallbacks) are kept.
def is_synthetic_positive(row):
    return row["candidates"][row["correct_idx"]]["source"] == "synthetic_positive"

# data/rerank_typo_v2/train.filtered.jsonl = train.jsonl minus those rows
```

**Acceptance for Step 5:**
- [ ] `train.filtered.jsonl` has zero rows where
      `candidates[correct_idx].source == "synthetic_positive"`
- [ ] The rejection rate (filtered / raw) is reported

#### Step 6 — Engine miss rate diagnostic

```
split                        total    synthetic_positive_correct    engine_surfaced
train                        ...      ...                            ...
dev                          ...
test_unseen_lemma            ...
test_unseen_inflection       ...
test_unseen_corruption       ...
```

The miss rate (synthetic_positive_correct / total) is
itself a useful diagnostic.

#### Step 7 — Per-feature oracle report

Per-row statistics (over the `train.filtered.jsonl`):

```
metric                                              fraction
correct_is_min_edit_distance                        ...
correct_is_unique_min_edit_distance                 ...
correct_is_only_candidate_with_correct_lemma       ...
correct_is_DirectLemma                              ...
correct_is_GeneratedForm                            ...
correct_is_NeighbourLemma                           ...
```

Then the **combined upper bound**:

```
correct is uniquely identified by:
  min edit distance                              ... %
  min edit distance + source priority           ... %
  min edit distance + source + lemma uniqueness ... %
```

If the "min edit distance alone" uniquely identifies
the answer in > 95% of rows, the neural reranker is
being asked to learn a very small residual problem.
If it's 50-60%, there's a real ranking problem to
attack.

#### Step 8 — Save `NO_POSITIONAL_FEATURE.txt`

A short Python script that prints the column layout of
the Slice-3.5 feature matrix. The model has no
`candidate_position` column, so the
100/0/0/0 `correct_idx` histogram is structurally
irrelevant to what the model can learn.

### Phase 2 — A/B/C/D ablations with two non-neural baselines

The A/B/C/D table now has **two non-neural baselines
above the A/B/C/D rows** and the existing 3k Slice-3.5
below them. This makes the "did scaling fix it?"
question immediately visible.

#### The model sizes (each is genuinely smaller)

| Model | Features | n_in | Params |
|---|---|---|---|
| A | chars + bigrams + edit_distance | 145 | ~15k |
| B | A + trigrams | 273 | ~28k |
| C | B + edit op counts (sub, ins, del, trans) | 277 | ~28k |
| D | C + edit position (start, end) | 365 | ~38k |

Each model is physically smaller — the ablated features
are **removed** from the x matrix layout, not zeroed.
`RerankerWeights.n_in` differs across A/B/C/D.

**Implementation note:** the model.py code will have
one canonical Slice-3.5 implementation; the ablations
are a small `if use_X` chain that decides which
sub-features to compute. The parity_test.py fixture
is for **D only** (different `n_in` for A/B/C would
require a different fixture each).

#### Step 1 — Train the 4 ablations

```bash
# A: chars + bigrams + edit_distance
python3 tools/train/train.py \
  --train data/rerank_typo_v2/train.filtered.jsonl \
  --dev   data/rerank_typo_v2/dev.jsonl \
  --epochs 20 \
  --output /tmp/torille_v2_A.npz \
  --seed 0 \
  --no-trigrams --no-edit-ops --no-edit-position

# B = A + trigrams
python3 tools/train/train.py ... --output /tmp/torille_v2_B.npz \
  --no-edit-ops --no-edit-position

# C = B + edit op counts
python3 tools/train/train.py ... --output /tmp/torille_v2_C.npz \
  --no-edit-position

# D = full Slice-3.5
python3 tools/train/train.py ... --output /tmp/torille_v2_D.npz
```

#### Step 2 — Multi-seed check for C vs D (only if D wins on seed 0)

If `D > C` on `test_unseen_lemma` by ≥ 0.01 on seed 0,
re-run C and D on seeds 1 and 2. Report mean ± std.

A and B run on seed 0 only.

#### Step 3 — Evaluate

```bash
for tag in A B C D; do
  python3 tools/train/eval.py \
    --weights /tmp/torille_v2_${tag}.npz \
    --data-dir data/rerank_typo_v2/ \
    --report  /tmp/torille_v2_${tag}.md
done
```

#### Step 4 — Permutation diagnostic for D

A tiny script that re-evaluates D on
`test_unseen_lemma` with the candidate order randomly
permuted per row. Because the model has no positional
feature, the top-1 should be identical (up to floating-
point noise).

```python
# pseudocode
for row in test:
    permuted_candidates = random_permutation(row.candidates)
    permuted_row = {**row, candidates: permuted_candidates}
    permuted_score = score(D, permuted_row)
    if permuted_score.argmax() == original_score.argmax():
        # Top-1 unchanged under permutation
        pass
```

Report: `fraction of rows where top-1 is unchanged
under random candidate-order permutation`. Expected
to be ~100%. If it's not, the model has an undetected
positional feature and we investigate.

#### Step 5 — Combined ablation report

`data/rerank_typo_v2/ABLATION_REPORT.md`:

| Model | Params | n_in | Dev Top-1 | Dev MRR | Test Top-1 | Test MRR |
|---|---|---|---|---|---|---|
| engine (rule) | — | — | 0.842 | ... | 0.854 | ... |
| min edit distance | — | — | ... | ... | ... | ... |
| deterministic ranker | — | — | 0.827 | ... | 0.869 | ... |
| A / 3k (existing 3k baseline) | ~15k | 145 | 0.475 | 0.664 | 0.455 | 0.640 |
| **A / scaled** | ~15k | 145 | ... | ... | ... | ... |
| **B / scaled** | ~28k | 273 | ... | ... | ... | ... |
| **C / scaled** | ~28k | 277 | ... | ... | ... | ... |
| **D / scaled** | ~38k | 365 | ... | ... | ... | ... |

The two non-neural baselines (`min edit distance` and
`deterministic ranker`) are mandatory. The "A / 3k"
row is the existing baseline; "A / scaled" is the
headline result. If `A / scaled` is much closer to the
deterministic ranker than the 3k Slice-3.5 was, the
data hypothesis is supported.

**Acceptance for Phase 2** (in priority order):

1. **Data success:** `A / scaled` > `A / 3k` by ≥ 0.15
   on `test_unseen_lemma`. This is the clean test of
   the central hypothesis.
2. **Production candidate:** the best of {A, B, C, D}
   beats the deterministic ranker on at least one of
   {dev, test_unseen_lemma}. If not, the neural model
   is not a meaningful upgrade and we go to "expose
   deterministic features" rather than adding more
   features.
3. **Feature attribution:** the B/C/D improvements
   (if any) are monotonic or otherwise explainable.
   If D > C is non-robust across seeds, D is not
   meaningfully better than C and we ship C.

### Phase 3 — Cross-data sanity check (the most important phase)

Take the best of {A, B, C, D} and evaluate it on the
**original** 3k real-data test set, without retraining:

```bash
python3 tools/train/eval.py \
  --weights /tmp/torille_v2_BEST.npz \
  --data-dir data/rerank_scale/ \
  --report /tmp/torille_v2_BEST_on_real.md
```

Reference: existing 3k Slice-3.5 at dev 0.475,
test_unseen_lemma 0.455.

#### Interpretation matrix

| Synthetic test | Real test | Diagnosis | Next step |
|---|---|---|---|
| ≥ 0.70 | ≥ 0.55 | Strong transfer | SCALE DATA further, or add morphology |
| ≥ 0.70 | < 0.45 | Synthetic overfit | IMPROVE DATA (diversify typo model) |
| 0.55 - 0.70 | ≥ 0.45 | Mild transfer | SCALE DATA, also compare to deterministic |
| 0.55 - 0.70 | < 0.45 | Weak transfer | IMPROVE DATA |
| < 0.55 | (any) | No transfer | Investigate candidate generation / oracle ceiling |

**Acceptance for Phase 3:**
- [ ] Best model evaluated on `data/rerank_scale/test_unseen_lemma.jsonl`
- [ ] Result ≥ 0.50 (vs the existing 0.455 baseline) — otherwise not yet a production candidate
- [ ] Interpretation matrix entry recorded

### Phase 4 — Decision

```
A ≥ 0.65 on test_unseen_lemma ?
        |
   +----+----+
   |         |
  NO        YES
   |         |
inspect   D vs A ?
oracle   +---+---+
candidate  |       |
gen      D>A   D≤A
   |       |     |
   |    keep D  ship A
   |            (smaller)
   |
   v
A < 0.65 but D ≥ 0.65?
   |         |
  NO        YES
   |         |
inspect   features matter
oracle    (don't jump to CNN
candidate  yet; examine which
gen       family produced the jump)
```

**Hard rules:**
- **Don't touch the CNN yet.** The 38k-param model
  trained on 3k examples from 14 lemmas is not enough
  evidence to claim the architecture has hit a ceiling.
- **Don't add features the data hasn't asked for.** If
  the model is already at 0.65 with just the
  chars+edit features (A), adding trigrams/ops/position
  is over-engineering.
- **Don't retire `data/rerank_scale/`.** It is now a
  permanent regression test, not a development set.
- **Always include the deterministic ranker in any
  final report.** Without it, "D = 0.69" sounds like a
  win; with it, "D = 0.69 vs deterministic = 0.87"
  is a clearer story.

## Risks / watch-outs

- **Lemma-balanced sampling may slow `train_data`.** If
  we need to add the 1%-cap sampling, it's a small SQL
  change (`GROUP BY lemma` + a per-lemma cap), but the
  current rayon batch structure assumes independent rows.
  We may need to either (a) implement the cap
  post-hoc as a Python filter on the output, or (b)
  pass `--forms-per-lemma N` to `train_data` and let
  the existing code handle it. **(b) is cleaner; try
  first, fall back to (a) if the binary's CLI doesn't
  support it.**

- **`rerank_data` is row-at-a-time.** No batch
  parallelism; each row's `suggest_detailed` call is
  single-digit ms. 200k rows ≈ 3-8 min. Acceptable.

- **Multi-seed C/D comparison requires 2 extra training
  runs.** ~80-120 min of additional wall time, only if
  D > C on seed 0. Skip otherwise.

- **The 4 ablations are real ablations, not "D with
  zeros".** `n_in` and `param_count()` should differ
  across A/B/C/D in the report. The model.py refactor
  to support this must keep the `score_batch` interface
  stable (the Rust side reads the saved .npz).

- **The `parity_test.py` fixture is for D only.** A,
  B, C have different `n_in`. Skip parity for the
  ablations — they're research models, not production
  candidates.

## What `tools/typo_data.py` would add (still deferred)

If at the end of Phase 4 the Rust `train_data` strategy
enum is too rigid (e.g. we want to add OCR-style
neighbour-pixel substitutions or swipe-typing patterns
and don't want to touch Rust), then a 200-line Python
tool with `tools/typo_weights.toml` becomes worth the
maintenance. **Defer to that point.** The Rust pipeline
is correct, complete, and faster than a Python rewrite
would be.

## Reproducibility

```
Phase 1:
  cargo run --release --bin train_data -- --output data/train_v2 --max-rows 200000 --seed 0
  cargo run --release --bin rerank_data -- --input data/train_v2 --output data/rerank_typo_v2 --max-rows 0
  python3 tools/filter_synthetic.py data/rerank_typo_v2/train.jsonl > data/rerank_typo_v2/train.filtered.jsonl
  python3 tools/oracle_ceiling.py data/rerank_typo_v2/train.filtered.jsonl > data/rerank_typo_v2/ORACLE.txt
  python3 tools/positional_check.py > data/rerank_typo_v2/NO_POSITIONAL_FEATURE.txt
  python3 tools/corruption_compare.py data/train/train.jsonl data/train_v2/train.jsonl > data/rerank_typo_v2/CORRUPTION_DELTA.txt

Phase 2:
  python3 tools/train/train.py --train data/rerank_typo_v2/train.filtered.jsonl --dev data/rerank_typo_v2/dev.jsonl --epochs 20 --output /tmp/torille_v2_A.npz --seed 0 --no-trigrams --no-edit-ops --no-edit-position
  python3 tools/train/train.py ... --output /tmp/torille_v2_B.npz --no-edit-ops --no-edit-position
  python3 tools/train/train.py ... --output /tmp/torille_v2_C.npz --no-edit-position
  python3 tools/train/train.py ... --output /tmp/torille_v2_D.npz
  for tag in A B C D: python3 tools/train/eval.py --weights /tmp/torille_v2_$tag.npz --data-dir data/rerank_typo_v2/ --report /tmp/torille_v2_$tag.md
  python3 tools/permutation_test.py --weights /tmp/torille_v2_D.npz --data data/rerank_typo_v2/test_unseen_lemma.jsonl
  # if D > C on seed 0:
  for seed in 1 2: do
    python3 tools/train/train.py ... --output /tmp/torille_v2_C_$seed.npz --no-edit-position --seed $seed
    python3 tools/train/train.py ... --output /tmp/torille_v2_D_$seed.npz --seed $seed
  done
  python3 tools/baselines.py --data data/rerank_typo_v2/ > data/rerank_typo_v2/BASELINES.txt

Phase 3:
  python3 tools/train/eval.py --weights /tmp/torille_v2_BEST.npz --data-dir data/rerank_scale/ --report /tmp/torille_v2_BEST_on_real.md
```

## Outcomes (to fill in as phases complete)

- [ ] Phase 1: 50k+ rows in `data/rerank_typo_v2/train.filtered.jsonl`, full diagnostics
- [ ] Phase 2: 4 ablations trained, A/B/C/D + deterministic + min-edit + 3k baseline in combined table, permutation test for D
- [ ] Phase 3: best v2 model evaluated on `data/rerank_scale/`, cross-data diagnosis recorded
- [ ] Phase 4: decision recorded
