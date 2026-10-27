#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate torille's noun forms against Omorfi + Voikko.

For each noun given on the command line (or the catalogue model words), generate
the full paradigm (every case x singular/plural) with Omorfi, confirm each form
with libvoikko, then check that torille's `inflect` emits that form as a `plain`
suggestion. A form that Omorfi generates and Voikko accepts but torille does not
produce is a missing or wrong noun form in the engine.

Omorfi is the oracle for *coverage* (what forms exist); Voikko is the
authoritative *correctness* check.

Usage:
    python3 tools/check_nouns_with_omorfi.py [LEMMA ...]
"""

import subprocess
import sys

from pyomorfi import load_omorfi
from voikko import libvoikko

OMORFI = load_omorfi(load_segmenter=False)
VOIKKO = libvoikko.Voikko("fi")

CASES = [
    "NOM", "GEN", "PAR", "ESS", "TRA", "INE", "ELA", "ILL",
    "ADE", "ABL", "ALL", "ABE", "INS",
]
NUMBERS = ["SG", "PL"]
POS_TAGS = ["NOUN", "ADJ", "PROPN"]

DEFAULT_NOUNS = [
    "kala", "talo", "valo", "käsi", "joki", "lapsi", "mies", "nainen",
    "tiili", "pieni", "ohut", "sisar", "askel", "hame", "korkea", "vapaa",
    "maa", "suo", "filee", "vieras", "solakka", "kuollut", "vanhempi",
    "parfait", "onneton", "vastaus", "kalleus", "kahdeksas", "tuhat",
    "kevät", "omena", "sisin", "vasen", "kynsi", "hammas", "lämmin",
]

# The known words and noun stems, read out of the sanalista, for detecting
# compounds.
_KNOWN_WORDS: set[str] = set()
_KNOWN_NOUNS: set[str] = set()
for _line in open("data/nykysuomensanalista.csv"):
    _p = _line.rstrip("\n").split("\t")
    if len(_p) >= 4:
        _KNOWN_WORDS.add(_p[0])
        if "substantiivi" in _p[2]:
            _KNOWN_NOUNS.add(_p[0])


def is_compound(word: str) -> bool:
    """Whether a word is a compound, inflected on a noun head.

    A compound (aavameri, harjakuusi) is a known modifier plus a noun head, so
    some split of the word has a known word in front and a listed noun after it.
    A word that merely ends like another (aakkosto's osto, liemi's mi) has no
    such split, so it is left in.
    """
    for i in range(1, len(word)):
        if word[:i] in _KNOWN_WORDS and word[i:] in _KNOWN_NOUNS:
            return True
    return False


def omorfi_generate(lemma: str, pos: str, num: str, case: str) -> str:
    return OMORFI.generate(f"[WORD_ID={lemma}][UPOS={pos}][NUM={num}][CASE={case}]")


def omorfi_forms(lemma: str) -> list[str]:
    """All (case, number) forms Omorfi generates for the lemma, trying POS tags."""
    for pos in POS_TAGS:
        forms = []
        for num in NUMBERS:
            for case in CASES:
                gen = omorfi_generate(lemma, pos, num, case)
                if isinstance(gen, str) and gen:
                    forms.extend(gen.split("/"))
        if forms:
            return forms
    return []


def voikko_ok(lemma: str, form: str) -> bool:
    analysis = VOIKKO.analyze(form)
    return any(
        a.get("BASEFORM") == lemma
        or lemma in a.get("WORDBASES", "")
        for a in analysis
    )


def torille_plain(lemma: str) -> tuple[set[str], set[str]]:
    out = subprocess.run(
        ["cargo", "run", "-q", "-p", "torille-tools", "--bin", "inflect", "--", lemma],
        capture_output=True, text=True, check=True,
    ).stdout
    plain = set()
    all_forms = set()
    in_plain = False
    for line in out.splitlines():
        if line.startswith("## plain"):
            in_plain = True
            continue
        if line.startswith("## repaired") or line.startswith("## guessed"):
            in_plain = False
            continue
        if line.strip():
            w = line.split()[0]
            all_forms.add(w)
            if in_plain:
                plain.add(w)
    return plain, all_forms


def main() -> None:
    nouns = sys.argv[1:] or DEFAULT_NOUNS
    total = missing = not_offered = 0
    for noun in nouns:
        if is_compound(noun):
            print(f"{noun}: compound, skipped")
            continue
        expected = omorfi_forms(noun)
        if not expected:
            print(f"{noun}: no Omorfi entry (skipped)")
            continue
        plain, all_forms = torille_plain(noun)
        noun_missing = noun_guessed = 0
        for form in sorted(set(expected)):
            if not voikko_ok(noun, form):
                continue  # not a Voikko-confirmed form, not authoritative
            total += 1
            if form not in all_forms:
                noun_missing += 1
                missing += 1
                print(f"  MISSING {noun}: {form}")
            elif form not in plain:
                noun_guessed += 1
                not_offered += 1
                print(f"  not-offered {noun}: {form}")
        print(f"{noun}: {len(expected)} Omorfi forms, {noun_missing} missing, "
              f"{noun_guessed} not offered")
    print(f"\n{total} Voikko-confirmed forms checked, {missing} missing from torille, "
          f"{not_offered} generated but not offered")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())