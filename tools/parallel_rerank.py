#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Shard the raw typo corpus and run N parallel rerank_data instances.

The single-threaded rerank_data binary is the bottleneck
(3+ hours for 40k rows). With 12 cores, we can split the
train split into N shards and run N instances in parallel;
each instance also processes the dev/test splits (which are
small), so the wall time is bounded by the slowest shard.

Usage:
    python3 tools/parallel_rerank.py --shards 4
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

DEFAULT_INPUT_DIR = Path("data/train")
DEFAULT_OUTPUT_DIR = Path("data/rerank_v2")
DEFAULT_WORK_DIR = Path("data/rerank_v2_work")
SHARD_PREFIX = "shard"


def shard_train(train_path: Path, n: int, input_dir: Path,
                 work_dir: Path) -> list[Path]:
    """Split train.jsonl into N roughly-equal line-count shards.

    Returns the list of shard paths. Each shard is a temp
    directory that contains the same dev/test splits plus a
    sharded train.jsonl.
    """
    lines = train_path.read_text().splitlines()
    total = len(lines)
    print(f"sharding {train_path} ({total:,} lines) into {n} shards...")

    # Make N shard dirs, each with the same dev/test + a sharded train.
    shard_dirs: list[Path] = []
    shard_size = (total + n - 1) // n
    for i in range(n):
        sd = work_dir / f"{SHARD_PREFIX}{i}"
        sd.mkdir(parents=True, exist_ok=True)
        # Copy dev/test unchanged.
        for fname in ("dev.jsonl", "test_unseen_corruption.jsonl",
                      "test_unseen_inflection.jsonl", "test_unseen_lemma.jsonl"):
            src = input_dir / fname
            if src.exists():
                shutil.copy(src, sd / fname)
        # Write the sharded train.jsonl.
        start = i * shard_size
        end = min((i + 1) * shard_size, total)
        (sd / "train.jsonl").write_text("\n".join(lines[start:end]) + "\n")
        shard_dirs.append(sd)
        print(f"  shard {i}: lines {start:,}..{end:,} -> {sd}/train.jsonl")
    return shard_dirs


def launch_shards(shard_dirs: list[Path], output_dir: Path) -> list[subprocess.Popen]:
    """Spawn one rerank_data instance per shard in parallel."""
    procs: list[subprocess.Popen] = []
    for i, sd in enumerate(shard_dirs):
        out = output_dir / f"{SHARD_PREFIX}{i}"
        out.mkdir(parents=True, exist_ok=True)
        cmd = [
            "cargo", "run", "--release",
            "--package", "torille-tools",
            "--bin", "rerank_data",
            "--", "--input", str(sd), "--output", str(out), "--max-rows", "0",
        ]
        log_path = out / "log.txt"
        log_f = open(log_path, "w")
        print(f"  launching shard {i}: {sd} -> {out}  (log: {log_path})")
        p = subprocess.Popen(cmd, stdout=log_f, stderr=subprocess.STDOUT)
        procs.append(p)
    return procs


def collect_outputs(shard_dirs: list[Path], n: int,
                    output_dir: Path) -> None:
    """Wait for all shards to finish, then concatenate the
    per-shard train.jsonl into the final output dir.
    """
    print("waiting for all shards to finish...")
    import time
    log_paths = [output_dir / f"{SHARD_PREFIX}{i}" / "log.txt" for i in range(n)]
    done = [False] * n
    while not all(done):
        for i, lp in enumerate(log_paths):
            if done[i]:
                continue
            txt = lp.read_text() if lp.exists() else ""
            if "rerank_data summary:" in txt:
                done[i] = True
                print(f"  shard {i} done")
        if not all(done):
            time.sleep(15)

    # Concatenate the per-shard outputs.
    out_train = output_dir / "train.jsonl"
    with out_train.open("w") as fout:
        for i in range(n):
            shard_train_path = output_dir / f"{SHARD_PREFIX}{i}" / "train.jsonl"
            with shard_train_path.open() as fin:
                shutil.copyfileobj(fin, fout)
    # Copy dev + test from shard 0 (they're identical across shards).
    for fname in ("dev.jsonl", "test_unseen_corruption.jsonl",
                  "test_unseen_inflection.jsonl", "test_unseen_lemma.jsonl"):
        src = output_dir / f"{SHARD_PREFIX}0" / fname
        if src.exists():
            shutil.copy(src, output_dir / fname)
    # Final summary.
    for i in range(n):
        log = (output_dir / f"{SHARD_PREFIX}{i}" / "log.txt").read_text()
        for line in log.splitlines():
            if "rerank_data summary" in line:
                print(f"  shard {i}: {line.strip()}")
    print(f"concatenated train.jsonl -> {out_train}")


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--shards", type=int, default=4)
    p.add_argument("--input", type=Path, default=DEFAULT_INPUT_DIR)
    p.add_argument("--output", type=Path, default=DEFAULT_OUTPUT_DIR)
    p.add_argument("--keep-shards", action="store_true",
                   help="Don't delete the per-shard temp dirs after collection.")
    args = p.parse_args()

    input_dir = args.input
    output_dir = args.output
    work_dir = output_dir.parent / (output_dir.name + "_work")

    train_path = input_dir / "train.jsonl"
    if not train_path.exists():
        print(f"missing {train_path}", file=sys.stderr)
        return 2

    shard_dirs = shard_train(train_path, args.shards, input_dir, work_dir)
    procs = launch_shards(shard_dirs, output_dir)
    try:
        # Poll the log files for completion rather than blocking
        # on the Popen objects (avoids the bash timeout).
        collect_outputs(shard_dirs, args.shards, output_dir)
    except KeyboardInterrupt:
        for p in procs:
            p.terminate()
        raise

    if not args.keep_shards:
        for sd in shard_dirs:
            shutil.rmtree(sd, ignore_errors=True)
        if work_dir.exists():
            shutil.rmtree(work_dir, ignore_errors=True)
        print(f"cleaned up {work_dir}")

    print(f"final output -> {output_dir}/train.jsonl")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
