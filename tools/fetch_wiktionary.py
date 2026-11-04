#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Fetch every model noun of the Kotus catalogue's declension from Wiktionary.

The Finnish Wiktionary renders each noun's declension from a `{{fi-subs-*}}`
template. Parsing the rendered HTML is the reliable way to get the exact table,
so this script asks the MediaWiki API for the expanded page and pulls the
declension table out of it: the rows it keeps are only the ones whose cells
contain a case name, which is what makes a stray table (or the `vartalot` block)
harmless.

The output is `data/wiktionary-taivutukset.txt`, one block per word:

    ### 27 käsi
    käsi|käden|kättä|...|käsin|käsine-

with 27 `|`-separated fields: the thirteen cases (nominative through
instructive) in the singular, those same thirteen in the plural, and the
comitative plural (a stem awaiting its possessive). A `-` where the table had
no primary form.
"""

import csv
import json
import os
import re
import sys
import time
import urllib.parse
import urllib.error
import urllib.request
from html import unescape

CACHE = os.path.join(os.path.dirname(__file__), "..", ".cache", "wiktionary")

API = "https://fi.wiktionary.org/w/api.php"

# The thirteen case labels that make up one number of a declension, in the order
# the table rows them. Accusative is left out: it is the nominative or the
# genitive again.
CASES = [
    "nominatiivi",
    "genetiivi",
    "partitiivi",
    "inessiivi",
    "elatiivi",
    "illatiivi",
    "adessiivi",
    "ablatiivi",
    "allatiivi",
    "essiivi",
    "translatiivi",
    "abessiivi",
    "instruktiivi",
]

# The comitative is only ever plural on a declension page, but it is a row of
# the tables the same way the others are.
LABELS = CASES + ["komitatiivi"]

HEADER = """\
# Wiktionary declensions of the model nouns of the Kotus taivutustyypit,
# fetched from https://fi.wiktionary.org. One block per class:
#   ### <class> <word>
#   <form 1>|<form 2>|...|<form 27>
# Twenty-seven fields: the thirteen cases (nominative through instructive) in
# the singular, the same in the plural, and the comitative plural as the bare
# stem awaiting a possessive. A `-` means the table had no primary form.
"""


def fetch_html(word: str) -> str:
    """The expanded page, cached to `.cache/wiktionary/`. Wikimedia throttles
    unauthenticated API use, so this waits a second between misses and backs
    off on 429s instead of hammering the server."""
    os.makedirs(CACHE, exist_ok=True)
    cache_path = os.path.join(CACHE, urllib.parse.quote(word, safe="") + ".html")
    if os.path.exists(cache_path):
        with open(cache_path, encoding="utf-8") as f:
            return f.read()
    params = urllib.parse.urlencode(
        {
            "action": "parse",
            "prop": "text",
            "format": "json",
            "formatversion": 2,
            "redirects": 1,
            "page": word,
        }
    )
    url = API + "?" + params
    req = urllib.request.Request(
        url, headers={"User-Agent": "torille-fetch/0.1 (https://github.com/torille)"}
    )
    wait = 2.0
    for attempt in range(6):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                html = json.load(resp)["parse"]["text"]
            with open(cache_path, "w", encoding="utf-8") as f:
                f.write(html)
            time.sleep(1.0)
            return html
        except urllib.error.HTTPError as exc:
            if exc.code != 429 or attempt == 5:
                raise
            print(f"    (429 on {word}, waiting {wait:.0f}s)", file=sys.stderr)
            time.sleep(wait)
            wait *= 2


def cells_of(row: str) -> list[str]:
    cells = []
    for part in re.split(r"<t[hd][^>]*>|</t[hd]>", row):
        # Alternates hang in the same cell one per line, and only the primary
        # one is wanted, so a cell reads as its first line.
        first = re.split(r"<br[^>]*>|\n", part, maxsplit=1)[0]
        text = unescape(re.sub(r"<[^>]+>", "", first))
        text = re.sub(r"\s+", " ", text).strip()
        if text:
            cells.append(text)
    return cells


def declension(html: str) -> dict[str, tuple[str, str]]:
    """Return {case: (singular, plural)} for the table after the declension
    heading. A row's own case label can sit behind the section header cell that
    covers the other rows, so the first cell of each row is only a case name for
    the row that starts each group; the label is the one cell of the row that
    matches a case, and the two cells after it are the two numbers."""
    start = html.find('id="Taivutus"')
    if start == -1:
        return {}
    table = html.find("<table", start)
    if table == -1:
        return {}
    end = html.find("</table>", table)
    out = {}
    for row in re.findall(r"<tr[^>]*>(.*?)</tr>", html[table:end], re.S):
        cells = cells_of(row)
        for i, cell in enumerate(cells):
            case = cell.lower().rstrip(".")
            if case not in LABELS:
                continue
            if len(cells) < i + 3:
                continue
            out[case] = (clean(cells[i + 1]), clean(cells[i + 2]))
            break
    return out


def clean(value: str) -> str:
    value = re.sub(r"\(.*?\)", "", value)
    value = value.split("+")[0].strip()
    if value in ("–", "—", ""):
        return "-"
    return value


def adjective_declension(html: str) -> dict[str, tuple[str, str]]:
    """The positive-degree half of an `Liite:Adjektiivitaivutus` table. The
    table is three degree groups side by side, each with its own case column,
    so a row is `[case, sg, pl, "", case, sg, pl, "", ...]` and the first three
    cells are the positive degree."""
    table = html.find("Positiivi")
    if table == -1:
        return {}
    # `Positiivi` is a cell of the table, so the `<table>` tag is before it.
    start = html.rfind("<table", 0, table)
    if start == -1:
        return {}
    end = html.find("</table>", start)
    out = {}
    for row in re.findall(r"<tr[^>]*>(.*?)</tr>", html[start:end], re.S):
        cells = cells_of(row)
        if len(cells) < 3:
            continue
        case = cells[0].lower().rstrip(".")
        if case not in LABELS:
            continue
        out[case] = (clean(cells[1]), clean(cells[2]))
    return out


def appendix_title(word: str) -> str:
    return f"Liite:Adjektiivitaivutus/suomi/{word}"


def model_words(path: str) -> dict[int, str]:
    """Read one model word per class.

    `data/catalogue.csv` is the distributed source: the slot-0 (singular
    nominative) form is the class's model word. The older Kotus text file is
    still accepted for a local checkout, one `class` line followed by the word.
    """
    if path.endswith(".csv"):
        words: dict[int, str] = {}
        with open(path, encoding="utf-8") as f:
            reader = csv.DictReader(f, delimiter="\t")
            for row in reader:
                if row.get("slot") != "0":
                    continue
                class_no = int(row["class"])
                words.setdefault(class_no, row["form"])
        return words
    words = {}
    cls = None
    for line in open(path, encoding="utf-8"):
        m = re.match(r"^\s*(\d+)\s*$", line)
        if m:
            cls = int(m.group(1))
            continue
        if cls is not None and line.strip() and cls not in words:
            words[cls] = line.strip()
        if not line.strip():
            cls = None
    return words


def main(argv: list[str]) -> int:
    path = argv[1] if len(argv) > 1 else "data/catalogue.csv"
    words = model_words(path)

    blocks = [HEADER]
    missing = []
    for class_no in range(1, 52):
        word = words.get(class_no)
        if word is None:
            continue
        try:
            page = fetch_html(word)
            table = declension(page)
            # Adjectives keep their paradigm on a `Liite:Adjektiivitaivutus`
            # appendix whose positive degree is the declension itself.
            if not table:
                try:
                    table = adjective_declension(fetch_html(appendix_title(word)))
                except Exception:
                    table = {}
        except Exception as exc:  # noqa: BLE001 - one bad page must not stop the rest
            missing.append((class_no, word, "error: " + str(exc)))
            continue
        if not table:
            missing.append((class_no, word, "no declension table"))
            continue
        singular = [table.get(c, ("-", "-"))[0] for c in CASES]
        plural = [table.get(c, ("-", "-"))[1] for c in CASES]
        comitative = table.get("komitatiivi", ("-", "-"))[1]
        blocks.append(f"### {class_no} {word}")
        blocks.append("|".join(singular + plural + [comitative]))

    out = "\n".join(blocks) + "\n"
    with open("data/wiktionary-taivutukset.txt", "w", encoding="utf-8") as f:
        f.write(out)
    sys.stdout.write(out)
    for class_no, word, why in missing:
        print(f"!! {class_no} {word}: {why}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))