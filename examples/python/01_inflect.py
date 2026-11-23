#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Inflection: confidence, readings, cases, possessives, class override.

Every form the engine produces is one of three confidences. Only `plain`
forms may be offered as corrections; `repaired` and `guessed` are accepted
words, so the counts below are the contract a suggestion layer relies on.
"""

from collections import Counter, defaultdict

from torille import Engine


def main() -> None:
    engine = Engine()

    forms = engine.inflect("talo")
    print(f"talo: {len(forms)} forms, {dict(Counter(f.confidence for f in forms))}")

    print("\nSingular cases (plain forms, no possessive):")
    by_case: dict[str, set[str]] = defaultdict(set)
    for form in forms:
        if (
            form.plain
            and form.number == "singular"
            and form.possessive is None
            and form.case is not None
        ):
            by_case[form.case].add(form.word)
    for case in sorted(by_case):
        print(f"  {case:<12} {', '.join(sorted(by_case[case]))}")

    print("\nThe possessive column (a selection):")
    possessives = [form for form in forms if form.possessive is not None]
    for form in possessives[:10]:
        print(
            f"  {form.word:<14} case={str(form.case):<12} "
            f"poss={form.possessive:<4} tags={form.tags}"
        )
    print(f"  ... {len(possessives)} possessive forms in total")

    print("\nOne form's reading:")
    form = next(form for form in forms if form.word == "talossa")
    print(f"  {form.word}: {form.tags}")

    print("\nA word the sanalista has never seen, given a class:")
    for form in engine.inflect("mököke", klass="1", pos="substantiivi")[:6]:
        print(f"  {form.word:<14} {form.confidence:<9} {form.tags}")

    print("\nWithout a class there is nothing to inflect from:")
    try:
        engine.inflect("mököke")
    except ValueError as error:
        print(f"  ValueError: {error}")


if __name__ == "__main__":
    main()
