# Style checker

The style layer is a separate, **opt-in** analyzer. It answers an optional
question — “is this wording repetitive, wordy or clichéd?” — and never claims
that the text is grammatically wrong. The design record is
[`PLAN_STYLE.md`](../PLAN_STYLE.md).

The grammar checker and the style checker share the tokeniser, the sentence
spans and the diagnostic primitives, but not the rule pass. `style_check` never
calls `syntax_check` and never emits an `Error` or a `Warning`.

## Public API

```rust
pub use torille::style_check::{check, StyleAnalysis, StyleDiagnostic, StyleKind};

let analysis = style_check::check("Tällä hetkellä tilanne on rauhallinen.");
for diagnostic in analysis.diagnostics {
    // diagnostic.span, diagnostic.kind, diagnostic.corrections
}
```

`check_tokens(&[Token])` is the same pass over an existing token stream. The
pass is linear and on demand; it is not part of the keystroke path or the
spell-check pipeline.

## Kinds and labels

| kind | UI label | meaning |
|---|---|---|
| `style_repetition` | toistuva sana | the same content lemma repeats within the window |
| `style_long_sentence` | pitkä virke | the sentence is over 35 word tokens |
| `style_cliche` | kulunut ilmaus | a curated stock phrase |
| `style_filler` | täytesana | a curated filler word or phrase |
| `style_wordy` | monisanainen ilmaus | a curated expression with a shorter form |
| `style_colloquial` | arkikielinen ilmaus | a curated informal word |

Every diagnostic is `Severity::Suggestion` and `DiagnosticConfidence::High`
for table hits or `Medium` for the repetition and length metrics. The
`message` is the stable kind name; `expected` carries the matched phrase or
lemma. A correction exists only when a curated table row names a safe
replacement.

## Rules

### Curated table

`data/style_phrases.csv` and `data/style_words.csv` are build inputs. A row is
`key,kind,replacement,note`; the note is curation-only and is not shipped. The
build emits `maps/style_phrases.bin.gz` and `maps/style_words.bin.gz`, rejects
unknown kinds, duplicate keys and phrases longer than four words, and
lowercases/whitespace-normalises the key.

Matching is exact on whole word tokens and case-insensitive. Multi-word
phrases use consecutive word tokens with single spaces between them and do not
cross a sentence boundary. A match covers the whole phrase and is reported
once; the scan resumes after it.

A replacement at the start of a sentence keeps the sentence capital:
`Tällä hetkellä` becomes `Nyt`, not `nyt`.

### Repetition

The same content lemma is reported on its second occurrence when at most ten
word tokens separate the two occurrences in the same sentence.

Skipped: pronouns, numerals, conjunctions, auxiliaries, light verbs (`olla`,
`tulla`, `voida`, `tehdä`), words shorter than four characters and non-initial
proper names. A sentence-initial capital is not a name signal; `Asia` is both
a name and the common noun, so the lowercase sanalista row wins. Phrase-table
spans are excluded, and one lemma is reported once per sentence.

Cross-sentence repetition is deliberately quiet: cohesive prose repeats
deliberately.

### Long sentence

A sentence over **35 word tokens** is a medium-confidence suggestion. The rule
requires a finite predicate, so headings, captions and list fragments are
quiet. A sentence that already has a phrase or repetition finding is skipped,
so one sentence never carries overlapping advice.

## Evaluation

`tests/fixtures/style_diagnostics.json` is the exact-output gate.
`tests/fixtures/style_eval.json` is the aggregate corpus. The current baseline
(2026-11-13):

```text
clean rows 40: 0 table hits, 0 metric findings, budget 2
cases 6: every emitted kind precision 1.00, recall 1.00
```

The clean corpus must have zero `Error`/`Warning`, zero curated table hits and
no more metric findings than one per 20 sentences. A rule that exceeds the
budget is not merged.

## Web and UI

`torille-wasm` exports:

```text
check_style(text) -> { diagnostics: [{ kind, severity, confidence,
                                      start, end, message, expected,
                                      corrections }] }
```

`start`/`end` are UTF-8 byte offsets in wasm; the worker converts them to
UTF-16 for the content script, exactly as `check_syntax` does. `severity` is
always `suggestion`.

The Firefox and Chromium extensions add **Tarkista tyyli** as a separate
context-menu command. It opens a `Tyyli` panel and paints blue style underlines
on the checked text. Findings whose span intersects an ignored range (code,
URLs, BBCode) are dropped before the panel and the underlines are shown. The
Leptos demo has a separate `Tyyli` panel next to `Kielioppi`.

Underlines are colour-coded across the three layers:

| layer | default colour | line style |
|---|---|---|
| spelling | red `#cc3333` | wavy |
| grammar error | orange `#c2410c` | double |
| grammar warning | amber `#d97706` | dashed |
| grammar suggestion | amber `#b45309` | dotted |
| style | blue `#2563eb` | solid |

The spelling colour and line style, the three grammar colours, the style
colour, and separate visibility toggles for grammar and style are all in the
options page under **Kieliopin ja tyylin alleviivaukset**. The values are
applied live through CSS custom properties; contenteditable uses the CSS
Highlight API with the same colours.

Grammar and style results are snapshots. They are cleared on the next edit so
a stale offset never paints over the wrong word; run the command again to
refresh them.

## Performance

The layer is on demand and never runs in the keystroke path. A style pass needs
morphology and sentence spans, not roles or clauses; it shares the per-textarea
morphology cache with the grammar checker when both are enabled. The measured
baseline, the cache design and the cold-path pruning plan are in
[`PLAN_GRAMMAR_STYLE_PERF.md`](../PLAN_GRAMMAR_STYLE_PERF.md).

## Known limitations

- **Table matching is exact.** A phrase inside a longer word
  (`systeeminen` vs `systeemi`) does not match. A clitic or an extra word
  between the phrase parts is a false negative.
- **Repetition is sentence-local.** A cross-sentence echo and a document-level
  density metric are not implemented.
- **The long-sentence threshold is fixed.** It is a readability nudge, not a
  measurement of sentence quality.
- **Corrections are curated.** The layer never generates a rewrite; a finding
  without a table replacement is label-only.
- **The extension command is context-menu only.** There is no per-site toggle
  or keystroke trigger.
- **No register model.** `arkikielinen` is a curated lexical label, not a
  judgement about the writer or the text type.
- **Not in the keystroke path.** Style is always an explicit check.
