# SPDX-License-Identifier: GPL-3.0-or-later
"""The binding and the `inflect` binary must report the same reading.

The harness reads torille's output as grammatical fields, and the binding
exists to report those fields directly rather than have them recovered from the
binary's printed words. That is only safe if the two agree, so this pins the
agreement: the same lemma through both paths has to give the same form, the
same confidence and the same reading, word for word.

Run with the binding built:

    (cd bindings && env -u CONDA_PREFIX maturin develop)
    python3 tools/test_binding_agrees_with_binary.py
"""

from __future__ import annotations

import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from omorfi_harness import (
    CONFIDENCES,
    Harness,
    TorilleForm,
    _load_engine,
    inflect_binary,
)

#: Words chosen to cover the shapes the harness's scope checks read: a plain
#: noun, a fixed-stem class whose singular reads a different stem, a two-class
#: noun, a verb with the negative and the participle readings, and a word whose
#: forms have two readings apiece.
WORDS = [
    "kala", "talo", "käsi", "nainen", "joki", "lapsi", "maa", "tieto",
    "sisu", "valo", "ori", "pieni", "hame", "ahkeraliisa", "aallokko",
    "laskea", "mennä", "tulla", "tehdä", "pitää", "rauhallinen",
    # A row with no class is a compound: Kotus omits the type because the word
    # declines like its final component, and both paths have to reach the
    # head's paradigm rather than refuse the word.
    "pikkuaivot", "aamuhartaus",
]


def forms_from_binary(binary: str, lemma: str) -> list[TorilleForm]:
    """The reading, recovered from the binary's printed output.

    This is deliberately the lossy path: `poss 1pl` and `third person` are two
    printed words for one field, so the fields are matched by position. The
    harness's own fallback recovers them the same way, and this calls that same
    code, so the comparison exercises the fallback as well as the binding.
    """
    out = subprocess.run(
        [binary, lemma], capture_output=True, text=True, check=True
    ).stdout
    forms: list[TorilleForm] = []
    for line in out.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or " forms; " in line:
            continue
        tokens = line.split()
        for i, token in enumerate(tokens):
            if token in CONFIDENCES:
                # Everything before the confidence word is the form, which is
                # more than one token for `ei laske` and `ei olisi laskettu`.
                forms.append(
                    Harness._form_from_tags(" ".join(tokens[:i]), token, tokens[i + 1:])
                )
                break
    return forms


#: The reading fields the harness's scope checks read. `tags` is compared as a
#: single space-joined string rather than as a list: the binding reports `poss 3`
#: as one field while the binary prints two whitespace-separated words, so the
#: list lengths differ while the readings do not. That difference is precisely
#: why the binding exists, and why the fields below are what gets compared.
FIELDS = (
    "pos", "case", "number", "tense", "person", "mood", "voice",
    "possessive", "negative",
)


def main() -> int:
    engine_cls = _load_engine()
    if engine_cls is None:
        print("binding not built; run: (cd bindings && maturin develop)", file=sys.stderr)
        return 77
    engine = engine_cls()
    binary = inflect_binary()
    failures = 0

    def field(f, key):
        value = getattr(f, key)
        # The binding spells the person `third_person`; the binary's
        # fallback parser spells it `third person`. They are the same field.
        if key == "person" and isinstance(value, str):
            return value.replace("_", " ")
        return value

    for lemma in WORDS:
        binding = [
            (f.word, f.confidence, " ".join(f.tags), tuple(field(f, k) for k in FIELDS))
            for f in engine.inflect(lemma)
        ]
        binary_forms = forms_from_binary(binary, lemma)
        from_binary = [
            (
                f.word,
                f.confidence,
                " ".join(f.tags),
                tuple(getattr(f, k) for k in FIELDS),
            )
            for f in binary_forms
        ]
        if sorted(binding) != sorted(from_binary):
            failures += 1
            print(f"MISMATCH {lemma}: binding={len(binding)} binary={len(from_binary)}")
            b_only = sorted(set(binding) - set(from_binary))
            n_only = sorted(set(from_binary) - set(binding))
            for x in b_only[:5]:
                print(f"    only binding: {x}")
            for x in n_only[:5]:
                print(f"    only binary : {x}")
        else:
            print(f"ok {lemma:12} {len(binding):4d} forms identical")
    if failures:
        print(f"\n{failures} lemma(s) disagree")
        return 1
    print(f"\nall {len(WORDS)} lemmas agree between the binding and the binary")
    return 0


if __name__ == "__main__":
    sys.exit(main())
