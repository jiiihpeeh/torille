#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Morphology: readings, agreement, depth, clitics, possessives, derivation.

A token keeps every reading the resolver found and the one value they agree
on. `tuli` is the noun and the past tense of `tulla`; the readings list is the
authority, and `case`/`number`/`pos` are the agreed summary (or `None` when
the readings disagree).
"""

from torille import Engine

TEXT = (
    "Siellähänkin asuu ystävä, jonka talossani kävi eilen. "
    "Laitilalainen kauppias toi Tuli-nimisen koiran."
)


def main() -> None:
    engine = Engine()

    print(f"text: {TEXT}\n")
    print(f"{'surface':<18} {'lemma':<14} {'pos':<11} {'case':<12} {'number':<9} depth")
    for row in engine.morphology(TEXT):
        print(
            f"{row.surface:<18} {str(row.lemma):<14} {str(row.pos):<11} "
            f"{str(row.case):<12} {str(row.number):<9} {row.depth}"
        )

    print("\nEvery reading of an ambiguous surface:")
    for row in engine.morphology("Tuli paloi."):
        if row.surface == "Tuli":
            print(f"  {row.surface}: {row.tags}")
            for number, reading in enumerate(row.readings, 1):
                print(f"    reading {number}: {reading}")

    print("\nClitic and possessive surfaces:")
    for word in ["siellähänkin", "talossani", "koiransa"]:
        row = engine.morphology(word)[0]
        print(
            f"  {word:<14} lemma={str(row.lemma):<10} clitic={row.clitic} "
            f"possessive={row.possessive} depth={row.depth}"
        )

    print("\nA productive derivation for an unknown surface:")
    row = engine.morphology("laitilalainen")[0]
    print(f"  {row.surface}: lemma={row.lemma} derivation={row.derivation}")

    print("\nResolution depths seen in the text:")
    depths: dict[str, int] = {}
    for row in engine.morphology(TEXT):
        depths[row.depth] = depths.get(row.depth, 0) + 1
    for depth, count in sorted(depths.items()):
        print(f"  {depth:<12} {count}")


if __name__ == "__main__":
    main()
