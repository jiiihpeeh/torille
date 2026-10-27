#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Parse Finnish Wiktionary dump XML and write to SQLite"""
import argparse
import json
import re
import sqlite3
import sys
from html import unescape
from typing import Any, Dict, List, Optional
from xml.etree import ElementTree as ET

NS = "http://www.mediawiki.org/xml/export-0.11/"

CASES = [
    "nominatiivi", "genetiivi", "partitiivi", "inessiivi", "elatiivi",
    "illatiivi", "adessiivi", "ablatiivi", "allatiivi", "essiivi",
    "translatiivi", "abessiivi", "instruktiivi",
]
LABELS = CASES + ["komitatiivi"]

POS_MAP = {
    "substantiivi": "noun", "nimi": "noun", "erisnimi": "proper_noun",
    "verbi": "verb", "adjektiivi": "adjective", "adverbi": "adverb",
    "pronomini": "pronoun", "numeraali": "numeral", "partikkeli": "particle",
    "konjunktio": "conjunction", "prepositio": "preposition",
    "postpositio": "postposition", "interjektio": "interjection",
}


def qname(tag):
    return f"{{{NS}}}{tag}"


def extract_finnish_section_wikitext(wikitext: str) -> str:
    m = re.search(r"^==\s*Suomi\s*==\s*$", wikitext, re.M | re.I)
    if not m:
        return ""
    start = m.end()
    m2 = re.search(r"^==\s*[^=\s][^=]*\s*==\s*$", wikitext[start:], re.M)
    if m2:
        return wikitext[start:start + m2.start()]
    return wikitext[start:]


def extract_pos_from_wikitext(section: str) -> Optional[str]:
    for m in re.finditer(r"^===\s*([^=\n]+?)\s*===\s*$", section, re.M):
        text = m.group(1).lower()
        for fi, en in POS_MAP.items():
            if fi == text or text.startswith(fi):
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
        if t in seen: continue
        seen.add(t)
        out.append(t)
    return out


def detect_register(wikitext: str) -> Dict[str, bool]:
    reg = {"colloquial": False, "slang": False, "archaic": False, 
           "obsolete": False, "dialectal": False, "childish": False}
    if re.search(r'\{\{\s*(puhekieli|puhek|puhek\.)\s*\}\}', wikitext, re.I):
        reg["colloquial"] = True
    if re.search(r'\{\{\s*(slangi|slang)\s*\}\}', wikitext, re.I):
        reg["slang"] = True
    if re.search(r'\{\{\s*(arkikieli|arkinen)\s*\}\}', wikitext, re.I):
        reg["colloquial"] = True
    if re.search(r'\{\{\s*(vanhentunut|vanh)\s*\}\}', wikitext, re.I):
        reg["obsolete"] = True
        reg["archaic"] = True
    if re.search(r'\{\{\s*(murteellinen|murre)\s*\}\}', wikitext, re.I):
        reg["dialectal"] = True
    if re.search(r'\{\{\s*(lapsenkieli|lasten kieli)\s*\}\}', wikitext, re.I):
        reg["childish"] = True
    return reg


def extract_definitions(wikitext: str) -> List[str]:
    defs = []
    for line in wikitext.split('\n'):
        line = line.strip()
        if re.match(r'^#+[^#*]', line):
            defn = re.sub(r'^#+\s*', '', line)
            defn = re.sub(r'\{\{[^}]+\}\}', '', defn)
            defn = re.sub(r'\[\[([^\]|]+)(\|[^\]]+)?\]\]', r'\1', defn)
            defn = defn.strip()
            if defn and not defn.startswith(':'):
                defs.append(defn)
    return defs


