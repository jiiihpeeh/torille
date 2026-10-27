#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Parse Finnish Wiktionary dump XML and write to SQLite in a format compatible with torille lexicons.

Extracts Finnish sections, POS, inflection templates, and declension/conjugation tables.
Can produce both noun-style and verb-style schemas or a unified schema.
"""
import argparse
import json
import os
import re
import sqlite3
import sys
import time
from html import unescape
from typing import Any, Dict, List, Optional, Tuple
from xml.etree import ElementTree as ET

NS = {"mw": "http://www.mediawiki.org/xml/export-0.11/"}

# Case labels for parsing declension tables
CASES = [
    "nominatiivi", "genetiivi", "partitiivi", "inessiivi", "elatiivi",
    "illatiivi", "adessiivi", "ablatiivi", "allatiivi", "essiivi",
    "translatiivi", "abessiivi", "instruktiivi",
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

BOUNDARY_MARKER = "\u2019"


def clean_form(value: str) -> str:
    value = re.sub(r"\(.*?\)", "", value)
    value = value.split("+")[0].strip()
    value = value.replace(BOUNDARY_MARKER, "")
    if value in ("–", "—", "-", ""):
        return "-"
    return value


def cells_of(row_html: str) -> List[str]:
    cells = []
    for part in re.split(r"<t[hd][^>]*>|</t[hd]>", row_html):
        first = re.split(r"<br[^>]*>|\n", part, maxsplit=1)[0]
        text = unescape(re.sub(r"<[^>]+>", "", first))
        text = re.sub(r"\s+", " ", text).strip()
        if text:
            cells.append(text)
    return cells


def extract_finnish_section_wikitext(wikitext: str) -> str:
    # Find ==Suomi== or ==suomi==
    m = re.search(r"^==\s*Suomi\s*==\s*$", wikitext, re.M | re.I)
    if not m:
        return ""
    start = m.end()
    # Find next language section at same level
    m2 = re.search(r"^==\s*[^=\s][^=]*\s*==\s*$", wikitext[start:], re.M)
    if m2:
        return wikitext[start:start + m2.start()]
    return wikitext[start:]


def extract_pos_from_wikitext(section: str) -> Optional[str]:
    # Look for ===Substantiivi=== etc
    for m in re.finditer(r"^===\s*([^=\n]+?)\s*===\s*$", section, re.M):
        text = m.group(1).lower()
        for fi, en in POS_MAP.items():
            if fi == text or fi in text:
                return en
    return None


def extract_inflection_templates(wikitext: str) -> List[str]:
    templates = []
    for m in re.finditer(r"\{\{\s*([a-z0-9_-]+)", wikitext, re.I):
        name = m.group(1).lower()
        if name.startswith("fi-") and any(
            name.startswith(p) for p in ["fi-subs", "fi-noun", "fi-verb", "fi-adj", "fi-decl", "fi-conj"]
        ):
            templates.append(name)
    seen = set()
    out = []
    for t in templates:
        if t in seen:
            continue
        seen.add(t)
        out.append(t)
    return out


def parse_declension_from_html(html_section: str) -> Optional[Dict[str, Any]]:
    # Try Taivutus heading
    start = html_section.find('id="Taivutus"')
    if start == -1:
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
    out = {}
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
        "singular": {c: s for c, s in zip(CASES, singular)},
        "plural": {c: p for c, p in zip(CASES, plural)},
        "comitative_plural_stem": comitative,
        "raw": "|".join(singular + plural + [comitative]),
    }


def parse_adjective_declension_from_html(html_section: str) -> Optional[Dict[str, Any]]:
    table_pos = html_section.find("Positiivi")
    if table_pos == -1:
        return None
    start = html_section.rfind("<table", 0, table_pos)
    if start == -1:
        return None
    end = html_section.find("</table>", start)
    if end == -1:
        return None
    out = {}
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
        "singular": {c: s for c, s in zip(CASES, singular)},
        "plural": {c: p for c, p in zip(CASES, plural)},
        "comitative_plural_stem": comitative,
        "raw": "|".join(singular + plural + [comitative]),
        "degree": "positive",
    }


def detect_register(wikitext: str) -> Dict[str, Any]:
    """Detect register/style markers like puhekieli, slangi, arkikieli, vanhentunut etc."""
    register = {
        "colloquial": False,  # puhekieli
        "slang": False,
        "archaic": False,
        "obsolete": False,  # vanhentunut
        "dialectal": False,
        "formal": False,
        "childish": False,
    }
    
    # Check for various register templates
    if re.search(r'\{\{\s*(puhekieli|puhek|puhek\.)\s*\}\}', wikitext, re.I):
        register["colloquial"] = True
    if re.search(r'\{\{\s*(slangi|slang)\s*\}\}', wikitext, re.I):
        register["slang"] = True
    if re.search(r'\{\{\s*(arkikieli|arkinen)\s*\}\}', wikitext, re.I):
        register["colloquial"] = True  # often grouped
    if re.search(r'\{\{\s*(vanhentunut|vanh)\s*\}\}', wikitext, re.I):
        register["obsolete"] = True
        register["archaic"] = True
    if re.search(r'\{\{\s*(murteellinen|murre)\s*\}\}', wikitext, re.I):
        register["dialectal"] = True
    if re.search(r'\{\{\s*(lapsenkieli|lasten kieli)\s*\}\}', wikitext, re.I):
        register["childish"] = True
    
    # Also check for {{keskustelu}} or other context
    return register


def extract_definitions(wikitext: str) -> List[str]:
    """Extract definition lines (# ...) from the Finnish section."""
    defs = []
    for line in wikitext.split('\n'):
        line = line.strip()
        if re.match(r'^#+[^#*]', line):  # definition, not sublist
            # Remove leading # and templates
            defn = re.sub(r'^#+\s*', '', line)
            defn = re.sub(r'\{\{[^}]+\}\}', '', defn)
            defn = re.sub(r'\[\[([^\]|]+)(\|[^\]]+)?\]\]', r'\1', defn)
            defn = defn.strip()
            if defn and not defn.startswith(':'):
                defs.append(defn)
    return defs


def init_db_noun_style(conn: sqlite3.Connection):
    conn.execute("""
        CREATE TABLE IF NOT EXISTS base (
            lemma          TEXT NOT NULL,
            homonym        TEXT NOT NULL DEFAULT '',
            part_of_speech TEXT NOT NULL,
            class          TEXT NOT NULL,
            PRIMARY KEY (lemma, homonym)
        ) WITHOUT ROWID
    """)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS forms (
            lemma      TEXT NOT NULL,
            homonym    TEXT NOT NULL DEFAULT '',
            form       TEXT NOT NULL,
            "case"     TEXT NOT NULL,
            number     TEXT NOT NULL,
            possessive TEXT NOT NULL DEFAULT '',
            ok         TEXT NOT NULL CHECK (ok IN ('both', 'omorfi')),
            PRIMARY KEY (lemma, homonym, form, "case", number, possessive)
        ) WITHOUT ROWID
    """)
    conn.execute("CREATE INDEX IF NOT EXISTS forms_lemma ON forms (lemma, homonym)")
    conn.commit()


def init_db_unified(conn: sqlite3.Connection):
    conn.execute("""
        CREATE TABLE IF NOT EXISTS base (
            lemma          TEXT NOT NULL,
            homonym        TEXT NOT NULL DEFAULT '',
            part_of_speech TEXT NOT NULL,
            class          TEXT NOT NULL,
            inflection_templates TEXT,
            register_colloquial INTEGER DEFAULT 0,
            register_slang INTEGER DEFAULT 0,
            register_archaic INTEGER DEFAULT 0,
            register_obsolete INTEGER DEFAULT 0,
            register_dialectal INTEGER DEFAULT 0,
            definitions TEXT,
            PRIMARY KEY (lemma, homonym)
        ) WITHOUT ROWID
    """)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS forms (
            lemma      TEXT NOT NULL,
            homonym    TEXT NOT NULL DEFAULT '',
            form       TEXT NOT NULL,
            "case"     TEXT NOT NULL,
            number     TEXT NOT NULL,
            possessive TEXT NOT NULL DEFAULT '',
            voice      TEXT DEFAULT '',
            mood       TEXT DEFAULT '',
            tense      TEXT DEFAULT '',
            person     TEXT DEFAULT '',
            infinitive TEXT DEFAULT '',
            participle TEXT DEFAULT '',
            polarity   TEXT DEFAULT '',
            reading    TEXT DEFAULT '',
            ok         TEXT NOT NULL DEFAULT 'omorfi',
            PRIMARY KEY (lemma, homonym, form, "case", number, possessive, voice, mood, tense, person)
        ) WITHOUT ROWID
    """)
    conn.execute("CREATE INDEX IF NOT EXISTS forms_lemma ON forms (lemma, homonym)")
    conn.commit()


