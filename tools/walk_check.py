#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Walk noun classes against the database reference, one word at a time.

For each class, a lemma is picked from the database (seeded, so reproducible)
and torille's output is lined up against the reference's confirmed forms. The
point is a *readable* discrepancy listing for a human chasing a class, so the
two things that are the database's own limits are filtered out rather than
presented as defects:

* the **comitative**. The reference grid is the 13 noun cases; it has no
  comitative row, so every `taloineen` would read as an invented word. They are
  real and omitted, not wrong.
* a **word the reference does not know** -- one with no confirmed form (Omorfi
  could not decline it). The reference has no opinion to disagree with, so it
  is skipped exactly the way the harness skips `no confirmed form`.

Everything else shown is a real disagreement. A `missing` form also shows what
torille offers for the cell instead, so a rows where torille merely picks the
other of two valid Finnish spellings (`yhdistelyjeni` beside `yhdistelyideni`)
is told apart from one where nothing is produced.

Usage:
    python3 tools/walk_check.py                 # all 51 numeric classes, one word each
    python3 tools/walk_check.py -c 27           # one class
    python3 tools/walk_check.py -c 5,9,9*B -n 3 # several, three words each
    python3 tools/walk_check.py -c all --seed 7

Flags:
    -c/--classes   comma list, 'numeric' (the 51 plain Kotus classes, default),
                   or 'all' (every class value incl. gradation variants)
    -n/--per-class words per class (default 1)
    --seed         seeded pick, reproducible (default 0)
    --lexicon      path to the reference database (default data/kotus_nomines.sqlite)
    -k/--max       discrepancies shown per word (default 6)
"""

from __future__ import annotations

import argparse
import random
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from omorfi_harness import Harness

REPO = Path(__file__).resolve().parent.parent
DEFAULT_LEXICON = REPO / "data" / "kotus_nomines.sqlite"


def db_cells(conn: sqlite3.Connection, lemma: str, form: str) -> list[tuple[str, str, str]]:
    return [tuple(r) for r in conn.execute(
        "SELECT DISTINCT [case], number, possessive FROM forms "
        "WHERE lemma=? AND form=? AND ok='both'", (lemma, form))]


def ts(cell: tuple[str, str, str]) -> str:
    case, number, poss = cell
    return f"{case}/{number}/{'poss ' + poss if poss else '-'}"


def walk(conn, harness, cls: str, lemmas: list[str], per_word: int, seed: int, cap: int) -> None:
    rng = random.Random(seed)
    for lemma in rng.sample(sorted(lemmas), min(per_word, len(lemmas))):
        confirmed = set(r[0] for r in conn.execute(
            "SELECT DISTINCT form FROM forms WHERE lemma=? AND ok='both'", (lemma,)))
        if not confirmed:
            print(f"  {lemma:20} SKIP — the reference has no confirmed form for it (Omorfi does not know it)")
            print()
            continue
        try:
            forms = harness.torille_forms(lemma)
        except (ValueError, RuntimeError) as exc:
            print(f"  {lemma:20} SKIP — not inflectable ({exc})")
            print()
            continue
        plain = [f for f in forms if f.plain]
        # The comitative is outside the reference grid; carry it separately.
        for f in plain:
            f._comitative = f.case == "comitative"
        plain_words = {f.word for f in plain if not getattr(f, "_comitative", False)}
        cell_of = {}
        for f in plain:
            if f.case != "comitative":
                cell_of.setdefault(f.word, (f.case, f.number, f.possessive))

        missing = sorted(confirmed - plain_words)[: cap]
        # A form torille offers as plain that the reference does not confirm,
        # and that is not one of the reference's own blind spots.
        excess = sorted(w for w in (plain_words - confirmed))
        print(f"  {lemma:20} class {cls:>5}   confirmed={len(confirmed):4}  "
              f"missing={len(missing):3}  excess={len(excess):3}")
        if missing:
            for w in missing[: len(missing)]:
                cells = db_cells(conn, lemma, w)
                cell = cells[0] if cells else ("", "", "")
                alt = cell_of.get(w)
                alt_s = f" elsewhere: {alt[0]}/{alt[1]}/{alt[2] or '-'}" if alt else " (nothing produced there)"
                print(f"   -  MISSING {w:24} {ts(cell)}{alt_s}")
        for w in excess[: cap]:
            c = cell_of.get(w, ("", "", ""))
            print(f"   +  EXCESS  {w:24} {ts((c[0], c[1], c[2]))}")
        print()


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("-c", "--classes", default="numeric",
                   help="comma list, 'numeric' (default), or 'all'")
    p.add_argument("-n", "--per-class", type=int, default=1, help="words per class (default 1)")
    p.add_argument("-k", "--max", type=int, default=8, help="discrepancies shown per word (default 8)")
    p.add_argument("--seed", type=int, default=0, help="seeded word pick")
    p.add_argument("--lexicon", default=str(DEFAULT_LEXICON), help="reference database")
    p.add_argument("-q", "--quiet", action="store_true")
    args = p.parse_args(argv)

    conn = sqlite3.connect(args.lexicon)
    harness = Harness("noun", lexicon=args.lexicon)

    if args.classes == "numeric":
        classes = sorted({r[0] for r in conn.execute("SELECT DISTINCT class FROM base") if r[0].isdigit()},
                         key=lambda c: int(c))
    elif args.classes == "all":
        classes = sorted({r[0] for r in conn.execute("SELECT DISTINCT class FROM base")})
    else:
        wanted = {c.strip() for c in args.classes.split(",") if c.strip()}
        classes = [c for c in sorted({r[0] for r in conn.execute("SELECT DISTINCT class FROM base")}) if c in wanted]

    for cls in classes:
        lemmas = [r[0] for r in conn.execute(
            "SELECT lemma FROM base WHERE class=? AND homonym=''", (cls,))]
        if not lemmas:
            continue
        if not args.quiet:
            print(f"== class {cls} ==")
        walk(conn, harness, cls, lemmas, args.per_class, args.seed, args.max)
    return 0


if __name__ == "__main__":
    sys.exit(main())