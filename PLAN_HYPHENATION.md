# Hyphenation plan

Status: milestone 1–3 implemented in `src/hyphenation/`, with CLI, wasm
export and golden tests. The Python binding and demo UI are still open;
see “Implementation status” below.

Goal: add Finnish line-break hyphenation to `torille` as a separate
typographic layer that reuses the morphology engine for compound
structure, without changing spell-checking or suggestion behaviour.

The design follows the three-way split in the request:

- `PatternHyphenator` — ordinary Knuth–Liang candidates.
- `MorphologyAdapter` — compound structure from the existing lexicon.
- `FinnishHyphenationPolicy` — what Finnish typography permits and prefers.

Keeping these separate means the pattern set, the morphology, and the
typographic rules can be tested and replaced independently.

## Implementation status

Implemented:

- vendored `hyph-fi-x-school` patterns with their MIT notice;
- parser, matcher and UTF-8-safe offsets;
- `HyphenationOptions`, `VowelBreaks`, `HyphenationPoint`, `BreakSource`;
- `positions`, `positions_with`, `split`;
- `LexicalCompoundAnalyzer`, classless-compound guard, recursive
  partition and ambiguity scoring;
- per-component hyphenation, so `veroprosentti` is not
  `verop-rosentti`;
- minimum fragments, vowel policy, penalties and `Exceptions`;
- `tools/src/bin/hyphenate.rs` with `--json`;
- wasm `hyphenate` export;
- unit tests and `tests/hyphenation_fi.rs`;
- docs in `docs/hyphenation.md`.

Still open:

- Python binding exposure;
- browser demo/extension presentation;
- full Voikko diagnostic triage (the comparison script exists; the
  curated corpus is still the gate) and the full `FormBuilder` adapter;
- soft-hyphen handling;
- performance review for whole-paragraph use.

## 1. Pattern source

Use the **MIT-licensed school patterns** from `hyph-utf8`, not the old
"freely distributable" default Finnish patterns.

Proposed vendored file:

```
data/hyphenation/hyph-fi-x-school.tex
data/hyphenation/LICENSE
data/hyphenation/README.md
```

Provenance:

- `hyph-fi-x-school`, version 1.0 (2021-03-21)
- Authors: Teemu Likonen, Arthur Rosendahl
- Licence: MIT
- It is the same pattern set as `hyphenation/basic-finnish`, but the
  upstream file carries the licence and authorship explicitly.

Why not the default `hyph-fi` (Kauko Saarinen) patterns? They encode
compound guesses that the morphology layer can do better, and they are
known to produce bad simple-word breaks (`paprika` → `papri-ka`,
`housut` → no break). The school/basic set produces `pap-ri-ka`,
`hou-sut`, and `ra-dio` for simple words; morphology supplies the
compound boundaries.

The file is data, not engine source. Add it to `data/LICENSE`, `NOTICE`,
and `data/hyphenation/README.md` with its own MIT notice. Do not
relicense it as `MIT OR Apache-2.0` engine code.

Parse it at runtime once through a `OnceLock` (about 315 patterns, a few
KB), or precompile it into a small trie in `build.rs` if startup cost
shows up in wasm. Parsing a few KB once is the simpler first version.

## 2. New module layout

```
src/hyphenation/
  mod.rs          public API, options, errors
  patterns.rs     pattern parser, trie/matcher, `.` boundaries
  finnish.rs      min fragments, vowel rules, fragment penalties
  compounds.rs    compound-boundary adapter
  exceptions.rs   built-in and user overrides
  offsets.rs      char/byte offset conversion (UTF-8-safe)
```

Public API (crate root re-export):

```rust
pub struct HyphenationOptions {
    pub left_min: usize,             // default 2 characters
    pub right_min: usize,            // default 2 characters
    pub compound_breaks: bool,       // default true
    pub vowel_breaks: VowelBreaks,   // default CompoundOnly
    pub prefer_compound: bool,       // default true
}

pub enum VowelBreaks {
    CompoundOnly, // default: only a compound boundary or an exception
    Never,
    Always,       // for syllable work / diagnostics
}

pub enum BreakSource {
    Pattern,
    CompoundBoundary,
    ExplicitException,
}

pub struct HyphenationPoint {
    pub byte_offset: usize,          // always a char boundary
    pub char_offset: usize,
    pub compound_boundary: bool,
    pub source: BreakSource,
    pub penalty: u16,                // ranking hint, not a confidence score
}

pub fn positions(word: &str) -> Vec<HyphenationPoint>;
pub fn positions_with(
    word: &str,
    options: &HyphenationOptions,
    analyzer: &dyn CompoundAnalyzer,
    exceptions: &Exceptions,
) -> Vec<HyphenationPoint>;

pub fn split<'a>(word: &'a str, points: &[HyphenationPoint]) -> Vec<&'a str>;
```

