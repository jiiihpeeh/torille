# SPDX-License-Identifier: GPL-3.0-or-later
"""Shared machinery for the Omorfi + Voikko harness scripts.

The oracle has two halves. **Omorfi** decides which forms exist: it is asked for
one (case, number) cell at a time and answers with the surface string, or with
nothing when the cell is not a word. **Voikko** decides which of those answers
are real: its dictionary is finite, so a form Omorfi generates that Voikko does
not recognise as belonging to the lemma is not authoritative and is dropped.
What survives both is the word's actual paradigm, and that set is compared
against what torille inflects.

The grid is the whole paradigm, possessives included. That is the point: the
consumer of `inflect` is a spell checker, so *every* form it offers as plain has
to be judged against the oracle rather than the 26 bare case/number cells. A
harness that stops at the bare grid checks 25 of the 121 plain forms `kissa`
offers and calls it clean.

The comparison is delicate about *scope* in two places:

* torille emits forms outside the grid - for verbs the negative (`ei laske`),
  the potential, the perfect compound, and for nouns the comitative
  (`kissoineen`), which is a productive `instructive`+`abessive` pair rather
  than a case. Those are not generated, so counting one of them as an illegal
  plain form would fail every word for the wrong reason. `in_scope` decides.
* the two sides spell their features differently, and not only in the verb
  features: Omorfi's omor string uses `MOOD=INDV`/`PERS=SG1`, and its possessive
  is `POSS=SG1` where torille reports `poss 1sg`. The maps below are the only
  place that is reconciled, and getting one backwards is silent -- an omor
  string that names a feature nobody has parses fine and answers with the empty
  string, so the run just confirms fewer forms and reports no error anywhere.

**Cost.** Both halves are loaded once and shared, which is the whole reason a
wordlist run is affordable:

* torille is reached through the `torille` extension module, which holds the
  Kotus data in one place instead of reparsing the catalogue and the sanalista
  for every lemma. Inflecting a noun is tens of microseconds; reloading the
  data to do it is 230ms. Without the extension the harness falls back to the
  `inflect` binary and is correct but two orders of magnitude slower.
* Omorfi costs about 7.5s to load, and a generate call is 15us, so it is loaded
  *once* and its calls are serialised on a lock. The old harness built one per
  thread, so `-j 8` paid the 7.5s eight times over: that, not the
  morphology, was most of a wordlist run.
"""

from __future__ import annotations

import os
import sqlite3
import subprocess
import sys
import threading
from dataclasses import dataclass, field
from pathlib import Path

from pyomorfi import load_omorfi
from voikko import libvoikko

# torille itself is reached through its own extension module, `Engine`, which is
# imported lazily inside `_load_engine()` rather than here. That import is
# deliberately not at the top: the binding is optional, and a top-level import
# of a module that may not be built would take the `inflect` binary fallback
# down with it. See `_load_engine` for how the two paths are chosen.

REPO = Path(__file__).resolve().parent.parent
SANALISTA = REPO / "data" / "nykysuomensanalista.csv"

CONFIDENCES = ("plain", "repaired", "guessed")

#: Omorfi case code -> the case name torille reports.
CASE_NAMES = {
    "NOM": "nominative",
    "GEN": "genitive",
    "PAR": "partitive",
    "ESS": "essive",
    "TRA": "translative",
    "INE": "inessive",
    "ELA": "elative",
    "ILL": "illative",
    "ADE": "adessive",
    "ABL": "ablative",
    "ALL": "allative",
    "ABE": "abessive",
    "INS": "instructive",
}
ALL_CASES = list(CASE_NAMES)
NUMBER_NAMES = {"SG": "singular", "PL": "plural"}

def _first_attr(raw: str, name: str) -> str | None:
    """The value of the first `[NAME=value]` in an omor analysis string."""
    start = raw.find(f"[{name}=")
    if start < 0:
        return None
    end = raw.find("]", start)
    if end < 0:
        return None
    return raw[start + len(name) + 2:end]


#: The verb vocabularies, in the spelling each side uses. torille reports
#: `first person` where the omor string has `SG1`. These two maps are the scope
#: mapping: only the moods and tenses the finite grid actually generates.
VERB_MOODS = {"indicative": "INDV", "conditional": "COND"}
VERB_PERSONS = {"first person": "1", "second person": "2", "third person": "3"}
VERB_TENSES = {"present": "PRESENT", "past": "PAST"}

#: Every value torille prints in each field, for recovering a reading from the
#: printed words when the extension module is not built. Wider than the maps
#: above on purpose: recovery has to recognise a field to place it correctly,
#: including the ones the finite grid never asks for. A token outside its
#: vocabulary is an absent field, not a shifted one; see `_form_from_tags`.
#: `comitative` and `prolative` are the instructive and abessive together.
CASE_NAMES_ALL = tuple(CASE_NAMES.values()) + ("comitative", "prolative")
TENSE_NAMES = ("present", "past", "perfect", "pluperfect")
MOOD_NAMES = ("indicative", "conditional", "imperative", "potential",
              "infinitive", "instructive")
VOICE_NAMES = ("active", "passive", "middle")
#: The person words, without the ` person` that follows them in the output.
PERSON_WORDS = ("first", "second", "third")
#: The possessive suffixes, as torille prints them after `poss`.
POSSESSIVE_NAMES = ("1sg", "2sg", "3", "1pl", "2pl")
#: Every part of speech torille can print, so a leading word that is not one is
#: a case or a number rather than a part of speech.
POS_NAMES = (
    "unknown", "noun", "adjective", "pronoun", "numeral", "ordinal", "verb",
    "negative verb", "adverb", "conjunction", "interjection", "preposition",
    "postposition", "other",
)


#: Omorfi possessive code -> the suffix torille reports. The two sides spell a
#: possessive in opposite order -- `POSS=SG1` in the omor string, `poss 1sg` in
#: torille's reading -- and only the third-person one, `3`, is the same in both.
#: Getting this backwards is silent: the omor string parses, answers with the
#: empty string for every cell, and the harness reports 63 confirmed forms
#: instead of 156 with no error anywhere.
POSSESSIVE_NAMES_MAP = {
    "SG1": "1sg",
    "SG2": "2sg",
    "PL1": "1pl",
    "PL2": "2pl",
    "3": "3",
}
#: Every possessive, in the order the grid builds them, as Omorfi spells them.
#: Empty string first: the bare case/number cells, which are the paradigm core.
ALL_POSSESSIVES = ["", "SG1", "SG2", "PL1", "PL2", "3"]
#: The same five in words, for a report's prose. `3` alone reads as a number
#: rather than a person, so a report saying "the 3 possessive the cell is" is
#: worse than useless.
POSSESSIVE_TEXT = {
    "SG1": "first person singular",
    "SG2": "second person singular",
    "PL1": "first person plural",
    "PL2": "second person plural",
    "3": "third person",
}
#: The same, for a verb cell's person field, which omorfi and the grid both
#: write as a bare `1`/`2`/`3`.
ORDINAL_PERSON = {"1": "first person", "2": "second person", "3": "third person"}

#: The Finnish vowels, for recognising the short third-person possessive.
_VOWELS = "aeiouyäö"


