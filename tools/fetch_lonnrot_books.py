#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Stage the Projekti Lönnrot texts for the demo's reader.

The prose fixtures under `tests/fixtures/prose/sources/lonnrot/` are the
public-domain novels the grammar corpus uses. This script copies them into
`torille-wasm/demo/books/lonnrot/`, decodes the ISO-8859-1 files to UTF-8,
strips the Projekti Lönnrot notice, and writes the small
`lonnrot/catalog.json` that `rebuild_book_catalog.py` merges into the main
catalogue.

    python3 tools/fetch_lonnrot_books.py

The output directory is under the demo's gitignored `books/` tree, like the
Gutenberg corpus.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / "tests" / "fixtures" / "prose" / "sources" / "lonnrot"
OUT = REPO / "torille-wasm" / "demo" / "books" / "lonnrot"

# (id, author, title)
BOOKS = [
    (559, "Volter Kilpi", "Bathseba"),
    (779, "Maria Jotuni", "Tyttö ruusutarhassa"),
    (886, "Joel Lehtonen", "Sorron lapset (Punainen mies I)"),
    (988, "Joel Lehtonen", "Punainen mylly"),
    (2508, "Maiju Lassila", "Pirttipohjalaiset"),
    (3715, "Pentti Haanpää", "Karavaani ja muita juttuja"),
    (3717, "Pentti Haanpää", "Iisakki vähäpuheinen"),
]


def read_text(path: Path) -> str:
    raw = path.read_bytes()
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        return raw.decode("iso-8859-1")


def strip_notice(text: str) -> str:
    """Drop the Projekti Lönnrot licence notice, keep the title page."""
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if "Tämän e-kirjan ovat tuottaneet" not in line:
            continue
        rest = index + 1
        while rest < len(lines) and not lines[rest].strip():
            rest += 1
        return "\n".join(lines[rest:]).lstrip("\n")
    return text


def source_path(book_id: int) -> Path:
    padded = SOURCE / f"{book_id:04d}.txt"
    return padded if padded.exists() else SOURCE / f"{book_id}.txt"


def main() -> int:
    if not SOURCE.is_dir():
        print(f"fetch_lonnrot_books: missing {SOURCE}", file=sys.stderr)
        return 2
    OUT.mkdir(parents=True, exist_ok=True)
    entries = []
    for book_id, author, title in BOOKS:
        path = source_path(book_id)
        if not path.exists():
            print(f"fetch_lonnrot_books: missing {path}", file=sys.stderr)
            return 2
        text = strip_notice(read_text(path))
        (OUT / f"{book_id}.txt").write_text(text, encoding="utf-8")
        entries.append({"id": book_id, "title": title, "author": author, "source": "lonnrot"})
    (OUT / "catalog.json").write_text(
        json.dumps(entries, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"fetch_lonnrot_books: wrote {len(entries)} texts to {OUT.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