The primary representation is **byte offsets**, because callers slice
Rust strings. `char_offset` is exposed for editing and display. Every
offset is produced from `char_indices`, never from a byte walk.

`positions()` returns **permitted** breaks. `penalty` is a ranking hint
for a caller that chooses among them; it is not a statistical
confidence. A future line-layout layer can combine the penalties with
line width and neighbouring lines. The core does not pretend to choose
the one best break without knowing the layout.

`split` is a convenience that returns borrowed slices; it never drops or
duplicates source characters.

## 3. Hyphenation pipeline

For a word:

1. Validate and normalize: lowercase for pattern lookup, remember the
   original byte positions. Only alphabetic runs are hyphenated.
2. If the word contains an existing hyphen (`linja-auto`), split it and
   hyphenate each part separately; the hyphen itself is an explicit
   break.
3. Obtain a compound segmentation from the adapter.
4. If a segmentation is chosen, hyphenate **each component separately**,
   then add the component boundaries. Do not run the whole-word patterns
   across a compound boundary: that is what produces `verop-rosentti`
   and `koneoh-jelmointi`-style junk.
5. If no segmentation is chosen, run the patterns on the whole word.
6. Apply the policy: minimum fragments, vowel rules, single-character
   rules, exception allow/deny.
7. Add, remove, and rank explicit exceptions last.
8. Return sorted, deduplicated points.

The per-component step is important. The school patterns applied to
`vero` + `prosentti` give `ve-ro` + `pro-sent-ti`; applied to the whole
word they give `ve-rop-ro-sent-ti`. Compound analysis must therefore
supply the partition, not merely a suggested boundary.

## 4. Finnish policy

Hard constraints:

- Break only at a UTF-8 character boundary.
- Do not produce an empty, one-character, or too-short fragment:
  `left_min` and `right_min` default to 2.
- Do not leave a single vowel alone at either end (covered by the
  minimum, and by the vowel rules below).
- Do not add a break merely because a morpheme or inflectional suffix
  starts there. Morphology contributes **compound boundaries only**;
  ordinary inflection is part of the written word.
- No break between two vowels by default. A break between vowels is
  permitted only when it is a recognised compound boundary or an
  explicit loanword/proper-name exception.
- An explicit exception can allow or deny any position, subject to the
  character-boundary check.

Soft preferences, expressed as penalties:

- A compound boundary outranks a pattern-only break (`penalty = 0` vs a
  small positive value).
- A break followed by a vowel-initial fragment is penalised, because
  Kotus recommends not starting the next line with a vowel when another
  break is available.
- A break inside a recognised compound is fine if it is a normal pattern
  break inside that component; it is not automatically preferred over
  the component boundary.

Expected examples:

| word | expected result |
|---|---|
| `matka` | `mat-ka` |
| `laivamatka` | `laiva-matka` preferred; `lai-va-mat-ka` also permitted |
| `radio` | `ra-dio`; never `radi-o` |
| `asia` | no permitted break at the default minimum |
| `veroprosentti` | `ve-ro-pro-sent-ti` |
| `tietokoneohjelmointi` | `tie-to-ko-ne-oh-jel-moin-ti` |
| `autonostaja` | `au-to-nos-ta-ja`, avoiding the `auton-ostaja` reading |
| `Äänekoski` | valid UTF-8 offsets, reassembly exact |

## 5. Morphology adapter

`CompoundAnalyzer` is a narrow trait, not a dependency on `FormBuilder`:

```rust
pub trait CompoundAnalyzer {
    fn analyses(&self, word: &str) -> Vec<CompoundAnalysis>;
}

pub struct CompoundAnalysis {
    pub parts: Vec<String>,
    pub boundaries: Vec<usize>, // byte offsets into the original word
}
```

Two implementations:

