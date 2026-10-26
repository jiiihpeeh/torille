#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Classify a Finnish lemma into a Kotus inflection class.

Given a *base form* (a lemma), this asks **Omorfi** to generate its paradigm,
confirms each generated form with **Voikko**, and then asks which Kotus class
expression (`1`, `9*B`, `38`, ...) makes the torille engine agree with that
confirmed paradigm best. Agreement is compared **cell by cell** (case/number,
or mood/tense/person/number for verbs), so the engine's extra forms -- the
possessive and degree variants it also emits -- do not drown the signal. The
class with the highest cell agreement wins.

The two oracles are the ground truth and the engine is the hypothesis, which is
the reverse of the build pipeline: that one starts from a class and generates.

Usage:
    python3 tools/classify_lemma.py kissa talo vesi
    python3 tools/classify_lemma.py kissan tahkoisella   # inflected is fine
    python3 tools/classify_lemma.py --pos verb juosta
    python3 tools/classify_lemma.py --top 5 --json plutokratia

Requires the torille binding (`cd bindings && env -u CONDA_PREFIX maturin
develop --release`) plus Omorfi and Voikko.

Notes:
  * The input may be any form: Voikko lemmatises it to a base first
    (`kissan` -> `kissa`, `tahkoisella` -> `tahkoinen`).
  * A word already in the sanalista prints its recorded class first; the
    classification is then a cross-check.
  * The possessive grid is on by default (it discriminates classes that
    differ only there); `--no-possessives` asks only the bare case/number
    grid, which is 6x cheaper but ties more classes.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from omorfi_harness import (  # noqa: E402
    ALL_CASES,
    ALL_POSSESSIVES,
    CASE_NAMES,
    CONFIDENCES,
    Harness,
    inflect_binary,
    noun_cells,
    verb_cells,
)

POS_WORDS = {
    "NOUN": "substantiivi",
    "PROPN": "substantiivi",
    "ADJ": "adjektiivi",
    "NUM": "numeraali",
    "PRON": "pronomini",
    "VERB": "verbi",
}
NOUN_POS = ("substantiivi", "adjektiivi", "numeraali", "pronomini")

NUMBERS = ["SG", "PL"]
#: Every case, including the plural-only comitative and the prolative, whose
#: shapes discriminate classes the core six cases do not.
GRID_CASES = ALL_CASES + ["COM", "PRO"]
VERB_TENSE_MOODS = [("PRESENT", "INDV"), ("PAST", "INDV"), ("", "COND")]

#: torille's name -> Omorfi's key, for reading the engine's forms back into the
#: same cells the oracle was asked for.
INV_CASE = {v: k for k, v in CASE_NAMES.items()}
INV_NUMBER = {"singular": "SG", "plural": "PL"}
INV_POSSESSIVE = {"1sg": "SG1", "2sg": "SG2", "3": "3", "1pl": "PL1", "2pl": "PL2"}
INV_MOOD = {"indicative": "INDV", "conditional": "COND"}
INV_TENSE = {"present": "PRESENT", "past": "PAST"}
INV_PERSON = {"first": "1", "second": "2", "third": "3"}


def omorfi_pos(harness: Harness, word: str) -> str | None:
    for raw in harness.analyse_raw(word):
        upos = _bit(raw, "UPOS")
        if upos and upos in POS_WORDS:
            return POS_WORDS[upos]
    return None


def _bit(raw: str, name: str) -> str | None:
    start = raw.find(f"[{name}=")
    if start < 0:
        return None
    end = raw.find("]", start)
    return raw[start + len(name) + 2 : end] if end > start else None


def lemma_of(harness: Harness, engine, word: str) -> str:
    """The base form of `word`, via Voikko (and Omorfi as a fallback).

    An inflected input (`kissan`, `tahkoisella`) is lemmatised back to its base
    (`kissa`, `tahkoinen`) before classification. Voikko may offer several
    baseforms (a participle is lemmatised to the participle *and*, sometimes,
    the whole surface); prefer one the sanalista knows, else the shortest.
    Voikko's analyser is finite and sometimes reads a surface it cannot
    analyse, so Omorfi's lemmas are tried next.
    """
    with harness._lock:
        analyses = harness.voikko.analyze(word)
    candidates = [a.get("BASEFORM") for a in analyses if a.get("BASEFORM")]
    if harness.omorfi is not None:
        with harness._lock:
            omorfi = harness.omorfi.analyse(word)
        for a in omorfi:
            for lemma in getattr(a, "lemmas", None) or []:
                if lemma and lemma != word and lemma not in candidates:
                    candidates.append(lemma)
    candidates = [c for c in candidates if c]
    if not candidates:
        return word
    for base in candidates:
        if engine.lookup(base):
            return base
    return min(candidates, key=len)


