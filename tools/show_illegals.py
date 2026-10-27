#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Show the illegal forms in a `report_by_class.py` report.

`report_by_class.py` answers "what does each class get wrong?" and writes the
whole report, confirmed rows included. This answers the narrower question a
fix starts from: *which non-words does the engine offer?* It reads a report
JSON (or a split directory of `class-*.json` files) and prints every finding
with the requested status, grouped by class.

By default it **refreshes when the report is missing or out of date**. Freshness
is a fingerprint of the engine sources, the embedded data files, the report
harness and the installed Python binding, plus the report's own parameters
(class filter, lemmas per class, bare/possessive). If none of those changed,
the existing report is reused; `--refresh` forces a run and `--no-refresh`
never runs one. The refresh is class-parallel: `-j` sets how many classes are
checked at once (default: the CPU count).

The default status is `illegal`: a `plain` form the oracle (Omorfi/Voikko or
the materialised lexicon) does not recognise. The refresh uses
`data/kotus_nomines.sqlite` by default, because that is the noun oracle the
class reports are baselined against; `--live` switches to live Omorfi+Voikko,
and `--lexicon PATH` names another database. `--status` accepts any of the
report's statuses, comma-separated, so the same tool can show `missing`,
`not_plain`, `mistagged`, or `unsupported` rows without a second script.

Usage:

    python3 tools/show_illegals.py
    python3 tools/show_illegals.py --summary
    python3 tools/show_illegals.py -c 51,21
    python3 tools/show_illegals.py --lemma bébé
    python3 tools/show_illegals.py --status missing
    python3 tools/show_illegals.py --status illegal,not_plain
    python3 tools/show_illegals.py reports/classes/          # split output
    python3 tools/show_illegals.py report.json --json > bad.json
    python3 tools/show_illegals.py report.json --no-refresh  # reuse it

Refreshing runs the Python binding, so rebuild it after a Rust change:

    (cd bindings && env -u CONDA_PREFIX maturin develop --release)

A finding prints as:

    51  särkynytsydän  särkyneensydämen  GEN SG  expected särkyneensydämen

`expected` is only shown when it differs from the form, because for an
`illegal` row the form itself is the interesting half. `--json` writes the
filtered findings with the class added, which is what a follow-up script
wants instead of parsing the human-readable lines.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

STATUSES = ("confirmed", "missing", "not_plain", "mistagged", "illegal",
            "unsupported")


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    p.add_argument("path", nargs="?",
                   help="report JSON file or a directory of class-*.json files "
                        "(default: reports/noun-classes.json)")
    mode = p.add_mutually_exclusive_group()
    mode.add_argument("--refresh", dest="refresh", action="store_true",
                      default=None,
                      help="force a refresh even if the report looks current")
    mode.add_argument("--no-refresh", dest="refresh", action="store_false",
                      help="never rerun report_by_class.py; read what is on disk")
    p.add_argument("-n", "--per-class", type=int, default=5,
                   help="lemmas per class when refreshing (default: 5)")
    p.add_argument("-j", "--jobs", type=int, default=0,
                   help="classes checked at once when refreshing "
                        "(default: CPU count)")
    p.add_argument("--bare", action="store_true",
                   help="refresh only the bare case/number cells")
    p.add_argument("--lexicon", default="data/kotus_nomines.sqlite",
                   metavar="SQLITE",
                   help="noun oracle for the refresh (default: "
                        "data/kotus_nomines.sqlite)")
    p.add_argument("--live", action="store_true",
                   help="refresh against live Omorfi+Voikko instead of the "
                        "materialised lexicon")
    p.add_argument("-s", "--status", default="illegal",
                   help="comma-separated statuses to show (default: illegal). "
                        f"known: {', '.join(STATUSES)}")
    p.add_argument("-c", "--class", dest="classes", metavar="LIST",
                   help="only these classes; accepts 51 or 51,21 or 9*B")
    p.add_argument("--lemma", metavar="TEXT",
                   help="only lemmas containing TEXT (case-sensitive)")
    p.add_argument("-l", "--limit", type=int, default=0, metavar="N",
                   help="at most N findings per class (0 = all)")
    p.add_argument("--summary", action="store_true",
                   help="print per-class counts only, not every finding")
    p.add_argument("--json", action="store_true",
                   help="write the filtered findings as JSON")
    p.add_argument("--fail-if-any", action="store_true",
                   help="exit 1 when at least one finding matches, else 0")
    return p.parse_args(argv)


