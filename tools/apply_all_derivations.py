#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Apply every productive derivative flag to the whole word list.

Reads `data/nykysuomensanalista.csv`, derives the candidates for each rule in
`RULES` from every verb row, keeps only candidates that Voikko confirms as an
independent lemma, and writes the full word list to

    data/nykysuomensanalista_derivates.csv

The flag stays on the **base** row (`nostaa  FREQ:67`); the generated file is
still a word list, not a form table. `src/expand/derived.rs` expands the flags
with the class engine at build/runtime.

Rules currently applied to classified rows:
    FREQ        verb -> verb      -tella / -tellä, class 67
    REFL        verb -> verb      -utua / -ytyä, class 52
    VERB_OI     noun -> verb      -oida / -öidä, class 62
    ADJ_LLINEN  noun -> adjective -llinen, class 38
    ADJ_INEN    noun -> adjective -inen, class 38
    ABSTRACT_UUS adjective -> noun -uus / -yys, class 40
    VERB_ISTAA  adjective -> verb -istaa / -istää, class 53
    PRIV_TON    noun -> adjective -ton / -tön, class 34*C
    POSS_KAS    noun -> adjective -kas / -käs, class 41*A
    ADV_STI_*   adjective -> adverb -sti, class 99 (indeclinable)
    ACTION_US_TA verb -> noun -us / -ys, class 39
    ACTION_MA   verb -> noun -ma / -mä, class 10
    ACTION_IN   verb -> noun -in, class 33
    NOUN_KKO    noun -> noun -kko / -kkö, class 4*A
    NOUN_STO    noun -> noun -sto / -stö, class 2
    NOUN_KUNTA  noun -> noun -kunta, class 10*J
    DIM_NEN     noun -> noun -nen, class 38
    ACTION_E_*  verb -> noun -e, class 48 (class-conditioned)
    ACTION_IN_* verb -> noun -in, class 33 (class-conditioned)
    FREQ_ELLA   verb -> verb  -ella / -ellä, class 67
    FREQ_SKELLA verb -> verb  -skella / -skellä, class 67
    MOM_AHTAA   verb -> verb  -ahtaa / -ähtää, class 53*F
    CAUS_TTAA   verb -> verb  -ttaa / -ttää, class 53*C

A derived lemma that is already a Kotus word is skipped: the base lexicon
already inflects it, and a flag would create a duplicate lexeme. Existing
flags are preserved.

Usage:
    python3 tools/apply_all_derivations.py
    python3 tools/apply_all_derivations.py --out /tmp/derivates.csv
    python3 tools/apply_all_derivations.py --no-voikko   # faster, unconfirmed
    python3 tools/apply_all_derivations.py --check
"""

from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / "data" / "nykysuomensanalista.csv"
OUT = REPO / "data" / "nykysuomensanalista_derivates.csv"

sys.path.insert(0, str(REPO / "tools"))
from add_derivation_flag import (  # noqa: E402
    BULK_RULES,
    RULES,
    derive_chain,
    inflect,
    row_flags,
    validate,
    write_rows,
)


def voikko_speller():
    try:
        from voikko import libvoikko
    except Exception as exc:  # noqa: BLE001
        print(f"warning: Voikko unavailable ({exc}); candidates are unconfirmed", file=sys.stderr)
        return None
    return libvoikko.Voikko("fi")


def confirmed_by_voikko(voikko, lemma: str) -> bool:
    return voikko.spell(lemma) and any(
        analysis.get("BASEFORM") == lemma for analysis in voikko.analyze(lemma)
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--no-voikko", action="store_true", help="do not require Voikko confirmation")
    ap.add_argument("--check", action="store_true", help="validate the output file, write nothing")
    args = ap.parse_args()

    with SOURCE.open(encoding="utf-8", newline="") as fh:
        raw = list(csv.reader(fh, delimiter="\t"))
    rows = [row for row in raw[1:] if row]
    words = {row[0] for row in rows}
    # "Classified" = the row carries a Kotus class. Classless compounds are
    # handled by the compound splitter and are not derivation bases here.
    classified = [
        row for row in rows
        if len(row) > 3 and row[3].strip()
        and row[2].strip() in {"substantiivi", "adjektiivi", "verbi"}
    ]

    if args.check:
        out = Path(args.out)
        if not out.exists():
            print(f"missing {out}", file=sys.stderr)
            return 2
        with out.open(encoding="utf-8", newline="") as fh:
            checked = list(csv.reader(fh, delimiter="\t"))
        checked_rows = [row for row in checked[1:] if row]
        if validate(checked_rows, quiet=True) != 0:
            return 1
        flagged = sum(1 for row in checked_rows if row_flags(row))
        print(f"ok: {len(checked_rows)} rows, {flagged} flagged base rows")
        return 0

    voikko = None if args.no_voikko else voikko_speller()
    added = {kind: 0 for kind in BULK_RULES}
    skipped_existing = 0
    rejected = 0
    seen: set[str] = set()
    claimed: dict[str, str] = {}

    # Existing flags (including the `sämpylä` chain) already claim their
    # derived lemmas; a later base must not create the same lexeme again.
    for row in rows:
        for flag in row_flags(row):
            try:
                lemma, _, _ = derive_chain(
                    row[0], flag, row[2].strip(),
                    row[3].strip() if len(row) > 3 else "",
                )
            except ValueError:
                continue
            claimed.setdefault(lemma, f"{row[0]}:{flag}")

    for row in classified:
        base = row[0]
        pos = row[2].strip()
        # The Kotus list has homonym rows with the same headword. A flag is a
        # property of the lemma, not of one homonym row; put it on the first
        # row only so the generated file does not repeat the same derivation.
        if base in seen:
            continue
        seen.add(base)
        row.extend([""] * (5 - len(row)))
        flags = row_flags(row)
        base_class = row[3].strip() if len(row) > 3 else ""
        for kind in BULK_RULES:
            rule = RULES[kind]
            if not pos.startswith(str(rule["input"])):
                continue
            allowed = rule.get("input_classes")
            if allowed:
                class_num = base_class.split("*", 1)[0]
                if not class_num.isdigit() or int(class_num) not in allowed:
                    continue
            flag = f"{kind}:{rule['class']}"
            # Do not duplicate a step that is already the start of a chain
            # (`sämpylä` carries `VERB_OI:62>...`; no bare `VERB_OI:62`).
            if any(existing.split(">", 1)[0] == flag for existing in flags):
                continue
            try:
                lemma, out_pos, out_class = derive_chain(
                    base, flag, pos, base_class
                )
            except ValueError:
                rejected += 1
                continue
            if lemma in words:
                skipped_existing += 1
                continue
            if lemma in claimed:
                continue
            if voikko is not None and not confirmed_by_voikko(voikko, lemma):
                rejected += 1
                continue
            forms = inflect(lemma, out_class, out_pos)
            if lemma not in forms:
                rejected += 1
                continue
            flags.append(flag)
            claimed[lemma] = f"{base}:{flag}"
            added[kind] += 1
        if flags:
            row[4] = ";".join(flags)

    # Existing flags (on rows that are not verbs, or were already present)
    # have already been carried through; validate the whole result before
    # writing so a bad rule cannot land silently.
    if validate(rows, quiet=True) != 0:
        return 1

    out = Path(args.out)
    write_rows(out, rows)
    total = sum(added.values())
    print(
        f"wrote {out}: {len(rows)} rows, {total} new flags "
        f"({', '.join(f'{k}={v}' for k, v in added.items())}); "
        f"skipped {skipped_existing} already-lemma candidates, "
        f"rejected {rejected} unconfirmed candidates"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
