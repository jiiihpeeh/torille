#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate torille's generated verb forms against libvoikko.

For each verb given on the command line (or a built-in sample), run torille's
`inflect` tool, take the `plain` forms, and feed each through libvoikko's
analyzer. A torille form that voikko does not recognize as the same verb is a
likely morphological bug.

Usage:
    python3 tools/check_verbs_with_voikko.py [LEMMA ...]
"""

import subprocess
import sys

from voikko import libvoikko

VOIKKO = libvoikko.Voikko("fi")

DEFAULT_VERBS = ["laskea", "puhua", "tulla", "mennä", "juosta", "nousta", "purra",
                 "rohkaista", "pierrä", "tavata", "tarvita", "vanheta", "saada",
                 "voida", "tuntea", "nähdä", "tehdä", "tupakoida", "salata"]


def torille_forms(lemma: str) -> list[str]:
    out = subprocess.run(
        ["cargo", "run", "-q", "-p", "torille-tools", "--bin", "inflect", "--", lemma],
        capture_output=True, text=True, check=True,
    ).stdout
    forms = []
    in_plain = False
    for line in out.splitlines():
        if line.startswith("## plain"):
            in_plain = True
            continue
        if line.startswith("## repaired") or line.startswith("## guessed"):
            in_plain = False
            continue
        if in_plain and line.strip():
            # The word is everything before the confidence token, and it can be
            # multi-word (`en teroita`, `olen puhunut`), so split on the known
            # confidence words rather than on the first token.
            parts = line.split()
            for i, tok in enumerate(parts):
                if tok in ("plain", "guessed", "repaired"):
                    forms.append(" ".join(parts[:i]))
                    break
    return forms


def main() -> None:
    verbs = sys.argv[1:] or DEFAULT_VERBS
    total = bad = 0
    for verb in verbs:
        verb_bad = 0
        for form in torille_forms(verb):
            # A multi-word analytic form (`en puhu`, `olen puhunut`, `on
            # jääty`) is analysed word by word: every word must be a real
            # Finnish word for the whole to be valid, so the per-word
            # recognitions are ANDed. A single-word form is valid if voikko
            # recognises it as the verb.
            words = form.split()
            if len(words) > 1:
                ok = all(VOIKKO.analyze(w) for w in words)
                if not ok:
                    verb_bad += 1
                    bad += 1
                    print(f"  BAD {verb}: {form} -> a word is not recognised")
                continue
            total += 1
            analysis = VOIKKO.analyze(form)
            # A form is valid if voikko recognises it as the verb: either its
            # base form is the verb, or the verb appears in the WORDBASES of a
            # participle/derived analysis (e.g. `pierevät` -> `+piere(pierrä)`).
            ok = any(
                a.get("BASEFORM") == verb
                or verb in a.get("WORDBASES", "")
                for a in analysis
            )
            if not ok:
                verb_bad += 1
                bad += 1
                bases = {a.get("BASEFORM") for a in analysis} if analysis else {"(none)"}
                print(f"  BAD {verb}: {form} -> {bases}")
        print(f"{verb}: {len(torille_forms(verb))} forms, {verb_bad} unrecognised")
    print(f"\n{total} plain forms checked, {bad} not recognised as their lemma")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())