def confirmed_cells(
    harness: Harness, base: str, pos: str, possessive_grid: bool
) -> dict[tuple, set[str]]:
    """`cell -> {form}`, for every cell Omorfi generates and Voikko reads."""
    if pos in NOUN_POS:
        harness.pos = "noun"
        poss = ALL_POSSESSIVES if possessive_grid else [""]
        cells = noun_cells(GRID_CASES, NUMBERS, poss)
    else:
        harness.pos = "verb"
        cells = verb_cells(VERB_TENSE_MOODS, NUMBERS)

    out: dict[tuple, set[str]] = {}
    for cell in cells:
        generated = harness.generate(base, cell)
        if not generated:
            continue
        key = (cell.case, cell.number, cell.possessive, cell.mood, cell.tense,
               cell.person)
        for spelling in generated.split("/"):
            form = spelling.strip()
            if form and harness.voikko_ok(base, form):
                out.setdefault(key, set()).add(form)
    return out


def cells_of_forms(forms) -> dict[tuple, set[str]]:
    """`cell -> {form}` from form objects (binding `PyForm` or `TorilleForm`)."""
    out: dict[tuple, set[str]] = {}
    for f in forms:
        key = (
            INV_CASE.get(f.case or "", ""),
            INV_NUMBER.get(f.number or "", ""),
            INV_POSSESSIVE.get(f.possessive or "", ""),
            INV_MOOD.get(f.mood or "", ""),
            INV_TENSE.get(f.tense or "", ""),
            INV_PERSON.get(f.person or "", ""),
        )
        if not key[0] and not key[3]:
            continue
        out.setdefault(key, set()).add(f.word)
    return out


def engine_cells(engine, base: str, klass: str, pos: str) -> dict[tuple, set[str]]:
    return cells_of_forms(engine.inflect(base, klass=klass, pos=pos))


def binary_cells(binary: str, base: str, klass: str, pos: str) -> dict[tuple, set[str]]:
    """The same, from the `inflect` binary.

    The binding reports `case=None` on possessive forms (their readings
    disagree on the case), so the possessive grid is unusable through it. The
    binary's printed reading keeps the case, and `_form_from_tags` recovers it.
    """
    out = subprocess.run([binary, base, klass, pos],
                         capture_output=True, text=True).stdout
    forms = []
    for line in out.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or " forms; " in line:
            continue
        tokens = line.split()
        for i, token in enumerate(tokens):
            if token in CONFIDENCES:
                forms.append(Harness._form_from_tags(
                    " ".join(tokens[:i]), token, tokens[i + 1:]))
                break
    return cells_of_forms(forms)


def candidate_classes(engine, pos: str) -> list[str]:
    seen: set[str] = set()
    for _lemma, klass in engine.words(pos):
        for one in klass.split(","):
            one = one.strip()
            # 50/51 are compound markers, not inflection classes: the engine
            # inflects by head and over-generates, which would win every tie.
            if one and one.split("*")[0] not in ("50", "51"):
                seen.add(one)
    return sorted(seen)


def agreement(confirmed: dict[tuple, set[str]],
              generated: dict[tuple, set[str]]) -> tuple[float, float, float, int]:
    total = len(confirmed)
    if total == 0:
        return (0.0, 0.0, 0.0, 0)
    correct = sum(
        1 for cell, forms in confirmed.items() if generated.get(cell, set()) & forms
    )
    recall = correct / total
    emitted = len(generated)
    precision = correct / emitted if emitted else 0.0
    f1 = (2 * precision * recall / (precision + recall)) if (precision + recall) else 0.0
    return (f1, precision, recall, correct)


def class_number(klass: str) -> int:
    m = re.match(r"\d+", klass)
    return int(m.group()) if m else 0


def classify(harness: Harness, engine, base: str, pos: str | None,
             possessive_grid: bool, top: int, binary: str | None = None) -> dict:
    recorded = engine.lookup(base)
    if pos is None:
        pos = recorded[0][1].split(",")[0].strip() if recorded else None
    if pos is None:
        pos = omorfi_pos(harness, base)
    if pos is None:
        return {"word": base, "recorded": recorded,
                "error": "could not decide the part of speech"}

    confirmed = confirmed_cells(harness, base, pos, possessive_grid)
    if not confirmed:
        return {"word": base, "pos": pos, "recorded": recorded,
                "error": "Omorfi/Voikko confirm no forms"}

    results = []
    cache: dict[str, dict[tuple, set[str]]] = {}
    for klass in candidate_classes(engine, pos):
        try:
            if binary:
                generated = binary_cells(binary, base, klass, pos)
            else:
                generated = engine_cells(engine, base, klass, pos)
        except Exception:
            continue
        cache[klass] = generated
        f1, precision, recall, correct = agreement(confirmed, generated)
        results.append({"class": klass, "f1": f1, "precision": precision,
                        "recall": recall, "correct": correct,
                        "cells": len(generated)})
    results.sort(key=lambda r: (-r["correct"], class_number(r["class"]), r["class"]))
    best = results[:top]

    missing: list[str] = []
    extra: list[str] = []
    if best and best[0]["correct"]:
        gen = cache[best[0]["class"]]
        for cell, forms in confirmed.items():
            if not (gen.get(cell, set()) & forms):
                missing.append("/".join(x for x in cell if x) + "=" + "/".join(sorted(forms)))
        for cell, forms in gen.items():
            if cell in confirmed and not (forms & confirmed[cell]):
                extra.append("/".join(x for x in cell if x) + "=" + "/".join(sorted(forms)))
    return {
        "word": base,
        "pos": pos,
        "recorded": recorded,
        "confirmed_cells": len(confirmed),
        "best": best,
        "missing_examples": sorted(missing)[:10],
        "extra_examples": sorted(extra)[:10],
    }


