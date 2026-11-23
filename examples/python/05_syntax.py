#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Syntax: clauses, roles, relative nesting and word-order features.

`syntax` returns `(tokens, clauses)`. Every input token has a role and a
clause index; every clause has the predicate/subject/object/complement token
indices, the boolean construction flags, and the nine word-order features as
`(name, value)` pairs. Relative clauses link to their parent and antecedent.
"""

from torille import Engine

TEXTS = [
    "Koira nukkuu talossa.",
    "Lapsi, joka lauloi, oli iloinen.",
    "Luen kirjaa ja kuuntelen musiikkia.",
    "Onko sinulla aikaa?",
]


def main() -> None:
    engine = Engine()

    for text in TEXTS:
        print(f"\n{text}")
        tokens, clauses = engine.syntax(text)
        for index, clause in enumerate(clauses):
            words = [
                f"{token.surface}:{token.role}"
                for token in tokens
                if token.clause == index and token.surface.strip()
            ]
            print(f"  clause {index}: {' '.join(words)}")
            print(
                f"    predicate={clause.predicate} subject={clause.subject} "
                f"object={clause.object} complement={clause.complement}"
            )
            features = dict(clause.features)
            print(
                "    features: "
                + ", ".join(f"{name}={value}" for name, value in features.items())
            )
            if clause.parent is not None or clause.children or clause.antecedent is not None:
                print(
                    f"    relative: parent={clause.parent} "
                    f"children={clause.children} antecedent={clause.antecedent}"
                )
            print(
                f"    flags: negative={clause.negative} "
                f"existential={clause.existential} passive={clause.passive}"
            )

    print("\nPer-token roles and governors:")
    tokens, _ = engine.syntax("Lapsi, joka lauloi, oli iloinen.")
    for token in tokens:
        if not token.surface.strip():
            continue
        print(
            f"  {token.surface:<12} role={token.role:<11} "
            f"confidence={token.confidence:<7} clause={token.clause} "
            f"head={token.head} lemmas={token.lemmas}"
        )


if __name__ == "__main__":
    main()
