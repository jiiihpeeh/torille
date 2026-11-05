#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Build `data/verb_stem_to_base_map.csv`: a (stem, base) table for
verb forms. Companion to `tools/build/build_stem_to_base_map.py` (the
noun one); same shape, same downstream consumer (`tools/build.rs`
embeds both as compile-time sorted slices).

Why a separate CSV for verbs:
  - Verb stems are computed by stripping *personal* endings
    (`tiedän` -> `tiedä`), *person+tense* combos, *participles*,
    and *infinitives* -- a different table from the noun case
    endings the noun builder uses.
  - The verb oracle (`data/kotus_verbit.sqlite`, 7 M confirmed
    forms, 9 759 lemmas) carries the (lemma, form, voice, mood,
    tense, person, number, infinitive, participle, polarity)
    breakdown so the strip can be exact per row, no longest-first
    guessing.

Scope:
  - Finite forms: indicative (present + past), conditional, potential,
    imperative -- the personal endings; passives (impersonal);
    the negation auxiliaries + their fusions.
  - Non-finite forms: infinitives (2nd, 4th) and participles (present
    active, past active, agent, present/past passive). Because they
    decline as nominals, each contributes its **case-stem** (the form
    minus its longest nominal case ending), so the fast path can peel a
    case ending (`lukeneen`, `lukeneita`, `kukistumiseen`) to reach it.
  - Multi-word forms (perfect / pluperfect "olemme tietäneet"
    type) and negation auxiliary forms ("en tiedä", "emme tiedä"
    type) are skipped -- they're compounds the engine doesn't
    inflect as a single word, and their stems would be ambiguous.

Inputs:
  data/kotus_verbit.sqlite

Outputs:
  data/verb_stem_to_base_map.csv   -- one (stem, base) row per form.

Usage:
  python3 tools/build/build_verb_stem_to_base_map.py
