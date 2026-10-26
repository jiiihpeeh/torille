#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Systematically audit noun classes against the database, word by word.

The workflow is class-at-a-time, in Kotus order: work through *every* word of
class 1, then every word of class 2, then class 3, and so on. For each word the
database (Ok=`both`) is the reference torille is checked against, and the two
discrepancies that matter are reported:

* **missing** -- a confirmed form torille does not offer;
* **excess** -- a `plain` form torille offers that the reference does not
  confirm.

As in `walk_check`, the two reference blind spots are filtered out: the
comitative (outside the 13-case grid) and words the reference cannot decline
(no confirmed form).

Given the classes are worked through in order, `--from` resumes where a thread
was left off, printing each class's words with a one-line discrepancy count per
word so an untouched class is visible at a glance and the discrepancies can be
drilled into.

Usage:
    python3 tools/audit_all.py --from 1            # class 1, every word
    python3 tools/audit_all.py --from 27 -k 6     # class 27 and up, 6 lines/word
    python3 tools/audit_all.py -c 1,2,3           # just these classes
    python3 tools/audit_all.py                       # every class (huge; use --from)
"""

from __future__ import annotations

import argparse
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from omorfi_harness import Harness

REPO = Path(__file__).resolve().parent.parent
DEFAULT_LEXICON = REPO / "data" / "kotus_nomines.sqlite"


def classify(conn, harness, lemma):
    """Return (missing, excess) against the database, filtering the two
    reference blind spots (comitative; Omorfi-unknown)."""
    confirmed = {r[0] for r in conn.execute(
        "SELECT DISTINCT form FROM forms WHERE lemma=? AND ok='both'", (lemma,))}
    if not confirmed:
        return None, None  # the reference cannot decline this word
    try:
        forms = harness.torille_forms(lemma)
    except (ValueError, RuntimeError):
        return "skip", ""
    plain = [f for f in forms if f.plain and f.case != "comitative"]
    words = {f.word for f in plain}
    return sorted(confirmed - words), sorted(words - confirmed)


def db_cells(conn, lemma, form):
    return [tuple(r) for r in conn.execute(
        "SELECT DISTINCT [case], number, possessive FROM forms "
        "WHERE lemma=? AND form=? AND ok='both'", (lemma, form))]


def ts(cell):
    case, number, poss = cell
    return f"{case}/{number}/{'poss ' + poss if poss else '-'}"


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("-c", "--classes", default="", help="comma list, else Kotus order from --from")
    p.add_argument("--from", type=int, default=1, dest="start", help="numeric class to start at")
    p.add_argument("-k", "--max", type=int, default=4, help="discrepancy lines per word")
    p.add_argument("--lexicon", default=str(DEFAULT_LEXICON))
    p.add_argument("-q", "--quiet", action="store_true")
    args = p.parse_args(argv)

    conn = sqlite3.connect(args.lexicon)
    harness = Harness("noun", lexicon=args.lexicon)

    if args.classes:
        wanted = [c.strip() for c in args.classes.split(",") if c.strip()]
        ordered = wanted
    else:
        # Kotus order: numeric classes in order from `--from`, then the `*`
        # variants grouped under their base class.
        all_classes = {r[0] for r in conn.execute("SELECT DISTINCT class FROM base")}
        numeric = sorted((int(c) for c in all_classes if c.isdigit()))
        variant_order = sorted(
            (c for c in all_classes if not c.isdigit()),
            key=lambda c: (int(c.split("*")[0]) if c.split("*")[0].isdigit() else 999, c))
        ordered = []
        for n in range(max(args.start, min(numeric)), max(numeric) + 1):
            c = str(n)
            if c in all_classes:
                ordered.append(c)
            for v in variant_order:
                if v.split("*")[0] == str(n):
                    ordered.append(v)

    for cls in ordered:
        lemmas = [r[0] for r in conn.execute(
            "SELECT lemma FROM base WHERE class=? AND homonym=''", (cls,))]
        if not lemmas:
            continue
        if not args.quiet:
            print(f"\n== class {cls} -- {len(lemmas)} words ==")
        clean_bad = clean_ok = 0
        for lemma in sorted(lemmas):
            miss, exc = classify(conn, harness, lemma)
            if miss is None:
                continue
            if miss == "skip":
                print(f"   {lemma:22} (not inflectable)")
                continue
            if not miss and not exc:
                clean_ok += 1
                continue
            clean_bad += 1
            print(f"   {lemma:22} +{len(exc)} plain-not-in-ref / -{len(miss)} confirmed-missing")
            for m in miss[: args.max]:
                c = db_cells(conn, lemma, m)
                print(f"       - {m:22} {ts(c[0]) if c else ''}")
            for e in exc[: args.max]:
                print(f"       + {e:22}")
        if not args.quiet:
            print(f"   -> {clean_ok} clean, {clean_bad} with discrepancies")
    return 0


if __name__ == "__main__":
    sys.exit(main())