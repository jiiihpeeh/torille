#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Extract a start-to-end inflection rule for one class, from the database.

A class rule says, given a lemma, how to derive every form of its paradigm —
which stem each cell is built on, and which suffix the cell wants. It is
extracted from the database (which is Omorfi+Voikko's ground truth) rather
than hand-written, so it can be checked: apply the rule to every word the
database has for the class and see whether the rule reproduces the paradigm.

Each class gets its own file under `rules/`, so a defect is fixed by editing
one class's file rather than by disambiguating the shared engine. Two classes
whose rules are identical can then be combined into one rule serving both —
the "combine where accessible" step — and `rules/` is the audit trail for
which classes actually agree.

The rule is deliberately mechanical and transparent:

    ->  the stem rules, lemma -> stem per cell
    ->  the cells, (case, number, possessive) -> stem + suffix
        (+ gradation/harmony where the class has it)

A class with consonant gradation (27, 39, ...) adds a `grade` table, because
the stem is not always the lemma: `sukka` is `sukka` in the strong grade and
`suka` in the weak. The extraction can only infer a rule it has seen, so a
cell with no example in the database is reported rather than invented.

Usage:
    python3 tools/oracle/extract_class_rules.py -c 1            # class 1
    python3 tools/oracle/extract_class_rules.py -c all          # every numeric class
    python3 tools/oracle/extract_class_rules.py -c all --check  # also re-derive and compare
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DEFAULT_LEXICON = REPO / "data" / "kotus_nomines.sqlite"
RULES = REPO / "rules"

CASE_WORDS = ("nominative", "genitive", "partitive", "inessive", "elative",
              "illative", "adessive", "ablative", "allative", "essive",
              "translative", "instructive", "abessive")

#: Db case word -> the engine's `Case` variant. The two sides name the same
#: grammar in different words (`abessive` is both here and in the engine).
CASE_RUST = {
    "nominative": "Nominative", "genitive": "Genitive", "partitive": "Partitive",
    "inessive": "Inessive", "elative": "Elative", "illative": "Illative",
    "adessive": "Adessive", "ablative": "Ablative", "allative": "Allative",
    "essive": "Essive", "translative": "Translative",
    "instructive": "Instructive", "abessive": "Abessive", "comitative": "Comitative",
    "accusative": "Accusative", "prolative": "Prolative",
}
POSS_RUST = {
    "": None, "first person singular": "FirstSingular",
    "second person singular": "SecondSingular", "third person": "Third",
    "first person plural": "FirstPlural", "second person plural": "SecondPlural",
}
NUM_RUST = {"singular": "Singular", "plural": "Plural", "collective": "Collective"}


def cell_key(case: str, number: str, poss: str) -> str:
    return f"{case}|{number}|{poss}"


def load_paradigm(conn: sqlite3.Connection, lemma: str) -> dict[str, list[str]]:
    """Every confirmed form of `lemma`, keyed by the cell it answers."""
    out: dict[str, list[str]] = defaultdict(list)
    for case, number, poss, form in conn.execute(
        "SELECT [case], number, possessive, form FROM forms "
        "WHERE lemma=? AND ok='both'", (lemma,)):
        k = cell_key(case, number, poss)
        if form not in out[k]:
            out[k].append(form)
    return dict(out)


def infer_stem_on(form: str, lemma: str) -> str:
    """The implicit stem `form` is built on, by dropping a shared suffix.

    For the bare nominative this is the lemma itself. For other cases we cut
    the longest suffix that leaves a stem the lemma starts with — the same
    "the class cuts the lemma" idea torille uses, but read off a concrete
    form instead of hypothesised.
    """
    for cut in range(min(len(form), len(lemma)), -1, -1):
        if lemma[:cut] == form[:cut]:
            return form[:cut]
    return ""


def _harmony_norm(s: str) -> str:
    # Front/back twins `ä`/`a` and `ö`/`o` carry the same suffix; the two is a
    # harmony choice of the word, not a different ending, so the two are pooled
    # when a cell's shared suffix is being found.
    return s.replace("ä", "a").replace("ö", "o")


def _longest_common_suffix_len(strings: list[str]) -> int:
    """How long a shared suffix the forms of a cell carry.

    Within a Kotus class every word of a cell takes the same ending (the
    possessive paradigmed the same), so the longest suffix the forms share,
    after pooling the harmony vowels, is the cell's ending. The stem varies but
    the ending does not, which is what makes this the ending.
    """
    if not strings:
        return 0
    k = 0
    while all(len(s) > k and s[-1 - k] == strings[0][-1 - k] for s in strings):
        k += 1
    return k


def _common_suffix(words: set[str], others: set[str]) -> str:
    """A suffix shared by `words` (and no `others`) long enough to be a real
    stem group, `vastaus`/`ehdotus` -> `us`. Empty when the rarer cut cannot be
    told apart from the default by its ending, in which case the alternate would
    misfire and is not emitted. Two letters is the floor: a single `i` or `a`
    is everywhere and would route half a class to the wrong stem."""
    if len(words) < 2:
        return ""
    words = list(words)
    for length in range(2, len(words[0]) + 1):
        suf = words[0][-length:]
        if all(w.endswith(suf) for w in words) and not any(o.endswith(suf) for o in others):
            return suf
    return ""


def stem_transform(lemma: str, stem: str) -> tuple[int, str]:
    """The smallest `lemma` -> `stem` cut: `(drop, add)` where
    `stem == lemma[:len-drop] + add`. `käsi` -> `käde` is `(2, "de")` and a
    word with `stem == lemma` is `(0, "")`."""
    n = min(len(lemma), len(stem))
    p = 0
    while p < n and lemma[p] == stem[p]:
        p += 1
    # `p` is the longest shared prefix; cut the lemma after it.
    return len(lemma) - p, stem[p:]


def infer_rule(conn: sqlite3.Connection, cls: str, lemmas: list[str]) -> dict:
    """Infer every cell's stem-carrying rule from the class's words.

    For a cell, the class's words agree on the *ending*; the longest common
    suffix (harmony pooled) is it. Each word then reads `stem = form - ending`,
    and the lemma -> stem cut is read off that. The rule for the cell is the
    majority cut and the distinct endings.

    `rule[cell]` = {"drop": int, "add": str, "suf": [str...]}.
    A cell with no consistent reading (an irregular plural) is left out rather
    than invented.
    """
    cells: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for lem in lemmas:
        for case, number, poss, form in conn.execute(
                "SELECT [case], number, possessive, form FROM forms "
                "WHERE lemma=? AND ok='both'", (lem,)):
            if not form or not case:
                continue
            cells[cell_key(case, number, poss)].append((lem, form))

    VOWELS = "aeiouyäö"
    rule: dict[str, dict] = {}
    for key, pairs in cells.items():
        norm = [_harmony_norm(f) for _, f in pairs]
        ending = _longest_common_suffix_len(norm)
        seen: dict[tuple[int, str], dict] = defaultdict(
            lambda: {"suf": set(), "lemmas": set()})
        saw_double = True   # stem = lemma with its last vowel doubled
        suffixes: set[str] = set()
        for lem, form in pairs:
            chars = list(form)
            if ending == 0:
                stem, suf = form, ""
            else:
                if len(chars) < ending:
                    continue
                stem = "".join(chars[:-ending])
                suf = form[-ending:]
            if not stem:
                continue
            suffixes.add(suf)
            drop, add = stem_transform(lem, stem)
            seen[(drop, add)]["suf"].add(suf)
            seen[(drop, add)]["lemmas"].add(lem)
            # A cell that doubles the lemma's last vowel has no single `add`:
            # `äly -> älyy` but `talo -> taloo`. It is one *operation*, not a
            # constant, so it is marked and emitted as "append the last vowel".
            if not (drop == 0 and len(add) == 1 and add == (lem[-1] if lem else "")
                    and lem and lem[-1] in VOWELS and stem == lem + lem[-1]):
                saw_double = False
        if not seen:
            continue
        if saw_double:
            rule[key] = {"double": True, "suf": sorted(suffixes)[:2]}
            continue
        # The cut all the words of the class share; a word that disagrees is a
        # member of a different grade and shows up as a second, rarer cut. The
        # majority cut is the class's default; each rarer cut is an alternate
        # stem whose words are grouped by the lemma ending that licenses it --
        # class 39's `-us` words take `-us -> -ukse` while its model `ajos` does
        # not, so `vastaus` needs the alternate and `ajos` the default.
        best = max(seen, key=lambda g: len(seen[g]["lemmas"]))
        drop, add = best
        rule[key] = {"drop": drop, "add": add, "suf": sorted(seen[best]["suf"])}
        alts = {}
        all_default = seen[best]["lemmas"]
        for group, rec in seen.items():
            if group == best:
                continue
            trig = _common_suffix(rec["lemmas"], all_default)
            if trig:
                alts[trig] = {"drop": group[0], "add": group[1],
                              "suf": sorted(rec["suf"])}
        if alts:
            rule[key]["alt"] = alts
    return rule


def write_rule(cls: str, model: str, stem: str, cells: dict) -> dict:
    return {
        "class": cls,
        "model_word": model,
        "help": "cells are (case|number|possessive); each maps to the bare "
                "suffix the model word's stem takes, or 'multi' for a cell "
                "with two equal answers, or absent when unseen",
        "stem": stem,
        "cells": cells,
    }


def rust_ident(cls: str) -> str:
    """A class value to a Rust module file name: `9*B` -> `class_9B.rs`,
    `9, 50` -> `class_9_50.rs`. Needed because a class value is text, not an
    identifier, and the file has to be one.

    The gradation letter keeps its case: `class_9B` is the name the tree
    actually uses, and lowercasing it here (`class_9b`) would make a
    regeneration write a second, near-identical module beside the real one
    rather than replacing it."""
    parts = cls.split("*", 1)
    head = "".join(c if c.isalnum() else "_" for c in parts[0].lower())
    tail = "_".join(c if c.isalnum() else "_" for c in parts[1]) if len(parts) > 1 else ""
    return "class_" + head + tail


def emit_rust(cls: str, model: str, stem: str, cells: dict) -> str:
    """The Rust module that encodes `class`'s start-to-end rule.

    Generated, not written by hand: every `push` below comes from one confirmed
    form in the database. The module is self-contained -- it knows its class's
    suffixes and how to read each word -- so a class is fixed by regenerating
    or editing its own file, and two classes that agree can be collapsed into
    one module by a `use` of the other's `inflect`.
    """
    head = (
        f"//! Inflection rule for class {cls}, generated from the noun lexicon.\n"
        "//! Regenerate with:  python3 tools/oracle/extract_class_rules.py --rust\n"
        "//! do not edit by hand; the database is the source of truth.\n"
        "#![allow(unused_imports)]\n\n"
        "use crate::tags::{Case, Confidence, Form, Number, Possessive, Tags};\n\n"
        f"pub const CLASS: &str = \"{cls}\";\n"
        f"pub const MODEL: &str = \"{model}\";\n\n"
        "/// Every form of `lemma`, one `Form` per confirmed cell.\n"
        "///\n"
        "/// `possessives` is the caller's decision, taken from the part of\n"
        "/// speech: a noun takes possessive suffixes, an adjective or a\n"
        "/// participle does not. The cells below are read off the database, which\n"
        "/// records both, so the flag is what keeps an adjective from being\n"
        "/// offered a possessive column it cannot have.\n"
        "pub fn inflect(lemma: &str, possessives: bool) -> Vec<Form> {\n"
        "    let mut out = Vec::new();\n"
    )
    stem = (
        "    // `stem` cuts the class's stem out of the lemma: `(drop, add)`\n"
        "    // makes `lemma[:len-drop] + add`. A non-graded class is `(0, \"\")`,\n"
        "    // so `stem` is the lemma itself; class 27 is `käsi -> käde`.\n"
        "    let stem = |lemma: &str, drop: usize, add: &str| -> String {\n"
        "        let chars: Vec<char> = lemma.chars().collect();\n"
        "        let keep = chars.len().saturating_sub(drop);\n"
        "        let mut s: String = chars[..keep].iter().collect();\n"
        "        s.push_str(add);\n"
        "        s\n"
        "    };\n"
        "    // `double` appends the lemma's last vowel: `äly -> älyy`, the\n"
        "    // illative (and a few others) doubling their stem's final vowel.\n"
        "    let double = |lemma: &str| -> String {\n"
        "        let mut s = lemma.to_string();\n"
        "        if let Some(v) = lemma.chars().last() {\n"
        "            s.push(v);\n"
        "        }\n"
        "        s\n"
        "    };\n"
        "    // Harmony: a cell of a class holds both the back (`a`/`o`) and the\n"
        "    // front (`ä`/`ö`) spelling of its ending; the lemma's own vowels\n"
        "    // pick the one. A word with a back vowel is back, one with only\n"
        "    // `e`/`i` or a front vowel is front, matching the engine's reading.\n"
        "    let front = |lemma: &str| -> bool {\n"
        "        // Finnish harmony is set by the *last* non-neutral vowel --\n"
        "        // `a/ä`, `o/ö`, `u/y` -- scanning right to left; `e`/`i` are\n"
        "        // neutral and pass. This is the real rule, and it settles the\n"
        "        // compound on its last constituent by itself: `hätähousu` is\n"
        "        // back on its `u` though its modifier is front.\n"
        "        for c in lemma.chars().rev() {\n"
        "            match c {\n"
        "                'ä'|'ö'|'y'|'Ä'|'Ö'|'Y' => return true,\n"
        "                'a'|'o'|'u'|'A'|'O'|'U' => return false,\n"
        "                _ => {}\n"
        "            }\n"
        "        }\n"
        "        true   // only neutral vowels; read front, as convention does\n"
        "    };\n"
        "    // `harmonize` fixes a spelling to the front or back half: `ssani`\n"
        "    // in the back half is `ssäni` in the front. A and O have front\n"
        "    // twins (Ä, Ö); a spelling already on the right half is unchanged.\n"
        "    let harmonize = |s: &str, front: bool| -> String {\n"
        "        if !front { return s.to_string(); }\n"
        "        s.chars().map(|c| match c { 'a' => 'ä', 'o' => 'ö',\n"
        "                                         'A' => 'Ä', 'O' => 'Ö', _ => c }).collect()\n"
        "    };\n"
    )
    push = (
        "    let mut push = |case: Case, num: Number, poss: Option<Possessive>, word: String| {\n"
        "        // Every confirmed cell is pushed through here, so this is the one\n"
        "        // place the flag can be honoured: a bare form is always offered,\n"
        "        // and an owned one only when the word can take the suffix.\n"
        "        if poss.is_some() && !possessives {\n"
        "            return;\n"
        "        }\n"
        "        let tags = match poss {\n"
        "            Some(p) => Tags::of_number(case, num).possessed_by(p),\n"
        "            None => Tags::of_number(case, num),\n"
        "        };\n"
        "        out.push(Form::new(word, tags).with(Confidence::Plain));\n"
        "    };\n"
    )
    body = []

    def esc(s: str) -> str:
        return "".join(ch if ch != '"' else '\\"' for ch in s)

    def stem_basis(cell2: dict, trig: str | None = None) -> str:
        def addr(add: str) -> str:
            a = esc(add)
            if any(v in add for v in "ao") and not any(v in add for v in "äö"):
                return f"&harmonize(\"{a}\", front(lemma))"
            return f"\"{a}\""
        if cell2.get("double") and trig is None:
            return "double(lemma)"
        if trig is not None:
            g = cell2["alt"][trig]
            if g.get("double"):
                return "double(lemma)"
            return f"stem(lemma, {g['drop']}, {addr(g['add'])})"
        return f"stem(lemma, {cell2['drop']}, {addr(cell2['add'])})"

    def split_harmony(sufs: list[str]) -> tuple[list[str], list[str], list[str]]:
        back = [s for s in sufs if any(v in s for v in "ao") and not any(v in s for v in "äö")]
        front = [s for s in sufs if any(v in s for v in "äö")]
        neutral = [s for s in sufs if not any(v in s for v in "aoäö")]
        return neutral, back, front

    def front_twin_of(s: str) -> str:
        return s.replace("a", "ä").replace("o", "ö").replace("A", "Ä").replace("O", "Ö")

    def arm(sufs: list[str], base: str, ind: str) -> list[str]:
        # A cell's suffix set is usually a harmony pair (`issa`/`issä`), which
        # harmonize collapses to a single push. Multi-spelling cells (genuine
        # alternatives like `-jen`/`-iden`) have neither twin and emit as-is.
        neu, back, front = split_harmony(sufs)
        if front and back and set(front) == {front_twin_of(x) for x in back}:
            lines = [f"{ind}push(Case::{c}, Number::{n}, {tag}, "
                     f"format!(\"{{}}{{}}\", {base}, \"{esc(s)}\"));" for s in neu]
            lines += [f"{ind}push(Case::{c}, Number::{n}, {tag}, "
                      f"format!(\"{{}}{{}}\", {base}, "
                      f"&harmonize(\"{esc(s)}\", front(lemma))));" for s in back]
            return lines
        if front and back and set(front) != set(back):
            # A harmonic cell whose twins do not collapse cleanly; keep both.
            lines = [f"{ind}push(Case::{c}, Number::{n}, {tag}, "
                     f"format!(\"{{}}{{}}\", {base}, \"{esc(s)}\"));" for s in neu]
            lines.append(f"{ind}if front(lemma) {{")
            lines += [f"{ind}    push(Case::{c}, Number::{n}, {tag}, "
                      f"format!(\"{{}}{{}}\", {base}, \"{esc(s)}\"));" for s in front]
            lines.append(f"{ind}}} else {{")
            lines += [f"{ind}    push(Case::{c}, Number::{n}, {tag}, "
                      f"format!(\"{{}}{{}}\", {base}, \"{esc(s)}\"));" for s in back]
            lines.append(f"{ind}}}")
            return lines
        return [f"{ind}push(Case::{c}, Number::{n}, {tag}, "
                f"format!(\"{{}}{{}}\", {base}, \"{esc(s)}\"));" for s in sufs]

    for key, cell in sorted(cells.items()):
        case, number, poss = key.split("|")
        c = CASE_RUST.get(case)
        n = NUM_RUST.get(number)
        po = POSS_RUST.get(poss)
        sufs = cell["suf"]
        if c is None or n is None or not sufs:
            continue
        tag = f"Some(Possessive::{po})" if po is not None else "None"
        alts = cell.get("alt")
        if not alts:
            body += arm(sufs, stem_basis(cell), "    ")
            continue
        # A cell with stem alternates: a lemma whose ending licenses an
        # alternate stem takes it, otherwise the class default. Longest trigger
        # first.
        for i, (trig, g) in enumerate(sorted(alts.items(), key=lambda kv: -len(kv[0]))):
            body.append(f"    {'if' if i == 0 else '} else if'} lemma.ends_with(\"{esc(trig)}\") {{")
            body += arm(g["suf"], stem_basis(cell, trig), "        ")
        body.append("    } else {")
        body += arm(sufs, stem_basis(cell), "        ")
        body.append("    }")
    tail = "    drop(stem);\n    drop(double);\n    drop(front);\n    drop(harmonize);\n    drop(push);\n    out\n}\n"
    return head + stem + push + "\n".join(body) + "\n" + tail


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("-c", "--classes", default="numeric",
                   help="comma list, 'numeric' (default), or 'all'")
    p.add_argument("--lexicon", default=str(DEFAULT_LEXICON))
    p.add_argument("--check", action="store_true",
                   help="re-derive every word in the class and report how "
                        "many forms the rule reproduces")
    p.add_argument("--rust", metavar="DIR", nargs="?", const="target/class-rules",
                   help="also emit the rule as a Rust module per class into DIR "
                        "(default target/class-rules), one file per class. This "
                        "is a scratch dump for inspecting a rule, not a source "
                        "generator: it writes one file per *raw* sanalista class "
                        "value, so `(1)`, `10, (11)` and `9*(D)` -- labels the "
                        "parser never produces -- would come back as "
                        "`class__1_.rs` and clobber the hand-written `mod.rs`. "
                        "Keep it out of src/.")
    p.add_argument("--out", default=str(RULES))
    args = p.parse_args(argv)

    conn = sqlite3.connect(args.lexicon)
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    if args.classes == "numeric":
        classes = sorted({r[0] for r in conn.execute("SELECT DISTINCT class FROM base") if r[0].isdigit()},
                         key=int)
    elif args.classes == "all":
        classes = sorted({r[0] for r in conn.execute("SELECT DISTINCT class FROM base")})
    else:
        wanted = {c.strip() for c in args.classes.split(",") if c.strip()}
        classes = [c for c in sorted({r[0] for r in conn.execute("SELECT DISTINCT class FROM base")}) if c in wanted]

    identifiers: dict[str, str] = {}

    for cls in classes:
        # Model word: the shortest lemma that inflects, the most regular look.
        lemmas = [r[0] for r in conn.execute(
            "SELECT lemma FROM base WHERE class=? AND homonym=''", (cls,))]
        if not lemmas:
            continue
        # Model word: the shortest lemma that has a confirmed paradigm in the
        # database. The very shortest is often an abbreviation or a loan the
        # oracles do not know (`ELY`), which has no paradigm to read a rule off;
        # the rule wants a real, inflecting word.
        model = None
        paradigm = {}
        for candidate in sorted(lemmas, key=lambda w: len(w)):
            paradigm = load_paradigm(conn, candidate)
            if paradigm:
                model = candidate
                break
        if model is None:
            continue
        cells = infer_rule(conn, cls, lemmas)
        stem = paradigm.get(cell_key("nominative", "singular", ""))
        rule = write_rule(cls, model, stem[0] if stem else model, cells)

        # --check: how many forms in the whole class are the lemma plus a
        # recorded suffix — the share a plain (non-graded, non-stem-changing)
        # rule already reproduces. Graded classes such as 27 are expected to be
        # low here; that is what the `grade` step (not built yet) is for.
        if args.check:
            reproduced = total = 0
            for lem in lemmas:
                for case, number, poss, form in conn.execute(
                        "SELECT [case], number, possessive, form FROM forms "
                        "WHERE lemma=? AND ok='both'", (lem,)):
                    total += 1
                    cell = cells.get(cell_key(case, number, poss))
                    if not cell:
                        continue
                    # An alternate stem (keyed by the lemma's ending) overrides
                    # the class default for the words it licenses.
                    trig = next((t for t in cell.get("alt", {}) if lem.endswith(t)), None)
                    g = cell["alt"][trig] if trig else cell
                    if g.get("double"):
                        stem = lem + (lem[-1] if lem else "")
                    else:
                        keep = len(lem) - g["drop"]
                        stem = "".join(list(lem)[:keep]) + g["add"]
                    tail = form[len(stem):]
                    # Harmony: only the ending agreeing with the lemma's side
                    # of the a/ä (o/ö) split is the form the cell wants.
                    def matches(s):
                        neu = not any(v in s for v in "aoäö")
                        if neu:
                            return tail == s
                        isfront = any(v in lem for v in "äöy") or not any(v in lem for v in "aou")
                        front = any(v in s for v in "äö")
                        return tail == s and (front == isfront)
                    if form.startswith(stem) and any(matches(s) for s in g["suf"]):
                        reproduced += 1
            rule["_check"] = {
                "words": len(lemmas),
                "forms_total": total,
                "reproduced": reproduced,
                "cover": round(reproduced / max(1, total), 3),
            }

        target = out_dir / f"class-{cls}.json"
        target.write_text(json.dumps(rule, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        if args.rust is not None:
            rs_dir = Path(args.rust)
            rs_dir.mkdir(parents=True, exist_ok=True)
            (rs_dir / f"{rust_ident(cls)}.rs").write_text(
                emit_rust(cls, model, stem, cells), encoding="utf-8")
            identifiers[rust_ident(cls)] = cls
        m = len([c for c in rule["cells"].values() if c])
        cell = "" if not args.check else f"  check {rule['_check']}"
        print(f"  class {cls:>4}  model {model:16} {m:3} cells seen{cell}")

    if args.rust is not None:
        from textwrap import dedent
        rs_dir = Path(args.rust)
        decls = [f"pub mod {ident};" for ident in sorted(identifiers)]
        # The dispatcher: class value -> the module that inflects it. Two
        # classes often share a rule; the map keeps them both pointing at the
        # same generated module, which is the "combine where accessible" step
        # made concrete in Rust.
        arms = []
        for ident, cls in sorted(identifiers.items(), key=lambda kv: kv[1]):
            arms.append(f'        "{cls}" => Some(class_{ident.split("_", 1)[1]}::inflect(lemma)),')
        dispatch = (
            "/// Route a lemma to its class's generated module, by the class\n"
            "/// value the sanalista records (`9` and `9*B` are two keys).\n"
            "pub fn dispatch(class: &str, lemma: &str) -> Option<Vec<Form>> {\n"
            "    match class {\n"
            + "\n".join(arms)
            + '\n        _ => None,\n    }\n}\n'
        )
        test = (
            "\n#[cfg(test)]\nmod tests {\n"
            "    use super::{class_1, dispatch};\n"
            '    use crate::tags::Confidence;\n\n'
            "    // The generated rule was read off the model word `äly`, so it\n"
            "    // reproduces `äly`'s paradigm, and dispatch routes a class\n"
            "    // value to it. Reproducing *other* words of the class needs the\n"
            "    // harmony step (a/ä, o/ö), which is the next piece.\n"
            "    #[test]\n"
            "    fn class_1_reproduces_its_model() {\n"
            '        let forms = class_1::inflect("äly");\n'
            "        let words: std::collections::HashSet<String> =\n"
            "            forms.iter().map(|f| f.word.clone()).collect();\n"
            '        for want in ["äly", "älyn", "älyä", "älyyn", "älyssä", "älyksi",\n'
            '                     "älyt", "älyjen", "älyihin", "älyni", "älyssäni"] {\n'
            '            assert!(words.contains(want), "class 1 model missing {want}");\n'
            "        }\n"
            "        for f in forms.iter().filter(|f| f.word == \"älyksi\") {\n"
            "            assert_eq!(f.confidence, Confidence::Plain);\n"
            "        }\n"
            "    }\n"
            "\n"
            "    #[test]\n"
            "    fn dispatch_routes_a_class_to_its_module() {\n"
            "        // class 1 routes to class_1; an unknown class is None.\n"
            '        let via = dispatch("1", "talo");\n'
            "        assert!(via.is_some(), \"dispatch(\\\"1\\\", talo) should route\");\n"
            '        assert!(dispatch("9999", "talo").is_none());\n'
            "        assert!(dispatch(\"27\", \"käsi\").is_some());\n"
            "    }\n"
            "}\n"
        )
        (rs_dir / "mod.rs").write_text(
            dedent("""\
            //! Generated per-class inflection rules.
            //!
            //! Produced by tools/oracle/extract_class_rules.py --rust from the noun
            //! lexicon. do not edit by hand; regenerate.
            #![allow(dead_code)]

            use crate::tags::Form;
            """)
            + "\n".join(decls)
            + "\n\n"
            + dispatch
            + test,
            encoding="utf-8",
        )

    # The combine step: identical rules collapse to one file, the audit trail
    # of which classes actually agree.
    groups = defaultdict(list)
    for rule_file in sorted(out_dir.glob("class-*.json")):
        r = json.load(open(rule_file))
        # The combine key is the rule body alone -- the stem and the cells.
        # The model word and the per-class check numbers differ for a good
        # reason and must not stand in the way of two classes agreeing.
        key = json.dumps({k: r[k] for k in ("stem", "cells")},
                         ensure_ascii=False, sort_keys=True)
        groups[key].append(r["class"])
    merged = sorted(",".join(v) for v in groups.values() if len(v) > 1)
    if merged:
        print(f"  rule body shared by {len(merged)} group(s), combinable:")
        for g in merged[:20]:
            print(f"     {g}")
    return 0


if __name__ == "__main__":
    sys.exit(main())