def map_case_finnish_to_english(case_fi: str) -> str:
    mapping = {
        "nominatiivi": "nominative",
        "genetiivi": "genitive",
        "partitiivi": "partitive",
        "inessiivi": "inessive",
        "elatiivi": "elative",
        "illatiivi": "illative",
        "adessiivi": "adessive",
        "ablatiivi": "ablative",
        "allatiivi": "allative",
        "essiivi": "essive",
        "translatiivi": "translative",
        "abessiivi": "abessive",
        "instruktiivi": "instructive",
        "komitatiivi": "comitative",
    }
    return mapping.get(case_fi.lower(), case_fi)


def main(argv: List[str]) -> int:
    p = argparse.ArgumentParser(description="Parse Finnish Wiktionary dump to SQLite")
    p.add_argument("--input", required=True, help="Path to fiwiktionary pages-articles.xml")
    p.add_argument("--output", required=True, help="Output SQLite file")
    p.add_argument("--schema", choices=["noun-style", "unified"], default="unified",
                   help="Database schema to use")
    p.add_argument("--pos", help="Comma-separated POS to include (noun,verb,adjective,...)")
    p.add_argument("--namespaces", default="0", help="Comma-separated namespaces (default: 0)")
    p.add_argument("--include-redirects", action="store_true", help="Include redirects")
    p.add_argument("--limit", type=int, help="Limit number of pages")
    p.add_argument("--ok", default="omorfi", help="ok value for forms (omorfi or both)")
    args = p.parse_args(argv[1:])
    
    namespaces = set(args.namespaces.split(","))
    pos_filter = None
    if args.pos:
        pos_filter = set(p.strip() for p in args.pos.split(","))
    
    conn = sqlite3.connect(args.output)
    if args.schema == "noun-style":
        init_db_noun_style(conn)
    else:
        init_db_unified(conn)
    
    cur = conn.cursor()
    
    context = ET.iterparse(args.input, events=("end",))
    count = 0
    processed = 0
    
    for event, elem in context:
        if elem.tag == f"{NS['mw']}page" or elem.tag.endswith("}page"):
            ns_elem = elem.find(f"{NS['mw']}ns") or elem.find("ns")
            ns = ns_elem.text if ns_elem is not None and ns_elem.text else ""
            
            if ns not in namespaces:
                elem.clear()
                continue
            
            title_elem = elem.find(f"{NS['mw']}title") or elem.find("title")
            title = title_elem.text if title_elem is not None and title_elem.text else ""
            
            # Check redirect
            redirect_elem = elem.find(f"{NS['mw']}redirect") or elem.find("redirect")
            if redirect_elem is not None and not args.include_redirects:
                elem.clear()
                continue
            
            revision = elem.find(f"{NS['mw']}revision") or elem.find("revision")
            if revision is None:
                elem.clear()
                continue
            
            text_elem = revision.find(f"{NS['mw']}text") or revision.find("text")
            if text_elem is None or text_elem.text is None:
                elem.clear()
                continue
            
            wikitext = text_elem.text
            
            # Extract Finnish section
            fi_section = extract_finnish_section_wikitext(wikitext)
            if not fi_section:
                elem.clear()
                continue
            
            pos = extract_pos_from_wikitext(fi_section)
            if not pos:
                elem.clear()
                continue
            
            if pos_filter and pos not in pos_filter:
                elem.clear()
                continue
            
            templates = extract_inflection_templates(wikitext)
            register = detect_register(wikitext)
            definitions = extract_definitions(fi_section)
            
            # Try to get declension (for nouns/adjectives)
            declension = None
            # We don't have HTML here, but maybe we can parse from wikitext patterns
            # Or skip for now - focus on what we can extract
            
            if args.schema == "noun-style":
                if pos in ("noun", "proper_noun", "pronoun", "numeral", "adjective"):
                    class_str = ",".join(templates) if templates else ""
                    try:
                        cur.execute("INSERT OR IGNORE INTO base VALUES (?,?,?,?)",
                                    (title, "", pos, class_str))
                    except Exception:
                        pass
            else:  # unified
                class_str = ",".join(templates) if templates else ""
                try:
                    cur.execute("""INSERT OR IGNORE INTO base VALUES (?,?,?,?,?,?,?,?,?,?)""",
                                (title, "", pos, class_str, 
                                 json.dumps(templates, ensure_ascii=False),
                                 1 if register["colloquial"] else 0,
                                 1 if register["slang"] else 0,
                                 1 if register["archaic"] else 0,
                                 1 if register["obsolete"] else 0,
                                 1 if register["dialectal"] else 0,
                                 json.dumps(definitions, ensure_ascii=False)))
                except Exception as e:
                    print(f"Error inserting base for {title}: {e}", file=sys.stderr)
            
            processed += 1
            if processed % 1000 == 0:
                conn.commit()
                print(f"Processed {processed} entries...", file=sys.stderr)
            
            if args.limit and processed >= args.limit:
                break
            
            elem.clear()
            count += 1
        
    conn.commit()
    conn.close()
    print(f"Done. Processed {processed} Finnish entries.", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