def third_person_base(form: str | None) -> str | None:
    """The stem a third-person possessive form attaches to, or `None`.

    The third person is spelled two ways and **both are standard Finnish**: the
    long `-nsa`/`-nsä` and the short form that doubles the case's final vowel
    and adds `n` -- `taloansa`/`taloaan`, `akkaansa`/`akkaaan`,
    `taloksensa`/`talokseen`. They answer the same cell, so the two strings
    share a base: stripping the ending from either leaves `taloa`, `akkaa`,
    `talokse`.

    Voikko accepts both spellings wherever its dictionary reaches, but that
    dictionary is finite and word-by-word it often holds only one of the pair.
    A cell whose confirmed list holds one spelling is therefore not made wrong
    by the engine offering the other; comparing bases is how the harness tells
    "the same word, spelled the other way" from a genuinely different form.
    """
    if not form:
        return None
    if form.endswith(("nsa", "nsä")):
        return form[:-3]
    if (form.endswith("n") and len(form) >= 3
            and form[-2] in _VOWELS and form[-3] == form[-2]):
        # base + base[-1] + `n`: the doubled vowel is the base's last one.
        return form[:-2]
    return None


@dataclass(frozen=True)
class Cell:
    """One paradigm cell: (case, number, possessive), or for a verb
    (tense, mood, person, number).

    The possessive is part of the cell rather than decoration on the form
    because a possessive is a paradigm cell in its own right. `kissani` is a
    word, and the point of the harness is that every word torille offers as
    plain gets checked against the oracle -- so the cell the oracle generates
    has to name the possessive or the form has nowhere to be confirmed.
    """

    case: str = ""
    number: str = ""
    tense: str = ""
    mood: str = ""
    person: str = ""
    possessive: str = ""

    def describe(self) -> str:
        """The cell in words, for a report's `detail` field.

        Omorfi's abbreviations are compact and a report full of `ESS/SG/3` is a
        report nobody reads. The long names come from the tables already in this
        module, so the prose cannot drift from the spelling the cell was built
        with. A verb cell spells out the person and the number as one phrase,
        because `3 PL` means "third person plural" and reading it as two fields
        is how a report ends up describing a form as belonging to no one.
        """
        if self.mood or self.tense:
            bits = [b for b in (self.mood, self.tense) if b]
            if self.person and self.number:
                bits.append(
                    f"{ORDINAL_PERSON.get(self.person, self.person)} "
                    f"{NUMBER_NAMES.get(self.number, self.number)}"
                )
            else:
                bits += [b for b in (self.person, self.number) if b]
            return " ".join(bits)
        bits = [CASE_NAMES.get(self.case, self.case),
                NUMBER_NAMES.get(self.number, self.number)]
        if self.possessive:
            bits.append(POSSESSIVE_TEXT.get(self.possessive, self.possessive))
            bits.append("possessive")
        return " ".join(b for b in bits if b)


@dataclass
class TorilleForm:
    """One form as torille reads it.

    The reading is held as named fields rather than the word list the `inflect`
    binary prints, because that list is whitespace to be split and position to
    be counted -- `poss 1pl` and `third person` are each two tokens in the
    printed output, so a caller parsing it silently depends on the layout. The
    extension module reports the fields directly and `torille_forms` fills them
    in; the `tags` field is kept only for the fallback path and for messages.
    """

    word: str
    confidence: str
    pos: str | None = None
    case: str | None = None
    number: str | None = None
    tense: str | None = None
    person: str | None = None
    mood: str | None = None
    voice: str | None = None
    possessive: str | None = None
    negative: bool = False
    tags: list[str] = field(default_factory=list)

    @property
    def plain(self) -> bool:
        return self.confidence == "plain"


@dataclass
class Finding:
    """One cell's outcome for one lemma, with the reading that explains it.

    A per-class report has to answer "what does class 27 get wrong", and that
    question is about *cells*, not about words. `käessä` being illegal is a
    statement about the fixed-stem class; which reading torille gave it, and
    what the oracle thought that cell should be, is what makes the statement
    actionable. Both are here.

    `status` is one of the four `Verdict` kinds, or `confirmed` for a cell whose
    form torille got right. `expected` is what Omorfi generated, which can be
    two strings for one cell (`kissakseen/kissaksensa`); `got` is what torille
    produced for that cell, or "" when it produced nothing.
    """

    lemma: str
    cell: Cell
    status: str
    form: str = ""
    expected: str = ""
    got: str = ""
    confidence: str = ""
    tags: list[str] = field(default_factory=list)
    detail: str = ""
    #: Every cell that expects this form. More than one is normal -- Omorfi
    #: generates `kissani` for both NOM/SG/SG1 and GEN/SG/SG1, because the
    #: nominative and the genitive singular of a noun are the same word. It is
    #: also how a reading with a field missing is reported: `kissani` is tagged
    #: with no case, so the cells it could satisfy are the candidates, and
    #: listing them is honest where picking one is a guess.
    candidates: list[Cell] = field(default_factory=list)


@dataclass
class Verdict:
    """The outcome for one lemma.

    The four kinds are kept apart because they mean different things, and
    conflating them hides the interesting one.

    `missing` is a confirmed form torille does not produce at all. `not_plain`
    is one it produces but does not offer as plain. `illegal` is a form it
    *does* offer as plain that the oracle does not confirm -- the precision
    failure, and the one the old harness never looked for. `mistagged` is a
    confirmed form that is offered as plain but filed under the wrong cell, so
    the recall direction passes while the tagging is still wrong.

    Recall is deliberately *not* tag-scoped. A form that is offered as plain has
    been offered, and whether its tag is right is a separate question: scoping
    recall would report a mistagged form as missing and lose the distinction.
    Only the precision direction is scoped, because without a scope every
    possessive and every `ei` form would count as an illegal plain form.
    """

    lemma: str
    confirmed: set[str] = field(default_factory=set)
    missing: set[str] = field(default_factory=set)
    not_plain: set[str] = field(default_factory=set)
    illegal: set[str] = field(default_factory=set)
    mistagged: set[str] = field(default_factory=set)
    skipped: str = ""

    @property
    def ok(self) -> bool:
        return not (self.missing or self.not_plain or self.illegal) and not self.skipped

    def report(self) -> str:
        if self.skipped:
            return f"{self.lemma}: skipped ({self.skipped})"
        bits = []
        if self.missing:
            bits.append(f"{len(self.missing)} missing")
        if self.not_plain:
            bits.append(f"{len(self.not_plain)} not plain")
        if self.illegal:
            bits.append(f"{len(self.illegal)} illegal in plain")
        if self.mistagged:
            bits.append(f"{len(self.mistagged)} mistagged")
        tail = ", ".join(bits) if bits else f"all {len(self.confirmed)} confirmed forms plain"
        return f"{self.lemma}: {tail}"


