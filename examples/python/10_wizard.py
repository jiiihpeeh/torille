#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""The word wizard: infer a Kotus class from a few example forms.

`class_candidates` returns `(candidates, next_question, tie, recorded)`.
Each candidate is `(class, hits, misses, unknown, recorded, tail_prior, prior)`.
Add answers one at a time and watch the ranking tighten; `use_recorded=False`
hides the sanalista's own class, which is how the published curve was measured.
"""

from torille import Engine

STEPS = [
    [],
    [("gen_sg", "mökin")],
    [("gen_sg", "mökin"), ("par_sg", "mökkiä")],
    [("gen_sg", "mökin"), ("par_sg", "mökkiä"), ("ill_sg", "mökkiin")],
]

UNKNOWN_STEPS = [
    [("gen_sg", "pöllykän")],
    [("gen_sg", "pöllykän"), ("par_sg", "pöllykkää")],
    [("gen_sg", "pöllykän"), ("par_sg", "pöllykkää"), ("ill_sg", "pöllykkään")],
]


def show(engine: Engine, word: str, answers, use_recorded: bool) -> None:
    candidates, next_question, tie, recorded = engine.class_candidates(
        word, "noun", answers, use_recorded
    )
    top = ", ".join(f"{klass}({hits}/{misses})" for klass, hits, misses, *_ in candidates[:3])
    print(f"  answers={len(answers)} next={next_question!r} tie={tie} top: {top}")
    if recorded:
        print(f"    sanalista says: {recorded}")


def main() -> None:
    engine = Engine()

    print("A known word, with the recorded class hidden (the evaluation shape):")
    for answers in STEPS:
        show(engine, "mökki", answers, use_recorded=False)

    print("\nThe same answers with the recorded prior visible (the UI shape):")
    for answers in STEPS:
        show(engine, "mökki", answers, use_recorded=True)

    print("\nA word the lexicon does not have:")
    for answers in UNKNOWN_STEPS:
        show(engine, "pöllykkä", answers, use_recorded=False)

    candidates, _, _, _ = engine.class_candidates(
        "pöllykkä", "noun", UNKNOWN_STEPS[-1], use_recorded=False
    )
    inferred = candidates[0][0]
    forms = engine.inflect("pöllykkä", klass=inferred)
    print(f"\nInferred class {inferred}; the paradigm starts:")
    for form in forms[:6]:
        print(f"  {form.word:<14} {form.confidence:<9} {form.tags}")


if __name__ == "__main__":
    main()