def init_db(conn: sqlite3.Connection):
    conn.execute("""CREATE TABLE IF NOT EXISTS base (
        lemma TEXT NOT NULL, homonym TEXT NOT NULL DEFAULT '',
        part_of_speech TEXT NOT NULL, class TEXT NOT NULL,
        inflection_templates TEXT,
        register_colloquial INTEGER DEFAULT 0, register_slang INTEGER DEFAULT 0,
        register_archaic INTEGER DEFAULT 0, register_obsolete INTEGER DEFAULT 0,
        register_dialectal INTEGER DEFAULT 0, register_childish INTEGER DEFAULT 0,
        definitions TEXT,
        PRIMARY KEY (lemma, homonym)
    ) WITHOUT ROWID""")
    conn.execute("""CREATE TABLE IF NOT EXISTS forms (
        lemma TEXT NOT NULL, homonym TEXT NOT NULL DEFAULT '',
        form TEXT NOT NULL, "case" TEXT NOT NULL DEFAULT '',
        number TEXT NOT NULL DEFAULT '', possessive TEXT NOT NULL DEFAULT '',
        voice TEXT DEFAULT '', mood TEXT DEFAULT '', tense TEXT DEFAULT '',
        person TEXT DEFAULT '', infinitive TEXT DEFAULT '', participle TEXT DEFAULT '',
        polarity TEXT DEFAULT '', reading TEXT DEFAULT '', ok TEXT NOT NULL DEFAULT 'omorfi',
        PRIMARY KEY (lemma, homonym, form, "case", number, possessive, voice, mood, tense, person)
    ) WITHOUT ROWID""")
    conn.execute("CREATE INDEX IF NOT EXISTS forms_lemma ON forms (lemma, homonym)")
    conn.commit()


def main(argv):
    p = argparse.ArgumentParser()
    p.add_argument("--input", required=True)
    p.add_argument("--output", required=True)
    p.add_argument("--pos", help="Comma-separated POS")
    p.add_argument("--namespaces", default="0")
    p.add_argument("--include-redirects", action="store_true")
    p.add_argument("--limit", type=int)
    args = p.parse_args(argv[1:])
    
    namespaces = set(args.namespaces.split(","))
    pos_filter = set(args.pos.split(",")) if args.pos else None
    
    conn = sqlite3.connect(args.output)
    init_db(conn)
    cur = conn.cursor()
    
    count = 0
    processed = 0
    
    for event, elem in ET.iterparse(args.input, events=("end",)):
        tag = elem.tag
        if tag == qname("page"):
            ns_elem = elem.find(qname("ns"))
            ns = ns_elem.text if ns_elem is not None else ""
            if ns not in namespaces:
                elem.clear()
                continue
            
            title_elem = elem.find(qname("title"))
            title = title_elem.text if title_elem is not None else ""
            
            if elem.find(qname("redirect")) is not None and not args.include_redirects:
                elem.clear()
                continue
            
            rev = elem.find(qname("revision"))
            if rev is None:
                elem.clear()
                continue
            text_elem = rev.find(qname("text"))
            if text_elem is None or not text_elem.text:
                elem.clear()
                continue
            
            wikitext = text_elem.text
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
            class_str = ",".join(templates)
            
            try:
                cur.execute("""INSERT OR IGNORE INTO base VALUES (?,?,?,?,?,?,?,?,?,?,?,?)""",
                    (title, "", pos, class_str,
                     json.dumps(templates, ensure_ascii=False),
                     int(register["colloquial"]), int(register["slang"]),
                     int(register["archaic"]), int(register["obsolete"]),
                     int(register["dialectal"]), int(register["childish"]),
                     json.dumps(definitions, ensure_ascii=False)))
            except Exception as e:
                print(f"err {title}: {e}", file=sys.stderr)
            
            processed += 1
            if processed % 2000 == 0:
                conn.commit()
                print(f"Processed {processed}...", file=sys.stderr)
            if args.limit and processed >= args.limit:
                break
            elem.clear()
    
    conn.commit()
    conn.close()
    print(f"Done. {processed} entries", file=sys.stderr)
    return 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
