#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""The torille benchmark suite driver.

Phase 0: one reproducible report over the existing release binaries. It does
not measure anything itself; it runs the tools, normalises their JSON into one
versioned report, fingerprints the corpora and the correctness output, and
compares two reports from the same machine.

Usage:
    python3 tools/bench.py run --profile quick
    python3 tools/bench.py run --profile full --taskset 0-3
    python3 tools/bench.py compare bench/baselines/reference.json reports/bench-full.json
    python3 tools/bench.py record reports/bench-full.json --name reference
    python3 tools/bench.py corpora --check
    python3 tools/bench.py selftest
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
TOOLS = REPO / "tools"
TARGET = REPO / "target" / "release"
MANIFEST = REPO / "bench" / "corpora.json"
BASELINES = REPO / "bench" / "baselines"
REPORTS = REPO / "reports"
GEN_CORPORA = TOOLS / "gen_perf_corpora.py"

SCHEMA = 1
TIMING_SUFFIXES = ("_ms", "_us", "_s", "_bytes", "_kb", "_mb")
DEFAULT_TOLERANCE = 0.10

BINARIES = {
    "grammar_style": "perf_grammar_style",
    "check_suggest": "perf_check_suggest",
    "index": "perf_index",
    "diagnostics": "dump_diagnostics",
    "forms": "inflect",
}

#: Lemmas whose complete paradigm is part of the correctness fingerprint. A
#: morphology change that the diagnostics cannot see must still fail the suite.
FORM_CANARIES = (
    "kissa", "talo", "havas", "live", "juosta", "pieni", "ahkeraliisa",
    "pikkuaivot",
)


# ─── report shape ───────────────────────────────────────────────────────


def fail(message: str, code: int = 2) -> None:
    print(f"bench: {message}", file=sys.stderr)
    sys.exit(code)


def validate(report: dict) -> None:
    if report.get("schema") != SCHEMA:
        fail(f"report schema {report.get('schema')!r}, expected {SCHEMA}")
    for key in ("meta", "corpora", "metrics", "correctness"):
        if key not in report:
            fail(f"report is missing {key!r}")
    meta = report["meta"]
    for key in ("fingerprint", "profile", "repeats", "date"):
        if key not in meta:
            fail(f"report meta is missing {key!r}")
    for key, value in report["metrics"].items():
        if not isinstance(value, dict) or "median" not in value:
            fail(f"metric {key!r} is not a sample (min/median/max/runs)")
    if not isinstance(report["corpora"], dict):
        fail("report corpora is not an object")


# ─── machine metadata ───────────────────────────────────────────────────


def cpu_name() -> str:
    try:
        text = Path("/proc/cpuinfo").read_text(encoding="utf-8")
        match = re.search(r"^model name\s*:\s*(.+)$", text, re.MULTILINE)
        if match:
            return match.group(1).strip()
    except OSError:
        pass
    return platform.processor() or platform.machine()


def rustc_version() -> str:
    try:
        return subprocess.run(
            ["rustc", "--version"], capture_output=True, text=True, check=True
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def git_state() -> tuple[str, bool]:
    try:
        commit = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=REPO, capture_output=True, text=True, check=True,
        ).stdout.strip()
        status = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=no"],
            cwd=REPO, capture_output=True, text=True, check=True,
        ).stdout
        return commit, bool(status.strip())
    except (OSError, subprocess.CalledProcessError):
        return "unknown", False


def fingerprint() -> tuple[str, dict]:
    meta = {
        "cpu": cpu_name(),
        "os": platform.platform(),
        "rustc": rustc_version(),
        "profile": "release",
    }
    blob = "|".join(meta[key] for key in ("cpu", "os", "rustc", "profile"))
    digest = hashlib.sha256(blob.encode("utf-8")).hexdigest()[:16]
    return digest, meta


# ─── corpora ────────────────────────────────────────────────────────────


def load_manifest() -> dict:
    if not MANIFEST.exists():
        fail(f"missing {MANIFEST}; run `python3 tools/bench.py corpora --update`")
    return json.loads(MANIFEST.read_text(encoding="utf-8"))


