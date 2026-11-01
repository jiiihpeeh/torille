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
    python3 tools/report_by_class.py --verify-illegal   # live Voikko + curated rules
    python3 tools/report_by_class.py --raw-illegal      # oracle's raw illegal rows

The default report applies `data/oracle_exceptions.json`; `--raw-illegal` turns
that off for the evidence gate. Exit is 1 if any class has a defect, so it works
as a gate.
"""

from __future__ import annotations

import argparse
import json
import random
import sys
import threading
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from omorfi_harness import (  # noqa: E402
    ALL_CASES,
    BOUNDARY_MARKER,
    Finding,
    Harness,
    eprint,
    noun_cells,
)
import illegal_evidence  # noqa: E402

STATUSES = ("confirmed", "missing", "oracle-only", "not_plain", "mistagged",
            "illegal", "unsupported")


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
    p.add_argument("--verify-illegal", action="store_true",
                   help="re-check illegal rows with live Voikko and the "
                        "curated rules in data/oracle_exceptions.json, and "
                        "count a form as confirmed when either vouches for it. "
                        "This separates a real engine error from a materialised "
                        "oracle that disagrees with the engine.")
    p.add_argument("--strict-illegal", action="store_true",
                   help="keep every illegal row illegal and annotate it with "
                        "the evidence rule that would have reclassified it. "
                        "This is the audit mode for reviewing a new rule; it "
                        "does not need live Voikko.")
    p.add_argument("--raw-illegal", action="store_true",
                   help="do not apply data/oracle_exceptions.json; report the "
                        "oracle's raw illegal rows. This is the input the "
                        "evidence gate checks, and it cannot be combined with "
                        "--verify-illegal or --strict-illegal.")
    p.add_argument("--classless", action="store_true",
                   help="also audit the rows the sanalista gives no type number, "
                        "under the class '(none)'. Kotus omits the type of a "
                        "compound because it declines like its final component; "
                        "the engine inflects it through compound_head_forms.")
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
        # The harness spells this status `not plain`; the totals and the other
        # tools use `not_plain`, so the JSON keeps one spelling for both.
        "status": f.status.replace(" ", "_"),
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
    verifier=None,
    verifier_lock=None,
    evidence: list[dict] | None = None,
    strict_illegal: bool = False,
    raw_illegal: bool = False,
) -> dict:
    findings: list[Finding] = []
    skipped = 0
    for lemma in lemmas:
        rows = harness.audit(lemma, cells)
        if not rows:
            skipped += 1
            continue
        findings.extend(rows)

    class_of_cache: dict[str, str] = {}

    def class_of(lemma: str) -> str:
        if lemma not in class_of_cache:
            class_of_cache[lemma] = harness.class_of(lemma)
        return class_of_cache[lemma]

    # The live second opinion. Strict mode deliberately skips it: the whole
    # point of an audit is to see the raw illegal rows and what the evidence
    # file says about them, not to watch them disappear.
    if verifier is not None and not strict_illegal:
        for finding in findings:
            if finding.status != "illegal" or not finding.form:
                continue
            word = finding.form.replace(BOUNDARY_MARKER, "")
            if verifier_lock is None:
                analyses = verifier.analyze(word)
            else:
                with verifier_lock:
                    analyses = verifier.analyze(word)
            if any(
                analysis.get("BASEFORM") == finding.lemma
                or finding.lemma in analysis.get("WORDBASES", "")
                for analysis in analyses
            ):
                finding.status = "confirmed"
                finding.detail = (
                    "offered as plain; live Voikko reads it as this lemma, "
                    "while the materialised oracle expects "
                    f"{finding.expected}"
                )

    if evidence and strict_illegal:
        for finding in findings:
            if finding.status != "illegal":
                continue
            matched = illegal_evidence.match(
                evidence, finding, cls, class_of)
            note = (
                "would be reclassified by "
                + ", ".join(rule["id"] for rule in matched)
                if matched else "no evidence rule"
            )
            finding.detail = (
                f"{finding.detail} [{note}]" if finding.detail
                else f"[{note}]"
            )
    elif evidence and not raw_illegal:
        for finding in findings:
            if finding.status not in ("illegal", "unsupported"):
                continue
            matched = illegal_evidence.match(
                evidence, finding, cls, class_of)
            if not matched:
                continue
            rule = matched[0]
            extra = ""
            if len(matched) > 1:
                extra = (
                    " (also matched "
                    + ", ".join(r["id"] for r in matched[1:])
                    + ")"
                )
            was = finding.status
            finding.status = "confirmed"
            if was == "illegal":
                finding.detail = (
                    "offered as plain; "
                    f"{rule['note']} "
                    f"[{rule['id']}; {rule['source']}]{extra}, while the "
                    f"materialised oracle expects {finding.expected}"
                )
            else:
                finding.detail = (
                    "offered as plain; "
                    f"{rule['note']} "
                    f"[{rule['id']}; {rule['source']}]{extra}, though neither "
                    "oracle lists it"
                )
        # A `missing` row is the other direction: the oracle claims a form the
        # engine does not produce. Where the evidence says the oracle's spelling
        # is not a word (a comparative's strong grade in a weak cell), the row
        # is the oracle's mistake, not a recall gap, so it is not counted as
        # missing.
        for finding in findings:
            if finding.status != "missing":
                continue
            matched = illegal_evidence.match(
                evidence, finding, cls, class_of)
            if not matched:
                continue
            rule = matched[0]
            finding.status = "oracle-only"
            finding.detail = (
                "the oracle's expected form is not a word; "
                f"{rule['note']} [{rule['id']}; {rule['source']}]"
            )
    counts = Counter(f.status.replace(" ", "_") for f in findings)
    # A class is healthy only if every cell it answered was right, and that
    # includes the cells nothing vouches for: an unbacked `plain` suggestion is
    # a risk even when no oracle can say what the right answer was, so
    # `unsupported` fails the class. `confirmed` is in the totals so the rate is
    # readable -- 1204 confirmed and 16 missing is a 99% class, and 0 confirmed
    # means something is badly wrong rather than that the class is clean.
    failed = sum(
        counts[s] for s in STATUSES
        if s not in ("confirmed", "oracle-only")
    )
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
    # `--bare` asks only the case/number cells. This works for both the live
    # harness and the materialised lexicon: the latter stores the whole grid,
    # but a cell is still filtered by the cell list the harness is asked for.
    possessives = [""] if args.bare else None
    cells = noun_cells(ALL_CASES, ["SG", "PL"], possessives)

    if args.raw_illegal and (args.verify_illegal or args.strict_illegal):
        eprint("--raw-illegal cannot be combined with --verify-illegal "
               "or --strict-illegal")
        return 2
    harness = Harness(pos="noun", lexicon=args.lexicon)
    verifier = None
    verifier_lock = None
    evidence = None
    if not args.raw_illegal:
        try:
            evidence = illegal_evidence.load()
        except illegal_evidence.EvidenceError as exc:
            eprint(f"evidence: {exc}")
            return 2
    if args.verify_illegal:
        try:
            from voikko import libvoikko
            verifier = libvoikko.Voikko("fi")
            verifier_lock = threading.Lock()
        except Exception as exc:  # pragma: no cover - depends on the host
            eprint(f"--verify-illegal: live Voikko unavailable ({exc})")
            # A verified run that silently did not verify is worse than no
            # run: the curated rules would still be applied, but the live
            # second opinion would be missing without saying so.
            return 2
    buckets = harness.words_in_class("substantiivi", args.classless)

    def class_key(c: str) -> tuple[int, str]:
        # `(none)` is not a number: it sorts after every real class.
        if c == "(none)":
            return (10**9, c)
        return (int(c.split("*")[0]), c)
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
        selected = sorted(buckets, key=class_key)

    rng = random.Random(args.seed)
    picked: dict[str, list[str]] = {}
    for cls in selected:
        words = sorted(buckets[cls])
        rng.shuffle(words)
        picked[cls] = words[: args.per_class]

    def work(cls: str) -> dict:
        return check_class(
            harness, cls, picked[cls], cells, args.only_failures,
            verifier, verifier_lock, evidence, args.strict_illegal,
            args.raw_illegal,
        )

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

    reports.sort(key=lambda r: class_key(r["class"]))

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