def _load_engine():
    """The `torille` extension module's `Engine`, or `None` if it is not built.

    `bindings/` holds the binding. To build it into the current environment:

        (cd bindings && env -u CONDA_PREFIX maturin develop)

    `maturin` refuses to run when both `VIRTUAL_ENV` and `CONDA_PREFIX` are set,
    which is why the unset is in the command. Without the binding the harness
    still runs, through the `inflect` binary, which is correct and about 180
    times slower per word.

    `TORILLE_NO_EXTENSION=1` forces the binary path, so the fallback can be
    checked against the binding rather than assumed to agree with it.
    """
    if os.environ.get("TORILLE_NO_EXTENSION") == "1":
        return None
    try:
        from torille import Engine
    except ImportError:
        return None
    return Engine


def inflect_binary() -> str:
    """Path to the `inflect` binary, built once, for the fallback path.

    Only used when the extension module is not importable. A release build is
    preferred and asked for: the binary parses the Kotus data on every
    invocation, which is 50ms of a 54ms invocation optimised and 220ms of a
    232ms one unoptimised, so the fallback's cost is almost entirely a build
    profile question.
    """
    override = os.environ.get("TORILLE_INFLECT")
    if override:
        return override
    subprocess.run(
        ["cargo", "build", "-q", "--release", "-p", "torille-tools", "--bin", "inflect"],
        cwd=REPO, check=True,
    )
    built = REPO / "target" / "release" / "inflect"
    if not built.exists():
        raise SystemExit(f"{built} was not produced by the build")
    return str(built)


#: Omorfi sometimes writes a generated spelling with U+2019 RIGHT SINGLE
#: QUOTATION MARK where the stem meets the ending: `luonnonlaiksi` comes back as
#: `luonnonlaiksi/luonnonla'iksi`, and `buffet` has no unmarked twin at all for
#: any of its 144 possessive cells. The marker is a boundary in the analysis,
#: not a character in the word, so it is removed before the spelling reaches
#: Voikko -- otherwise `buffet'nsa` is a spelling the dictionary cannot read and
#: the whole paradigm reads as unattested.
BOUNDARY_MARKER = "\u2019"


