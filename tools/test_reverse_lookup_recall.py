# SPDX-License-Identifier: MIT OR Apache-2.0
"""The reverse lookup has to name the base form the reference names.

`lookup` answers "which word is this form of", and until now nothing checked
that its answer agreed with anything. The fingerprint cannot: it digests the
*forward* direction -- how many forms each lemma makes -- so an index that drops
a form on the way in, or names the wrong base on the way out, leaves the digest
byte-identical. This is the measurement that does see it.

It measures two things and separates them, because they are two questions:

* **recall** -- is the lemma the reference names among the word's base forms.
  This is the direction the engine gets credit for. 5.4% of the reference's words
  genuinely have two base forms, and a gate that scored only the first base
  would be measuring ambiguity rather than the engine.
* **the first base** -- reported under the sanalista's order and under
  `ClassPrior`, so the ranking is measured against the alternative it replaces
  instead of argued for. The gate fails if the ranking is *worse*; a ranking that
  is no better is not a failure, and the numbers are printed so a human can
  judge whether it is carrying its weight.

Recall is the only sound direction here, for the reason `compound_recall.py`
gives: the reference does not list a form as wrong, so a form absent from it is
unconfirmed rather than incorrect, and precision cannot be measured this way at
all. It compares like with like -- `ok='both'` (the only double-confirmed value
the column has) and `possessive=''` -- and drops the nominative, where the base
form and the form are the same word and the pair says nothing.

Thresholds are floors with headroom under the measured baseline, not the
baseline itself: a defect that costs less than the headroom will pass, and the
point of a floor is to catch a regression rather than to pin a number. Re-baseline
only when this gate or a data audit finds a defect, and say so in the commit.

Run from the repository root, with the reference present:

    python3 tools/test_reverse_lookup_recall.py

It builds `lookup_recall`, which indexes every lemma in the sanalista, so it
takes the better part of a minute. The per-pair results are written next to it
for inspection:

    python3 tools/test_reverse_lookup_recall.py OUT.tsv
"""

from __future__ import annotations

import os
import sqlite3
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DB = os.path.join(REPO, "data", "kotus_nomines.sqlite")

#: The right base has to be among the answers for at least this share of the
#: reference's `(lemma, form)` pairs.
#:
#: `a93f744` measured 99.79%. The class-38 loanword harmony fix came next and
#: recovered exactly the 322 pairs that cause was worth, moving it to 99.85%, so
#: the floor follows to 99.8%. That is the point of a floor -- a fix should
#: tighten it -- and it is still not the measured number, so there is room to
#: fall.
#:
#: The 0.05 points of headroom above the floor is 313 pairs. The open defects
#: the remaining costs come to 965 pairs, more than that, so this gate still will
#: not catch them; it catches a *regression*, which is what it is for. Do not
#: raise it to the measured figure without fixing something.
RECALL_FLOOR = 0.998


def reference_pairs() -> list[tuple[str, str]]:
    """Every `(lemma, form)` the reference double-confirms, deduplicated.

    A form is asked about once: the reference lists a word once per homonym and
    per spelling, and a duplicate would weigh the words that happen to be
    ambiguous more heavily than the ones that are not.
    """
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    rows = db.execute(
        "SELECT lemma, form FROM forms"
        " WHERE ok='both' AND possessive='' AND lemma<>form"
    )
    seen: set[tuple[str, str]] = set()
    for lemma, form in rows:
        seen.add((form, lemma))
    return sorted(seen)


def build_binary() -> str:
    """Path to `lookup_recall`, built once.

    Not `target/release/torille`: that is the 13-byte stub, and the tools are
    separate binaries in `torille-tools`.
    """
    subprocess.run(
        [
            "cargo", "build", "-q", "--release",
            "-p", "torille-tools", "--bin", "lookup_recall",
        ],
        cwd=REPO, check=True,
    )
    built = os.path.join(REPO, "target", "release", "lookup_recall")
    if not os.path.exists(built):
        raise SystemExit(f"{built} was not produced by the build")
    return built


