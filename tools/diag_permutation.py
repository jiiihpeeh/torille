# SPDX-License-Identifier: MIT OR Apache-2.0
"""Permutation diagnostic for a trained model.

Evaluates a saved model on a test split twice: once with the
original candidate order, once with the candidate order
randomly permuted per row. Because Slice-3.5 has no
positional feature, the model's scores for each (query,
candidate_word) pair are independent of position -- so the
top-1 should be identical (up to floating-point noise).

This is the empirical demonstration of the "no positional
leak" argument. The structural argument is the
`NO_POSITIONAL_FEATURE.txt` source-code audit; this is the
data-side audit.

Usage:
    python3 tools/diag_permutation.py \
        --weights data/rerank_v2/weights/slice35_v2_D.npz \
        --data data/rerank_typo_v2/test_unseen_lemma.jsonl
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, "tools/train")
from model import (  # noqa: E402
    QueryFeatures, RerankerWeights, score_batch,
)


def _load_rows(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open() as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return rows


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--weights", type=Path, required=True)
    p.add_argument("--data", type=Path, required=True)
    p.add_argument("--max-rows", type=int, default=1000)
    p.add_argument("--seed", type=int, default=0)
    args = p.parse_args()

    weights = RerankerWeights.load(args.weights)
    rows = _load_rows(args.data)[:args.max_rows]
    rng = np.random.default_rng(args.seed)

    same_top1 = 0
    n = 0
    sample_diffs: list[tuple[str, list[str], int, int, np.ndarray, np.ndarray]] = []
    for row in rows:
        ci = row.get("correct_idx", -1)
        cands = row.get("candidates", [])
        if not (0 <= ci < len(cands)) or len(cands) < 2:
            continue
        n += 1
        q = QueryFeatures.from_word(row["misspelled"], weights)
        cand_words = [c["word"] for c in cands]
        ed = [c.get("edit_distance", 0) for c in cands]
        original_scores = score_batch(weights, q, cand_words, ed)
        original_top1 = int(np.argmax(original_scores))

        perm = rng.permutation(len(cands))
        perm_cand_words = [cand_words[i] for i in perm]
        perm_ed = [ed[i] for i in perm]
        perm_scores = score_batch(weights, q, perm_cand_words, perm_ed)
        perm_top1 = int(np.argmax(perm_scores))

        # Map back: perm_top1 in permuted list -> original index
        perm_back_to_orig = perm[perm_top1]
        if perm_back_to_orig == original_top1:
            same_top1 += 1
        elif len(sample_diffs) < 5:
            sample_diffs.append((
                row["misspelled"], cand_words, original_top1, perm_back_to_orig,
                original_scores, perm_scores,
            ))

    print(f"rows:                       {n:>10,}")
    print(f"top-1 unchanged:            {same_top1:>10,}  "
          f"({100.0 * same_top1 / max(1, n):.2f}%)")
    print(f"top-1 changed:              {n - same_top1:>10,}  "
          f"({100.0 * (n - same_top1) / max(1, n):.2f}%)")
    print()
    if sample_diffs:
        print("Sample divergences (first 5):")
        for ms, cw, orig, perm, oscores, pscores in sample_diffs:
            print(f"  misspelled={ms!r}  cand={cw}  "
                  f"orig_top1={orig}  perm_top1={perm}")
            print(f"    orig scores: {oscores}")
            print(f"    perm scores: {pscores}")
    print()
    if same_top1 == n:
        print("Top-1 is identical under permutation. Empirically confirms")
        print("that the model has no positional feature.")
    else:
        print("WARNING: some top-1s changed under permutation. Investigate.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
