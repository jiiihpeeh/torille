#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Check the structural contract of the feature-reference Markdown.

This is deliberately small and dependency-free. It catches the failures that
made the first grammar/syntax reference hard to read:

* a Markdown table whose rows have different column counts;
* a relative link that no longer resolves;
* a duplicate heading anchor;
* an orphan triple-backtick fence.

Usage:
    python3 tools/check_docs.py [PATH ...]

The default is `docs/grammar_syntax.md`.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

DEFAULT = ["docs/grammar_syntax.md"]


def slugify(text: str) -> str:
    text = text.strip().lower()
    text = re.sub(r"[`*_~]", "", text)
    text = re.sub(r"[^\w\s-]", "", text)
    return re.sub(r"\s+", "-", text)


def check_table_block(
    path: Path, start: int, rows: list[tuple[int, int]]
) -> list[str]:
    errors: list[str] = []
    if len(rows) < 2:
        return errors
    width = rows[0][1]
    for line_number, cells in rows[1:]:
        if cells != width:
            errors.append(
                f"{path}:{line_number}: table row has {cells} cells, "
                f"header has {width}"
            )
    return errors


def check_file(path: Path) -> list[str]:
    errors: list[str] = []
    text = path.read_text(encoding="utf-8")
    if text.count("```") % 2:
        errors.append(f"{path}: unbalanced triple-backtick fence")

    in_fence = False
    table: list[tuple[int, int]] = []
    table_start = 0
    for line_number, line in enumerate(text.splitlines(), 1):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if stripped.startswith("|") and stripped.endswith("|"):
            if not table:
                table_start = line_number
            table.append((line_number, stripped.count("|") - 1))
        else:
            errors.extend(check_table_block(path, table_start, table))
            table = []
    errors.extend(check_table_block(path, table_start, table))

    for match in re.finditer(r"\]\(([^)\s]+)", text):
        target = match.group(1)
        if target.startswith(("http://", "https://", "mailto:", "#", "/")):
            continue
        link_path = target.split("#", 1)[0]
        if link_path and not (path.parent / link_path).exists():
            errors.append(f"{path}: unresolved link {target!r}")

    seen: dict[str, int] = {}
    for line_number, line in enumerate(text.splitlines(), 1):
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

    return errors


def main(argv: list[str]) -> int:
    paths = [Path(argument) for argument in argv] or [Path(DEFAULT[0])]
    errors: list[str] = []
    for path in paths:
        if not path.exists():
            errors.append(f"{path}: file does not exist")
            continue
        errors.extend(check_file(path))
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        print(f"{len(errors)} documentation contract error(s)", file=sys.stderr)
        return 1
    print("documentation contract ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
