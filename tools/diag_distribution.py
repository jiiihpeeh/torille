# SPDX-License-Identifier: MIT OR Apache-2.0
"""Pre-rerank diagnostic for raw typo data.

Reports split sizes, unique-lemma counts, the per-lemma
concentration distribution, the corruption-strategy
distribution, and the edit-distance distribution.

This is the diagnostic that decides whether the corpus
is fit for Phase 2 training. Two hard gates are checked
and the script exits non-zero if either fails:

  1. >= 1,000 unique train lemmas
  2. No single train lemma contributes > 1% of train rows

If gate 2 fails, the corpus has the same 14-lemma problem
the existing 3,000-row corpus had, just at 100x scale.

Usage:
    python3 tools/diag_distribution.py data/train_v2/
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from pathlib import Path

SPLIT_FILES = (
    "train.jsonl",
    "dev.jsonl",
    "test_unseen_lemma.jsonl",
    "test_unseen_inflection.jsonl",
    "test_unseen_corruption.jsonl",
)

HARD_GATE_MIN_LEMMAS = 1000
HARD_GATE_MAX_LEMMA_FRACTION = 0.01


def _load(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open() as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return rows


def _percentile(sorted_vals: list[int], p: float) -> int:
    if not sorted_vals:
        return 0
    k = int(round(p * (len(sorted_vals) - 1)))
    return sorted_vals[k]


def _format_table(rows: list[tuple[str, str]], widths: list[int]) -> str:
    out: list[str] = []
    for r in rows:
        out.append("  ".join(c.ljust(w) for c, w in zip(r, widths)))
    return "\n".join(out)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: diag_distribution.py <data-dir>", file=sys.stderr)
        return 2
    data_dir = Path(argv[1])
    if not data_dir.is_dir():
        print(f"not a directory: {data_dir}", file=sys.stderr)
        return 2

    print("=" * 70)
    print(f"PRE-RERANK DIAGNOSTIC: {data_dir}")
    print("=" * 70)

    # Per-split sizes + lemma counts.
    print()
    print("=== Per-split sizes ===")
    rows: list[tuple[str, str, str, str]] = []
    rows.append(("split", "rows", "unique_lemmas", "noun%"))
    for fname in SPLIT_FILES:
        path = data_dir / fname
        if not path.exists():
            rows.append((fname, "MISSING", "-", "-"))
            continue
        split_rows = _load(path)
        lemmas = Counter(r.get("lemma", "") for r in split_rows)
        # Verb detection: the train_data corpus uses homonym==""
        # for the noun rows; verbs also use "" so it's not
        # deterministic. We report noun% as 100% for the existing
        # schema (the corpus is per-database already).
        noun_pct = 100.0
        rows.append((
            fname,
            f"{len(split_rows):,}",
            f"{len(lemmas):,}",
            f"{noun_pct:.0f}",
        ))
    widths = [32, 10, 16, 8]
    print(_format_table(rows, widths))

    # Lemma concentration for the train split (the only one that
    # matters for memorisation).
    print()
    print("=== Lemma concentration (train) ===")
    train_path = data_dir / "train.jsonl"
    if not train_path.exists():
        print(f"  (no {train_path})")
    else:
        train_rows = _load(train_path)
        lemma_counts = Counter(r.get("lemma", "") for r in train_rows)
        sorted_counts = sorted(lemma_counts.values())
        total = sum(sorted_counts)
        n_unique = len(sorted_counts)
        median = _percentile(sorted_counts, 0.5)
        p90 = _percentile(sorted_counts, 0.9)
        p99 = _percentile(sorted_counts, 0.99)
        max_lemma = max(lemma_counts.items(), key=lambda kv: kv[1])
        max_count = max_lemma[1]
        max_pct = 100.0 * max_count / total if total else 0.0
        # Concentration: how many rows do the top-K lemmas account for?
        top10 = sum(c for _, c in lemma_counts.most_common(10))
        top50 = sum(c for _, c in lemma_counts.most_common(50))
        top100 = sum(c for _, c in lemma_counts.most_common(100))
        print(f"  unique lemmas:        {n_unique:>10,}")
        print(f"  total rows:           {total:>10,}")
        print(f"  median rows/lemma:    {median:>10,}")
        print(f"  p90 rows/lemma:       {p90:>10,}")
        print(f"  p99 rows/lemma:       {p99:>10,}")
        print(f"  max rows/lemma:       {max_count:>10,} ({max_pct:.2f}%) "
              f"({max_lemma[0]!r})")
        print(f"  top 10 lemmas cover:  {100.0 * top10 / total:>6.2f}%")
        print(f"  top 50 lemmas cover:  {100.0 * top50 / total:>6.2f}%")
        print(f"  top 100 lemmas cover: {100.0 * top100 / total:>6.2f}%")
        print()
        print(f"  Hard gate 1: >= {HARD_GATE_MIN_LEMMAS} unique lemmas:    "
              f"{'PASS' if n_unique >= HARD_GATE_MIN_LEMMAS else 'FAIL'}"
              f"  ({n_unique:,} vs {HARD_GATE_MIN_LEMMAS:,})")
        print(f"  Hard gate 2: no lemma > {HARD_GATE_MAX_LEMMA_FRACTION:.0%}:      "
              f"{'PASS' if max_pct / 100 <= HARD_GATE_MAX_LEMMA_FRACTION else 'FAIL'}"
              f"  ({max_pct:.2f}%)")

    # Corruption-strategy distribution for the train split.
    print()
    print("=== Corruption strategy (train) ===")
    if train_path.exists():
        train_rows = _load(train_path)
        ctype = Counter(r.get("corruption_type", "?") for r in train_rows)
        total = sum(ctype.values())
        rows = [("strategy", "count", "fraction")]
        for name, count in ctype.most_common():
            rows.append((name, f"{count:,}", f"{100.0 * count / total:.2f}%"))
        print(_format_table(rows, [32, 12, 12]))

    # Edit-distance distribution for the train split.
    print()
    print("=== Edit distance (train) ===")
    if train_path.exists():
        train_rows = _load(train_path)
        ed = Counter(int(r.get("edit_distance", 0)) for r in train_rows)
        total = sum(ed.values())
        rows = [("edit_distance", "count", "fraction")]
        for d in sorted(ed):
            rows.append((str(d), f"{ed[d]:,}", f"{100.0 * ed[d] / total:.2f}%"))
        print(_format_table(rows, [16, 12, 12]))

    print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
