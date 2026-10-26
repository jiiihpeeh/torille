# SPDX-License-Identifier: MIT OR Apache-2.0
"""Filter rows where the positive candidate is synthetic.

The v5 plan: only drop rows where `candidates[correct_idx]
.source == "synthetic_positive"`. Synthetic-positive
candidates that are merely distractors (the engine
found the correct answer plus a fallback) are kept.

Usage:
    python3 tools/diag_filter_synthetic.py <in.jsonl> <out.jsonl>
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: diag_filter_synthetic.py <in.jsonl> <out.jsonl>",
              file=sys.stderr)
        return 2
    src = Path(argv[1])
    dst = Path(argv[2])
    dst.parent.mkdir(parents=True, exist_ok=True)

    total = 0
    kept = 0
    rejected = 0
    with src.open() as fin, dst.open("w") as fout:
        for line in fin:
            line = line.strip()
            if not line:
                continue
            total += 1
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            ci = row.get("correct_idx", -1)
            cands = row.get("candidates", [])
            if 0 <= ci < len(cands) and cands[ci].get("source") == "synthetic_positive":
                rejected += 1
                continue
            kept += 1
            fout.write(json.dumps(row, ensure_ascii=False) + "\n")

    print(f"total rows:   {total:>10,}")
    print(f"rejected:     {rejected:>10,}  "
          f"({100.0 * rejected / max(1, total):.2f}%)")
    print(f"kept:         {kept:>10,}  -> {dst}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
