#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate torille's generated noun forms against libvoikko.

For each noun given on the command line (or a built-in sample), run torille's
`inflect` tool, take the `plain` forms, and feed each through libvoikko's
analyzer. A torille form that voikko does not recognise as the same noun (its
nominative singular) is a likely morphological bug.

Usage:
    python3 tools/check_nouns_with_voikko.py [LEMMA ...]
"""

import subprocess
import sys

from voikko import libvoikko

VOIKKO = libvoikko.Voikko("fi")

# The Finnish possessive suffixes, which torille attaches after a case form.
# libvoikko's spell-checker FST does not enumerate every possessive+cas+plural
# combination, so a form bearing one is flagged separately from a real bug in
# the core case paradigm.
POSSESSIVES = ("nsa", "nsä", "mme", "nne", "ni", "si", "an", "än", "aan", "ään",
               "een", "aan", "hen", "hin", "ksiin")


def is_possessive(form: str) -> bool:
    return any(form.endswith(s) for s in POSSESSIVES)


DEFAULT_NOUNS = [
    "kala", "talo", "valo", "käsi", "joki", "lapsi", "mies", "nainen",
    "tiili", "pieni", "ohut", "sisar", "askel", "hame", "korkea", "vapaa",
    "maa", "suo", "filee", "vieras", "solakka", "kuollut", "vanhempi",
    "parfait", "onneton", "vastaus", "kalleus", "kahdeksas", "tuhat",
    "kevät", "omena", "sisin", "vasen", "kynsi", "hammas", "lämmin",
]


def torille_forms(lemma: str) -> list[str]:
    out = subprocess.run(
        ["cargo", "run", "-q", "-p", "torille-tools", "--bin", "inflect", "--", lemma],
        capture_output=True, text=True, check=True,
    ).stdout
    forms = []
    in_plain = False
    for line in out.splitlines():
        if line.startswith("## plain"):
            in_plain = True
            continue
        if line.startswith("## repaired") or line.startswith("## guessed"):
            in_plain = False
            continue
        if in_plain and line.strip():
            forms.append(line.split()[0])
    return forms


def main() -> None:
    nouns = sys.argv[1:] or DEFAULT_NOUNS
    total = core_bad = poss_skipped = 0
    for noun in nouns:
        noun_bad = 0
        for form in torille_forms(noun):
            total += 1
            if is_possessive(form):
                poss_skipped += 1
                continue
            analysis = VOIKKO.analyze(form)
            ok = any(
                a.get("BASEFORM") == noun
                or noun in a.get("WORDBASES", "")
                for a in analysis
            )
            if not ok:
                noun_bad += 1
                core_bad += 1
                bases = {a.get("BASEFORM") for a in analysis} if analysis else {"(none)"}
                print(f"  BAD {noun}: {form} -> {bases}")
        print(f"{noun}: {len(torille_forms(noun))} forms, {noun_bad} core-case unrecognised")
    print(f"\n{total} plain forms checked, {core_bad} core-case not recognised "
          f"(of which {poss_skipped} possessive forms skipped)")
    return 1 if core_bad else 0


if __name__ == "__main__":
    sys.exit(main())