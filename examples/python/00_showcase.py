#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""One page through the whole engine.

Reads a short text and walks the layers in the order the checker does:

    tokens -> morphology -> clauses and roles -> grammar/style -> typography

then shows the two public word-level questions (`check`, `suggest`) and the
line-break API. Everything here comes from the `torille` Python extension;
see the neighbouring scripts for one layer each.
"""

from collections import Counter

from torille import Engine

TEXT = (
    "Tällä hetkellä koira nukkuvat talossa. "
    "Hän tuli tupaan, jossa roihusi tuli. "
    "Ostin omenia, appelsiineja ja banaaneja."
)

# A separate sample for the mechanical typography pass: a space before the
# semicolon and a doubled one after the first.
TYPO_TEXT = "Ostin omenia ;appelsiineja;; banaaneja."


def banner(title: str) -> None:
    print()
    print(title)
    print("=" * len(title))


def span_text(text: str, start: int, end: int) -> str:
    """The source text of a byte span.

    Diagnostics carry byte offsets (they index the UTF-8 source), while a
    Python string is indexed in characters; `ä` is two bytes, so the two
    diverge as soon as the text has a Finnish letter.
    """
    return text.encode("utf-8")[start:end].decode("utf-8")


def main() -> None:
    engine = Engine()

    banner("1. Morphology: one row per word token")
    for row in engine.morphology(TEXT):
        reading = ", ".join(row.tags) if row.tags else "-"
        print(
            f"  {row.surface:<16} {str(row.lemma):<14} {str(row.pos):<11} "
            f"{str(row.case):<12} {str(row.number):<9} depth={row.depth:<12} {reading}"
        )

    banner("2. Syntax: clauses, roles and word-order features")
    tokens, clauses = engine.syntax(TEXT)
    for index, clause in enumerate(clauses):
        words = [
            f"{token.surface}:{token.role}"
            for token in tokens
            if token.surface.strip() and token.clause == index
        ]
        features = dict(clause.features)
        print(f"  clause {index}: {' '.join(words)}")
        print(
            f"    predicate={clause.predicate} subject={clause.subject} "
            f"object={clause.object} complement={clause.complement}"
        )
        print(
            f"    type={features.get('clause_type')} "
            f"subject_kind={features.get('subject_kind')} "
            f"negative={clause.negative} passive={clause.passive}"
        )

    banner("3. Grammar and style over one morphology pass")
    grammar, style = engine.grammar_and_style(TEXT)
    for diagnostic in grammar:
        print(
            f"  grammar {diagnostic.severity:<9} {diagnostic.kind:<30} "
            f"[{diagnostic.start}:{diagnostic.end}] {diagnostic.message}"
        )
        for edits, score, note, license_name, confidence in diagnostic.corrections[:1]:
            replacement = "".join(replacement for _, _, replacement in edits)
            print(
                f"    {span_text(TEXT, diagnostic.start, diagnostic.end)!r} -> "
                f"{replacement!r} ({license_name}, {confidence}, score {score}, "
                f"note {note})"
            )
    for diagnostic in style:
        print(
            f"  style   {diagnostic.severity:<9} {diagnostic.kind:<30} "
            f"[{diagnostic.start}:{diagnostic.end}] {diagnostic.expected or diagnostic.message}"
        )

    banner("4. Typography: its own mechanical pass")
    for diagnostic in engine.typography(TYPO_TEXT):
        print(
            f"  {diagnostic.rule:<32} [{diagnostic.start}:{diagnostic.end}] "
            f"-> {diagnostic.replacement!r}"
        )
    print(f"  before: {TYPO_TEXT}")
    print(f"  after:  {engine.apply_typography(TYPO_TEXT)}")

    banner("5. Is it spelled right, and what did the user mean?")
    for word in ["talossa", "kahvikekkereitä"]:
        outcome = engine.check(word)
        print(f"  check({word!r}) -> {outcome.kind}, bases={outcome.bases}")
    for word in ["äitj", "Äitj"]:
        outcome = engine.check(word)
        print(f"  check({word!r}) -> {outcome.kind}")
        for suggestion in engine.suggest(word, 3, 2):
            print(
                f"    suggest {suggestion.word:<14} d={suggestion.edit_distance} "
                f"score={suggestion.existing_score:.2f} from {suggestion.source}"
            )

    banner("6. Line breaks")
    word = "tietokoneohjelmointi"
    breaks = engine.hyphenate(word)
    print(f"  {word}: {[(p.char_offset, p.source) for p in breaks]}")
    fragments, previous = [], 0
    for point in breaks:
        fragments.append(word[previous : point.char_offset])
        previous = point.char_offset
    fragments.append(word[previous:])
    print(f"  split: {'-'.join(fragments)}")

    banner("7. What the text is made of")
    counts = Counter(f.confidence for f in engine.inflect("talo"))
    print(f"  talo has {sum(counts.values())} forms: {dict(counts)}")
    print(f"  lexicon: {len(engine.words('substantiivi'))} classed noun lemmas")


if __name__ == "__main__":
    main()