def measure(binary: str, pairs: list[tuple[str, str]], out_path: str | None):
    """Runs the tool and reads its counts back, writing the raw rows if asked."""
    with tempfile.TemporaryDirectory() as tmp:
        pairs_path = os.path.join(tmp, "pairs.tsv")
        with open(pairs_path, "w", encoding="utf-8") as fh:
            for form, lemma in pairs:
                fh.write(f"{form}\t{lemma}\n")
        rows_path = os.path.join(tmp, "rows.tsv")
        with open(pairs_path, encoding="utf-8") as fin, open(rows_path, "wb") as fout:
            subprocess.run([binary], stdin=fin, stdout=fout, check=True)
        totals = {"n": 0, "found": 0, "plain": 0, "ranked": 0, "len_bad": 0, "amb": 0}
        with open(rows_path, encoding="utf-8") as fh:
            for line in fh:
                parts = line.rstrip("\n").split("\t")
                if len(parts) != 8:
                    raise SystemExit(f"unexpected tool output: {line!r}")
                form, _lemma, found, nbases, plain, ranked, len_ok, _first_base = parts
                totals["n"] += 1
                totals["found"] += int(found)
                totals["plain"] += int(plain)
                totals["ranked"] += int(ranked)
                totals["len_bad"] += 1 - int(len_ok)
                totals["amb"] += int(nbases) > 1
        if out_path:
            with open(rows_path, encoding="utf-8") as src, open(out_path, "w", encoding="utf-8") as dst:
                dst.write(src.read())
    return totals


def main() -> int:
    if not os.path.exists(DB):
        raise SystemExit(f"{DB} is not present; this gate needs the reference")
    pairs = reference_pairs()
    print(f"reference (lemma, form) pairs: {len(pairs)}")
    # The documented clean build has 640 873 `(lemma, form)` pairs. A database
    # built with the old `--engine` default wrote engine rows as `ok='both'`,
    # which inflates the reference and makes this gate measure the engine
    # against an older copy of itself. Refuse that reference instead of
    # reporting a meaningless recall number.
    if len(pairs) > 1_000_000:
        raise SystemExit(
            f"the reference has {len(pairs)} pairs, far above the clean "
            "baseline (640 873). This database was probably built with the old "
            "`--engine` default, which marked engine rows as `ok='both'`. "
            "Rebuild it with `--no-engine` (or a current `--engine`, which "
            "writes `ok='engine'`) before running this gate."
        )
    binary = build_binary()
    totals = measure(binary, pairs, sys.argv[1] if len(sys.argv) > 1 else None)

    n = totals["n"]
    if n == 0:
        raise SystemExit("the tool reported no pairs; the reference query returned nothing")
    recall = totals["found"] / n
    plain = totals["plain"] / n
    ranked = totals["ranked"] / n
    print(f"  the right base is among the answers  {totals['found']:>8}  {recall * 100:6.2f}%")
    print(f"  first base right, sanalista order   {totals['plain']:>8}  {plain * 100:6.2f}%")
    print(f"  first base right, ranked            {totals['ranked']:>8}  {ranked * 100:6.2f}%")
    print(f"  words with more than one base form  {totals['amb']:>8}  {totals['amb'] / n * 100:6.2f}%")

    failures = []
    if recall < RECALL_FLOOR:
        failures.append(
            f"recall {recall * 100:.2f}% is below the {RECALL_FLOOR * 100:.1f}% floor"
        )
    if ranked < plain:
        failures.append(
            f"the ranking made the first base worse: {ranked * 100:.2f}% against "
            f"{plain * 100:.2f}% in the sanalista's order"
        )
    if totals["len_bad"]:
        failures.append(
            f"{totals['len_bad']} words came back with a different number of base "
            "forms after ranking; the ranking must reorder and never filter"
        )
    for f in failures:
        print(f"FAIL: {f}", file=sys.stderr)
    if ranked > plain:
        print(
            f"\nthe ranking is worth {(ranked - plain) * 100:.2f} points over the "
            "sanalista's own order"
        )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
