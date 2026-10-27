#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""A per-inflection-class report: what each noun class gets right and wrong.

`check_word_against_omorfi.py` answers about a word. This answers about a
*class*, which is the question that gets a fix made. "The translative does not
elide before a possessive suffix" is one line of code; "seventeen words are
slightly wrong" is not a bug report.

The unit is the class as torille reads it, so `9` and `9*B` are separate classes
and stay that way: the `*B` is a consonant gradation, so `kauppa` (`9*B`) has
`kaupan` where `kala` (`9`) has `kalan`, and lumping them would report every
kauppa form as a defect of the `9` class.

Output is a JSON array, one object per class, in class order:

    {
      "class": "9*B",
      "lemmas_checked": 8,
      "lemmas_skipped": 1,
      "totals": {"confirmed": 1204, "missing": 16, ...},
      "ok": false,
      "findings": [
        {"lemma": "kauppa", "form": "kaupaksemme",
         "status": "illegal",
         "cell": {"case": "TRA", "number": "SG", "possessive": "PL1"},
         "expected": "kaupaksemme", "got": "kaupaksimme",
         "confidence": "plain", "tags": ["noun", "translative", ...],
         "detail": "..."}
      ]
    }

`confirmed` findings are included deliberately. A report of only defects cannot
be read as "the rest is fine" -- the class may have had nine cells and eight are
missing, and the reader cannot tell that from a list of nine bad ones.

The unit is the *word*, not the cell, because a surface form is often the answer
for several cells at once: `kissani` is both the nominative and the genitive
singular of a noun, so two cells generate it. One row per cell would report one
correct form twice and inflate every count. Where more than one cell expects the
form -- or where torille's reading leaves a field out, which is the same problem
seen from the other side -- the row carries `candidates` and names them all
rather than picking one.

The four failure statuses are the same as the per-word tool's, but per cell and
with `expected`, so `kissaksimme` reports as

    {"form": "kissaksimme", "expected": "kissaksemme",
     "cell": {"case": "TRA", "number": "SG", "possessive": "PL1"}}

which says what the defect is, where a bare "illegal: kissaksimme" does not.

Usage:

    python3 tools/report_by_class.py                    # every class, 5 lemmas each
    python3 tools/report_by_class.py -n 20              # 20 lemmas per class
    python3 tools/report_by_class.py -c 9,9B,27         # just these
    python3 tools/report_by_class.py --bare             # bare case/number cells
    python3 tools/report_by_class.py --live             # live Omorfi+Voikko
    python3 tools/report_by_class.py -o report.json
    python3 tools/report_by_class.py --split reports/   # one file per class

