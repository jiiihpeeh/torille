#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Extract the `misspelled` column of a rerank JSONL split, one word per line.

Feeds `tools/src/bin/rerank_bench.rs --words`. Kept as a script rather
than a `python3 -c` one-liner so that non-ASCII query words survive the
round trip -- shell heredocs in this environment mangle them.

Usage: extract_queries.py <input.jsonl> <output.txt>
"""
import json
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    words = []
    skipped = 0
    for line in src.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        # `data/rerank/dev.jsonl` ends in a truncated row (the file
        # was written by a run that stopped mid-line). eval.py skips
        # JSON errors the same way and reports 108 rows; skip here so
        # the query list matches what the accuracy eval actually
        # scores, rather than failing on a file that has always been
        # like this.
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            skipped += 1
            continue
        w = row.get("misspelled")
        if w is None:
            raise SystemExit(f"{src}: row without a misspelled field")
        words.append(w)
    if not words:
        raise SystemExit(f"{src}: no usable rows")
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_text("\n".join(words) + "\n", encoding="utf-8")
    note = f", {skipped} malformed row(s) skipped" if skipped else ""
    print(f"{src} -> {dst}: {len(words)} queries{note}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
