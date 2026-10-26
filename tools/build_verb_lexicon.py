#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Materialise every verb form Omorfi generates and Voikko confirms.

This is the verb half of what `build_nomine_lexicon.py` does for noun-likes. It
asks the same two oracles the same question -- **Omorfi** which forms exist,
**Voikko** which of those are real -- and writes the answer to
`data/kotus_verbit.sqlite`, so a later harness pass reads `ok='both'` back with
a query instead of loading two C++ libraries and generating the paradigm again.

Where a noun's cell is `(case, number, possessive)`, a verb's is the reading:
voice, mood, tense, person, number, and, for the non-finite half, the
infinitive or participle plus the case it declines in. The grid is therefore
not one rectangle. It is every slice below, and it is deliberately the *whole*
verb: the finite active and passive in all four moods, the connegatives the
negative auxiliary takes, the five infinitives, the participles declined
through the full nominal case/number/possessive grid, and the analytic
perfects and negatives (`olen puhunut`, `en ole puhunut`, `ei puhuta`).

## The two tables

Exactly two, as for nouns: a reference row per word and a row per form per
cell.

```
base:   lemma │ homonym │ part_of_speech │ class              (one row per word)
forms:  lemma │ homonym │ form │ voice │ mood │ tense │ person │ number │
        case │ infinitive │ participle │ possessive │ polarity │ reading │ ok
```

The cell dimensions are spelled out (`indicative`, `past`, `first person`) with
no code table, and `reading` keeps the Oracle's own tag string -- the omor
suffix the form was generated from -- so a cell can be handed back to Omorfi
verbatim. `ok` is `both` when the two oracles agree the form is real and
`omorfi` when only Omorfi asserts it, the same contract as the noun lexicon.

## The synthetic and the analytic

A synthetic form is one word, and both oracles speak about it directly: Omorfi
generates it from a tag string, Voikko reads it back. That is the grid of
`verb_slices`.

An analytic form is several -- `olen puhunut`, `en ole puhunut`, `ei puhuttaisi`
-- and neither oracle has a surface for it: Omorfi generates one word per
reading and Voikko analyses one word at a time. So those are built
compositionally: the auxiliaries (`olla` and the negative `ei`) are generated
once per worker from their own paradigms, combined with the main verb's own
participle or connegative, and the compound is `both` when that main-verb word
is one Voikko confirms for the lemma -- exactly the word-by-word AND
`check_verbs_with_voikko.py` does. The `reading` of a compound is its two (or
three) component readings joined with a space, so it is as reconstructible as a
synthetic one.

## What is *not* here

The negative auxiliary's own paradigm (`en`, `et`, `ei`, `emme`, `ette`,
`eivät`, and the imperative `älä`, ...) is a separate part of speech in the
sanalista, `kieltoverbi`, and is not built as a lemma: several of its entries
are fused conjunctions (`ettei`) and it has no 1st infinitive to key on. Its
forms appear here as the first word of every negative compound.

Two corners of the paradigm are not `(case, number)` grid cells, so they are
added separately: the comitative of the participles and the `-minen`
infinitive (plural-only and number-invariant, so Omorfi generates it without a
`NUM`), and the active negative conditional, potential and imperative
(`en puhuisi`, `älä puhu`), whose connegatives Omorfi does generate
(`[VOICE=ACT][MOOD=COND][NEG=CON]` is `puhuisi`) even though the grid above
only assembles the indicative negatives.

After the grid, the same command writes the `connectives` table and the
`verb_connectives.csv` / `verb_connegatives.csv` build inputs. The default
`--engine` pass also adds finite forms for sanalista verbs Omorfi does not
analyse; `--no-engine` leaves an oracle-only build.

## Usage

    # the whole thing (about 1380 cells per verb)
    python3 tools/build_verb_lexicon.py

    # a fast smoke build: 30 verbs per class, synthetic grid only
    python3 tools/build_verb_lexicon.py -n 30 --possessives none --compounds none

    # the bare synthetic grid, every verb, on four cores
    python3 tools/build_verb_lexicon.py --possessives none -j 4

    # one Kotus type
    python3 tools/build_verb_lexicon.py --classes 61

## Parallelism

`-j N` forks N workers, each with its own Omorfi and Voikko, because the two
libraries are C++ objects the harness serialises on a lock -- threads would only
contend on it. The unit of work is a `--chunk` of lemmas, not a class: the
largest Kotus type (`53*C`, 1883 lemmas) is over two hours of single-core work,
so leaving it whole pins the wall clock at that two hours however many workers
there are. Chunking lets the pool balance it and the run fall with `-j` again,
bounded by cores and by RAM (each worker is about 800 MB).

A long run can be killed without warning -- the OOM killer takes the process
group and leaves no traceback. The writer commits each job as it finishes and
records the count in `<output>.progress`, so `--resume` picks the run back up
where it stopped instead of rebuilding from the first lemma.
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import random
import re
import sqlite3
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from omorfi_harness import (  # noqa: E402
    BOUNDARY_MARKER,
    CASE_NAMES,
    SANALISTA,
    Harness,
    eprint,
)

#: What the two oracles said about a form. `both` is the only value that means
#: two independent authorities agree; `omorfi` means the word is in the
#: morphology database and no lexicon has been asked about it.
BOTH = "both"
OMORFI_ONLY = "omorfi"

#: The finite person/number endings Omorfi names `PERS`, and the passive's one
#: pseudo-person `PE4`. The number is carried inside the person (`SG1`, `PL3`),
#: so the two are read back out of one attribute.
PERSONS = ("SG1", "SG2", "SG3", "PL1", "PL2", "PL3")
PASSIVE_PERSON = "PE4"
#: The imperative has no first-person singular and no first-person plural ending
#: in the active (`*puhun`, `*puhuamme` are not the imperative), only these.
IMPERATIVE_PERSONS = ("SG2", "SG3", "PL1", "PL2", "PL3")

NUMBERS = ("SG", "PL")
#: The nominal cases a participle, the 4th infinitive or the negative
#: participle declines through. All 13 the noun grid uses. The verb grid also
#: has the comitative; it is kept out of `CASES` because it is plural-only and
#: number-invariant, so it is not a `(case, number)` grid cell.
CASES = tuple(CASE_NAMES)
VERB_CASE_NAMES = {**CASE_NAMES, "COM": "comitative"}

#: The participle and infinitive stems that decline in the comitative. The
#: participle path needs `CMP=POS` or generation answers with nothing; the 4th
#: infinitive is a derivation and takes the case directly. `[DRV=NUT]` is
#: `olla`'s past participle and generates nothing for any other verb, so it is
#: left out and the `[VOICE=PSS][PCP=NUT]` slice covers the passive.
COMITATIVE_SLICES = (
    "[VOICE=ACT][PCP=VA][CMP=POS][CASE=COM]",
    "[VOICE=PSS][PCP=VA][CMP=POS][CASE=COM]",
    "[VOICE=ACT][PCP=NUT][CMP=POS][CASE=COM]",
    "[VOICE=PSS][PCP=NUT][CMP=POS][CASE=COM]",
    "[PCP=AGENT][CMP=POS][CASE=COM]",
    "[PCP=NEG][CMP=POS][CASE=COM]",
    "[DRV=MINEN][CASE=COM]",
)

