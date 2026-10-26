#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Audit the engine's plain verb forms against the verb lexicon.

The oracle is `data/kotus_verbit.sqlite`. It holds two rows per form, one from
Kotus-Verbit and one from omorfi, and `ok` records whether the two agree:
`both` where they do, `omorfi` where only omorfi has it. So there are two
useful reference sets, and the tool reports both:

* `both` -- the forms both sources confirm. This is the strict target: a form
  outside it is one the Kotus-Verbit side never listed, and the class modules
  reach it exactly.
* `union` -- every form either source lists. This is the real precision test,
  because `both` is *incomplete* for rare words (`arkipaiväistaa` has 226 in
  `both` but 963 overall), so a form outside `union` is one neither source
  knows and the engine has invented it.

A form in `union` but not `both` is a real form only one source generated, so
these two columns say different things and both are worth reading.

Only `Plain` forms count: a `Repaired` form is the engine declining to offer it
as a correction, and class 52's module deliberately keeps the standalone passive
connegatives (`sanottaisi`) there, since they are not words on their own.

    python tools/audit_verbs.py puhua aakkostaa ...
"""
import sqlite3
import sys

import torille

DB = "file:data/kotus_verbit.sqlite?mode=ro"


def main(lemmas: list[str]) -> int:
    con = sqlite3.connect(DB, uri=True)
    engine = torille.Engine()
    worst = 0
    for lemma in lemmas:
        cls = con.execute("SELECT class FROM base WHERE lemma=?", (lemma,)).fetchone()
        both = {
            r[0]
            for r in con.execute(
                "SELECT DISTINCT form FROM forms WHERE lemma=? AND ok='both'", (lemma,)
            )
        }
        union = {
            r[0]
            for r in con.execute(
                "SELECT DISTINCT form FROM forms WHERE lemma=? AND ok IN ('both','omorfi')",
                (lemma,),
            )
        }
        got = {f.word for f in engine.inflect(lemma) if f.plain}
        # A form outside either source has genuinely been invented.
        invented = sorted(got - union)
        # Outside `both` but inside `union` is only one-sided confirmation.
        onesided = sorted(got - both)
        missing = sorted(both - got)
        missing_more = sorted(union - got)
        worst += len(missing)
        print(
            f"{lemma:20s} class={cls[0] if cls else '?':8s} "
            f"both={len(both):4d} union={len(union):4d} got={len(got):4d} "
            f"invented={len(invented):3d} one-sided={len(onesided):3d} "
            f"missing={len(missing):3d} missing|union={len(missing_more):4d}"
        )
        if invented:
            print("   INVENTED:", " ".join(invented))
        if missing:
            print("   MISSING:", " ".join(missing))
        elif missing_more:
            print("   UNION-ONLY:", " ".join(missing_more))
    return 0 if worst == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
