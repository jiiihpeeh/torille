#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Hyphenation: points, fragments, options and soft hyphens.

`hyphenate` returns the same points as the CLI and the wasm: byte and character
offsets, whether the break is a compound boundary, where the break came from,
and a penalty. Character offsets are what a Python string can slice with.
"""

from torille import Engine


def split(word: str, points) -> list[str]:
    fragments, previous = [], 0
    for point in points:
        fragments.append(word[previous : point.char_offset])
        previous = point.char_offset
    fragments.append(word[previous:])
    return fragments


def show(engine: Engine, word: str, **options) -> None:
    points = engine.hyphenate(word, **options)
    fragments = split(word, points)
    print(f"{word!r}: {'-'.join(fragments)}")
    print(f"  points: {[(p.char_offset, p.source, p.penalty) for p in points]}")


def main() -> None:
    engine = Engine()

    for word in ["tietokoneohjelmointi", "linja-auto", "kissa", "epäjärjestelmällisyys"]:
        show(engine, word)

    print("\nOptions:")
    show(engine, "tietokoneohjelmointi", left_min=4, right_min=4)
    show(engine, "tietokoneohjelmointi", compound_breaks=False)
    show(engine, "kofeiini", vowel_breaks="always")

    print("\nA soft hyphen is already an explicit break and stays in the source:")
    soft = "kissa\u00adkoira"
    points = engine.hyphenate(soft)
    print(f"  {soft!r}: {[(p.char_offset, p.source) for p in points]}")
    print(f"  split: {[fragment for fragment in split(soft, points)]}")


if __name__ == "__main__":
    main()