#: The possessives Omorfi spells `POSS`. The third person is one code for both
#: numbers, as it is for a noun.
ALL_POSSESSIVES = ("SG1", "SG2", "PL1", "PL2", "3")

#: Omorfi's values -> the words stored. The two sides spell a feature
#: differently often enough (`PERS=SG1` is `first person`, `POSS=SG1` is
#: `first person singular`) that these are maps rather than a format string.
VOICE_WORDS = {"ACT": "active", "PSS": "passive"}
MOOD_WORDS = {
    "INDV": "indicative",
    "COND": "conditional",
    "IMPV": "imperative",
    "POTN": "potential",
}
TENSE_WORDS = {"PRESENT": "present", "PAST": "past"}
NUMBER_WORDS = {"SG": "singular", "PL": "plural"}
INF_WORDS = {"A": "1st", "E": "2nd", "MA": "3rd", "MINEN": "4th", "MAISILLA": "5th"}
POSSESSIVE_WORDS = {
    "SG1": "first person singular",
    "SG2": "second person singular",
    "PL1": "first person plural",
    "PL2": "second person plural",
    "3": "third person",
}
#: `PCP=VA`/`NUT` names the tense; the voice picks the half.
PARTICIPLE_WORDS = {
    ("VA", "active"): "present active",
    ("VA", "passive"): "present passive",
    ("NUT", "active"): "past active",
    ("NUT", "passive"): "past passive",
}

_ATTR = re.compile(r"\[([^=\]]+)=([^\]]*)\]")


def attrs(raw: str) -> dict[str, str]:
    """The `[KEY=VALUE]` pairs of an omor string, last one winning."""
    return dict(_ATTR.findall(raw))


def _person(pers: str) -> str:
    """`SG1` -> `first`; `PE4` (the passive) and anything else -> `""`."""
    if len(pers) >= 3 and pers[:2] in ("SG", "PL") and pers[2] in "123":
        return {"1": "first", "2": "second", "3": "third"}[pers[2]]
    return ""


def reading_of(tags: str) -> dict[str, str]:
    """The spelled-out reading a slice's tag string denotes.

    The cell dimensions are derived from the tag string rather than carried
    beside it, so the grid below is one list of tags and the labels cannot drift
    from the tags they describe.
    """
    a = attrs(tags)
    voice = VOICE_WORDS.get(a.get("VOICE", ""), "")
    mood = MOOD_WORDS.get(a.get("MOOD", ""), "")
    tense = TENSE_WORDS.get(a.get("TENSE", ""), "")
    pers = a.get("PERS", "")
    person = _person(pers)
    # A finite form carries its number inside the person (`SG3` is a singular
    # third person), so the number falls back to the person tag when there is
    # no `NUM` of its own.
    number = NUMBER_WORDS.get(a.get("NUM", ""))
    if number is None:
        number = {"SG": "singular", "PL": "plural"}.get(pers[:2], "")
    case = VERB_CASE_NAMES.get(a.get("CASE", ""), "")
    # The comitative is plural-only and Omorfi generates it without a `NUM`.
    if case == "comitative" and not number:
        number = "plural"
    polarity = "connegative" if a.get("NEG") == "CON" else "affirmative"

    infinitive = INF_WORDS.get(a.get("INF", ""), "")
    participle = PARTICIPLE_WORDS.get((a.get("PCP", ""), voice), "")
    if a.get("PCP") == "AGENT":
        participle = "agent"
    elif a.get("PCP") == "NEG":
        participle = "negative"
    # The 4th infinitive, the 5th and `olla`'s past participle are lexc
    # *derivations* rather than the `INF`/`PCP` path (`ollut` has no
    # `[VOICE=ACT][PCP=NUT]` reading at all), so the derivation tag names the
    # same cell the regular path would.
    elif a.get("DRV") == "MINEN":
        infinitive = "4th"
    elif a.get("DRV") == "MAISILLA":
        infinitive = "5th"
    elif a.get("DRV") == "NUT":
        participle = "past active"

    return {
        "voice": voice,
        "mood": mood,
        "tense": tense,
        "person": person,
        "number": number,
        "case": case,
        "infinitive": infinitive,
        "participle": participle,
        "possessive": POSSESSIVE_WORDS.get(a.get("POSS", ""), ""),
        "polarity": polarity,
    }


def verb_slices(possessives: tuple[str, ...] = ALL_POSSESSIVES) -> list[str]:
    """Every inflectional tag tail a verb is asked to generate.

    Each entry is the suffix after the word's own `[WORD_ID=..][UPOS=..]` head.
    The list is the paradigm: the finite active and passive, the connegatives,
    all five infinitives, and the participles through the full nominal grid. A
    form that answers more than one slice is one word with two readings, which
    `build_lemma` keeps as two cells on one form.

    `possessives` is the possessive series hung off the nominal-like slices
    (the participles, the 2nd and 4th infinitives). The finite cell never takes
    one, and the 5th infinitive exists only with one.
    """
    slices: list[str] = []

    # -- finite active ----------------------------------------------------
    for tense in ("PRESENT", "PAST"):
        for pers in PERSONS:
            slices.append(f"[VOICE=ACT][MOOD=INDV][TENSE={tense}][PERS={pers}]")
    for pers in PERSONS:
        # The conditional and the potential are present-only: Finnish has no
        # conditional past, and the potential's past is the compound `lienee`.
        slices.append(f"[VOICE=ACT][MOOD=COND][PERS={pers}]")
        slices.append(f"[VOICE=ACT][MOOD=POTN][PERS={pers}]")
    for pers in IMPERATIVE_PERSONS:
        slices.append(f"[VOICE=ACT][MOOD=IMPV][PERS={pers}]")
    # The connegative is the main verb of the negative sentence (`en puhu`),
    # the one word of it that inflects; the auxiliary is a separate lemma. The
    # present one is number-invariant (`puhu`), the past one agrees with the
    # subject (`en puhunut`, `emme puhuneet`).
    slices.append("[VOICE=ACT][MOOD=INDV][TENSE=PRESENT][NEG=CON]")
    for num in NUMBERS:
        slices.append(f"[VOICE=ACT][MOOD=INDV][TENSE=PAST][NUM={num}][NEG=CON]")

    # -- finite passive ---------------------------------------------------
    for tense in ("PRESENT", "PAST"):
        slices.append(f"[VOICE=PSS][MOOD=INDV][TENSE={tense}][PERS={PASSIVE_PERSON}]")
    for mood in ("COND", "POTN", "IMPV"):
        slices.append(f"[VOICE=PSS][MOOD={mood}][PERS={PASSIVE_PERSON}]")
    for tense in ("PRESENT", "PAST"):
        slices.append(
            f"[VOICE=PSS][MOOD=INDV][TENSE={tense}][PERS={PASSIVE_PERSON}][NEG=CON]"
        )

    # -- infinitives ------------------------------------------------------
    # 1st: the citation form itself, in the lative (`puhua`).
    for num in NUMBERS:
        slices.append(f"[VOICE=ACT][INF=A][NUM={num}][CASE=LAT]")
    # 2nd: inessive (`puhuessa`) and instructive (`puhuen`), with the
    # possessives it takes (`puhuessani`). The passive spells `puhuttaessa`.
    for case in ("INE", "INS"):
        slices.append(f"[VOICE=ACT][INF=E][NUM=SG][CASE={case}]")
        for p in possessives:
            slices.append(f"[VOICE=ACT][INF=E][NUM=SG][CASE={case}][POSS={p}]")
        slices.append(f"[VOICE=PSS][INF=E][NUM=SG][CASE={case}]")
    # 3rd: the active in six cases (`puhumaan`, `puhumatta`, ...), the passive
    # one in the instructive (`puhuttaman`) only.
    for case in ("ILL", "INE", "ELA", "ADE", "ABE", "INS"):
        slices.append(f"[VOICE=ACT][INF=MA][NUM=SG][CASE={case}]")
    slices.append("[VOICE=PSS][INF=MA][NUM=SG][CASE=INS]")
    # 5th: exists only with a possessive (`puhumaisillaan`).
    for p in possessives:
        slices.append(f"[VOICE=ACT][DRV=MAISILLA][POSS={p}]")
    # 4th: a nominal in `-minen` (`puhuminen`), declined through the noun grid.
    for num in NUMBERS:
        for case in CASES:
            base = f"[DRV=MINEN][NUM={num}][CASE={case}]"
            slices.append(base)
            for p in possessives:
                slices.append(base + f"[POSS={p}]")

    # -- participles ------------------------------------------------------
    # Present and past, active and passive, declared like the nouns they are.
    for pcp in ("VA", "NUT"):
        for voice in ("ACT", "PSS"):
            for num in NUMBERS:
                for case in CASES:
                    base = f"[VOICE={voice}][PCP={pcp}][CMP=POS][NUM={num}][CASE={case}]"
                    slices.append(base)
                    for p in possessives:
                        slices.append(base + f"[POSS={p}]")
    # The agent participle and the negative participle likewise.
    for tag in ("AGENT", "NEG"):
        for num in NUMBERS:
            for case in CASES:
                base = f"[PCP={tag}][CMP=POS][NUM={num}][CASE={case}]"
                slices.append(base)
                for p in possessives:
                    slices.append(base + f"[POSS={p}]")
    # `olla`'s past participle is a derivation (`ollut`), not a `PCP=NUT`.
    for num in NUMBERS:
        for case in CASES:
            base = f"[DRV=NUT][CMP=POS][NUM={num}][CASE={case}]"
            slices.append(base)
            for p in possessives:
                slices.append(base + f"[POSS={p}]")

    # -- comitative -------------------------------------------------------
    # Plural-only and number-invariant, so it is not a `(case, number)` cell of
    # the grid above. The participles and the `-minen` infinitive all decline
    # in it (`puhuvine`, `puhumisineen`), with and without a possessive.
    for base in COMITATIVE_SLICES:
        slices.append(base)
        for p in possessives:
            slices.append(base + f"[POSS={p}]")

    return slices