def corpus_path(entry: dict) -> Path:
    return REPO / entry["path"]


def corpus_stats(path: Path) -> dict:
    data = path.read_bytes()
    text = data.decode("utf-8")
    return {
        "path": str(path.relative_to(REPO)),
        "bytes": len(data),
        "words": len(text.split()),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def generate_corpora(entry: dict) -> None:
    """Run the deterministic generator once for the entry's source/dir."""
    source = REPO / entry["source"]
    out = corpus_path(entry).parent
    seed = entry.get("seed", 7)
    percent = entry.get("percent", 8)
    subprocess.run(
        [
            sys.executable, str(GEN_CORPORA),
            "--source", str(source),
            "--out", str(out),
            "--seed", str(seed),
            "--percent", str(percent),
        ],
        cwd=REPO, check=True,
    )


def cmd_corpora(args: argparse.Namespace) -> int:
    manifest = load_manifest()
    if args.regenerate:
        seen = set()
        for entry in manifest["corpora"].values():
            if entry.get("kind") == "generated":
                key = (entry["source"], str(corpus_path(entry).parent), entry.get("seed"))
                if key not in seen:
                    generate_corpora(entry)
                    seen.add(key)
    failures = 0
    updated = False
    for name, entry in manifest["corpora"].items():
        path = corpus_path(entry)
        if not path.exists():
            if entry.get("kind") == "generated":
                print(f"corpora: {name} missing; generating")
                generate_corpora(entry)
            else:
                print(f"corpora: {name} missing at {path}", file=sys.stderr)
                failures += 1
                continue
        stats = corpus_stats(path)
        if args.update:
            entry.update(stats)
            updated = True
        expected = entry.get("sha256")
        if expected and expected != stats["sha256"]:
            print(
                f"corpora: {name} drifted ({expected[:12]} != {stats['sha256'][:12]})",
                file=sys.stderr,
            )
            failures += 1
        print(
            f"corpora: {name:18} {stats['words']:>7} words  "
            f"{stats['bytes']:>8} bytes  {stats['sha256'][:12]}"
        )
    if updated:
        MANIFEST.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        print(f"corpora: updated {MANIFEST}")
    return 1 if failures else 0


# ─── running ────────────────────────────────────────────────────────────


def tool(name: str) -> Path:
    path = TARGET / BINARIES[name]
    if not path.exists():
        fail(f"{path} is missing; run without --no-build")
    return path


def run_json(command: list[str], timeout: int = 1800) -> dict:
    result = subprocess.run(
        command, cwd=REPO, capture_output=True, text=True, timeout=timeout,
    )
    if result.returncode != 0:
        fail(f"command failed ({result.returncode}): {' '.join(command)}\n{result.stderr}")
    lines = [line for line in result.stdout.splitlines() if line.strip()]
    if not lines:
        fail(f"no JSON output from {' '.join(command)}")
    try:
        return json.loads(lines[-1])
    except json.JSONDecodeError as error:
        fail(f"invalid JSON from {' '.join(command)}: {error}\n{lines[-1][:400]}")


def flatten(prefix: str, value, out: dict) -> None:
    if isinstance(value, dict):
        for key, item in value.items():
            flatten(f"{prefix}{key}.", item, out)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            flatten(f"{prefix}{index}.", item, out)
    elif isinstance(value, bool) or value is None:
        return
    elif isinstance(value, (int, float)):
        out[f"{prefix.rstrip('.')}"] = float(value)


def flatten_check_suggest(name: str, data: dict, samples: dict) -> None:
    """Readable keys for the wordlist/book tool.

    Its JSON puts the totals in a `corpus` list and the per-word rows in
    `words`; the generic list flatten would name them `.0.` and `.1.`, which
    is useless in a comparison table.
    """
    for key, value in data.items():
        if key in ("corpus", "words"):
            continue
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            run_samples(samples, f"check_suggest.{name}.{key}", float(value))
    for row in data.get("corpus") or []:
        for key, value in row.items():
            if isinstance(value, (int, float)) and not isinstance(value, bool):
                run_samples(samples, f"check_suggest.{name}.corpus.{key}", float(value))
    for row in data.get("words") or []:
        word = str(row.get("word", "")).replace(" ", "_")
        for key, value in row.items():
            if key == "word":
                continue
            if isinstance(value, (int, float)) and not isinstance(value, bool):
                run_samples(
                    samples, f"check_suggest.{name}.word.{word}.{key}", float(value)
                )


def run_samples(samples: dict, key: str, value: float) -> None:
    samples.setdefault(key, []).append(value)


def finish_samples(samples: dict) -> dict:
    metrics = {}
    for key, runs in samples.items():
        runs = [run for run in runs if run is not None]
        if not runs:
            continue
        runs_sorted = sorted(runs)
        metrics[key] = {
            "min": runs_sorted[0],
            "median": statistics.median(runs_sorted),
            "max": runs_sorted[-1],
            "runs": runs,
        }
    return metrics


def build_tools() -> None:
    subprocess.run(
        [
            "cargo", "build", "--release", "-p", "torille-tools",
            "--bin", BINARIES["grammar_style"],
            "--bin", BINARIES["check_suggest"],
            "--bin", BINARIES["index"],
            "--bin", BINARIES["diagnostics"],
            "--bin", BINARIES["forms"],
        ],
        cwd=REPO, check=True,
    )


def selected_corpora(manifest: dict, profile: str, only: list[str]) -> dict:
    if only:
        missing = [name for name in only if name not in manifest["corpora"]]
        if missing:
            fail(f"unknown corpora: {', '.join(missing)}")
        return {name: manifest["corpora"][name] for name in only}
    if profile == "quick":
        wanted = ("page-2000", "homonyms")
    else:
        wanted = tuple(manifest["corpora"])
    return {name: manifest["corpora"][name] for name in wanted}


def run_report(args: argparse.Namespace) -> dict:
    manifest = load_manifest()
    corpora = selected_corpora(manifest, args.profile, args.corpus)
    repeats = args.repeat or (1 if args.profile == "quick" else 7)
    single = True
    threads = args.profile == "full" or args.threads
    samples: dict[str, list[float]] = {}
    raw: dict = {}
    correctness: dict = {}
    taskset = ["taskset", "-c", args.taskset] if args.taskset else []

    def invoke(command: list[str]) -> dict:
        return run_json(taskset + command)

    for name, entry in corpora.items():
        path = corpus_path(entry)
        if not path.exists():
            fail(f"corpus {name} is missing at {path}; run `bench.py corpora --check`")
        stats = corpus_stats(path)
        correctness[name] = {"corpus": stats}

        diagnostics = invoke([str(tool("diagnostics")), "--file", str(path), "--json"])
        correctness[name]["diagnostics"] = {
            "total": diagnostics["total"],
            "hash": diagnostics["hash"],
            "counts": diagnostics["counts"],
        }
        raw[f"diagnostics.{name}"] = diagnostics

        modes = [("single", ["--single-thread"])]
        if threads:
            modes.append(("threads4", ["--threads", "4"]))
        for mode, mode_args in modes:
            for run in range(repeats):
                data = invoke(
                    [
                        str(tool("grammar_style")),
                        "--file", str(path),
                        "--words", "0",
                        "--repeat", "1",
                        "--json",
                        *mode_args,
                    ]
                )
                flat = {}
                flatten("", data, flat)
                for key, value in flat.items():
                    run_samples(samples, f"grammar_style.{name}.{mode}.{key}", value)
                if run == 0:
                    raw[f"grammar_style.{name}.{mode}"] = data

        for run in range(repeats):
            data = invoke(
                [
                    str(tool("check_suggest")),
                    "--corpus", str(path),
                    "--tokens", str(max(stats["words"], 1)),
                    "--iters", "1",
                    "--fast-iters", "20",
                    "--json",
                ]
            )
            flatten_check_suggest(name, data, samples)
            if run == 0:
                raw[f"check_suggest.{name}"] = data

    if args.profile == "full":
        for run in range(min(repeats, 3)):
            data = invoke([str(tool("index")), "--single-thread", "--json"])
            flat = {}
            flatten("index.single_thread.", data, flat)
            for key, value in flat.items():
                run_samples(samples, key, value)
            if run == 0:
                raw["index.single_thread"] = data

    if args.criterion and args.profile == "full":
        subprocess.run(
            ["cargo", "bench", "--bench", "torille", "--", "--save-baseline", "bench"],
            cwd=REPO, check=True,
        )
        raw["criterion"] = {"ran": True}

    forms = {}
    for lemma in FORM_CANARIES:
        result = subprocess.run(
            [str(tool("forms")), lemma],
            cwd=REPO, capture_output=True, text=True, check=True,
        )
        forms[lemma] = hashlib.sha256(result.stdout.encode("utf-8")).hexdigest()

    digest, machine = fingerprint()
    machine = dict(machine)
    build_profile = machine.pop("profile")
    commit, dirty = git_state()
    report = {
        "schema": SCHEMA,
        "meta": {
            "generator": "tools/bench.py",
            "date": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
            "git": commit,
            "dirty": dirty,
            "profile": args.profile,
            "build_profile": build_profile,
            "repeats": repeats,
            "taskset": args.taskset,
            "fingerprint": digest,
            **machine,
        },
        "corpora": {
            name: corpus_stats(corpus_path(entry)) for name, entry in corpora.items()
        },
        "metrics": finish_samples(samples),
        "correctness": {**correctness, "forms": forms},
        "resources": {},
        "raw": raw if args.raw else {},
    }
    validate(report)
    return report


# ─── compare ────────────────────────────────────────────────────────────


def is_timing(key: str) -> bool:
    return any(key.endswith(suffix) for suffix in TIMING_SUFFIXES)


def compare_reports(
    baseline: dict,
    current: dict,
    tolerance: float,
    allow_machine: bool = False,
) -> tuple[dict, bool]:
    validate(baseline)
    validate(current)
    if (
        baseline["meta"]["fingerprint"] != current["meta"]["fingerprint"]
        and not allow_machine
    ):
        fail(
            "machine fingerprints differ: "
            f"{baseline['meta']['fingerprint']} != {current['meta']['fingerprint']} "
            "(use --allow-machine-mismatch only to inspect)"
        )
    failures = []
    rows = []
    for key, base in sorted(baseline["metrics"].items()):
        cur = current["metrics"].get(key)
        if cur is None:
            rows.append((key, base["median"], None, None, "missing"))
            failures.append(key)
            continue
        if not is_timing(key):
            continue
        base_median = base["median"]
        cur_median = cur["median"]
        if base_median == 0:
            continue
        delta = (cur_median - base_median) / base_median * 100.0
        verdict = "ok"
        if delta > tolerance * 100.0:
            verdict = "REGRESSION"
            failures.append(key)
        rows.append((key, base_median, cur_median, delta, verdict))

    correctness_ok = baseline["correctness"] == current["correctness"]
    if not correctness_ok:
        failures.append("correctness")
    return {
        "rows": rows,
        "failures": failures,
        "correctness": correctness_ok,
    }, not failures


def cmd_run(args: argparse.Namespace) -> int:
    if not args.no_build:
        build_tools()
    report = run_report(args)
    out = Path(args.out) if args.out else REPORTS / f"bench-{args.profile}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"bench: wrote {out}")
    if args.json:
        print(json.dumps(report, ensure_ascii=False))
    else:
        diagnostics = sum(
            item["diagnostics"]["total"]
            for item in report["correctness"].values()
            if isinstance(item, dict) and "diagnostics" in item
        )
        print(
            f"bench: {args.profile} profile, {report['meta']['repeats']} run(s), "
            f"{len(report['metrics'])} metrics, {diagnostics} diagnostics"
        )
    return 0


def cmd_compare(args: argparse.Namespace) -> int:
    baseline = json.loads(Path(args.baseline).read_text(encoding="utf-8"))
    current = json.loads(Path(args.report).read_text(encoding="utf-8"))
    result, ok = compare_reports(
        baseline, current, args.tolerance, args.allow_machine_mismatch
    )
    print(f"{'metric':<72} {'base':>10} {'current':>10} {'delta':>8}  verdict")
    for key, base, cur, delta, verdict in result["rows"]:
        if cur is None:
            print(f"{key:<72} {base:>10.3f} {'-':>10} {'-':>8}  {verdict}")
        else:
            print(f"{key:<72} {base:>10.3f} {cur:>10.3f} {delta:>7.1f}%  {verdict}")
    if not result["correctness"]:
        print("correctness: CHANGED")
    if args.json:
        print(json.dumps(result, ensure_ascii=False))
    return 0 if ok else 1


def cmd_record(args: argparse.Namespace) -> int:
    report = json.loads(Path(args.report).read_text(encoding="utf-8"))
    validate(report)
    BASELINES.mkdir(parents=True, exist_ok=True)
    target = BASELINES / f"{args.name}.json"
    if target.exists() and not args.force:
        fail(f"{target} exists; use --force")
    shutil.copyfile(args.report, target)
    print(f"bench: recorded {target} ({report['meta']['fingerprint']})")
    return 0


def cmd_selftest(_: argparse.Namespace) -> int:
    digest, machine = fingerprint()
    meta = {
        "fingerprint": digest,
        "profile": "quick",
        "repeats": 1,
        "date": "selftest",
        **machine,
    }
    base = {
        "schema": SCHEMA,
        "meta": meta,
        "corpora": {},
        "metrics": {"a_ms": {"min": 1.0, "median": 1.0, "max": 1.0, "runs": [1.0]}},
        "correctness": {"x": {"hash": "abc"}},
    }
    same = json.loads(json.dumps(base))
    _, ok = compare_reports(base, same, DEFAULT_TOLERANCE)
    assert ok, "identical reports must compare equal"
    worse = json.loads(json.dumps(base))
    worse["metrics"]["a_ms"]["median"] = 1.2
    result, ok = compare_reports(base, worse, DEFAULT_TOLERANCE)
    assert not ok and result["failures"] == ["a_ms"], result
    changed = json.loads(json.dumps(base))
    changed["correctness"]["x"]["hash"] = "def"
    result, ok = compare_reports(base, changed, DEFAULT_TOLERANCE)
    assert not ok and "correctness" in result["failures"], result
    print("bench: selftest ok")
    return 0


# ─── CLI ────────────────────────────────────────────────────────────────


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="run the suite and write a JSON report")
    run.add_argument("--profile", choices=("quick", "full"), default="quick")
    run.add_argument("--corpus", action="append", default=[],
                     help="corpus name from bench/corpora.json (repeatable)")
    run.add_argument("--repeat", type=int, default=0,
                     help="fresh-process runs (quick 1, full 7)")
    run.add_argument("--threads", action="store_true",
                     help="also run the 4-worker shape")
    run.add_argument("--taskset", help="CPU list for taskset -c")
    run.add_argument("--out", help="report path")
    run.add_argument("--json", action="store_true", help="print the report too")
    run.add_argument("--raw", action="store_true", help="include raw tool output")
    run.add_argument("--criterion", action="store_true",
                     help="also run cargo bench (full profile)")
    run.add_argument("--no-build", action="store_true")
    run.set_defaults(func=cmd_run)

    compare = sub.add_parser("compare", help="compare a report with a baseline")
    compare.add_argument("baseline")
    compare.add_argument("report")
    compare.add_argument("--tolerance", type=float, default=DEFAULT_TOLERANCE)
    compare.add_argument("--allow-machine-mismatch", action="store_true")
    compare.add_argument("--json", action="store_true")
    compare.set_defaults(func=cmd_compare)

    record = sub.add_parser("record", help="copy a report into bench/baselines")
    record.add_argument("report")
    record.add_argument("--name", required=True)
    record.add_argument("--force", action="store_true")
    record.set_defaults(func=cmd_record)

    corpora = sub.add_parser("corpora", help="verify or generate the corpus set")
    corpora.add_argument("--check", action="store_true")
    corpora.add_argument("--regenerate", action="store_true")
    corpora.add_argument("--update", action="store_true",
                         help="write the computed hashes into the manifest")
    corpora.set_defaults(func=cmd_corpora)

    selftest = sub.add_parser("selftest", help="check the comparator")
    selftest.set_defaults(func=cmd_selftest)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
