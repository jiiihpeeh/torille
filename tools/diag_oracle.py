# SPDX-License-Identifier: MIT OR Apache-2.0
"""Per-feature oracle-ceiling diagnostic for the rerank data.

For each candidate list, asks whether the correct candidate
is uniquely identifiable by simple features the model
already has access to. If the answer is "yes, it's the
obvious choice on most signals" the neural reranker is
being asked to learn a very small residual problem; the
fix is candidate generation, not model capacity.

Reports per-row metrics and the combined upper bound.

Usage:
    python3 tools/diag_oracle.py <rerank.jsonl> [more.jsonl ...]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from collections import Counter


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


def _analyse(rows: list[dict]) -> dict:
    n = 0
    correct_is_min_ed = 0
    correct_is_unique_min_ed = 0
    correct_is_only_correct_lemma = 0
    correct_is_only_lemma_with_min_ed = 0
    src_counter: Counter = Counter()
    correct_src_counter: Counter = Counter()

    for row in rows:
        ci = row.get("correct_idx", -1)
        cands = row.get("candidates", [])
        if not (0 <= ci < len(cands)):
            continue
        n += 1
        correct = cands[ci]
        others = [c for i, c in enumerate(cands) if i != ci]
        ed = correct.get("edit_distance", 0)
        min_ed = min(c.get("edit_distance", 0) for c in cands)
        # correct is min edit distance
        if ed == min_ed:
            correct_is_min_ed += 1
        # correct is unique min edit distance
        if ed == min_ed and sum(1 for c in cands if c.get("edit_distance", 0) == min_ed) == 1:
            correct_is_unique_min_ed += 1
        # correct is the only candidate from its lemma
        cl = correct.get("lemma", "")
        if cl and sum(1 for c in cands if c.get("lemma", "") == cl) == 1:
            correct_is_only_correct_lemma += 1
        # correct is the only lemma-AND-min-ed candidate
        if cl and ed == min_ed and not any(
            c.get("lemma", "") == cl and c.get("edit_distance", 0) == min_ed
            for c in others
        ):
            correct_is_only_lemma_with_min_ed += 1
        # source distribution
        for c in cands:
            src_counter[c.get("source", "?")] += 1
        correct_src_counter[correct.get("source", "?")] += 1

    if n == 0:
        return {"n": 0}

    return {
        "n": n,
        "correct_is_min_ed": correct_is_min_ed / n,
        "correct_is_unique_min_ed": correct_is_unique_min_ed / n,
        "correct_is_only_correct_lemma": correct_is_only_correct_lemma / n,
        "correct_is_only_lemma_with_min_ed": correct_is_only_lemma_with_min_ed / n,
        "source_distribution": dict(src_counter),
        "correct_source_distribution": dict(correct_src_counter),
    }


def _pct(x: float) -> str:
    return f"{100.0 * x:>6.2f}%"


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: diag_oracle.py <rerank.jsonl> [more.jsonl ...]",
              file=sys.stderr)
        return 2
    for path_str in argv[1:]:
        path = Path(path_str)
        if not path.exists():
            print(f"missing: {path}", file=sys.stderr)
            continue
        rows = _load(path)
        stats = _analyse(rows)
        print("=" * 70)
        print(f"ORACLE DIAGNOSTIC: {path}")
        print("=" * 70)
        if stats.get("n", 0) == 0:
            print("  (no valid rows)")
            continue
        n = stats["n"]
        print(f"  rows:                                   {n:>10,}")
        print()
        print(f"  correct is min edit distance:           {_pct(stats['correct_is_min_ed'])}")
        print(f"  correct is unique min edit distance:    {_pct(stats['correct_is_unique_min_ed'])}")
        print(f"  correct is only candidate w/ its lemma: {_pct(stats['correct_is_only_correct_lemma'])}")
        print(f"  correct is only lemma + min-ed:         {_pct(stats['correct_is_only_lemma_with_min_ed'])}")
        print()
        print("  candidate source distribution:")
        for src, count in sorted(stats["source_distribution"].items(),
                                 key=lambda kv: -kv[1]):
            pct = 100.0 * count / (n * 1)  # approximate
            print(f"    {src:<28} {count:>10,}")
        print()
        print("  correct-candidate source distribution:")
        for src, count in sorted(stats["correct_source_distribution"].items(),
                                 key=lambda kv: -kv[1]):
            pct = 100.0 * count / n
            print(f"    {src:<28} {count:>10,}  ({pct:>5.2f}% of correct)")
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
