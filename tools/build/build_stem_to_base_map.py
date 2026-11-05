#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build `data/stem_to_base_map.csv`: a `(stem, base)` table the lib's
`build.rs` reads at compile time and emits as the front-coded stem map.

The lib's `is_known_inflected_form` is the typing-path first pass (an O(1)
form set). When the form set misses -- irregular verbs, typos, clitic
combinations the engine does not generate -- the slow check needs a way to
recognise "is there a Finnish lemma whose paradigm emits this word?". The
reverse-stem hashmap is the principled answer: strip known Finnish case
endings, look up the resulting stem, get the candidate lemmas, verify by
re-inflecting.

The stem is class-specific: for class 27 (`vesi`) the genitive `veden` is
built from the weak stem `vede`, not the lemma `vesi`; the illative `veteen`
is built from the strong stem `vete`. The hashmap keys on the actual stem, not
the lemma, so the lookup is direct.

# Sources

`--source engine` (the default) inflects every nominal sanalista lemma with the
current engine. This is the safe direction for a checked-in blob: when an
inflection bug is fixed, its old, invented surfaces cannot survive in the map.

`--source oracle` reads the materialised Kotus oracle's `ok='both'` rows. That
is the historical source, but a database built with `--engine` also contains
engine-generated rows, so it is not an independent authority.

`--source hybrid` uses the oracle as a *candidate* set and
keeps only entries the current engine can still produce -- or whose base is not
a sanalista lemma at all (productive derivations and classless compounds the
oracle enrichment supplied). This removes stale keys such as
`mudtaraaakuit -> mustaraamattu` without losing coverage for derived words.

Inputs:
  data/nykysuomensanalista.csv  -- the embedded lemma list
  data/kotus_nomines.sqlite     -- oracle, only for `oracle`/`hybrid`
  the `torille` Python binding  -- for `engine`/`hybrid`

Outputs:
  data/stem_to_base_map.csv     -- one row per `(stem, base)` pair

Usage:
  python3 tools/build/build_stem_to_base_map.py
  python3 tools/build/build_stem_to_base_map.py --source engine
  python3 tools/build/build_stem_to_base_map.py --source oracle --db data/kotus_nomines.sqlite