- `LexicalCompoundAnalyzer` (default, wasm-safe): uses the existing
  `LexiconIndex`, `modifier_is_known`, `is_known_word_no_compound`,
  `all_forms` and derived-head checks. This is the same bounded
  machinery `is_known_compound_fast` already uses; it does not build the
  reverse index.
- `BuilderCompoundAnalyzer<'a>` (tools/CLI/tests): wraps
  `compound::analyze` with a caller-supplied `FormBuilder` for the full
  answer. Never built implicitly per word, and never used in the wasm
  hot path.

Selection rules for an ambiguous word:

- Prefer a split whose modifier is a lemma or bound prefix over one whose
  modifier is only an inflected form (`auto` + `nostaja` over `auton` +
  `ostaja`).
- Prefer a split whose head is a content word or known derived nominal.
- Prefer the sanalista's own class-50/51 split when the whole word is
  listed.
- If two analyses tie, do not silently pick the first: either return
  both analyses for diagnostics and use the conservative pattern-only
  partition, or make the tie an explicit policy choice. This is what
  prevents `autonostaja` from becoming a confident wrong break.

Reuse, don't duplicate: factor the split checks currently living in
`suggest/known.rs::detect_compound_pass` into a helper that can yield
candidate boundaries; keep the existing single-answer behaviour as the
first/best candidate so spell-check does not change.

Morphology availability must not break the fallback. With
`LexicalCompoundAnalyzer` returning nothing, `positions` must still
return the pattern-only result and satisfy all offset/fragment
invariants.

## 6. Exceptions

Built-in data:

```
data/hyphenation/exceptions.csv
```

Suggested simple schema:

```
word	allow	deny	note
kofeiini	4		loanword vowel break
```

- `allow` / `deny` are character offsets after which a break is forced
  or forbidden. `allow` also overrides the vowel filter.
- The file is embedded with `include_str!`; parsing is lazy and
  allocation-light.
- User code can supply an `Exceptions` value with the same semantics.
- Built-in exceptions are for loanwords and established proper names;
  they are not a place to patch every missing morphology rule. A missing
  compound belongs in the compound layer.

Initial candidates: `kofeiini` (loanword vowel break), and any
proper-name fixture the tests need. Do not add broad suffix lists.

## 7. Integration surfaces

### Library

`pub mod hyphenation` in `src/lib.rs`, with the types above re-exported
at the crate root if they are part of the public API.

### CLI

Add `tools/src/bin/hyphenate.rs` and a `[[bin]]` entry:

```
hyphenate [--left N] [--right N] [--no-compounds] [--vowel-breaks MODE] [--json] WORD...
```

Default output is the word with all permitted breaks shown, plus a
machine-readable `--json` mode with offsets, source and penalty. This is
the quick manual probe and the input to the verification script.

### wasm

Add one exported function, shaped for JS:

```rust
#[wasm_bindgen]
pub fn hyphenate(word: &str, left_min: u32, right_min: u32, compounds: bool)
    -> js_sys::Array;
```

Return objects with `byteOffset`, `charOffset`, `compound`, `source`
and `penalty`. Keep it synchronous and allocation-bounded; the fast
lexical adapter only. Rebuild `torille-wasm/pkg` and the demo bundle
after the engine change even if no UI consumes it yet.

### Python binding

Optional in milestone 5: add `Engine.hyphenate(word, ...)` returning the
same point fields. The binding is for the harness, so this is useful for
corpus checks but not required for the first usable release.

### Browser extension / demo UI

Not part of the first milestone. If added later, hyphenation is a
presentation concern: the extension overlay can use native CSS
`hyphens` or render soft hyphens from the same points. Do not make the
spell-check overlay depend on hyphenation.

## 8. Tests

Unit tests beside each module:

- pattern parser accepts the vendored file and rejects malformed lines;
- matcher gives the expected scores for a handful of patterns;
- byte/char offset conversion for `ä`, `ö`, `å`, `š`, `ž`;
- min-fragment and single-character rules;
- vowel filter with `CompoundOnly`, `Never`, `Always`;
- exception allow/deny precedence;
- `split` reassembles exactly.

Golden integration corpus (`tests/hyphenation_fi.rs`) with a table of
`(word, expected permitted offsets, reason)`, using the examples above
plus Kotus/Kielitoimisto cases and repo words.

