#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build a noun lexicon: every form of every classed noun, with its grammar.

The sanalista records a *class* for a noun, not its forms -- `talo` is class 1,
and being class 1 means having a paradigm. This asks the two external
authorities what that paradigm actually is, for every cell, and writes the
answer down:

* **Omorfi** decides which forms *exist*. It is asked one cell at a time and
  answers with the surface string, or with the empty string when the cell is not
  a word. A cell can have more than one right answer: the partitive possessive of
  `talo` is `taloaan/taloansa` and the translative is `talokseen/taloksensa`,
  and Finnish uses both.
* **Voikko** decides which of those answers are *real*. Its dictionary is
  finite, so a form Omorfi generates that Voikko does not read as belonging to
  the lemma is kept but marked `"ok": "omorfi"` rather than dropped.

Both answers are recorded, because "Omorfi says so" and "two independent
authorities agree" are different claims, and a lexicon that kept only the second
would throw away exactly the cells worth arguing about. `taloaan` is `both` even
though Voikko's dictionary is the one that has to be lucky; `allergikkooihin` is
a form this file never contains, because neither oracle produces it.

The unit of a row is the **form**, not the cell, because one form can answer
several cells: `kissani` is both the nominative singular and the genitive
singular first-person possessive. A row per cell would say the word twice and
lose the fact that they are one word -- which is also the fact a spell checker
needs, since it is one suggestion.

## Four output shapes, because the two files want different things

`--format sqlite` is the default and is the shape this is for, and it is **two
tables, and only two**:

    base:   lemma | homonym | part_of_speech | class
    forms:  lemma | homonym | form | case | number | possessive | ok

`base` is one row per word; `forms` is one row per form per cell, and joins to
`base` on `(lemma, homonym)` -- the key the source itself uses. The point of the
split is that the word's own facts are written once and not copied onto every
one of its ~150 forms, which is the repetition that makes the nested JSON three
quarters redundant bytes. Ask for a word, or a cell, and the database answers:

    -- every form of one word, with the class that owns it
    SELECT f.form, f."case", f.number, f.possessive, f.ok, b.class, b.part_of_speech
      FROM forms f JOIN base b USING (lemma, homonym)
     WHERE f.lemma = 'talo';

    -- what a class must be able to produce, in one query
    SELECT f."case", f.number FROM forms f JOIN base b USING (lemma, homonym)
      WHERE b.class = '41' AND f.ok = 'both' GROUP BY 1, 2;

The cell dimensions `case`, `number`, `possessive` are written out
(`nominative`, `singular`, `first person singular`) so there is no code table
and no third table for the grammar. `homonym` earns its two characters on the
join -- `ahtaus` is class 40 and class 39 at once, so `lemma` alone cannot say
which word a form belongs to; it is empty for the sole word. `part_of_speech`
is the whole Sanaluokka column, often more than one word -- `toinen` is
`numeraali, järjestysluku, pronomini, adjektiivi`. `class` is the whole
Taivutustiedot column, verbatim, so it can hold the rich shapes: `9, 50` is two
classes, `8, (21)` declines its plural like class 21.

`--format csv` is the same facts as a flat table (one row per form per case) and
`tsv` the same with tabs; they also write the base as a `.base.csv` companion
so the join survives in a format that has no join. `json` is the nested
document. All four are built from one run and hold the same rows.

The JSON is the right shape for a file that gets *reviewed*: a diff of it in a
commit shows exactly which form changed. The SQLite is the right shape for one
that gets *queried*. Three quarters of the JSON's bytes are the six key names
`case`/`number`/`possessive`/... repeated in every reading, and the complete
file has millions of readings; normalising them into columns removes that
repetition and turns the two questions anyone actually asks -- "every form of
`talo`", "every form in class 41" -- into one-liners:

    SELECT form, "case", number, possessive, ok
      FROM form_grammar WHERE lemma = 'talo';

The complete file is also the one that decides the format question on its own:
parsed as JSON it costs about 7 GB of resident memory before it can answer a
single question, against 12 GB of RAM on this machine.

Usage:
    python3 tools/build_nomine_lexicon.py --classes 27   # sqlite, subset, to stdout file
    python3 tools/build_nomine_lexicon.py -f csv -n 1 -o data/kotus_nomines_example.csv
    python3 tools/build_nomine_lexicon.py -o data/kotus_nomines.sqlite -j 4

