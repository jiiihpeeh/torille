#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build `data/adj_degrees.csv`: each adjective's comparative and superlative
nominatives, derived by Omorfi.

Finnish comparatives (`-mpi`) and superlatives (`-in`) are derivational,
not inflectional: the sanalista lists only 44 as lemmas, and the engine has
no rule for them, so `suurempi`, `kauniimpi`, `kaunein` are unknown to the
checker. Omorfi does derive them (`DRV=MPI`/`IN²`, `CMP=CMP`/`SUP`) from the
positive, so this asks it for the pair, once, and writes a table the engine
embeds.

Output (tab-separated):

    positive  comparative  superlative
    suuri     suurempi     suurin
    kaunis    kauniimpi    kaunein
    ...

A cell that Omorfi does not derive is left empty; a row with both empty is
dropped.

Usage:
    python3 tools/build_adj_degrees.py
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "data" / "adj_degrees.csv"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--limit", type=int, default=None)
    args = ap.parse_args()

    try:
        from torille import Engine
        from pyomorfi import load_omorfi
        from voikko import libvoikko
    except ImportError as e:
        print(f"missing dependency: {e}", file=sys.stderr)
        return 2

    engine = Engine()
    omorfi = load_omorfi(load_segmenter=False)
    voikko = libvoikko.Voikko("fi")

    lemmas = [w for w, _ in engine.words("adjektiivi")]
    # `usea` is filed as a pronomini, but Omorfi derives its degrees exactly
    # like an adjective's (`useampi`, `usein`/`useimmat`). Sampling only the
    # adjective tag left the superlative `useimmat` unknown even though the
    # degree machinery handles it (`inflect usein 36` emits it). A pronoun
    # Omorfi derives nothing for drops out below (both cells empty).
    seen = set(lemmas)
    for word, _ in engine.words("pronomini"):
        if word not in seen:
            seen.add(word)
            lemmas.append(word)
    if args.limit:
        lemmas = lemmas[: args.limit]

    rows: list[tuple[str, str, str]] = []
    for word in lemmas:
        comp = generate_degree(omorfi, voikko, word, "MPI", "CMP", "comparative")
        sup = generate_degree(omorfi, voikko, word, "IN²", "SUP", "superlative")
        if comp or sup:
            rows.append((word, comp, sup))

    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write("positive\tcomparative\tsuperlative\n")
        for positive, comp, sup in rows:
            fh.write(f"{positive}\t{comp}\t{sup}\n")

    both = sum(1 for _, c, s in rows if c and s)
    print(
        f"{len(rows)} adjectives with a degree form "
        f"({both} with both), of {len(lemmas)}",
        file=sys.stderr,
    )
    return 0


def generate_degree(omorfi, voikko, word: str, drv: str, cmp_: str,
                    comparison: str) -> str:
    """The degree nominative for `word`, or "".

    Omorfi's `generate` can return several spellings, and not all are the
    degree (`paha` gives `pahoin/pahin`, where `pahoin` is the adverbial).
    Voikko decides: a candidate counts only when it analyses with
    `BASEFORM == word`, the wanted `COMPARISON`, singular number and the
    nominative (`SIJAMUOTO == "nimento"`). This also drops Omorfi's
    over-generations (`hyvä` -> `hyvempi`, which Voikko does not know).
    """
    tag = f"[WORD_ID={word}][UPOS=ADJ][DRV={drv}][CMP={cmp_}][NUM=SG][CASE=NOM]"
    try:
        got = omorfi.generate(tag)
    except Exception:
        return ""
    if not isinstance(got, str) or not got:
        return ""
    for candidate in got.split("/"):
        candidate = candidate.strip()
        if not candidate or " " in candidate or "\t" in candidate:
            continue
        if len(candidate) <= len(word):
            continue
        try:
            analyses = voikko.analyze(candidate)
        except Exception:
            continue
        for a in analyses:
            if (a.get("BASEFORM") == word
                    and a.get("COMPARISON") == comparison
                    and a.get("NUMBER") == "singular"
                    and a.get("SIJAMUOTO") == "nimento"):
                return candidate
    return ""


if __name__ == "__main__":
    sys.exit(main())