Exit is 1 if any class has a defect, so it works as a gate.
"""

from __future__ import annotations

import argparse
import json
import random
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from omorfi_harness import (  # noqa: E402
    ALL_CASES,
    Finding,
    Harness,
    eprint,
    noun_cells,
)

STATUSES = ("confirmed", "missing", "not_plain", "mistagged", "illegal",
            "unsupported")


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    p.add_argument("-n", "--per-class", type=int, default=5,
                   help="lemmas to check per class (default: 5)")
    p.add_argument("-c", "--classes", default="all",
                   help="comma-separated class numbers to report, or 'all'")
    p.add_argument("-j", "--jobs", type=int, default=4,
                   help="classes to check at once (default: 4)")
    p.add_argument("--seed", type=int, default=0,
                   help="seed for picking which lemmas of a class to check")
    p.add_argument("--bare", action="store_true",
                   help="check only the bare case/number cells, not the "
                        "possessive paradigm")
    p.add_argument("--lexicon", metavar="SQLITE",
                   default="data/kotus_nomines.sqlite",
                   help="noun reference to use (default: "
                        "data/kotus_nomines.sqlite). The reference is the "
                        "database's ok='both' forms; see docs/lexicon.md")
    p.add_argument("--live", action="store_true",
                   help="use live Omorfi+Voikko instead of the materialised "
                        "lexicon")
    p.add_argument("--only-failures", action="store_true",
                   help="omit the confirmed findings from the report")
    p.add_argument("-o", "--output", help="write the JSON here instead of stdout")
    p.add_argument("--split", metavar="DIR",
                   help="also write one JSON file per class into DIR, named "
                        "after the class: class-9.json, class-9B.json")
    p.add_argument("-q", "--quiet", action="store_true",
                   help="print only the failing classes")
    return p.parse_args(argv)


def cell_json(cell) -> dict:
    """A cell as JSON, in Omorfi's spellings since that is what the grid is.

    The keys are always present, empty string included: a consumer can then read
    `cell["possessive"]` without first testing whether the key exists, which is
    the sort of defensive code that quietly treats a missing key as a different
    cell instead of as a bare one.
    """
    return {
        "case": cell.case,
        "number": cell.number,
        "possessive": cell.possessive,
        "tense": cell.tense,
        "mood": cell.mood,
        "person": cell.person,
    }


def finding_json(f: Finding) -> dict:
    out = {
        "lemma": f.lemma,
        "form": f.form,
        "status": f.status,
        "cell": cell_json(f.cell),
        "expected": f.expected,
        "got": f.got,
        "confidence": f.confidence,
        "tags": f.tags,
    }
    if f.detail:
        out["detail"] = f.detail
    # `candidates` is only interesting when there is more than one, which is
    # either because Omorfi answers several cells with the same word or because
    # the reading lacks a field. One candidate is the cell, already given.
    if len(f.candidates) > 1:
        out["candidates"] = [cell_json(c) for c in f.candidates]
    return out


def check_class(
    harness: Harness,
    cls: str,
    lemmas: list[str],
    cells: list,
    only_failures: bool,
) -> dict:
    findings: list[Finding] = []
    skipped = 0
    for lemma in lemmas:
        rows = harness.audit(lemma, cells)
        if not rows:
            skipped += 1
            continue
        findings.extend(rows)
    counts = Counter(f.status for f in findings)
    # A class is healthy only if every cell it answered was right, and that
    # includes the cells nothing vouches for: an unbacked `plain` suggestion is
    # a risk even when no oracle can say what the right answer was, so
    # `unsupported` fails the class. `confirmed` is in the totals so the rate is
    # readable -- 1204 confirmed and 16 missing is a 99% class, and 0 confirmed
    # means something is badly wrong rather than that the class is clean.
    failed = sum(counts[s] for s in STATUSES if s != "confirmed")
    rows = [
        finding_json(f) for f in findings
        if not only_failures or f.status != "confirmed"
    ]
    return {
        "class": cls,
        "lemmas_checked": len(lemmas) - skipped,
        "lemmas_skipped": skipped,
        "totals": {s: counts.get(s, 0) for s in STATUSES},
        "ok": failed == 0,
        "findings": rows,
    }


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    if args.live:
        args.lexicon = None
    elif args.lexicon and not Path(args.lexicon).exists():
        # A checkout without the materialised oracle still runs; the live
        # reference is slower and can be unavailable, but it is the only one.
        eprint(f"{args.lexicon}: no such lexicon; using live Omorfi+Voikko")
        args.lexicon = None
    # A lexicon reference holds the whole grid; asking it for the bare grid
    # would let in_scope/acceptable filter possessed readings whose reference
    # set still includes them, so `--bare` is ignored under `--lexicon`.
    possessives = [""] if (args.bare and not args.lexicon) else None
    cells = noun_cells(ALL_CASES, ["SG", "PL"], possessives)

    harness = Harness(pos="noun", lexicon=args.lexicon)
    buckets = harness.words_in_class("substantiivi")
    if args.classes != "all":
        wanted = [c.strip() for c in args.classes.split(",") if c.strip()]
        # `9B` and `9*B` are the same class written two ways; accept both.
        canonical = {w.replace("*", "").upper(): w for w in buckets}
        selected: list[str] = []
        for w in wanted:
            key = w.replace("*", "").upper()
            if key in canonical:
                selected.append(canonical[key])
            elif w in buckets:
                selected.append(w)
            else:
                eprint(f"unknown class: {w}")
                eprint(f"known: {', '.join(sorted(buckets))}")
                return 2
    else:
        selected = sorted(buckets, key=lambda c: (int(c.split("*")[0]), c))

    rng = random.Random(args.seed)
    picked: dict[str, list[str]] = {}
    for cls in selected:
        words = sorted(buckets[cls])
        rng.shuffle(words)
        picked[cls] = words[: args.per_class]

    def work(cls: str) -> dict:
        return check_class(harness, cls, picked[cls], cells, args.only_failures)

    # Classes are independent, so the parallelism is over classes rather than
    # over lemmas: each worker walks a whole class. The single shared harness
    # serialises the Omorfi and Voikko calls under its own lock, so `-j` buys
    # the parts of a lemma that are torille's own work rather than the oracle's.
    total = len(selected)
    jobs = max(1, min(args.jobs, total))
    reports: list[dict] = []
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        for report in pool.map(work, selected):
            reports.append(report)
            if not args.quiet or not report["ok"]:
                t = report["totals"]
                bits = ", ".join(f"{s}={t[s]}" for s in STATUSES if t[s])
                print(
                    f"class {report['class']:6} "
                    f"{report['lemmas_checked']:3} lemmas  {bits}",
                    file=sys.stderr,
                )

    reports.sort(key=lambda r: (int(r["class"].split("*")[0]), r["class"]))

    if args.output:
        Path(args.output).write_text(
            json.dumps(reports, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8")
    else:
        print(json.dumps(reports, ensure_ascii=False, indent=2))

    if args.split:
        # One file per class, for the case where the question is about a single
        # class and reading the other 145 out of a 40MB array is not the way.
        # Each file holds a one-element array, not a bare object, so a consumer
        # written against the combined file reads a split file unchanged.
        outdir = Path(args.split)
        outdir.mkdir(parents=True, exist_ok=True)
        for report in reports:
            name = "class-" + report["class"].replace("*", "") + ".json"
            (outdir / name).write_text(
                json.dumps([report], ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8")
        eprint(f"{len(reports)} file(s) -> {outdir}/")

    bad = [r["class"] for r in reports if not r["ok"]]
    where = f" -> {args.output}" if args.output else ""
    eprint(f"\n{len(reports)} class(es) reported{where}, {len(bad)} with defects"
           + (f": {', '.join(bad[:20])}" if bad else ""))
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))