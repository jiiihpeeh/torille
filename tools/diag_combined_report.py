# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build the combined ABLATION_REPORT.md.

Reads the per-ablation eval reports and produces a single
table with: model, params, n_in, dev top-1, dev MRR,
test_unseen_lemma top-1, test_unseen_lemma MRR.

Also reads the non-neural baselines and the existing 3k
Slice-3.5 result for comparison.

Usage:
    python3 tools/diag_combined_report.py \
        --weights data/rerank_v2/weights/slice35_v2_{A,B,C,D}.npz \
        --data-dir data/rerank_typo_v2/ \
        --baseline-data data/rerank_scale/ \
        --output ABLATION_REPORT.md
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, "tools/train")
from model import RerankerWeights  # noqa: E402


def parse_md_table(md: str, header_pattern: str) -> dict:
    """Find a table row in markdown by header and return the
    trailing numeric cells.
    """
    for line in md.splitlines():
        if header_pattern in line:
            return line
    return ""


def eval_one(weights_path: Path, data_dir: Path) -> dict:
    """Run tools/train/eval.py for one set of weights and parse
    the markdown report.
    """
    cmd = [
        "python3", "tools/train/eval.py",
        "--weights", str(weights_path),
        "--data-dir", str(data_dir),
        "--report", str(weights_path.with_suffix(".md")),
    ]
    subprocess.run(cmd, check=True, capture_output=True)
    md = weights_path.with_suffix(".md").read_text()
    metrics: dict = {}
    for line in md.splitlines():
        m = re.match(
            r"\s*(\S+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+)\s+(\d+)",
            line,
        )
        if m:
            split = m.group(1)
            metrics[split] = {
                "gen_rec": float(m.group(2)),
                "engine_top1": float(m.group(3)),
                "det_top1": float(m.group(4)),
                "model_top1": float(m.group(5)),
                "det_mrr": float(m.group(6)),
                "model_mrr": float(m.group(7)),
                "n_eval": int(m.group(8)),
                "n_miss": int(m.group(9)),
            }
    return metrics


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--weights", nargs="+", type=Path, required=True)
    p.add_argument("--data-dir", type=Path, required=True)
    p.add_argument("--baseline-data", type=Path, default=None,
                   help="Optional separate dir for cross-data check")
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()

    # Load all weights and record model size.
    weight_info = []
    for wp in args.weights:
        w = RerankerWeights.load(wp)
        weight_info.append((wp, w))

    # Run eval on the primary data dir.
    primary_metrics = {}
    for wp, w in weight_info:
        m = eval_one(wp, args.data_dir)
        primary_metrics[wp.stem] = m

    # Cross-data: only on the first (best) weight set.
    cross_metrics = None
    if args.baseline_data:
        cross_metrics = eval_one(weight_info[0][0], args.baseline_data)

    # Non-neural baselines on the same data dir.
    print("Running min-edit-distance + deterministic baselines...",
          file=sys.stderr)
    baselines_proc = subprocess.run(
        ["python3", "tools/diag_baselines.py", str(args.data_dir)],
        capture_output=True, text=True,
    )
    baselines_text = baselines_proc.stdout
    # Parse: lines like "  rerank_typo_v2/dev.jsonl      0.737  0.911  0.661 ..."
    baseline_metrics: dict = {}
    for line in baselines_text.splitlines():
        m = re.match(
            r"\s*(\S+/\S+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+\.\d+)\s+(\d+)",
            line,
        )
        if m:
            split = m.group(1)
            baseline_metrics[split] = {
                "min_top1": float(m.group(2)),
                "min_mrr": float(m.group(3)),
                "det_top1": float(m.group(4)),
                "det_mrr": float(m.group(5)),
                "n": int(m.group(6)),
            }

    # Compose the report.
    lines: list[str] = []
    lines.append("# ABLATION_REPORT.md")
    lines.append("")
    lines.append(f"Primary data dir: `{args.data_dir}`")
    if args.baseline_data:
        lines.append(f"Cross-data check: `{args.baseline_data}`")
    lines.append("")
    lines.append("## Model sizes")
    lines.append("")
    lines.append("| tag | weights file | n_in | params |")
    lines.append("| --- | --- | --- | --- |")
    for wp, w in weight_info:
        tag = wp.stem.split("_")[-1]
        lines.append(f"| {tag} | `{wp}` | {w.n_in} | {w.param_count():,} |")
    lines.append("")

    lines.append("## Per-split metrics (primary data)")
    lines.append("")
    lines.append("| split | model | top-1 | MRR | engine | det | min_edit | n |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- |")
    for tag, m in primary_metrics.items():
        for split_name in m:
            row = m[split_name]
            # Pull the matching baseline for this split.
            bkey = f"{args.data_dir.name}/{Path(split_name).name}"
            b = baseline_metrics.get(bkey, {})
            min_top1 = b.get("min_top1", "n/a")
            if isinstance(min_top1, float):
                min_top1 = f"{min_top1:.3f}"
            lines.append(
                f"| {split_name} | {tag} | {row['model_top1']:.3f} | "
                f"{row['model_mrr']:.3f} | {row['engine_top1']:.3f} | "
                f"{row['det_top1']:.3f} | {min_top1} | {row['n_eval']:,} |"
            )
        # Non-neural rows for this split.
        for split_name in m:
            bkey = f"{args.data_dir.name}/{Path(split_name).name}"
            b = baseline_metrics.get(bkey, {})
            if not b:
                continue
            lines.append(
                f"| {split_name} | min_edit_distance | {b.get('min_top1', 0):.3f} | "
                f"{b.get('min_mrr', 0):.3f} | - | - | - | {b.get('n', 0):,} |"
            )
            lines.append(
                f"| {split_name} | deterministic_ranker | - | - | - | "
                f"{b.get('det_top1', 0):.3f} | {b.get('det_mrr', 0):.3f} | {b.get('n', 0):,} |"
            )
            # Print each baseline only once.
            del baseline_metrics[bkey]
    lines.append("")

    if cross_metrics is not None:
        lines.append("## Cross-data check (best v2 model on the original 3k corpus)")
        lines.append("")
        lines.append("| split | model_top1 | model_mrr | engine | det | n |")
        lines.append("| --- | --- | --- | --- | --- | --- |")
        for split_name, row in cross_metrics.items():
            lines.append(
                f"| {split_name} | {row['model_top1']:.3f} | "
                f"{row['model_mrr']:.3f} | {row['engine_top1']:.3f} | "
                f"{row['det_top1']:.3f} | {row['n_eval']:,} |"
            )
        lines.append("")

    lines.append("## Notes")
    lines.append("")
    lines.append("- D = full Slice-3.5 (chars + bigrams + trigrams + 12-dim edit features)")
    lines.append("- C = D without edit position")
    lines.append("- B = D without edit ops and edit position")
    lines.append("- A = D without trigrams, edit ops, and edit position")
    lines.append("- Model file convention: `data/rerank_v2/weights/slice35_v2_<data>_<TAG>.npz`")
    lines.append("- Best by `test_unseen_lemma top-1` is the production candidate.")

    args.output.write_text("\n".join(lines) + "\n")
    print(f"report -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
