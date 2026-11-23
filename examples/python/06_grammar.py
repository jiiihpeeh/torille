#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Grammar diagnostics: severities, byte spans, corrections, re-checking.

A diagnostic carries a stable rule id (the ignore-list key), its severity and
confidence, a byte span, and one or more corrections. The correction edits are
byte ranges too, so apply them on the encoded text.
"""

from torille import Engine

TEXT = (
    "Koira nukkuvat pihalla. Me olen iloinen. "
    "Minä menen sinä tulet. Ostin omenia, appelsiineja ja banaaneja."
)


def span_text(text: str, start: int, end: int) -> str:
    """The source of a byte span (Python indexes characters, diagnostics bytes)."""
    return text.encode("utf-8")[start:end].decode("utf-8")


def apply_edits(text: str, edits) -> str:
    """Apply `(start, end, replacement)` byte edits from the end."""
    raw = text.encode("utf-8")
    for start, end, replacement in sorted(edits, reverse=True):
        raw = raw[:start] + replacement.encode("utf-8") + raw[end:]
    return raw.decode("utf-8")


def main() -> None:
    engine = Engine()
    diagnostics = engine.grammar(TEXT)

    print(f"text: {TEXT}\n")
    for diagnostic in diagnostics:
        surface = span_text(TEXT, diagnostic.start, diagnostic.end)
        print(
            f"{diagnostic.severity:<9} {diagnostic.kind:<30} "
            f"[{diagnostic.start}:{diagnostic.end}] {surface!r}"
        )
        print(
            f"  rule_id={diagnostic.rule_id} confidence={diagnostic.confidence} "
            f"message={diagnostic.message} expected={diagnostic.expected}"
        )
        for edits, score, note, license_name, confidence in diagnostic.corrections:
            replacement = "".join(text for _, _, text in edits)
            print(
                f"  -> {replacement!r} ({license_name}, {confidence}, "
                f"score {score:.1f}, note {note})"
            )

    if not diagnostics:
        return

    print("\nApply every edit of the first diagnostic and re-check:")
    first = diagnostics[0]
    edits = [edit for correction in first.corrections[:1] for edit in correction[0]]
    fixed = apply_edits(TEXT, edits)
    print(f"  {fixed}")
    after = engine.grammar(fixed)
    before_same = sum(1 for d in diagnostics if d.kind == first.kind)
    after_same = sum(1 for d in after if d.kind == first.kind)
    print(f"  findings before: {len(diagnostics)}, after: {len(after)}")
    print(f"  {first.kind}: {before_same} -> {after_same}")


if __name__ == "__main__":
    main()
