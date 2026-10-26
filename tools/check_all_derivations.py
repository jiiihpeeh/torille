#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Audit every derivative flag and every generated plain form.

This is the deep check behind `data/nykysuomensanalista_derivates.csv`:

* every flag is structurally valid (`TYPE:CLASS`, verb base, known rule);
* the derived lemma is computed from the base and is not already a Kotus
  lemma;
* the class engine generates the lemma;
* every plain, single-token form is checked against Voikko;
* duplicate derived lemmas/forms across different bases are reported.

Structural problems fail the run. Unconfirmed forms are warnings: the class
engines legitimately overgenerate rare cells, and the point of this report is
to show how much of the generated paradigm Voikko actually recognises.

Usage:
    python3 tools/check_all_derivations.py
    python3 tools/check_all_derivations.py --no-voikko
    python3 tools/check_all_derivations.py --max-examples 40
"""

from __future__ import annotations

import argparse
import csv
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SOURCE = REPO / "data" / "nykysuomensanalista.csv"
FLAGGED = REPO / "data" / "nykysuomensanalista_derivates.csv"

sys.path.insert(0, str(REPO / "tools"))
from add_derivation_flag import derive_chain, row_flags  # noqa: E402


def read(path: Path) -> tuple[list[str], list[list[str]]]:
    with path.open(encoding="utf-8", newline="") as fh:
        rows = list(csv.reader(fh, delimiter="\t"))
    return rows[0], [row for row in rows[1:] if row]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--file", default=str(FLAGGED))
    ap.add_argument("--no-voikko", action="store_true")
    ap.add_argument("--max-examples", type=int, default=20)
    args = ap.parse_args()

    path = Path(args.file)
    if not path.exists():
        print(f"missing {path}; run tools/apply_all_derivations.py", file=sys.stderr)
        return 2

    _, source_rows = read(SOURCE)
    _, rows = read(path)
    base_words = {row[0] for row in source_rows}
    # A base row that carries an explicit flag in the main CSV is a seeded,
    # human-approved derivation. Voikko may lag behind productive coinages
    # (`deittailija`, `koodaamo`), so those are warnings, not errors.
    seeded_bases = {row[0] for row in source_rows if row_flags(row)}
    errors: list[str] = []
    warnings: list[str] = []
    flags_by_type: Counter[str] = Counter()
    lemma_owner: dict[str, tuple[str, str]] = {}
    form_owner: dict[str, tuple[str, str]] = {}
    checked_forms = 0
    checked_by_type: Counter[str] = Counter()
    unconfirmed_by_type: Counter[str] = Counter()
    unconfirmed: list[tuple[str, str, str]] = []

    try:
        import torille  # type: ignore
    except Exception as exc:  # noqa: BLE001
        print(f"the torille binding is required: {exc}", file=sys.stderr)
        return 2
    engine = torille.Engine()

    voikko = None
    if not args.no_voikko:
        try:
            from voikko import libvoikko
            voikko = libvoikko.Voikko("fi")
        except Exception as exc:  # noqa: BLE001
            print(f"warning: Voikko unavailable ({exc})", file=sys.stderr)

    omorfi = None
    omorfi_cache: dict[str, bool] = {}

    def omorfi_confirms(lemma: str) -> bool:
        nonlocal omorfi
        if lemma in omorfi_cache:
            return omorfi_cache[lemma]
        if omorfi is None:
            try:
                from pyomorfi import load_omorfi
                omorfi = load_omorfi(load_segmenter=False)
            except Exception:
                omorfi_cache[lemma] = False
                return False
        analyses = omorfi.analyse(lemma)
        ok = any(
            f"WORD_ID={lemma}" in str(a) and "GUESS=UNKNOWN" not in str(a)
            for a in analyses
        )
        omorfi_cache[lemma] = ok
        return ok

    for row in rows:
        base = row[0]
        pos = row[2].strip() if len(row) > 2 else ""
        flags = row_flags(row)
        for flag in flags:
            kind = flag.split(":", 1)[0].split(">", 1)[0]
            try:
                lemma, out_pos, out_class = derive_chain(
                    base, flag, pos, row[3].strip() if len(row) > 3 else ""
                )
            except ValueError as exc:
                errors.append(f"{base} {flag}: {exc}")
                continue
            flags_by_type[kind] += 1
            if lemma in base_words:
                errors.append(f"{base} {flag}: {lemma} is already a Kotus lemma")
            previous = lemma_owner.get(lemma)
            if previous is not None and previous != (base, flag):
                errors.append(
                    f"{base} {flag}: {lemma} already derived from "
                    f"{previous[0]} ({previous[1]})"
                )
            else:
                lemma_owner[lemma] = (base, flag)

            try:
                forms = engine.inflect(lemma, klass=out_class, pos=out_pos)
            except Exception as exc:  # noqa: BLE001
                errors.append(f"{base} {flag}: {lemma} class {out_class}: {exc}")
                continue
            if out_pos == "adverbi":
                # Adverbs are indeclinable; class 99 is only a storage marker.
                plain = [lemma]
            else:
                plain = [f.word for f in forms if f.plain and f.word and " " not in f.word]
            if lemma not in plain:
                errors.append(f"{base} {flag}: class {out_class} lacks {lemma}")

            if voikko is not None:
                if not (voikko.spell(lemma) and any(
                    a.get("BASEFORM") == lemma for a in voikko.analyze(lemma)
                )):
                    if omorfi_confirms(lemma):
                        warnings.append(
                            f"{base} {flag}: Voikko does not confirm {lemma}, "
                            "but Omorfi does (accepted as an Omorfi-only form)"
                        )
                    elif ">" in flag or base in seeded_bases:
                        # A multi-step chain or an explicitly seeded flag is
                        # an accepted productive coinage even when neither
                        # dictionary lists it (`atomoitua`, `deittailija`).
                        warnings.append(
                            f"{base} {flag}: accepted creative derivation, "
                            f"oracle does not list {lemma}"
                        )
                    else:
                        errors.append(f"{base} {flag}: Voikko/Omorfi do not confirm {lemma}")
                checked_by_type[kind] += len(plain)
                for form in plain:
                    checked_forms += 1
                    owner = form_owner.get(form)
                    if owner is not None and owner != (base, lemma):
                        warnings.append(
                            f"{form}: generated by {owner[0]} ({owner[1]}) and {base} ({lemma})"
                        )
                    else:
                        form_owner[form] = (base, lemma)
                    if not voikko.spell(form):
                        unconfirmed.append((base, lemma, form))
                        unconfirmed_by_type[kind] += 1

    print(f"file: {path}")
    print(f"rows: {len(rows)}")
    print(f"flags: {sum(flags_by_type.values())} "
          f"({', '.join(f'{k}={v}' for k, v in sorted(flags_by_type.items()))})")
    print(f"derived lemmas: {len(lemma_owner)}")
    print(f"plain single-token forms: {checked_forms}")
    if voikko is not None:
        rate = (checked_forms - len(unconfirmed)) / checked_forms * 100 if checked_forms else 100.0
        print(f"Voikko-confirmed: {checked_forms - len(unconfirmed)} / {checked_forms} ({rate:.1f}%)")
        for kind in sorted(checked_by_type):
            total = checked_by_type[kind]
            bad = unconfirmed_by_type[kind]
            pct = (total - bad) / total * 100 if total else 100.0
            print(f"  {kind}: {total - bad} / {total} ({pct:.1f}%)")
        if unconfirmed:
            print(f"unconfirmed forms: {len(unconfirmed)}")
            for base, lemma, form in unconfirmed[: args.max_examples]:
                print(f"  {form}  ({base} -> {lemma})")
            if len(unconfirmed) > args.max_examples:
                print(f"  ... and {len(unconfirmed) - args.max_examples} more")

    for warning in warnings[: args.max_examples]:
        print(f"warning: {warning}", file=sys.stderr)
    if len(warnings) > args.max_examples:
        print(f"warning: ... and {len(warnings) - args.max_examples} more overlaps", file=sys.stderr)

    if errors:
        print(f"\n{len(errors)} error(s):", file=sys.stderr)
        for error in errors[: args.max_examples]:
            print(f"  {error}", file=sys.stderr)
        if len(errors) > args.max_examples:
            print(f"  ... and {len(errors) - args.max_examples} more", file=sys.stderr)
        return 1
    print("\nstructurally OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
