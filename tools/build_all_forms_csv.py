#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build a CSV of every form in the kotus_nomines oracle.

The lib's `build.rs` reads this CSV at compile time and emits a
`phf::Set<String>` of every form the engine recognises (the
typing-path form set) and a `phf::Map<&str, &[u32]>` from stem
to lemma-index (the slow-check stem index). The form set is the
hash of the wordlist -- O(1) lookup at the typing-path first
pass; the stem index lets the slow check resolve a form the
form set missed by stripping known Finnish case endings and
probing the engine's own output.

Inputs:
  data/kotus_nomines.sqlite   -- Omorfi/Voikko materialised noun
                                  forms (3.5 M unique forms, 3.5 M
                                  unique (form, lemma) pairs, 30 k
                                  base lemmas)

Outputs:
  data/all_forms.csv          -- one row per unique (form, lemma,
                                  case, number, possessive),
                                  tab-separated. The form set the
                                  first pass consults; the case +
                                  number + possessive columns drive
                                  the stem hashmap (a future slice
                                  can derive the canonical
                                  stem-ending split from those).

The CSV is build-time data: run this script whenever the SQLite
oracle is regenerated, commit the CSV, and the lib picks it up
on the next `cargo build`. No runtime walk, no parallel
HashSet::new, no 8 s cold start; the compile-time phf emission
takes the place of all of that.

The verb partition is out of scope for this script -- the
sanalista carries verbs (`@sanasto ~104 k lemmas`) and the engine
emits their forms via `inflect_all`, but Omorfi's materialised
form database is noun-only. A future `kotus_verb` oracle would
extend this script in the obvious way.

Usage:
  python3 tools/build_all_forms_csv.py
"""
import sqlite3
import sys

DB = "data/kotus_nomines.sqlite"
OUT = "data/all_forms.csv"

# We only want forms that both Omorfi AND Voikko accept. The
# oracle's `ok` column is the AND of the two: `both` means
# "this form is real Finnish", `omorfi` alone means "Omorfi says
# yes, Voikko says no" (less reliable for the spell-check first
# pass). Restricting to `both` keeps the form set close to the
# ground truth.
OK_FILTER = "both"


def main() -> int:
    con = sqlite3.connect(DB)
    cur = con.cursor()
    cur.execute(
        "SELECT COUNT(DISTINCT form) FROM forms WHERE ok = ?",
        (OK_FILTER,),
    )
    unique_forms = cur.fetchone()[0]
    cur.execute(
        "SELECT COUNT(*) FROM ("
        "  SELECT form, lemma FROM forms WHERE ok = ? "
        "  GROUP BY form, lemma"
        ")",
        (OK_FILTER,),
    )
    pair_count = cur.fetchone()[0]
    print(
        f"oracle: {unique_forms} unique forms, "
        f"{pair_count} unique (form, lemma) pairs",
        file=sys.stderr,
    )

    cur.execute(
        "SELECT form, lemma, \"case\", number, possessive "
        "FROM forms WHERE ok = ? "
        "GROUP BY form, lemma, \"case\", number, possessive "
        "ORDER BY form, lemma",
        (OK_FILTER,),
    )
    written = 0
    with open(OUT, "w", encoding="utf-8") as f:
        for form, lemma, case_, number, possessive in cur:
            # The oracle's forms are lowercased; the sanalista
            # is lowercased; the lib's first pass lowercases.
            # No case-folding needed at the CSV layer. The
            # `case` + `number` + `possessive` columns are
            # carried for the stem hashmap the build.rs also
            # emits; the form set just needs the form.
            f.write(
                f"{form}\t{lemma}\t{case_}\t{number}\t{possessive}\n"
            )
            written += 1

    print(f"wrote {OUT} ({written} rows)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())