class Harness:
    """Omorfi, Voikko and torille, loaded once and shared across threads."""

    def __init__(self, pos: str = "noun", lexicon: "str | os.PathLike | None" = None,
                 load_torille: bool = True) -> None:
        self.pos = pos
        # Cache of the omorfi feature-prefix (WORD_ID/UPOS/NUMTYPE/blacklist)
        # for each lemma, discovered from its base-form reading. Generation
        # answers to the word's *own* UPOS (`kaksi` is NUM, `eräs` PRON,
        # `talo` NOUN), and to the short-compound blacklist marker
        # (`suo` -> `suon` only with BLACKLIST=TOOSHORTFORCOMPOUND present).
        # The old code hardcoded `[UPOS=NOUN]` for every lemma, which silently
        # returned the empty string for numerals, pronouns and the class-19
        # short compounds -- a 0-forms build with no error anywhere.
        self._feat_prefix: dict[str, str | None] = {}
        if load_torille:
            engine_cls = _load_engine()
            if engine_cls is not None:
                self.engine = engine_cls()
                self.binary = None
            else:
                self.engine = None
                self.binary = inflect_binary()
        else:
            # The oracle half needs neither torille nor its Kotus data. A build
            # that only generates and confirms forms should not pay the ~3 GB
            # that instantiating `Engine` costs, especially six times over: it
            # is the difference between a worker being 0.8 GB and 3.7 GB.
            self.engine = None
            self.binary = None
        if lexicon is None:
            self.lexicon = None
            self.omorfi = load_omorfi(load_segmenter=False)
            # One Omorfi for the whole process, and one Voikko. Both are C++
            # objects with no promise of concurrent use, so every call is
            # serialised; the calls are microseconds and the load is seconds,
            # which settles it.
            self._lock = threading.Lock()
            self.voikko = libvoikko.Voikko("fi")
        else:
            # A lexicon reference is `confirmed_forms` already materialised
            # (`ok='both'` in data/kotus_nomines.sqlite, see docs/lexicon.md),
            # so Omorfi and Voikko never need to be loaded. The reference set is
            # read back with a query instead of live morphology. What is traded
            # is the `cells` argument: the lexicon holds the full grid, so this
            # path answers for the whole paradigm, not a named subgrid.
            self.lexicon = sqlite3.connect(str(lexicon), check_same_thread=False)
            self.omorfi = None
            self.voikko = None
            self._lock = threading.Lock()
        self._known_words, self._known_nouns, self._classed = self._load_sanalista()

    # -- sanalista ---------------------------------------------------------
    def _load_sanalista(self) -> tuple[set[str], set[str], set[str]]:
        words: set[str] = set()
        nouns: set[str] = set()
        classed: set[str] = set()
        with open(SANALISTA, encoding="utf-8") as fh:
            next(fh, None)  # header
            for line in fh:
                parts = line.rstrip("\n").split("\t")
                if len(parts) >= 3:
                    words.add(parts[0])
                    if "substantiivi" in parts[2]:
                        nouns.add(parts[0])
                if len(parts) >= 4 and parts[0] and parts[3]:
                    classed.add(parts[0])
        return words, nouns, classed

    def is_compound(self, word: str) -> bool:
        """Whether the word is a compound, which inflects on its head.

        A compound is a known modifier plus a noun head, so some split of it
        has a known word in front and a listed noun after. Both sides of the
        comparison inflect only the head, so the word is left out rather than
        reported as a mass of missing forms.

        This is a heuristic and it is not complete: Finnish compounds join with
        a linking consonant, so `aamu` + `sarastus` is written `aamunsarastus`
        and no split of the string puts a listed word on either side of the
        `n`. `has_class` is the exact test for the same thing and is checked
        first; this remains only for the case where the sanalista does list a
        class for a word that is nonetheless a compound.
        """
        for i in range(1, len(word)):
            if word[:i] in self._known_words and word[i:] in self._known_nouns:
                return True
        return False

    def has_class(self, word: str) -> bool:
        """Whether the sanalista gives the word a class to inflect with.

        Exact, and better than guessing at compounds: the sanalista records no
        class at all for a compound or an indeclinable, because both inflect on
        something else and neither has a paradigm of its own. Without a class
        there is nothing for torille to generate and the word cannot be checked
        at all, so it is skipped rather than reported as a mass of missing
        forms.
        """
        if self.engine is not None:
            return any(classes for classes, _pos in self.engine.lookup(word))
        return word in self._classed

    # -- the oracle --------------------------------------------------------
    def class_of(self, lemma: str) -> str:
        """The inflection class the sanalista gives the word, or "".

        Read through torille's own parser rather than by splitting the sanalista
        again, so `9`, `9*B` and `9, 50` are decided the same way here as they
        are there. A word with no class cannot be inflected at all.
        """
        if self.engine is not None:
            for classes, _pos in self.engine.lookup(lemma):
                if classes:
                    return classes
            return ""
        with open(SANALISTA, encoding="utf-8") as fh:
            next(fh, None)
            for line in fh:
                parts = line.rstrip("\n").split("\t")
                if parts and parts[0] == lemma:
                    return parts[3] if len(parts) >= 4 else ""
        return ""

    def words_in_class(self, part_of_speech: str = "substantiivi") -> dict[str, list[str]]:
        """Every class the sanalista uses for this part of speech, to its words.

        The class value is torille's, so `9` and `9*B` are separate keys. That
        is deliberate: the `*B` is a gradation, so `kauppa` (`9*B`, consonant
        gradation) has different forms from `kala` (`9`, no gradation), and a
        report that lumped them would report every `kaupan` as a defect.
        """
        buckets: dict[str, list[str]] = {}
        if self.engine is not None:
            for word, cls in self.engine.words(part_of_speech):
                buckets.setdefault(cls, []).append(word)
        else:
            with open(SANALISTA, encoding="utf-8") as fh:
                next(fh, None)
                for line in fh:
                    parts = line.rstrip("\n").split("\t")
                    if len(parts) >= 4 and parts[0] and parts[3] and part_of_speech in parts[2]:
                        buckets.setdefault(parts[3], []).append(parts[0])
        return buckets

    def confirmed_forms(self, lemma: str, cells: list[Cell]) -> set[str]:
        """The forms Omorfi generates for these cells that Voikko confirms.

        With a lexicon reference (see `__init__`) this is the same set read back
        from the database's `ok='both'` rows, which is exactly what the live
        pair would answer -- at a query's cost instead of a morphology load. The
        `cells` argument is honoured only on the live path; the lexicon holds
        the whole grid, so it returns the full confirmed paradigm.
        """
        if self.lexicon is not None:
            rows = self.lexicon.execute(
                "SELECT DISTINCT form FROM forms WHERE lemma=? AND ok='both'",
                (lemma,),
            ).fetchall()
            return {r[0] for r in rows}
        out: set[str] = set()
        for cell in cells:
            for form in self.generate(lemma, cell).split("/"):
                form = form.strip()
                if form and self.voikko_ok(lemma, form):
                    out.add(form)
        return out

    def _omorfi_prefix(self, lemma: str) -> str | None:
        """The `[WORD_ID=..][UPOS=..][NUMTYPE=..][BLACKLIST=..]` prefix to
        generate `lemma`'s forms, discovered from its base-form reading.

        Omorfi's generator answers to the word's own part of speech: `talo` is
        NOUN, `kaksi` NUM (CARD), `eräs` PRON. The old code padded every lemma
        with `[UPOS=NOUN]`, which works for nouns but returns nothing for
        numerals and pronouns. It also needs the short-compound blacklist
        marker exactly where the analyser has it -- `suo`'s reading carries
        `BLACKLIST=TOOSHORTFORCOMPOUND` and generates only with it present.

        Returns None when the lemma has no base form in Omorfi at all; the
        caller then reports an empty generation rather than lettering a wrong
        prefix produce forms that are not there.
        """
        cached = self._feat_prefix.get(lemma, ...)
        if cached is not ...:
            return cached
        prefix = None
        with self._lock:
            readings = self.omorfi.analyse(lemma)
        for analysis in readings:
            raw = getattr(analysis, "raw", None)
            if not raw:
                continue
            if "[NUM=SG]" not in raw or "[CASE=NOM]" not in raw:
                continue
            if "[POSS=" in raw:
                continue
            bits = []
            wid = _first_attr(raw, "WORD_ID")
            upos = _first_attr(raw, "UPOS")
            if wid is None or upos is None:
                continue
            bits.append(f"[WORD_ID={wid}][UPOS={upos}]")
            # Keep every disambiguating attribute from the base reading --
            # NUMTYPE for numerals, CMP for adjectives (`ahkera` only generates
            # with `CMP=POS`), PRONTYPE for pronouns (`eräs` needs `PRONTYPE=IND`,
            # not its `SUBCAT=QUANTIFIER` base), and so on. Everything but the
            # inflectional bits (NUM, CASE, POSS) and WEIGHT is preserved, so a
            # generation reading is never padded with a feature it does not
            # carry. `BLACKLIST` is a flag that must come before NUM/CASE, so it
            # is hoisted ahead of the rest.
            attr_start = raw.find("[")
            while attr_start >= 0:
                attr_end = raw.find("]", attr_start)
                if attr_end < 0:
                    break
                attr = raw[attr_start:attr_end + 1]
                name = attr[1:].split("=", 1)[0]
                if name == "BLACKLIST":
                    bits.append(attr)
                elif name not in ("NUM", "CASE", "POSS", "WEIGHT", "WORD_ID",
                                  "UPOS"):
                    bits.append(attr)
                attr_start = raw.find("[", attr_end)
            candidate = "".join(bits)
            # Verify the reading generates *the lemma itself* as its base form.
            # Two wrong readings must be rejected here: a reading that pads a
            # word with features no generation reading carries (answers with
            # the empty string for every cell), and a compound-head reading --
            # `aaltomainen`'s first analysis is `[WORD_ID=aalto]...[BOUNDARY=
            # COMPOUND][WORD_ID=mainen]`, which, if it generated, would file
            # `aalto`'s forms under `aaltomainen`. Requiring the NOM/SG
            # generation to be the lemma keeps both out: the lexeme's own
            # reading comes back `aaltomainen`, the compound head cannot.
            nom = self.omorfi.generate(f"{candidate}[NUM=SG][CASE=NOM]")
            forms = nom.split("/") if isinstance(nom, str) else nom
            if lemma in forms:
                prefix = candidate
                break
        self._feat_prefix[lemma] = prefix
        return prefix

    def generate(self, lemma: str, cell: Cell) -> str:
        if self.pos == "noun":
            prefix = self._omorfi_prefix(lemma)
            if prefix is None:
                return ""
            feats = f"{prefix}[NUM={cell.number}][CASE={cell.case}]"
            if cell.possessive:
                # The omor string wants `POSS=SG1`, not the `1sg` torille
                # reports. See POSSESSIVE_NAMES_MAP.
                feats += f"[POSS={cell.possessive}]"
        else:
            # Omorfi's omor feature names, which are not the UD names `ufeats`
            # reports; generation only answers to these. `PERS` carries the
            # number with the person: `SG1`, `PL3`.
            person = f"{cell.number}{cell.person}"
            bits = [f"[WORD_ID={lemma}][UPOS=VERB]", "[VOICE=ACT]", f"[MOOD={cell.mood}]"]
            if cell.tense:
                bits.append(f"[TENSE={cell.tense}]")
            bits.append(f"[PERS={person}]")
            feats = "".join(bits)
        with self._lock:
            result = self.omorfi.generate(feats)
        return result if isinstance(result, str) else ""

    def analyse_raw(self, word: str) -> list[str]:
        """Every raw `[KEY=VALUE]` omor string Omorfi reads for `word`.

        The generator is driven by tag strings, and those tags are not the ones
        this module's maps use: the omor feature names (`MOOD=INDV`) are the
        ones generation answers to, and the only way to learn the exact set a
        word carries is to read its own analyses back. The lexicon builder uses
        this to discover the `[WORD_ID=..][UPOS=..]` prefix a verb generates
        from, the same way `_omorfi_prefix` does for a noun.
        """
        with self._lock:
            analyses = self.omorfi.analyse(word)
        return [raw for a in analyses if (raw := getattr(a, "raw", None))]

    def generate_tags(self, prefix: str, tags: str) -> str:
        """Generate from an explicit tag string.

        `generate` picks the tags for a [`Cell`] and cannot say the richer verb
        readings (the infinitives, the participles, `NEG=CON`). This is the raw
        call either side of that: `prefix` is the `[WORD_ID=..][UPOS=..]` head
        `analyse_raw` found, `tags` the inflectional tail appended to it. The
        empty string means the reading is not a form of the word.
        """
        with self._lock:
            result = self.omorfi.generate(f"{prefix}{tags}")
        return result if isinstance(result, str) else ""

    def voikko_ok(self, lemma: str, form: str) -> bool:
        with self._lock:
            analyses = self.voikko.analyze(form)
        for analysis in analyses:
            if analysis.get("BASEFORM") == lemma or lemma in analysis.get("WORDBASES", ""):
                return True
        return False

    # -- torille -----------------------------------------------------------
    def torille_forms(self, lemma: str,
                      klass: str | None = None) -> list[TorilleForm]:
        if self.engine is not None:
            return [
                TorilleForm(
                    word=f.word,
                    confidence=f.confidence,
                    pos=f.pos,
                    case=f.case,
                    number=f.number,
                    tense=f.tense,
                    person=f.person,
                    mood=f.mood,
                    voice=f.voice,
                    possessive=f.possessive,
                    negative=f.negative,
                    tags=list(f.tags),
                )
                for f in self.engine.inflect(lemma, klass=klass)
            ]
        return self._torille_forms_via_binary(lemma, klass)

    def _torille_forms_via_binary(self, lemma: str,
                                  klass: str | None = None) -> list[TorilleForm]:
        """The same reading, recovered from the `inflect` binary's text output.

        Used only when the extension module is not built. The tags arrive as
        whitespace-separated words, which is lossy in a way that matters: `poss
        1pl` and `third person` are two words for one field, so they are matched
        here by position within a known shape. The binding exists to avoid
        needing this.
        """
        command = [self.binary, lemma]
        if klass:
            command.append(klass)
        out = subprocess.run(
            command, capture_output=True, text=True, check=True,
        ).stdout
        forms: list[TorilleForm] = []
        for line in out.splitlines():
            line = line.strip()
            if not line or line.startswith("#") or " forms; " in line:
                continue
            tokens = line.split()
            for i, token in enumerate(tokens):
                if token in CONFIDENCES:
                    # Everything before the confidence word is the form, which
                    # is more than one token for `ei laske` and `ei olisi ...`.
                    words = " ".join(tokens[:i])
                    forms.append(self._form_from_tags(words, token, tokens[i + 1:]))
                    break
        return forms

    @staticmethod
    def _form_from_tags(word: str, confidence: str, tags: list[str]) -> TorilleForm:
        """Reconstructs the reading fields from the printed tag words.

        The words are walked in the order `format_form` prints them -- case,
        number, tense, person, mood, voice, possessive -- and each field is
        taken only if the next words can be that field, so a field the reading
        does not have is skipped rather than shifting everything after it. That
        is the whole difficulty: `ei tehdä` prints `verb singular present third
        person passive negative` and has no mood at all, so reading the mood
        from a fixed slot files `passive` as the mood and `negative` as the
        voice.

        Order is also the only thing that separates the two readings of
        `instructive`, which is a case in `verb instructive plural past
        instructive active` and a mood in the same string. The vocabularies
        above are disjoint, so a greedy walk in a fixed order is unambiguous.

        Anything the walk cannot place keeps no field, which puts the reading
        outside the scope -- the same answer the binding's `None` gives.
        """
        form = TorilleForm(word=word, confidence=confidence, tags=tags)
        # The part of speech is only a part of speech if it is one. A reading
        # can be a bare `singular poss 1pl` with nothing in front, and treating
        # `singular` as the pos would take the number with it.
        pos_words = set(POS_NAMES)
        named = bool(tags) and tags[0] in pos_words
        form.pos = tags[0] if named else None
        rest = tags[1:] if named else tags
        form.negative = "negative" in rest
        i = 0

        def take(vocabulary) -> str | None:
            nonlocal i
            if i < len(rest) and rest[i] in set(vocabulary):
                value = rest[i]
                i += 1
                return value
            return None

        form.case = take(CASE_NAMES_ALL)
        form.number = take(NUMBER_NAMES.values())
        form.tense = take(TENSE_NAMES)
        # The binary prints the person either as `first person` (two
        # words) or `first_person` (one token); accept both.
        if i < len(rest) and rest[i].endswith("_person"):
            word = rest[i].removesuffix("_person")
            if word in PERSON_WORDS:
                form.person = f"{word} person"
                i += 1
        elif i + 1 < len(rest) and rest[i + 1] == "person" and rest[i] in PERSON_WORDS:
            form.person = f"{rest[i]} person"
            i += 2
        form.mood = take(MOOD_NAMES)
        form.voice = take(VOICE_NAMES)
        if i < len(rest) and rest[i] == "poss" and i + 1 < len(rest):
            form.possessive = rest[i + 1]
        return form

    # -- scope -------------------------------------------------------------
    def finite(self, form: TorilleForm) -> tuple[str, str, str, str] | None:
        """The `(tense, mood, person, number)` of a finite verb reading, if it is one.

        A finite verb is read as a verb, with a number, a tense, a person, a
        mood and a voice, and is not negative. Everything else -- a participle,
        a participle's nominal case, a negative, a potential -- is None, and
        those are legitimate output that simply was not asked about.
        """
        if form.pos != "verb" or form.negative or form.mood is None:
            return None
        if form.number is None or form.person is None or form.tense is None:
            return None
        if form.voice is None:
            return None
        mood = VERB_MOODS.get(form.mood)
        person = VERB_PERSONS.get(form.person)
        tense = VERB_TENSES.get(form.tense)
        if mood is None or person is None or tense is None:
            return None
        if form.number not in NUMBER_NAMES.values():
            return None
        return (tense, mood, person, form.number)

    def in_scope(self, form: TorilleForm, cells: list[Cell]) -> bool:
        """Whether this reading could be one of the cells the oracle was asked for.

        The check is deliberately *partial*: a field the reading does not have
        is a wildcard, not a disqualification. `kissani` is tagged `noun
        singular poss 1sg` with no case at all -- a real defect, which
        `acceptable` reports -- but the form itself is still squarely inside the
        possessive grid, so it must be judged rather than skipped. A scope test
        that demanded a case would have let it past untouched.

        Strictness lives in `acceptable`, which is the one that decides whether a
        *tag* is right.
        """
        if form.pos != self.pos:
            return False
        if self.pos == "noun":
            return any(
                (form.case is None or CASE_NAMES[c.case] == form.case)
                and (form.number is None or NUMBER_NAMES[c.number] == form.number)
                and (
                    form.possessive is None
                    or POSSESSIVE_NAMES_MAP.get(c.possessive) == form.possessive
                )
                for c in cells
            )
        reading = self.finite(form)
        if reading is None:
            return False
        tense, mood, person, number = reading
        for c in cells:
            if (c.mood, c.person, NUMBER_NAMES[c.number]) != (mood, person, number):
                continue
            # A conditional cell pins no tense, because Omorfi gives the mood
            # none while torille reports the conditional as `present`.
            if c.tense == "" or c.tense == tense:
                return True
        return False

    def acceptable(self, form: TorilleForm, cells: list[Cell]) -> bool:
        """Whether a confirmed form may be counted as correctly filed.

        The recall direction is more forgiving than the precision one, because
        a surface form can be two words at once. `laskevat` is the finite third
        person plural *and* the nominative of the present participle, and
        torille files it under the participle; that is a real reading of a real
        form, so it is not a mis-tag just because it is not the cell the oracle
        happened to generate.
        """
        if self.pos == "noun":
            # Compatible, not strict: a field the reading does not claim is a
            # wildcard. The reference confirms the same surface under several
            # cells -- `aamuni` is the NOM/SG, GEN/SG and NOM/PL 1sg possessive
            # of `aamu` -- so a possessive whose case `union()` cannot pin is
            # genuinely ambiguous, and tagging it without a case is correct
            # rather than a defect. Only a claim that *contradicts* the cell
            # (a stated case that is not the cell's, for instance) is a
            # mis-tag; a missing field never is.
            return any(
                (form.case is None or CASE_NAMES[c.case] == form.case)
                and (form.number is None or NUMBER_NAMES[c.number] == form.number)
                and (
                    form.possessive is None
                    or POSSESSIVE_NAMES_MAP.get(c.possessive) == form.possessive
                )
                for c in cells
            )
        return self.in_scope(form, cells) or self.finite(form) is None

    def same_third_person(self, word: str, confirmed: str) -> bool:
        """Whether `word` is the other spelling of `confirmed`'s third person.

        `taloaan` and `taloansa` are the same cell's word, so offering the one
        Voikko's finite dictionary happens not to hold is not a precision
        defect. A different stem still fails the comparison, so a genuine grade
        or stem error is reported as `illegal` as before.
        """
        base = third_person_base(word)
        return base is not None and base == third_person_base(confirmed)

    def cells_of(self, form: TorilleForm, cells: list[Cell]) -> list[Cell]:
        """The cells a reading could be, most likely first.

        An exact reading -- every field present and matching -- is its cell. A
        reading with a field missing matches several, and that is what makes a
        per-cell report ambiguous in a way `check` never had to be: the
        possessive nominative is tagged `noun singular poss 1sg` with no case,
        so it could be NOM/SG/SG1 or GEN/SG/SG1 and the report has to say
        rather than pick.

        The order is by how little each candidate *assumes*, because the report
        quotes the oracle's answer for `candidates[0]` and that answer has to be
        the relevant one. A reading of `noun genitive singular` with no
        possessive is compatible with all six genitive-singular cells; naming
        the first of them in grid order would quote `eukonkantoni` for a form
        that is plainly not possessive, and would bury the actual finding, which
        is that the engine wants `eukonkandon`. The bare cell assumes nothing
        the reading did not say, so it comes first.
        """
        scored: list[tuple[int, int, Cell]] = []
        for i, c in enumerate(cells):
            if self.pos == "noun":
                if (form.case is not None and CASE_NAMES[c.case] != form.case):
                    continue
                if (form.number is not None
                        and NUMBER_NAMES[c.number] != form.number):
                    continue
                if (form.possessive is not None
                        and POSSESSIVE_NAMES_MAP.get(c.possessive) != form.possessive):
                    continue
                # Count the cell's fields the reading did not state. The
                # possessive is the one that matters in practice, and it is
                # almost always the one that is missing.
                assumed = sum((
                    bool(c.case) and form.case is None,
                    bool(c.number) and form.number is None,
                    bool(c.possessive) and form.possessive is None,
                ))
            else:
                reading = self.finite(form)
                if reading is None:
                    continue
                tense, mood, person, number = reading
                if (c.mood, c.person, NUMBER_NAMES[c.number]) != (mood, person, number):
                    continue
                if c.tense and c.tense != tense:
                    continue
                assumed = int(bool(c.tense) and form.tense is None)
            scored.append((assumed, i, c))
        scored.sort(key=lambda t: (t[0], t[1]))
        return [c for _, _, c in scored]

    def audit(self, lemma: str, cells: list[Cell],
              klass: str | None = None) -> list[Finding]:
        """Every form of this lemma's paradigm, with the reading that explains it.

        One `Finding` per confirmed form, carrying every cell that expects it.
        The unit is the form rather than the cell because a surface form is
        often the answer for several cells at once -- `kissani` is both the
        nominative and the genitive singular -- and a row per cell would count
        one correct form twice and make every rate computed from the report
        wrong. Where a cell answers with two strings, each gets its own row, so
        neither `kissakseen` nor `kissaksensa` can be lost.

        `confirmed` entries are the ones that were right, and they are kept
        because a report that lists only defects cannot be read as "the rest is
        fine" -- the class may have had nine cells and eight of them are
        missing.

        The four failure kinds carry the same meaning as in `Verdict`, but each
        carries `got` and `expected`, so `kissaksimme` is reported as `illegal`
        with `expected: kissaksemme` rather than as an unexplained word.
        """
        if self.lexicon is not None:
            return self._audit_lexicon(lemma, cells, klass)
        if not self.has_class(lemma):
            return []
        try:
            forms = self.torille_forms(lemma, klass)
        except (ValueError, subprocess.CalledProcessError):
            return []
        by_word: dict[str, list[TorilleForm]] = {}
        for form in forms:
            by_word.setdefault(form.word, []).append(form)

        # Collect first, group second. The same surface form is the answer for
        # several cells -- `kissani` is the nominative and the genitive
        # singular, `kissansa` the third-person possessive of many oblique cases
        # -- so a row per (cell, form) would report one correct form four times
        # and a per-class rate computed from it would be nonsense.
        expected_by_form: dict[str, list[Cell]] = {}
        expected_raw: dict[str, list[str]] = {}
        for cell in cells:
            generated = self.generate(lemma, cell)
            wanted = [w.strip() for w in generated.split("/") if w.strip()]
            # Omorfi declining a cell is not a finding: the form does not exist,
            # so there is nothing to match and nothing to report. That is a large
            # share of a possessive grid -- instructive and illative cells
            # largely do not exist -- and a row for each would bury the real ones.
            if not wanted:
                continue
            # Every cell that answers with this form gets one mention in
            # `expected_raw`, so a cell with two answers (`kissakseen` and
            # `kissaksensa` from the same cell) survives into the report instead
            # of being folded away by the set.
            for want in wanted:
                expected_raw.setdefault(want, [])
                if generated not in expected_raw[want]:
                    expected_raw[want].append(generated)
            for want in wanted:
                if self.voikko_ok(lemma, want):
                    expected_by_form.setdefault(want, []).append(cell)

        findings: list[Finding] = []
        if not expected_by_form:
            # Omorfi does not know this word. `iso` is in the Kotus sanalista
            # and inflects perfectly well, but it is not in Omorfi's
            # dictionary, so every cell comes back empty. There is no oracle
            # here to disagree with, and the precision pass below would read
            # every plain form torille offers as a defect -- a whole lemma's
            # worth of findings that say only "Omorfi has never heard of it".
            # A word the oracle cannot judge is a skip, and `check` already
            # skips it for the same reason.
            return []
        for want, expecting in expected_by_form.items():
            given = by_word.get(want, [])
            plain = [r for r in given if r.plain]
            if not given:
                status, detail = "missing", "torille produces no such form"
            elif not plain:
                status = "not plain"
                detail = f"offered at {given[0].confidence}"
            elif not any(self.acceptable(r, cells) for r in plain):
                # The word is right, the label is not. Both are given, because
                # "mistagged: kissani" is a symptom and "kissani is tagged with
                # no case" is the bug.
                status = "mistagged"
                detail = (f"offered as plain but tagged {' '.join(plain[0].tags)}"
                          f", not as the {expecting[0].describe()} the cell is")
            else:
                status, detail = "confirmed", ""
            findings.append(
                Finding(lemma, expecting[0], status, form=want,
                        expected="/".join(expected_raw.get(want, [want])),
                        got=want if plain else (given[0].word if given else ""),
                        confidence=plain[0].confidence if plain else
                        (given[0].confidence if given else ""),
                        tags=list(plain[0].tags) if plain else
                        (list(given[0].tags) if given else []),
                        detail=detail, candidates=list(expecting))
            )
        # Forms torille offers as plain that no cell accounts for are the
        # precision failures. `expected` is what the oracle *would* have said for
        # the reading's own cell, which is what makes the row actionable: for
        # `kissaksini` the reading is TRA/SG/1sg, so the row carries
        # `expected: kissakseni` and the defect reads as "this cell should have
        # produced that instead" rather than "unknown word".
        #
        # Only the first candidate's answer is quoted. A reading of `noun
        # abessive plural` with no possessive is compatible with all six
        # abessive-plural cells, and concatenating their answers turns one
        # useful fact -- the engine wants `epäonnistujitta`, not
        # `epäonnistujittä` -- into seven strings, of which six are guesses the
        # reading does not support. `candidates` already carries the rest.
        accounted = {f.form for f in findings if f.form}
        for word, given in sorted(by_word.items()):
            plain = [r for r in given if r.plain]
            if word in accounted or not plain:
                continue
            places = self.cells_of(plain[0], cells)
            if not places:
                continue
            if self.voikko_ok(lemma, word):
                # Voikko reads it as a form of this lemma, so it is one. Omorfi
                # just does not reach the cell -- its dictionary has no abessive
                # plural for `eukonkanto`, though the word is there. This is a
                # gap in the grid, not in the engine, and reporting it as a
                # defect would be the harness disagreeing with its own
                # authority: Voikko decides which forms are real, and it said
                # yes. It is still a row, because "confirmed" is the reader's
                # evidence that the rest of the class is fine, and a form the
                # report silently dropped is a form nobody counted.
                findings.append(
                    Finding(lemma, places[0], "confirmed", form=word,
                            expected=word, got=word,
                            confidence=plain[0].confidence,
                            tags=list(plain[0].tags), candidates=list(places),
                            detail="offered as plain and confirmed by Voikko; "
                                   "Omorfi has no form for this cell, so the "
                                   "grid is incomplete here rather than the "
                                   "engine")
                )
                continue
            # Voikko does not read it as a form of this lemma either. Now the
            # question is whether the oracle can say what the right form was.
            # What the oracle *would* have said for the cell this reading is
            # tagged for, and what of that it will stand behind. The two are
            # different questions and conflating them is how a report ends up
            # asserting a defect nobody can see: Omorfi generates
            # `epäonnistujutta` for the abessive plural of `epäonnistuja`,
            # but Voikko's dictionary does not reach that long a derived word,
            # so there is no answer *for or against* the engine.
            generated = [w.strip()
                         for w in self.generate(lemma, places[0]).split("/")
                         if w.strip()]
            confirmed_here = [w for w in generated if self.voikko_ok(lemma, w)]
            if confirmed_here and places[0].possessive == "3" and any(
                    self.same_third_person(word, want)
                    for want in confirmed_here):
                # The other spelling of the same third-person cell; see
                # `same_third_person`. Voikko holds one of the pair here,
                # Finnish has both.
                status = "confirmed"
                detail = ("offered as plain; the other standard spelling of "
                          f"the confirmed {'/'.join(confirmed_here)}")
            elif confirmed_here:
                # The cell exists, the oracle vouched for it, and it is a
                # different word. Proven wrong, and the row names the string
                # that should have come out.
                status = "illegal"
                detail = ("offered as plain, but the cell it is tagged for "
                          f"expects {'/'.join(confirmed_here)}")
            else:
                # Nothing vouches for the cell and nothing vouches against it.
                # Still worth a row -- an unbacked suggestion is a risk, and the
                # whole point of the precision direction is to see them -- but it
                # is not a proven defect, so it is reported as `unsupported`
                # rather than dressed up as one.
                status = "unsupported"
                if generated:
                    detail = ("offered as plain, and no oracle vouches for the "
                              "cell it is tagged for (Omorfi suggests "
                              f"{'/'.join(generated)}, Voikko confirms neither "
                              "it nor this)")
                else:
                    detail = ("offered as plain, and this paradigm has no form "
                              "for the cell it is tagged for")
            findings.append(
                Finding(lemma, places[0], status, form=word,
                        expected="/".join(confirmed_here), got=word,
                        confidence=plain[0].confidence,
                        tags=list(plain[0].tags), candidates=list(places),
                        detail=detail)
            )
        findings.sort(key=lambda f: (f.cell.case, f.cell.number, f.cell.possessive, f.form))
        return findings

    def _audit_lexicon(self, lemma: str, cells: list[Cell],
                       klass: str | None = None) -> list[Finding]:
        """The per-form findings, read from a lexicon reference instead of live
        Omorfi + Voikko (see docs/lexicon.md).

        The database holds the confirmed paradigm (`ok='both'`) with its cells,
        so `expected_by_form` comes from a query rather than a generate loop,
        and the precision pass resolves an unconfirmed plain form by asking the
        database what it knows, and does not, about the reading's own cell. The
        shape of the findings matches `audit` exactly, so a report reads the
        same either way.

        One difference is deliberate and a consequence of the reference: the
        grid-gap `confirmed` row does not occur here. A form neither oracle
        reaches has no `ok='both'` row to account for it, and the reader only
        has the database's yes/no -- there is no finer "Voikko says yes but
        Omorfi never reached the cell" to tell apart from "not a word", so such
        a form is `unsupported` rather than `confirmed`.
        """
        try:
            forms = self.torille_forms(lemma, klass)
        except (ValueError, subprocess.CalledProcessError):
            return []
        by_word: dict[str, list[TorilleForm]] = {}
        for form in forms:
            by_word.setdefault(form.word, []).append(form)

        # Readable-name -> omor code, the lexicon's columns are the readable
        # names and the reading's cells are the codes.
        inv_case = {v: k for k, v in CASE_NAMES.items()}
        poss_name = {"": "", "SG1": "first person singular",
                     "SG2": "second person singular", "PL1": "first person plural",
                     "PL2": "second person plural", "3": "third person"}
        inv_pos = {v: k for k, v in poss_name.items()}
        num_name = {"SG": "singular", "PL": "plural"}
        inv_num = {v: k for k, v in num_name.items()}

        rows = self.lexicon.execute(
            "SELECT form, [case], number, possessive FROM forms "
            "WHERE lemma=? AND ok='both'", (lemma,)).fetchall()
        if not rows:
            # A word with no confirmed form is a skip (`no confirmed form`).
            return []
        expected_by_form: dict[str, list[Cell]] = {}
        expected_raw: dict[str, list[str]] = {}
        for form, cname, nm, pnm in rows:
            if cname not in inv_case or nm not in inv_num:
                # A row outside the cell grid -- the DB also keeps
                # `comitative`, which is not one of the cases the grid asks
                # about (torille's comitative is the instructive/abessive
                # pair, generated and judged separately). It is real data,
                # just not a grid cell.
                continue
            expected_by_form.setdefault(form, []).append(
                Cell(case=inv_case[cname], number=inv_num[nm],
                     possessive=inv_pos.get(pnm, pnm)))
            expected_raw.setdefault(form, [form])
        # Every form Omorfi so much as generated, whatever `ok`, so the
        # precision pass can tell "Omorfi asserts it but Voikko does not" from
        # "Omorfi never produced it".
        all_forms = {r[0] for r in self.lexicon.execute(
            "SELECT form FROM forms WHERE lemma=?", (lemma,))}

        def cell_key(place: Cell) -> tuple[str, str, str]:
            return (CASE_NAMES[place.case], num_name[place.number],
                    poss_name[place.possessive])

        findings: list[Finding] = []
        for want, expecting in expected_by_form.items():
            given = by_word.get(want, [])
            plain = [r for r in given if r.plain]
            if not given:
                status, detail = "missing", "torille produces no such form"
            elif not plain:
                status = "not plain"
                detail = f"offered at {given[0].confidence}"
            elif not any(self.acceptable(r, cells) for r in plain):
                status = "mistagged"
                detail = (f"offered as plain but tagged {' '.join(plain[0].tags)}"
                          f", not as the {expecting[0].describe()} the cell is")
            else:
                status, detail = "confirmed", ""
            findings.append(Finding(
                lemma, expecting[0], status, form=want,
                expected=want, got=want if plain else (given[0].word if given else ""),
                confidence=plain[0].confidence if plain else
                (given[0].confidence if given else ""),
                tags=list(plain[0].tags) if plain else
                (list(given[0].tags) if given else []),
                detail=detail, candidates=list(expecting)))

        accounted = {f.form for f in findings if f.form}
        for word, given in sorted(by_word.items()):
            plain = [r for r in given if r.plain]
            if word in accounted or not plain:
                continue
            places = self.cells_of(plain[0], cells)
            if not places:
                continue
            cname, nm, pnm = cell_key(places[0])
            cell_expected = [r[0] for r in self.lexicon.execute(
                "SELECT form FROM forms WHERE lemma=? AND [case]=? AND number=? "
                "AND possessive=? AND ok='both'", (lemma, cname, nm, pnm))]
            if cell_expected:
                if pnm == "third person" and any(
                        self.same_third_person(word, want)
                        for want in cell_expected):
                    # The other spelling of the same third-person cell: the
                    # long `-nsa`/`-nsä` against the short `-Vn` the engine
                    # offers, or the reverse. Voikko's finite dictionary holds
                    # one of the pair for this word; Finnish and the engine
                    # have both. Not a wrong word, so not `illegal`.
                    status = "confirmed"
                    detail = ("offered as plain; the other standard spelling "
                              f"of the confirmed {'/'.join(cell_expected)}")
                else:
                    status = "illegal"
                    detail = (f"offered as plain, but the cell it is tagged for "
                              f"expects {'/'.join(cell_expected)}")
            elif word in all_forms:
                # Omorfi generates it but Voikko does not back it, and the cell
                # has no confirmed answer -- nothing for or against this word.
                status = "unsupported"
                detail = ("offered as plain, and no oracle vouches for the cell "
                          "it is tagged for")
            else:
                status = "unsupported"
                detail = ("offered as plain, and this paradigm has no form for "
                          "the cell it is tagged for")
            findings.append(Finding(
                lemma, places[0], status, form=word,
                expected="/".join(cell_expected), got=word,
                confidence=plain[0].confidence,
                tags=list(plain[0].tags), candidates=list(places), detail=detail))
        findings.sort(key=lambda f: (f.cell.case, f.cell.number, f.cell.possessive, f.form))
        return findings

    def check(self, lemma: str, cells: list[Cell], lenient: bool = False) -> Verdict:
        if not self.has_class(lemma):
            return Verdict(lemma, skipped="no class in the sanalista")
        if self.is_compound(lemma):
            return Verdict(lemma, skipped="compound")
        confirmed = self.confirmed_forms(lemma, cells)
        if not confirmed:
            return Verdict(lemma, skipped="no confirmed form")
        verdict = Verdict(lemma, confirmed=confirmed)
        try:
            forms = self.torille_forms(lemma)
        except (ValueError, subprocess.CalledProcessError) as exc:
            # A lemma the sanalista classes but torille will not inflect is a
            # skip, not a crash: a wordlist run is thousands of lemmas and one
            # awkward word must not end it.
            return Verdict(lemma, confirmed=confirmed, skipped=f"not inflectable ({exc})")
        readings: dict[str, list[TorilleForm]] = {}
        for form in forms:
            readings.setdefault(form.word, []).append(form)
        for word in confirmed:
            given = readings.get(word)
            if not given:
                verdict.missing.add(word)
            elif not any(r.plain for r in given):
                verdict.not_plain.add(word)
            elif not any(self.acceptable(r, cells) for r in given):
                # Offered as plain, but not under the cell the oracle put it in.
                verdict.mistagged.add(word)
        for word, given in readings.items():
            if (word not in confirmed and any(r.plain for r in given)
                    and any(self.in_scope(r, cells) for r in given)):
                verdict.illegal.add(word)
        if lenient:
            # Voikko's dictionary is finite, so an unconfirmed plain form is a
            # suspicion rather than a proof. `--lenient` keeps it visible but
            # stops it deciding the run.
            verdict.illegal = set()
        return verdict


def noun_cells(
    cases: list[str],
    numbers: list[str],
    possessives: list[str] | None = None,
) -> list[Cell]:
    """The noun cells asked of the oracle.

    Every cell is (case, number, possessive). The possessive default is every
    one of them, because a spell checker has to have an opinion about
    `kissani`: it is a word torille offers as plain, and it has to be confirmed
    by the oracle or reported as illegal. Pass `possessives=[""]` to ask only
    for the bare case/number grid, which is 6x cheaper and appropriate when the
    question is specifically about the cases.
    """
    if possessives is None:
        possessives = ALL_POSSESSIVES
    return [
        Cell(case=c, number=n, possessive=p)
        for c in cases
        for n in numbers
        for p in possessives
    ]


def verb_cells(tense_moods: list[tuple[str, str]], numbers: list[str]) -> list[Cell]:
    cells: list[Cell] = []
    for tense, mood in tense_moods:
        for number in numbers:
            for person in ("1", "2", "3"):
                cells.append(Cell(tense=tense, mood=mood, person=person, number=number))
    return cells


def eprint(*args: object) -> None:
    print(*args, file=sys.stderr)
