# M0 reading-reranking models

Two Phase 3 candidate-preference models are exported for the Rust inference
path:

- `m0-tags-v1.bin` — tags-only model, embedded with the `neural-m0` feature.
- `m0-stack-v1.bin` — tags plus deterministic reading order, candidate count,
  depth and neighbour morphology, embedded with the `neural-m0-stack` feature.

Neither model uses per-reading lemma provenance, so the runtime does not need
to re-inflect lemmas.

## Provenance

- Training corpus: UD Finnish-FTB (CC BY 4.0), train split.
- Mapping: `fi-ud-tags-1` (`tools/src/ud/mapping.rs`).
- Features: candidate tags + surface; `stack` adds `det:index`,
  `det:first`, `det:candidate_count`, `det:depth` and the first reading of the
  previous/next word plus POS/case/number agreement.
- Feature hash: FNV-1a 32-bit, dimension 512.
- Model: one hidden layer, 32 units, softmax over candidates.
- Seed: 29 for both artifacts; selected by dev top-1 among seeds 13/17/29.
- Training: `tools/neural/train_m0.py --hash fnv`;
  export: `tools/neural/export_m0.py`.

Three-seed results on UD Finnish-FTB:

| variant | dev top-1 | test top-1 mean | test range |
|---|---:|---:|---:|
| tags | 0.877–0.882 | 0.8862 | 0.8848–0.8872 |
| stack | 0.911–0.916 | **0.9027** | 0.9015–0.9043 |
| first candidate | — | 0.5074 | — |

## Product evaluation (2026-11-15)

Both artifacts were wired through `classify_with_preference` and measured on UD
test and two prose corpora. The current weak-hint consumer keeps the baseline
role when it is confident or adverbial, and only lets a decision re-read a weak
predicate. It is **not activated**.

| metric | baseline | tags | stack |
|---|---:|---:|---:|
| candidate top-1 | 0.5074 | 0.8867 | **0.9043** |
| UD role accuracy, all mapped tokens | 0.6643 | 0.6661 | **0.6694** |
| UD role accuracy, decided tokens | 0.6697 | 0.6803 | **0.6885** |
| adjudicated clean rows with a diagnostic | 0 | 0 | 0 |
| adjudicated true positives with exact kinds | 1/1 | 1/1 | 1/1 |
| adjudicated known-FP rows unchanged | 10/10 | 10/10 | 10/10 |
| page-2000 grammar diagnostics | 67 | 72 | 71 |

An earlier aggressive consumer let the preferred reading win outright and
regressed adjudicated clean prose with an `Error` `object_case`. The weak-hint
consumer removed that regression. The stack model improves candidate ranking
and role accuracy over tags, but page-2000 diagnostics still rise, so neither
model is activated. Role/clause hardening is the next step.

## Licence

The models are derived from UD Finnish-FTB, which is licensed CC BY 4.0. The
model artifacts are distributed under CC BY 4.0 with attribution to the UD
Finnish-FTB contributors; they are not relicensed under the engine's MIT/Apache
terms. See <https://universaldependencies.org/treebanks/fi_ftb/>.

## Activation

Both models are **off by default**. They are only used when the corresponding
Cargo feature is enabled (`neural-m0` or `neural-m0-stack`, not both).
Candidate top-1 is not a product-quality metric; role accuracy and
adjudicated-prose diagnostics must be evaluated before any default activation.

## Binary layout

Little-endian:

```text
"TDM0" u8 version=1 u8 hash_id=1 (FNV-1a) u16 reserved
u32 dim u32 hidden
f32 w1[dim * hidden] f32 b1[hidden] f32 w2[hidden] f32 b2[1]
```
