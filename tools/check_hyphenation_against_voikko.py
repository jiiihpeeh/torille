#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare torille's permitted hyphenation points with Voikko.

Voikko's ``hyphenate`` returns a syllabification, not the same
line-breaking policy: it offers breaks between vowels that Finnish
typography normally avoids, and it does not know about torille's
compound preference. This is therefore a **diagnostic**, not a CI gate.
The curated golden corpus in ``tests/hyphenation_fi.rs`` is the gate.

Usage:
    python3 tools/check_hyphenation_against_voikko.py [--limit N] [WORD...]

With no words, samples every ``--limit``th lemma from the sanalista.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

from voikko import libvoikko

REPO = Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "release" / "hyphenate"
SANALISTA = REPO / "data" / "nykysuomensanalista.csv"


def char_offsets(hyphenated: str) -> list[int]:
    """Turn ``mat-ka`` into the character offsets after each part."""
    offsets = []
    total = 0
    for part in hyphenated.split("-"):
        total += len(part)
        offsets.append(total)
    return offsets[:-1]


def load_words(limit: int) -> list[str]:
    words = []
    with SANALISTA.open(encoding="utf-8") as handle:
        next(handle, None)
        for index, line in enumerate(handle):
            if index % limit:
                continue
            word = line.split("\t", 1)[0]
            if word and "-" not in word:
                words.append(word)
    return words


def engine_points(words: list[str]) -> dict[str, list[int]]:
    points: dict[str, list[int]] = {}
    for start in range(0, len(words), 2000):
        chunk = words[start : start + 2000]
        result = subprocess.run(
            [str(BINARY), "--json", *chunk],
            capture_output=True,
            text=True,
            check=True,
        )
        for line in result.stdout.splitlines():
            row = json.loads(line)
            points[row["word"]] = [point["char_offset"] for point in row["points"]]
    return points


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("words", nargs="*", help="words to check")
    parser.add_argument(
        "--limit",
        type=int,
        default=997,
        help="sample every Nth sanalista lemma (default: 997)",
    )
    parser.add_argument(
        "--show",
        type=int,
        default=20,
        help="how many examples of each kind to print (default: 20)",
    )
    parser.add_argument("--fail", action="store_true", help="exit 1 on differences")
    args = parser.parse_args()

    if not BINARY.exists():
        print("build the CLI first: cargo build --release -p torille-tools", file=sys.stderr)
        return 2
    words = args.words or load_words(max(1, args.limit))
    ours = engine_points(words)
    voikko = libvoikko.Voikko("fi")

    missing: list[tuple[str, int]] = []
    extra: list[tuple[str, int]] = []
    vowel_extra: list[tuple[str, int]] = []
    for word in words:
        expected = char_offsets(voikko.hyphenate(word))
        actual = ours.get(word, [])
        for offset in expected:
            if offset not in actual:
                missing.append((word, offset))
        for offset in actual:
            if offset not in expected:
                extra.append((word, offset))
                if 0 < offset < len(word):
                    left, right = word[offset - 1], word[offset]
                    if left in "aeiouyäöå" and right in "aeiouyäöå":
                        vowel_extra.append((word, offset))

    print(f"words: {len(words)}")
    print(f"missing (Voikko break not offered): {len(missing)}")
    print(f"extra   (offered, Voikko does not): {len(extra)}")
    print(f"  of which vowel-vowel:             {len(vowel_extra)}")
    for label, rows in (
        ("missing", missing),
        ("extra", extra),
        ("vowel", vowel_extra),
    ):
        if rows:
            print(f"\n{label} examples:")
            for word, offset in rows[: max(0, args.show)]:
                print(f"  {word:24} {offset}")
    return 1 if args.fail and (missing or extra) else 0


if __name__ == "__main__":
    sys.exit(main())
