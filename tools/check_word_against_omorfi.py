#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check one word's paradigm against Omorfi + Voikko, in both directions.

For each lemma, Omorfi is asked for every cell of its paradigm and Voikko is
asked whether each answer is a real form of that lemma. The forms that survive
both are the word's paradigm; torille's `inflect` is then run and compared
against it.

A lemma passes only if the comparison is exact in both directions:

* every confirmed form is offered as **plain** - produced but not plain is a
  failure just as much as not produced at all;
* no form torille offers as plain is outside the confirmed set - an illegal
  spelling offered as plain is a failure.

The exit status is 1 if any lemma fails. The two failure kinds are kept apart in
the output because they mean different things: a missing form is a gap in the
engine, an illegal one is a wrong claim.

Usage:
    python3 tools/check_word_against_omorfi.py [-p noun|verb] [-c CASES] [-n sg|pl|both] LEMMA...
"""

from __future__ import annotations

import argparse
import sys

from omorfi_harness import (
    ALL_CASES,
    Harness,
    noun_cells,
    verb_cells,
    eprint,
)

#: (tense, mood) pairs of the finite verb, in the order a reader expects them.
VERB_TENSE_MOODS = [("PRESENT", "INDV"), ("PAST", "INDV"), ("", "COND")]


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("lemmas", nargs="+", help="words to check")
    p.add_argument("-p", "--pos", choices=("noun", "verb"), default="noun",
                   help="part of speech to generate (default: noun)")
    p.add_argument("-c", "--cases", default="all",
                   help="comma-separated case codes, or 'all' "
                        f"({','.join(ALL_CASES)})")
    p.add_argument("-n", "--numbers", choices=("sg", "pl", "both"), default="both",
                   help="which numbers to compare (default: both)")
    p.add_argument("--no-possessives", action="store_true",
                   help="check only the bare case/number cells, skipping the "
                        "possessive paradigm. Faster, and it is the question "
                        "being about the cases")
    p.add_argument("--lenient", action="store_true",
                   help="report unconfirmed plain forms but do not fail on them")
    p.add_argument("--strict-tags", action="store_true",
                   help="also fail on a confirmed form offered as plain under the "
                        "wrong case or number")
    p.add_argument("-q", "--quiet", action="store_true",
                   help="print only the failing lemmas")
    return p.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    numbers = ["SG", "PL"] if args.numbers == "both" else [args.numbers.upper()]

    if args.pos == "noun":
        cases = ALL_CASES if args.cases == "all" else [
            c.strip().upper() for c in args.cases.split(",") if c.strip()
        ]
        bad = [c for c in cases if c not in ALL_CASES]
        if bad:
            eprint(f"unknown case code(s): {', '.join(bad)}")
            eprint(f"known: {','.join(ALL_CASES)}")
            return 2
        cells = noun_cells(cases, numbers, [""] if args.no_possessives else None)
    else:
        # A verb has no case to select; the numbers pick the person series.
        cells = verb_cells(VERB_TENSE_MOODS, numbers)

    harness = Harness(pos=args.pos)
    failed: list[str] = []
    checked = 0
    for lemma in args.lemmas:
        verdict = harness.check(lemma, cells, lenient=args.lenient)
        if verdict.skipped:
            if not args.quiet:
                print(verdict.report())
            continue
        checked += 1
        bad = not verdict.ok or (args.strict_tags and verdict.mistagged)
        if bad:
            failed.append(lemma)
            print(verdict.report())
            for form in sorted(verdict.missing):
                print(f"    MISSING   {form}")
            for form in sorted(verdict.not_plain):
                print(f"    NOT-PLAIN {form}")
            for form in sorted(verdict.mistagged):
                print(f"    MISTAGGED {form}")
            if not args.lenient:
                for form in sorted(verdict.illegal):
                    print(f"    ILLEGAL   {form}")
        elif not args.quiet:
            print(verdict.report())

    print(f"\n{checked} word(s) checked, {len(failed)} failed")
    if failed:
        print(f"failing: {', '.join(failed)}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
