#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build `data/adverbit.csv`: adverbial forms Voikko confirms but the engine
does not make.

Adverbs mostly do not inflect, and their degree forms are the adjective's
(the engine derives those). The exception is a possessive suffix on a few
`-ksi` adverbs: `kylliksi` -> `kylliksenne`, `tarpeeksi` -> `tarpeekseni`.
Omorfi cannot `generate` these (`[UPOS=ADV][POSS=..]` returns nothing, and
its analysis of the form is `GUESS=UNKNOWN`), so they cannot be
materialised the way the noun/verb oracles were. Voikko *does* analyse
them, so this generates candidates by rule (the adverb's stem plus each
possessive suffix) and keeps the ones Voikko confirms as that adverb.

Output (tab-separated):

    lemma  form
    kylliksi  kylliksenne
    ...

Usage:
    python3 tools/build_adverbi_forms.py
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SANALISTA = REPO / "data" / "nykysuomensanalista.txt"
OUT = REPO / "data" / "adverbit.csv"

#: The possessive suffixes. The third person has both spellings.
POSSESSIVES = ("ni", "si", "nsa", "nsä", "mme", "nne")


def adverb_lemmas() -> list[str]:
    lemmas: list[str] = []
    for line in SANALISTA.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        word, pos = parts[0].strip(), parts[2]
        if word and "adverbi" in pos:
            lemmas.append(word)
    return lemmas


def stem_variants(lemma: str) -> list[str]:
    """The stems a possessive can attach to.

    A `-ksi` adverb takes the possessive on a `-kse` stem (`kylliksi` ->
    `kyllikse` + `nne`); the bare lemma is tried too, since some adverbs
    do not change.
    """
    variants = [lemma]
    if lemma.endswith("i") and len(lemma) > 1:
        variants.append(lemma[:-1] + "e")
    return variants


def adjective_lemmas() -> list[str]:
    lemmas: list[str] = []
    for line in SANALISTA.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        word, pos = parts[0].strip(), parts[2]
        if word and "adjektiivi" in pos:
            lemmas.append(word)
    return lemmas


def sti_forms(voikko, engine) -> list[tuple[str, str]]:
    """The productive `-sti` manner adverb of each adjective.

    The regular rule is the adjective's **genitive stem + `sti`**:
    `nopea` -> `nopean` -> `nopeasti`, `kaunis` -> `kauniin` ->
    `kauniisti`, `iloinen` -> `iloisen` -> `iloisesti`. The stem comes from
    the engine (which knows the genitive); Voikko confirms the result is
    the adjective's adverb, so a blind suffix is never written.
    """
    out: list[tuple[str, str]] = []
    for adjective in adjective_lemmas():
        try:
            forms = engine.inflect(adjective)
        except ValueError:
            continue
        genitive = next(
            (f.word for f in forms
             if f.case == "genitive" and f.number == "singular"
             and not f.possessive),
            None,
        )
        if not genitive or not genitive.endswith("n"):
            continue
        candidate = genitive[:-1] + "sti"
        try:
            analyses = voikko.analyze(candidate)
        except Exception:
            continue
        for a in analyses:
            if a.get("BASEFORM") == adjective and a.get("CLASS", "").endswith("laatusana"):
                out.append((adjective, candidate))
                break
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(OUT))
    args = ap.parse_args()

    try:
        from voikko import libvoikko
        from torille import Engine
    except ImportError as e:
        print(f"missing dependency: {e}", file=sys.stderr)
        return 2

    voikko = libvoikko.Voikko("fi")
    engine = Engine()
    rows: list[tuple[str, str]] = []
    for lemma in adverb_lemmas():
        seen: set[str] = set()
        for stem in stem_variants(lemma):
            for possessive in POSSESSIVES:
                candidate = f"{stem}{possessive}"
                if candidate in seen or candidate == lemma:
                    continue
                seen.add(candidate)
                try:
                    analyses = voikko.analyze(candidate)
                except Exception:
                    continue
                for a in analyses:
                    if a.get("BASEFORM") == lemma and a.get("POSSESSIVE"):
                        rows.append((lemma, candidate))
                        break
    rows.extend(sti_forms(voikko, engine))

    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write("lemma\tform\n")
        for lemma, form in rows:
            fh.write(f"{lemma}\t{form}\n")
    print(f"{len(rows)} adverb forms", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