"""
import argparse
import sqlite3
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SANALISTA = REPO / "data" / "nykysuomensanalista.csv"
DB = REPO / "data" / "kotus_nomines.sqlite"
OUT = REPO / "data" / "stem_to_base_map.csv"

# The oracle's `ok` value for a form both Omorfi and Voikko accepted.
OK_FILTER = "both"

NOMINAL_POS = ("substantiivi", "adjektiivi", "numeraali", "pronomini")

# The Python binding reports the possessive as `1sg`, `3`, ...; the oracle
# schema spells the same values out. `stem_of` reads the oracle spelling.
POSSESSIVE_BY_BINDING = {
    "1sg": "first person singular",
    "2sg": "second person singular",
    "1pl": "first person plural",
    "2pl": "second person plural",
    "3": "third person",
}

# (case, number) -> list of possible endings, longest first so the canonical
# split wins.
ENDINGS_BY_CASE_NUMBER = {
    ("nominative", "singular"): [""],
    ("nominative", "plural"): ["t"],
    ("genitive", "singular"): ["n"],
    ("genitive", "plural"): ["en", "jen", "ten", "den"],
    ("partitive", "singular"): ["tta", "ttä", "ta", "tä", "a", "ä"],
    ("partitive", "plural"): ["ja", "jä", "a", "ä"],
    ("illative", "singular"): ["seen", "hon", "hön", "en", "an", "än", "in"],
    ("illative", "plural"): ["ihin", "siin", "in"],
    ("inessive", "singular"): ["ssä", "ssa"],
    ("inessive", "plural"): ["issä", "issa"],
    ("elative", "singular"): ["stä", "sta"],
    ("elative", "plural"): ["istä", "ista"],
    ("adessive", "singular"): ["llä", "lla"],
    ("adessive", "plural"): ["illä", "illa"],
    ("ablative", "singular"): ["ltä", "lta"],
    ("ablative", "plural"): ["iltä", "ilta"],
    ("allative", "singular"): ["lle"],
    ("allative", "plural"): ["ille"],
    ("essive", "singular"): ["nä", "na"],
    ("essive", "plural"): ["inä", "ina"],
    ("translative", "singular"): ["ksi"],
    ("translative", "plural"): ["iksi"],
    ("instructive", "plural"): ["in"],
}

# Possessive suffix: strip it before the case ending.
POSSESSIVE_SUFFIX = {
    "first person singular": "ni",
    "second person singular": "si",
    "first person plural": "mme",
    "second person plural": "nne",
    "third person": "nsa",
}


def stem_of(form: str, case: str, number: str, possessive: str) -> str | None:
    """Recover the engine's stem from a `(form, case, number, possessive)`.

    Returns `None` when no ending matches; the row is then dropped.
    """
    working = form
    if possessive:
        suffix = POSSESSIVE_SUFFIX.get(possessive)
        if suffix and working.endswith(suffix):
            working = working[: -len(suffix)]
        else:
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
            return working
        if working.endswith(ending) and len(working) > len(ending):
            return working[: -len(ending)]
    # A nominative plural that does not end in `-t`: suppletive pronouns and
    # `-toista` numerals. Nothing strips, so emit the whole form as the key.
    if case == "nominative" and not possessive:
        return working
    return None


def nominal_rows() -> tuple[list[str], dict[str, set[str]], set[str]]:
    """Every nominal lemma, its POS tags, and the classless lemmas.

    The class is the fourth column. A classless noun is a compound whose head
    names the class (`ilkialaston`, `kristinusko`); it is a real lemma but its
    own row cannot tell the engine how to decline it.
    """
    lemmas: list[str] = []
    pos_by_lemma: dict[str, set[str]] = {}
    classless: set[str] = set()
    seen: set[str] = set()
    with open(SANALISTA, encoding="utf-8") as fh:
        next(fh, None)  # header
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 3 or not parts[0]:
                continue
            tags = {pos for pos in NOMINAL_POS if pos in parts[2]}
            if not tags:
                continue
            lemma = parts[0]
            pos_by_lemma.setdefault(lemma, set()).update(tags)
            if lemma not in seen:
                seen.add(lemma)
                lemmas.append(lemma)
            if len(parts) < 4 or not parts[3].strip():
                classless.add(lemma)
    return lemmas, pos_by_lemma, classless


def from_engine(lemmas: list[str] | None = None):
    """Yield `(form, lemma, case, number, possessive)` from the current engine.

    A classless lemma (a compound whose head names the class) can fail to
    plan; it is skipped rather than guessed. The engine is the thing that will
    verify the map at run time, so a fixed inflection bug cannot leave stale
    keys behind.
    """
    from torille import Engine

    engine = Engine()
    if lemmas is None:
        lemmas = nominal_lemmas()
    for lemma in lemmas:
        try:
            forms = engine.inflect(lemma)
        except (ValueError, RuntimeError):
            continue
        for form in forms:
            if form.case is None or form.number is None:
                continue
            if form.negative or form.clitic is not None:
                continue
            if form.confidence == "guessed":
                continue
            possessive = (
                POSSESSIVE_BY_BINDING.get(form.possessive, form.possessive)
                if form.possessive
                else ""
            )
            yield form.word, lemma, form.case, form.number, possessive


def from_oracle(db: Path):
    """Yield the same tuple shape from the materialised Kotus oracle."""
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    cur = con.cursor()
    cur.execute(
        'SELECT form, lemma, "case", number, possessive FROM forms WHERE ok = ?',
        (OK_FILTER,),
    )
    yield from cur


def rows_to_pairs(rows) -> tuple[list[tuple[str, str]], int]:
    pairs: list[tuple[str, str]] = []
    skipped = 0
    seen: set[tuple[str, str]] = set()
    for form, lemma, case_, number, possessive in rows:
        stem = stem_of(form, case_, number, possessive)
        if stem is None or not stem:
            skipped += 1
            continue
        key = (stem, lemma)
        if key in seen:
            continue
        seen.add(key)
        pairs.append(key)
    return pairs, skipped


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--source",
        choices=("hybrid", "engine", "oracle"),
        default="engine",
        help="candidate source (default: engine -- no database needed; "
             "hybrid/oracle read the Kotus oracle)",
    )
    p.add_argument("--db", type=Path, default=DB, help=f"oracle (default: {DB})")
    p.add_argument("--out", type=Path, default=OUT, help=f"output (default: {OUT})")
    args = p.parse_args(argv)

    lemmas, pos_by_lemma, classless = nominal_rows()
    lemma_set = set(lemmas)
    # A classless lemma is declined only through its head. Recording the
    # classless surface as its own base shadows the compound gate that names
    # the head (`kristinuskon` should resolve to `usko`), so leave it to the
    # oracle-derived candidate set.
    classed_lemmas = [lemma for lemma in lemmas if lemma not in classless]
    engine_pairs, engine_skipped = rows_to_pairs(from_engine(classed_lemmas))

    if args.source == "engine":
        pairs = engine_pairs
        skipped = engine_skipped
    elif args.source == "oracle":
        pairs, skipped = rows_to_pairs(from_oracle(args.db))
    else:
        # Candidate set from the oracle, filtered by what the current engine
        # can still produce. A base that is not a sanalista lemma is a
        # productive derivation or a classless compound the enrichment added;
        # it has no paradigm to re-check, so it is kept.
        valid = set(engine_pairs)
        oracle_pairs, oracle_skipped = rows_to_pairs(from_oracle(args.db))
        kept = []
        for key in oracle_pairs:
            stem, base = key
            if base not in lemma_set:
                # A productive derivation or a classless compound head the
                # oracle enrichment supplied; there is no paradigm of its
                # own to re-check.
                kept.append(key)
                continue
            if key in valid:
                kept.append(key)
                continue
            if base in classless:
                # The old oracle's classless-compound rows are the only
                # source for these heads; keep them rather than losing the
                # whole compound family.
                kept.append(key)
                continue
            # A numeral or pronoun has combining forms the paradigm does not
            # emit (`kolmi` for `kolme`, `kahde` for `kaksi`). Keep those; the
            # rest of the unverified noun/adjective rows are the stale output
            # of an engine bug (`mudtaraaakuit`, `hulpmeaa`).
            if pos_by_lemma.get(base, set()) & {"numeraali", "pronomini"}:
                kept.append(key)
        pairs = sorted(set(kept) | valid)
        skipped = engine_skipped + oracle_skipped

    with open(args.out, "w", encoding="utf-8") as f:
        for stem, lemma in pairs:
            f.write(f"{stem}\t{lemma}\n")

    print(
        f"wrote {args.out} ({len(pairs)} unique (stem, base) pairs, "
        f"{skipped} rows skipped) from {args.source}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
