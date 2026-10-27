#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Add one lemma to the Kotus sanalista, updating both the `.csv` and `.txt` copies.

The build reads `data/nykysuomensanalista.csv` (embedded with `include_str!`);
the `.txt` is the legacy companion with only the four Kotus columns. A word has
to be added to both so they stay in step. Both are tab-separated:

    txt:  lemma <TAB> homonym <TAB> pos <TAB> class
    csv:  Hakusana <TAB> Homonymia <TAB> Sanaluokka <TAB> Taivutustiedot
          (then the same rows, plus an empty `Johdokset` column)

The sanalista's sort order is a Finnish collation the files do not spell out
(leading hyphens are ignored and `ä`/`ö` sort after `z`), so this does **not**
guess a position: name the neighbour with `--after`/`--before`. The row goes in
immediately after/before the matching row in both files. Idempotent: if the
word is already present, it says so and changes nothing.

Usage:
    python3 tools/add_sanalista_word.py --after plussata plutokratia substantiivi 12
    python3 tools/add_sanalista_word.py --before plutonium plutokratia substantiivi 12
    python3 tools/add_sanalista_word.py --after plussata --dry-run plutokratia substantiivi 12
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TXT = REPO / "data" / "nykysuomensanalista.txt"
CSV = REPO / "data" / "nykysuomensanalista.csv"


def row(word: str, homonym: str, pos: str, klass: str) -> str:
    # The CSV carries a fifth `Johdokset` column; new Kotus rows leave it
    # empty. The .txt copy has only the four Kotus columns.
    return f"{word}\t{homonym}\t{pos}\t{klass}\n"


def insert(
    path: Path, new_row: str, word: str, after: str | None, before: str | None,
    header: bool, dry_run: bool,
) -> str:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    start = 1 if header else 0
    if header and (not lines or not lines[0].startswith("Hakusana")):
        raise SystemExit(f"{path}: expected the `Hakusana` header on line 1")

    for line in lines[start:]:
        first = line.split("\t", 1)[0]
        if first == word:
            return f"{path.name}: already present (no change)"

    anchor, offset = (after, 1) if after is not None else (before, 0)
    for n, line in enumerate(lines):
        if line.split("\t", 1)[0] == anchor:
            pos = n + offset
            if dry_run:
                return f"{path.name}: would insert at row {pos + 1}"
            lines.insert(pos, new_row)
            path.write_text("".join(lines), encoding="utf-8")
            return f"{path.name}: inserted at row {pos + 1}"

    raise SystemExit(f"{path.name}: anchor {anchor!r} not found")


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("word", help="lemma to add")
    ap.add_argument("pos", help="part of speech, e.g. substantiivi")
    ap.add_argument("klass", help="Kotus class expression, e.g. 12 or 9*B")
    ap.add_argument("--homonym", default="", help="the homonym number (default empty)")
    ap.add_argument("--after", metavar="WORD", help="insert after this lemma")
    ap.add_argument("--before", metavar="WORD", help="insert before this lemma")
    ap.add_argument("--dry-run", action="store_true", help="report, write nothing")
    args = ap.parse_args()

    if (args.after is None) == (args.before is None):
        ap.error("give exactly one of --after WORD / --before WORD")

    new = row(args.word, args.homonym, args.pos, args.klass)
    csv_new = new.rstrip("\n") + "\t\n"
    print(f"row: {new.rstrip()}")
    for path, header, new_row in ((TXT, False, new), (CSV, True, csv_new)):
        if not path.exists():
            print(f"{path.name}: missing", file=sys.stderr)
            return 2
        print(insert(path, new_row, args.word, args.after, args.before, header, args.dry_run))
    return 0


if __name__ == "__main__":
    sys.exit(main())
