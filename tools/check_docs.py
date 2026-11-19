#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Check the structural contract of the feature-reference Markdown.

This is deliberately small and dependency-free. It catches the failures that
made the grammar/syntax reference hard to read and keeps the source
plain-text-first:

* a Markdown table whose rows have different column counts;
* a table wider than the render-safe column limit;
* a table cell that is long enough to collapse in a line-based renderer;
* box-drawing art outside a fenced code block;
* a heading without a blank line before or after it;
* a relative link that no longer resolves;
* a duplicate heading anchor;
* an orphan triple-backtick fence.

With `--render`, it also runs `pandoc -f gfm -t plain` and rejects box-drawing
characters or over-wide lines in the exported text. The export is a review
artifact, never a committed second copy.

Usage:
    python3 tools/check_docs.py [--render] [PATH ...]

The default is `docs/grammar_syntax.md`.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

DEFAULT = ["docs/grammar_syntax.md"]
BOX_DRAWING = set("┌┐└┘├┤┬┴┼│─━┃┏┓┗┛┣┫┳┻╋")
DEFAULT_MAX_COLUMNS = 3
DEFAULT_CELL_WARN = 90
DEFAULT_CELL_ERROR = 140
DEFAULT_RENDER_MAX_LINE = 160


def slugify(text: str) -> str:
    text = text.strip().lower()
    text = re.sub(r"[`*_~]", "", text)
    text = re.sub(r"[^\w\s-]", "", text)
    return re.sub(r"\s+", "-", text)


def table_cells(stripped: str) -> list[str]:
    return [cell.strip() for cell in stripped.strip("|").split("|")]


def is_separator_row(cells: list[str]) -> bool:
    return all(re.fullmatch(r":?-{3,}:?", cell) for cell in cells)


def check_table_block(
    path: Path,
    rows: list[tuple[int, list[str]]],
    max_columns: int,
    cell_warn: int,
    cell_error: int,
) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    if len(rows) < 2:
        return errors, warnings

    header_line, header = rows[0]
    width = len(header)
    if width > max_columns:
        errors.append(
            f"{path}:{header_line}: table has {width} columns, "
            f"limit is {max_columns}"
        )
    for line_number, cells in rows[1:]:
        if len(cells) != width:
            errors.append(
                f"{path}:{line_number}: table row has {len(cells)} cells, "
                f"header has {width}"
            )
    for line_number, cells in rows:
        if is_separator_row(cells):
            continue
        for column, cell in enumerate(cells, 1):
            length = len(cell)
            if length > cell_error:
                errors.append(
                    f"{path}:{line_number}: table cell {column} is {length} "
                    f"characters, hard limit is {cell_error}"
                )
            elif length > cell_warn:
                warnings.append(
                    f"{path}:{line_number}: table cell {column} is {length} "
                    f"characters, recommended limit is {cell_warn}"
                )
    return errors, warnings


