#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Performance gate for the check/suggest plan (PERFORMANCE_TODO.md).

Runs `perf_check_suggest` over a book excerpt and compares the first-pass
timings with a recorded baseline. The engine's budgets are still moving, so
the default mode is a *regression* check against the baseline; pass `--budget`
to enforce the absolute ceilings once the plan's phases have landed.

Usage:
  python3 tools/test_performance.py                 # compare with baseline
  python3 tools/test_performance.py --record        # write the baseline
  python3 tools/test_performance.py --budget        # enforce the ceilings
  python3 tools/test_performance.py --tokens 5000   # bigger sample

The baseline lives in reports/perf-baseline.json (gitignored, machine-local).
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "release" / "perf_check_suggest"
BASELINE = REPO / "reports" / "perf-baseline.json"
CORPUS = REPO / "torille-wasm" / "demo" / "books" / "45448.txt"

# Absolute ceilings from PERFORMANCE_TODO.md. Only checked with --budget.
BUDGET = {
    "per_token_ms": 0.25,
    "p95_us": 500,
    "p99_us": 2000,
}


def run_probe(tokens: int, single_thread: bool) -> dict:
    if not BINARY.exists():
        print(
            f"missing {BINARY.relative_to(REPO)}; build it with\n"
            "  cargo build --release -p torille-tools --bin perf_check_suggest",
            file=sys.stderr,
        )
        raise SystemExit(2)
    cmd = [
        str(BINARY),
        "--json",
        "--corpus",
        str(CORPUS),
        "--tokens",
        str(tokens),
    ]
    if single_thread:
        cmd.append("--single-thread")
    out = subprocess.run(cmd, check=True, capture_output=True, text=True).stdout
    data = json.loads(out)
    if not data.get("corpus"):
        print(f"probe produced no corpus row for {CORPUS}", file=sys.stderr)
        raise SystemExit(2)
    return data["corpus"][0]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tokens", type=int, default=2000)
    parser.add_argument("--record", action="store_true")
    parser.add_argument("--budget", action="store_true")
    parser.add_argument("--single-thread", action="store_true")
    args = parser.parse_args()

    current = run_probe(args.tokens, args.single_thread)
    print(
        f"{CORPUS.name}: {current['tokens']} tokens, "
        f"{current['per_token_ms']:.3f} ms/token, "
        f"p50={current['p50_us']}µs p95={current['p95_us']}µs "
        f"p99={current['p99_us']}µs max={current['max_us']}µs"
    )

    failed = False
    if args.budget:
        for key, limit in BUDGET.items():
            if current[key] > limit:
                print(f"  BUDGET FAIL: {key}={current[key]} > {limit}")
                failed = True

    if args.record:
        BASELINE.parent.mkdir(parents=True, exist_ok=True)
        BASELINE.write_text(
            json.dumps(
                {
                    "corpus": str(CORPUS.relative_to(REPO)),
                    "single_thread": args.single_thread,
                    **current,
                },
                indent=2,
            )
            + "\n"
        )
        print(f"recorded {BASELINE.relative_to(REPO)}")
    elif BASELINE.exists():
        baseline = json.loads(BASELINE.read_text())
        for key in ("per_token_ms", "p95_us", "p99_us"):
            old = baseline.get(key)
            if old is None:
                continue
            ratio = current[key] / old if old else float("inf")
            status = "ok" if ratio <= 1.25 else "REGRESSION"
            if ratio > 1.25:
                failed = True
            print(
                f"  {key}: {current[key]} vs baseline {old} "
                f"({ratio:.2f}x) {status}"
            )
    else:
        print(
            "no baseline yet; run with --record to create "
            f"{BASELINE.relative_to(REPO)}"
        )

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
