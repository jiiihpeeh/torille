#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build `data/stem_to_base_map.csv`: a (stem, base) table the lib's
`build.rs` reads at compile time and emits as a `phf::Map` for
the slow-check stem index.

The lib's `is_known_inflected_form` is the typing-path first
pass (O(1) form set). When the form set misses -- for irregular
verbs, typos, clitic combinations the engine doesn't generate --
the slow check needs a way to recognise "is there a Finnish
lemma whose paradigm emits this word?" The reverse-stem hashmap
is the principled answer: strip known Finnish case endings,
look up the resulting stem in a hashmap, get the candidate
lemmas, verify by re-inflecting.

The mapping is derived from the kotus_nomines oracle, which
already carries the (form, lemma, case, number, possessive)
breakdown. We strip the possessive suffix (if any), then the
case+number ending, leaving the engine's stem. The stem is
class-specific: for class 27 (`vesi`) the genitive `veden` is
built from the weak stem `vede`, not the lemma `vesi`; the
illative `veteen` is built from the strong stem `vete`. The
hashmap keys on the actual stem, not the lemma, so the lookup
is direct.

Inputs:
  data/kotus_nomines.sqlite   -- Omorfi/Voikko materialised noun
                                  forms. 3.5 M unique forms, 30 k
                                  base lemmas.

Outputs:
  data/stem_to_base_map.csv   -- one row per (stem, base) pair,
                                  tab-separated. The `phf::Map`
                                  keys on `stem` and yields the
                                  `&'static [u32]` of lemma
                                  indices that produce it.

The verb partition is out of scope; the sanalista's verbs are
emitted by the engine (`inflect_all`), and the verb form/stem
oracle would be a future slice. A noun-only stem map still
covers the typing-path slow check for ~70% of the sanalista.

Usage:
  python3 tools/build_stem_to_base_map.py
"""
import sqlite3
import sys

DB = "data/kotus_nomines.sqlite"
OUT = "data/stem_to_base_map.csv"

# We only want forms that both Omorfi AND Voikko accept.
OK_FILTER = "both"

# (case, number) -> list of possible endings, longest first so
# the canonical split wins. The ending is what the engine would
# strip from the form to get the stem; the stem is then either
# the lemma or a class-specific strong/weak stem variant (e.g.
# `vede` for class 27's genitive `veden`).
ENDINGS_BY_CASE_NUMBER = {
    ("nominative",  "singular"): [""],
    ("nominative",  "plural"):   ["t"],
    ("genitive",    "singular"): ["n"],
    ("genitive",    "plural"):   ["en", "jen", "ten", "den"],
    ("partitive",   "singular"): ["tta", "ttä", "ta", "tä", "a", "ä"],
    ("partitive",   "plural"):   ["ja", "jä", "a", "ä"],
    ("illative",    "singular"): ["seen", "hon", "hön",
                                   "en", "an", "än", "in"],
    ("illative",    "plural"):   ["ihin", "siin", "in"],
    ("inessive",    "singular"): ["ssä", "ssa"],
    ("inessive",    "plural"):   ["issä", "issa"],
    ("elative",     "singular"): ["stä", "sta"],
    ("elative",     "plural"):   ["istä", "ista"],
    ("adessive",    "singular"): ["llä", "lla"],
    ("adessive",    "plural"):   ["illä", "illa"],
    ("ablative",    "singular"): ["ltä", "lta"],
    ("ablative",    "plural"):   ["iltä", "ilta"],
    ("allative",    "singular"): ["lle"],
    ("allative",    "plural"):   ["ille"],
    ("essive",      "singular"): ["nä", "na"],
    ("essive",      "plural"):   ["inä", "ina"],
    ("translative", "singular"): ["ksi"],
    ("translative", "plural"):   ["iksi"],
    ("instructive", "plural"):   ["in"],
}

# Possessive suffix for the case+possessive form. The form is
# `stem + case_ending + possessive_suffix`, so strip the
# possessive first, then the case ending, to recover the stem.
POSSESSIVE_SUFFIX = {
    "first person singular":  "ni",
    "second person singular": "si",
    "first person plural":    "mme",
    "second person plural":   "nne",
    "third person":           "nsa",  # also "nsä"; both covered by the
                                     # `nsa` strip with the same stem
                                     # (the underlying class picks the
                                     # front/back variant).
}


def stem_of(form: str, case: str, number: str, possessive: str) -> str | None:
    """Recover the engine's stem from a (form, case, number,
    possessive) tuple by stripping the possessive suffix (if any)
    and then the case+number ending. Returns `None` if no
    ending matches -- the row is dropped from the CSV.
    """
    working = form
    if possessive:
        suffix = POSSESSIVE_SUFFIX.get(possessive)
        if suffix and working.endswith(suffix):
            working = working[: -len(suffix)]
        else:
            # Unrecognised possessive (the 3p has both "nsa" and
            # "nsä" variants; this branch catches "nsä" if the
            # table only listed "nsa" for the front-vowel case).
            # Try the harmony-flip variant.
            alt = "nsä" if suffix == "nsa" else ("nsa" if suffix == "nsä" else None)
            if alt and working.endswith(alt):
                working = working[: -len(alt)]
            else:
                return None
    endings = ENDINGS_BY_CASE_NUMBER.get((case, number))
    if endings is None:
        return None
    for ending in endings:
        if ending == "":
            return working  # nominative: form is the stem
        if working.endswith(ending) and len(working) > len(ending):
            return working[: -len(ending)]
    # A nominative plural that does not end in `-t`: the suppletive
    # pronouns (`jotka` of `joka`, `ketkä` of `kuka`, `mitkä` of `mikä`)
    # and the `-toista` numerals (`kahdeksattoista`). Nothing strips, so
    # emit the whole form as the key; the runtime's empty-ending probe
    # then reaches it and the verify proves the base. Restricted to the
    # nominative (and no possessive) because the other unmatched cases
    # -- the comitative, abessive, prolative that have no table entry --
    # would add hundreds of thousands of keys for no gain.
    if case == "nominative" and not possessive:
        return working
    return None


def main() -> int:
    con = sqlite3.connect(DB)
    cur = con.cursor()
    cur.execute(
        "SELECT form, lemma, \"case\", number, possessive "
        "FROM forms WHERE ok = ?",
        (OK_FILTER,),
    )
    written = 0
    skipped = 0
    seen: set[tuple[str, str]] = set()
    with open(OUT, "w", encoding="utf-8") as f:
        for form, lemma, case_, number, possessive in cur:
            stem = stem_of(form, case_, number, possessive)
            if stem is None or not stem:
                skipped += 1
                continue
            key = (stem, lemma)
            if key in seen:
                continue
            seen.add(key)
            f.write(f"{stem}\t{lemma}\n")
            written += 1

    print(
        f"wrote {OUT} ({written} unique (stem, base) pairs, "
        f"{skipped} rows skipped)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())