def class_key(name: str) -> tuple[int, str]:
    """Sort `9` before `9*B` before `10`, not lexicographically."""
    try:
        return (int(name.split("*")[0]), name)
    except ValueError:
        return (1 << 30, name)


def expand_path(path: Path) -> list[Path]:
    if path.is_dir():
        files = sorted(path.glob("class-*.json"))
        if not files:
            raise SystemExit(f"{path}: no class-*.json files")
        return files
    if path.is_file():
        return [path]
    raise SystemExit(f"{path}: no such file or directory")


def read_reports(paths: list[Path]) -> list[dict]:
    reports: list[dict] = []
    for path in paths:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            raise SystemExit(f"{path}: {exc}") from exc
        # A split file and the combined file are both arrays; a bare object is
        # accepted so a hand-written one-class report works too.
        if isinstance(data, dict):
            data = [data]
        if not isinstance(data, list):
            raise SystemExit(f"{path}: expected a JSON array of class reports")
        for report in data:
            if not isinstance(report, dict) or "class" not in report:
                raise SystemExit(f"{path}: a report without a class")
            reports.append(report)
    return reports


def parse_statuses(raw: str) -> list[str]:
    wanted = [s.strip() for s in raw.split(",") if s.strip()]
    unknown = [s for s in wanted if s not in STATUSES]
    if unknown:
        raise SystemExit(
            f"unknown status: {', '.join(unknown)}\n"
            f"known: {', '.join(STATUSES)}")
    return wanted


def parse_classes(raw: str | None) -> set[str] | None:
    if not raw:
        return None
    return {c.strip() for c in raw.split(",") if c.strip()}


def is_split_target(path: Path) -> bool:
    """A JSON file path has a suffix; a bare path is a split directory."""
    return path.is_dir() or (not path.exists() and not path.suffix)


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def input_files(root: Path) -> list[Path]:
    """The files whose content the report depends on.

    Everything that can change a generated form or the harness that judges it:
    the engine and codegen under `src/`/`build/`, the embedded lexicon sources,
    the report harness, and the installed binding. `target/` and the huge
    Wiktionary dump are deliberately absent.
    """
    files: set[Path] = {
        root / "Cargo.toml",
        root / "Cargo.lock",
        root / "build.rs",
        root / "bindings" / "Cargo.toml",
        root / "tools" / "Cargo.toml",
        root / "data" / "nykysuomensanalista.txt",
        root / "data" / "kotus-tyypit.txt",
        root / "data" / "catalogue.csv",
        root / "data" / "kotus_nomines.sqlite",
    }
    for rel in ("src", "build", "bindings/src", "tools/src"):
        base = root / rel
        if base.is_dir():
            files.update(p for p in base.rglob("*") if p.is_file())
    tools = root / "tools"
    if tools.is_dir():
        files.update(p for p in tools.glob("*.py") if p.is_file())
    precomputed = root / "data" / "precomputed"
    if precomputed.is_dir():
        files.update(p for p in precomputed.glob("*") if p.is_file())

    spec = importlib.util.find_spec("torille")
    if spec and spec.submodule_search_locations:
        package = Path(list(spec.submodule_search_locations)[0])
        if package.is_dir():
            files.update(p for p in package.iterdir()
                         if p.is_file() and (p.suffix == ".so" or p.suffix == ".py"))
    return sorted(files)


