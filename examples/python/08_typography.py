#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Typography: the mechanical semicolon pass, and why it is separate.

Spacing around `;` is a mechanical edit, not a grammar finding: it has its own
rule ids, no severity, and `apply_typography` turns the findings into text.
Fenced code, inline code, URLs and quotations are protected, so a semicolon
inside them is left alone.
"""

from torille import Engine

SAMPLES = [
    "Ostin omenia ;appelsiineja;; banaaneja.",
    "Katso https://example.com/a;b ja `koodi;tässä`.",
    "```\nkoodi ;lohko\n```",
    'Hän sanoi: "omena ;banaani".',  # a quotation, protected
    "Oikein: omenia; appelsiineja ja banaaneja.",
]


def main() -> None:
    engine = Engine()

    for text in SAMPLES:
        diagnostics = engine.typography(text)
        print(f"{text!r}")
        if not diagnostics:
            print("  (clean)")
        for diagnostic in diagnostics:
            print(
                f"  {diagnostic.rule:<32} [{diagnostic.start}:{diagnostic.end}] "
                f"-> {diagnostic.replacement!r}"
            )
        if diagnostics:
            print(f"  fixed: {engine.apply_typography(text)!r}")
        print()

    text = SAMPLES[0]
    print("The same pass is not part of grammar or style:")
    print(f"  grammar:    {len(engine.grammar(text))} findings")
    print(f"  style:      {len(engine.style(text))} findings")
    print(f"  typography: {len(engine.typography(text))} findings")


if __name__ == "__main__":
    main()
