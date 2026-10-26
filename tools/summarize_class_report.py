#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Turn a class report into a triage list: which defect to fix first.

`report_by_class.py` says what each class gets wrong. `this` says which wrong
thing is *one* wrong thing. A hundred classes each with fifteen illegal forms
is not a hundred problems; it is a handful of bugs showing up in a hundred
places, and the fix for each is a different line of `src/expand/inflect.rs`.

The grouping key is deliberately a shape rather than a word. Two findings are
the same defect when they are the same status, the same case, the same
possessive, and the same pair of endings -- so `kissaksimme` (expected
`kissaksemme`) and `taloihimme` (expected `taloihin`) are different defects,
while `kissaksimme` and `kirjoitaksimme` are the same one. That is what makes
the list actionable: the "before" and "after" columns are literally the two
strings to choose between in the code.

Output is a table, most damaging defect first, with the classes each one
touches. `--json` emits the same grouping as an array, for feeding back into
something.

Usage:

    python3 tools/summarize_class_report.py reports/noun_classes.json
    python3 tools/summarize_class_report.py reports/noun_classes.json -n 20
    python3 tools/summarize_class_report.py reports/noun_classes.json -c TRA
    python3 tools/summarize_class_report.py reports/noun_classes.json --json
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    p.add_argument("report", help="the JSON array report_by_class.py wrote")
    p.add_argument("-n", "--top", type=int, default=25,
                   help="how many defects to list (default: 25)")
    p.add_argument("-c", "--case", default="",
                   help="only defects in this case, e.g. TRA")
    p.add_argument("-s", "--status", default="",
                   help="only this status: missing, not_plain, mistagged, "
                        "illegal, unsupported")
    p.add_argument("--include-confirmed", action="store_true",
                   help="tally the confirmed rows too. Off by default: they are "
                        "the class's rate, not a defect, and they outnumber the "
                        "real findings several times over")
    p.add_argument("--json", action="store_true",
                   help="emit the grouping as JSON instead of a table")
    p.add_argument("--examples", type=int, default=3,
                   help="example forms per defect (default: 3)")
    return p.parse_args(argv)


def tail_pair(lemma: str, want: str, got: str) -> tuple[str, str]:
    """The two endings that differ, for a form and the form it should have been.

    Only meaningful when the word really is the lemma plus an ending, which is
    the case for every finding the harness can produce: it compares surface
    forms of the same lemma, so whatever is not the lemma is the ending. If
    the endings come out equal the difference is inside the stem -- a gradation
    or a vowel change -- and returning them unchanged is honest, because then
    there is no ending-level difference to point at.

    `want` is a `/`-separated list when the oracle offered alternatives, and
    only the first is a candidate for comparison: `kissoikseen/kissoiksensa` is
    what a `-ksi` cell answers with, and taking it as one string would fail the
    `startswith` below and lose the endings for the single largest group of
    findings in a full sweep -- 3380 forms across 117 classes, all of them the
    same missing `i`.
    """
    first = want.split("/")[0]
    if not first.startswith(lemma) or not got.startswith(lemma):
        return ("", "")
    return (first[len(lemma):], got[len(lemma):])


def key_of(finding: dict) -> tuple:
    """The defect's shape, as a sort key and a table label.

    `expected` and `got` are what make two rows comparable, and they are only
    present on some statuses. A `missing` row has nothing torille produced to
    contrast, so its key is just the cell: "class 9 has no partitive
    possessive at all" is one defect, and the six lemmas that show it are
    evidence, not six defects.
    """
    cell = finding.get("cell", {})
    case = cell.get("case", "")
    poss = cell.get("possessive", "")
    status = finding.get("status", "")
    if status == "illegal":
        before, after = tail_pair(finding["lemma"], finding.get("expected", ""),
                                  finding.get("got", ""))
        # `got` is the wrong word and the oracle confirmed a different one, so
        # the pair is the whole story. `unsupported` has no such answer and
        # falls through to the cell-only key below.
        return (status, case, poss, before, after)
    if status == "mistagged":
        return (status, case, poss, " ".join(finding.get("tags", [])), "")
    return (status, case, poss, "", "")


