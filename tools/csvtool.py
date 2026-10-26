#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""A small, dependency-free CSV/TSV command-line toolkit.

The Linux CSV CLIs (csvkit, mlr, xsv) are not installed here, and the repo's
data files are tab-separated, so this covers the common read/filter/select/
sort/count/edit operations with the Python standard library only.

Delimiter: tab by default (`--csv` switches to a comma, `--delimiter C` to any
character). Columns on the command line are **1-based**. Pass `--header` when
the first row is a header (it is preserved by `sort`/`cut`/`filter` and shown
by `cols`).

Commands
--------
    head   FILE [-n N]                     first N rows (default 10)
    cols   FILE [--header]                 the column indices (and header)
    cut    FILE -f 1,4 [--header]          select columns
    filter FILE -c 4 --eq 12 [--header]    keep rows where column matches
    sort   FILE -k 1 [-r] [--numeric]      sort rows by a column
    count  FILE [-c 4]                     row count, or per distinct value
    grep   FILE PATTERN [-c N]             rows matching a regex
    add    FILE --row "a<TAB>b" [--sorted -k 1]   append or sorted-insert

Operators for `filter`: `--eq V`, `--ne V`, `--contains V`, `--in "a,b"`,
`--re REGEX`, `--gt N`, `--lt N` (the last two numeric).

Examples
--------
    python3 tools/csvtool.py count data/nykysuomensanalista.txt -c 4
    python3 tools/csvtool.py filter data/nykysuomensanalista.txt -c 4 --eq 12
    python3 tools/csvtool.py grep data/nykysuomensanalista.txt '^pluto'
    python3 tools/csvtool.py cut data/nykysuomensanalista.csv --header -f 1,4

