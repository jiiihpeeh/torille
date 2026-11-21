#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Generate the deterministic cold-morphology benchmark corpora.

The clean corpus is committed (`tests/fixtures/perf/page-2000.txt`).  These
three variants are derived so every probe-path measurement uses the same text:

  page-errors-x.txt    every alphabetic word with an appended `x` (all unknown)
  page-errors-8pct.txt ~8 % of the unique words with one vowel substituted
  manifest.json        the seed, the percentage and every substitution

The substitutions are chosen from the sorted unique surface list, so the
output does not depend on Python's set/dict iteration order.  The whitespace
and punctuation of the source are preserved byte for byte.
"""

from __future__ import annotations

import argparse
import json
import random
import re
from pathlib import Path

WORD = re.compile(r"[^\W\d_]+", re.UNICODE)
VOWELS = "aeiouyäö"
DEFAULT_SOURCE = Path("tests/fixtures/perf/page-2000.txt")
DEFAULT_OUT = Path("reports/corpora")


def corrupt_word(rng: random.Random, word: str) -> str:
    positions = [
        index
        for index, char in enumerate(word)
        if 0 < index < len(word) - 1 and char.lower() in VOWELS
    ]
    if not positions:
        return word
    index = rng.choice(positions)
    original = word[index]
    replacement = rng.choice([vowel for vowel in VOWELS if vowel != original.lower()])
    if original.isupper():
        replacement = replacement.upper()
    return word[:index] + replacement + word[index + 1 :]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument(
        "--percent",
        type=float,
        default=8.0,
        help="percentage of unique words to corrupt in the 8%% variant",
    )
    args = parser.parse_args()

    text = args.source.read_text(encoding="utf-8")
    words = sorted(set(WORD.findall(text)))

    appended = WORD.sub(lambda match: match.group(0) + "x", text)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "page-errors-x.txt").write_text(appended, encoding="utf-8")

    rng = random.Random(args.seed)
    substitutions: dict[str, str] = {}
    for word in words:
        if len(word) < 5:
            continue
        if rng.random() * 100.0 >= args.percent:
            continue
        changed = corrupt_word(rng, word)
        if changed != word:
            substitutions[word] = changed

    corrupted = WORD.sub(lambda match: substitutions.get(match.group(0), match.group(0)), text)
    (args.out / "page-errors-8pct.txt").write_text(corrupted, encoding="utf-8")

    manifest = {
        "source": str(args.source),
        "seed": args.seed,
        "percent": args.percent,
        "unique_words": len(words),
        "substitutions": dict(sorted(substitutions.items())),
        "appended_x": True,
    }
    (args.out / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    print(f"wrote {args.out}/page-errors-x.txt ({len(words)} unique words)")
    print(
        f"wrote {args.out}/page-errors-8pct.txt "
        f"({len(substitutions)} substitutions, seed {args.seed})"
    )
    print(f"wrote {args.out}/manifest.json")


if __name__ == "__main__":
    main()
