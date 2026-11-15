# M0 reading-reranking model

`m0-tags-v1.bin` is the Phase 3 M0 candidate-preference model exported for the
Rust inference path.

## Provenance

- Training corpus: UD Finnish-FTB (CC BY 4.0), train split.
- Mapping: `fi-ud-tags-1` (`tools/src/ud/mapping.rs`).
- Features: candidate morphology + surface/depth/candidate count; no
  per-reading lemma provenance, so the runtime does not need to re-inflect
  lemmas.
- Feature hash: FNV-1a 32-bit, dimension 512.
- Model: one hidden layer, 32 units, softmax over candidates.
- Seed: 29; selected by dev top-1 among seeds 13/17/29.
- Training/evaluation: `tools/neural/train_m0.py` with
  `--variants tags --hash fnv`; export with `tools/neural/export_m0.py`.

Three-seed tag-only results on UD Finnish-FTB:

| variant | dev top-1 | test top-1 mean | test range |
|---|---:|---:|---:|
| tags | 0.877–0.882 | 0.8862 | 0.8848–0.8872 |
| first candidate | — | 0.5074 | — |

## Product evaluation (2026-11-15)

The seed-29 artifact was wired through `classify_with_preference` and measured
on UD test and two prose corpora. It is **not activated**.

| metric | baseline | M0 |
|---|---:|---:|
| candidate top-1 | 0.5074 | 0.8867 |
| UD role accuracy, all mapped tokens | 0.6643 | 0.6683 |
| UD role accuracy, decided tokens | 0.6697 | 0.6899 |
| adjudicated clean rows with a diagnostic | 0 | 1 |
| adjudicated true positives with exact kinds | 1/1 | 1/1 |
| adjudicated known-FP rows unchanged | 10/10 | 8/10 |
| page-2000 grammar diagnostics | 67 | 70 |

The clean regression is an `Error` `object_case` on correct prose because a
correct predicate preference cascades into the still-unsplit clause and the
greedy subject selection. The feature stays off by default.

## Licence

The model is derived from UD Finnish-FTB, which is licensed CC BY 4.0. The
model artifact is distributed under CC BY 4.0 with attribution to the UD
Finnish-FTB contributors; it is not relicensed under the engine's MIT/Apache
terms. See <https://universaldependencies.org/treebanks/fi_ftb/>.

## Activation

The model is **off by default**. It is only used when the `neural-m0` Cargo
feature is enabled and the activation policy is enabled. Candidate top-1 is not
a product-quality metric; role accuracy and adjudicated-prose diagnostics must
be evaluated before any default activation.

## Binary layout

Little-endian:

```text
"TDM0" u8 version=1 u8 hash_id=1 (FNV-1a) u16 reserved
u32 dim u32 hidden
f32 w1[dim * hidden] f32 b1[hidden] f32 w2[hidden] f32 b2[1]
```