`-n` limits the lemmas per class (default: all of them). `--classes` chooses
`all` (146 values, the class string exactly as torille reads it, so `9` and
`9*B` stay apart) or `numeric` (the 51 plain Kotus classes) or a comma list.
`--pos` chooses the Sanaluokka values to include -- `numeraali` gets the
numerals, which are recorded under the same noun classes as nouns. `--classes
all` with `-j 8` over every lemma builds the complete file in well under an
hour. `--possessives ''` drops the possessive dimension.
"""

from __future__ import annotations

import argparse
import json
import os
import random
import sqlite3
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from omorfi_harness import (  # noqa: E402
    ALL_CASES,
    ALL_POSSESSIVES,
    CASE_NAMES,
    SANALISTA,
    Harness,
    eprint,
    noun_cells,
)

#: What the two oracles said about a form. `both` is the only value that means
#: two independent authorities agree; `omorfi` means the word is in the
#: morphology database and no lexicon has been asked about it.
BOTH = "both"
OMORFI_ONLY = "omorfi"
ENGINE = "engine"

#: Parts of speech whose forms belong in the noun lexicon. The engine also
#: has classless compounds and numerals, handled by the enrichment pass below.
NOMINAL_POS = ("substantiivi", "adjektiivi", "numeraali", "pronomini")

#: The binding's possessive codes to the words the lexicon stores.
ENGINE_POSSESSIVE_WORDS = {
    "1sg": "first person singular",
    "2sg": "second person singular",
    "3": "third person",
    "1pl": "first person plural",
    "2pl": "second person plural",
}

#: Omorfi possessive code -> the word for it. Written out rather than derived
#: from the code, because the two sides spell a possessive in opposite order
#: (`POSS=SG1` in the omor string, `poss 1sg` in a reading) and only the
#: third-person one is the same in both.
POSSESSIVE_WORDS = {
    "": "",
    "SG1": "first person singular",
    "SG2": "second person singular",
    "PL1": "first person plural",
    "PL2": "second person plural",
    "3": "third person",
}

#: Spelled out to match the case names, so a row reads as grammar rather than
#: as abbreviations; the codes are kept alongside for anyone who wants to hand
#: the cell back to Omorfi.
NUMBER_WORDS = {"SG": "singular", "PL": "plural"}

#: Omorfi sometimes writes a spelling with U+2019 RIGHT SINGLE QUOTATION MARK
#: where the stem meets the ending: `luonnonlaiksi/luonnonla'iksi`, and for
#: `buffet` every possessive, where no unmarked twin is offered at all
#: (`buffet'ni`, `buffet'nsa`). The marker is a boundary in the *analysis*, not
#: a character in the word -- no Finnish word contains an apostrophe -- so it is
#: removed before the spelling is stored or offered to Voikko. Without this the
#: whole `buffet` possessive paradigm is missing, because every one of its 144
#: marked spellings is a spelling Voikko cannot possibly read.
BOUNDARY_MARKER = "’"

# Set in each worker by `_init_worker`; one Omorfi and one Voikko per process,
# which is the whole reason the parallelism is over processes. Both are C++
# objects with no promise of concurrent use, so the harness serialises their
# calls under a lock -- which means threads inside a process would buy nothing
# for the oracle half of the work, which is nearly all of it.
_HARNESS: Harness | None = None
_CELLS: list | None = None


def _init_worker(cases: list[str], possessives: list[str]) -> None:
    global _HARNESS, _CELLS
    _HARNESS = Harness("noun")
    _CELLS = noun_cells(cases, ["SG", "PL"], possessives)


def _class_order(name: str) -> tuple:
    """Kotus order: numeric classes in numeric order, then the `*` variants
    next to their base class rather than at the end."""
    base = name.split("*")[0]
    return (int(base) if base.isdigit() else 999, name)


def build_lemma(harness: Harness, lemma: str, cells: list) -> tuple[list[dict], str]:
    """Every form of one lemma, one row per form, plus a skip reason.

    A form answering two cells becomes one row with two readings, so the count
    of rows is the count of *words* the checker can be asked about rather than
    the count of cells it was asked about.
    """
    by_form: dict[str, dict] = {}
    # Which cells each form has already been credited with. Stripping the
    # boundary marker can make two of Omorfi's own spellings the same word --
    # `keskikookseen/keskiko'okseen` is one form offered twice -- and a cell
    # credited twice is a duplicate reading, not a second reading.
    credited: dict[str, set[tuple[str, str, str]]] = {}
    saw_any_cell = False

    for cell in cells:
        generated = harness.generate(lemma, cell)
        if generated:
            saw_any_cell = True
        key = (cell.case, cell.number, cell.possessive)
        for spelling in generated.split("/"):
            form = spelling.replace(BOUNDARY_MARKER, "").strip()
            if not form:
                continue
            row = by_form.get(form)
            if row is None:
                # A form's *word* is confirmed or it is not; a lexicon does not
                # vary by cell, so a later confirmation promotes every reading.
                confirmed = harness.voikko_ok(lemma, form)
                by_form[form] = row = {
                    "form": form,
                    "ok": BOTH if confirmed else OMORFI_ONLY,
                    "readings": [],
                }
                credited[form] = set()
            elif row["ok"] == OMORFI_ONLY and harness.voikko_ok(lemma, form):
                row["ok"] = BOTH
            if key in credited[form]:
                continue
            credited[form].add(key)
            row["readings"].append({
                "case": CASE_NAMES[cell.case],
                "number": NUMBER_WORDS[cell.number],
                "possessive": POSSESSIVE_WORDS.get(cell.possessive, cell.possessive),
                "case_code": cell.case,
                "number_code": cell.number,
                "possessive_code": cell.possessive,
            })

    if not saw_any_cell:
        return [], "no form in the oracle"
    return [by_form[f] for f in sorted(by_form)], ""


def build_class(job: tuple[str, list[tuple[str, str, str]]]) -> dict:
    """One class: every (lemma, homonym)'s forms, plus the words skipped.

    Runs in a worker process. The class is the unit because it is the unit the
    file is keyed on, and because a class is small enough to come back over the
    process boundary -- the largest, class 5, is 5896 lemmas and about 350k
    rows, which pickles without complaint.

    The two homonyms of a spelling are two *words*, so the key carries the
    homonym and every row carries it back out into the file. Where two rows
    would otherwise collide in the nested formats -- `ahtaus` is a word twice --
    the homonym disambiguates the key; in the flat table it is just a column.
    """
    name, words = job
    assert _HARNESS is not None and _CELLS is not None
    per_word: dict[str, list[dict]] = {}
    skipped: dict[str, str] = {}
    for lemma, homonym, pos in words:
        rows, reason = build_lemma(_HARNESS, lemma, _CELLS)
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


#: The columns of the flat table, in order. This is the shape the sanalista
#: itself uses -- one row per form per case, nothing nested -- and it is what a
#: reader (or a spreadsheet, or `grep`, or a diff) actually wants.
#:
#: `possessive`, `personal` and `singular` are 0/1 because a table column has no
#: third state and the question "does this form have a possessive suffix" is
#: yes-or-no. The three that are *not* yes-or-no are spelled out instead:
#:
#:   possessive           1 when a possessive suffix is attached at all
#:   personal             1 when that suffix names a speaker -- first or second
#:                        person. The third-person suffix is a possessive but
#:                        not a personal one, which is why this is a separate
#:                        column and not the same thing twice.
#:   person               1, 2 or 3, the possessor's person; empty when bare
#:   possessive_singular  1 when the *possessor* is singular. Empty when bare,
#:                        and empty for the third person, which marks no
#:                        number -- so it is empty, not 0, because "no number
#:                        is claimed" and "plural is claimed to be false" are
#:                        different claims and the column has to keep them apart.
#:   singular             1 when the *noun* is singular, which is a different
#:                        thing from the possessor's number: `taloini` is a
#:                        singular noun with a singular possessor and
#:                        `talomme` is a plural noun with a plural possessor.
# The flat table is **two** tables rather than one, because one of the two is
# a reference and the other is the data. The forms table is one row per form
# per case, and the base table is one row per word -- the four source fields,
# repeated once instead of on every form. To know "what class is this form",
# join them on (lemma, homonym):
#
#   base : lemma, homonym, part_of_speech, class
#   forms: lemma, homonym, form, case, ...
#   joined = base JOIN forms USING (lemma, homonym)
#
# `homonym` earns its two characters on the join: `ahtaus` is class 40 and
# class 39 at once, so lemma alone cannot identify which word a form belongs
# to. It is empty for the sole word, and the majority of rows carry it empty.
FORMS_COLUMNS = (
    "lemma", "homonym", "form", "case", "singular",
    "possessive", "personal", "person", "possessive_singular",
    "confirmed",
    "case_code", "number_code", "possessive_code",
)
BASE_COLUMNS = ("lemma", "homonym", "part_of_speech", "class")

#: Omorfi possessive code -> (has a possessive suffix?, possessor's person,
#: is the possessor singular?). `None` in the third slot means the number is
#: simply not claimed. The third person is a possessive but not a *personal*
#: one, so it is the only entry where the second column is derived rather than
#: copied -- see CSV_COLUMNS.
_POSSESSIVE_FACTS = {
    "":    (0, "",  None),
    "SG1": (1, "1", "1"),
    "SG2": (1, "2", "1"),
    "PL1": (1, "1", "0"),
    "PL2": (1, "2", "0"),
    "3":   (1, "3", None),
}


class CsvWriter:
    """Two flat tables: the forms, and the base next to them.

    The flat view of the same facts the other two formats hold, kept in two
    files so the word's own metadata is not copied onto every form. It is the
    shape that survives being pasted somewhere else: `cut -d, -f3,4` answers a
    question, `sort | uniq -c` counts a distribution, and a diff of two runs
    shows one changed word per line instead of a moved brace.
    """

    def __init__(self, path: str, meta: dict, started: float,
                 delimiter: str = ",", comments: bool = True) -> None:
        self.path = path
        self.started = started
        self.delimiter = delimiter
        # The forms table goes to the named output; the base table goes to the
        # same name with `.base` before the extension, so both exist for a
        # `JOIN` without a second flag to remember.
        self.out = sys.stdout if path == "-" else open(path, "w", encoding="utf-8")
        self.base_out = None
        if path != "-":
            stem = Path(path)
            base_path = stem.with_name(stem.stem + ".base" + stem.suffix)
            self.base_out = open(base_path, "w", encoding="utf-8")
        self.classes = 0
        self.forms = 0
        self.lemmas = 0
        self.skipped = 0
        self.rows = 0
        # The header is a comment above the columns rather than a row of data,
        # so the file stays a table and not a table with a preamble. A leading
        # `#` is ignored by every tool that reads a table, including a
        # spreadsheet's text import.
        if comments:
            note = ("# join the forms to the base on (lemma, homonym) to get "
                    "the class, part_of_speech and Taivutustiedot of a form\n")
            for out in (self.out, self.base_out):
                if out is None:
                    continue
                out.write("# " + meta["description"] + "\n")
                out.write("# generated_by " + meta["generated_by"]
                          + "  at " + meta["generated_at"] + "\n")
                out.write(note)
        self.out.write(delimiter.join(FORMS_COLUMNS) + "\n")
        if self.base_out is not None:
            self.base_out.write(delimiter.join(BASE_COLUMNS) + "\n")
        self._base_seen = set()

    def add_class(self, result: dict) -> None:
        cls = result["class"]
        self.classes += 1
        self.lemmas += len(result["lemmas"])
        self.skipped += len(result["skipped"])
        buf: list[str] = []
        base_buf: list[str] = []
        for key, forms in result["lemmas"].items():
            self.forms += len(forms)
            # The nested key may carry a `#homonym` suffix that is not part of
            # the word; the flat table restores the bare lemma and a column.
            lemma = key.split("#", 1)[0]
            homonym = key.partition("#")[2]
            pos = forms[0]["part_of_speech"]
            if key not in self._base_seen:
                self._base_seen.add(key)
                base_buf.append(self.delimiter.join((lemma, homonym, pos, cls)))
            for row in forms:
                for rd in row["readings"]:
                    possessive, person, psg = _POSSESSIVE_FACTS[rd["possessive_code"]]
                    buf.append(self.delimiter.join((
                        lemma, homonym, row["form"], rd["case"],
                        "1" if rd["number_code"] == "SG" else "0",
                        str(possessive),
                        "1" if possessive and person in ("1", "2") else "0",
                        person, "" if psg is None else psg,
                        row["ok"],
                        rd["case_code"], rd["number_code"], rd["possessive_code"],
                    )))
        if buf:
            self.out.write("\n".join(buf) + "\n")
        if base_buf and self.base_out is not None:
            self.base_out.write("\n".join(base_buf) + "\n")
        self.rows += len(buf)

    def close(self) -> dict:
        stats = {
            "class_values": self.classes,
            "lemmas": self.lemmas,
            "forms": self.forms,
            "readings": self.rows,
            "skipped_lemmas": self.skipped,
            "elapsed_seconds": round(time.time() - self.started, 1),
        }
        if self.out is not sys.stdout:
            self.out.close()
        if self.base_out is not None:
            self.base_out.close()
        return stats


class JsonWriter:
    """Streams one nested document, a class at a time.

    The header is written before the run because everything in it is knowable
    before it, and the counts that are not -- forms, readings, seconds -- go in
    a trailing `stats` object. That is what lets the class bodies stream: at
    `-j 8` over every lemma the whole document does not fit in memory twice.

    Kept because the example file has to be plain text. A diff of a JSON file
    in a commit shows which *form* changed; a diff of a SQLite file shows that
    a byte moved somewhere in the file.
    """

    def __init__(self, path: str, meta: dict, started: float) -> None:
        self.path = path
        self.started = started
        self.out = sys.stdout if path == "-" else open(path, "w", encoding="utf-8")
        self.first = True
        self.classes = 0
        self.forms = 0
        self.readings = 0
        self.lemmas = 0
        self.skipped = 0
        # The dumped object supplies its own opening brace; only the closing one
        # is dropped, so `"classes"` can be appended as a sibling rather than
        # nested inside `"meta"`.
        self.out.write(json.dumps({"meta": meta}, ensure_ascii=False, indent=1)[:-1])
        self.out.write(",\n \"classes\": {")

    def add_class(self, result: dict) -> None:
        for rows in result["lemmas"].values():
            self.forms += len(rows)
            self.readings += sum(len(r["readings"]) for r in rows)
        self.lemmas += len(result["lemmas"])
        self.skipped += len(result["skipped"])
        self.classes += 1
        self.out.write(("\n" if self.first else ",\n") + json.dumps(
            result["class"], ensure_ascii=False) + ": ")
        self.out.write(json.dumps({k: v for k, v in result.items() if k != "class"},
                                  ensure_ascii=False))
        self.first = False

    def close(self) -> dict:
        stats = {
            "class_values": self.classes,
            "lemmas": self.lemmas,
            # `forms` is the count of rows, which is the count of distinct
            # words; `readings` is larger because one word can answer two
            # cells, and the difference is the number of words with more than
            # one reading.
            "forms": self.forms,
            "readings": self.readings,
            "skipped_lemmas": self.skipped,
            "elapsed_seconds": round(time.time() - self.started, 1),
        }
        self.out.write("\n},\n \"stats\": ")
        self.out.write(json.dumps(stats, ensure_ascii=False, indent=1))
        self.out.write("\n}\n")
        if self.out is not sys.stdout:
            self.out.close()
        return stats


#: Two tables, and only two. One is the reference for a word, the other is the
#: paradigm. Everything the source knows about a word is in `base`, once; every
#: form it can take is in `forms`, and joins to `base` on (lemma, homonym) --
#: the key the source itself uses, because `ahtaus` is one spelling that is
#: class 40 and class 39 at once. A cell is (case, number, possessive), and the
#: three are columns in `forms` already spelled out, so there is no lookup-table
#: join to make and no third table to hold the grammar.
#:
#:     SELECT * FROM forms       WHERE lemma = 'talo';
#:     SELECT class, part_of_speech FROM base WHERE lemma = 'talo';
#:     SELECT DISTINCT class FROM base;
SCHEMA = """
PRAGMA journal_mode = OFF;      -- a build artifact, rebuilt from the oracles
PRAGMA synchronous = OFF;
PRAGMA temp_store = MEMORY;

