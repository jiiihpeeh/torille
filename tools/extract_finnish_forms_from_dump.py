#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Extract all Finnish word forms from Wiktionary dump XML.

Parses Finnish sections and inflection templates to generate declension/conjugation forms.
Outputs JSON with forms organized by grammatical categories.
"""
import argparse
import json
import re
import sys
from typing import Any, Dict, List, Optional
from xml.etree import ElementTree as ET

NS = "http://www.mediawiki.org/xml/export-0.11/"


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
    POS_MAP = {
        "substantiivi": "noun", "nimi": "noun", "erisnimi": "proper_noun",
        "verbi": "verb", "adjektiivi": "adjective", "adverbi": "adverb",
        "pronomini": "pronoun", "numeraali": "numeral",
    }
    for m in re.finditer(r"^===\s*([^=\n]+?)\s*===\s*$", section, re.M):
        text = m.group(1).lower().strip()
        for fi, en in POS_MAP.items():
            if fi == text or text.startswith(fi):
                return en
    return None


def get_template_calls(section: str) -> List[Dict[str, Any]]:
    """Extract template calls like {{fi-subs-kala|kiss}}"""
    templates = []
    # Match {{...}} but be careful about nesting
    for m in re.finditer(r'\{\{([a-zA-Z0-9_-]+)(.*?)\}\}', section, re.S):
        name = m.group(1).lower()
        params_str = m.group(2)
        if name.startswith(('fi-subs', 'fi-noun', 'fi-adj', 'fi-verb', 'fi-decl', 'fi-pron')):
            params = []
            # If params start with |, skip it
            if params_str and params_str.startswith('|'):
                params_str = params_str[1:]
            if not params_str.strip():
                templates.append({'name': name, 'params': []})
                continue
            depth = 0
            current = ''
            for c in params_str:
                if c == '{':
                    depth += 1
                    current += c
                elif c == '}':
                    depth -= 1
                    current += c
                elif c == '|' and depth == 0:
                    params.append(current.strip())
                    current = ''
                else:
                    current += c
            if current.strip() != '':
                params.append(current.strip())
            templates.append({'name': name, 'params': params})
    return templates


# Declension patterns based on common Finnish noun types
# We need to implement common patterns from Wiktionary templates

CASES = ['nominative', 'genitive', 'partitive', 'inessive', 'elative', 
        'illative', 'adessive', 'ablative', 'allative', 'essive', 
        'translative', 'abessive', 'instructive', 'comitative']

CASES_FI = ['nominatiivi', 'genetiivi', 'partitiivi', 'inessiivi', 'elatiivi',
            'illatiivi', 'adessiivi', 'ablatiivi', 'allatiivi', 'essiivi',
            'translatiivi', 'abessiivi', 'instruktiivi', 'komitatiivi']


def generate_kala(stem1, stem2=None, stem3=None, stem4=None, vowel='a'):
    """Generate forms for kala type (type 10)"""
    if stem2 is None: stem2 = ''
    if stem3 is None: stem3 = ''
    if stem4 is None: stem4 = ''
    v = vowel if vowel else 'a'
    # vowel harmony
    ov = 'ö' if v == 'ä' else 'o'
    forms = {}
    
    # Singular
    forms['nominative_singular'] = stem1 + stem2 + v
    forms['genitive_singular'] = stem1 + stem3 + stem4 + v + 'n'
    forms['partitive_singular'] = stem1 + stem2 + v + v
    forms['inessive_singular'] = stem1 + stem3 + stem4 + v + 'ss' + v
    forms['elative_singular'] = stem1 + stem3 + stem4 + v + 'st' + v
    forms['illative_singular'] = stem1 + stem2 + v + v + 'n'
    forms['adessive_singular'] = stem1 + stem3 + stem4 + v + 'll' + v
    forms['ablative_singular'] = stem1 + stem3 + stem4 + v + 'lt' + v
    forms['allative_singular'] = stem1 + stem3 + stem4 + v + 'lle'
    forms['essive_singular'] = stem1 + stem2 + v + 'n' + v
    forms['translative_singular'] = stem1 + stem3 + stem4 + v + 'ksi'
    forms['abessive_singular'] = stem1 + stem3 + stem4 + v + 'tt' + v
    forms['instructive_singular'] = '-'
    
    # Plural
    forms['nominative_plural'] = stem1 + stem3 + stem4 + v + 't'
    forms['genitive_plural'] = stem1 + stem2 + ov + 'jen'
    forms['partitive_plural'] = stem1 + stem2 + ov + 'j' + v
    forms['inessive_plural'] = stem1 + stem3 + ov + 'iss' + v
    forms['elative_plural'] = stem1 + stem3 + ov + 'ist' + v
    forms['illative_plural'] = stem1 + stem2 + ov + 'ihin'
    forms['adessive_plural'] = stem1 + stem3 + ov + 'ill' + v
    forms['ablative_plural'] = stem1 + stem3 + ov + 'ilt' + v
    forms['allative_plural'] = stem1 + stem3 + ov + 'ille'
    forms['essive_plural'] = stem1 + stem2 + ov + 'in' + v
    forms['translative_plural'] = stem1 + stem3 + ov + 'iksi'
    forms['abessive_plural'] = stem1 + stem3 + ov + 'itt' + v
    forms['instructive_plural'] = stem1 + stem3 + ov + 'in'
    forms['comitative_plural'] = stem1 + stem2 + ov + 'ine-'
    
    return forms


def generate_valo(stem1, stem2=None, stem3=None, stem4=None, vowel='o'):
    """Generate forms for valo type (type 1)"""
    if stem2 is None or stem2 == '': stem2 = stem1
    if stem3 is None or stem3 == '': stem3 = stem1
    if stem4 is None or stem4 == '': stem4 = stem1
    v = vowel if vowel else 'o'
    ov = 'ö' if v == 'ä' else 'o'
    # For most words like valo, stem1 is the base
    forms = {}
    forms['nominative_singular'] = stem1 + (stem4 if stem4 != stem1 and len(stem4) > 0 else '') + v if stem1 != lemma_check(stem1) else stem1+v  # rough
    # Better: use lemma if stem1 alone doesn't make sense
    forms['nominative_singular'] = stem1 + v if stem2 == stem1 or len(stem2) == 0 else (stem1 + stem4 + v if len(stem4) > 0 and stem4 != stem1 else stem1 + stem2 + v)
    # Simpler approach for standard cases
    base = stem1
    forms['nominative_singular'] = base + v
    forms['genitive_singular'] = base + v + 'n'
    forms['partitive_singular'] = base + v + 'a'
    forms['inessive_singular'] = base + v + 'ssa'
    forms['elative_singular'] = base + v + 'sta'
    forms['illative_singular'] = base + v + 'on'
    forms['adessive_singular'] = base + v + 'lla'
    forms['ablative_singular'] = base + v + 'lta'
    forms['allative_singular'] = base + v + 'lle'
    forms['essive_singular'] = base + v + 'na'
    forms['translative_singular'] = base + v + 'ksi'
    forms['abessive_singular'] = base + v + 'tta'
    forms['instructive_singular'] = '-'
    forms['nominative_plural'] = base + v + 't'
    forms['genitive_plural'] = base + v + 'jen'
    forms['partitive_plural'] = base + v + 'ja'
    forms['illative_plural'] = base + v + 'ihin'
    forms['inessive_plural'] = base + v + 'issa'
    forms['elative_plural'] = base + v + 'ista'
    forms['adessive_plural'] = base + v + 'illa'
    forms['ablative_plural'] = base + v + 'ilta'
    forms['allative_plural'] = base + v + 'ille'
    forms['essive_plural'] = base + v + 'ina'
    forms['translative_plural'] = base + v + 'iksi'
    forms['abessive_plural'] = base + v + 'itta'
    forms['instructive_plural'] = base + v + 'in'
    forms['comitative_plural'] = base + v + 'ine-'
    return forms

def lemma_check(x): return x


def generate_kulkija(stem1, vowel='a'):
    """Generate forms for kulkija type (type 12)"""
    v = vowel if vowel else 'a'
    ov = 'ö' if v == 'ä' else 'o'
    forms = {}
    forms['nominative_singular'] = stem1 + v
    forms['genitive_singular'] = stem1 + v + 'n'
    forms['partitive_singular'] = stem1 + v + v
    forms['inessive_singular'] = stem1 + v + 'ss' + v
    forms['elative_singular'] = stem1 + v + 'st' + v
    forms['illative_singular'] = stem1 + v + v + 'n'
    forms['adessive_singular'] = stem1 + v + 'll' + v
    forms['ablative_singular'] = stem1 + v + 'lt' + v
    forms['allative_singular'] = stem1 + v + 'lle'
    forms['essive_singular'] = stem1 + v + 'n' + v
    forms['translative_singular'] = stem1 + v + 'ksi'
    forms['abessive_singular'] = stem1 + v + 'tt' + v
    forms['instructive_singular'] = '-'
    forms['nominative_plural'] = stem1 + v + 't'
    forms['genitive_plural'] = stem1 + ov + 'iden'
    forms['partitive_plural'] = stem1 + ov + 'it' + v
    forms['inessive_plural'] = stem1 + ov + 'iss' + v
    forms['elative_plural'] = stem1 + ov + 'ist' + v
    forms['illative_plural'] = stem1 + ov + 'ihin'
    forms['adessive_plural'] = stem1 + ov + 'ill' + v
    forms['ablative_plural'] = stem1 + ov + 'ilt' + v
    forms['allative_plural'] = stem1 + ov + 'ille'
    forms['essive_plural'] = stem1 + ov + 'in' + v
    forms['translative_plural'] = stem1 + ov + 'iksi'
    forms['abessive_plural'] = stem1 + ov + 'itt' + v
    forms['instructive_plural'] = stem1 + ov + 'in'
    forms['comitative_plural'] = stem1 + ov + 'ine-'
    return forms


def generate_maa(stem1, stem2='', vowel='a'):
    """Generate forms for maa type (type 18)"""
    v = vowel if vowel else 'a'
    forms = {}
    forms['nominative_singular'] = stem1 + stem2
    forms['genitive_singular'] = stem1 + stem2 + 'n'
    forms['partitive_singular'] = stem1 + stem2 + 't' + v
    forms['inessive_singular'] = stem1 + stem2 + 'ss' + v
    forms['elative_singular'] = stem1 + stem2 + 'st' + v
    forms['illative_singular'] = stem1 + stem2 + 'h' + stem2 + 'n' if stem2 else stem1 + 'han'
    forms['adessive_singular'] = stem1 + stem2 + 'll' + v
    forms['ablative_singular'] = stem1 + stem2 + 'lt' + v
    forms['allative_singular'] = stem1 + stem2 + 'lle'
    forms['essive_singular'] = stem1 + stem2 + 'n' + v
    forms['translative_singular'] = stem1 + stem2 + 'ksi'
    forms['abessive_singular'] = stem1 + stem2 + 'tt' + v
    forms['instructive_singular'] = '-'
    forms['nominative_plural'] = stem1 + stem2 + 't'
    forms['genitive_plural'] = stem1 + 'iden'
    forms['partitive_plural'] = stem1 + 'it' + v
    forms['inessive_plural'] = stem1 + 'iss' + v
    forms['elative_plural'] = stem1 + 'ist' + v
    forms['illative_plural'] = stem1 + 'ihin'
    forms['adessive_plural'] = stem1 + 'ill' + v
    forms['ablative_plural'] = stem1 + 'ilt' + v
    forms['allative_plural'] = stem1 + 'ille'
    forms['essive_plural'] = stem1 + 'in' + v
    forms['translative_plural'] = stem1 + 'iksi'
    forms['abessive_plural'] = stem1 + 'itt' + v
    forms['instructive_plural'] = stem1 + 'in'
    forms['comitative_plural'] = stem1 + 'ine-'
    return forms


def generate_askel(stem1, stem2='', stem3='', vowel='a'):
    """Generate forms for askel type (type 48)"""
    v = vowel if vowel else 'a'
    forms = {}
    # Use strong/weak forms - simplified
    weak = stem1 + stem2  # e.g., ommel
    strong = stem1 + stem3  # e.g., ompel
    forms['nominative_singular'] = weak
    forms['genitive_singular'] = strong + 'een'
    forms['partitive_singular'] = strong + 'etta'
    forms['inessive_singular'] = strong + 'eessa'
    forms['elative_singular'] = strong + 'eesta'
    forms['illative_singular'] = strong + 'eeseen'
    forms['adessive_singular'] = strong + 'eella'
    forms['ablative_singular'] = strong + 'eelta'
    forms['allative_singular'] = strong + 'eelle'
    forms['essive_singular'] = strong + 'eena'
    forms['translative_singular'] = strong + 'eeksi'
    forms['abessive_singular'] = strong + 'eetta'
    forms['instructive_singular'] = '-'
    forms['nominative_plural'] = strong + 'eet'
    forms['genitive_plural'] = strong + 'eiden'
    forms['partitive_plural'] = strong + 'eita'
    forms['inessive_plural'] = strong + 'eissa'
    forms['elative_plural'] = strong + 'eista'
    forms['illative_plural'] = strong + 'eisiin'
    forms['adessive_plural'] = strong + 'eilla'
    forms['ablative_plural'] = strong + 'eilta'
    forms['allative_plural'] = strong + 'eille'
    forms['essive_plural'] = strong + 'eina'
    forms['translative_plural'] = strong + 'eiksi'
    forms['abessive_plural'] = strong + 'eitta'
    forms['instructive_plural'] = strong + 'ein'
    forms['comitative_plural'] = strong + 'eine-'
    return forms


def extract_forms_from_template(template_name: str, params: List[str], lemma: str) -> Dict[str, str]:
    """Try to generate forms from template"""
    forms = {}
    
    if 'kala' in template_name:
        stem1 = params[0] if len(params) > 0 and params[0] else lemma
        stem2 = params[1] if len(params) > 1 and params[1] != '' else None
        stem3 = params[2] if len(params) > 2 and params[2] != '' else None
        stem4 = params[3] if len(params) > 3 and params[3] != '' else None
        vowel = params[4] if len(params) > 4 and params[4] in ['a', 'ä', 'o', 'ö', 'u', 'y'] else None
        if vowel is None:
            vowel = 'ä' if any(c in lemma for c in ['ä', 'y']) and 'a' not in lemma else 'a'
        forms = generate_kala(stem1, stem2, stem3, stem4, vowel)
    elif 'kulkija' in template_name:
        stem1 = params[0] if len(params) > 0 and params[0] else lemma
        vowel = params[1] if len(params) > 1 and params[1] in ['a', 'ä'] else None
        if vowel is None:
            vowel = 'ä' if 'ä' in lemma else 'a'
        forms = generate_kulkija(stem1, vowel)
    elif 'maa' in template_name:
        stem1 = params[0] if len(params) > 0 and params[0] else lemma
        stem2 = params[1] if len(params) > 1 and params[1] != '' else ''
        vowel = params[2] if len(params) > 2 and params[2] in ['a', 'ä'] else None
        if vowel is None:
            vowel = 'ä' if any(c in lemma for c in ['ä', 'y']) else 'a'
        forms = generate_maa(stem1, stem2, vowel)
    elif 'askel' in template_name:
        stem1 = params[0] if len(params) > 0 and params[0] else lemma
        stem2 = params[1] if len(params) > 1 and params[1] != '' else ''
        stem3 = params[2] if len(params) > 2 and params[2] != '' else stem1 + stem2
        vowel = params[3] if len(params) > 3 and params[3] in ['a', 'ä'] else None
        if vowel is None:
            vowel = 'ä' if any(c in lemma for c in ['ä', 'y']) else 'a'
        forms = generate_askel(stem1, stem2, stem3, vowel)
    elif 'valo' in template_name:
        stem1 = params[0] if len(params) > 0 and params[0] != '' else lemma
        stem2 = params[1] if len(params) > 1 and params[1] != '' else None
        stem3 = params[2] if len(params) > 2 and params[2] != '' else None
        stem4 = params[3] if len(params) > 3 and params[3] != '' else None
        vowel = params[4] if len(params) > 4 and params[4] in ['a', 'ä', 'o', 'ö', 'u', 'y'] else None
        if vowel is None:
            vowel = 'ä' if any(c in lemma for c in ['ä', 'y']) and 'a' not in lemma and 'o' not in lemma else 'o'
        # For juoksu style: stem1='juo', stem4='u' gives juo + u = juou? Or different
        # Check the actual pattern
        if stem2 == stem3 and stem2 and stem2 not in stem1:
            forms = generate_valo(stem1 + stem2, vowel=vowel)
        elif stem4 and stem4 != stem1 and len(stem4) == 1 and (stem1 + stem4) == lemma[:len(stem1)+len(stem4)]:
            forms = generate_valo(stem1 + stem4, vowel=vowel)
        else:
            forms = generate_valo(stem1, vowel=vowel)
    elif template_name == 'fi-subs' or template_name == 'fi-noun':
        stem = lemma
        forms['nominative_singular'] = lemma
    elif 'risti' in template_name:
        stem1 = params[0] if len(params) > 0 else lemma
        stem2 = params[1] if len(params) > 1 else ''
        vowel = params[3] if len(params) > 3 and params[3] in ['a', 'ä'] else 'ä'
        forms = generate_kala(stem1, stem2, stem1, stem1, vowel)
    
    return forms


def main(argv):
    p = argparse.ArgumentParser(description="Extract Finnish forms from Wiktionary dump")
    p.add_argument("--input", required=True, help="Wiktionary XML dump")
    p.add_argument("--output", required=True, help="Output JSONL file")
    p.add_argument("--pos", help="Comma-separated POS")
    p.add_argument("--limit", type=int)
    p.add_argument("--lemma", help="Extract forms for a specific lemma only")
    args = p.parse_args(argv[1:])
    
    pos_filter = set(args.pos.split(",")) if args.pos else None
    count = 0
    processed = 0
    
    out = open(args.output, 'w', encoding='utf-8')
    
    for event, elem in ET.iterparse(args.input, events=("end",)):
        if elem.tag == qname('page'):
            ns_elem = elem.find(qname('ns'))
            ns = ns_elem.text if ns_elem is not None else ""
            if ns != '0':
                elem.clear()
                continue
            
            title_elem = elem.find(qname('title'))
            title = title_elem.text if title_elem is not None else ""
            
            if args.lemma and title != args.lemma:
                elem.clear()
                continue
            
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
            
            templates = get_template_calls(fi_section)
            if not templates:
                elem.clear()
                continue
            
            all_forms = {}
            for tmpl in templates:
                forms = extract_forms_from_template(tmpl['name'], tmpl['params'], title)
                all_forms.update(forms)
            
            if all_forms:
                result = {
                    'lemma': title,
                    'pos': pos,
                    'templates': templates,
                    'forms': all_forms,
                    'forms_fi': {k.replace('_', ' '): v for k, v in all_forms.items()}
                }
                out.write(json.dumps(result, ensure_ascii=False) + '\n')
                out.flush()
            
            processed += 1
            if processed % 1000 == 0:
                print(f"Processed {processed}...", file=sys.stderr)
            if args.limit and processed >= args.limit:
                break
            elem.clear()
    
    out.close()
    print(f"Done. {processed} entries", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
