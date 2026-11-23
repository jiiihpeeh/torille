#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""The lexicon: homonyms, parts of speech, and classless compounds.

`lookup` answers with the sanalista's own rows. A word with several rows is a
homonym: `kuusi` is the spruce (class 24) and the numeral six (class 27), and
a caller must keep both readings until context decides.
"""

from torille import Engine


def main() -> None:
    engine = Engine()

    print("Homonyms in the sanalista:")
    for word in ["kuusi", "sata", "tuuli", "mieli", "kissa"]:
        print(f"  {word:<8} {engine.lookup(word)}")

    nouns = engine.words("substantiivi")
    print(f"\nClassed noun lemmas: {len(nouns)}")
    for word, klass in nouns[:5]:
        print(f"  {word:<16} {klass}")

    all_nouns = engine.words("substantiivi", include_classless=True)
    classless = [row for row in all_nouns if row[1].startswith("(none")]
    print(f"\nIncluding classless compounds: {len(all_nouns)}")
    print(f"  with a classed head: {sum(1 for row in classless if row[1] != '(none)')}")
    print(f"  with no classed head: {sum(1 for row in classless if row[1] == '(none)')}")
    for word, klass in classless[:8]:
        print(f"  {word:<20} {klass}")

    print("\nOther parts of speech:")
    for pos in ["verbi", "adjektiivi", "adverbi", "numeraali", "pronomini"]:
        print(f"  {pos:<12} {len(engine.words(pos))} lemmas")


if __name__ == "__main__":
    main()
