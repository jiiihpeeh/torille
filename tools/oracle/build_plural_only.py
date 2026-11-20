#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Write `data/plural_only.tsv`: the sanalista rows with no standalone singular.

The candidates are the `-t` rows whose current class engine emits only plural
forms. That is a mechanical first pass, not the authority: the table is
committed and reviewed, and the build reads it rather than guessing from a
class or a suffix. The manual entries are the class-17 / `1*F` gaps the engine
does not yet recognize (`talkoot`, `hynttyyt`, `pidot`).

The oracle is used only to reject a candidate: a row with a confirmed
`ok='both'` singular form is not plural-only. It cannot confirm a candidate,
because the oracle has no `both` rows for most `-t` citations.

    (cd bindings && maturin develop --release)
    python3 tools/oracle/build_plural_only.py
"""

from __future__ import annotations

import sqlite3
import sys
from pathlib import Path

from torille import Engine

REPO = Path(__file__).resolve().parents[2]
SANALISTA = REPO / "data" / "nykysuomensanalista.csv"
DB = REPO / "data" / "kotus_nomines.sqlite"
OUT = REPO / "data" / "plural_only.tsv"

# The class engines currently miss these; a row is `(lemma, homonymy, note)`.
MANUAL = [
    ("talkoot", "", "class 17 plural-only citation"),
    ("hynttyyt", "", "class 17 plural-only citation"),
    ("pidot", "", "class 1*F plural-only citation; singular base is pito"),
]


def confirmed_singular(con: sqlite3.Connection, lemma: str, homonymy: str) -> bool:
    row = con.execute(
        "SELECT 1 FROM forms WHERE lemma=? AND homonym=? AND ok='both'"
        " AND number='singular' LIMIT 1",
        (lemma, homonymy),
    ).fetchone()
    return row is not None


def main() -> int:
    engine = Engine()
    rows: list[tuple[str, str, str, str]] = []
    for line in SANALISTA.read_text(encoding="utf-8").splitlines():
        fields = line.split("\t")
        if len(fields) < 4:
            continue
        word, homonymy, _pos, cls = fields[:4]
        if not word.endswith("t"):
            continue
        # 50/51 already have their own plural-only handling and are not
        # candidates for this table.
        if cls.startswith("50") or cls.startswith("51"):
            continue
        try:
            forms = engine.inflect(word)
        except ValueError:
            continue
        numbers = {tag for form in forms for tag in form.tags if tag in ("singular", "plural")}
        if "plural" in numbers and "singular" not in numbers:
            rows.append((word, homonymy, "engine", ""))

    con = None
    if DB.exists():
        con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
        rows = [
            row
            for row in rows
            if con is None or not confirmed_singular(con, row[0], row[1])
        ]

    seen = {(word, homonymy) for word, homonymy, _, _ in rows}
    for word, homonymy, note in MANUAL:
        if (word, homonymy) not in seen:
            rows.append((word, homonymy, "manual", note))

    rows.sort(key=lambda row: (row[0], row[1]))
    lines = ["# torille-plural-only v1", "# lemma\thomonymy\tsource\tnote"]
    lines.extend(f"{word}\t{homonymy}\t{source}\t{note}" for word, homonymy, source, note in rows)
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{OUT.relative_to(REPO)}: {len(rows)} rows")
    return 0


if __name__ == "__main__":
    sys.exit(main())
