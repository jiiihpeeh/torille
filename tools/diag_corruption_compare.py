# SPDX-License-Identifier: MIT OR Apache-2.0
"""Compare the corruption-strategy distribution of two JSONL corpora.

Reports the per-strategy fraction in each corpus and flags any
strategy whose proportion changes by more than 10 percentage
points. The "10pp" threshold is the plan's gate for "is the new
corpus materially different from the old one".

Usage:
    python3 tools/diag_corruption_compare.py <v1.jsonl> <v2.jsonl>
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from pathlib import Path


def _load(path: Path) -> Counter:
    c: Counter = Counter()
    with path.open() as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            c[row.get("corruption_type", "?")] += 1
    return c


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: diag_corruption_compare.py <v1.jsonl> <v2.jsonl>",
              file=sys.stderr)
        return 2
    v1 = _load(Path(argv[1]))
    v2 = _load(Path(argv[2]))
    total1, total2 = sum(v1.values()), sum(v2.values())
    if total1 == 0 or total2 == 0:
        print("one of the corpora is empty", file=sys.stderr)
        return 1

    print(f"v1 = {argv[1]}  ({total1:,} rows)")
    print(f"v2 = {argv[2]}  ({total2:,} rows)")
    print()
    print(f"  {'strategy':<32} {'v1 %':>8} {'v2 %':>8} {'delta_pp':>10}  flag")
    print("  " + "-" * 70)

    all_keys = sorted(set(v1) | set(v2))
    any_flag = False
    for k in all_keys:
        p1 = 100.0 * v1.get(k, 0) / total1
        p2 = 100.0 * v2.get(k, 0) / total2
        delta = p2 - p1
        flag = ""
        if abs(delta) > 10.0:
            flag = "  <-- > 10pp"
            any_flag = True
        print(f"  {k:<32} {p1:>7.2f}% {p2:>7.2f}% {delta:>+9.2f}pp{flag}")

    print()
    if any_flag:
        print("  WARNING: at least one strategy changed by > 10pp.")
        print("  Inspect whether the change is intentional or a")
        print("  sampling artefact before trusting the new corpus.")
    else:
        print("  All strategies within +/- 10pp of v1. Distribution")
        print("  is comparable.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
