#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Add and validate derivative flags on the sanalista word-list CSV.

The base source is `data/nykysuomensanalista.csv`: the first four columns are
the Kotus lemma, homonym, POS and class; the fifth (`Johdokset`) carries zero
or more flags on the **base** row:

    nostaa     verbi  53    FREQ:67
    jouduttaa  verbi  53*C  REFL:52

`TYPE:CLASS` means "the productive TYPE derivative of this base exists and
inflects as Kotus CLASS". The rule table here mirrors `src/expand/derived.rs`.
No derived lemma row is added: the flag is the source of truth and the class
engine generates the forms.

Usage:
    python3 tools/add_derivation_flag.py --check
    python3 tools/add_derivation_flag.py --base nostaa --type FREQ
    python3 tools/add_derivation_flag.py --base jouduttaa --type REFL --out \\
        data/nykysuomensanalista_derivedtest.csv
    python3 tools/add_derivation_flag.py --list
"""

from __future__ import annotations

import argparse
import csv
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CSV = REPO / "data" / "nykysuomensanalista.csv"
INFLECT = REPO / "target" / "release" / "inflect"
HEADER = ["Hakusana", "Homonymia", "Sanaluokka", "Taivutustiedot", "Johdokset"]

# Mirrors RULES in src/expand/derived.rs. `stem` is `vowel` (drop the
# base's final vowel) or `d` (replace the infinitive's `da`/`dä`).
RULES: dict[str, dict[str, object]] = {
    "FREQ": {
        "class": "67", "back": "tella", "front": "tellä",
        "input": "verbi", "output": "verbi", "stem": "vowel",
    },
    "REFL": {
        "class": "52", "back": "utua", "front": "ytyä",
        "input": "verbi", "output": "verbi", "stem": "vowel",
    },
    "REFL_ITA": {
        "class": "52", "back": "utua", "front": "ytyä",
        "input": "verbi", "output": "verbi", "stem": "ta",
    },
    "VERB_OI": {
        "class": "62", "back": "oida", "front": "öidä",
        "input": "substantiivi", "output": "verbi", "stem": "vowel",
    },
    "REFL_OI": {
        "class": "52", "back": "tua", "front": "tyä",
        "input": "verbi", "output": "verbi", "stem": "d",
    },
    "ACTION_MINEN": {
        "class": "38", "back": "minen", "front": "minen",
        "input": "verbi", "output": "substantiivi", "stem": "vowel",
    },
    "ACTION_MIMEN": {
        "class": "38", "back": "minen", "front": "minen",
        "input": "verbi", "output": "substantiivi", "stem": "vowel",
    },
    "ADJ_LLINEN": {
        "class": "38", "back": "inen", "front": "inen",
        "input": "substantiivi", "output": "adjektiivi", "stem": "llinen",
    },
    "ADJ_INEN": {
        "class": "38", "back": "inen", "front": "inen",
        "input": "substantiivi", "output": "adjektiivi", "stem": "vowel",
    },
    "ABSTRACT_ISUUS": {
        "class": "40", "back": "isuus", "front": "isyys",
        "input": "adjektiivi", "output": "substantiivi", "stem": "direct",
    },
    "ABSTRACT_UUS": {
        "class": "40", "back": "uus", "front": "yys",
        "input": "adjektiivi", "output": "substantiivi", "stem": "nen_s",
    },
    "VERB_ISTAA": {
        "class": "53", "back": "istaa", "front": "istää",
        "input": "adjektiivi", "output": "verbi", "stem": "is",
    },
    "VERB_ISOIDA": {
        "class": "62", "back": "isoida", "front": "isöidä",
        "input": "substantiivi", "output": "verbi", "stem": "ia",
    },
    "ACTION_NTI": {
        "class": "5*J", "back": "nti", "front": "nti",
        "input": "verbi", "output": "substantiivi", "stem": "da",
    },
    "ACTION_UMINEN": {
        "class": "38", "back": "uminen", "front": "yminen",
        "input": "substantiivi", "output": "substantiivi", "stem": "vowel",
    },
    "ACTION_MA": {
        "class": "10", "back": "ma", "front": "mä",
        "input": "verbi", "output": "substantiivi", "stem": "vowel",
    },
    "ACTION_IN": {
        "class": "33", "back": "in", "front": "in",
        "input": "verbi", "output": "substantiivi", "stem": "ta",
    },
    "ACTION_IN_KEA": {
        "class": "33*L", "back": "in", "front": "in",
        "input": "verbi", "output": "substantiivi", "stem": "ske_j",
    },
    "ACTION_E_56": {
        "class": "48", "back": "e", "front": "e",
        "input": "verbi", "output": "substantiivi", "stem": "kaa",
        "input_classes": [56], "inherit_gradation": True,
    },
    "ACTION_E_52": {
        "class": "48", "back": "e", "front": "e",
        "input": "verbi", "output": "substantiivi", "stem": "kua",
        "input_classes": [52], "inherit_gradation": True,
    },
    "ACTION_E_53": {
        "class": "48", "back": "e", "front": "e",
        "input": "verbi", "output": "substantiivi", "stem": "aa",
        "input_classes": [53], "inherit_gradation": True,
    },
    "ACTION_E_58": {
        "class": "48", "back": "je", "front": "je",
        "input": "verbi", "output": "substantiivi", "stem": "kea",
        "input_classes": [58], "inherit_gradation": True,
    },
    "ACTION_IN_58": {
        "class": "33", "back": "in", "front": "in",
        "input": "verbi", "output": "substantiivi", "stem": "ske_j",
        "input_classes": [58], "inherit_gradation": True,
    },
    "ACTION_IN_53_TTAA": {
        "class": "33", "back": "in", "front": "in",
        "input": "verbi", "output": "substantiivi", "stem": "ttaa_t",
        "input_classes": [53], "inherit_gradation": True,
    },
    "ACTION_IN_53_TAA": {
        "class": "33", "back": "in", "front": "in",
        "input": "verbi", "output": "substantiivi", "stem": "taa_d",
        "input_classes": [53], "inherit_gradation": True,
    },
    "ACTION_IN_57": {
        "class": "33", "back": "in", "front": "in",
        "input": "verbi", "output": "substantiivi", "stem": "taa_d",
        "input_classes": [57], "inherit_gradation": True,
    },
    "AGENT_JA_TSI": {
        "class": "10", "back": "ja", "front": "jä",
        "input": "verbi", "output": "substantiivi", "stem": "ita_itsi",
    },
    "AGENT_JA": {
        "class": "10", "back": "ja", "front": "jä",
        "input": "verbi", "output": "substantiivi", "stem": "ta",
    },
    "AGENT_JA_ATA": {
        "class": "10", "back": "ja", "front": "jä",
        "input": "verbi", "output": "substantiivi", "stem": "ata_aa",
    },
    "AGENT_JA_ELLA": {
        "class": "10", "back": "lija", "front": "lijä",
        "input": "verbi", "output": "substantiivi", "stem": "lla_strong",
    },
    "AGENT_JA_ILLA": {
        "class": "12", "back": "ija", "front": "ijä",
        "input": "verbi", "output": "substantiivi", "stem": "la",
    },
    "AGENT_JA_TTAA": {
        "class": "10", "back": "taja", "front": "täjä",
        "input": "verbi", "output": "substantiivi", "stem": "taa_strip",
    },
    "FREQ_ILLA": {
        "class": "67", "back": "illa", "front": "illä",
        "input": "verbi", "output": "verbi", "stem": "ta",
    },
    "ACTION_ILU": {
        "class": "2", "back": "u", "front": "y",
        "input": "verbi", "output": "substantiivi", "stem": "la",
    },
    "PLACE_MO": {
        "class": "2", "back": "mo", "front": "mö",
        "input": "verbi", "output": "substantiivi", "stem": "ata_aa",
    },
    "ACTION_US_TTAA": {
        "class": "39", "back": "tus", "front": "tys",
        "input": "verbi", "output": "substantiivi", "stem": "ttaa_strip",
    },
    "ADJ_MAINEN": {
        "class": "38", "back": "mainen", "front": "mainen",
        "input": "substantiivi", "output": "adjektiivi", "stem": "direct",
    },
    "ADJ_LLINEN_I": {
        "class": "38", "back": "inen", "front": "inen",
        "input": "substantiivi", "output": "adjektiivi", "stem": "i_ill",
        "input_classes": [5],
    },
    "FEM_TAR": {
        "class": "32*C", "back": "tar", "front": "tär",
        "input": "substantiivi", "output": "substantiivi", "stem": "direct",
    },
    "NOUN_KKO": {
        "class": "4*A", "back": "kko", "front": "kkö",
        "input": "substantiivi", "output": "substantiivi", "stem": "direct",
    },
    "NOUN_STO": {
        "class": "2", "back": "sto", "front": "stö",
        "input": "substantiivi", "output": "substantiivi", "stem": "direct",
    },
    "NOUN_KUNTA": {
        "class": "10*J", "back": "kunta", "front": "kunta",
        "input": "substantiivi", "output": "substantiivi", "stem": "direct",
    },
    "DIM_NEN": {
        "class": "38", "back": "nen", "front": "nen",
        "input": "substantiivi", "output": "substantiivi", "stem": "direct",
    },
    "FREQ_ELLA": {
        "class": "67", "back": "ella", "front": "ellä",
        "input": "verbi", "output": "verbi", "stem": "aa",
    },
    "FREQ_SKELLA_I": {
        "class": "67", "back": "skella", "front": "skellä",
        "input": "verbi", "output": "verbi", "stem": "ksia",
    },
    "FREQ_SKELLA": {
        "class": "67", "back": "skella", "front": "skellä",
        "input": "verbi", "output": "verbi", "stem": "ske",
    },
    "MOM_AHTAA": {
        "class": "53*F", "back": "ahtaa", "front": "ähtää",
        "input": "verbi", "output": "verbi", "stem": "aa",
    },
    "CAUS_TTAA": {
        "class": "53*C", "back": "ttaa", "front": "ttää",
        "input": "verbi", "output": "verbi", "stem": "vowel",
    },
    "ACTION_US_TA": {
        "class": "39", "back": "us", "front": "ys",
        "input": "verbi", "output": "substantiivi", "stem": "ta",
    },
    "PRIV_TON_E": {
        "class": "34*C", "back": "ton", "front": "tön",
        "input": "substantiivi", "output": "adjektiivi", "stem": "el_e",
    },
    "ACTION_US_TA_STRONG": {
        "class": "39", "back": "us", "front": "ys",
        "input": "verbi", "output": "substantiivi", "stem": "ta_strong",
    },
    "AGENT_JA_NNA": {
        "class": "10", "back": "ja", "front": "jä",
        "input": "verbi", "output": "substantiivi", "stem": "nna_ni",
    },
    "AGENT_JA_ATA_STRONG": {
        "class": "10", "back": "ja", "front": "jä",
        "input": "verbi", "output": "substantiivi", "stem": "ata_aa_strong",
    },
    "VERB_TTAA_WEAK": {
        "class": "53*C", "back": "ttaa", "front": "ttää",
        "input": "substantiivi", "output": "verbi", "stem": "weak",
    },
    "VERB_ITTAA": {
        "class": "53*C", "back": "ittaa", "front": "ittää",
        "input": "substantiivi", "output": "verbi", "stem": "direct",
    },
    "PRIV_TON_I": {
        "class": "34*C", "back": "ton", "front": "tön",
        "input": "substantiivi", "output": "adjektiivi", "stem": "i_e_weak",
    },
    "PRIV_TON": {
        "class": "34*C", "back": "ton", "front": "tön",
        "input": "substantiivi", "output": "adjektiivi", "stem": "weak",
    },
    "FREQ_TTAA_ELLA": {
        "class": "67*C", "back": "ella", "front": "ellä",
        "input": "verbi", "output": "verbi", "stem": "ttaa_t",
    },
    "FREQ_OITSIA": {
        "class": "61", "back": "tsia", "front": "tsiä",
        "input": "verbi", "output": "verbi", "stem": "da",
    },
    "FREQ_ILLA_STRONG": {
        "class": "67", "back": "illa", "front": "illä",
        "input": "verbi", "output": "verbi", "stem": "ta_strong",
    },
    "CAUS_ITTAA": {
        "class": "53*C", "back": "ittaa", "front": "ittää",
        "input": "verbi", "output": "verbi", "stem": "vowel",
    },
    "CAUS_YTTAA": {
        "class": "53*C", "back": "tyttaa", "front": "tyttää",
        "input": "verbi", "output": "verbi", "stem": "ttaa",
    },
    "ACTION_MUS": {
        "class": "39", "back": "mus", "front": "mys",
        "input": "verbi", "output": "substantiivi", "stem": "vowel",
    },
    "ACTION_MINEN_DA": {
        "class": "38", "back": "minen", "front": "minen",
        "input": "verbi", "output": "substantiivi", "stem": "da",
    },
    "ACTION_MINEN_LLA": {
        "class": "38", "back": "minen", "front": "minen",
        "input": "verbi", "output": "substantiivi", "stem": "lla_le_strong",
    },
    "POSS_KAS": {
        "class": "41*A", "back": "kas", "front": "käs",
        "input": "substantiivi", "output": "adjektiivi", "stem": "direct",
    },
    "ADV_STI_A": {
        "class": "99", "back": "asti", "front": "ästi",
        "input": "adjektiivi", "output": "adverbi", "stem": "vowel",
    },
    "ADV_STI_NEN": {
        "class": "99", "back": "esti", "front": "esti",
        "input": "adjektiivi", "output": "adverbi", "stem": "nen_s",
    },
    "ADV_STI_IS": {
        "class": "99", "back": "isti", "front": "isti",
        "input": "adjektiivi", "output": "adverbi", "stem": "is_i",
    },
    "ADV_STI_I": {
        "class": "99", "back": "sti", "front": "sti",
        "input": "adjektiivi", "output": "adverbi", "stem": "i_e",
    },
    "ADV_STI_TON": {
        "class": "99", "back": "ttomasti", "front": "ttömästi",
        "input": "adjektiivi", "output": "adverbi", "stem": "ton",
    },
}

# Rules `apply_all_derivations.py` applies to every matching base row.
BULK_RULES = (
    "FREQ", "REFL",
    "VERB_OI", "ADJ_LLINEN", "ADJ_INEN", "ABSTRACT_UUS", "VERB_ISTAA",
    "ACTION_MA", "ACTION_IN", "ACTION_IN_KEA",
    "ACTION_E_56", "ACTION_E_52", "ACTION_E_53", "ACTION_E_58",
    "ACTION_IN_58", "ACTION_IN_53_TTAA", "ACTION_IN_53_TAA", "ACTION_IN_57",
    "NOUN_KKO", "NOUN_STO", "NOUN_KUNTA", "DIM_NEN",
    "FREQ_ELLA", "FREQ_SKELLA", "MOM_AHTAA", "CAUS_TTAA",
    "ACTION_US_TA", "PRIV_TON", "POSS_KAS",
    "ADV_STI_A", "ADV_STI_NEN", "ADV_STI_IS", "ADV_STI_I", "ADV_STI_TON",
)


def front(word: str) -> bool:
    for char in reversed(word):
        if char in "äöyüÄÖYÜ":
            return True
        if char in "aouôåAOUÅ":
            return False
    return True


#: (strong, weak) pairs, matching `src/gradation.rs::Gradation::pair`.
GRADATION_PAIRS = {
    "A": ("kk", "k"), "B": ("pp", "p"), "C": ("tt", "t"),
    "D": ("k", ""), "E": ("p", "v"), "F": ("t", "d"),
    "G": ("nk", "ng"), "H": ("mp", "mm"), "I": ("lt", "ll"),
    "J": ("nt", "nn"), "K": ("rt", "rr"), "L": ("k", "j"),
    "M": ("k", "v"),
}


def apply_gradation(word: str, gradation: str, grade: str) -> str:
    pair = GRADATION_PAIRS.get(gradation.upper())
    if not pair:
        return word
    strong, weak = pair
    source, target = (strong, weak) if grade == "weak" else (weak, strong)
    if not source:
        return word
    at = word.rfind(source)
    if at < 0:
        return word
    return word[:at] + target + word[at + len(source):]


def derive(base: str, kind: str, gradation: str = "") -> str:
    rule = RULES[kind]
    mode = rule["stem"]
    if mode == "weak":
        stem = apply_gradation(base, gradation, "weak")
    elif mode == "ta_strong":
        stem = next(
            (base[: -len(suffix)] for suffix in ("ta", "tä") if base.endswith(suffix)),
            base[:-1],
        )
        stem = apply_gradation(stem, gradation, "strong")
    elif mode == "ttaa":
        for sfx in ("ttaa", "ttää"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)]
                break
        else:
            raise ValueError(f"{kind} needs a -ttaa/-ttää verb, got {base!r}")
    elif mode == "lla_le_strong":
        for sfx in ("llä", "lla"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)] + "le"
                break
        else:
            raise ValueError(f"{kind} needs an -llä/-lla verb, got {base!r}")
        stem = apply_gradation(stem, gradation, "strong")
    elif mode == "d":
        stem = next(
            (base[: -len(suffix)] for suffix in ("da", "dä") if base.endswith(suffix)),
            base[:-1],
        )
    elif mode == "da":
        stem = next(
            (base[: -len(suffix)] for suffix in ("da", "dä") if base.endswith(suffix)),
            base[:-1],
        )
    elif mode == "ta":
        stem = next(
            (base[: -len(suffix)] for suffix in ("ta", "tä") if base.endswith(suffix)),
            base[:-1],
        )
    elif mode == "aa":
        stem = next(
            (base[: -len(suffix)] for suffix in ("aa", "ää") if base.endswith(suffix)),
            base[:-1],
        )
    elif mode == "kaa":
        stem = next(
            (base[: -len(sfx)] for sfx in ("kaa", "kää") if base.endswith(sfx)),
            base[:-1],
        )
    elif mode == "kua":
        stem = next(
            (base[: -len(sfx)] for sfx in ("kua", "kyä") if base.endswith(sfx)),
            base[:-1],
        )
    elif mode == "kea":
        stem = next(
            (base[: -len(sfx)] for sfx in ("kea", "keä") if base.endswith(sfx)),
            base[:-1],
        )
    elif mode == "ttaa_t":
        for sfx in ("ttaa", "ttää"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)] + "t"
                break
        else:
            raise ValueError(f"{kind} needs a -ttaa/-ttää verb, got {base!r}")
    elif mode == "taa_d":
        for sfx in ("taa", "tää"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)] + "d"
                break
        else:
            raise ValueError(f"{kind} needs a -taa/-tää verb, got {base!r}")
    elif mode == "ske_j":
        for ending in ("kea", "keä"):
            if base.endswith(ending):
                stem = base[: -len(ending)] + "j"
                break
        else:
            raise ValueError(f"{kind} needs a -kea/-keä verb, got {base!r}")
    elif mode == "ske":
        if base.endswith("kea"):
            stem = base[:-3] + "e"
        elif base.endswith("keä"):
            stem = base[:-3] + "e"
        else:
            raise ValueError(f"{kind} needs a -kea/-keä verb, got {base!r}")
    elif mode == "ia":
        stem = next(
            (base[: -len(suffix)] for suffix in ("ia", "iä") if base.endswith(suffix)),
            base[:-1],
        )
    elif mode == "llinen":
        stem = base
        for ending, replacement in (("ni", "nell"), ("de", "teell"), ("i", "ell")):
            if stem.endswith(ending):
                stem = stem[: -len(ending)] + replacement
                break
    elif mode == "nen_s":
        if not base.endswith("nen"):
            raise ValueError(f"{kind} needs an -nen adjective, got {base!r}")
        stem = base[:-3] + "s"
    elif mode == "is_i":
        if not base.endswith("is"):
            raise ValueError(f"{kind} needs an -is adjective, got {base!r}")
        stem = base[:-2] + "i"
    elif mode == "i_e":
        if not base.endswith("i"):
            raise ValueError(f"{kind} needs an -i adjective, got {base!r}")
        stem = base[:-1] + "e"
    elif mode == "ton":
        for ending in ("ton", "tön"):
            if base.endswith(ending):
                stem = base[: -len(ending)]
                break
        else:
            raise ValueError(f"{kind} needs an -ton/-tön adjective, got {base!r}")
    elif mode == "is":
        if not base.endswith("is"):
            raise ValueError(f"{kind} needs an -is adjective, got {base!r}")
        stem = base[:-2]
    elif mode == "ata_aa":
        stem = None
        for suffix, repl in (("ata", "aa"), ("ätä", "ää")):
            if base.endswith(suffix):
                stem = base[: -len(suffix)] + repl
                break
        if stem is None:
            raise ValueError(f"{kind} needs an -ata/-ätä verb, got {base!r}")
    elif mode == "ttaa_tt":
        stem = None
        for suffix in ("ttaa", "ttää"):
            if base.endswith(suffix):
                stem = base[: -len(suffix)] + "tt"
                break
        if stem is None:
            raise ValueError(f"{kind} needs a -ttaa/-ttää verb, got {base!r}")
    elif mode == "ta_a":
        stem = next(
            (base[: -len(suffix)] + ("a" if suffix == "ta" else "ä")
             for suffix in ("ta", "tä") if base.endswith(suffix)),
            None,
        )
        if stem is None:
            raise ValueError(f"{kind} needs a -ta/-tä verb, got {base!r}")
    elif mode == "la":
        stem = next(
            (base[: -len(suffix)] for suffix in ("la", "lä") if base.endswith(suffix)),
            base[:-1],
        )
    elif mode == "i_ill":
        if not base.endswith("i"):
            raise ValueError(f"{kind} needs an -i noun, got {base!r}")
        stem = base[:-1] + "ill"
    elif mode == "ttaa_strip":
        stem = next(
            (base[: -len(suffix)] for suffix in ("ttaa", "ttää") if base.endswith(suffix)),
            None,
        )
        if stem is None:
            raise ValueError(f"{kind} needs a -ttaa/-ttää verb, got {base!r}")
    elif mode == "taa_strip":
        stem = next(
            (base[: -len(suffix)] for suffix in ("taa", "tää") if base.endswith(suffix)),
            None,
        )
        if stem is None:
            raise ValueError(f"{kind} needs a -taa/-tää verb, got {base!r}")
    elif mode == "i_e_weak":
        stem = apply_gradation(base, gradation, "weak")
        if not stem.endswith("i"):
            raise ValueError(f"{kind} needs an -i noun, got {base!r}")
        stem = stem[:-1] + "e"
    elif mode == "lla_strong":
        for sfx in ("lla", "llä"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)]
                break
        else:
            raise ValueError(f"{kind} needs an -lla/-llä verb, got {base!r}")
        stem = apply_gradation(stem, gradation, "strong")
    elif mode == "ksia":
        for sfx in ("ksia", "ksiä"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)]
                break
        else:
            raise ValueError(f"{kind} needs a -ksia/-ksiä verb, got {base!r}")
    elif mode == "ita_itsi":
        for sfx in ("ita", "itä"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)] + "itsi"
                break
        else:
            raise ValueError(f"{kind} needs an -ita/-itä verb, got {base!r}")
    elif mode == "nna_ni":
        for sfx in ("nna", "nnä"):
            if base.endswith(sfx):
                stem = base[: -len(sfx)] + "ni"
                break
        else:
            raise ValueError(f"{kind} needs an -nna/-nnä verb, got {base!r}")
    elif mode == "el_e":
        if base.endswith(("el", "er")):
            stem = base[:-2] + base[-2] + "e"
        else:
            stem = base
    elif mode == "ata_aa_strong":
        stem = None
        for sfx, repl in (("ata", "aa"), ("ätä", "ää")):
            if base.endswith(sfx):
                stem = base[: -len(sfx)] + repl
                break
        if stem is None:
            raise ValueError(f"{kind} needs an -ata/-ätä verb, got {base!r}")
        stem = apply_gradation(stem, gradation, "strong")
    elif mode == "direct":
        stem = base
    else:
        stem = base[:-1]
    suffix = str(rule["front"] if front(stem) else rule["back"])
    return stem + suffix


def derive_chain(base: str, chain: str, pos: str, base_class: str = "") -> tuple[str, str, str]:
    """Apply `TYPE:CLASS>TYP:CLASS...` and return `(lemma, pos, class)`."""
    lemma = base
    current_pos = pos
    out_class = ""
    class_num = base_class.split("*", 1)[0]
    gradation = base_class.split("*", 1)[1] if "*" in base_class else ""
    for step in chain.split(">"):
        kind, _, klass = step.strip().partition(":")
        rule = RULES.get(kind)
        if rule is None:
            raise ValueError(f"unknown derivation {kind!r}")
        expected = str(rule["input"])
        if not current_pos.startswith(expected):
            raise ValueError(f"{kind} needs {expected}, got {current_pos}")
        allowed = rule.get("input_classes")
        if allowed and class_num.isdigit() and int(class_num) not in allowed:
            raise ValueError(f"{kind} needs class {allowed}, got {base_class}")
        lemma = derive(lemma, kind, gradation)
        current_pos = str(rule["output"])
        out_class = klass or str(rule["class"])
        if rule.get("inherit_gradation") and "*" not in out_class and gradation:
            out_class = f"{out_class}*{gradation}"
    return lemma, current_pos, out_class


def read_rows(path: Path) -> tuple[list[str], list[list[str]]]:
    with path.open(encoding="utf-8", newline="") as fh:
        rows = list(csv.reader(fh, delimiter="\t"))
    if not rows or rows[0][:4] != HEADER[:4]:
        raise SystemExit(f"{path}: not a sanalista CSV (header {rows[:1]!r})")
    header = rows[0]
    if len(header) < 5:
        header = HEADER
    return header, [row for row in rows[1:] if row]


def write_rows(path: Path, rows: list[list[str]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh, delimiter="\t", lineterminator="\n")
        writer.writerow(HEADER)
        for row in rows:
            writer.writerow((row + [""] * 5)[:5])


def row_flags(row: list[str]) -> list[str]:
    if len(row) < 5 or not row[4].strip():
        return []
    return [flag.strip() for flag in row[4].split(";") if flag.strip()]


_ENGINE = None


def inflect(lemma: str, klass: str, pos: str) -> list[str]:
    global _ENGINE
    try:
        import torille  # type: ignore
    except Exception:
        if not INFLECT.exists():
            raise SystemExit(
                "add_derivation_flag: build the torille binding or the inflect binary"
            )
        proc = subprocess.run(
            [str(INFLECT), lemma, klass, pos],
            capture_output=True,
            text=True,
            check=False,
        )
        return [line.split()[0] for line in proc.stdout.splitlines() if line.strip()]
    if _ENGINE is None:
        _ENGINE = torille.Engine()
    return [form.word for form in _ENGINE.inflect(lemma, klass=klass, pos=pos)]


def validate(rows: list[list[str]], quiet: bool = False) -> int:
    failures = 0
    flagged = 0
    for row in rows:
        base, pos = row[0], row[2].strip()
        for flag in row_flags(row):
            flagged += 1
            try:
                lemma, out_pos, out_class = derive_chain(
                    base, flag, pos, row[3].strip() if len(row) > 3 else ""
                )
            except ValueError as exc:
                print(f"FAIL {base}: {exc}", file=sys.stderr)
                failures += 1
                continue
            try:
                forms = inflect(lemma, out_class, out_pos)
            except Exception as exc:  # noqa: BLE001
                print(f"FAIL {base}: {lemma} class {out_class}: {exc}", file=sys.stderr)
                failures += 1
                continue
            if lemma not in forms:
                print(
                    f"FAIL {base}: class {out_class} does not generate {lemma}",
                    file=sys.stderr,
                )
                failures += 1
            elif not quiet:
                print(f"ok   {base} {flag} -> {lemma} ({len(forms)} forms)")
    if failures:
        print(f"{failures} failure(s)", file=sys.stderr)
        return 1
    if not quiet:
        print(f"{flagged} flag(s) valid")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="validate only")
    ap.add_argument("--list", action="store_true", help="print flagged base rows")
    ap.add_argument("--base", help="base lemma to flag")
    ap.add_argument("--type", choices=sorted(RULES), help="one-step derivation type")
    ap.add_argument("--chain", help="chain, e.g. VERB_OI:62>REFL_OI:52>ACTION_MINEN:38")
    ap.add_argument("--class", dest="klass", help="output class (default: from type)")
    ap.add_argument("--out", default=str(CSV), help="CSV to write")
    args = ap.parse_args()

    header, rows = read_rows(CSV)
    out = Path(args.out)

    if args.list:
        for row in rows:
            flags = row_flags(row)
            if flags:
                print(f"{row[0]}\t{row[2]}\t{row[3]}\t{';'.join(flags)}")
        return 0

    changed = False
    if args.base and (args.type or args.chain):
        target = next((row for row in rows if row[0] == args.base), None)
        if target is None:
            print(f"base not found: {args.base}", file=sys.stderr)
            return 2
        if args.chain:
            flag = args.chain
        else:
            klass = args.klass or str(RULES[args.type]["class"])
            flag = f"{args.type}:{klass}"
        if flag not in row_flags(target):
            target.extend([""] * (5 - len(target)))
            existing = target[4].strip()
            target[4] = f"{existing};{flag}".strip(";")
            changed = True
    elif args.base or args.type or args.chain:
        ap.error("--base and one of --type/--chain must be given together")

    if validate(rows, quiet=args.check) != 0:
        return 1
    if args.check:
        return 0
    if changed or out != CSV:
        # The test copy gets the full current CSV, including existing flags.
        write_rows(out, rows)
        print(f"wrote {out} ({len(rows)} rows)")
    else:
        print("no changes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