def report(harness, engine, binary, sample, seed, pos_sel, top, possessive_grid) -> None:
    """Classify a random sample of sanalista lemmas and report per class."""
    import random

    rng = random.Random(seed)
    pool = engine.words(pos_sel)
    rng.shuffle(pool)
    pool = pool[:sample]

    def base_num(cls: str) -> str:
        m = re.match(r"\d+", cls)
        return m.group() if m else cls

    n = exact = base1 = topk = 0
    per: dict[str, list[int]] = {}
    for lemma, klass in pool:
        rec = base_num(klass.split(",")[0].strip())
        r = classify(harness, engine, lemma, None, possessive_grid, top, binary)
        if "error" in r:
            continue
        n += 1
        best = [b["class"] for b in r["best"]]
        b0 = base_num(best[0]) if best else ""
        exact += (best and best[0] == klass.split(",")[0].strip())
        base1 += (b0 == rec)
        topk += any(base_num(b) == rec for b in best)
        u = per.setdefault(rec, [0, 0, 0])
        u[0] += 1
        u[1] += (b0 == rec)
        u[2] += any(base_num(b) == rec for b in best)

    print(f"sample: {n} lemmas, pos={pos_sel}, top={top}, "
          f"possessives={'on' if possessive_grid else 'off'}")
    if n:
        print(f"  exact top-1:       {exact}/{n} = {100*exact/n:.1f}%")
        print(f"  class-number top-1:{base1}/{n} = {100*base1/n:.1f}%")
        print(f"  class-number top-{top}:{topk}/{n} = {100*topk/n:.1f}%")
    print("  per recorded class:")
    for cls in sorted(per, key=class_number):
        c, h1, hk = per[cls]
        print(f"    {cls:>4}  n={c:<4} top-1 {100*h1/c:5.1f}%  top-{top} {100*hk/c:5.1f}%")


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("words", nargs="*", help="form(s) to classify")
    ap.add_argument("--pos", default=None,
                    help="force the part of speech (e.g. substantiivi, verbi)")
    ap.add_argument("--top", type=int, default=5, help="how many classes to show")
    ap.add_argument("--possessives", action="store_true",
                    help="include the possessive cells (uses the inflect binary)")
    ap.add_argument("--sample", type=int, default=0,
                    help="classify a random sample of N sanalista lemmas and report")
    ap.add_argument("--seed", type=int, default=1, help="sample seed")
    ap.add_argument("--sample-pos", default="substantiivi",
                    help="part of speech to sample (default substantiivi)")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    args = ap.parse_args()

    if not args.words and not args.sample:
        ap.error("give one or more words, or --sample N")

    try:
        from torille import Engine
    except ImportError:
        print("the torille binding is not built:\n"
              "  (cd bindings && env -u CONDA_PREFIX maturin develop --release)",
              file=sys.stderr)
        return 2

    harness = Harness(pos="noun")
    engine = harness.engine or Engine()
    binary = inflect_binary() if args.possessives else None

    if args.sample:
        report(harness, engine, binary, args.sample, args.seed, args.sample_pos,
               args.top, args.possessives)
        return 0

    out = []
    for word in args.words:
        base = lemma_of(harness, engine, word)
        r = classify(harness, engine, base, args.pos, args.possessives, args.top,
                     binary)
        r["input"] = word
        out.append(r)

    if args.json:
        print(json.dumps(out, ensure_ascii=False, indent=2))
        return 0

    for r in out:
        if r.get("input") and r["input"] != r["word"]:
            print(f"\n{r['input']}  (lemma: {r['word']})")
        else:
            print(f"\n{r['word']}")
        if "error" in r:
            print(f"  ! {r['error']}")
            continue
        if r["recorded"]:
            print("  sanalista: " + ", ".join(f"{c} ({p})" for c, p in r["recorded"]))
        recorded = {c for c, _ in r["recorded"]}
        print(f"  pos={r['pos']}  confirmed cells={r['confirmed_cells']}")
        for i, b in enumerate(r["best"]):
            mark = "->" if i == 0 else "  "
            star = "*" if b["class"] in recorded else " "
            print(f"  {mark}{star}class {b['class']:>8}  cells={b['correct']}"
                  f"/{r['confirmed_cells']}  (P={b['precision']:.3f} "
                  f"R={b['recall']:.3f}, {b['cells']} emitted)")
        if r["missing_examples"]:
            print("     missing: " + "  ".join(r["missing_examples"]))
        if r["extra_examples"]:
            print("     extra:   " + "  ".join(r["extra_examples"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
