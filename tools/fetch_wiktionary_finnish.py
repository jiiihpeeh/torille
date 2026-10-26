#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Download Finnish words from Finnish Wiktionary with morphological information.

This script fetches Finnish lemmas from Finnish Wiktionary categories (e.g.
substantives, verbs, adjectives) and extracts morphological data from the
Finnish section: part of speech, inflection class/templates, and parsed
declension/conjugation forms where possible.

It is designed to complement tools/fetch_wiktionary.py (which only fetches
Kotus model nouns). The output is JSONL suitable for building lexicons or
feeding into torille's tooling.

Usage:
  python3 tools/fetch_wiktionary_finnish.py --pos noun --limit 1000
  python3 tools/fetch_wiktionary_finnish.py --pos noun,verb,adjective --output data/wiktionary_fi.jsonl
  python3 tools/fetch_wiktionary_finnish.py --category "Luokka:Suomen kielen substantiivit" --max-pages 5
"""

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from html import unescape
from typing import Any, Dict, Iterable, List, Optional, Tuple

CACHE = os.path.join(os.path.dirname(__file__), "..", ".cache", "wiktionary")
API = "https://fi.wiktionary.org/w/api.php"

# Finnish Wiktionary categories for common POS
DEFAULT_CATEGORIES = {
    "noun": "Luokka:Suomen kielen substantiivit",
    "verb": "Luokka:Suomen kielen verbit",
    "adjective": "Luokka:Suomen kielen adjektiivit",
    "adverb": "Luokka:Suomen kielen adverbit",
    "pronoun": "Luokka:Suomen kielen pronominit",
    "numeral": "Luokka:Suomen kielen numeraalit",
    "proper_noun": "Luokka:Suomen kielen erisnimet",
}

# Case labels for parsing declension tables (same as fetch_wiktionary.py)
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
LABELS = CASES + ["komitatiivi"]

POS_MAP = {
    "substantiivi": "noun",
    "nimi": "noun",
    "erisnimi": "proper_noun",
    "verbi": "verb",
    "adjektiivi": "adjective",
    "adverbi": "adverb",
    "pronomini": "pronoun",
    "numeraali": "numeral",
    "partikkeli": "particle",
    "konjunktio": "conjunction",
    "prepositio": "preposition",
    "postpositio": "postposition",
    "interjektio": "interjection",
}


def fetch_json(params: Dict[str, Any]) -> Any:
    """Fetch from MediaWiki API with basic retries and backoff."""
    url = API + "?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(
        url, headers={"User-Agent": "torille-fetch-fi/0.1 (https://github.com/torille)"}
    )
    wait = 2.0
    for attempt in range(6):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return json.load(resp)
        except urllib.error.HTTPError as exc:
            if exc.code != 429 or attempt == 5:
                raise
            print(f"    (429, waiting {wait:.0f}s)", file=sys.stderr)
            time.sleep(wait)
            wait *= 2
        except Exception:
            if attempt == 5:
                raise
            time.sleep(wait)
            wait *= 2


def fetch_html(word: str) -> str:
    """Fetch parsed HTML for a page, cached to .cache/wiktionary/."""
    os.makedirs(CACHE, exist_ok=True)
    cache_path = os.path.join(CACHE, urllib.parse.quote(word, safe="") + ".fi.html")
    if os.path.exists(cache_path):
        with open(cache_path, encoding="utf-8") as f:
            return f.read()
    data = fetch_json(
        {
            "action": "parse",
            "prop": "text",
            "format": "json",
            "formatversion": 2,
            "redirects": 1,
            "page": word,
        }
    )
    html = data["parse"]["text"]
    with open(cache_path, "w", encoding="utf-8") as f:
        f.write(html)
    time.sleep(0.5)
    return html


def fetch_wikitext(word: str) -> str:
    """Fetch raw wikitext for a page (useful for templates)."""
    os.makedirs(CACHE, exist_ok=True)
    cache_path = os.path.join(CACHE, urllib.parse.quote(word, safe="") + ".fi.wikitext")
    if os.path.exists(cache_path):
        with open(cache_path, encoding="utf-8") as f:
            return f.read()
    data = fetch_json(
        {
            "action": "query",
            "prop": "revisions",
            "rvprop": "content",
            "format": "json",
            "formatversion": 2,
            "redirects": 1,
            "titles": word,
        }
    )
    try:
        pages = data["query"]["pages"]
        if not pages:
            return ""
        revs = pages[0].get("revisions") or []
        if not revs:
            return ""
        text = revs[0].get("content") or ""
    except Exception:
        text = ""
    with open(cache_path, "w", encoding="utf-8") as f:
        f.write(text)
    time.sleep(0.5)
    return text


def get_category_members(category: str, limit: Optional[int] = None) -> List[str]:
    """Iterate category members (namespace 0 by default)."""
    members = []
    cmcontinue = None
    while True:
        params: Dict[str, Any] = {
            "action": "query",
            "list": "categorymembers",
            "cmtitle": category,
            "cmlimit": "500",
            "format": "json",
            "formatversion": 2,
        }
        if cmcontinue:
            params["cmcontinue"] = cmcontinue
        data = fetch_json(params)
        cm = data.get("query", {}).get("categorymembers") or []
        for m in cm:
            if m.get("ns") == 0:  # main namespace
                members.append(m["title"])
                if limit and len(members) >= limit:
                    return members
        cmcontinue = data.get("continue", {}).get("cmcontinue")
        if not cmcontinue:
            break
        time.sleep(0.3)
    return members


def cells_of(row: str) -> List[str]:
    cells = []
    for part in re.split(r"<t[hd][^>]*>|</t[hd]>", row):
        first = re.split(r"<br[^>]*>|\n", part, maxsplit=1)[0]
        text = unescape(re.sub(r"<[^>]+>", "", first))
        text = re.sub(r"\s+", " ", text).strip()
        if text:
            cells.append(text)
    return cells


def clean_form(value: str) -> str:
    value = re.sub(r"\(.*?\)", "", value)
    value = value.split("+")[0].strip()
    if value in ("–", "—", "-", ""):
        return "-"
    return value


def extract_finnish_section(html: str) -> str:
    """Extract the ==Suomi== section HTML roughly."""
    # Try to find Finnish language header
    m = re.search(r'<span[^>]+id=["\']Suomi["\'][^>]*>.*?</span>', html, re.I)
    if not m:
        m = re.search(r'>\s*Suomi\s*<', html, re.I)
    if not m:
        return html
    start = m.start()
    # Find next language header at h2 level
    # Look for next <h2> after start
    h2s = list(re.finditer(r'<h2[^>]*>.*?</h2>', html[start:], re.S | re.I))
    if h2s:
        end = start + h2s[0].start()
        return html[start:end]
    return html[start:]


def parse_declension_table(html_section: str) -> Optional[Dict[str, Any]]:
    """Parse noun/adjective declension tables similar to fetch_wiktionary.py."""
    start = html_section.find('id="Taivutus"')
    if start == -1:
        # try Finnish heading text
        m = re.search(r'>\s*Taivutus\s*<', html_section, re.I)
        if m:
            start = m.start()
    if start == -1:
        return None
    table_start = html_section.find("<table", start)
    if table_start == -1:
        return None
    table_end = html_section.find("</table>", table_start)
    if table_end == -1:
        return None
    table_html = html_section[table_start:table_end]
    out: Dict[str, Tuple[str, str]] = {}
    for row in re.findall(r"<tr[^>]*>(.*?)</tr>", table_html, re.S):
        cells = cells_of(row)
        for i, cell in enumerate(cells):
            case = cell.lower().rstrip(".")
            if case not in LABELS:
                continue
            if len(cells) < i + 3:
                continue
            out[case] = (clean_form(cells[i + 1]), clean_form(cells[i + 2]))
            break
    if not out:
        return None
    singular = [out.get(c, ("-", "-"))[0] for c in CASES]
    plural = [out.get(c, ("-", "-"))[1] for c in CASES]
    comitative = out.get("komitatiivi", ("-", "-"))[1]
    return {
        "singular": dict(zip(CASES, singular)),
        "plural": dict(zip(CASES, plural)),
        "comitative_plural_stem": comitative,
        "raw": "|".join(singular + plural + [comitative]),
    }


def parse_adjective_declension(html_section: str) -> Optional[Dict[str, Any]]:
    table_pos = html_section.find("Positiivi")
    if table_pos == -1:
        return None
    start = html_section.rfind("<table", 0, table_pos)
    if start == -1:
        return None
    end = html_section.find("</table>", start)
    if end == -1:
        return None
    out: Dict[str, Tuple[str, str]] = {}
    for row in re.findall(r"<tr[^>]*>(.*?)</tr>", html_section[start:end], re.S):
        cells = cells_of(row)
        if len(cells) < 3:
            continue
        case = cells[0].lower().rstrip(".")
        if case not in LABELS:
            continue
        out[case] = (clean_form(cells[1]), clean_form(cells[2]))
    if not out:
        return None
    singular = [out.get(c, ("-", "-"))[0] for c in CASES]
    plural = [out.get(c, ("-", "-"))[1] for c in CASES]
    comitative = out.get("komitatiivi", ("-", "-"))[1]
    return {
        "singular": dict(zip(CASES, singular)),
        "plural": dict(zip(CASES, plural)),
        "comitative_plural_stem": comitative,
        "raw": "|".join(singular + plural + [comitative]),
        "degree": "positive",
    }


def extract_inflection_templates(wikitext: str) -> List[str]:
    """Find Finnish inflection templates like {{fi-subs-...}}, {{fi-verb-...}} etc."""
    templates = []
    for m in re.finditer(r"\{\{\s*([a-z0-9_-]+)", wikitext, re.I):
        name = m.group(1).lower()
        if name.startswith("fi-") and any(
            name.startswith(p) for p in ["fi-subs", "fi-noun", "fi-verb", "fi-adj", "fi-decl", "fi-conj"]
        ):
            # capture full template roughly
            # simple: just name for now
            templates.append(name)
    # deduplicate preserving order
    seen = set()
    out = []
    for t in templates:
        if t in seen:
            continue
        seen.add(t)
        out.append(t)
    return out


def extract_pos_from_section(html_section: str) -> Optional[str]:
    """Find POS header in Finnish section."""
    # Look for h3/h4 like ===Substantiivi===
    for m in re.finditer(r'<h[34][^>]*>.*?</h[34]>', html_section, re.S | re.I):
        text = unescape(re.sub(r"<[^>]+>", "", m.group(0))).lower()
        text = re.sub(r"\s+", " ", text).strip()
        # remove === etc if any
        text = text.strip("= ")
        for fi, en in POS_MAP.items():
            if fi in text:
                return en
    # also try plain text
    for fi, en in POS_MAP.items():
        if re.search(r"\b" + re.escape(fi) + r"\b", html_section, re.I):
            # be a bit careful not to match elsewhere
            pass
    return None


def fetch_word_morph(word: str) -> Dict[str, Any]:
    result: Dict[str, Any] = {
        "lemma": word,
        "source": "fi.wiktionary.org",
        "pos": None,
        "inflection_templates": [],
        "declension": None,
        "notes": [],
    }
    try:
        wikitext = fetch_wikitext(word)
        result["inflection_templates"] = extract_inflection_templates(wikitext)
    except Exception as e:
        result["notes"].append(f"wikitext_error:{e}")

    try:
        html = fetch_html(word)
        section = extract_finnish_section(html)
        pos = extract_pos_from_section(section)
        if pos:
            result["pos"] = pos
        # try declension
        decl = parse_declension_table(section)
        if not decl and result["pos"] in ("adjective",):
            decl = parse_adjective_declension(section)
        if decl:
            result["declension"] = decl
        # if adjective didn't match, try generic
        if not decl and result["pos"] == "adjective":
            decl = parse_adjective_declension(section)
            if decl:
                result["declension"] = decl
    except Exception as e:
        result["notes"].append(f"html_error:{e}")

    return result


def main(argv: List[str]) -> int:
    p = argparse.ArgumentParser(description="Fetch Finnish words from Wiktionary with morphology")
    p.add_argument("--pos", help="Comma-separated POS: noun,verb,adjective,adverb,pronoun,numeral,proper_noun", default="noun")
    p.add_argument("--category", action="append", help="Custom category title (can be repeated)")
    p.add_argument("--limit", type=int, help="Limit total words fetched")
    p.add_argument("--max-pages", type=int, help="Limit number of pages to process")
    p.add_argument("--output", default="data/wiktionary_finnish_morph.jsonl", help="Output JSONL file")
    p.add_argument("--delay", type=float, default=0.5, help="Delay between word fetches (seconds)")
    p.add_argument("--skip-existing", action="store_true", help="Skip words already in output")
    args = p.parse_args(argv[1:] if len(argv) > 0 and argv[0].endswith('.py') else argv)

    categories = []
    if args.category:
        categories.extend(args.category)
    else:
        for pos in args.pos.split(","):
            pos = pos.strip()
            if pos in DEFAULT_CATEGORIES:
                categories.append(DEFAULT_CATEGORIES[pos])
            elif pos:
                print(f"Warning: unknown pos '{pos}', skipping", file=sys.stderr)

    if not categories:
        print("No categories to fetch", file=sys.stderr)
        return 1

    # Collect members
    all_words: List[str] = []
    seen_words = set()
    for cat in categories:
        print(f"Fetching category members: {cat}", file=sys.stderr)
        try:
            members = get_category_members(cat, limit=args.limit)
        except Exception as e:
            print(f"Error fetching {cat}: {e}", file=sys.stderr)
            continue
        for w in members:
            if w in seen_words:
                continue
            seen_words.add(w)
            all_words.append(w)
            if args.limit and len(all_words) >= args.limit:
                break
        if args.limit and len(all_words) >= args.limit:
            break

    print(f"Collected {len(all_words)} unique words", file=sys.stderr)

    # Load existing if skipping
    existing = set()
    if args.skip_existing and os.path.exists(args.output):
        try:
            with open(args.output, encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        obj = json.loads(line)
                        if "lemma" in obj:
                            existing.add(obj["lemma"])
                    except Exception:
                        pass
        except Exception:
            pass

    os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
    out_f = open(args.output, "a", encoding="utf-8")

    processed = 0
    for word in all_words:
        if args.max_pages and processed >= args.max_pages:
            break
        if word in existing:
            continue
        print(f"[{processed+1}/{len(all_words)}] {word}", file=sys.stderr)
        try:
            morph = fetch_word_morph(word)
            out_f.write(json.dumps(morph, ensure_ascii=False) + "\n")
            out_f.flush()
        except Exception as e:
            print(f"  ERROR: {e}", file=sys.stderr)
            err = {"lemma": word, "error": str(e), "source": "fi.wiktionary.org"}
            out_f.write(json.dumps(err, ensure_ascii=False) + "\n")
            out_f.flush()
        processed += 1
        time.sleep(args.delay)

    out_f.close()
    print(f"Done. Wrote to {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
