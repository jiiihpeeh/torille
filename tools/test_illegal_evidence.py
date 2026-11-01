#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""The evidence gate for `illegal` rows.

The default materialised-oracle report is allowed to disagree with the engine,
but only where `data/oracle_exceptions.json` says so. This gate asserts two
things about the fixed 5-lemma sample:

1. every `illegal` row matches exactly one evidence rule;
2. every evidence rule still matches at least one row.

The second check is the staleness guard: when an engine fix or an oracle
rebuild removes a disagreement, its rule stops matching and this gate goes red.
That is deliberate -- a rule that no longer explains anything is either dead or
wrong, and silently keeping it is how a class-wide exemption starts.

Usage:

    python3 tools/test_illegal_evidence.py
    python3 tools/test_illegal_evidence.py --report reports/noun-classes.json
    python3 tools/test_illegal_evidence.py --evidence data/oracle_exceptions.json

The report must be the **default** one, not a `--verify-illegal` report: the
verified report has no `illegal` rows, so every rule would look stale.
"""

from __future__ import annotations

import argparse
import functools
import json
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import illegal_evidence  # noqa: E402
from omorfi_harness import SANALISTA, Cell, Finding  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
REPORT_TOOL = REPO / "tools" / "report_by_class.py"


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    p.add_argument("--report", metavar="JSON",
                   help="a default (non---verify-illegal) report; generated "
                        "with -n 5 -j 8 if omitted")
    p.add_argument("-n", "--per-class", type=int, default=5,
                   help="lemmas per class the report was (or will be) built "
                        "with; rules marked for a wider sample are not called "
                        "stale by a narrower one")
    p.add_argument("--evidence", metavar="JSON",
                   default=str(illegal_evidence.DEFAULT_PATH),
                   help="the curated evidence file")
    p.add_argument("--lexicon", metavar="SQLITE",
                   default=str(REPO / "data" / "kotus_nomines.sqlite"),
                   help="the materialised oracle the report was built from")
    p.add_argument("-j", "--jobs", type=int, default=8,
                   help="classes to check at once when generating the report")
    return p.parse_args(argv)


@functools.lru_cache(maxsize=None)
def class_of(lemma: str) -> str:
    """The sanalista class string for a lemma, read once per lemma.

    The report tool asks the harness; the gate reads the source directly so it
    does not need to open the 2.4 GB oracle just to answer the `also-class`
    rule.
    """
    with open(SANALISTA, encoding="utf-8") as fh:
        next(fh, None)
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if parts and parts[0] == lemma:
                return parts[3] if len(parts) >= 4 else ""
    return ""


def as_finding(row: dict) -> Finding:
    """The report's JSON row as the harness's `Finding`."""
    cell = row.get("cell") or {}
    return Finding(
        lemma=row["lemma"],
        cell=Cell(
            case=cell.get("case", ""),
            number=cell.get("number", ""),
            tense=cell.get("tense", ""),
            mood=cell.get("mood", ""),
            person=cell.get("person", ""),
            possessive=cell.get("possessive", ""),
        ),
        status=row.get("status", ""),
        form=row.get("form", ""),
        expected=row.get("expected", ""),
    )


def generate_report(path: str, per_class: int, jobs: int,
                    lexicon: str) -> None:
    """Run the default report into `path`; a defect exit (1) is expected."""
    command = [
        sys.executable, str(REPORT_TOOL),
        "-n", str(per_class), "-j", str(jobs), "-o", path,
        "--lexicon", lexicon, "--raw-illegal",
    ]
    result = subprocess.run(command, check=False)
    if result.returncode not in (0, 1):
        raise SystemExit(
            f"report_by_class.py exited {result.returncode}; see its output")
    if not Path(path).exists():
        raise SystemExit(f"report_by_class.py did not write {path}")


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    try:
        rules = illegal_evidence.load(args.evidence)
    except illegal_evidence.EvidenceError as exc:
        print(f"evidence: {exc}", file=sys.stderr)
        return 2

    generated = args.report is None
    if args.report:
        report_path = args.report
    else:
        with tempfile.NamedTemporaryFile(
                prefix="illegal-evidence-", suffix=".json",
                delete=False) as handle:
            report_path = handle.name
        generate_report(
            report_path, args.per_class, args.jobs, args.lexicon)

    try:
        report = json.loads(Path(report_path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"{report_path}: {exc}", file=sys.stderr)
        return 2
    finally:
        if generated:
            Path(report_path).unlink(missing_ok=True)
    if not isinstance(report, list):
        print(f"{report_path}: expected a JSON array of classes",
              file=sys.stderr)
        return 2

    illegal = 0
    hits: dict[str, int] = {rule["id"]: 0 for rule in rules}
    unmatched: list[tuple] = []
    ambiguous: list[tuple] = []

    for entry in report:
        cls = entry.get("class", "")
        for row in entry.get("findings", []):
            if row.get("status") != "illegal":
                continue
            illegal += 1
            finding = as_finding(row)
            matched = illegal_evidence.match(
                rules, finding, cls, class_of)
            if not matched:
                unmatched.append((
                    cls, finding.lemma, finding.form, finding.expected,
                    finding.cell.describe(),
                ))
                continue
            for rule in matched:
                hits[rule["id"]] += 1
            if len(matched) > 1:
                ambiguous.append((
                    cls, finding.lemma, finding.form,
                    [rule["id"] for rule in matched],
                ))

    # The `missing` side: a rule can explain an oracle spelling that is not a
    # word (`kuurompalta` for the standard `kuurommalta`). Those rules are
    # checked against the report's `missing` rows so one cannot go stale.
    missing = 0
    for entry in report:
        cls = entry.get("class", "")
        for row in entry.get("findings", []):
            if row.get("status") != "missing":
                continue
            missing += 1
            finding = as_finding(row)
            for rule in illegal_evidence.match(rules, finding, cls, class_of):
                hits[rule["id"]] += 1

    # The `unsupported` side: a rule can say the engine's form is right even
    # though neither oracle lists it (`piitä` for `pii`). Fed to the same
    # staleness count so an unsupported-only rule cannot go stale unnoticed.
    unsupported = 0
    for entry in report:
        cls = entry.get("class", "")
        for row in entry.get("findings", []):
            if row.get("status") != "unsupported":
                continue
            unsupported += 1
            finding = as_finding(row)
            for rule in illegal_evidence.match(rules, finding, cls, class_of):
                hits[rule["id"]] += 1

    stale = [
        rule["id"] for rule in rules
        if illegal_evidence.sample_of(rule) <= args.per_class
        and hits[rule["id"]] == 0
    ]

    print(f"illegal rows: {illegal}")
    print(f"missing rows: {missing}")
    print(f"unsupported rows: {unsupported}")
    print(f"evidence rules: {len(rules)} "
          f"(checked for staleness at n={args.per_class})")

    failed = False
    if illegal == 0:
        print(
            "this report has no illegal rows; run the gate against the "
            "default report, not a --verify-illegal report",
            file=sys.stderr,
        )
        failed = True
    if unmatched:
        failed = True
        print(f"\n{len(unmatched)} illegal row(s) with no evidence rule:",
              file=sys.stderr)
        for cls, lemma, form, expected, cell in unmatched:
            print(f"  {cls:8} {lemma}: {form} -> {expected} ({cell})",
                  file=sys.stderr)
    if ambiguous:
        failed = True
        print(f"\n{len(ambiguous)} illegal row(s) matched by more than one "
              "rule:", file=sys.stderr)
        for cls, lemma, form, rule_ids in ambiguous:
            print(f"  {cls:8} {lemma}: {form} -> {', '.join(rule_ids)}",
                  file=sys.stderr)
    if stale:
        failed = True
        print(f"\n{len(stale)} evidence rule(s) no longer match anything:",
              file=sys.stderr)
        for rule_id in stale:
            print(f"  {rule_id}", file=sys.stderr)

    if failed:
        return 1
    print("OK: every illegal row is explained and every in-scope rule is live")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
