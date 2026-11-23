#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Measure the word wizard against the materialised noun oracle.

The oracle is `data/kotus_nomines.sqlite`: every `ok='both'` row is a form
Omorfi generated and Voikko confirmed (see docs/lexicon.md). For a sample of
sanalista nouns this script:

  1. reads the class the sanalista records and the confirmed cells;
  2. asks `Engine.class_candidates` for the adaptive next question, answers it
     from the oracle, and repeats up to `--answers` times;
  3. repeats the same cells in a fixed order;
  4. reports top-1/top-3 base-class and exact-expression accuracy at each
     answer budget.

The inference is the engine; the oracle is the authority, so this is the
outward number the plan asks for. It is a development tool, not a gate:
`--limit` keeps the runtime sane.

Usage:
    python3 tools/wizard_eval.py --limit 200
    python3 tools/wizard_eval.py --limit 50 --answers 3 --json
"""

from __future__ import annotations

import argparse
import json
import random
import re
import sqlite3
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_DB = REPO / "data" / "kotus_nomines.sqlite"

#: The cells the wizard can ask, in the fixed-order comparison. The adaptive
#: run uses the question the engine picks instead.
CELLS = ["gen_sg", "par_sg", "ill_sg", "nom_pl", "gen_pl", "par_pl", "ill_pl"]

CASE_KEY = {
    ("nominative", "singular"): "nom_sg",
    ("genitive", "singular"): "gen_sg",
    ("partitive", "singular"): "par_sg",
    ("illative", "singular"): "ill_sg",
    ("nominative", "plural"): "nom_pl",
    ("genitive", "plural"): "gen_pl",
    ("partitive", "plural"): "par_pl",
    ("illative", "plural"): "ill_pl",
}

#: A class that marks a compound or a closed paradigm is not a model the
#: wizard offers.
SKIP_CLASSES = {50, 51, 99, 100, 101}


def class_numbers(field: str) -> list[int]:
    """Every model class number in a sanalista `taivutustiedot` field."""
    out: list[int] = []
    for item in field.split(","):
        match = re.match(r"\s*\(?(\d+)", item.strip())
        if not match:
            continue
        number = int(match.group(1))
        if number not in SKIP_CLASSES and number not in out:
            out.append(number)
    return out


def class_expressions(field: str) -> list[str]:
    """The full expressions (class plus gradation) worth comparing exactly."""
    out: list[str] = []
    for item in field.split(","):
        item = item.strip().strip("()")
        if not item:
            continue
        match = re.match(r"(\d+(?:\*[A-Z])*)", item)
        if not match:
            continue
        number = int(re.match(r"\d+", item).group(0))
        if number in SKIP_CLASSES:
            continue
        if match.group(1) not in out:
            out.append(match.group(1))
    return out


def base_of(expression: str) -> int:
    match = re.match(r"(\d+)", expression)
    return int(match.group(1)) if match else 0


def load_bases(db: sqlite3.Connection) -> list[tuple[str, str, str]]:
    rows = db.execute(
        "SELECT lemma, homonym, class FROM base "
        "WHERE class <> '' AND part_of_speech LIKE '%substantiivi%'"
    ).fetchall()
    out: list[tuple[str, str, str]] = []
    for lemma, homonym, field in rows:
        if class_numbers(field):
            out.append((lemma, homonym or "", field))
    return out


def confirmed_forms(
    db: sqlite3.Connection, lemma: str, homonym: str
) -> dict[str, str]:
    rows = db.execute(
        "SELECT \"case\", number, form FROM forms "
        "WHERE lemma = ? AND homonym = ? AND ok = 'both' "
        "AND possessive = ''",
        (lemma, homonym),
    ).fetchall()
    cells: dict[str, list[str]] = {}
    for case, number, form in rows:
        key = CASE_KEY.get((case, number))
        if key is not None and form:
            cells.setdefault(key, []).append(form)
    return {key: sorted(forms)[0] for key, forms in cells.items()}


def score(candidates, bases: set[int], exact: set[str]) -> dict[str, bool]:
    top = candidates[0] if candidates else None
    top3 = candidates[:3]
    return {
        "top1_base": bool(top) and base_of(top[0]) in bases,
        "top3_base": any(base_of(c[0]) in bases for c in top3),
        "top1_exact": bool(top) and top[0] in exact,
        "top3_exact": any(c[0] in exact for c in top3),
    }


def run_adaptive(engine, word: str, forms: dict[str, str], budget: int):
    answers: list[tuple[str, str]] = []
    for _ in range(budget):
        candidates, question, _tie, _recorded = engine.class_candidates(
            word, "noun", answers, use_recorded=False
        )
        if question is None:
            break
        answers.append((question, forms.get(question.key, "")))
    candidates, question, tie, recorded = engine.class_candidates(
        word, "noun", answers, use_recorded=False
    )
    return candidates, question, tie, recorded


def run_fixed(engine, word: str, forms: dict[str, str], budget: int):
    from torille import Cell

    answers = [
        (Cell.parse(cell), forms[cell]) for cell in CELLS[:budget] if cell in forms
    ]
    candidates, question, tie, recorded = engine.class_candidates(
        word, "noun", answers, use_recorded=False
    )
    return candidates, question, tie, recorded


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--db", default=str(DEFAULT_DB), help="noun oracle sqlite")
    parser.add_argument("--limit", type=int, default=200, help="sample size")
    parser.add_argument("--seed", type=int, default=1, help="sample seed")
    parser.add_argument(
        "--answers", type=int, default=5, help="maximum answers per word"
    )
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    args = parser.parse_args()

    db_path = Path(args.db)
    if not db_path.exists():
        print(
            f"{db_path} is missing; build it with\n"
            "  python3 tools/oracle/build_nomine_lexicon.py "
            f"-o {db_path}",
            file=sys.stderr,
        )
        return 2
    try:
        from torille import Engine
    except ImportError:
        print(
            "the torille binding is not built:\n"
            "  (cd bindings && env -u CONDA_PREFIX maturin develop --release)",
            file=sys.stderr,
        )
        return 2

    db = sqlite3.connect(db_path)
    pool = load_bases(db)
    if not pool:
        print("the oracle has no classed nouns", file=sys.stderr)
        return 2
    rng = random.Random(args.seed)
    sample = rng.sample(pool, min(args.limit, len(pool)))

    engine = Engine()
    totals = {
        "adaptive": [dict(top1_base=0, top3_base=0, top1_exact=0, top3_exact=0) for _ in range(args.answers)],
        "fixed": [dict(top1_base=0, top3_base=0, top1_exact=0, top3_exact=0) for _ in range(args.answers)],
    }
    used = 0
    skipped = 0

    for lemma, homonym, field in sample:
        forms = confirmed_forms(db, lemma, homonym)
        if not any(cell in forms for cell in CELLS):
            skipped += 1
            continue
        bases = set(class_numbers(field))
        exact = set(class_expressions(field))
        if not bases:
            skipped += 1
            continue
        used += 1
        for budget in range(1, args.answers + 1):
            adaptive = run_adaptive(engine, lemma, forms, budget)[0]
            fixed = run_fixed(engine, lemma, forms, budget)[0]
            for key, value in score(adaptive, bases, exact).items():
                totals["adaptive"][budget - 1][key] += int(value)
            for key, value in score(fixed, bases, exact).items():
                totals["fixed"][budget - 1][key] += int(value)

    result = {
        "sample": used,
        "skipped": skipped,
        "answers": args.answers,
        "seed": args.seed,
        "adaptive": totals["adaptive"],
        "fixed": totals["fixed"],
    }
    if args.json:
        print(json.dumps(result, indent=2))
        return 0

    print(f"sample: {used} nouns, {skipped} skipped (no confirmed core cell)")
    print(f"pos: noun   seed: {args.seed}\n")
    print("k  adaptive top1-base  top3-base  top1-exact  top3-exact"
          "   fixed top1-base  top3-base  top1-exact  top3-exact")
    for index in range(args.answers):
        adaptive = totals["adaptive"][index]
        fixed = totals["fixed"][index]
        def pct(value: int) -> str:
            return f"{100 * value / used:5.1f}%" if used else "  —  "
        print(
            f"{index + 1}  "
            f"{pct(adaptive['top1_base'])}       {pct(adaptive['top3_base'])}"
            f"      {pct(adaptive['top1_exact'])}       {pct(adaptive['top3_exact'])}"
            f"     {pct(fixed['top1_base'])}       {pct(fixed['top3_base'])}"
            f"      {pct(fixed['top1_exact'])}       {pct(fixed['top3_exact'])}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
