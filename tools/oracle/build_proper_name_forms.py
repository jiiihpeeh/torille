#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate the singular case paradigm of every name in `data/proper_names.csv`.

Wikidata supplies the nominative. Omorfi already has a `PROPN` lexicon and can
generate the cases (`Helsinki` -> `Helsingin`, `Helsingissä`, ...), so each name
is expanded to the cases a running Finnish text uses. Voikko is *not* used as a
filter here: it covers fewer names than Wikidata, and a rejected form would
silently drop a real name. The output is a `(form, name)` table, one row per
unique surface form.

The readme/docs are explicit that proper names are accepted, never suggested;
the form table is a fast-path accept list, not a lemma source.

Usage:
    python3 tools/oracle/build_proper_name_forms.py
    python3 tools/oracle/build_proper_name_forms.py --limit 100 --dry-run
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
NAMES = REPO / "data" / "proper_names.csv"
OUT = REPO / "data" / "proper_name_forms.csv"

sys.path.insert(0, str(REPO / "tools"))

#: Singular cases only. Plural proper names (`Helsingit`) are rare and the
#: plural paradigms add roughly as much data as the singular ones.
CASES = ["NOM", "GEN", "PAR", "ILL", "INE", "ELA", "ADE", "ABL", "ALL", "ESS", "TRA", "ABE"]


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--names", default=str(NAMES))
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    from build_nomine_lexicon import noun_cells
    from omorfi_harness import Harness

    names = []
    for line in Path(args.names).read_text(encoding="utf-8").splitlines():
        if "\t" not in line:
            continue
        name, _kind = line.split("\t", 1)
        if name:
            names.append(name)
    if args.limit:
        names = names[: args.limit]

    # Hand-maintained forms for names Omorfi does not know (Moomins,
    # Kanteletar). They live in a separate file so a Wikidata/Omorfi
    # refresh cannot silently drop them.
    forms: dict[str, str] = {}
    extra = REPO / "data" / "proper_name_forms_extra.csv"
    if extra.exists():
        for line in extra.read_text(encoding="utf-8").splitlines():
            if "\t" not in line:
                continue
            form, name = line.split("\t", 1)
            if form and name:
                forms.setdefault(form.lower(), name)

    harness = Harness("noun")
    cells = noun_cells(CASES, ["SG"], [""])
    skipped = 0
    for n, name in enumerate(names, 1):
        # The nominative is always accepted, even for a name Omorfi has no
        # paradigm for. Everything is stored lowercase because the typing
        # path lowercases before probing.
        forms.setdefault(name.lower(), name)
        got_any = False
        for cell in cells:
            generated = harness.generate(name, cell)
            if not generated:
                continue
            got_any = True
            for form in generated.split("/"):
                form = form.replace("\u2019", "").strip()
                if form:
                    forms.setdefault(form.lower(), name)
        if not got_any:
            skipped += 1
        if n % 2000 == 0:
            print(f"  {n}/{len(names)} names, {len(forms)} forms", file=sys.stderr)

    print(
        f"{len(names)} names -> {len(forms)} unique forms "
        f"({skipped} names with no paradigm)",
        file=sys.stderr,
    )
    if args.dry_run:
        for form, name in list(sorted(forms.items()))[:20]:
            print(f"{form}\t{name}")
        return 0

    with open(args.out, "w", encoding="utf-8") as f:
        for form, name in sorted(forms.items()):
            f.write(f"{form}\t{name}\n")
    print(f"wrote {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
