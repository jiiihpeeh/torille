#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Write `maps/oi_plural_lemmas.bin`: the class-10 words whose
reference paradigm confirms the alternative `-oi-` plural.

Class 10's plural stem changes the final `a`/`ä` to `i` (`koira` -> `koiri`,
`ampuja` -> `ampuji`). A lexically restricted subset also has an `-oi-`/`-öi-`
plural (`ampuja` -> `ampujoi-`: `ampujoita`, `ampujoiden`,
`ampujoillamme`). The subset is not derivable: `ampuja` has it, `alkaja` does
not, and both are class 10. It is read off the reference instead of guessed,
the same way the stem maps are.

The output is a sorted `DER1` set (see `src/expand/blob.rs`), read by
`src/expand/nominals/class_10.rs`. Regenerate after rebuilding
`data/kotus_nomines.sqlite`:

    python3 tools/oracle/build_oi_plural_lemmas.py
"""

from __future__ import annotations

import sqlite3
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DB = REPO / "data" / "kotus_nomines.sqlite"
OUT = REPO / "maps" / "oi_plural_lemmas.bin"


def oi_stem(lemma: str) -> str | None:
    """The `-oi-`/`-öi-` plural stem, or `None` if the lemma has no `a`/`ä`."""
    if lemma.endswith("a"):
        return lemma[:-1] + "oi"
    if lemma.endswith("ä"):
        return lemma[:-1] + "öi"
    return None


def main() -> int:
    if not DB.exists():
        raise SystemExit(f"{DB} is not present")
    con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    rows = con.execute(
        "SELECT b.lemma, f.form FROM forms f JOIN base b USING (lemma, homonym)"
        " WHERE b.class LIKE '10%' AND f.ok='both'"
        " AND f.\"case\"='partitive' AND f.number='plural'"
        " AND f.possessive=''"
    )
    lemmas: set[str] = set()
    for lemma, form in rows:
        stem = oi_stem(lemma)
        if stem is not None and form.startswith(stem):
            lemmas.add(lemma)
    keys = sorted(lemmas)
    key_bytes = b"".join(k.encode("utf-8") for k in keys)
    offsets = [0]
    for key in keys:
        offsets.append(offsets[-1] + len(key.encode("utf-8")))
    out = bytearray(b"DER1")
    out += struct.pack("<III", len(keys), len(key_bytes), 0)
    out += struct.pack(f"<{len(offsets)}I", *offsets)
    out += key_bytes
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(bytes(out))
    print(f"{OUT.relative_to(REPO)}: {len(keys)} lemmas, {len(out)} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
