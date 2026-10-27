#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Run the Omorfi + Voikko harness over a wordlist.

The selection is the point of this script: a full wordlist is tens of
thousands of lemmas and the interesting part is a slice of it. The words come
from a file, from the sanalista, or from a built-in sample; they can be cut
down at random, restricted to particular cases or numbers, or both at once.
`--list` prints what would be run and stops, so a selection can be checked
before spending the time.

The comparison itself is `check_word_against_omorfi`: Omorfi generates the
paradigm, Voikko confirms it, and the surviving set is compared with what
torille offers as plain, in both directions.

`-j` only pays off with the torille extension module built, which is what lets
inflection run outside the GIL. Without it the work happens in the `inflect`
binary, one process per word:

    (cd bindings && env -u CONDA_PREFIX maturin develop)

Examples:
    # every case of one word
    python3 tools/run_wordlist.py kala

    # 40 random nouns, genitive and partitive only
    python3 tools/run_wordlist.py --random 40 -c GEN,PAR --seed 7

    # the whole sanalista, plural oblique only, in parallel
    python3 tools/run_wordlist.py --sanalista -c INE,ADE,ALL -j 8

Usage:
    python3 tools/run_wordlist.py [options] [LEMMA...]
"""

from __future__ import annotations

import argparse
import random
import sys
import threading

from omorfi_harness import (
    ALL_CASES,
    Harness,
    Verdict,
    eprint,
    noun_cells,
    verb_cells,
)

#: (tense, mood) pairs of the finite verb.
VERB_TENSE_MOODS = [("PRESENT", "INDV"), ("PAST", "INDV"), ("", "COND")]

SAMPLE = [
    "kala", "talo", "valo", "käsi", "joki", "lapsi", "mies", "nainen", "tiili",
    "pieni", "ohut", "sisar", "askel", "hame", "korkea", "vapaa", "maa", "suo",
    "filee", "vieras", "solakka", "kuollut", "vanhempi", "onneton", "vastaus",
]

_harnesses: dict[str, Harness] = {}
_harnesses_lock = threading.Lock()


def harness(pos: str) -> Harness:
    """The one `Harness` for the process, however many threads ask for it.

    Not one per thread. Building a `Harness` loads Omorfi, which costs about
    7.5s, so a per-thread harness made `-j 8` pay that eight times: 60s of
    loading before any word was checked. Omorfi and Voikko are not safe for
    concurrent use, so `Harness` serialises their calls on a lock instead --
    a call is microseconds, the load is seconds, and one copy of each is the
    better trade by three orders of magnitude.
    """
    got = _harnesses.get(pos)
    if got is None:
        with _harnesses_lock:
            got = _harnesses.get(pos)
            if got is None:
                got = _harnesses[pos] = Harness(pos=pos)
    return got


def sanalista_words(pos: str, limit: int | None = None) -> list[tuple[str, str]]:
    """Lemmas of the given part of speech, with their first Kotus class.

    Read through the binding's `words()` when it is available, so the sanalista
    is parsed once by the Rust parser rather than again by splitting the file
    here, and the part-of-speech test is the sanalista's own rather than a
    substring match. Falls back to reading the file.
    """
    wanted = "substantiivi" if pos == "noun" else "verbi"
    from omorfi_harness import _load_engine

    engine_cls = _load_engine()
    if engine_cls is not None:
        pairs = engine_cls().words(wanted)
        return pairs[:limit] if limit else pairs
    out: list[tuple[str, str]] = []
    with open("data/nykysuomensanalista.csv", encoding="utf-8") as fh:
        next(fh, None)
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if len(parts) >= 4 and wanted in parts[2] and parts[0] and parts[3]:
                out.append((parts[0], parts[3]))
            if limit and len(out) >= limit:
                break
    return out


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("lemmas", nargs="*", help="words to check (default: a sample)")
    p.add_argument("-p", "--pos", choices=("noun", "verb"), default="noun")
    p.add_argument("-c", "--cases", default="all",
                   help=f"comma-separated case codes, or 'all' ({','.join(ALL_CASES)})")
    p.add_argument("-n", "--numbers", choices=("sg", "pl", "both"), default="both")
    src = p.add_argument_group("where the words come from")
    src.add_argument("-f", "--file", help="a file of lemmas, one per line")
    src.add_argument("--sanalista", action="store_true",
                     help="use every lemma of the part of speech in the sanalista")
    src.add_argument("--limit", type=int, help="stop after this many sanalista lemmas")
    src.add_argument("-r", "--random", type=int, metavar="N",
                     help="pick N words at random from the list")
    src.add_argument("--seed", type=int, help="seed for --random, so a run repeats")
    p.add_argument("-j", "--jobs", type=int, default=4,
                   help="words to check at once (default: 4)")
    p.add_argument("--lenient", action="store_true",
                   help="report unconfirmed plain forms but do not fail on them")
    p.add_argument("--no-possessives", action="store_true",
                   help="check only the bare case/number cells, skipping the "
                        "possessive paradigm")
    p.add_argument("--strict-tags", action="store_true",
                   help="also fail on a confirmed form offered under the wrong cell")
    p.add_argument("--list", action="store_true",
                   help="print the selection and stop")
    p.add_argument("-q", "--quiet", action="store_true", help="print failures only")
    return p.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    numbers = ["SG", "PL"] if args.numbers == "both" else [args.numbers.upper()]
    if args.pos == "noun":
        cases = ALL_CASES if args.cases == "all" else [
            c.strip().upper() for c in args.cases.split(",") if c.strip()
        ]
        bad = [c for c in cases if c not in ALL_CASES]
        if bad:
            eprint(f"unknown case code(s): {', '.join(bad)}")
            eprint(f"known: {','.join(ALL_CASES)}")
            return 2
        cells = noun_cells(cases, numbers, [""] if args.no_possessives else None)
    else:
        if args.cases != "all":
            eprint("a verb has no case to select; drop -c or use -n")
            return 2
        cells = verb_cells(VERB_TENSE_MOODS, numbers)

    words = list(args.lemmas)
    classes: dict[str, str] = {}
    if args.file:
        with open(args.file, encoding="utf-8") as fh:
            words += [ln.strip() for ln in fh if ln.strip()]
    if args.sanalista:
        pairs = sanalista_words(args.pos, args.limit)
        for word, cls in pairs:
            classes.setdefault(word, cls)
        words += [w for w, _ in pairs]
    if not words:
        words = list(SAMPLE)
    # A word given twice is checked twice otherwise, and the selection is the
    # thing being reported, so duplicates go.
    seen: set[str] = set()
    words = [w for w in words if not (w in seen or seen.add(w))]

    if args.random:
        if args.random > len(words):
            eprint(f"--random {args.random} but only {len(words)} words available")
            return 2
        words = random.Random(args.seed).sample(words, args.random)

    if args.list:
        for word in words:
            cls = classes.get(word)
            print(f"{word} {cls}" if cls else word)
        print(f"\n{len(words)} word(s) selected", file=sys.stderr)
        return 0

    # Each worker builds its own Harness, so the pool is entered once here
    # rather than per word; the first call pays for the load.
    from concurrent.futures import ThreadPoolExecutor

    def work(word: str) -> Verdict:
        return harness(args.pos).check(word, cells, lenient=args.lenient)

    jobs = max(1, min(args.jobs, len(words)))
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        verdicts = list(pool.map(work, words))

    failed: list[Verdict] = []
    skipped = 0
    for verdict in verdicts:
        if verdict.skipped:
            skipped += 1
            if not args.quiet:
                print(verdict.report())
            continue
        bad = not verdict.ok or (args.strict_tags and verdict.mistagged)
        if bad:
            failed.append(verdict)
        if bad or not args.quiet:
            print(verdict.report())
            if bad:
                for form in sorted(verdict.missing):
                    print(f"    MISSING   {form}")
                for form in sorted(verdict.not_plain):
                    print(f"    NOT-PLAIN {form}")
                for form in sorted(verdict.mistagged):
                    print(f"    MISTAGGED {form}")
                if not args.lenient:
                    for form in sorted(verdict.illegal):
                        print(f"    ILLEGAL   {form}")

    checked = [v for v in verdicts if not v.skipped]
    total_forms = sum(len(v.confirmed) for v in checked)
    totals = {
        "missing": sum(len(v.missing) for v in checked),
        "not plain": sum(len(v.not_plain) for v in checked),
        "illegal in plain": sum(len(v.illegal) for v in checked),
        "mistagged": sum(len(v.mistagged) for v in checked),
    }
    print(f"\n{len(checked)} word(s) checked ({skipped} skipped), "
          f"{total_forms} confirmed forms")
    for kind, count in totals.items():
        print(f"  {count:6d} {kind}")
    if failed:
        print(f"\n{len(failed)} word(s) failed: "
              f"{', '.join(v.lemma for v in failed[:20])}"
              f"{' ...' if len(failed) > 20 else ''}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