def verb_prefix(harness: Harness, lemma: str) -> str | None:
    """The `[WORD_ID=..][UPOS=..]` head to generate `lemma`'s forms from.

    The lemma is a form of itself and thus appears in its own analyses, but not
    always as the reading that names the lexeme: `jäädä`'s first verb analysis
    is `[WORD_ID=jäätää][MOOD=IMPV][PERS=SG2]` (the imperative of `jäätää`),
    and `kieltää`/`rakentaa` are their own present third person. The candidate
    is therefore accepted only if it generates *the lemma itself* as the 1st
    infinitive, which pins the reading to the lexeme and not to one of its
    homographs.

    Returns `None` for a lemma Omorfi has no verb reading for -- the caller
    records it as skipped rather than lettering a wrong head generate forms
    that are not there.
    """
    candidates: list[str] = []
    for raw in harness.analyse_raw(lemma):
        a = attrs(raw)
        if a.get("UPOS") not in ("VERB", "AUX") or not a.get("WORD_ID"):
            continue
        head = f"[WORD_ID={a['WORD_ID']}][UPOS={a['UPOS']}]"
        # The lexeme's own reading is the 1st infinitive; prefer it over a
        # finite homograph of the same word id.
        if a.get("INF") == "A" and a.get("VOICE") == "ACT":
            candidates.insert(0, head)
        elif head not in candidates:
            candidates.append(head)
    for head in candidates:
        generated = harness.generate_tags(head, "[VOICE=ACT][INF=A][NUM=SG][CASE=LAT]")
        if lemma in generated.split("/"):
            return head
    return None


class SqliteWriter:
    """Writes exactly two tables: `base` and `forms`, as the noun lexicon does.

    Classes arrive in Kotus order and are inserted as they come, so nothing
    larger than one class is held on this side.
    """

    def __init__(self, path: str, started: float) -> None:
        self.started = started
        if path != ":memory:":
            Path(path).parent.mkdir(parents=True, exist_ok=True)
        self.conn = sqlite3.connect(path)
        self.conn.executescript(SCHEMA)
        self.conn.commit()

    def add_class(self, result: dict) -> None:
        cls = result["class"]
        base_rows: list[tuple] = []
        form_rows: list[tuple] = []
        for key, rows in result["lemmas"].items():
            lemma, _, homonym = key.partition("#")
            base_rows.append((lemma, homonym, rows[0]["part_of_speech"], cls))
            for r in rows:
                for rd in r["readings"]:
                    form_rows.append((
                        lemma, homonym, r["form"],
                        rd["voice"], rd["mood"], rd["tense"], rd["person"],
                        rd["number"], rd["case"], rd["infinitive"],
                        rd["participle"], rd["possessive"], rd["polarity"],
                        rd["reading"], r["ok"],
                    ))
        self.conn.executemany("INSERT OR REPLACE INTO base VALUES (?, ?, ?, ?)",
                              base_rows)
        self.conn.executemany(
            "INSERT OR REPLACE INTO forms VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
            "?, ?, ?, ?, ?)",
            form_rows)
        self.conn.commit()

    def close(self) -> dict:
        one = self.conn.execute(
            "SELECT (SELECT COUNT(*) FROM base), "
            "       (SELECT COUNT(*) FROM forms)"
        ).fetchone()
        stats = {
            "class_values": self.conn.execute(
                "SELECT COUNT(DISTINCT class) FROM base").fetchone()[0],
            "lemmas": one[0],
            "words": self.conn.execute(
                "SELECT COUNT(*) FROM (SELECT DISTINCT lemma, homonym FROM forms)"
            ).fetchone()[0],
            "rows": one[1],
            "elapsed_seconds": round(time.time() - self.started, 1),
        }
        self.conn.commit()
        self.conn.close()
        return stats


