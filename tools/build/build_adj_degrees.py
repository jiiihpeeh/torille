#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build `data/adj_degrees.csv`: each adjective's comparative and superlative
nominatives, derived by the engine.

Finnish comparatives (`-mpi`) and superlatives (`-in`) are derivational,
not inflectional: the sanalista lists only a few as lemmas, so the engine
derives them from the positive. `src/expand/derived_forms` implements the
rule, with a productive fallback for the `-inen`/`-lainen` families. This
asks the current engine (the `torille` binding) for the degrees, once, and
writes a table the engine embeds so the forms are available on the fast
path.

Output (tab-separated):

    positive  comparative  superlative
    suuri     suurempi     suurin
    kaunis    kauniimpi    kaunein
    ...

A cell the engine does not derive is left empty; a row with both empty is
dropped.

Usage:
    python3 tools/build/build_adj_degrees.py
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "data" / "adj_degrees.csv"


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--limit", type=int, default=None)
    args = ap.parse_args()

    try:
        from torille import Engine
    except ImportError as e:
        print(f"missing dependency: {e}", file=sys.stderr)
        return 2

    engine = Engine()
    lemmas = [w for w, _ in engine.words("adjektiivi")]
    # `usea` is filed as a pronomini, but the engine derives its degrees
    # exactly like an adjective's (`useampi`, `usein`/`useimmat`). Sampling
    # only the adjective tag left the superlative `useimmat` unknown even
    # though the degree machinery handles it. A pronoun with no degree forms
    # drops out below (both cells empty).
    seen = set(lemmas)
    for word, _ in engine.words("pronomini"):
        if word not in seen:
            seen.add(word)
            lemmas.append(word)
    if args.limit:
        lemmas = lemmas[: args.limit]

    rows: list[tuple[str, str, str]] = []
    for word in lemmas:
        try:
            forms = engine.inflect(word)
        except Exception:
            continue
        nominatives = list(
            dict.fromkeys(
                f.word
                for f in forms
                if f.case == "nominative" and f.number == "singular"
            )
        )
        comparative = next(
            (w for w in nominatives if w != word and w.endswith("mpi")), ""
        )
        superlative = next(
            (
                w
                for w in nominatives
                if w not in (word, comparative) and w.endswith("in")
            ),
            "",
        )
        if comparative or superlative:
            rows.append((word, comparative, superlative))

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


if __name__ == "__main__":
    sys.exit(main())