-- One row per word: the four source fields, once each.
CREATE TABLE base (
    lemma          TEXT NOT NULL,
    homonym        TEXT NOT NULL DEFAULT '',
    part_of_speech TEXT NOT NULL,
    class          TEXT NOT NULL,
    PRIMARY KEY (lemma, homonym)
) WITHOUT ROWID;

-- One row per form per cell. `case`, `number`, `possessive` are written out
-- (`nominative`, `singular`, `first person singular`) so a reader does not need
-- a code table. A form answering two cells -- `kissani` is both NOM/SG and
-- GEN/SG -- is one row per cell, which is what it is: two cells, one word.
-- `ok` says whether both oracles agreed the word is real or only Omorfi made it.
CREATE TABLE forms (
    lemma      TEXT NOT NULL,
    homonym    TEXT NOT NULL DEFAULT '',
    form       TEXT NOT NULL,
    "case"     TEXT NOT NULL,
    number     TEXT NOT NULL,
    possessive TEXT NOT NULL DEFAULT '',
    ok         TEXT NOT NULL CHECK (ok IN ('both', 'omorfi', 'engine')),
    PRIMARY KEY (lemma, homonym, form, "case", number, possessive)
) WITHOUT ROWID;

-- Queries on `base` hit its primary key; queries on `forms` that start from a
-- word hit this index instead of scanning the whole paradigm table.
CREATE INDEX forms_lemma ON forms (lemma, homonym);
"""


class SqliteWriter:
    """Writes exactly two tables: `base` (one row per word) and `forms`.

    Both writers take `add_class` and `close` so `main` does not know which one
    it has. Classes arrive in Kotus order and are inserted as they come, so
    nothing larger than one class is ever held in memory on this side. The
    provenance the other formats keep in a header is deliberately not stored --
    the file is two tables, and two tables only, so a reader's mental model of
    it is `base` joined to `forms` and nothing else.
    """

    def __init__(self, path: str, cases: list[str], possessives: list[str],
                 meta: dict, started: float) -> None:
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
            # write the base row once, ahead of the loop over its forms
            pos = rows[0]["part_of_speech"]
            base_rows.append((lemma, homonym, pos, cls))
            for r in rows:
                for rd in r["readings"]:
                    # The three cell dimensions are already spelled out in the
                    # reading (`nominative`, `singular`, `first person
                    # singular`), so the row drops straight in with no code
                    # lookup and no third table.
                    form_rows.append((lemma, homonym, r["form"], rd["case"],
                                      rd["number"], rd["possessive"], r["ok"]))
        # One transaction per class: a crash costs the class in flight, not the
        # run, and a few-hundred-thousand-row class commits in well under a
        # second.
        self.conn.executemany("INSERT OR REPLACE INTO base VALUES (?, ?, ?, ?)",
                              base_rows)
        self.conn.executemany(
            "INSERT OR REPLACE INTO forms VALUES (?, ?, ?, ?, ?, ?, ?)",
            form_rows)
        self.conn.commit()

    def close(self) -> dict:
        """Finish the file and return the same counts the JSON writer reports.

        Counted with SQL rather than kept as it goes, so a caller that reads
        the file back gets exactly the numbers the run reported.
        """
        one = self.conn.execute(
            "SELECT (SELECT COUNT(*) FROM base), "
            "       (SELECT COUNT(*) FROM forms)"
        ).fetchone()
        stats = {
            "class_values": self.conn.execute(
                "SELECT COUNT(DISTINCT class) FROM base").fetchone()[0],
            "lemmas": one[0],
            "forms": one[1],
            "readings": self.conn.execute(
                "SELECT COUNT(*) FROM forms").fetchone()[0],
            "skipped_lemmas": 0,
            "elapsed_seconds": round(time.time() - self.started, 1),
        }
        self.conn.commit()
        self.conn.close()
        return stats


def read_records(positions: list[str]) -> list[tuple[str, str, str, str]]:
    """Every (class, lemma, homonym, part_of_speech) chosen from the sanalista.

    Read directly rather than through the engine's `words`, because the source
    is keyed on (lemma, homonym) and the engine dedupes a lemma to a single
    class. Two homonyms are two words: `ahtaus` is class 40 and class 39 at
    once, and dropping the homonym index here would quietly fuse them back into
    a wrong single paradigm. An empty `positions` means every row with a class.
    """
    out = []
    with open(SANALISTA, encoding="utf-8") as fh:
        next(fh, None)
        for line in fh:
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 4 or not fields[0] or not fields[3]:
                continue
            lemma, homonym, pos, cls = fields[0], fields[1], fields[2], fields[3]
            if positions and not any(p in pos for p in positions):
                continue
            out.append((cls, lemma, homonym, pos))
    return out


def class_numbers(class_str: str) -> set[int]:
    """The base class numbers in a `Taivutustiedot` string (`9, (21)` -> {9, 21})."""
    out: set[int] = set()
    for part in class_str.split(","):
        digits = ""
        for ch in part.strip().lstrip("("):
            if ch.isdigit():
                digits += ch
            else:
                break
        if digits:
            out.add(int(digits))
    return out


def enrich_with_engine(
    db_path: str,
    *,
    ok: str = ENGINE,
    upgrade: bool = False,
    wanted_classes: set[int] | None = None,
    limit: int = 0,
    dry_run: bool = False,
) -> dict[str, int]:
    """Add the engine's nominal forms and bases to an already-built oracle.

    The Omorfi + Voikko grid cannot cover forms only one authority knows, and
    it omits classless compounds entirely. This pass is additive: existing rows
    are kept, and `upgrade` promotes an `omorfi`-only row the engine agrees
    with. It runs as `build_nomine_lexicon.py --engine` (or `--engine-only`).
    """
    from torille import Engine

    con = sqlite3.connect(db_path)
    if ok == ENGINE:
        schema = con.execute(
            "SELECT sql FROM sqlite_master WHERE name='forms'"
        ).fetchone()
        if schema is None or "'engine'" not in (schema[0] or ""):
            raise SystemExit(
                f"{db_path}: this database predates the `ok='engine'` value. "
                "Rebuild it (or use --engine-ok omorfi/both) so engine rows "
                "cannot be mistaken for the ok='both' reference."
            )
    cur = con.cursor()
    rows = list(cur.execute(
        "SELECT lemma, homonym, part_of_speech, class FROM base"
    ))
    engine = Engine()

    def wanted(class_str: str) -> bool:
        return wanted_classes is None or bool(class_numbers(class_str) & wanted_classes)

    known = {lemma for lemma, _, _, _ in rows}
    added_bases = 0
    for pos_word in NOMINAL_POS:
        for lemma, class_str in engine.words(pos_word):
            if lemma in known or not wanted(class_str):
                continue
            known.add(lemma)
            rows.append((lemma, "", pos_word, class_str))
            added_bases += 1
            if not dry_run:
                cur.execute(
                    "INSERT OR IGNORE INTO base "
                    "(lemma, homonym, part_of_speech, class) VALUES (?,?,?,?)",
                    (lemma, "", pos_word, class_str),
                )

    # `words()` skips a lemma with no class, and the oracle has none of them
    # either. The engine derives two kinds: a classless numeral (`kuusitoista`
    # = 16) and a classless compound from its classed head (`periaate`).
    for line in Path(SANALISTA).read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        word, pos_word, class_str = parts[0].strip(), parts[2], parts[3].strip()
        if (not word or class_str or word in known
                or not any(p in pos_word for p in NOMINAL_POS)):
            continue
        if wanted_classes is not None:
            # Classless lemmas have no number to match; the enrichment is
            # scoped by class, so leave them for the full pass.
            continue
        try:
            if not engine.inflect(word):
                continue
        except ValueError:
            continue
        known.add(word)
        rows.append((word, "", pos_word, class_str))
        added_bases += 1
        if not dry_run:
            cur.execute(
                "INSERT OR IGNORE INTO base "
                "(lemma, homonym, part_of_speech, class) VALUES (?,?,?,?)",
                (word, "", pos_word, class_str),
            )

    lemmas = inserted = upgraded = 0
    for lemma, homonym, pos, class_str in rows:
        if not any(p in pos for p in NOMINAL_POS):
            continue
        if not wanted(class_str):
            continue
        if limit and lemmas >= limit:
            break
        lemmas += 1
        existing: dict[tuple[str, str, str, str], str] = {
            (f, c, n, p): ok
            for f, c, n, p, ok in cur.execute(
                'SELECT form, "case", number, possessive, ok FROM forms '
                "WHERE lemma=? AND homonym=?",
                (lemma, homonym),
            )
        }
        try:
            forms = engine.inflect(lemma)
        except ValueError:
            continue
        for f in forms:
            if f.case is None or f.number is None:
                continue
            if getattr(f, "negative", False) or f.clitic is not None:
                continue
            if f.confidence == "guessed":
                continue
            key = (
                f.word,
                f.case,
                f.number,
                ENGINE_POSSESSIVE_WORDS.get(f.possessive, "") if f.possessive else "",
            )
            current = existing.get(key)
            if current is None:
                inserted += 1
                if not dry_run:
                    cur.execute(
                        'INSERT OR IGNORE INTO forms '
                        '(lemma, homonym, form, "case", number, possessive, ok) '
                        "VALUES (?,?,?,?,?,?,?)",
                        (lemma, homonym, key[0], key[1], key[2], key[3], ok),
                    )
            elif upgrade and current == OMORFI_ONLY and ok == BOTH:
                upgraded += 1
                if not dry_run:
                    cur.execute(
                        'UPDATE forms SET ok=? WHERE lemma=? AND homonym=? AND '
                        'form=? AND "case"=? AND number=? AND possessive=?',
                        (ok, lemma, homonym, key[0], key[1], key[2], key[3]),
                    )

    if dry_run:
        con.rollback()
    else:
        con.commit()
    con.close()
    return {
        "lemmas": lemmas,
        "added_bases": added_bases,
        "inserted": inserted,
        "upgraded": upgraded,
    }


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("-o", "--output", default="-", help="output path, or - for stdout")
    p.add_argument("-f", "--format", choices=("sqlite", "csv", "tsv", "json"),
                   default="sqlite",
                   help="sqlite is the default: exactly two tables, `base` for "
                        "the word's own fields and `forms` for its paradigm. "
                        "csv/tsv are the same two tables as flat files, json a "
                        "nested document")
    p.add_argument("--no-comments", action="store_true",
                   help="omit the three '#' provenance lines above the header, "
                        "for a file that has to start with the column names")
    p.add_argument("-n", "--per-class", type=int, default=0,
                   help="lemmas per class, sampled (default: all of them)")
    p.add_argument("--seed", type=int, default=0, help="sample seed")
    p.add_argument("--classes", default="all",
                   help="'all' (146 values), 'numeric' (the 51 plain Kotus "
                        "classes), or a comma list")
    p.add_argument("--pos", default="substantiivi",
                   help="part(s) of speech to include, as in the Sanaluokka "
                        "column: 'substantiivi', 'numeraali', 'adjektiivi', "
                        "'pronomini', a comma list, or 'all' to take every "
                        "word that has a class")
    p.add_argument("--possessives", default="all",
                   help="'all', a comma list of omorfi codes, or '' for the "
                        "bare case/number grid")
    p.add_argument("-j", "--processes", type=int, default=1,
                   help="worker processes over classes")
    p.add_argument("-q", "--quiet", action="store_true")
    p.add_argument("--engine", dest="engine", action="store_true", default=True,
                   help="enrich the sqlite output with the engine's forms "
                        "(default; needs the torille Python binding)")
    p.add_argument("--no-engine", dest="engine", action="store_false",
                   help="leave the output as the Omorfi + Voikko grid only")
    p.add_argument("--engine-only", action="store_true",
                   help="enrich an existing sqlite file without rebuilding it")
    p.add_argument("--engine-ok", choices=(BOTH, OMORFI_ONLY, ENGINE), default=ENGINE,
                   help="the `ok` value written for engine-only rows (default: "
                        "engine, which keeps them out of the ok='both' reference)")
    p.add_argument("--engine-upgrade", action="store_true",
                   help="promote an existing omorfi-only row the engine agrees with")
    p.add_argument("--engine-limit", type=int, default=0,
                   help="stop the engine pass after N lemmas")
    p.add_argument("--engine-dry-run", action="store_true",
                   help="report the engine pass without writing")
    args = p.parse_args(argv)

    if args.format == "sqlite" and args.output == "-":
        # sqlite cannot stream to stdout; default to the artifact's home.
        args.output = str(Path(SANALISTA).parent / "kotus_nomines.sqlite")

    if args.engine_only:
        if args.format != "sqlite":
            p.error("--engine-only needs --format sqlite")
        if not Path(args.output).exists():
            p.error(f"{args.output}: not found")
        wanted = None
        if args.classes not in ("all", "numeric"):
            wanted = {int(c) for c in args.classes.split(",") if c.strip().isdigit()}
        try:
            stats = enrich_with_engine(
                args.output, ok=args.engine_ok, upgrade=args.engine_upgrade,
                wanted_classes=wanted, limit=args.engine_limit,
                dry_run=args.engine_dry_run,
            )
        except ImportError:
            print("the torille binding is not built:\n"
                  "  (cd bindings && env -u CONDA_PREFIX maturin develop --release)",
                  file=sys.stderr)
            return 2
        verb = "would enrich" if args.engine_dry_run else "enriched"
        print(f"{verb} {stats['lemmas']} lemmas ({stats['added_bases']} new bases): "
              f"{stats['inserted']} rows inserted, {stats['upgraded']} upgraded")
        return 0

    if args.possessives == "all":
        possessives = list(ALL_POSSESSIVES)
    elif args.possessives == "":
        possessives = [""]
    else:
        possessives = [s.strip() for s in args.possessives.split(",") if s.strip()]
    cases = list(ALL_CASES)

    positions = [p.strip() for p in args.pos.split(",") if p.strip()]
    if positions == ["all"]:
        positions = []
    records = read_records(positions)

    buckets: dict[str, list[tuple[str, str, str]]] = {}
    for cls, lemma, homonym, pos in records:
        key = (lemma, homonym)
        bucket = buckets.get(cls)
        if bucket is None:
            buckets[cls] = bucket = []
        if key not in {(x[0], x[1]) for x in bucket}:
            bucket.append((lemma, homonym, pos))

    if args.classes == "all":
        selected = sorted(buckets, key=_class_order)
    elif args.classes == "numeric":
        selected = sorted((c for c in buckets if c.isdigit() and "," not in c
                           and "(" not in c), key=_class_order)
    else:
        wanted = {c.strip() for c in args.classes.split(",") if c.strip()}
        selected = sorted((c for c in buckets if c in wanted), key=_class_order)

    rng = random.Random(args.seed)
    plan: list[tuple[str, list[tuple[str, str, str]]]] = []
    for cls in selected:
        words = sorted(buckets[cls])
        if args.per_class:
            # Sampled, not truncated: the head of an alphabetical list is
            # acronyms and abbreviations, which is a terrible sample of a class.
            rng.shuffle(words)
            words = words[: args.per_class]
        if words:
            plan.append((cls, words))

    cells = noun_cells(cases, ["SG", "PL"], possessives)
    total_lemmas = sum(len(w) for _, w in plan)
    if not args.quiet:
        eprint(f"{len(plan)} class(es), {total_lemmas} lemmas, {len(cells)} cells each")
        eprint(f"estimating at 250 ms/lemma single-process: "
               f"{total_lemmas * 0.25 / 60 / max(1, args.processes):.0f} min at -j {args.processes}")

    meta = {
        "description": "every confirmed form of every classed noun, with its "
                       "grammatical classification",
        "generated_by": "tools/build_nomine_lexicon.py",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "source": "data/nykysuomensanalista.csv",
        "oracles": {
            "omorfi": "decides which forms exist",
            "voikko": "decides which of those are real",
        },
        "ok_values": {
            BOTH: "both oracles agree the form is real",
            OMORFI_ONLY: "Omorfi generates it; Voikko's finite dictionary "
                         "does not have it",
        },
        "boundary_marker_removed": BOUNDARY_MARKER,
        "source_fields": {
            "lemma": "the headword (a spelling shared by two words in the "
                     "file is disambiguated by the homonym)",
            "homonym": "the source's Homonymia index, empty for the sole word",
            "part_of_speech": "the source's Sanaluokka column",
            "class": "the source's Taivutustiedot column, verbatim, so it can "
                     "hold the rich shapes: `9, 50` is two classes, `8, (21)` "
                     "declines its plural like class 21",
        },
        "parts_of_speech": positions or ["all"],
        "cells_per_lemma": len(cells),
        "cases": cases,
        "possessives": possessives,
        "class_values": len(plan),
        "lemmas": total_lemmas,
        "sampling": (f"{args.per_class} per class, seed {args.seed}"
                     if args.per_class else "every lemma in every class"),
    }

    started = time.time()
    if args.format in ("csv", "tsv"):
        writer = CsvWriter(args.output, meta, started,
                           "\t" if args.format == "tsv" else ",",
                           comments=not args.no_comments)
    elif args.format == "json":
        writer = JsonWriter(args.output, meta, started)
    else:
        if args.output == "-":
            p.error("--format sqlite needs a file path, not stdout")
        # A stale file would be read as this run's output, and a partial one
        # would be read as a complete one; neither is worth the bytes saved.
        if Path(args.output).exists():
            os.unlink(args.output)
        writer = SqliteWriter(args.output, cases, possessives, meta, started)

    jobs = max(1, min(args.processes, len(plan)))
    if jobs == 1:
        _init_worker(cases, possessives)
        for i, job in enumerate(plan, 1):
            writer.add_class(build_class(job))
            progress(i, len(plan), started, args.quiet)
    else:
        with ProcessPoolExecutor(max_workers=jobs, initializer=_init_worker,
                                 initargs=(cases, possessives)) as pool:
            # `map` yields in submission order, so the file is in Kotus class
            # order however the workers finish.
            for i, result in enumerate(pool.map(build_class, plan), 1):
                writer.add_class(result)
                progress(i, len(plan), started, args.quiet)

    stats = writer.close()

    if args.engine and args.format == "sqlite" and not args.per_class:
        wanted = None
        if args.classes != "all":
            wanted = set()
            for cls in selected:
                wanted |= class_numbers(cls)
        try:
            extra = enrich_with_engine(
                args.output, ok=args.engine_ok, upgrade=args.engine_upgrade,
                wanted_classes=wanted, limit=args.engine_limit,
                dry_run=args.engine_dry_run,
            )
        except ImportError:
            eprint("warning: the torille binding is not built; the lexicon was "
                   "left as the Omorfi + Voikko grid. Build it with "
                   "`(cd bindings && env -u CONDA_PREFIX maturin develop --release)`.")
        else:
            verb = "would enrich" if args.engine_dry_run else "enriched"
            eprint(f"engine pass: {verb} {extra['lemmas']} lemmas "
                   f"({extra['added_bases']} new bases), "
                   f"{extra['inserted']} rows inserted, "
                   f"{extra['upgraded']} upgraded")

    if not args.quiet:
        mb = (0 if args.output == "-" else Path(args.output).stat().st_size) / 1e6
        where = "stdout" if args.output == "-" else f"{args.output} ({mb:.1f} MB)"
        eprint(f"\n{stats['forms']} forms ({stats['readings']} rows) over "
               f"{stats['lemmas']} lemmas in {len(plan)} classes "
               f"({stats['skipped_lemmas']} skipped)")
        eprint(f"wrote {where} as {args.format} in {time.time() - started:.0f}s")
    return 0


def progress(i: int, total: int, started: float, quiet: bool) -> None:
    if quiet:
        return
    done = time.time() - started
    eta = done / i * (total - i)
    eprint(f"  [{i:3}/{total}] {done / 60:6.1f}m elapsed, eta {eta / 60:6.1f}m")


if __name__ == "__main__":
    sys.exit(main())