Property tests without a new dependency: iterate a sample of the
sanalista and assert:

- every offset is a valid UTF-8 boundary;
- offsets strictly increase and are unique;
- no fragment is shorter than the configured minimum;
- joining the split parts equals the input;
- disabling the morphology adapter changes only compound-related
  positions, never corrupts the pattern fallback;
- with `VowelBreaks::Never`, no returned boundary has vowels on both
  sides.

Morphology tests:

- `laivamatka` gets the component boundary after `laiva`;
- `veroprosentti` is hyphenated component-wise, not as
  `verop-rosentti`;
- `autonostaja` prefers `auto` + `nostaja` and does not confidently
  choose `auton` + `ostaja`;
- `tietokoneohjelmointi` gets all three component boundaries;
- `linja-auto` hyphenates each side and keeps the existing hyphen;
- a word with no compound analysis still returns pattern positions.

## 9. External evaluation

Add `tools/check_hyphenation_against_voikko.py` (GPL-3.0-or-later,
because it imports `libvoikko`; add it to the GPL list in
`tools/README.md`). It runs the engine's permitted breaks against
Voikko's `hyphenate()` for a word list and reports:

- `missing` — Voikko break not offered;
- `extra` — engine break Voikko does not offer;
- `vowel` — extra break between vowels;
- `fragment` — short-fragment candidate.

This is a **diagnostic**, not a CI gate. Voikko returns syllabification,
not the same line-breaking policy, so differences such as allowed
geminate breaks and compound preferences must be reviewed, not counted
as failures. The curated golden corpus is the gate.

Optionally compare against the vendored pattern algorithm alone to show
how much the morphology adapter adds.

## 10. Milestones

1. **Pattern baseline**
   Vendor `hyph-fi-x-school` with its MIT notice; implement the parser
   and matcher; expose `positions` / `split`; add the CLI; pin simple
   words (`matka`, `radio`, `asia`, `paprika`, `housut`, `kofeiini`).

2. **Morphology integration**
   Factor candidate compound boundaries out of the spell-check detector;
   implement `LexicalCompoundAnalyzer`; partition compounds before
   pattern matching; add the ambiguity ranking; add the wasm function.

3. **Finnish policy**
   Implement min fragments, vowel filtering, fragment penalties, and the
   exception table; add the golden examples (`laivamatka`,
   `veroprosentti`, `autonostaja`, `tietokoneohjelmointi`).

4. **Corpus and evaluation**
   Run the Voikko diagnostic over the Kotus lemmas; review the largest
   difference classes; add reviewed exceptions; add property tests and
   a timing check (per-word cost, no full `FormBuilder` in the hot
   path).

5. **Polish**
   Python binding if wanted; `docs/hyphenation.md`; README/AGENTS
   pointers; regenerate wasm and demo; add `PLAN_HYPHENATION.md` to the
   docs map if the plan is kept.

## 11. Risks and decisions to settle

- **Pattern set**: proposed `hyph-fi-x-school` (MIT). The default
  `hyph-fi` patterns are more compound-aware but have known simple-word
  defects; morphology makes their compound guesses redundant.
- **Compound recall**: the fast adapter will miss rare derived heads.
  Pattern fallback still produces a usable answer; full recall needs
  the `FormBuilder` adapter, which is too heavy for wasm.
- **Ambiguous compounds**: `autonostaja` has two plausible readings.
  The plan refuses to make an arbitrary first-match choice; the exact
  tie policy needs a product decision.
- **Vowel breaks in loanwords**: a hard default ban loses some valid
  breaks; the exception table must be curated and reviewed.
- **Whitespace/hyphenated phrases**: `Helsingin Sanomat -ravintola`
  is a phrase, not a single word. The first version handles single
  orthographic words and existing hyphens; phrase-level line breaking
  is out of scope.
- **Soft hyphens**: decide whether an existing U+00AD is preserved as
  an explicit break and whether the returned offsets refer to the
  original string including it. Recommended: treat it as an explicit
  break and keep the input untouched.

## 12. Non-goals for the first release

- Full paragraph line-breaking or line-width optimisation.
- Guessing hyphenation for arbitrary non-Finnish words.
- Changing `check`/`suggest` or the spell-check overlay.
- A new pattern dictionary format or a runtime data file.
- A neural or statistical hyphenation model.
