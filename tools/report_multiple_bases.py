#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""How good is the ranked first base, once genuine ambiguity is allowed?

`test_reverse_lookup_recall.py` scores the first base against one reference
pair: the first base has to be *this* pair's lemma. That is the right question
for recall, but it is the wrong question for a surface that really is a form of
several lemmas. `aavistamattoman` is a form of the adjective `aavistamaton`
and of the verb `aavistaa`; both are `ok='both'` in their respective oracles.
A first base of `aavistaa` is not an error, it is a different valid answer.

This tool reads the TSV `lookup_recall` writes and separates three questions:

* **exact** -- the first base is the reference pair's lemma (97.53% over all
  640,873 pairs at the 2026-11-03 baseline);
* **confirmed** -- the first base is *a* `ok='both'` base of the surface, even
  if it is not the reference's lemma (99.60% over all pairs, 99.70% over the
  640,139 found pairs);
* **one authority** -- the first base is confirmed by Omorfi or Voikko but not
  both (99.85% over all pairs, 99.95% over found pairs);
* **unconfirmed** -- the first base has an `engine` row or no row at all
  (934 rows over all pairs: 328 found-but-not-first, the audit list, and 606
  not-found recall misses).

The verb oracle (`data/kotus_verbit.sqlite`) has `ok IN ('both','omorfi')`;
the nominal oracle also has `engine`, which is the engine's own output and not
an authority. A row with no first base (the reference base was not found at
all) is counted as unconfirmed here; it is a recall miss, not a ranking miss.

Usage:

    python3 tools/report_multiple_bases.py reports/recall5.tsv
    python3 tools/report_multiple_bases.py reports/recall5.tsv --unconfirmed-out reports/unconfirmed_first.tsv

