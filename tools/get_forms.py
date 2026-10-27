#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Quick way to get all forms for a Finnish word from Wiktionary dump"""
import json
import subprocess
import sys
import os

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_DUMP = os.path.join(
    REPO, "data", "fiwiktionary-20261001-pages-articles.xml")


def get_forms(lemma, dump_path=DEFAULT_DUMP):
    script = os.path.join(REPO, 'tools', 'extract_finnish_forms_from_dump.py')
    result = subprocess.run([
        sys.executable, script,
        '--input', dump_path,
        '--output', '/tmp/_forms_tmp.jsonl',
        '--lemma', lemma
    ], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    
    forms = []
    if os.path.exists('/tmp/_forms_tmp.jsonl'):
        with open('/tmp/_forms_tmp.jsonl') as f:
            for line in f:
                if line.strip():
                    forms.append(json.loads(line))
    return forms

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: get_forms.py <lemma>")
        sys.exit(1)
    lemma = sys.argv[1]
    forms = get_forms(lemma)
    if not forms:
        print(f"No forms found for {lemma}")
        sys.exit(1)
    for item in forms:
        print(json.dumps(item['forms_fi'], ensure_ascii=False, indent=2))