def example(row: dict) -> str:
    """One finding as a line a person can act on.

    `expected` is the oracle's whole answer for the cell, which is two strings
    when the cell has two forms, so the row's own form is stripped out of it
    before it is shown: `kissakseen, or kissaksensa` says what else is a valid
    form, while restating the form back to the reader says nothing.
    """
    status = row.get("status", "")
    form = row.get("form", "")
    if status == "illegal":
        want = row.get("expected", "")
        if want:
            return f"{form} should be {want}"
        return f"{form}: offered, no such form in this paradigm"
    if status == "unsupported":
        return f"{form}: offered, and no oracle vouches for that cell"
    if status == "missing":
        want = [w for w in row.get("expected", "").split("/") if w and w != form]
        extra = f" (or {', '.join(want)})" if want else ""
        return f"{form}: torille produces nothing{extra}"
    if status == "mistagged":
        return f"{form}: tagged {' '.join(row.get('tags', [])) or '(nothing)'}"
    if status == "not_plain":
        return f"{form}: only at {row.get('confidence', '?')}"
    return f"{row.get('lemma')}: {form}"


def describe(key: tuple) -> str:
    status, case, poss, before, after = key
    label = f"{status:9} {case or '-':4}"
    if poss:
        label += f" poss {poss}"
    if before or after:
        label += f": got -{before}, wanted -{after}"
    elif status == "unsupported":
        label += ": offered, and no oracle vouches for the cell"
    elif status == "mistagged":
        label += f": tagged [{before}]"
    return label


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    reports = json.loads(Path(args.report).read_text(encoding="utf-8"))

    groups: dict[tuple, list[dict]] = defaultdict(list)
    for report in reports:
        for finding in report.get("findings", []):
            status = finding.get("status", "")
            if status == "confirmed" and not args.include_confirmed:
                continue
            if args.case and finding.get("cell", {}).get("case") != args.case:
                continue
            if args.status and status != args.status:
                continue
            # The class lives on the enclosing object, not on the finding, and
            # the whole point of the table is "which classes", so it is stamped
            # onto each row rather than carried alongside a second index that
            # would have to be kept in step.
            groups[key_of(finding)].append({**finding, "class": report["class"]})

    ordered = sorted(groups.items(),
                     key=lambda kv: (-len(kv[1]), kv[0]))

    if args.json:
        payload = [
            {
                "defect": describe(key),
                "status": key[0],
                "case": key[1],
                "possessive": key[2],
                "ending_before": key[3],
                "ending_after": key[4],
                "count": len(rows),
                "classes": sorted({r["class"] for r in rows}),
                "lemmas": sorted({r["lemma"] for r in rows}),
                "examples": [
                    {"lemma": r["lemma"], "form": r["form"],
                     "expected": r.get("expected", ""), "got": r.get("got", "")}
                    for r in rows[:args.examples]
                ],
            }
            for key, rows in ordered
        ]
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return 0

    if not ordered:
        print("no findings match that filter", file=sys.stderr)
        return 0

    total_classes = len(reports)
    print(f"{len(ordered)} distinct defects over {total_classes} classes\n")
    print("The counts are forms, not cells: Omorfi answers one cell with two "
          "strings\n(`kissakseen/kissaksensa`) and each is a row, so a case that "
          "is wholly\nabsent counts twice. The 'classes' column is the number to "
          "read for reach.\n")
    width = max(len(describe(k)) for k, _ in ordered[: args.top])
    for key, rows in ordered[: args.top]:
        classes = sorted({r["class"] for r in rows})
        shown = ", ".join(classes[:6])
        if len(classes) > 6:
            shown += f", +{len(classes) - 6} more"
        print(f"{len(rows):6}  {describe(key):{width}}  {len(classes):3} classes  "
              f"{shown}")
        for row in rows[: args.examples]:
            print(f"{'':6}  {'':{width}}    e.g. {example(row)}")
    hidden = len(ordered) - args.top
    if hidden > 0:
        print(f"\n{hidden} more defect(s) not shown; raise --top to see them")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