#: One reference table and one paradigm table, with the cell spelled out and
#: the oracle's own `reading` kept beside it. The primary key is one row per
#: form per *cell*: a surface that answers two readings is two rows, which is
#: what it is.
SCHEMA = """
PRAGMA journal_mode = OFF;
PRAGMA synchronous = OFF;
PRAGMA temp_store = MEMORY;

CREATE TABLE IF NOT EXISTS base (
    lemma          TEXT NOT NULL,
    homonym        TEXT NOT NULL DEFAULT '',
    part_of_speech TEXT NOT NULL,
    class          TEXT NOT NULL,
    PRIMARY KEY (lemma, homonym)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS forms (
    lemma       TEXT NOT NULL,
    homonym     TEXT NOT NULL DEFAULT '',
    form        TEXT NOT NULL,
    voice       TEXT NOT NULL DEFAULT '',
    mood        TEXT NOT NULL DEFAULT '',
    tense       TEXT NOT NULL DEFAULT '',
    person      TEXT NOT NULL DEFAULT '',
    number      TEXT NOT NULL DEFAULT '',
    "case"      TEXT NOT NULL DEFAULT '',
    infinitive  TEXT NOT NULL DEFAULT '',
    participle  TEXT NOT NULL DEFAULT '',
    possessive  TEXT NOT NULL DEFAULT '',
    polarity    TEXT NOT NULL DEFAULT '',
    reading     TEXT NOT NULL,
    ok          TEXT NOT NULL CHECK (ok IN ('both', 'omorfi')),
    PRIMARY KEY (lemma, homonym, form, voice, mood, tense, person, number,
                 "case", infinitive, participle, possessive, polarity)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS forms_lemma ON forms (lemma, homonym);
"""


#: The six person/number slots with the two words a row spells them with. The
#: omor tag carries both (`SG1`), the report wants them apart.
PERSON_NUMBERS = (
    ("SG1", "first", "singular"),
    ("SG2", "second", "singular"),
    ("SG3", "third", "singular"),
    ("PL1", "first", "plural"),
    ("PL2", "second", "plural"),
    ("PL3", "third", "plural"),
)
_IMPERATIVE_PERSONS = ("SG2", "SG3", "PL1", "PL2", "PL3")


def _negative_head(harness: Harness) -> str | None:
    """The `[WORD_ID=ei][UPOS=AUX][SUBCAT=NEG]` head of the negative auxiliary.

    The auxiliary is not a verb of the `verbi` part of speech and has no 1st
    infinitive for `verb_prefix` to verify against, so its head is read straight
    off its own analysis, `SUBCAT=NEG` and all -- the `SUBCAT` is what makes the
    generator answer with `en`/`et`/`ei` rather than with `olla`.
    """
    for raw in harness.analyse_raw("ei"):
        a = attrs(raw)
        if a.get("SUBCAT") == "NEG" and a.get("WORD_ID"):
            return f"[WORD_ID={a['WORD_ID']}][UPOS={a['UPOS']}][SUBCAT=NEG]"
    return None


def auxiliary_forms(harness: Harness) -> dict[str, tuple[list[str], str]]:
    """Every auxiliary word the compounds need, generated once.

    Two auxiliaries carry Finnish analytic verb phrases: `olla` for the perfect
    and pluperfect (`olen puhunut`, `on puhuttu`) and `ei` for the negative
    (`en puhu`, `ei puhuta`). They are the same for every main verb, so they are
    generated once per worker and combined with each verb's own participles.

    The key names the slot (`olla-present-SG1`, `ei-impv-SG3`, ...) and maps to
    the surface spellings and the tag they came from; the tag is kept so a
    compound row can record exactly which two readings it was built from.
    """
    olla = verb_prefix(harness, "olla")
    ei = _negative_head(harness)
    forms: dict[str, tuple[list[str], str]] = {}

    def put(name: str, head: str | None, tags: str) -> None:
        if not head:
            return
        surfaces = [s.strip() for s in harness.generate_tags(head, tags).split("/")
                    if s.strip()]
        if surfaces:
            forms[name] = (surfaces, tags)

    for pers, _person, _number in PERSON_NUMBERS:
        put(f"olla-present-{pers}", olla,
            f"[VOICE=ACT][MOOD=INDV][TENSE=PRESENT][PERS={pers}]")
        put(f"olla-past-{pers}", olla,
            f"[VOICE=ACT][MOOD=INDV][TENSE=PAST][PERS={pers}]")
        put(f"olla-cond-{pers}", olla, f"[VOICE=ACT][MOOD=COND][PERS={pers}]")
        put(f"olla-potn-{pers}", olla, f"[VOICE=ACT][MOOD=POTN][PERS={pers}]")
    put("olla-impv-SG3", olla, "[VOICE=ACT][MOOD=IMPV][PERS=SG3]")
    put("olla-impv-PL3", olla, "[VOICE=ACT][MOOD=IMPV][PERS=PL3]")
    put("olla-ole", olla, "[VOICE=ACT][MOOD=INDV][TENSE=PRESENT][NEG=CON]")
    put("olla-ollut", olla, "[VOICE=ACT][MOOD=INDV][TENSE=PAST][NUM=SG][NEG=CON]")
    put("olla-olleet", olla, "[VOICE=ACT][MOOD=INDV][TENSE=PAST][NUM=PL][NEG=CON]")
    put("olla-liene", olla, "[VOICE=ACT][MOOD=POTN][NEG=CON]")
    put("olla-olisi", olla, "[VOICE=ACT][MOOD=COND][NEG=CON]")
    put("olla-olko", olla, "[VOICE=ACT][MOOD=IMPV][NEG=CON]")
    for pers, _person, _number in PERSON_NUMBERS:
        put(f"ei-{pers}", ei, f"[VOICE=ACT][PERS={pers}]")
    for pers in _IMPERATIVE_PERSONS:
        put(f"ei-impv-{pers}", ei, f"[VOICE=ACT][MOOD=IMPV][PERS={pers}]")

    return forms


def main_parts(harness: Harness, head: str) -> dict[str, list[tuple[str, str]]]:
    """The main verb's pieces an auxiliary combines with.

    Each is a non-finite form the synthetic grid already generates in its own
    right -- the connegatives of the negative sentences, the past active
    participle that agrees with the subject, the passive past participle. The
    first tag that generates wins; the fallback is for `olla`, whose past
    participle Omorfi spells as the derivation `[DRV=NUT]` rather than through
    `[PCP=NUT]`.
    """
    def part(*tag_options: str) -> list[tuple[str, str]]:
        for tags in tag_options:
            out = harness.generate_tags(head, tags)
            if out:
                return [(s.replace(BOUNDARY_MARKER, "").strip(), tags)
                        for s in out.split("/") if s.strip()]
        return []

    return {
        "present_neg": part("[VOICE=ACT][MOOD=INDV][TENSE=PRESENT][NEG=CON]"),
        # The active negative conditional, potential and imperative are not
        # part of the indicative grid. Their main-verb word is still a
        # connegative Omorfi generates.
        "cond_neg": part("[VOICE=ACT][MOOD=COND][NEG=CON]"),
        "potn_neg": part("[VOICE=ACT][MOOD=POTN][NEG=CON]"),
        "impv_neg": part("[VOICE=ACT][MOOD=IMPV][NEG=CON]"),
        "past_act_SG": part("[VOICE=ACT][PCP=NUT][CMP=POS][NUM=SG][CASE=NOM]",
                            "[DRV=NUT][CMP=POS][NUM=SG][CASE=NOM]"),
        "past_act_PL": part("[VOICE=ACT][PCP=NUT][CMP=POS][NUM=PL][CASE=NOM]",
                            "[DRV=NUT][CMP=POS][NUM=PL][CASE=NOM]"),
        "pass_present_neg": part(
            "[VOICE=PSS][MOOD=INDV][TENSE=PRESENT][PERS=PE4][NEG=CON]"),
        "pass_past_neg": part(
            "[VOICE=PSS][MOOD=INDV][TENSE=PAST][PERS=PE4][NEG=CON]"),
        "pass_cond_neg": part("[VOICE=PSS][MOOD=COND][PERS=PE4][NEG=CON]"),
        "pass_potn_neg": part("[VOICE=PSS][MOOD=POTN][PERS=PE4][NEG=CON]"),
        "pass_impv_neg": part("[VOICE=PSS][MOOD=IMPV][PERS=PE4][NEG=CON]"),
        "pass_ppart": part("[VOICE=PSS][PCP=NUT][CMP=POS][NUM=SG][CASE=NOM]"),
    }