The TSV must come from a current `lookup_recall` (eight columns, including
`first_base`). The output is a dev-time report; the SQLite files are gitignored
and the engine never reads them.
"""

from __future__ import annotations

import argparse
import os
import sqlite3
import sys
from collections import Counter

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOMINAL_DB = os.path.join(REPO, "data", "kotus_nomines.sqlite")
VERB_DB = os.path.join(REPO, "data", "kotus_verbit.sqlite")

#: SQLite's expression-depth limit is 1000; a term is two parameters, so keep
#: each chunk comfortably below it.
CHUNK = 250

def read_rows(path: str) -> list[tuple[str, str, str, str, str, str, str, str]]:
    """Read the eight-column `lookup_recall` TSV.

    A seven-column file is the old format and cannot answer the question; say
    so rather than silently treating the missing column as an empty base.
    """
    rows: list[tuple[str, str, str, str, str, str, str, str]] = []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 7:
                raise SystemExit(
                    f"{path}: seven columns; regenerate it with the current "
                    "lookup_recall so it carries first_base"
                )
            if len(parts) != 8:
                raise SystemExit(f"{path}: unexpected line {line!r}")
            form, lemma, found, nbases, plain, ranked, len_ok, first_base = parts
            rows.append((form, lemma, found, nbases, plain, ranked, len_ok, first_base))
    return rows


def nominal_both() -> set[tuple[str, str]]:
    """Every `(form, lemma)` the nominal oracle double-confirms."""
    db = sqlite3.connect(f"file:{NOMINAL_DB}?mode=ro", uri=True)
    try:
        return {
            (form, lemma)
            for lemma, form in db.execute(
                "SELECT lemma, form FROM forms"
                " WHERE ok='both' AND possessive='' AND lemma<>form"
            )
        }
    finally:
        db.close()


def statuses(
    db_path: str, pairs: set[tuple[str, str]], table: str = "forms"
) -> dict[tuple[str, str], set[str]]:
    """The `ok` values for each `(form, lemma)` pair, in chunks."""
    if not pairs:
        return {}
    db = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    found: dict[tuple[str, str], set[str]] = {}
    work = list(pairs)
    try:
        for i in range(0, len(work), CHUNK):
            chunk = work[i : i + CHUNK]
            where = " OR ".join("(lemma=? AND form=?)" for _ in chunk)
            # `pairs` are (form, lemma); the SQL term is (lemma, form).
            params = [x for form, lemma in chunk for x in (lemma, form)]
            for lemma, form, ok in db.execute(
                f"SELECT lemma, form, ok FROM {table} WHERE {where}", params
            ):
                found.setdefault((form, lemma), set()).add(ok)
    finally:
        db.close()
    return found


def classify(
    form: str,
    reference: str,
    first_base: str,
    both: set[tuple[str, str]],
    nominal: dict[tuple[str, str], set[str]],
    verb: dict[tuple[str, str], set[str]],
) -> str:
    """The strongest thing either oracle says about the first base."""
    if not first_base:
        return "absent"
    pair = (form, first_base)
    # The reference pairs are all `both` by construction, so the common case
    # (first_base == reference) never has to touch SQLite.
    if pair in both:
        return "both nominal"
    n = nominal.get(pair, set())
    if "both" in n:
        return "both nominal"
    v = verb.get(pair, set())
    if "both" in v:
        return "both verb"
    if "omorfi" in n:
        return "omorfi nominal"
    if "omorfi" in v:
        return "omorfi verb"
    if "engine" in n:
        return "engine nominal"
    return "absent"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "rows",
        nargs="?",
        default=os.path.join(REPO, "reports", "recall5.tsv"),
        help="eight-column lookup_recall TSV (default: reports/recall5.tsv)",
    )
    parser.add_argument(
        "--unconfirmed-out",
        default=os.path.join(REPO, "reports", "unconfirmed_first.tsv"),
        help="write the unconfirmed first-base rows here",
    )
    parser.add_argument(
        "--wrong-first-out",
        default=os.path.join(REPO, "reports", "wrong_first.tsv"),
        help="write every found-but-not-first row here",
    )
    args = parser.parse_args()

    for path in (NOMINAL_DB, VERB_DB):
        if not os.path.exists(path):
            raise SystemExit(f"{path} is not present; this report needs both oracles")
    if not os.path.exists(args.rows):
        raise SystemExit(
            f"{args.rows} is not present; run "
            "`python3 tools/test_reverse_lookup_recall.py reports/recall5.tsv` first"
        )

    rows = read_rows(args.rows)
    both = nominal_both()

    # Only the first bases that are not already in the nominal `both` set need
    # a database question. That is the wrong-first rows plus the few rows where
    # the reference itself was not found, so the query stays small.
    need: set[tuple[str, str]] = set()
    for form, _lemma, _found, _nbases, _plain, _ranked, _len_ok, first in rows:
        if first and (form, first) not in both:
            need.add((form, first))
    nominal = statuses(NOMINAL_DB, need)
    verb = statuses(VERB_DB, need)

    exact = 0
    confirmed_alt = 0
    confirmed_alt_found = 0
    one_alt = 0
    one_alt_found = 0
    unconfirmed = 0
    unconfirmed_found = 0
    unconfirmed_not_found = 0
    no_base = 0
    n_found = 0
    by_status: Counter[str] = Counter()
    unconfirmed_rows: list[tuple[str, str, str, str, str, str]] = []
    wrong_first_rows: list[tuple[str, str, str, str, str]] = []

    for form, lemma, found, nbases, _plain, ranked, _len_ok, first in rows:
        status = classify(form, lemma, first, both, nominal, verb)
        by_status[status] += 1
        is_found = found == "1"
        if is_found:
            n_found += 1
        if ranked == "0" and is_found:
            wrong_first_rows.append((form, lemma, first, status, nbases))
        if ranked == "1":
            exact += 1
        elif status in ("both nominal", "both verb"):
            confirmed_alt += 1
            if is_found:
                confirmed_alt_found += 1
        elif status in ("omorfi nominal", "omorfi verb"):
            one_alt += 1
            if is_found:
                one_alt_found += 1
        else:
            unconfirmed += 1
            if is_found:
                unconfirmed_found += 1
            else:
                unconfirmed_not_found += 1
                if not first:
                    no_base += 1
            unconfirmed_rows.append((form, lemma, first, status, nbases, found))

    n = len(rows)
    confirmed = exact + confirmed_alt
    confirmed_found = exact + confirmed_alt_found
    one_authority = confirmed + one_alt
    one_authority_found = confirmed_found + one_alt_found
    pct = lambda v: v / n * 100 if n else 0.0  # noqa: E731
    pct_found = lambda v: v / n_found * 100 if n_found else 0.0  # noqa: E731

    print(f"rows: {n} ({n_found} found, {n - n_found} not found)")
    print(f"  exact reference lemma:              {exact:>8}  {pct(exact):6.2f}%")
    print(f"  a different both-confirmed base:    {confirmed_alt:>8}")
    print(f"  first base confirmed (all rows):    {confirmed:>8}  {pct(confirmed):6.2f}%")
    print(f"  a different one-authority base:     {one_alt:>8}")
    print(f"  first base confirmed by one oracle: {one_authority:>8}  {pct(one_authority):6.2f}%")
    print(f"  first base unconfirmed:             {unconfirmed:>8}  {pct(unconfirmed):6.2f}%")
    print()
    print("over the found rows only (the ranking question):")
    print(f"  exact reference lemma:              {exact:>8}  {pct_found(exact):6.2f}%")
    print(f"  first base confirmed (either way):  {confirmed_found:>8}  {pct_found(confirmed_found):6.2f}%")
    print(f"  first base confirmed by one oracle: {one_authority_found:>8}  {pct_found(one_authority_found):6.2f}%")
    print(f"  first base unconfirmed:             {unconfirmed_found:>8}  {pct_found(unconfirmed_found):6.2f}%")
    print()
    print(f"unconfirmed rows: {unconfirmed} = {unconfirmed_found} found-but-not-first"
          f" + {unconfirmed_not_found} not found")
    if no_base:
        print(f"  of the not-found rows, {no_base} have no first base at all")
    print()
    print("first base status:")
    for status, count in by_status.most_common():
        print(f"  {status:>16}  {count:>8}  {pct(count):6.2f}%")

    if args.unconfirmed_out:
        os.makedirs(os.path.dirname(args.unconfirmed_out) or ".", exist_ok=True)
        with open(args.unconfirmed_out, "w", encoding="utf-8") as fh:
            fh.write("form\treference\tfirst_base\tstatus\tnbases\tfound\n")
            for row in unconfirmed_rows:
                fh.write("\t".join(row) + "\n")
        print(f"\nunconfirmed rows: {args.unconfirmed_out} ({len(unconfirmed_rows)})")
    if args.wrong_first_out:
        os.makedirs(os.path.dirname(args.wrong_first_out) or ".", exist_ok=True)
        with open(args.wrong_first_out, "w", encoding="utf-8") as fh:
            fh.write("form\treference\tfirst_base\tstatus\tnbases\n")
            for row in wrong_first_rows:
                fh.write("\t".join(row) + "\n")
        print(f"found-but-not-first rows: {args.wrong_first_out} ({len(wrong_first_rows)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