def current_fingerprint() -> str:
    root = repo_root()
    digest = hashlib.blake2b(digest_size=16)
    for path in input_files(root):
        try:
            stat = path.stat()
        except OSError:
            continue
        try:
            name = path.relative_to(root).as_posix()
        except ValueError:
            name = "installed/" + path.name
        digest.update(f"{name}\0{stat.st_mtime_ns}\0{stat.st_size}\n".encode())
    return digest.hexdigest()


def report_params(args: argparse.Namespace) -> dict:
    return {
        "classes": args.classes or "all",
        "per_class": args.per_class,
        "bare": bool(args.bare),
        "lexicon": None if args.live else args.lexicon,
    }


def meta_path(path: Path) -> Path:
    if is_split_target(path):
        return path / ".report-meta.json"
    return path.with_name(path.name + ".meta.json")


def read_meta(path: Path) -> dict | None:
    try:
        return json.loads(meta_path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def write_meta(path: Path, fingerprint: str, args: argparse.Namespace) -> None:
    meta = {
        "fingerprint": fingerprint,
        "params": report_params(args),
        "generated": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    meta_path(path).write_text(
        json.dumps(meta, indent=2) + "\n", encoding="utf-8")


def needs_refresh(path: Path, args: argparse.Namespace,
                  fingerprint: str) -> tuple[bool, str]:
    if args.refresh is False:
        return False, "forced read"
    if args.refresh is True:
        return True, "forced refresh"
    if not path.exists():
        return True, "no report yet"
    if is_split_target(path) and not any(path.glob("class-*.json")):
        return True, "no class files"
    meta = read_meta(path)
    if meta is None:
        return True, "no freshness metadata"
    if meta.get("fingerprint") != fingerprint:
        return True, "engine, data, harness or binding changed"
    if meta.get("params") != report_params(args):
        return True, "report parameters changed"
    return False, "current"


def refresh_report(path: Path, args: argparse.Namespace,
                   fingerprint: str) -> None:
    """Run report_by_class.py so the file the user reads is current."""
    script = Path(__file__).resolve().with_name("report_by_class.py")
    split = is_split_target(path)
    jobs = args.jobs if args.jobs > 0 else (os.cpu_count() or 4)
    base = [sys.executable, str(script),
            "-n", str(args.per_class), "-j", str(jobs)]
    if args.classes:
        base += ["-c", args.classes]
    if args.bare:
        base.append("--bare")
    if not args.live:
        lexicon = Path(args.lexicon)
        if not lexicon.is_absolute():
            lexicon = repo_root() / lexicon
        if lexicon.exists():
            base += ["--lexicon", str(lexicon)]
        else:
            print(f"{lexicon}: no such lexicon; using live Omorfi+Voikko",
                  file=sys.stderr)

    print(f"refreshing {path} ...", file=sys.stderr)
    if not split:
        path.parent.mkdir(parents=True, exist_ok=True)
        proc = subprocess.run(base + ["-o", str(path)],
                              capture_output=True, text=True)
    else:
        # Build the split in a temporary directory and move it into place only
        # after the report succeeds, so a failed refresh cannot leave a mix of
        # old and missing class files.
        import shutil
        import tempfile
        path.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=path.parent) as tmp:
            proc = subprocess.run(base + ["--split", tmp],
                                  capture_output=True, text=True)
            if proc.returncode in (0, 1):
                for old in path.glob("class-*.json"):
                    old.unlink()
                for new in Path(tmp).glob("class-*.json"):
                    shutil.move(str(new), path / new.name)

    if proc.returncode not in (0, 1):
        sys.stderr.write(proc.stdout)
        sys.stderr.write(proc.stderr)
        raise SystemExit(
            f"report_by_class.py failed ({proc.returncode}); "
            "rebuild the binding and try again")
    write_meta(path, fingerprint, args)


def matches_class(class_name: str, wanted: set[str] | None) -> bool:
    if wanted is None:
        return True
    # `9B` and `9*B` name the same class; accept either spelling.
    plain = {c.replace("*", "").upper() for c in wanted}
    return class_name in wanted or class_name.replace("*", "").upper() in plain


def format_cell(cell: dict) -> str:
    bits = [cell.get("case", ""), cell.get("number", "")]
    if cell.get("possessive"):
        bits.append(f"poss={cell['possessive']}")
    if cell.get("tense"):
        bits.append(cell["tense"])
    if cell.get("mood"):
        bits.append(cell["mood"])
    if cell.get("person"):
        bits.append(cell["person"])
    return " ".join(b for b in bits if b)


def collect(reports: list[dict], args: argparse.Namespace,
            statuses: list[str]) -> dict[str, list[dict]]:
    wanted = parse_classes(args.classes)
    by_class: dict[str, list[dict]] = defaultdict(list)
    for report in reports:
        cls = report["class"]
        if not matches_class(cls, wanted):
            continue
        for finding in report.get("findings", []):
            if finding.get("status") not in statuses:
                continue
            lemma = finding.get("lemma", "")
            if args.lemma and args.lemma not in lemma:
                continue
            row = dict(finding)
            row["class"] = cls
            by_class[cls].append(row)
    for cls in by_class:
        by_class[cls].sort(
            key=lambda f: (f.get("lemma", ""), f.get("form", ""),
                           format_cell(f.get("cell", {}))))
    return by_class


def human_line(cls: str, finding: dict) -> str:
    cell = format_cell(finding.get("cell", {}))
    line = (
        f"{cls:6}  {finding.get('lemma', ''):20}  "
        f"{finding.get('form', ''):28}  {cell}"
    )
    expected = finding.get("expected", "")
    if expected and expected != finding.get("form"):
        line += f"  expected {expected}"
    got = finding.get("got", "")
    if got and got != finding.get("form"):
        line += f"  (engine read {got})"
    if finding.get("status") != "illegal":
        line += f"  [{finding['status']}]"
    return line.rstrip()


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    statuses = parse_statuses(args.status)

    path = Path(args.path) if args.path else Path("reports/noun-classes.json")
    fingerprint = current_fingerprint()
    refresh, reason = needs_refresh(path, args, fingerprint)
    if refresh:
        if reason != "no report yet":
            print(f"{path}: {reason}", file=sys.stderr)
        refresh_report(path, args, fingerprint)
    else:
        print(f"{path}: {reason}; reusing", file=sys.stderr)
    if not path.exists():
        raise SystemExit(f"{path}: no such report")

    reports = read_reports(expand_path(path))
    by_class = collect(reports, args, statuses)

    total = sum(len(rows) for rows in by_class.values())
    shown = 0
    if args.json:
        out = [row for cls in sorted(by_class, key=class_key)
               for row in by_class[cls]]
        print(json.dumps(out, ensure_ascii=False, indent=2))
        shown = len(out)
    elif args.summary:
        for cls in sorted(by_class, key=class_key):
            rows = by_class[cls]
            lemmas = len({r.get("lemma", "") for r in rows})
            forms = len({r.get("form", "") for r in rows})
            print(f"class {cls:6}  {len(rows):5} findings  "
                  f"{lemmas:4} lemmas  {forms:5} distinct forms")
        print(f"total {total} finding(s) across {len(by_class)} class(es)")
    else:
        for cls in sorted(by_class, key=class_key):
            rows = by_class[cls]
            if args.limit > 0:
                rows = rows[: args.limit]
            if not rows:
                continue
            lemmas = len({r.get("lemma", "") for r in by_class[cls]})
            print(f"class {cls}  ({len(by_class[cls])} findings, "
                  f"{lemmas} lemma(s))")
            for finding in rows:
                print("  " + human_line(cls, finding))
            if args.limit and len(by_class[cls]) > args.limit:
                print(f"  ... {len(by_class[cls]) - args.limit} more")
            print()
        print(f"{total} {','.join(statuses)} finding(s) across "
              f"{len(by_class)} class(es)")
        shown = total

    if args.fail_if_any and shown:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