def _compound_specs() -> list[tuple]:
    """The analytic cells, as `(aux, middle, main, voice, mood, tense, person,
    number, polarity)`.

    `aux` and `middle` name entries of `auxiliary_forms`, `main` an entry of
    `main_parts`. A cell with no `middle` is two words (`olen puhunut`); the
    three-word ones are the perfects whose auxiliary is itself a compound
    (`en ole puhunut`, `ei ollut puhuttu`). Passive cells carry no person, and
    the passive auxiliary is always the third singular.
    """
    out: list[tuple] = []
    for pers, person, number in PERSON_NUMBERS:
        singular = number == "singular"
        part = "past_act_SG" if singular else "past_act_PL"
        for aux, mood, tense in (
            (f"olla-present-{pers}", "indicative", "perfect"),
            (f"olla-past-{pers}", "indicative", "pluperfect"),
            (f"olla-cond-{pers}", "conditional", "perfect"),
            (f"olla-potn-{pers}", "potential", "perfect"),
        ):
            out.append((aux, None, part, "active", mood, tense, person, number,
                        "affirmative"))
        out.append((f"ei-{pers}", None, "present_neg", "active", "indicative",
                    "present", person, number, "negative"))
        out.append((f"ei-{pers}", None, part, "active", "indicative", "past",
                    person, number, "negative"))
        out.append((f"ei-{pers}", "olla-ole", part, "active", "indicative",
                    "perfect", person, number, "negative"))
        out.append((f"ei-{pers}", "olla-ollut" if singular else "olla-olleet",
                    part, "active", "indicative", "pluperfect", person, number,
                    "negative"))
        # The negative conditional and potential use the same auxiliary as
        # the indicative negative, but the main verb is the mood's connegative
        # (`en puhuisi`, `en puhune`).
        out.append((f"ei-{pers}", None, "cond_neg", "active", "conditional",
                    "present", person, number, "negative"))
        out.append((f"ei-{pers}", None, "potn_neg", "active", "potential",
                    "present", person, number, "negative"))
    # The negative imperative has its own auxiliary (`älä`, `älköön`, ...).
    for pers in _IMPERATIVE_PERSONS:
        person, number = next(
            (p, n) for pers_, p, n in PERSON_NUMBERS if pers_ == pers
        )
        # `älä puhu` keeps the bare present connegative; every other person
        # takes the `-ko` one (`älköön puhuko`).
        main = "present_neg" if pers == "SG2" else "impv_neg"
        out.append((f"ei-impv-{pers}", None, main, "active", "imperative",
                    "present", person, number, "negative"))
    # The imperative perfect has only the third persons (`olkoon jäänyt`,
    # `olkoot jääneet`).
    out.append(("olla-impv-SG3", None, "past_act_SG", "active", "imperative",
                "perfect", "third", "singular", "affirmative"))
    out.append(("olla-impv-PL3", None, "past_act_PL", "active", "imperative",
                "perfect", "third", "plural", "affirmative"))
    # The passive compounds take no person, so each has only the third singular.
    for aux, mood, tense in (
        ("olla-present-SG3", "indicative", "perfect"),
        ("olla-past-SG3", "indicative", "pluperfect"),
        ("olla-cond-SG3", "conditional", "perfect"),
        ("olla-potn-SG3", "potential", "perfect"),
        ("olla-impv-SG3", "imperative", "perfect"),
    ):
        out.append((aux, None, "pass_ppart", "passive", mood, tense, "", "",
                    "affirmative"))
    for aux, middle, main, mood, tense in (
        ("ei-SG3", None, "pass_present_neg", "indicative", "present"),
        ("ei-SG3", None, "pass_past_neg", "indicative", "past"),
        ("ei-SG3", None, "pass_cond_neg", "conditional", "present"),
        ("ei-SG3", None, "pass_potn_neg", "potential", "present"),
        ("ei-impv-SG3", None, "pass_impv_neg", "imperative", "present"),
        ("ei-SG3", "olla-ole", "pass_ppart", "indicative", "perfect"),
        ("ei-SG3", "olla-ollut", "pass_ppart", "indicative", "pluperfect"),
        ("ei-SG3", "olla-liene", "pass_ppart", "potential", "perfect"),
        ("ei-SG3", "olla-olisi", "pass_ppart", "conditional", "perfect"),
        ("ei-impv-SG3", "olla-olko", "pass_ppart", "imperative", "perfect"),
    ):
        out.append((aux, middle, main, "passive", mood, tense, "", "",
                    "negative"))
    return out


COMPOUND_SPECS = _compound_specs()


def add_compounds(harness: Harness, head: str, lemma: str,
                  aux: dict[str, tuple[list[str], str]], credit) -> None:
    """Credit every analytic cell this verb has the pieces for.

    A compound is not one word to either oracle: Omorfi has no surface to
    generate for `olen puhunut` and Voikko no surface to analyse. What is
    checked is the main verb's part of it, word by word, exactly as
    `check_verbs_with_voikko.py` does -- the auxiliary is a known word of
    `olla`/`ei`, and the participle is a form of the lemma or the phrase is not
    one. So a compound is `both` when its main-verb word is `both` and `omorfi`
    otherwise.
    """
    parts = main_parts(harness, head)
    for aux_key, middle_key, part_key, voice, mood, tense, person, number, polarity in COMPOUND_SPECS:
        if aux_key not in aux:
            continue
        aux_words, aux_tags = aux[aux_key]
        if middle_key:
            if middle_key not in aux:
                continue
            mid_words, mid_tags = aux[middle_key]
        else:
            mid_words, mid_tags = ("",), ""
        mains = parts.get(part_key) or []
        if not mains:
            continue
        reading = {
            "voice": voice, "mood": mood, "tense": tense, "person": person,
            "number": number, "case": "", "infinitive": "", "participle": "",
            "possessive": "", "polarity": polarity,
        }
        for aux_word in aux_words:
            for mid_word in mid_words:
                for main_word, main_tags in mains:
                    words = [w for w in (aux_word, mid_word, main_word) if w]
                    form = " ".join(words)
                    tags = " ".join(t for t in (aux_tags, mid_tags, main_tags) if t)
                    credit(form, reading, tags,
                           confirmed=harness.voikko_ok(lemma, main_word))


