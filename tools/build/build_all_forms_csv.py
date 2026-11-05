#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build `data/all_forms.csv`, the compile-time form subset.

The lib's `build.rs` reads this CSV and emits `maps/all_forms.bin`
-- a front-coded set of bare lemmas and genitive-singular forms, the forms the
typing path checks first. The set must contain only forms the engine actually
recognises as *its own* current output.

`--source engine` (the default) asks the `torille` Python binding for the
nominal paradigms, so a fixed inflection bug cannot leave its old invented
forms in a checked-in blob.

`--source oracle` is the historical path: it reads `ok='both'` rows from
`data/kotus_nomines.sqlite`. A database built before the `ok='engine'`
separation marks engine-enriched rows as `both`, so that source can bake a
stale engine bug into the fast set; use it only with a clean oracle.

Usage:
  python3 tools/build/build_all_forms_csv.py
  python3 tools/build/build_all_forms_csv.py --source oracle
"""
import argparse
import sqlite3
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DB = REPO / "data" / "kotus_nomines.sqlite"
OUT = REPO / "data" / "all_forms.csv"
SANALISTA = REPO / "data" / "nykysuomensanalista.csv"

# Only rows both Omorfi and Voikko accepted are authoritative.
OK_FILTER = "both"

NOMINAL_POS = ("substantiivi", "adjektiivi", "numeraali", "pronomini")


def nominal_lemmas() -> list[str]:
    lemmas: list[str] = []
    seen: set[str] = set()
    with open(SANALISTA, encoding="utf-8") as fh:
        next(fh, None)  # header
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 3 or not parts[0]:
                continue
            if not any(pos in parts[2] for pos in NOMINAL_POS):
                continue
            # A classless compound is declined through its head. Its own
            # surface must not enter the form set as a base, or the compound
            # gate that names the head (`kristinuskon` -> `usko`) never runs.
            if len(parts) < 4 or not parts[3].strip():
                continue
            if parts[0] not in seen:
                seen.add(parts[0])
                lemmas.append(parts[0])
    return lemmas


def from_engine():
    """Yield `(form, lemma, case, number, possessive)` from the engine."""
    from torille import Engine

    engine = Engine()
    for lemma in nominal_lemmas():
        try:
            forms = engine.inflect(lemma)
        except (ValueError, RuntimeError):
            continue
        for form in forms:
            if form.negative or form.clitic is not None:
                continue
            if form.confidence == "guessed":
                continue
            yield (
                form.word,
                lemma,
                form.case or "",
                form.number or "",
                form.possessive or "",
            )


def from_oracle():
    con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    cur = con.cursor()
    cur.execute(
        'SELECT form, lemma, "case", number, possessive '
        "FROM forms WHERE ok = ? "
        'GROUP BY form, lemma, "case", number, possessive '
        "ORDER BY form, lemma",
        (OK_FILTER,),
    )
    yield from cur


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--source",
        choices=("engine", "oracle"),
        default="engine",
        help="read the forms from the current engine (default) or the "
             "materialised Kotus oracle",
    )
    p.add_argument("--out", type=Path, default=OUT, help=f"output (default: {OUT})")
    args = p.parse_args(argv)

    rows = from_engine() if args.source == "engine" else from_oracle()
    written = 0
    seen: set[tuple[str, str, str, str, str]] = set()
    with open(args.out, "w", encoding="utf-8") as f:
        for form, lemma, case_, number, possessive in rows:
            key = (form, lemma, case_, number, possessive)
            if key in seen:
                continue
            seen.add(key)
            f.write(f"{form}\t{lemma}\t{case_}\t{number}\t{possessive}\n")
            written += 1

    print(f"wrote {args.out} ({written} rows) from {args.source}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