"""
from __future__ import annotations

import sqlite3
import sys

DB = "data/kotus_verbit.sqlite"
OUT = "data/verb_stem_to_base_map.csv"

# Forms both Omorfi AND Voikko accept are the base; **plus** the
# Omorfi-only participles. Voikko lemmatises a participle to the
# participle itself (`rutistuneen` -> BASEFORM `rutistunut`) or renders
# the base differently (`rutistaa` vs `rutistua`), so the oracle's
# `voikko_ok` (a WORDBASES substring test) marks most participles
# `omorfi` even when Voikko accepts the surface. The runtime verify
# (re-inflect the candidate base with the engine) still gates every hit,
# so an over-included participle pair is harmless.
OK_FILTER = "both"

# ---------------------------------------------------------------------------
# Personal endings, keyed by (mood, tense, person, number). Each value is
# a list of suffix shapes to try in order; the stripper returns the first
# match. Harmony: front/back vowel variants are listed as alternates.
# Empty-string entry means "the form *is* the stem" (e.g., 3sg present
# indicative = bare stem, 2sg imperative = bare stem).
# ---------------------------------------------------------------------------
PERSONAL_ENDINGS: dict[tuple[str, str, str, str], list[str]] = {
    # Indicative, present, active
    ("indicative", "present", "first",  "singular"): ["n"],
    ("indicative", "present", "second", "singular"): ["t"],
    ("indicative", "present", "third",  "singular"): [""],
    ("indicative", "present", "first",  "plural"):   ["mme"],
    ("indicative", "present", "second", "plural"):   ["tte"],
    ("indicative", "present", "third",  "plural"):   ["vat", "vät"],
    # Indicative, past, active
    ("indicative", "past", "first",  "singular"): ["in"],
    ("indicative", "past", "second", "singular"): ["it"],
    ("indicative", "past", "third",  "singular"): ["i", ""],
    ("indicative", "past", "first",  "plural"):   ["imme"],
    ("indicative", "past", "second", "plural"):   ["itte"],
    ("indicative", "past", "third",  "plural"):   ["ivat", "ivät"],
    # Conditional, active (no tense distinction: "isi" + personal ending)
    ("conditional", "", "first",  "singular"): ["isin"],
    ("conditional", "", "second", "singular"): ["isit"],
    ("conditional", "", "third",  "singular"): ["isi"],
    ("conditional", "", "first",  "plural"):   ["isimme"],
    ("conditional", "", "second", "plural"):   ["isitte"],
    ("conditional", "", "third",  "plural"):   ["isivät", "isivat"],
    # Potential, active ("ne" + personal ending)
    ("potential", "", "first",  "singular"): ["nen"],
    ("potential", "", "second", "singular"): ["net"],
    ("potential", "", "third",  "singular"): ["nee"],
    ("potential", "", "first",  "plural"):   ["nemme"],
    ("potential", "", "second", "plural"):   ["nette"],
    ("potential", "", "third",  "plural"):   ["nevät", "nevat"],
    # Imperative, active
    ("imperative", "", "first",  "singular"): ["koon", "köön"],
    ("imperative", "", "second", "singular"): [""],
    ("imperative", "", "third",  "singular"): ["koon", "köön"],
    ("imperative", "", "first",  "plural"):   ["kaamme", "käämme"],
    ("imperative", "", "second", "plural"):   ["kaa", "kää"],
    ("imperative", "", "third",  "plural"):   ["koot", "kööt"],
}

# ---------------------------------------------------------------------------
# Passive forms. Finnish passive is impersonal -- person/number are
# empty in the oracle -- and the stem shape is class-driven:
#
#   * lukea (class 52-ish): passive stem `lueta-` (basic form `lueta`,
#     present 3sg `luetaan`, past `luettiin`, conditional
#     `luettaisiin`, potential `luettaneen`, imperative
#     `luettakoon`).
#   * tietää (class 76*F): passive stem `tiedetä-` (basic `tiedetä`,
#     present 3sg `tiedetään`, past `tiedettiin`, ...). The `e`
#     before `tä` is part of the verb-class gradation.
#
# The stripper emits `(stem, base)` pairs for the longest first
# matching suffix; the verify step in `check_normal`'s gate 4b
# re-inflects `lemma` and checks the form against the surface.
# Wrong splits fail the verify and the right split wins, so
# over-emitting candidates is harmless.
# ---------------------------------------------------------------------------
PASSIVE_PERSONAL_ENDINGS: dict[tuple[str, str], list[str]] = {
    # Indicative, present, 3sg (impersonal).
    ("indicative", "present"): ["taan", "tään", "aan", "ään"],
    # Indicative, past, impersonal.
    ("indicative", "past"):    ["ttiin", "tiin", "iin"],
    # Conditional, impersonal.
    ("conditional", ""):       ["ttaisiin", "täisiin",
                                "taisiin", "isiin"],
    # Potential, impersonal.
    ("potential", ""):         ["ttaneen", "täneen",
                                "taneen", "neen"],
    # Imperative, impersonal (singular/plural collapse).
    ("imperative", ""):        ["ttäköön", "täköön",
                                "ttakoon", "takoon",
                                "köön", "koon"],
}

# ---------------------------------------------------------------------------
# Passive participles. Same split-and-verify logic as the active
# ones; the participle's own paradigm has cases/numbers that
# generate the full inflection set, and those are not in scope
# here -- the noun stem map already catches `tiedetty` (it's a
# separate lemma in the sanalista).
# ---------------------------------------------------------------------------
PASSIVE_PARTICIPLE_ENDINGS: dict[str, list[str]] = {
    "past passive":    ["ttu", "tty", "tu", "ty"],
    "present passive": ["ttävä", "ttava", "tävä", "tava"],
}

# ---------------------------------------------------------------------------
# Nominal case endings. Participles and infinitives decline as nominals, so
# their oblique forms (`lukeneen`, `lukeneita`, `kukistumiseen`) are reached
# on the fast path by peeling a *case* ending, not a verb ending. For every
# non-finite row we emit the **case-stem** -- the form with its longest
# matching case ending removed -- and the runtime's gate 4b
# (`find_base_via_verb_stem_map`) also strips this list against the verb
# stem map.
#
# Must stay in sync with `ENDINGS` in `src/expand/check_normal.rs`: an
# ending added there has to be added here so the builder emits the matching
# stem, and vice versa.
# ---------------------------------------------------------------------------
CASE_ENDINGS: list[str] = sorted(
    [
        "iltä", "ilta", "istä", "ista", "issä", "inä", "ina",
        "jen", "ten", "den", "siin",
        "isiin", "kaan", "kään", "seen", "hon", "hön", "jaa", "jää",
        "ihin", "issa", "istä", "illa", "iltä", "ille", "iksi",
        "tta", "ttä", "issa", "istä", "illa", "illä", "iltä", "ille",
        "ssa", "ssä", "sta", "stä", "lla", "llä", "lle", "lta", "ltä",
        "ksi", "han", "hän", "kin", "isi",
        "na", "nä", "ta", "tä", "ja", "jä",
        "en", "an", "än", "in", "on", "ön", "un", "yn",
        "a", "ä", "n", "t",
    ],
    key=len,
    reverse=True,
)


def case_stem(form: str) -> str | None:
    """The longest matching case ending stripped from `form`."""
    for ending in CASE_ENDINGS:
        if form.endswith(ending) and len(form) > len(ending):
            return form[: -len(ending)]
    return None


#: The vowels stripped from the end of a non-finite stem. Finnish stem
#: vowels are these; the consonant letters are not.
STEM_VOWELS = "aeiouyäö"


def strip_stem_vowels(stem: str | None) -> str | None:
    """Drop the trailing stem vowels of a non-finite stem.

    Every case of a participle/infinitive reduces to one stem once the
    stem vowel is gone: `lukeneen` -> `lukenee` -> `luken`,
    `lukeneita` -> `lukenei` -> `luken`, `lukenutta` -> `lukenu` ->
    `luken`. The runtime mirrors this by probing the ending-stripped stem
    and then its vowel-stripped form (`find_base_via_verb_stem_map`),
    so one key covers the whole participle/infinitive paradigm instead of
    one key per case. Kept at >= 2 chars so a stem never collapses to a
    bare consonant."""
    if not stem:
        return stem
    trimmed = stem.rstrip(STEM_VOWELS)
    return trimmed if len(trimmed) >= 2 else stem


def stem_of(row: tuple) -> str | None:
    """Recover the verb stem from an oracle row.

    `row` is `(form, voice, mood, tense, person, number, infinitive,
    participle, possessive, polarity)`. Returns `None` if the row is
    out of scope or no ending matches.
    """
    (form, voice, mood, tense, person, number, infinitive,
     participle, possessive, polarity) = row

    # Out-of-scope filters.
    if polarity == "negative":
        return None
    if " " in form:
        # Multi-word: "olemme tietäneet", "en tiedä".
        return None
    if voice not in ("active", "passive", ""):
        return None

    # --- Passive forms (impersonal: person/number empty) ---------
    if voice == "passive":
        # Personal (impersonal): mood + tense set, person/number empty.
        if mood and (person == "" and number == ""):
            key = (mood, tense)
            for ending in PASSIVE_PERSONAL_ENDINGS.get(key, []):
                if ending == "":
                    return form
                if form.endswith(ending) and len(form) > len(ending):
                    return form[: -len(ending)]
            # The bare passive form (no suffix to remove) is itself
            # a stem and a valid emission: `tiedetä`, `lueta`.
            if (mood, tense) in PASSIVE_PERSONAL_ENDINGS:
                return form
            return None
        # Participles.
        if participle:
            for ending in PASSIVE_PARTICIPLE_ENDINGS.get(participle, []):
                if form.endswith(ending) and len(form) > len(ending):
                    return form[: -len(ending)]
            return None
        return None

    # --- Active personal forms (mood + person + number set) -------
    if mood and person and number:
        key = (mood, tense, person, number)
        for ending in PERSONAL_ENDINGS.get(key, []):
            if ending == "":
                return form
            if form.endswith(ending) and len(form) > len(ending):
                return form[: -len(ending)]
        return None

# Maps an active-personal row to ALL of its stem-map keys, so
# the 3sg past row (`["i", ""]`) emits both the stripped stem
# (`ol`) and the surface form (`oli`). The runtime's verb-direct
# probe reaches `oli` directly; the verb-strip path reaches `ol`
# by peeling `-i`. The verify step handles over-emission.
_ACTIVE_MULTI: set[tuple[str, str, str, str]] = {
    ("indicative", "past", "third", "singular"),  # 3sg past: "oli" + "ol"
}


def stems_of(row: tuple) -> list[str] | None:
    """Like [`stem_of`] but emits every matching ending as a
    separate stem. Used for the few 3sg cases where the surface
    form is itself a wanted key (`oli` -> `olla`, alongside the
    stripped `ol`)."""
    (form, voice, mood, tense, person, number, infinitive,
     participle, possessive, polarity) = row

    if polarity == "negative":
        return None
    if " " in form:
        return None
    if voice not in ("active", "passive", ""):
        return None
    if not (mood and person and number):
        return None
    if (mood, tense, person, number) not in _ACTIVE_MULTI:
        return None

    key = (mood, tense, person, number)
    out = []
    for ending in PERSONAL_ENDINGS.get(key, []):
        if ending == "":
            out.append(form)
            continue
        if form.endswith(ending) and len(form) > len(ending):
            out.append(form[: -len(ending)])
    return out or None


def main() -> int:
    con = sqlite3.connect(DB)
    cur = con.cursor()
    cur.execute(
        "SELECT form, voice, mood, tense, person, number, "
        'infinitive, participle, possessive, polarity, lemma, "case" '
        "FROM forms WHERE ok = ? OR (ok = 'omorfi' AND (participle != '' "
        "OR (mood = 'indicative' AND tense = 'past' AND person = 'third' "
        "AND number = 'singular')))",
        (OK_FILTER,),
    )
    written = 0
    skipped = 0
    seen: set[tuple[str, str]] = set()
    # Per (lemma, non-finite kind), the shortest form -- the bare
    # nominative (`muistava`, `tiennyt`, `lukema`, `kukistuminen`).
    bare_nonshort: dict[tuple[str, str], str] = {}
    lines: list[str] = []

    def emit(stem: str | None, lemma: str) -> None:
        nonlocal written
        if not stem or not lemma:
            return
        key = (stem, lemma)
        if key in seen:
            return
        seen.add(key)
        lines.append(f"{stem}\t{lemma}\n")
        written += 1

    for row in cur:
        (form, voice, mood, tense, person, number, infinitive,
         participle, possessive, polarity, lemma, case) = row

        # Out-of-scope rows: negative polarity, multiword forms, and any
        # voice the oracle does not use.
        if polarity == "negative" or " " in form:
            skipped += 1
            continue
        if voice not in ("active", "passive", ""):
            skipped += 1
            continue

        # --- Non-finite forms (participles + infinitives) -----------
        #
        # These decline as nominals: `lukenut` -> genitive `lukeneen`,
        # partitive plural `lukeneita`; the 4th infinitive `kukistuminen`
        # -> illative `kukistumiseen`. The fast path reaches an oblique
        # form by peeling one **case** ending, so emit the case-stem
        # (form minus its longest matching case ending) for every case,
        # plus the bare nominative for a direct probe.
        #
        # Possessive variants (`saamiaan`) are reached through gate 3b
        # (strip the possessive, resolve the bare host `saamia`), so
        # emitting them here would only inflate the map.
        if participle or infinitive:
            if possessive:
                continue
            kind = participle or f"inf:{infinitive}"
            key = (lemma, kind)
            if key not in bare_nonshort or len(form) < len(bare_nonshort[key]):
                bare_nonshort[key] = form
            # Emit the case-stem only for a genuine oblique form. The
            # bare nominative singular (`muistava`, `lukenut`,
            # `kukistuminen`) is emitted whole below; stripping its final
            # vowel would emit a spurious short stem (`muistav`,
            # `lukenu`) that the compound-modifier check would then treat
            # as a combining form (`kammoksuttav|asti`).
            if not (case in ("", "nominative") and number == "singular"):
                emit(strip_stem_vowels(case_stem(form)), lemma)
            continue

        # --- Finite forms (personal endings) ------------------------
        stem = stem_of(row[:10])
        if stem is None or not stem or not lemma:
            skipped += 1
            continue
        emit(stem, lemma)

        # 3sg cases (e.g. past 3sg `oli` of `olla`) emit both the
        # stripped stem (`ol`) and the surface form (`oli`). Emitting
        # both is harmless thanks to the verify step.
        for extra in stems_of(row[:10]) or ():
            emit(extra, lemma)

    # Emit the bare non-finite nominatives. By construction these are the
    # shortest form in their (lemma, kind) group; the stem vowels are
    # stripped so they share a key with the oblique case-stems.
    for (lemma, _kind), bare in bare_nonshort.items():
        emit(strip_stem_vowels(bare), lemma)

    with open(OUT, "w", encoding="utf-8") as f:
        f.writelines(lines)

        # Negation verb. The kotus verb oracle doesn't carry
        # the negation verb `ei` on its own -- it only ever
        # appears as the first word of a multi-word form
        # (`ei tule`, `en tiennyt`, ...). The finite forms
        # (`en`, `et`, `ei`, `emme`, `ette`, `eivät`) are
        # still words the spell-check should accept on the
        # fast path. Hardcode them as direct verb stem-map
        # keys pointing to `ei`.
        #
        # The verify step (`inflect_all`) is bypassed: the
        # engine doesn't generate these single-word surface
        # forms, so the verify would always fail. Trust the
        # list: each entry is a real Finnish finite form of
        # the negation verb.
        for surface in ("en", "et", "ei", "emme", "ette", "eivät"):
            k = (surface, "ei")
            if k in seen:
                continue
            seen.add(k)
            f.write(f"{surface}\tei\n")
            written += 1

        # The negation fusions (`enkä`, `ettet`, `ellette`, ...), read from
        # the `connectives` table `tools/build/build_verb_lexicon.py`
        # creates. The engine does not generate them either, so the runtime
        # trusts the row without the verify, exactly as for the finite
        # forms above. An older DB without the table contributes nothing.
        try:
            connectives = cur.execute(
                "SELECT surface, base FROM connectives"
            ).fetchall()
        except sqlite3.OperationalError:
            connectives = []
        for surface, base in connectives:
            k = (surface, base)
            if k in seen:
                continue
            seen.add(k)
            f.write(f"{surface}\t{base}\n")
            written += 1

    print(
        f"wrote {OUT} ({written} unique (stem, base) pairs, "
        f"{skipped} rows skipped)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())