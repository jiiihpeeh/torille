# SPDX-License-Identifier: MIT OR Apache-2.0
"""Reference recall over every class-50/51 compound, measured on the real engine.

The fingerprint cannot see compound defects. It draws its lemmas from the kotus
noun database, and only 22 of the 695 compound-marker lemmas in the sanalista
appear there at all -- so a compound fix can leave the digest byte-identical
while changing 16 words' output. This is the measurement that does see them.

It measures *recall only*, and it compares like with like: the reference is
filtered to `ok='both'` (the only double-confirmed value) and to
`possessive=''`, because the sanalista rows that carry no inflection field
would otherwise pin every figure to the same number and measure nothing. Recall
is the only sound direction here -- the reference does not list a form as wrong,
so a form absent from it is unconfirmed, not incorrect, and precision has to be
spot-checked by eye.

Both classes of a marker row run, as `inflect` does, unless a single class is
named on the command line. That matters: measuring one class at a time reports
`isoviha` class 50 as broken at 1/23 when the union covers 23/23.

    python3 tools/compound_recall.py OUT.tsv
"""

import io
import sys

import sqlite3

from torille import Engine

SANALISTA = "data/nykysuomensanalista.txt"
DB = "file:data/kotus_nomines.sqlite?mode=ro"


def marker_lemmas():
    """Every `(lemma, class)` the sanalista marks 50 or 51."""
    out = set()
    with io.open(SANALISTA, encoding="utf-8") as fh:
        head = next(fh).strip().split("\t")
        ci, wi = head.index("Taivutustiedot"), head.index("Hakusana")
        for line in fh:
            p = line.rstrip("\n").split("\t")
            if len(p) <= ci or not p[wi]:
                continue
            for item in p[ci].split(","):
                if item.strip().split("*")[0] in ("50", "51"):
                    out.add((p[wi], item.strip()))
    return sorted(out)


def main(path, only_class=None):
    engine = Engine()
    db = sqlite3.connect(DB, uri=True)
    rows = []
    for word, cls in marker_lemmas():
        ref = {
            r[0]
            for r in db.execute(
                "SELECT DISTINCT form FROM forms"
                " WHERE lemma=? AND ok='both' AND possessive=''",
                (word,),
            )
        }
        if not ref:
            # Absence of reference forms is unconfirmed, not a wrong answer, and
            # a zero denominator would inflate every rate below.
            continue
        forms = (
            engine.inflect(word, klass=only_class)
            if only_class
            else engine.inflect(word)
        )
        got = {f.word for f in forms if not f.possessive}
        rows.append((word, cls, len(ref & got), len(ref), len(got)))
    with open(path, "w", encoding="utf-8") as fh:
        for row in rows:
            fh.write("\t".join(str(x) for x in row) + "\n")

    total_ref = sum(r[3] for r in rows)
    total_hit = sum(r[2] for r in rows)
    print(f"marker lemmas with reference forms: {len(rows)}")
    print(f"recall: {total_hit}/{total_ref} = {total_hit / total_ref * 100:.1f}%")
    print(f"at 100%: {sum(1 for r in rows if r[2] == r[3])}/{len(rows)}")
    # Below 100% but with more than one confirmed form: a lemma at 1/1 is done.
    short = [r for r in rows if r[2] < r[3] and r[3] > 1]
    print(f"short of the reference: {len(short)}")
    for word, cls, hit, n, _ in sorted(short, key=lambda r: r[3] - r[2], reverse=True):
        print(f"    {word:20} {cls:5} {hit}/{n}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None)