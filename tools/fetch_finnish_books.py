#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Download the Finnish Project Gutenberg corpus for the demo.

Lists every Finnish book from gutendex (the metadata API), saves each
one's UTF-8 plain text to `<out>/<id>.txt`, and writes
`<out>/catalog.json` (id, title, author). The demo's reader then serves
books from disk: no gutendex, no gutenberg.org, no CORS, no `/proxy`.

Idempotent: an existing non-empty `<id>.txt` is skipped, so re-running
resumes.

Usage:
    python3 tools/fetch_finnish_books.py
    python3 tools/fetch_finnish_books.py --out torille-wasm/demo/books --jobs 8
    python3 tools/fetch_finnish_books.py --limit 20
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

from rebuild_book_catalog import collect, write_catalog

REPO = Path(__file__).resolve().parent.parent
DEFAULT_OUT = REPO / "torille-wasm" / "demo" / "books"
GUTENDEX = "https://gutendex.com/books"
LANGUAGE = "fi"
USER_AGENT = "torille-demo/1.0"


def get(url: str, timeout: float = 60.0, tries: int = 6) -> bytes:
    """GET with a retry for transient failures (5xx / network)."""
    last = "no attempt"
    for attempt in range(tries):
        request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                return response.read()
        except urllib.error.HTTPError as e:
            # 4xx is not transient (404: no such text); 5xx is.
            if e.code < 500:
                raise
            last = f"http {e.code}"
        except Exception as e:  # noqa: BLE001 - urllib raises a wide net
            last = str(e)
        time.sleep(min(8.0, 0.5 * (2**attempt)))
    raise RuntimeError(f"{url}: {last}")


def natural_author(name: str) -> str:
    """'Canth, Minna' -> 'Minna Canth'; a name without a comma is kept."""
    surname, sep, forename = name.partition(", ")
    return f"{forename} {surname}" if sep and forename else (surname or name)


def download_text(book_id: int) -> bytes | None:
    """The UTF-8 plain text of `book_id`, or `None` if there is none.

    `cache/epub` is UTF-8 and does not redirect. `ebooks/<id>.txt.utf-8`
    redirects to an `http://` cache URL, and `files/…-0.txt` covers the
    older books; both are fallbacks.
    """
    urls = (
        f"https://www.gutenberg.org/cache/epub/{book_id}/pg{book_id}.txt",
        f"https://www.gutenberg.org/ebooks/{book_id}.txt.utf-8",
        f"https://www.gutenberg.org/files/{book_id}/{book_id}-0.txt",
    )
    for url in urls:
        try:
            data = get(url)
            if data:
                return data
        except Exception:
            continue
    return None


def have(out: Path, book_id: int) -> bool:
    path = out / f"{book_id}.txt"
    return path.is_file() and path.stat().st_size > 0


def download_one(out: Path, book_id: int) -> tuple[int, bool]:
    if have(out, book_id):
        return book_id, True
    data = download_text(book_id)
    if data is None:
        return book_id, False
    try:
        (out / f"{book_id}.txt").write_bytes(data)
    except OSError as e:
        print(f"  write {book_id}: {e}", file=sys.stderr)
        return book_id, False
    return book_id, True


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--out", default=str(DEFAULT_OUT), help="output directory")
    ap.add_argument("--jobs", type=int, default=8, help="parallel downloads")
    ap.add_argument("--limit", type=int, default=0, help="stop after N books")
    args = ap.parse_args()

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    # List and download in the same pass: each page's books are queued for
    # download immediately, so the (slow) gutendex listing does not hold up
    # the (fast) text downloads.
    entries: list[dict] = []
    failed: list[int] = []
    queued = 0
    done = 0

    def drain(futures: list, announce: bool) -> None:
        nonlocal done
        for future in as_completed(futures):
            book_id, ok = future.result()
            done += 1
            if not ok:
                failed.append(book_id)
            if announce and (done % 25 == 0 or done == queued):
                print(f"  {done}/{queued} texts ({len(failed)} failed)", flush=True)

    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        futures: list = []
        url: str | None = f"{GUTENDEX}?languages={LANGUAGE}"
        while url:
            data = json.loads(get(url))
            for book in data.get("results", []):
                authors = [
                    natural_author(a["name"])
                    for a in book.get("authors", [])
                    if a.get("name")
                ]
                entry = {
                    "id": book["id"],
                    "title": (book.get("title") or "").strip(),
                    "author": ", ".join(authors),
                }
                entries.append(entry)
                if not have(out, entry["id"]):
                    futures.append(pool.submit(download_one, out, entry["id"]))
                    queued += 1
            url = data.get("next")
            print(
                f"  listed {len(entries)}, queued {queued}, done {done}…", flush=True
            )
            if args.limit and len(entries) >= args.limit:
                break
        drain(futures, announce=True)

    # Build the catalogue from every text on disk, not only the entries this
    # run happened to list. That keeps pre-existing books visible after a
    # partial fetch and lets the reader serve the whole local corpus.
    available = collect(out)
    catalog_path = write_catalog(out, available)
    print(
        f"fetch-books: {len(available)} books available, {len(failed)} failed; "
        f"catalog at {catalog_path}",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
