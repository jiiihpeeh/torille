# Hyphenation

`torille::hyphenation` answers a different question from `check` and
`suggest`: where may a Finnish word be broken across a line?

It does **not** choose a single best break without knowing line width.
It returns permitted points with offsets, their source, and a ranking
penalty. A layout engine can then choose among them.

## Quick use

```rust
use torille::hyphenation::{positions, split};

let points = positions("tietokoneohjelmointi");
let parts = split("tietokoneohjelmointi", &points);
assert_eq!(parts.concat(), "tietokoneohjelmointi");
```

Default output:

```text
tie-to-ko-ne-oh-jel-moin-ti
```

`positions` uses `HyphenationOptions::default()` and
`LexicalCompoundAnalyzer`. Use `positions_with` to supply options, a
pattern-only analyzer, or user exceptions.

## The three layers

1. **Patterns** (`src/hyphenation/patterns.rs`). The MIT-licensed Finnish
   school patterns from `hyph-utf8`, vendored at
   `data/hyphenation/hyph-fi-x-school.tex`. They produce ordinary
   Knuth–Liang candidates.
2. **Compounds** (`src/hyphenation/compounds.rs`). `LexicalCompoundAnalyzer`
   partitions a recognised compound into components using the embedded
   lexicon and the fast word gates. It never builds the full
   `FormBuilder` reverse index.
3. **Finnish policy** (`src/hyphenation/mod.rs` and `finnish.rs`).
   Minimum fragments, vowel rules, ranking and explicit exceptions.

The separation is deliberate: a pattern change cannot change the
morphology layer, and a morphology change cannot silently turn every
derivation into a compound.

## Policy

- Minimum two characters on each side by default.
- No break at a single-character fragment.
- A break between two vowels is removed unless it is a recognised
  compound boundary or an explicit exception. `VowelBreaks::Always`
  keeps all pattern candidates for syllabification/diagnostics;
  `VowelBreaks::Never` removes even compound and exception vowel breaks.
- Compound boundaries get penalty `0` and are marked
  `compound_boundary = true`; pattern candidates start at penalty `10`.
  A pattern break followed by a vowel-initial fragment gets a small extra
  penalty.
- Morphology adds **compound boundaries only**. It does not add a break
  at every inflectional suffix.
- Existing hyphens in the input (`linja-auto`) are preserved and each
  side is hyphenated separately. An existing hyphen is not returned as a
  generated point.
- `U+00AD` (soft hyphen) is not treated as a pattern break in this
  version; callers that already have soft hyphens should strip or honour
  them before calling. This is a known follow-up.

## Offsets

`HyphenationPoint::byte_offset` is the primary representation and is
always a valid UTF-8 character boundary. `char_offset` is provided for
editing and display. `split` returns borrowed slices and never loses or
duplicates a character.

## Compounds and ambiguity

The analyzer is conservative:

- a word the sanalista lists with a real inflection class is one lexical
  unit (`kansalaisuus`);
- a classless row may still be a compound (`tietokone`, `laivamatka`,
  `veroprosentti`, `aamukahvi`);
- class 50/51 rows are compounds by declaration;
- components must be known modifiers/heads, and a head must be a content
  word.

For `autonostaja`, the scored partition is `auto` + `nostaja`, not
`auton` + `ostaja`: a lemma modifier outranks an inflected one. If no
defensible partition is found, hyphenation falls back to the patterns.

The fast analyzer deliberately misses rare derived heads. That is a
recall limitation, not a correctness failure: the pattern fallback still
returns usable points. A full `FormBuilder` adapter belongs in tools and
tests, not the wasm hot path.

## Exceptions

Built-in rows are in `data/hyphenation/exceptions.csv`:

```text
word,allow,deny,note
kofeiini,4,,loanword vowel break
```

Offsets are character counts from the start. `allow` overrides the
vowel filter; `deny` removes a candidate. User code can build its own
`Exceptions` value:

```rust
let mut exceptions = torille::hyphenation::Exceptions::empty();
exceptions.allow("Nimeni", 3);
exceptions.deny("Nimeni", 2);
```

Keep this table small. A missing compound belongs in the compound layer,
not in a growing suffix table.

## Surfaces

- Library: `torille::hyphenation`.
- CLI: `hyphenate [--left N] [--right N] [--no-compounds] [--vowel-breaks MODE] [--json] WORD...`
- wasm: `hyphenate(word, left_min, right_min, compound_breaks)` returns
  objects with `byteOffset`, `charOffset`, `compound`, `source` and
  `penalty`.
- The Python binding and browser overlay do not yet expose it.

## Tests

- `src/hyphenation/*` unit tests: pattern parsing, offsets, exceptions,
  policy, compound partition.
- `tests/hyphenation_fi.rs`: golden words and invariants over a sanalista
  sample (char boundaries, minimum fragments, strict ordering,
  reassembly).
- `tools/check_hyphenation_against_voikko.py` is a diagnostic comparison
  with Voikko (GPL, like the other Voikko tools). It is not a gate:
  Voikko returns syllabification, while this layer applies line-break
  policy.
- The plan and open issues are in [`PLAN_HYPHENATION.md`](../PLAN_HYPHENATION.md).

## Pattern provenance

`hyph-fi-x-school`, version 1.0 (2021-03-21), by Teemu Likonen and
Arthur Rosendahl, is MIT-licensed. The vendored file and its licence are
in `data/hyphenation/`. It is data, not engine source, and is not
relicensed by `MIT OR Apache-2.0`.
