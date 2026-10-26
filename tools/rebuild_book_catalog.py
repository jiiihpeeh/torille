#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Rebuild the demo's Gutenberg catalogue from the local text files.

`tools/fetch_finnish_books.py` writes `catalog.json` from the gutendex
listing. That is correct on a fresh download, but it cannot see texts that
are already on disk after a partial fetch, and a hand-edited catalogue can
easily fall behind the corpus. This script scans every `books/<id>.txt`,
reads the Project Gutenberg header, and writes an entry for each text:

    {"id": 10480, "title": "Kauppa-Lopo", "author": "Minna Canth"}

No network is used. Run it after adding/removing texts, or pass
`--check` in CI to fail when the catalogue is stale.

Usage:
    python3 tools/rebuild_book_catalog.py
    python3 tools/rebuild_book_catalog.py --books-dir torille-wasm/demo/books
    python3 tools/rebuild_book_catalog.py --check
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_BOOKS_DIR = REPO / "torille-wasm" / "demo" / "books"

# Header fields that identify the creator, in preference order. `Author`
# is the normal one; the others cover anthologies and translated works.
AUTHOR_FIELDS = ("Author", "Adapter", "Editor", "Compiler", "Translator", "Illustrator")
FIELD_RE = re.compile(
    r"^(Title|" + "|".join(AUTHOR_FIELDS) + r"):[ \t]*(.*)$",
)
START_RE = re.compile(r"\*\*\*\s*START OF (THE|THIS)?\s*PROJECT GUTENBERG", re.I)


class Entry:
    __slots__ = ("id", "title", "author")

    def __init__(self, book_id: int, title: str, author: str) -> None:
        self.id = book_id
        self.title = title
        self.author = author

    def as_dict(self) -> dict:
        return {"id": self.id, "title": self.title, "author": self.author}


def header_of(text: str) -> str:
    """The Project Gutenberg metadata block, before the body marker."""
    match = START_RE.search(text)
    return text[: match.start()] if match else text[:8000]


def parse_header(head: str) -> dict[str, str]:
    """Read `Field: value` pairs, joining indented continuation lines."""
    fields: dict[str, str] = {}
    current: str | None = None
    for raw_line in head.splitlines():
        line = raw_line.rstrip()
        if not line.strip():
            current = None
            continue
        match = FIELD_RE.match(line)
        if match:
            current = match.group(1).lower()
            fields[current] = match.group(2).strip()
            continue
        if current is not None and raw_line[:1].isspace():
            continuation = line.strip()
            if continuation:
                fields[current] = f"{fields[current]} {continuation}".strip()
    return fields


def clean_field(value: str) -> str:
    """Collapse whitespace and Gutenberg's MARC-style `$b` separators."""
    value = re.sub(r"\s*\$[a-z]\s*", " : ", value)
    value = re.sub(r"\s+", " ", value)
    return value.strip(" .;:")


def read_entry(path: Path) -> Entry:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        text = ""
    fields = parse_header(header_of(text))
    title = clean_field(fields.get("title", ""))
    author = ""
    for field in AUTHOR_FIELDS:
        candidate = clean_field(fields.get(field.lower(), ""))
        if candidate:
            author = candidate
            break
    if not title:
        # A few older files have no header. Use the first non-empty line
        # after the START marker, or the id as a last resort.
        body = text
        match = START_RE.search(text)
        if match:
            body = text[match.end() :]
        for line in body.splitlines():
            candidate = line.strip()
            if candidate:
                title = candidate[:120]
                break
    if not title:
        title = f"Kirja #{path.stem}"
    return Entry(int(path.stem), title, author or "Tuntematon")


def collect(books_dir: Path) -> list[Entry]:
    entries = [
        read_entry(path)
        for path in books_dir.glob("*.txt")
        if path.stem.isdigit()
    ]
    entries.sort(key=lambda entry: entry.id)
    return entries


def write_catalog(books_dir: Path, entries: list[Entry]) -> Path:
    path = books_dir / "catalog.json"
    payload = json.dumps([entry.as_dict() for entry in entries], ensure_ascii=False, indent=2)
    path.write_text(payload + "\n", encoding="utf-8")
    return path


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--books-dir", default=str(DEFAULT_BOOKS_DIR))
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when catalog.json does not match the texts on disk",
    )
    args = parser.parse_args()

    books_dir = Path(args.books_dir)
    if not books_dir.is_dir():
        print(f"rebuild-book-catalog: no such directory: {books_dir}", file=sys.stderr)
        return 2

    entries = collect(books_dir)
    if not entries:
        print(f"rebuild-book-catalog: no <id>.txt files in {books_dir}", file=sys.stderr)
        return 2

    catalog_path = books_dir / "catalog.json"
    if args.check:
        try:
            current = json.loads(catalog_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            current = None
        expected = [entry.as_dict() for entry in entries]
        if current != expected:
            print(
                f"rebuild-book-catalog: {catalog_path} is stale "
                f"({len(current) if isinstance(current, list) else 0} entries, "
                f"{len(expected)} texts); rerun without --check",
                file=sys.stderr,
            )
            return 1
        print(f"rebuild-book-catalog: {catalog_path} is up to date ({len(entries)} books)")
        return 0

    write_catalog(books_dir, entries)
    authors = len({entry.author for entry in entries})
    print(f"rebuild-book-catalog: wrote {len(entries)} books ({authors} authors) to {catalog_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