def build_lemma(harness: Harness, lemma: str, slices: list[str],
                aux: dict[str, tuple[list[str], str]] | None = None) -> tuple[list[dict], str]:
    """Every form of one lemma, one row per form, plus a skip reason.

    One row per *word*, with one `reading` per cell it answers, so the count is
    the count of words a checker can be asked about rather than the count of
    cells it was asked about.
    """
    head = verb_prefix(harness, lemma)
    if head is None:
        return [], "no verb reading in the oracle"

    by_form: dict[str, dict] = {}
    # Which cells a form has already been credited with. Two slices can spell
    # the same cell (`[VOICE=ACT][PCP=NUT]` and `[DRV=NUT]` are both the past
    # active participle), and a cell credited twice is a duplicate reading.
    credited: dict[str, set[tuple]] = {}

    def credit(form: str, reading: dict, tags: str,
               confirmed: bool | None = None) -> None:
        row = by_form.get(form)
        if row is None:
            by_form[form] = row = {"form": form, "ok": OMORFI_ONLY, "readings": []}
            credited[form] = set()
            if confirmed is None:
                confirmed = harness.voikko_ok(lemma, form)
            if confirmed:
                row["ok"] = BOTH
        elif row["ok"] == OMORFI_ONLY:
            if confirmed is None:
                confirmed = harness.voikko_ok(lemma, form)
            if confirmed:
                row["ok"] = BOTH
        key = tuple(reading.values())
        if key in credited[form]:
            return
        credited[form].add(key)
        row["readings"].append({**reading, "reading": tags})

    for tags in slices:
        generated = harness.generate_tags(head, tags)
        reading = reading_of(tags)
        for spelling in generated.split("/"):
            form = spelling.replace(BOUNDARY_MARKER, "").strip()
            if not form:
                continue
            credit(form, reading, tags)

    if aux:
        add_compounds(harness, head, lemma, aux, credit)

    if not by_form:
        return [], "no form in the oracle"
    return [by_form[f] for f in sorted(by_form)], ""


#: Set in each worker by `_init_worker`; one Omorfi and one Voikko per process,
#: and the auxiliary words generated once, since every main verb combines with
#: the same `olla` and `ei`.
_HARNESS: Harness | None = None
_SLICES: list[str] | None = None
_AUX: dict[str, tuple[list[str], str]] | None = None


def _init_worker(slices: list[str], compounds: bool) -> None:
    global _HARNESS, _SLICES, _AUX
    _HARNESS = Harness("verb", load_torille=False)
    _SLICES = slices
    _AUX = auxiliary_forms(_HARNESS) if compounds else None


def _class_order(name: str) -> tuple:
    """Kotus order: numeric classes in numeric order, then the `*` variants."""
    base = name.split("*")[0].strip("(")
    return (int(base) if base.isdigit() else 999, name)


def build_class(job: tuple[str, list[tuple[str, str, str]]]) -> dict:
    """One class: every (lemma, homonym)'s forms, plus the words skipped."""
    name, words = job
    assert _HARNESS is not None and _SLICES is not None
    per_word: dict[str, list[dict]] = {}
    skipped: dict[str, str] = {}
    seen: set[tuple[str, str]] = set()
    for lemma, homonym, pos in words:
        if (lemma, homonym) in seen:
            continue
        seen.add((lemma, homonym))
        rows, reason = build_lemma(_HARNESS, lemma, _SLICES, _AUX)
        key = lemma if not homonym else f"{lemma}#{homonym}"
        if reason:
            skipped[key] = reason
            continue
        for row in rows:
            row["part_of_speech"] = pos
            row["homonym"] = homonym
        per_word[key] = rows
    return {"class": name, "lemmas": per_word, "skipped": skipped,
            "class_size": len(words)}


def read_records(positions: list[str]) -> list[tuple[str, str, str, str]]:
    """Every (class, lemma, homonym, part_of_speech) chosen from the sanalista.

    Read directly rather than through the engine, because the source is keyed on
    (lemma, homonym) and the engine dedupes a lemma to a single class. Two
    homonyms are two words.

    The part of speech is matched a *token* at a time, not a substring: the
    Sanaluokka column joins several with `,` or ` +` (`kieltoverbi, substantiivi`)
    and `verbi` is a substring of `adverbi` and `kieltoverbi`, so a substring
    test would pull in every adverb in the file. A token must equal the wanted
    value.
    """
    out = []
    with open(SANALISTA, encoding="utf-8") as fh:
        next(fh, None)
        for line in fh:
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 4 or not fields[0] or not fields[3]:
                continue
            lemma, homonym, pos, cls = fields[0], fields[1], fields[2], fields[3]
            if positions:
                tokens = {t.strip() for t in re.split(r"[,+]", pos) if t.strip()}
                if not any(p in tokens for p in positions):
                    continue
            out.append((cls, lemma, homonym, pos))
    return out


def sanalista_verbs() -> list[tuple[str, str]]:
    """Every classed verb in the sanalista, as `(lemma, class expression)`."""
    out: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line in Path(SANALISTA).read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        word, pos, class_str = parts[0].strip(), parts[2].strip(), parts[3].strip()
        if not word or word in seen or pos != "verbi" or not class_str:
            continue
        seen.add(word)
        out.append((word, class_str))
    return out


def enrich_missing_verbs(
    db_path: str,
    *,
    wanted_classes: set[str] | None = None,
    limit: int = 0,
    dry_run: bool = False,
) -> dict[str, int]:
    """Add finite forms for sanalista verbs the Omorfi-built oracle lacks.

    Omorfi does not analyse every productive frequentative or colloquial verb,
    so those lemmas have no `base` row and the fast path's stem map cannot
    reach them. The engine, given the sanalista's own class, can inflect them;
    this is the `--engine` post-pass (or `--engine-only`).
    """
    from torille import Engine

    engine = Engine()
    con = sqlite3.connect(db_path)
    cur = con.cursor()
    known = {lemma for (lemma,) in cur.execute("SELECT DISTINCT lemma FROM base")}

    lemmas = [
        (word, klass)
        for word, klass in sanalista_verbs()
        if word not in known
        and (wanted_classes is None or klass in wanted_classes)
    ]
    if limit:
        lemmas = lemmas[:limit]

    bases = inserted = processed = 0
    for word, class_str in lemmas:
        try:
            forms = engine.inflect(word, klass=class_str, pos="verbi")
        except ValueError:
            continue
        processed += 1
        if not dry_run:
            cur.execute(
                "INSERT OR IGNORE INTO base "
                "(lemma, homonym, part_of_speech, class) VALUES (?,?,?,?)",
                (word, "", "verbi", class_str),
            )
        bases += 1
        for f in forms:
            if f.confidence == "guessed" or f.negative or f.clitic or f.possessive:
                continue
            if " " in f.word or f.case is not None:
                continue
            if f.mood not in ("indicative", "conditional", "imperative", "potential"):
                continue
            if f.voice == "active":
                if not (f.person and f.number):
                    continue
            elif f.voice == "passive":
                if f.person or f.number:
                    continue
            else:
                continue
            inserted += 1
            if dry_run:
                continue
            cur.execute(
                'INSERT OR IGNORE INTO forms '
                '(lemma, homonym, form, voice, mood, tense, person, number, '
                '"case", infinitive, participle, possessive, polarity, reading, ok) '
                "VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                (
                    word, "", f.word, f.voice or "", f.mood or "", f.tense or "",
                    (f.person[:-len("_person")] if f.person and
                     f.person.endswith("_person") else (f.person or "")),
                    f.number or "", "", "", "", "", "affirmative",
                    " ".join(f.tags), "both",
                ),
            )

    if dry_run:
        con.rollback()
    else:
        con.commit()
    con.close()
    return {"processed": processed, "bases": bases, "inserted": inserted}


