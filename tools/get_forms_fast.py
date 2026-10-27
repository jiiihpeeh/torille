#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Fast form extraction using grep to find specific page"""
import json
import re
import sys
import subprocess
from extract_finnish_forms_from_dump import (
    extract_finnish_section_wikitext, extract_pos_from_wikitext, 
    get_template_calls, extract_forms_from_template
)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_DUMP = os.path.join(
    REPO, "data", "fiwiktionary-20261001-pages-articles.xml")


def get_forms_fast(lemma, dump_path=DEFAULT_DUMP):
    # Use Python with offset
    try:
        with open(dump_path, 'r', encoding='utf-8') as f:
            # Search for the title
            content = f.read()
        # Find page containing this title
        # Look for <page> ... <title>lemma</title> ... </page>
        pattern = r'<page>\s*<title>' + re.escape(lemma) + r'</title>.*?</page>'
        m = re.search(pattern, content, re.S)
        if not m:
            return []
        page_xml = m.group(0)
        # Extract wikitext from revision/text
        text_m = re.search(r'<text[^>]*>(.*?)</text>', page_xml, re.S)
        if not text_m:
            return []
        wikitext = text_m.group(1)
        # Unescape basic XML entities
        wikitext = wikitext.replace('&lt;', '<').replace('&gt;', '>').replace('&amp;', '&').replace('&quot;', '"').replace('&apos;', "'")
        
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
    except Exception as e:
        print(e, file=sys.stderr)
        return []

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: get_forms_fast.py <lemma>")
        sys.exit(1)
    lemma = sys.argv[1]
    forms = get_forms_fast(lemma)
    if not forms:
        print(f"No forms found for {lemma}")
        sys.exit(1)
    for item in forms:
        print(json.dumps(item['forms_fi'], ensure_ascii=False, indent=2))
