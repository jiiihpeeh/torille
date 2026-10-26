#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Generate `data/catalogue.csv`: the paradigm templates, sourced from the
Omorfi + Voikko oracles rather than the Kotus taivutustyypit document.

`catalogue.csv` is the replacement for `data/kotus-tyypit.txt`. The engine
needs, per inflection class, one representative word's forms in the eight
canonical catalogue slots (the templates the heuristic aligns a lemma onto).
Those templates are read out of the *oracle* -- the same Omorfi/Voikko ground
truth `rules/` and the stem maps are built from -- so the tyypit document is
never needed at build time or runtime.

## Inputs

    data/kotus_nomines.sqlite   nouns:  (lemma, class) + full case/number grid
    data/kotus_verbit.sqlite    verbs:  (lemma, class) + full reading grid
    rules/*.json                per-class `model_word`, the representative the
                                class-rule extraction already picked

## Output

    data/catalogue.csv   tab-separated: class, slot, form, optional

`slot` is the canonical position (0-7). The first row for a slot is the model
form; later rows are alternants. `optional` is 1 for a form the oracle only
half-confirms (`ok='omorfi'`, Omorfi says yes but Voikko does not read it) --
the analogue of the parentheses in the Kotus document, and something the
engine demotes.

No Kotus taivutus document is read. `data/nykysuomensanalista.csv` is not read
either; a class with no rule falls back to the oracle's own members.

Usage:
    python3 tools/build_catalogue_csv.py
"""

from __future__ import annotations

import glob
import json
import re
import sqlite3
import sys
from pathlib import Path
from typing import Iterator

REPO = Path(__file__).resolve().parent.parent
NOUN_DB = REPO / "data" / "kotus_nomines.sqlite"
VERB_DB = REPO / "data" / "kotus_verbit.sqlite"
RULES = REPO / "rules"
OUT = REPO / "data" / "catalogue.csv"

#: The base class numbers the catalogue carries. 1-51 are nominal, 52-75
#: verbal. (76-78 have no paradigm block in the source document and are
#: handled by hand-written verb engines, so they are not templates.)
NOUN_MAX = 51
VERB_MAX = 75

#: The eight canonical noun slots, by position.
NOUN_SLOTS = [
    ("nominative", "singular"),
    ("genitive", "singular"),
    ("partitive", "singular"),
    ("illative", "singular"),
    ("nominative", "plural"),
    ("genitive", "plural"),
    ("partitive", "plural"),
    ("illative", "plural"),
]

#: The eight canonical verb slots, by position, as `forms`-column equality
#: predicates. See `verb_slot_tags` in `src/expand/inflect.rs` for the reading
#: the engine assigns each position.
VERB_SLOTS = [
    {"infinitive": "1st", "voice": "active"},
    {"voice": "active", "mood": "indicative", "tense": "present",
     "person": "first", "number": "singular"},
    {"voice": "active", "mood": "indicative", "tense": "past",
     "person": "third", "number": "singular"},
    {"voice": "active", "mood": "conditional",
     "person": "third", "number": "singular"},
    {"voice": "active", "mood": "potential",
     "person": "third", "number": "singular"},
    {"voice": "active", "mood": "imperative",
     "person": "third", "number": "singular"},
    {"voice": "active", "participle": "past active",
     "case": "nominative", "number": "singular"},
    {"voice": "passive", "mood": "indicative", "tense": "past"},
]


def load_model_words() -> dict[str, str]:
    """`class string -> model_word`, from the per-class rule files."""
    out: dict[str, str] = {}
    for path in glob.glob(str(RULES / "*.json")):
        with open(path, encoding="utf-8") as fh:
            data = json.load(fh)
        cls, word = data.get("class"), data.get("model_word")
        if cls and word:
            out[str(cls)] = word
    return out


def model_word_for(n: int, rules: dict[str, str]) -> str | None:
    """The representative for base class `n`.

    Prefer the plain `class-N.json` rule; otherwise the first rule whose
    numeric part is `n`, preferring the simplest class string (no gradation
    suffix, not parenthesised) so the template is the ungradated shape, the
    same one the current catalogue uses.
    """
    if str(n) in rules:
        return rules[str(n)]
    candidates = []
    for cls, word in rules.items():
        m = re.match(r"^\(?(\d+)", cls)
        if m and int(m.group(1)) == n:
            score = (cls.startswith("("), "*" in cls, "," in cls, len(cls))
            candidates.append((score, word))
    return min(candidates)[1] if candidates else None


def class_members(cur: sqlite3.Cursor, cls: int) -> Iterator[str]:
    """Class members, plain class first, then its gradation/comma variants."""
    seen: set[str] = set()
    for pattern in (str(cls), f"{cls}*%", f"{cls},%"):
        op = "=" if pattern == str(cls) else "LIKE"
        for (lemma,) in cur.execute(
            f"SELECT lemma FROM base WHERE class {op} ? ORDER BY lemma", (pattern,)
        ):
            if lemma not in seen:
                seen.add(lemma)
                yield lemma


def slot_rows(cur: sqlite3.Cursor, lemma: str, noun: bool) -> list[list[tuple[str, str]]]:
    """The `(form, ok)` rows for each of the eight slots of `lemma`."""
    out: list[list[tuple[str, str]]] = []
    if noun:
        for case, number in NOUN_SLOTS:
            out.append(list(cur.execute(
                'SELECT form, ok FROM forms '
                "WHERE lemma=? AND homonym='' AND \"case\"=? AND number=? "
                "AND possessive=''",
                (lemma, case, number),
            )))
    else:
        for pred in VERB_SLOTS:
            where = " AND ".join(f'"{k}"=?' for k in pred)
            out.append(list(cur.execute(
                "SELECT form, ok FROM forms "
                f"WHERE lemma=? AND homonym='' AND possessive='' "
                f"AND polarity='affirmative' AND form NOT LIKE '% %' "
                f"AND {where}",
                (lemma, *pred.values()),
            )))
    return out


def ordered(rows: list[tuple[str, str]]) -> list[tuple[str, bool]]:
    """Dedupe a slot's forms and order them: fully-confirmed (`both`) first,
    then the Omorfi-only ones, each alphabetically. Returns `(form, optional)`
    where `optional` marks the half-confirmed readings."""
    seen: dict[str, bool] = {}
    for form, ok in rows:
        optional = ok != "both"
        seen[form] = seen.get(form, True) and optional
    result = sorted(seen.items(), key=lambda kv: (kv[1], kv[0]))
    # The first form of a slot is the model and is never "optional"; the flag
    # only distinguishes the alternants that follow it.
    if result:
        result[0] = (result[0][0], False)
    return result


def select_template(
    cur: sqlite3.Cursor, cls: int, rules: dict[str, str], noun: bool
) -> tuple[str | None, list[list[tuple[str, str]]] | None]:
    """Choose `(model_word, slots)` for a class.

    A nominal class uses its rule's `model_word` when that word has a complete
    paradigm. A verbal class has no rule, and a nominal class with no usable
    rule falls back to the class's own members, preferring one whose slots are
    all confirmed by both oracles (so no template form is forced optional).
    """
    preferred = model_word_for(cls, rules) if noun else None
    if preferred:
        slots = slot_rows(cur, preferred, noun)
        if all(slots):
            return preferred, slots

    best: tuple[str, list[list[tuple[str, str]]], int] | None = None
    for word in class_members(cur, cls):
        slots = slot_rows(cur, word, noun)
        if not all(slots):
            continue
        score = sum(1 for slot in slots for _, ok in slot if ok != "both")
        if best is None or score < best[2]:
            best = (word, slots, score)
        if score == 0:
            break
    if best:
        return best[0], best[1]
    return None, None


def main() -> int:
    for db in (NOUN_DB, VERB_DB):
        if not db.exists():
            print(f"missing oracle: {db}", file=sys.stderr)
            return 2
    rules = load_model_words()
    noun = sqlite3.connect(f"file:{NOUN_DB}?mode=ro", uri=True)
    verb = sqlite3.connect(f"file:{VERB_DB}?mode=ro", uri=True)
    ncur, vcur = noun.cursor(), verb.cursor()

    rows: list[tuple[int, int, str, int]] = []
    missing: list[int] = []
    for cls in range(1, VERB_MAX + 1):
        noun_class = cls <= NOUN_MAX
        cur = ncur if noun_class else vcur
        word, slots = select_template(cur, cls, rules, noun_class)
        if word is None:
            missing.append(cls)
            print(f"  class {cls:2d}: -- no complete template --", file=sys.stderr)
            continue
        for slot, forms in enumerate(slots):
            for form, optional in ordered(forms):
                rows.append((cls, slot, form, 1 if optional else 0))
        print(f"  class {cls:2d}: {word}", file=sys.stderr)

    rows.sort(key=lambda r: (r[0], r[1]))
    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write("class\tslot\tform\toptional\n")
        for cls, slot, form, optional in rows:
            fh.write(f"{cls}\t{slot}\t{form}\t{optional}\n")
    print(
        f"\nwrote {OUT.relative_to(REPO)}: {len(rows)} rows, "
        f"{len({r[0] for r in rows})} classes",
        file=sys.stderr,
    )
    if missing:
        print(f"classes with no complete template: {missing}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
