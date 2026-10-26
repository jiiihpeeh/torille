#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Per-lemma fingerprint of everything the engine generates for a noun.

A refactor that is supposed to move no form has to be able to prove it moved
none, and there are too many forms to check by eye. So the gate is the
engine's own output rather than an oracle: every authoritative class's every
lemma, and for each one a sha256 over the forms it generates. A word moved,
a reading changed, a confidence flipped, a form added or dropped -- all of it
changes the hash, and `diff` of two runs says exactly which lemmas.

That makes this the tool for the *last* stage of the nominal refactor, the one
where the change is meant to be visible: `src/expand/nominals/` becoming
`src/expand/nominals-refactored/`, which stops the engine emitting `noun` for
5 913 adjective, numeral, pronoun and interjection rows. That diff should be
readable -- the expected movement is those rows and 16 two-character fragments
-- and anything else in it is a bug.

Usage:
    python3 tools/fingerprint_nouns.py /tmp/base.txt     # before
    python3 tools/fingerprint_nouns.py /tmp/after.txt    # after
    diff /tmp/base.txt /tmp/after.txt
"""

import hashlib
import re
import sqlite3
import sys

import torille

DB = "file:data/kotus_nomines.sqlite?mode=ro"
EXPAND = "src/expand/mod.rs"

# Every attribute a form carries, so the hash covers the reading and not just
# the spelling. `str()` because the optional attributes come back as None.
FIELDS = ("word", "pos", "case", "number", "possessive", "confidence")


def authoritative_classes():
    """The classes whose module supersedes the heuristic, read out of the source
    rather than restated here, so the two cannot drift apart."""
    src = open(EXPAND, encoding="utf-8").read()
    block = src.split("let authoritative = matches!(")[1].split(");")[0]
    return set(re.findall(r'"([^"]+)"', block))


def lemmas_in(auth):
    db = sqlite3.connect(DB, uri=True)
    return sorted(
        {word for word, cell in db.execute("SELECT lemma,class FROM base")
         if any(c.strip() in auth for c in cell.split(","))}
    )


def main(out_path):
    auth = authoritative_classes()
    lemmas = lemmas_in(auth)
    engine = torille.Engine()
    total = 0
    with open(out_path, "w", encoding="utf-8") as fh:
        for lemma in lemmas:
            rows = sorted(
                "\t".join(str(getattr(f, k)) for k in FIELDS)
                for f in engine.inflect(lemma)
            )
            total += len(rows)
            digest = hashlib.sha256("\n".join(rows).encode()).hexdigest()
            fh.write(f"{lemma}\t{len(rows)}\t{digest}\n")
    print(f"{len(auth)} classes, {len(lemmas)} lemmas, {total} forms -> {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))