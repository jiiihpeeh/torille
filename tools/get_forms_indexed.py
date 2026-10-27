#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Fast form extraction using prebuilt index"""
import json
import sys
import os
from extract_finnish_forms_from_dump import (
    extract_finnish_section_wikitext, extract_pos_from_wikitext, 
    get_template_calls, extract_forms_from_template
)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INDEX_PATH = os.path.join(REPO, "data", "fiwiktionary_index.json")

def get_forms_indexed(lemma):
    if not os.path.exists(INDEX_PATH):
        print("Index not found. Run build_wiktionary_index.py first.", file=sys.stderr)
        return []
    
    # Load index (could be slow first time, but better than parsing XML each time)
    with open(INDEX_PATH, 'r', encoding='utf-8') as f:
        index = json.load(f)
    
    if lemma not in index:
        return []
    
    wikitext = index[lemma]
    fi_section = extract_finnish_section_wikitext(wikitext)
    if not fi_section:
        return []
    pos = extract_pos_from_wikitext(fi_section)
    if not pos:
        return []
    templates = get_template_calls(fi_section)
    if not templates:
        return []
    all_forms = {}
    for tmpl in templates:
        forms = extract_forms_from_template(tmpl['name'], tmpl['params'], lemma)
        all_forms.update(forms)
    if all_forms:
        return [{'lemma': lemma, 'pos': pos, 'templates': templates, 
                 'forms': all_forms, 'forms_fi': {k.replace('_', ' '): v for k, v in all_forms.items()}}]
    return []

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: get_forms_indexed.py <lemma>")
        sys.exit(1)
    lemma = sys.argv[1]
    forms = get_forms_indexed(lemma)
    if not forms:
        print(f"No forms found for {lemma}")
        sys.exit(1)
    for item in forms:
        print(json.dumps(item['forms_fi'], ensure_ascii=False, indent=2))
