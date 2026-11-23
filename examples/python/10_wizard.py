#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""The word wizard: infer a Kotus class from a few example forms.

`class_candidates` returns `(candidates, next_question, tie, recorded)`.
Each candidate is `(class, hits, misses, unknown, recorded, tail_prior, prior)`.
Add answers one at a time and watch the ranking tighten; `use_recorded=False`
hides the sanalista's own class, which is how the published curve was measured.
"""

from torille import Cell, Engine, Luokka

STEPS = [
    [],
    [(Cell.GEN_SG, "mökin")],
    [(Cell.GEN_SG, "mökin"), (Cell.PAR_SG, "mökkiä")],
    [(Cell.GEN_SG, "mökin"), (Cell.PAR_SG, "mökkiä"), (Cell.ILL_SG, "mökkiin")],
]

UNKNOWN_STEPS = [
    [(Cell.GEN_SG, "pöllykän")],
    [(Cell.GEN_SG, "pöllykän"), (Cell.PAR_SG, "pöllykkää")],
    [
        (Cell.GEN_SG, "pöllykän"),
        (Cell.PAR_SG, "pöllykkää"),
        (Cell.ILL_SG, "pöllykkään"),
    ],
]


def show(engine: Engine, word: str, answers, use_recorded: bool) -> None:
    candidates, next_question, tie, recorded = engine.class_candidates(
        word, "noun", answers, use_recorded
    )
    top = ", ".join(
        f"{luokka}({hits}/{misses})" for luokka, hits, misses, *_ in candidates[:3]
    )
    next_key = next_question.key if next_question is not None else None
    print(f"  answers={len(answers)} next={next_key!r} tie={tie} top: {top}")
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

    print("\nThe cells are a real enum, not free-form strings:")
    print(f"  Cell.ILL_SG -> {Cell.ILL_SG.key!r} ({Cell.ILL_SG})")
    print(f"  Cell.parse('ill_sg') -> {Cell.parse('ill_sg')}")
    print(f"  Cell.parse('ill-sg') -> {Cell.parse('ill-sg')}")

    candidates, _, _, _ = engine.class_candidates(
        "pöllykkä", "noun", UNKNOWN_STEPS[-1], use_recorded=False
    )
    inferred = candidates[0][0]
    forms = engine.inflect("pöllykkä", luokka=Luokka(inferred))
    print(f"\nInferred class {inferred}; the paradigm starts:")
    for form in forms[:6]:
        print(f"  {form.word:<14} {form.confidence:<9} {form.tags}")


if __name__ == "__main__":
    main()
