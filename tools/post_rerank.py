#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Post-rerank processing: filter, train A/B/C/D, evaluate, report.

Runs after the parallel rerank_data has produced train.jsonl in
the output dir. Steps:

  1. Filter rows where the POSITIVE candidate is
     synthetic_positive (write train.filtered.jsonl).
  2. Run the A/B/C/D ablations on the filtered training set.
  3. Evaluate each on the held-out test set.
  4. Cross-data: evaluate the best model on the original
     data/rerank_scale/.
  5. Write a combined ABLATION_REPORT.md.

Usage:
    python3 tools/post_rerank.py --data-dir data/rerank_v2 \
        --cross-data-dir data/rerank_scale --output ABLATION_REPORT.md
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

SPLIT_FILES = (
    "train.jsonl", "dev.jsonl", "test_unseen_lemma.jsonl",
    "test_unseen_inflection.jsonl", "test_unseen_corruption.jsonl",
)
TAGS = ("A", "B", "C", "D")


def run(cmd: list[str], **kw) -> None:
    print(f"  $ {' '.join(cmd)}", flush=True)
    subprocess.run(cmd, check=True, **kw)


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--data-dir", type=Path, required=True)
    p.add_argument("--cross-data-dir", type=Path, default=None)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--epochs", type=int, default=20)
    p.add_argument("--seed", type=int, default=0)
    args = p.parse_args()

    data_dir = args.data_dir
    train_path = data_dir / "train.jsonl"
    if not train_path.exists():
        print(f"missing {train_path}", file=sys.stderr)
        return 1

    # 1. Filter synthetic_positive positives from the train split.
    filtered_path = data_dir / "train.filtered.jsonl"
    print("\n[1] Filtering synthetic_positive positives...")
    run([
        "python3", "tools/diag_filter_synthetic.py",
        str(train_path), str(filtered_path),
    ])

    # 2. Run A/B/C/D ablations.
    weights: dict[str, Path] = {}
    weights_dir = data_dir / "weights"
    weights_dir.mkdir(parents=True, exist_ok=True)
    for tag in TAGS:
        out_npz = weights_dir / f"slice35_v2_{tag}.npz"
        weights[tag] = out_npz
        flags = []
        if tag in ("A",):
            flags = ["--no-trigrams", "--no-edit-ops", "--no-edit-position"]
        elif tag == "B":
            flags = ["--no-edit-ops", "--no-edit-position"]
        elif tag == "C":
            flags = ["--no-edit-position"]
        # tag == "D" has no flags (everything on)
        print(f"\n[2.{tag}] Training {tag}...")
        run([
            "python3", "tools/train/train.py",
            "--train", str(filtered_path),
            "--dev", str(data_dir / "dev.jsonl"),
            "--epochs", str(args.epochs),
            "--output", str(out_npz),
            "--seed", str(args.seed),
            *flags,
        ])

    # 3. Evaluate each on the scaled test set.
    print(f"\n[3] Evaluating {len(TAGS)} models on scaled data...")
    for tag, npz in weights.items():
        run([
            "python3", "tools/train/eval.py",
            "--weights", str(npz),
            "--data-dir", str(data_dir),
            "--report", str(npz.with_suffix(".md")),
        ])

    # 4. Cross-data check: best model on the original 3k corpus.
    if args.cross_data_dir:
        print(f"\n[4] Cross-data check on {args.cross_data_dir}...")
        # Find the best of A/B/C/D by test_unseen_lemma top-1 in the
        # scaled reports. Pick the highest.
        best_tag = "D"
        best_score = -1.0
        for tag, npz in weights.items():
            md = npz.with_suffix(".md").read_text()
            for line in md.splitlines():
                if "test_unseen_lemma" in line:
                    parts = line.split("|")
                    # columns: split, gen_rec, engine, det, model, ...
                    try:
                        model_top1 = float(parts[4].strip())
                    except (IndexError, ValueError):
                        continue
                    if model_top1 > best_score:
                        best_score = model_top1
                        best_tag = tag
        print(f"  best scaled model: {best_tag} (test_unseen_lemma={best_score:.3f})")
        run([
            "python3", "tools/train/eval.py",
            "--weights", str(weights[best_tag]),
            "--data-dir", str(args.cross_data_dir),
            "--report", str(weights[best_tag].with_name(
                f"torille_v2_scaled_{best_tag}_on_real.md"
            )),
        ])

    # 5. Combined report.
    print("\n[5] Building combined report...")
    cmd = [
        "python3", "tools/diag_combined_report.py",
        "--data-dir", str(data_dir),
        "--output", str(args.output),
    ]
    for tag in TAGS:
        cmd += ["--weights", str(weights[tag])]
    if args.cross_data_dir:
        cmd += ["--baseline-data", str(args.cross_data_dir)]
    run(cmd)

    print(f"\nDone. Report -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
