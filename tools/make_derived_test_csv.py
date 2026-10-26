#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Copy the flagged sanalista word list to the experimental test file.

The production source is `data/nykysuomensanalista.csv`; its `Johdokset`
column carries derivative flags on the base rows. This script writes the same
word list to `data/nykysuomensanalista_derivedtest.csv` so the experimental
file can be inspected or used as a build input without touching production.

Usage:
    python3 tools/make_derived_test_csv.py
    python3 tools/make_derived_test_csv.py --out /tmp/derived.csv
"""

from __future__ import annotations

import argparse
import csv
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / "data" / "nykysuomensanalista.csv"
OUT = REPO / "data" / "nykysuomensanalista_derivedtest.csv"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(OUT))
    args = ap.parse_args()
    out = Path(args.out)

    with SOURCE.open(encoding="utf-8", newline="") as fh:
        rows = list(csv.reader(fh, delimiter="\t"))
    if not rows or rows[0][:4] != ["Hakusana", "Homonymia", "Sanaluokka", "Taivutustiedot"]:
        raise SystemExit(f"{SOURCE}: unexpected header {rows[:1]!r}")
    header = rows[0]
    if len(header) < 5:
        header = header + ["Johdokset"]
    with out.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh, delimiter="\t", lineterminator="\n")
        writer.writerow(header)
        for row in rows[1:]:
            if row:
                writer.writerow((row + [""] * 5)[:5])
    flagged = sum(1 for row in rows[1:] if len(row) > 4 and row[4].strip())
    print(f"wrote {out}: {len(rows) - 1} rows ({flagged} with derivative flags)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
