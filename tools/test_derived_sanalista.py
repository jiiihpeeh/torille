#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Test derivative flags on the word-list CSV.

The flags live on **base** rows (`nostaa` carries `FREQ:67`), not on separate
derived rows. This checks:

1. the test copy keeps the production CSV's first four columns unchanged;
2. every `TYPE:CLASS` flag names a known rule and a verb base;
3. the derived lemma is computed from the base and the class engine generates
   the expected forms;
4. no derived lemma is duplicated as a row of its own.

Run `tools/make_derived_test_csv.py` first if the copy is missing.

Usage:
    python3 tools/test_derived_sanalista.py
"""

from __future__ import annotations

import csv
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / "data" / "nykysuomensanalista.csv"
TEST = REPO / "data" / "nykysuomensanalista_derivedtest.csv"

sys.path.insert(0, str(REPO / "tools"))
from add_derivation_flag import (  # noqa: E402
    derive_chain,
    inflect,
    row_flags,
)


def read(path: Path) -> tuple[list[str], list[list[str]]]:
    with path.open(encoding="utf-8", newline="") as fh:
        rows = list(csv.reader(fh, delimiter="\t"))
    return rows[0], [row for row in rows[1:] if row]


def main() -> int:
    if not TEST.exists():
        print(f"missing {TEST}; run tools/make_derived_test_csv.py", file=sys.stderr)
        return 2

    src_header, src_rows = read(SOURCE)
    test_header, test_rows = read(TEST)
    failures = 0

    def ok(cond: bool, msg: str) -> None:
        nonlocal failures
        print(("ok   " if cond else "FAIL ") + msg)
        if not cond:
            failures += 1

    ok(test_header[:4] == src_header[:4], f"header {test_header[:4]}")
    ok(test_header[4:] == ["Johdokset"], f"flag column {test_header[4:]}")
    ok(
        len(test_rows) == len(src_rows),
        f"row count {len(test_rows)} == {len(src_rows)} (no derived rows)",
    )

    mismatch = [
        i for i, (before, after) in enumerate(zip(src_rows, test_rows))
        if before[:4] != after[:4]
    ]
    ok(not mismatch, f"first four columns unchanged ({len(mismatch)} mismatch)")

    expected = {
        "nostaa": ("FREQ", "67", "nostatella", ["nostatella", "nostatelee", "nostateli"]),
        "jouduttaa": (
            "REFL",
            "52",
            "jouduttautua",
            ["jouduttautua", "jouduttautuu", "jouduttautui"],
        ),
    }

    row_words = {row[0] for row in test_rows}
    for row in test_rows:
        base = row[0]
        flags = row_flags(row)
        if not flags:
            continue
        for flag in flags:
            try:
                lemma, out_pos, out_class = derive_chain(base, flag, row[2].strip())
            except ValueError as exc:
                ok(False, f"{base}: {flag}: {exc}")
                continue
            ok(lemma not in row_words,
               f"{base}: derived lemma {lemma} is not a separate row")
            forms = inflect(lemma, out_class, out_pos)
            ok(lemma in forms, f"{base}: class {out_class} generates {lemma}")
            if base in expected:
                want_kind, want_class, want_lemma, want_forms = expected[base]
                ok(flag == f"{want_kind}:{want_class}",
                   f"{base}: expected {want_kind}:{want_class}, got {flag}")
                ok(lemma == want_lemma, f"{base}: derived {want_lemma}, got {lemma}")
                for form in want_forms:
                    ok(form in forms, f"{base}: {lemma} generates {form}")

    flagged = sum(1 for row in test_rows if row_flags(row))
    ok(flagged >= 2, f"{flagged} flagged base rows")
    print()
    if failures:
        print(f"{failures} failure(s)")
        return 1
    print("ALL PASS - flags live on base rows and the class engine generates the forms")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