`-o FILE` writes to a file instead of stdout (in place is allowed). `add`
edits the file in place; `--dry-run` reports without writing.
"""

from __future__ import annotations

import argparse
import csv
import os
import re
import sys
from pathlib import Path


def read_rows(path: str, delim: str, header: bool):
    with open(path, "r", newline="", encoding="utf-8") as f:
        rows = list(csv.reader(f, delimiter=delim))
    if header and rows:
        return rows[0], rows[1:]
    return None, rows


def write_rows(rows, delim: str, out) -> None:
    csv.writer(out, delimiter=delim, lineterminator="\n").writerows(rows)


def col(spec: str, header_row) -> int:
    """A 1-based column spec -> 0-based index (or a header name)."""
    spec = spec.strip()
    if spec.isdigit():
        return int(spec) - 1
    if header_row and spec in header_row:
        return header_row.index(spec)
    raise SystemExit(f"unknown column {spec!r}")


def parse_columns(spec: str, header_row) -> list[int]:
    return [col(s, header_row) for s in spec.split(",") if s.strip()]


def matches(value: str, args) -> bool:
    if args.eq is not None:
        return value == args.eq
    if args.ne is not None:
        return value != args.ne
    if args.contains is not None:
        return args.contains in value
    if args.in_ is not None:
        return value in {v.strip() for v in args.in_.split(",")}
    if args.re_ is not None:
        return re.search(args.re_, value) is not None
    if args.gt is not None:
        return _num(value) > float(args.gt)
    if args.lt is not None:
        return _num(value) < float(args.lt)
    raise SystemExit("filter needs one operator (--eq/--ne/--contains/--in/--re/--gt/--lt)")


def _num(value: str) -> float:
    try:
        return float(value)
    except ValueError:
        return float("-inf")


def collation_key():
    """A Finnish alphabetical sort key, when the locale is available.

    The sanalista is in Finnish order (`ä`/`ö` after `z`, leading hyphens
    ignored). `fi_FI.UTF-8` is installed here, so `locale.strxfrm` matches it;
    otherwise fall back to stripping leading punctuation.
    """
    import locale

    for loc in ("fi_FI.UTF-8", "fi_FI.utf8", "fi_FI"):
        try:
            locale.setlocale(locale.LC_COLLATE, loc)
            return locale.strxfrm
        except locale.Error:
            continue

    def stripped(s: str) -> str:
        i = 0
        while i < len(s) and not s[i].isalnum():
            i += 1
        return s[i:]

    return stripped


def emit(rows, header_row, args) -> None:
    all_rows = ([header_row] if header_row is not None else []) + rows
    if args.output:
        target = sys.stdout if args.output == "-" else open(args.output, "w", newline="", encoding="utf-8")
        try:
            write_rows(all_rows, args.delimiter, target)
        finally:
            if target is not sys.stdout:
                target.close()
    else:
        write_rows(all_rows, args.delimiter, sys.stdout)


def main() -> int:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--csv", action="store_true", help="comma-separated (default: tab)")
    common.add_argument("--delimiter", default=None, help="explicit delimiter character")
    common.add_argument("--header", action="store_true", help="the first row is a header")

    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = ap.add_subparsers(dest="command", required=True)

    p = sub.add_parser("head", parents=[common], help="first N rows")
    p.add_argument("file")
    p.add_argument("-n", type=int, default=10)

    p = sub.add_parser("cols", parents=[common], help="show column indices")
    p.add_argument("file")

    p = sub.add_parser("cut", parents=[common], help="select columns")
    p.add_argument("file")
    p.add_argument("-f", required=True, help="comma-separated columns (1-based or header names)")

    p = sub.add_parser("filter", parents=[common], help="keep matching rows")
    p.add_argument("file")
    p.add_argument("-c", required=True, help="column")
    p.add_argument("--eq")
    p.add_argument("--ne")
    p.add_argument("--contains")
    p.add_argument("--in", dest="in_", metavar="A,B")
    p.add_argument("--re", dest="re_")
    p.add_argument("--gt")
    p.add_argument("--lt")

    p = sub.add_parser("sort", parents=[common], help="sort rows by a column")
    p.add_argument("file")
    p.add_argument("-k", required=True, help="column")
    p.add_argument("-r", action="store_true", help="reverse")
    p.add_argument("--numeric", action="store_true")

    p = sub.add_parser("count", parents=[common], help="row count, or per distinct value")
    p.add_argument("file")
    p.add_argument("-c", help="count per distinct value of this column")
    p.add_argument("--unique", action="store_true", help="distinct values only")

    p = sub.add_parser("grep", parents=[common], help="rows matching a regex")
    p.add_argument("file")
    p.add_argument("pattern")
    p.add_argument("-c", help="restrict to a column")

    p = sub.add_parser("add", parents=[common], help="append or insert a row")
    p.add_argument("file")
    p.add_argument("--row", required=True, help="delimiter-separated row")
    p.add_argument("--after", metavar="WORD", help="insert after the row whose first field is WORD")
    p.add_argument("--before", metavar="WORD", help="insert before the row whose first field is WORD")
    p.add_argument("--sorted", action="store_true",
                   help="best-effort sorted insert (the sanalista uses a custom collation)")
    p.add_argument("-k", default="1", help="sort key column when --sorted")
    p.add_argument("--dry-run", action="store_true")

    for p in sub.choices.values():
        p.add_argument("-o", "--output", default=None, help="write to FILE instead of stdout")

    args = ap.parse_args()
    args.delimiter = args.delimiter or ("," if args.csv else "\t")

    header_row, rows = read_rows(args.file, args.delimiter, args.header)

    if args.command == "head":
        emit(rows[: args.n], header_row, args)
    elif args.command == "cols":
        width = max(len(header_row or []), max((len(r) for r in rows), default=0))
        for i in range(width):
            name = header_row[i] if header_row and i < len(header_row) else ""
            print(f"{i + 1}\t{name}")
    elif args.command == "cut":
        idxs = parse_columns(args.f, header_row)
        emit([[r[i] if i < len(r) else "" for i in idxs] for r in rows],
             [header_row[i] if header_row and i < len(header_row) else "" for i in idxs]
             if header_row else None, args)
    elif args.command == "filter":
        i = col(args.c, header_row)
        emit([r for r in rows if i < len(r) and matches(r[i], args)], header_row, args)
    elif args.command == "sort":
        i = col(args.k, header_row)
        key = (lambda r: _num(r[i]) if i < len(r) else float("-inf")) if args.numeric \
            else (lambda r: r[i] if i < len(r) else "")
        rows = sorted(rows, key=key, reverse=args.r)
        emit(rows, header_row, args)
    elif args.command == "count":
        if not args.c:
            print(len(rows))
            return 0
        i = col(args.c, header_row)
        counts: dict[str, int] = {}
        for r in rows:
            v = r[i] if i < len(r) else ""
            counts[v] = counts.get(v, 0) + 1
        if args.unique:
            for v in sorted(counts):
                print(v)
        else:
            for v, n in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0])):
                print(f"{n}\t{v}")
    elif args.command == "grep":
        i = col(args.c, header_row) if args.c else None
        pat = re.compile(args.pattern)
        hit = [r for r in rows if pat.search(r[i] if i is not None and i < len(r) else args.delimiter.join(r))]
        emit(hit, header_row, args)
    elif args.command == "add":
        new = next(csv.reader([args.row], delimiter=args.delimiter))
        if args.after is not None or args.before is not None:
            anchor = args.after if args.after is not None else args.before
            pos = None
            for n, r in enumerate(rows):
                if r and r[0] == anchor:
                    pos = n + 1 if args.after is not None else n
                    break
            if pos is None:
                raise SystemExit(f"anchor {anchor!r} not found")
        elif args.sorted:
            key_index = col(args.k, header_row)
            sk = collation_key()
            pos = len(rows)
            for n, r in enumerate(rows):
                if sk(r[key_index] if key_index < len(r) else "") > sk(
                    new[key_index] if key_index < len(new) else ""
                ):
                    pos = n
                    break
        else:
            pos = len(rows)
        if args.dry_run:
            print(f"would insert at row {pos + 1}")
            return 0
        rows.insert(pos, new)
        if not args.output:
            args.output = args.file
        had_newline = Path(args.file).read_bytes().endswith(b"\n")
        emit(rows, header_row, args)
        # The repo's sanalista has no trailing newline; csv.writer always adds
        # one, which would show up as a spurious change. Preserve the state.
        if args.output == args.file and not had_newline:
            data = Path(args.file).read_bytes()
            if data.endswith(b"\n"):
                Path(args.file).write_bytes(data[:-1])
    return 0


if __name__ == "__main__":
    import signal

    try:
        signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    except (AttributeError, ValueError):
        pass
    try:
        sys.exit(main())
    except BrokenPipeError:
        os._exit(0)