def check_file(
    path: Path,
    max_columns: int,
    cell_warn: int,
    cell_error: int,
) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    if text.count("```") % 2:
        errors.append(f"{path}: unbalanced triple-backtick fence")

    in_fence = False
    table: list[tuple[int, list[str]]] = []
    for line_number, line in enumerate(lines, 1):
        stripped = line.strip()
        if stripped.startswith("```"):
            block_errors, block_warnings = check_table_block(
                path, table, max_columns, cell_warn, cell_error
            )
            errors.extend(block_errors)
            warnings.extend(block_warnings)
            table = []
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        bad = sorted(set(re.sub(r"`[^`\n]*`", "", line)) & BOX_DRAWING)
        if bad:
            errors.append(
                f"{path}:{line_number}: box-drawing character(s) outside a "
                f"fence: {' '.join(bad)}"
            )
        if stripped.startswith("|") and stripped.endswith("|"):
            table.append((line_number, table_cells(stripped)))
        else:
            block_errors, block_warnings = check_table_block(
                path, table, max_columns, cell_warn, cell_error
            )
            errors.extend(block_errors)
            warnings.extend(block_warnings)
            table = []
    block_errors, block_warnings = check_table_block(
        path, table, max_columns, cell_warn, cell_error
    )
    errors.extend(block_errors)
    warnings.extend(block_warnings)

    in_fence = False
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if not re.match(r"^#{1,6}\s+\S", line):
            continue
        line_number = index + 1
        if index > 0 and lines[index - 1].strip():
            errors.append(
                f"{path}:{line_number}: heading needs a blank line before it"
            )
        if index + 1 < len(lines) and lines[index + 1].strip():
            errors.append(
                f"{path}:{line_number}: heading needs a blank line after it"
            )

    for match in re.finditer(r"\]\(([^)\s]+)", text):
        target = match.group(1)
        if target.startswith(("http://", "https://", "mailto:", "#", "/")):
            continue
        link_path = target.split("#", 1)[0]
        if link_path and not (path.parent / link_path).exists():
            errors.append(f"{path}: unresolved link {target!r}")

    seen: dict[str, int] = {}
    for line_number, line in enumerate(lines, 1):
        heading = re.match(r"^#{1,6}\s+(.*)$", line)
        if not heading:
            continue
        slug = slugify(heading.group(1))
        if slug in seen:
            errors.append(
                f"{path}:{line_number}: duplicate heading anchor {slug!r} "
                f"(first at line {seen[slug]})"
            )
        else:
            seen[slug] = line_number

    return errors, warnings


def check_render(path: Path) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    pandoc = shutil.which("pandoc")
    if not pandoc:
        return errors, ["pandoc is not installed; render check skipped"]
    result = subprocess.run(
        [
            pandoc,
            str(path),
            "-f",
            "gfm",
            "-t",
            "plain",
            "--wrap=auto",
            "--columns=100",
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        errors.append(
            f"{path}: pandoc plain render failed: {result.stderr.strip()}"
        )
        return errors, warnings
    for line_number, line in enumerate(result.stdout.splitlines(), 1):
        bad = sorted(set(re.sub(r"`[^`\n]*`", "", line)) & BOX_DRAWING)
        if bad:
            errors.append(
                f"{path}: pandoc output line {line_number} has box-drawing "
                f"character(s): {' '.join(bad)}"
            )
        if len(line) > DEFAULT_RENDER_MAX_LINE:
            warnings.append(
                f"{path}: pandoc output line {line_number} is {len(line)} "
                f"characters, wider than {DEFAULT_RENDER_MAX_LINE}"
            )
    return errors, warnings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", help="Markdown files to check")
    parser.add_argument(
        "--max-columns",
        type=int,
        default=DEFAULT_MAX_COLUMNS,
        help="maximum table columns (default: %(default)s)",
    )
    parser.add_argument(
        "--cell-warn",
        type=int,
        default=DEFAULT_CELL_WARN,
        help="warn above this table-cell length (default: %(default)s)",
    )
    parser.add_argument(
        "--cell-error",
        type=int,
        default=DEFAULT_CELL_ERROR,
        help="fail above this table-cell length (default: %(default)s)",
    )
    parser.add_argument(
        "--render",
        action="store_true",
        help="also export with pandoc -t plain and check the result",
    )
    args = parser.parse_args(argv)

    paths = [Path(argument) for argument in args.paths] or [Path(DEFAULT[0])]
    errors: list[str] = []
    warnings: list[str] = []
    for path in paths:
        if not path.exists():
            errors.append(f"{path}: file does not exist")
            continue
        file_errors, file_warnings = check_file(
            path, args.max_columns, args.cell_warn, args.cell_error
        )
        errors.extend(file_errors)
        warnings.extend(file_warnings)
        if args.render:
            render_errors, render_warnings = check_render(path)
            errors.extend(render_errors)
            warnings.extend(render_warnings)

    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        print(f"{len(errors)} documentation contract error(s)", file=sys.stderr)
        return 1
    print("documentation contract ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