#: The six finite forms of the negation auxiliary, with the person and number
#: each carries. Used by the `connectives` post-pass.
CONNECTIVE_FINITE = (
    ("en", "first", "singular"),
    ("et", "second", "singular"),
    ("ei", "third", "singular"),
    ("emme", "first", "plural"),
    ("ette", "second", "plural"),
    ("eivät", "third", "plural"),
)

CONNECTIVES_SCHEMA = """
CREATE TABLE IF NOT EXISTS connectives (
    surface     TEXT PRIMARY KEY,
    base        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    auxiliary   TEXT NOT NULL,
    conjunction TEXT NOT NULL,
    person      TEXT NOT NULL,
    number      TEXT NOT NULL,
    polarity    TEXT NOT NULL
)
"""


def joined_form(conjunction: str, auxiliary: str) -> str:
    """The written fusion of `conjunction` and `auxiliary`."""
    if conjunction == "ellei":
        return "ellei" if auxiliary == "ei" else "ell" + auxiliary
    if conjunction == "jollei":
        return "jollei" if auxiliary == "ei" else "joll" + auxiliary
    return conjunction[:-1] + auxiliary


def connectives_rows() -> list[tuple[str, str, str, str, str, str, str, str]]:
    """The closed set of finite auxiliaries, `-kä` clitics and fused conjunctions."""
    out: list[tuple[str, str, str, str, str, str, str, str]] = []
    for surface, person, number in CONNECTIVE_FINITE:
        out.append((surface, "ei", "finite", surface, "", person, number, "negative"))
    for surface, person, number in CONNECTIVE_FINITE:
        out.append((surface + "kä", "ei", "clitic", surface, "", person, number, "negative"))
    for conjunction in ("että", "ellei", "jollei"):
        for surface, person, number in CONNECTIVE_FINITE:
            out.append((
                joined_form(conjunction, surface), conjunction, "conjunction",
                surface, conjunction, person, number, "negative",
            ))
    return out


