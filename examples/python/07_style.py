#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Style: suggestion-only observations, never an error or a warning.

The style layer is opt-in and additive. Every finding has
`severity == "suggestion"`; a curated table row may carry a replacement, and
the panel decides whether to show it.
"""

from torille import Engine

TEXT = (
    "Koira koira nukkuu. Hän tuli tupaan, jossa roihusi tuli. "
    "Tällä hetkellä tilanne on rauhallinen. Viime kädessä päätös on sinun. "
    "Se on niin sanotusti aika hyvä. Tämä on minun duuni. "
    "Tänään aamulla heräsin hyvin aikaisin ja päätin lähteä pitkälle "
    "kävelylle rantaan, jossa tapasin vanhan ystävän, jonka kanssa "
    "keskustelin pitkään tulevista suunnitelmista ja menneistä muistoista, "
    "ja sen jälkeen menin läheiseen kahvilaan lukemaan sanomalehteä ja "
    "juomaan kupin kuumaa kahvia rauhassa ennen töihin lähtöä."
)


def span_text(text: str, start: int, end: int) -> str:
    return text.encode("utf-8")[start:end].decode("utf-8")


def main() -> None:
    engine = Engine()
    diagnostics = engine.style(TEXT)

    print(f"text: {TEXT}\n")
    by_kind: dict[str, list] = {}
    for diagnostic in diagnostics:
        by_kind.setdefault(diagnostic.kind, []).append(diagnostic)

    for kind in sorted(by_kind):
        print(f"{kind}:")
        for diagnostic in by_kind[kind]:
            surface = span_text(TEXT, diagnostic.start, diagnostic.end)
            print(
                f"  [{diagnostic.start}:{diagnostic.end}] {surface!r} "
                f"severity={diagnostic.severity} expected={diagnostic.expected}"
            )
            for edits, score, note, license_name, confidence in diagnostic.corrections:
                replacement = "".join(text for _, _, text in edits)
                print(
                    f"    -> {replacement!r} ({license_name}, {confidence}, "
                    f"score {score:.1f}, note {note})"
                )

    print(f"\n{len(diagnostics)} findings, all suggestion-only: "
          f"{all(d.severity == 'suggestion' for d in diagnostics)}")

    print("\nThe grammar pass does not see style, and vice versa:")
    print(f"  grammar findings: {len(engine.grammar(TEXT))}")
    print(f"  style findings:   {len(engine.style(TEXT))}")


if __name__ == "__main__":
    main()
