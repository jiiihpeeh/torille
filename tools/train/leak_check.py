#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Measure how much a train/dev overlap in the corpora moves the headline number.

`data/rerank_v2` was built by merging the original `data/rerank` splits into a
much larger corpus. Two things follow from that, and both matter when quoting an
accuracy figure:

  * `data/rerank_v2/dev` *contains* `data/rerank/dev` (all 108 rows), so the two
    dev sets are not independent measurements -- the 0.724 and the 0.685 are
    partly the same rows.
  * a handful of rows appear in **both** `data/rerank_v2/train` and
    `data/rerank/dev`, i.e. the model has seen them while being tested on them.

This script finds those rows and re-reports top-1 / MRR with them excluded, so
the leak can be stated as a number instead of a caveat.

Usage:
    leak_check.py --weights W.npz --dev data/rerank/dev.jsonl \
                  --leaky-from data/rerank_v2/train.jsonl
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from model import QueryFeatures, RerankerWeights, score_batch  # noqa: E402


def load(path: Path) -> list[dict]:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            continue  # data/rerank/dev.jsonl ends in a truncated row
    return rows


def key(row: dict) -> tuple[str, str]:
    return (row.get("misspelled"), row.get("surface_form"))


def metrics(rows: list[dict], weights: RerankerWeights) -> tuple[int, float, int]:
    """Return (n, top1, mrr) over rows that carry a correct candidate."""
    hits = 0
    mrr = 0.0
    n = 0
    for row in rows:
        cands = row["candidates"]
        if not cands or row.get("correct_idx", -1) < 0:
            continue
        qf = QueryFeatures.from_word(row["misspelled"], weights)
        logits = score_batch(
            weights, qf, [c["word"] for c in cands],
            [c.get("edit_distance", 0) for c in cands],
        )
        order = sorted(range(len(logits)), key=lambda i: (-float(logits[i]), i))
        truth = row["correct_idx"]
        n += 1
        rank = order.index(truth)
        hits += rank == 0
        mrr += 1.0 / (rank + 1)
    return n, (hits / max(1, n)), (mrr / max(1, n))


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--weights", type=Path, required=True)
    p.add_argument("--dev", type=Path, required=True)
    p.add_argument("--leaky-from", type=Path, required=True)
    args = p.parse_args()

    weights = RerankerWeights.load(args.weights)
    dev = load(args.dev)
    train_keys = {key(r) for r in load(args.leaky_from)}

    leaky = [r for r in dev if key(r) in train_keys]
    clean = [r for r in dev if key(r) not in train_keys]

    n_all, t1_all, mrr_all = metrics(dev, weights)
    n_ok, t1_ok, mrr_ok = metrics(clean, weights)

    print(f"dev rows              : {len(dev)} ({args.dev})")
    print(f"rows also in train    : {len(leaky)} -> "
          f"{sorted({k[0] for k in {key(r) for r in leaky}})}")
    print(f"model top-1 / MRR all : {t1_all:.3f} / {mrr_all:.3f}  (n={n_all})")
    print(f"model top-1 / MRR leak-free: {t1_ok:.3f} / {mrr_ok:.3f}  (n={n_ok})")
    print(f"inflation from overlap : top-1 {t1_all - t1_ok:+.3f}, "
          f"MRR {mrr_all - mrr_ok:+.3f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
