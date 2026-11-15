# M0 neural-disambiguation experiments

Phase 3 candidate-reranking experiments for
[`docs/neural_disambiguation.md`](../../docs/neural_disambiguation.md).

These scripts are offline research tooling. They are not built into the
shipped engine and do not change grammar or style diagnostics.

## Data

Use UD Finnish-FTB (CC BY 4.0). The data is deliberately **not committed**:

```bash
for split in train dev test; do
  curl -L --fail \
    -o /tmp/fi_ftb-ud-$split.conllu \
    https://raw.githubusercontent.com/UniversalDependencies/UD_Finnish-FTB/master/fi_ftb-ud-$split.conllu
done
```

## Features

```bash
cargo run --release -p torille-tools --bin ud_m0_features -- \
  --train /tmp/fi_ftb-ud-train.conllu \
  --dev /tmp/fi_ftb-ud-dev.conllu \
  --test /tmp/fi_ftb-ud-test.conllu \
  --out /tmp/m0
```

One JSONL row is emitted per ambiguous token whose gold reading is a real
torille candidate. The row carries all candidates, the label index, local
context, sentence context and the morphology depth. Candidate-to-lemma
provenance is explicit; rows with unavailable or ambiguous provenance are not
labelled.

## Train and evaluate

```bash
python3 tools/neural/train_m0.py --data /tmp/m0 --out /tmp/m0-run --hidden 32 --dim 512
```

The script uses only Python 3 and numpy. It trains three one-hidden-layer
candidate scorers:

| variant | context |
|---|---|
| `morph` | candidate morphology + surface only |
| `local` | morph + previous/next token and position |
| `sentence` | local + hashed bag of the sentence's other tokens |

`tools/neural/m0_report.json` records three seeds (13, 17, 29). Results:

| variant | dev top-1 | test top-1 (mean) | test range |
|---|---:|---:|---:|
| first candidate | — | 0.5074 | — |
| morph | 0.883–0.887 | **0.8886** | 0.8820–0.8924 |
| local | 0.900–0.902 | 0.8864 | 0.8801–0.8934 |
| sentence | 0.881–0.883 | 0.8774 | 0.8739–0.8829 |

The local model wins on dev but not consistently on test; the sentence model is
worse. This is why the project does not proceed to BiLSTM/Transformer on
context gain alone. The morph M0 is the current candidate baseline.

The runtime variant is tags-only, so it does not need per-reading lemma
provenance:

```bash
python3 tools/neural/train_m0.py --data /tmp/m0 --out /tmp/m0-tags-29 \
  --hidden 32 --dim 512 --hash fnv --variants tags --seed 29
python3 tools/neural/export_m0.py /tmp/m0-tags-29/m0-tags.npz \
  /tmp/m0-tags.bin
```

`tools/neural/m0_report.json` records the three context variants. The tags-only
three-seed test mean is 0.8862.

## Product evaluation

Build with `--features neural-m0` to run the embedded model:

```bash
cargo run --release -p torille-tools --features neural-m0 --bin ud_m0_eval \
  -- --file /tmp/fi_ftb-ud-test.conllu
cargo run --release -p torille-tools --features neural-m0 --bin m0_prose_eval \
  -- --file tests/fixtures/prose/adjudicated.json
cargo run --release -p torille-tools --features neural-m0 --bin m0_prose_eval \
  -- --text tests/fixtures/perf/page-2000.txt
```

Result: candidate top-1 0.8867 and decided-token role accuracy 0.6697 -> 0.6899,
but adjudicated clean prose gains an `Error` `object_case` and page-2000
diagnostics move 67 -> 70. The model remains off by default; see
[`docs/neural_disambiguation.md`](../../docs/neural_disambiguation.md) Phase 3b.

Candidate top-1 is **not** product quality. The next work is a safer consumer
or role/clause hardening, not a larger model.
