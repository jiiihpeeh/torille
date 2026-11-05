#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Generate the class-15 lemmas that confirm the long partitive `-ta/-tä`.

Class 15 is the `-ea`/`-eä` family. Most members confirm both the short
partitive (`korkeaa`) and the long one (`korkeata`), but some confirm only the
short form (`vihreä` -> `vihreää`, not `vihreätä`). The inflector needs the
per-lemma answer: the long partitive is plain only for the lemmas listed here;
for the rest it is a repaired guess.

Usage:

    python3 tools/oracle/build_class15_long_partitive.py
"""

from __future__ import annotations

import os
import sqlite3

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DB = os.path.join(REPO, "data", "kotus_nomines.sqlite")
OUT = os.path.join(
    REPO, "src", "expand", "nominals", "class_15_long_partitive.rs"
)


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> int:
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    rows = db.execute(
        "SELECT DISTINCT b.lemma FROM forms f JOIN base b USING (lemma, homonym)"
        " WHERE b.class='15' AND f.ok='both' AND f.\"case\"='partitive'"
        " AND f.number='singular' AND (f.form LIKE '%ta' OR f.form LIKE '%tä')"
    ).fetchall()
    db.close()
    lemmas = sorted({lemma for (lemma,) in rows})
    lines = [
        "// SPDX-License-Identifier: MIT OR Apache-2.0",
        "// Class-15 lemmas that confirm the long partitive -ta/-tä, generated",
        "// by tools/oracle/build_class15_long_partitive.py from the nominal oracle.",
        "// Do not edit.",
        "",
        "/// Sorted; use `binary_search`.",
        "pub const LONG_PARTITIVE: &[&str] = &[",
    ]
    lines += [f"    {rust_str(lemma)}," for lemma in lemmas]
    lines += ["];", ""]
    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))
    print(f"{OUT}: {len(lemmas)} lemmas")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
