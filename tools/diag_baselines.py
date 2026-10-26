# SPDX-License-Identifier: MIT OR Apache-2.0
"""Non-neural baseline rankers for the rerank data.

Two baselines, both computed without any learned model:

  1. min_edit_distance: pick the candidate with the lowest
     `edit_distance`. Ties are broken by source priority
     (`DirectLemma > GeneratedForm > NeighbourLemma >
     MorphologyHypothesis > synthetic_positive`).
     Final ties broken by lemma match with the source
     row's `lemma` field.

  2. deterministic_rank: the existing hand-coded ranker
     in `tools/train/deterministic_ranker.py`.

Both report top-1 accuracy and MRR. Output is meant to be
the non-neural row in the ABLATION_REPORT.md table.

Usage:
    python3 tools/diag_baselines.py <rerank-dir> [<more-dir> ...]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
import numpy as np

sys.path.insert(0, "tools/train")
from deterministic_ranker import (  # noqa: E402
    deterministic_rank,
)


# Source priority for tie-breaking in min_edit_distance. Lower = better.
SOURCE_PRIORITY = {
    "direct_lemma": 0,
    "generated_form": 1,
    "neighbour_lemma": 2,
    "morphology_hypothesis": 3,
    "synthetic_positive": 4,
}


def _load(path: Path) -> list[dict]:
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


def _min_edit_distance_pick(candidates: list[dict], row_lemma: str) -> int:
    """Pick the index of the best candidate by min-edit-distance
    with source-priority + lemma-match tie-breaks.
    """
    n = len(candidates)
    if n == 0:
        return -1
    best_score = None
    best_idx = 0
    for i, c in enumerate(candidates):
        ed = c.get("edit_distance", 0)
        src_rank = SOURCE_PRIORITY.get(c.get("source", ""), 99)
        lemma_match = 0 if c.get("lemma", "") == row_lemma and row_lemma else 1
        # Lower tuple is better.
        score = (ed, src_rank, lemma_match)
        if best_score is None or score < best_score:
            best_score = score
            best_idx = i
    return best_idx


def _metrics(rows: list[dict], pick_fn, score_fn) -> tuple[float, float, int]:
    """Top-1 accuracy and MRR for a given pick_fn (top-1) and
    score_fn (full per-candidate scores, for MRR ordering).
    """
    n_correct = 0
    mrr_sum = 0.0
    n = 0
    for row in rows:
        ci = row.get("correct_idx", -1)
        cands = row.get("candidates", [])
        if not (0 <= ci < len(cands)) or not cands:
            continue
        n += 1
        pick = pick_fn(row, cands)
        if pick == ci:
            n_correct += 1
        scores = score_fn(row, cands)
        try:
            arr = np.asarray(scores, dtype=float)
        except (TypeError, ValueError):
            continue
        if arr.size != len(cands) or not np.isfinite(arr).all():
            continue
        order = np.argsort(-arr)
        if ci in order:
            rank = int(np.where(order == ci)[0][0])
            mrr_sum += 1.0 / (rank + 1)
    return (
        n_correct / n if n else 0.0,
        mrr_sum / n if n else 0.0,
        n,
    )


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: diag_baselines.py <rerank-dir> [<more-dir> ...]",
              file=sys.stderr)
        return 2

    files = [
        "train.jsonl",
        "dev.jsonl",
        "test_unseen_lemma.jsonl",
        "test_unseen_inflection.jsonl",
        "test_unseen_corruption.jsonl",
    ]

    print(f"  {'split':<32} {'min_edit_top1':>14} {'min_edit_mrr':>14} {'det_top1':>12} {'det_mrr':>12}  {'n':>8}")
    print("  " + "-" * 96)

    for dir_str in argv[1:]:
        d = Path(dir_str)
        for fname in files:
            p = d / fname
            if not p.exists():
                continue
            rows = _load(p)

            def min_pick(row, cands):
                return _min_edit_distance_pick(cands, row.get("lemma", ""))

            def min_score(row, cands):
                # Higher = better. We negate the (ed, src_rank, lemma_match)
                # tuple so that lower tuples become higher scores. The
                # tuple order already encodes the tie-break; we just need
                # to make a single comparable scalar.
                src_priority = SOURCE_PRIORITY
                scores = []
                for c in cands:
                    ed = c.get("edit_distance", 0)
                    src_rank = src_priority.get(c.get("source", ""), 99)
                    lemma_match = 0 if c.get("lemma", "") == row.get("lemma", "") and row.get("lemma", "") else 1
                    scores.append(-(ed * 1000 + src_rank * 10 + lemma_match))
                return scores

            def det_pick(row, cands):
                try:
                    scores = deterministic_rank(row["misspelled"], cands)
                    arr = np.asarray(scores, dtype=float)
                except (TypeError, ValueError, KeyError):
                    return min_pick(row, cands)
                if arr.size != len(cands) or not np.isfinite(arr).all():
                    return min_pick(row, cands)
                return int(np.argmax(arr))

            def det_score(row, cands):
                try:
                    return list(np.asarray(
                        deterministic_rank(row["misspelled"], cands), dtype=float))
                except (TypeError, ValueError, KeyError):
                    return min_score(row, cands)

            min_top1, min_mrr, n = _metrics(rows, min_pick, min_score)
            det_top1, det_mrr, _ = _metrics(rows, det_pick, det_score)
            print(f"  {p.parent.name + '/' + fname:<32} "
                  f"{min_top1:>13.3f} {min_mrr:>13.3f} "
                  f"{det_top1:>11.3f} {det_mrr:>11.3f}  {n:>8,}")
        print()

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
