#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build an index for fast lookup in Wiktionary XML dump"""
import json
import os
import re
import sys
from xml.etree import ElementTree as ET

NS = "http://www.mediawiki.org/xml/export-0.11/"
def qname(tag): return f"{{{NS}}}{tag}"

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DUMP_PATH = os.path.join(REPO, "data", "fiwiktionary-20261001-pages-articles.xml")
INDEX_PATH = os.path.join(REPO, "data", "fiwiktionary_index.json")

def main():
    dump_path = DUMP_PATH
    index_path = INDEX_PATH
    
    index = {}
    count = 0
    
    for event, elem in ET.iterparse(dump_path, events=("end",)):
        if elem.tag == qname('page'):
            ns_elem = elem.find(qname('ns'))
            ns = ns_elem.text if ns_elem is not None else ""
            if ns != '0':
                elem.clear()
                continue
            
            title_elem = elem.find(qname('title'))
            title = title_elem.text if title_elem is not None else ""
            
            if elem.find(qname('redirect')) is not None:
                elem.clear()
                continue
            
            rev = elem.find(qname('revision'))
            if rev is None:
                elem.clear()
                continue
            text_elem = rev.find(qname('text'))
            if text_elem is None or not text_elem.text:
                elem.clear()
                continue
            
            wikitext = text_elem.text
            # Quick check if has Finnish section
            if re.search(r'==\s*Suomi\s*==', wikitext, re.I):
                index[title] = wikitext
            
            count += 1
            if count % 10000 == 0:
                print(f"Indexed {count} pages, {len(index)} Finnish entries...", file=sys.stderr)
            elem.clear()
    
    print(f"Saving index with {len(index)} entries...", file=sys.stderr)
    with open(index_path, 'w', encoding='utf-8') as f:
        json.dump(index, f, ensure_ascii=False)
    print(f"Done. Index saved to {index_path}", file=sys.stderr)

if __name__ == '__main__':
    main()
