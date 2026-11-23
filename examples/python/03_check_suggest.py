#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Spell checking and suggestions: `check`, `suggest`, `check_normal`.

`check` answers "is this a word, and what is it a form of". `suggest` ranks
corrections by edit distance and the engine's own score. `check_normal` is the
shipping per-keystroke gate, with the base and the gate that recognised the
word.
"""

from torille import Engine


def main() -> None:
    engine = Engine()

    print("check: what kind of word is this?")
    for word in ["talo", "talossa", "kahvikekkereitä", "nuori-isäntä", "xyzzy"]:
        outcome = engine.check(word)
        bases = ", ".join(outcome.bases) if outcome.bases else "-"
        print(f"  {word:<20} {outcome.kind:<10} bases: {bases}")

    print("\nsuggest: what did the user probably mean?")
    for word in ["äitj", "Äitj", "kisssa", "salamyhkää", "silmäränpäyksen"]:
        print(f"  {word!r}:")
        for suggestion in engine.suggest(word, 4, 2):
            lemma = suggestion.lemma or "-"
            print(
                f"    {suggestion.word:<16} d={suggestion.edit_distance} "
                f"score={suggestion.existing_score:.2f} "
                f"confidence={suggestion.confidence:<9} "
                f"source={suggestion.source:<24} lemma={lemma}"
            )

    print("\ncheck_normal: the gate behind the typing path")
    for word in ["talossa", "paloi", "siellähänkin", "xyzzy"]:
        outcome = engine.check_normal(word)
        print(
            f"  {word:<16} {outcome.kind:<10} "
            f"base={outcome.base or '-':<10} gate={outcome.gate or '-'}"
        )

    print("\nThe input's case is restored, including the Finnish capitals:")
    print("  äitj ->", [s.word for s in engine.suggest("äitj", 3, 2)])
    print("  Äitj ->", [s.word for s in engine.suggest("Äitj", 3, 2)])


if __name__ == "__main__":
    main()