def write_connectives(db_path: str) -> dict[str, int]:
    """Record the negation fusions and the CSV `build.rs` embeds."""
    data = connectives_rows()
    con = sqlite3.connect(db_path)
    cur = con.cursor()
    cur.execute(CONNECTIVES_SCHEMA)
    before = cur.execute("SELECT COUNT(*) FROM connectives").fetchone()[0]
    cur.executemany(
        "INSERT OR IGNORE INTO connectives "
        "(surface, base, kind, auxiliary, conjunction, person, number, polarity) "
        "VALUES (?,?,?,?,?,?,?,?)",
        data,
    )
    after = cur.execute("SELECT COUNT(*) FROM connectives").fetchone()[0]
    con.commit()
    con.close()
    out = Path(SANALISTA).parent / "verb_connectives.csv"
    with open(out, "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, delimiter="\t", lineterminator="\n")
        w.writerow(["surface", "base", "kind"])
        for surface, base, kind, _aux, _conj, _person, _number, _polarity in data:
            w.writerow([surface, base, kind])
    return {"before": before, "after": after, "offered": len(data)}


def connegative_rows(con: sqlite3.Connection) -> list[tuple[str, str, str, str, str, str]]:
    """The bare potential and imperative connegatives attested in phrases."""
    seen: set[tuple[str, str]] = set()
    out: list[tuple[str, str, str, str, str, str]] = []
    cur = con.execute(
        "SELECT form, lemma, mood, voice, person, number "
        "FROM forms WHERE polarity = 'negative' AND ok = 'both' "
        "AND mood IN ('potential', 'imperative') AND form LIKE '% %'"
    )
    for form, lemma, mood, voice, person, number in cur:
        if not lemma or form.count(" ") != 1:
            continue
        if mood == "imperative" and person == "second" and number == "singular":
            continue
        surface = form.rsplit(" ", 1)[1]
        key = (surface, lemma)
        if key in seen:
            continue
        seen.add(key)
        out.append((surface, lemma, mood, voice, person, number))
    out.sort()
    return out


def write_connegatives(db_path: str) -> dict[str, int]:
    """Add the bare connegative rows and rewrite `verb_connegatives.csv`."""
    con = sqlite3.connect(db_path)
    rows = connegative_rows(con)
    cur = con.cursor()
    cur.executemany(
        "INSERT OR IGNORE INTO forms "
        "(lemma, homonym, form, voice, mood, tense, person, number, "
        '"case", infinitive, participle, possessive, polarity, reading, ok) '
        "VALUES (?, '', ?, ?, ?, 'present', ?, ?, '', '', '', '', "
        "'connegative', ?, 'both')",
        [(lemma, surface, voice, mood, person, number, f"connegative {mood}")
         for surface, lemma, mood, voice, person, number in rows],
    )
    inserted = cur.rowcount
    con.commit()
    con.close()
    out = Path(SANALISTA).parent / "verb_connegatives.csv"
    with open(out, "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, delimiter="\t")
        w.writerow(["surface", "base", "mood", "voice"])
        for surface, lemma, mood, voice, _person, _number in rows:
            w.writerow([surface, lemma, mood, voice])
    return {"derived": len(rows), "inserted": inserted}


def progress(i: int, total: int, started: float, quiet: bool) -> None:
    if quiet:
        return
    done = time.time() - started
    eta = done / i * (total - i)
    eprint(f"  [{i:3}/{total}] {done / 60:6.1f}m elapsed, eta {eta / 60:6.1f}m")


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0],
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("-o", "--output", default=str(SANALISTA.parent / "kotus_verbit.sqlite"),
                   help="sqlite output path (default: data/kotus_verbit.sqlite)")
    p.add_argument("-n", "--per-class", type=int, default=0,
                   help="lemmas per class, sampled (default: all of them)")
    p.add_argument("--seed", type=int, default=0, help="sample seed")
    p.add_argument("--classes", default="all",
                   help="'all', 'numeric' (the plain Kotus verb types), or a "
                        "comma list")
    p.add_argument("--pos", default="verbi",
                   help="part(s) of speech to include, matched as whole "
                        "tokens of the Sanaluokka column (default: verbi)")
    p.add_argument("--possessives", default="all",
                   help="'all' for the possessive grid on the nominal-like "
                        "slices, or 'none' for the bare synthetic grid (about "
                        "6x cheaper)")
    p.add_argument("--compounds", default="all",
                   help="'all' to add the analytic perfect/pluperfect and "
                        "negative cells (`olen puhunut`, `ei puhuta`), or "
                        "'none' for the synthetic single-word grid only")
    p.add_argument("--chunk", type=int, default=200,
                   help="lemmas per job: a class larger than this is split so "
                        "the pool can balance the big types (53*C is 1883 "
                        "lemmas). 0 schedules one class per job")
    p.add_argument("--resume", action="store_true",
                   help="continue a killed run from its <output>.progress "
                        "journal instead of starting over. Ignored if the "
                        "output or journal is missing or the schedule changed")
    p.add_argument("-j", "--processes", type=int, default=1,
                   help="worker processes over classes")
    p.add_argument("-q", "--quiet", action="store_true")
    p.add_argument("--engine", dest="engine", action="store_true", default=True,
                   help="add finite forms for sanalista verbs the oracle lacks "
                        "(default; needs the torille Python binding)")
    p.add_argument("--no-engine", dest="engine", action="store_false",
                   help="leave the output as the Omorfi + Voikko grid only")
    p.add_argument("--engine-only", action="store_true",
                   help="enrich an existing sqlite file without rebuilding it")
    p.add_argument("--engine-limit", type=int, default=0,
                   help="stop the engine pass after N lemmas")
    p.add_argument("--engine-dry-run", action="store_true",
                   help="report the engine pass without writing")
    args = p.parse_args(argv)

    if args.engine_only:
        if not Path(args.output).exists():
            p.error(f"{args.output}: not found")
        wanted = None
        if args.classes not in ("all", "numeric"):
            wanted = {c.strip() for c in args.classes.split(",") if c.strip()}
        try:
            stats = enrich_missing_verbs(
                args.output, wanted_classes=wanted,
                limit=args.engine_limit, dry_run=args.engine_dry_run,
            )
        except ImportError:
            print("the torille binding is not built:\n"
                  "  (cd bindings && env -u CONDA_PREFIX maturin develop --release)",
                  file=sys.stderr)
            return 2
        verb = "would enrich" if args.engine_dry_run else "enriched"
        print(f"{verb} {stats['processed']} missing verbs: "
              f"{stats['bases']} base rows, {stats['inserted']} finite form rows")
        return 0

    possessives = () if args.possessives == "none" else ALL_POSSESSIVES
    sliced = verb_slices(possessives)
    compounds = args.compounds != "none"

    positions = [x.strip() for x in args.pos.split(",") if x.strip()]
    if positions == ["all"]:
        positions = []
    records = read_records(positions)

    buckets: dict[str, list[tuple[str, str, str]]] = {}
    for cls, lemma, homonym, pos in records:
        bucket = buckets.setdefault(cls, [])
        bucket.append((lemma, homonym, pos))

    if args.classes == "all":
        selected = sorted(buckets, key=_class_order)
    elif args.classes == "numeric":
        selected = sorted((c for c in buckets
                           if c.isdigit() and "," not in c and "(" not in c),
                          key=_class_order)
    else:
        wanted = {c.strip() for c in args.classes.split(",") if c.strip()}
        selected = sorted((c for c in buckets if c in wanted), key=_class_order)

    rng = random.Random(args.seed)
    plan: list[tuple[str, list[tuple[str, str, str]]]] = []
    for cls in selected:
        words = sorted(buckets[cls])
        if args.per_class:
            rng.shuffle(words)
            words = words[: args.per_class]
        if words:
            plan.append((cls, words))

    total_words = sum(len(w) for _, w in plan)
    # One job per class leaves the biggest type whole, and class 53*C is 1883
    # lemmas: with the work unit that coarse, `-j` stops helping at about `-j6`
    # because one worker is stuck on `53*C` while the rest go idle. Splitting
    # each class into fixed-size chunks lets the scheduler keep every worker
    # busy and the wall clock fall as `total / j` again. The writer does not
    # care: a chunk inserts its own rows, and `INSERT OR REPLACE` on (lemma,
    # homonym) cannot collide across chunks since a word lives in exactly one.
    schedule: list[tuple[str, list[tuple[str, str, str]]]] = []
    for cls, words in plan:
        if args.chunk and len(words) > args.chunk:
            for i in range(0, len(words), args.chunk):
                schedule.append((cls, words[i:i + args.chunk]))
        else:
            schedule.append((cls, words))

    cells = len(sliced) + (len(COMPOUND_SPECS) if compounds else 0)
    total_jobs = len(schedule)
    if not args.quiet:
        eprint(f"{len(plan)} class(es), {total_words} words, {cells} cells each")
        eprint(f"{total_jobs} job(s) at -j {args.processes}")
        # Measured: the oracle pair runs at roughly 3.5 ms a cell, dominated by
        # the Voikko confirmation of each new surface form. The floor is the
        # biggest single job, which the chunk size bounds.
        biggest = max((len(w) for _, w in schedule), default=0)
        minutes = max(total_words * cells * 0.0035 / 60 / max(1, args.processes),
                      biggest * cells * 0.0035 / 60)
        eprint(f"estimating {minutes:.0f} min at -j {args.processes}")

    # A killed run -- the OOM killer takes the whole process group with no
    # traceback -- would otherwise lose every completed job, because the output
    # is deleted before the first insert. The journal records how many jobs of
    # *this* schedule are already in the file, and is written only after a job
    # commits, so `--resume` never skips work that is not there. A schedule of a
    # different shape (other flags) makes the count meaningless, so it is
    # ignored and the build starts over rather than trusting it.
    journal = Path(str(args.output) + ".progress")
    done = 0
    if args.resume and Path(args.output).exists() and journal.exists():
        try:
            saved = json.loads(journal.read_text())
        except (OSError, ValueError):
            saved = {}
        if saved.get("jobs") == total_jobs and saved.get("cells") == cells:
            done = min(int(saved.get("done", 0)), total_jobs)
    if done:
        eprint(f"resuming: {done}/{total_jobs} job(s) already written")
        schedule = schedule[done:]
    else:
        for stale in (args.output, journal):
            if Path(stale).exists():
                # A stale file would be read as this run's output, and a partial
                # one would be read as a complete one.
                os.unlink(stale)

    started = time.time()
    writer = SqliteWriter(args.output, started)

    def record(n: int) -> None:
        journal.write_text(json.dumps({"jobs": total_jobs, "cells": cells,
                                       "done": n}))

    jobs = max(1, min(args.processes, len(schedule)))
    if jobs == 1:
        _init_worker(sliced, compounds)
        for i, job in enumerate(schedule, 1):
            writer.add_class(build_class(job))
            record(done + i)
            progress(done + i, total_jobs, started, args.quiet)
    else:
        with ProcessPoolExecutor(max_workers=jobs, initializer=_init_worker,
                                 initargs=(sliced, compounds)) as pool:
            for i, result in enumerate(pool.map(build_class, schedule), 1):
                writer.add_class(result)
                record(done + i)
                progress(done + i, total_jobs, started, args.quiet)

    stats = writer.close()
    if journal.exists():
        os.unlink(journal)

    if args.engine and not args.per_class:
        wanted = None if args.classes == "all" else set(selected)
        try:
            extra = enrich_missing_verbs(
                args.output, wanted_classes=wanted,
                limit=args.engine_limit, dry_run=args.engine_dry_run,
            )
        except ImportError:
            eprint("warning: the torille binding is not built; the verb "
                   "lexicon was left as the Omorfi + Voikko grid.")
        else:
            verb = "would enrich" if args.engine_dry_run else "enriched"
            eprint(f"engine pass: {verb} {extra['processed']} missing verbs "
                   f"({extra['bases']} bases, {extra['inserted']} finite rows)")

    if not args.engine_dry_run:
        conn = write_connectives(args.output)
        eprint(f"connectives: {conn['after']} rows ({conn['before']} before)")
        if args.classes == "all" and not args.per_class:
            neg = write_connegatives(args.output)
            eprint(f"connegatives: {neg['derived']} derived, "
                   f"{neg['inserted']} new rows")

    if not args.quiet:
        mb = Path(args.output).stat().st_size / 1e6
        eprint(f"\n{stats['words']} words, {stats['rows']} rows over "
               f"{stats['lemmas']} lemmas in {len(plan)} classes")
        eprint(f"wrote {args.output} ({mb:.1f} MB) in "
               f"{time.time() - started:.0f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
