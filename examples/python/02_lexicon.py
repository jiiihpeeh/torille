#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""The lexicon: homonyms, parts of speech, and classless compounds.

`lookup` answers with the sanalista's own rows. A word with several rows is a
homonym: `kuusi` is the spruce (class 24) and the numeral six (class 27), and
a caller must keep both readings until context decides.
"""

from torille import Engine, Pos


def main() -> None:
    engine = Engine()

    print("Homonyms in the sanalista:")
    for word in ["kuusi", "sata", "tuuli", "mieli", "kissa"]:
        print(f"  {word:<8} {engine.lookup(word)}")

    nouns = engine.words(Pos.NOUN)
    print(f"\nClassed noun lemmas: {len(nouns)}")
    for word, luokka in nouns[:5]:
        print(f"  {word:<16} {luokka}")

    all_nouns = engine.words(Pos.NOUN, include_classless=True)
    classless = [row for row in all_nouns if row[1].startswith("(none")]
    print(f"\nIncluding classless compounds: {len(all_nouns)}")
    print(f"  with a classed head: {sum(1 for row in classless if row[1] != '(none)')}")
    print(f"  with no classed head: {sum(1 for row in classless if row[1] == '(none)')}")
    for word, luokka in classless[:8]:
        print(f"  {word:<20} {luokka}")

    print("\nOther parts of speech (the enum, and one name for comparison):")
    for pos in [Pos.VERB, Pos.ADJECTIVE, Pos.ADVERB, Pos.NUMERAL, Pos.PRONOUN]:
        print(f"  {pos.name:<12} {len(engine.words(pos))} lemmas")
    print(f"  {'substantiivi':<12} {len(engine.words('substantiivi'))} lemmas")


if __name__ == "__main__":
    main()
