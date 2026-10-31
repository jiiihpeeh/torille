# Sentence analyzer plan

Status: **phases 1–2 implemented** · owner: engine / wasm · target: case-aware checking

Goal: replace the per-separator heuristics behind sentence-initial casing
with one deterministic sentence analyzer, and build a second, on-demand
layer that annotates clauses with subject, object, predicate, and case
information.

The analyzer is **not** a full dependency parser and not an NLP model. It
has two layers with different costs:

1. **Sentence boundaries** — cheap enough for every render. Where does a
   sentence start and end? Is this token sentence-initial? Is it inside a
   quotation, heading, or list item?
2. **Clause and role analysis** — on demand. Which token is the
   predicate, subject, object, or adverbial? Which morphological case
   licenses that role? This layer consumes the existing morphology
   engine (`Tags`, `Case`, `BasesReport`) and is allowed to return
   `Unknown` rather than guess.

Everything else (lemma lookup, inflection, suggestions) stays in the
existing engine. The role layer is advisory: a wrong or missing role must
never change spell-checking output.

## Implementation status

Phases 1–2 are implemented:

- `src/sentence.rs` with `TokenContext`, `SentenceSpan`, `analyze`, `spans`,
  `separator_ends_sentence`, and `separator_breaks_line`;
- `Token::ends_sentence` delegates to the sentence module;
- `tokenise::annotate_sentence_initial` is a compatibility wrapper over
  `sentence::analyze`;
- closed abbreviation list (`esim.`, `ym.`, `jne.`, …) and single-initial
  exception (`J. Virtanen`), while ellipses still split;
- unit tests for periods, newlines, colons/quotes, abbreviations, initials,
  spans, and quote depth.

Phases 3+ (state integration, syntax roles `src/syntax.rs`) remain open.

## Why now

The case layer currently infers sentence starts from
`Token::ends_sentence`, a one-token helper in `src/tokenise.rs`. It has
grown by accretion:

- `.`, `!`, `?`, `:`;
- line breaks, because Gutenberg verse and dialogue lines start with a
  capital;
- quote marks, because `»Elä` follows a colon;
- a separate `annotate_sentence_initial` walk in the tokeniser;
- another copy of the state machine in `torille-wasm/src/state.rs` for
  proper-name learning.

That works for the reported examples, but the rules are split across
three places and cannot answer “which sentence is this token in?”. The
sentence analyzer becomes the single source of truth.

The same gap exists one level up: `Tags` already carries `Case`,
`Number`, `Person`, `Mood`, `Voice`, and `Possessive`, but nothing maps
those readings onto sentence roles. Subject/object analysis therefore has
no shared API to build on, and every caller would repeat the Finnish
case rules. `syntax.rs` is that home.

## Current behaviour to preserve

These are contract tests, not aspirations:

| Input | Result |
|---|---|
| `Talo on kaunis.` | `Talo` accepted |
| `… ja Talo istuu.` | `Talo` flagged as case-only |
| `Helsinki on kaupunki.` | `Helsinki` accepted as a proper name |
| `helsinki` | flagged |
| `turkulainen` | accepted lowercase |
| `… ja Helsinkiläinen …` | flagged as case-only |
| `Sami Ruotsalainen` mid-sentence | accepted (surname) |
| `Aloe` mid-sentence | learned, suggested as `Aloe` |
| `Pieniä eläjiä olivat.\nElä ole …` | both line-initial capitals accepted |

The analyzer must not change any non-case engine output. A pure sentence
refactor should change zero `Form`s.

## Non-goals

- No full dependency parser, no semantic role labelling, no treebank
  training.
- No statistical/neural sentence splitter in this plan.
- No new dictionary source. Proper-name decisions keep using
  `data/precomputed/proper_names.bin` and the learned `proper_nouns`
  set.
- No change to `suggest()` matching semantics; only the returned
  surface case (already implemented in `suggest/casing.rs`) and the
  sentence context stay in scope.
- Clause/role analysis does not feed the per-keystroke checker. It is a
  separate API for callers that need grammar context.

## Output model

New root module `src/sentence.rs`:

```rust
pub struct SentenceSpan {
    /// Byte offset in the source text.
    pub start: usize,
    /// Exclusive byte offset.
    pub end: usize,
    /// 0-based sentence number in this text.
    pub index: usize,
    /// True when the sentence is the first in a paragraph or line block.
    pub paragraph_start: bool,
}

pub struct TokenContext {
    pub sentence_index: usize,
    pub sentence_start: bool,
    pub paragraph_start: bool,
    /// Depth of `»`, `"`, `“`, `‘` quotations at this token.
    pub quote_depth: u16,
}

pub fn analyze(tokens: &[crate::tokenise::Token]) -> Vec<TokenContext>;
pub fn spans(tokens: &[crate::tokenise::Token]) -> Vec<SentenceSpan>;
```

`analyze` is O(tokens) and allocation-light: one `Vec<TokenContext>` and,
when requested, one `Vec<SentenceSpan>`. No per-token strings.

The existing `tokenise::annotate_sentence_initial` and
`Token::ends_sentence` become thin deprecated wrappers, or are removed
once all callers use `sentence::analyze`. Keep `Token` itself dumb:
case/context metadata does not belong in the token.

## Boundary rules

Rules are applied in order; the first match decides.

1. **Line break**
   - Any separator containing `\n` starts a new sentence and paragraph.
   - Blank lines are not special beyond this; a single newline is enough
     for verse and dialogue.

2. **Terminal punctuation**
   - `.`, `!`, `?`, `…`, and runs such as `...` end a sentence.
   - A closing quote after terminal punctuation keeps the boundary but
     belongs to the closing sentence, not the next.

3. **Colon**
   - `:` starts a new sentence for case purposes. Finnish dialogue often
     introduces a capital after a colon, and the current behavior already
     accepts that.
   - This is deliberately a **case** rule, not a claim that every colon
     introduces a grammatical sentence.

4. **Abbreviation exception**
   - A period after a known abbreviation does not end a sentence.
   - Start with a closed list:
     `esim.`, `ym.`, `yms.`, `jne.`, `jms.`, `mm.`, `ns.`, `n.`, `s.`,
     `v.`, `t.`, `klo.`, `nro.`, `prof.`, `tri.`, `toht.`, `ym.`
   - A single-letter initial followed by a capitalized word
     (`J. Virtanen`) does not end a sentence.
   - Numbers/decimals (`3.5`, `1.`) must not create a boundary; the
     tokeniser already treats pure numbers as separators, so inspect the
     separator and neighbours rather than assuming every `.` is terminal.

5. **Quote handling**
   - Opening `»`, `“`, `‘`, and `"` after a boundary leaves the first
     word sentence-initial.
   - A closing quote after terminal punctuation belongs to the previous
     sentence.
   - A quote that opens mid-sentence after `:` starts the quoted
     sentence, matching the Gutenberg dialogue case.
   - Nested quotation depth is tracked only for consumers that need it;
     case validation treats the first word after an opening quote as
     sentence-initial.

6. **Headings and list markers**
   - A short line with no terminal punctuation (chapter heading, list
     item) is its own sentence block. For now, the newline rule already
     handles this; do not add a separate Markdown parser.
   - Roman numerals and numbers at line start are ignored by the word
     tokeniser.

## Part II — clause, role, and case analysis

This is the layer that answers “what is this word doing in the sentence?”
It runs after sentence boundaries and after the morphology engine has
produced readings for each word. It is deliberately conservative: when
two analyses are equally plausible, the role is `Unknown` or carries a
low confidence.

### API

New module `src/syntax.rs`:

```rust
pub enum SyntacticRole {
    Predicate,
    Subject,
    Object,
    Adverbial,
    Complement,   // predicative / object complement
    Modifier,     // attribute, genitive modifier, apposition
    Unknown,
}

pub enum RoleConfidence {
    Certain,   // morphology + valency agree
    Likely,    // morphology fits, word order supports it
    Weak,      // positional fallback only
}

pub struct Clause {
    /// Byte span in the source text.
    pub start: usize,
    pub end: usize,
    /// Token indices.
    pub tokens: std::ops::Range<usize>,
    pub predicate: Option<usize>,
    pub subject: Option<usize>,
    pub object: Option<usize>,
    /// True for a negated clause (`en`, `et`, ... + connegative).
    pub negative: bool,
    /// True for an existential construction (`pöydällä on kirja`).
    pub existential: bool,
}

pub struct TokenSyntax {
    pub role: SyntacticRole,
    pub confidence: RoleConfidence,
    pub clause: usize,
    /// Governor token index: the predicate for subject/object/adverbial,
    /// or the head for a modifier.
    pub head: Option<usize>,
    /// The morphological case that licensed the role, if any.
    pub case: Option<crate::tags::Case>,
    /// The chosen lemma reading. A token may have several; keep the
    /// chosen one and the alternatives.
    pub lemma: Option<String>,
    pub readings: Vec<crate::tags::Tags>,
}

pub struct SyntaxAnalysis {
    pub sentences: Vec<crate::sentence::SentenceSpan>,
    pub clauses: Vec<Clause>,
    pub tokens: Vec<TokenSyntax>,
}
```

`analyze(tokens) -> SyntaxAnalysis` is the entry point. It is not called
from `find_misspells`; it is a separate API and a future UI/grammar
feature.

### Morphological input

The role layer does not invent forms. Each word is first resolved through
the existing engine:

- `expand::find_bases_with` for the lemma(s) and generated forms;
- `Form::readings` for `Tags` (`case`, `number`, `person`, `mood`,
  `voice`, `possessive`, `polarity`);
- `Case::Accusative` for total objects, `Case::Partitive` for
  irresultative/negative objects, and the local cases for adverbials.

A word with more than one reading stays ambiguous. The role resolver
scores the readings and emits `RoleConfidence::Likely` or `Weak` instead
of pretending there is one answer.

### Clause segmentation

Split each sentence at:

- a finite verb (the predicate),
- coordinating conjunctions (`ja`, `mutta`, `tai`, `eli`) when they join
  full clauses,
- subordinating conjunctions (`että`, `kun`, `jos`, `vaikka`, `koska`,
  `jotta`) and relative pronouns (`joka`, `mikä`, `ken`),
- an infinitive or participial construction when it has its own object
  or adverbial,
- a colon-introduced quotation (already a sentence boundary for case).

Coordination shares roles when the second clause has no explicit subject
or object: `Sami söi puuron ja joi maidon` has one subject and two
predicates/objects.

### Role rules (first slice)

Finnish marks roles morphologically more than by position. Use this
order:

1. **Predicate**
   - finite verb: `mood` is indicative/conditional/imperative/potential
     and `person` is present;
   - copula `olla` is a predicate, and its complement is handled below;
   - passive verbs have no subject; the logical object is the nominative
     argument (`Talo rakennettiin`).

2. **Subject**
   - agrees with the predicate in person and number;
   - nominative by default;
   - partitive in existential, negative, quantified, and irresultative
     constructions (`Pöydällä on kirjoja`, `Kirjoja ei ole`);
   - genitive in necessity/obligation constructions (`Minun on
     mentävä`) and with some modal expressions;
   - a multiword proper name (`Sami Ruotsalainen`) is one subject; the
     surname is an apposition, not a separate object.

3. **Object**
   - only with a transitive predicate;
   - total object: accusative/genitive (`Luen kirjan`, `Söin
     aamupuuroni`);
   - partitive object: negative, irresultative, quantified, imperative,
     passive, and infinitive constructions (`En syö puuroa`, `Luen
     kirjaa`, `Syö puuroa!`);
   - pronoun accusatives (`minut`, `sinut`, `hänet`, `meidät`,
     `teidät`, `heidät`) are objects, not subjects;
   - a possessive suffix on the object (`aamupuuronsa`) does not change
     the role.

4. **Complement**
   - `olla` and result/change verbs (`tulla`, `tulla joksikin`, `tehdä
     joksikin`) take a nominative, partitive, essive, or translative
     complement;
   - predicative adjectives agree with the subject in number/case where
     the construction requires it.

5. **Adverbial**
   - local cases (inessive, elative, illative, adessive, ablative,
     allative), abessive, comitative, instructive;
   - postposition phrases: the postposition governs genitive or
     partitive (`pöydän alla`, `talon takana`);
   - adverbs and infinitives with adverbial function.

6. **Modifier**
   - genitive/nominative attributes before a nominal head;
   - appositions and titles (`tohtori Virtanen`);
   - compounds are one token and are not split into roles.

7. **Unknown**
   - anything the rules cannot place. Never guess a subject or object
     from word order alone without at least one morphological cue;
     positional fallback may produce `RoleConfidence::Weak`.

### Case knowledge table

| Role | Usual case | Triggers / notes |
|---|---|---|
| Subject | nominative | default, agreement with predicate |
| Subject | partitive | existential, negation, quantifiers, mass/plural indefinite |
| Subject | genitive | necessity/obligation (`minun on mentävä`) |
| Object | accusative / genitive | total/resultative object |
| Object | partitive | negation, irresultative, quantifiers, imperative, passive, infinitive |
| Predicate complement | nominative / partitive / essive / translative | `on`, `tulee`, `tehdään` |
| Adverbial | inessive/elative/illative/adessive/ablative/allative | place, time, instrument, possessor with `olla` |
| Adverbial | abessive/comitative/instructive | manner/ accompaniment |
| Modifier | genitive/nominative | attribute, apposition, title |

`olla` possession (`minulla on kirja`) is an adverbial possessor plus a
subject, not a subject `minulla`.

### Performance and caching

- Sentence boundaries run in the hot render path.
- Syntax analysis is on demand and cached per text generation; a
  14 000-character chapter is not re-parsed on every keystroke.
- The role layer may spend seconds on a long text; it must never block
  the first-pass misspell render or the suggestion popup.
- Cache key: `(text generation, byte range)`; invalidate on edit.

### How the role layer plugs in

- `torille-wasm` may expose `analyze_syntax(text)` later for a grammar
  panel. It is not part of `analyze_misspells`.
- The demo can render roles in a diagnostic view without changing the
  underlay or suggestion protocol.
- Extensions do not need the role layer in this plan unless a future UI
  requests it.
- The sentence-boundary half is still the only part used by case
  validation and proper-name learning.

## Integration

### `torille-wasm/src/state.rs`

Replace both ad-hoc walks with:

```rust
let tokens = torille::tokenise::tokenise(text);
let ctx = torille::sentence::analyze(&tokens);
```

Use `ctx[i].sentence_start` for `find_misspells` case validation and
`ctx[i].sentence_start` for the proper-name learning passes in `update`.
Do not recompute sentence state in either pass.

The `caseOnly` flag stays as it is. The analyzer only decides whether a
capital is licensed; the existing casing policy still decides which
surfaces are acceptable.

`state.rs` does **not** call `syntax::analyze`. Role/case analysis is a
separate, on-demand API; the render path keeps only the cheap sentence
marks.

### `suggest/casing.rs`

No structural change. `case_is_valid(word, sentence_initial, proper)`
continues to receive the analyzer's boolean. The doc comment should link
to `sentence` once it exists.

### Suggestions

`match_case` and `case_is_valid` stay independent of sentence
segmentation. Future work may use `sentence_index` to avoid suggesting
the same name across unrelated sentences, but that is **not** part of
this plan.

### Demo and extensions

The wasm output shape changes only if `TokenContext` is exposed; the
current `analyze_misspells` object remains `word/start/end/caseOnly`.
Rebuild all consumers after the wasm change:

- `torille-wasm/demo`
- `extensions/firefox`
- `extensions/chromium`

No TypeScript API change is required for the first slice.

## File plan

New:

- `src/sentence.rs` — sentence boundaries, token context, spans, unit tests.
- `src/syntax.rs` — clause segmentation, role annotation, case mapping,
  and unit tests.

Changed:

- `src/tokenise.rs` — keep tokenisation; move `ends_sentence` /
  `annotate_sentence_initial` usage out. If the old helpers remain, mark
  them deprecated and implement them through `sentence::analyze`.
- `src/lib.rs` — add `pub mod sentence;` and `pub mod syntax;` if the
  APIs are public. If only wasm needs them, use `pub(crate)` and
  re-export through `torille-wasm`.
- `torille-wasm/src/state.rs` — use `sentence::analyze` in
  `find_misspells` and `update`. Do not call `syntax::analyze` there.
- `torille-wasm/src/lib.rs` — optional `analyze_syntax(text)` export for
  a future grammar panel, not wired into the misspell render.
- `src/suggest/casing.rs` — docs only.
- `AGENTS.md` / `README.md` — repo map entry for `sentence.rs`.

## Tests

### Unit tests in `src/sentence.rs`

- empty input and single word;
- `.`, `!`, `?`, `…`, `...`;
- colon + capital;
- newline and blank-line blocks;
- opening/closing quotes, including `»Elä`;
- abbreviations (`esim.`, `ym.`, `jne.`) do **not** split;
- initials (`J. Virtanen`) do **not** split;
- decimals/numbers do not split;
- start/end byte offsets partition the text exactly once;
- every word token belongs to exactly one span;
- no span splits a word;
- monotonic sentence indices;
- `paragraph_start` is true exactly on the first sentence after a line
  break.

### Regression tests in `torille-wasm/src/state.rs`

Keep and extend the current cases:

- `Pieniä`, `Elä`, `Ehkäpä`, `Vaan`, `Aivan` at line starts;
- `Talo` sentence-initial vs mid-sentence;
- `Helsinki` / `helsinki`;
- `Sami Ruotsalainen`;
- `Helsinkiläinen` mid-sentence;
- `Aloe` learned and returned as `Aloe`;
- book 11879 chapter 0 has no false positives for the reported words.

### Unit tests in `src/syntax.rs`

- `Sami söi aamupuuronsa.` → subject `Sami`, predicate `söi`, object
  `aamupuuronsa`, object case accusative/genitive;
- `En syö puuroa.` → negative predicate, partitive object;
- `Luen kirjaa.` / `Luen kirjan.` → partitive vs total object;
- `Pöydällä on kirja.` → existential subject, adverbial possessor/location;
- `Minun on mentävä.` → genitive necessity subject;
- `Sami söi puuron ja joi maidon.` → coordinated predicates and objects,
  shared subject;
- `Hän tuli iloiseksi.` → translative complement;
- `Talo rakennettiin.` → passive predicate, no subject;
- a multiword proper name is one subject, not an object;
- ambiguous readings produce `Unknown`/`Weak`, never a silent guess.

### Fixture

Add a small checked-in fixture under `tests/fixtures/sentences.tsv`
(`text`, `expected_spans`) rather than depending on the downloaded
Gutenberg corpus in CI. Optionally add a ignored/local test that reads
`torille-wasm/demo/books/11879.txt` when present.

## Phases

1. **Extract** — new `sentence` module with the current boundary rules
   and exactly the same output. No behavior change; all existing tests
   must stay green.
2. **Abbreviations and initials** — add the closed list and the
   single-initial rule. Unit tests only; no consumer change.
3. **Quotes and verse** — replace `ends_sentence` usage in `state.rs`
   with `sentence::analyze`; add the Gutenberg regression tests.
4. **Cleanup** — remove or deprecate `annotate_sentence_initial`;
   update docs and `AGENTS.md`.
5. **Role extraction** — add `syntax.rs` with morphological input and
   `Unknown`-first role assignment. No UI consumer yet.
6. **Clause segmentation and core roles** — finite verbs, subjects,
   objects, complements, adverbials; unit tests for the sentences above.
7. **Coordination and non-finite clauses** — shared subjects/objects,
   infinitives, participles, passive constructions.
8. **Optional wasm export** — `analyze_syntax(text)` for diagnostics; keep
   it out of the keystroke path.
9. **Rebuild** — wasm, demo, both extension bundles.

## Acceptance criteria

- The reported book 11879 chapter 0 words stay unflagged.
- `Talo` is accepted only at a sentence start (or as a name).
- `Ruotsalainen` is accepted mid-sentence.
- `Aloe` is learned and suggested as `Aloe`.
- `cargo test --release --workspace` stays green.
- Clippy is clean.
- Sentence boundaries: a 14 000-character chapter is analyzed in well
  under 1 ms on native and adds no measurable latency to the wasm first
  pass.
- Syntax roles: the test sentences above produce the expected
  subject/object/complement and case, with `Unknown` for genuinely
  ambiguous readings.
- No non-case engine output changes.

## Open questions

1. Should a newline always license a capital, even in hard-wrapped prose?
   It currently does; this favors fewer false positives over catching
   every mid-sentence capitalization error.
2. Should all-caps words remain exempt from case validation? Current
   policy says yes.
3. Should built-in proper names ever be suggested, or only accepted?
   Current policy: accepted, never suggested.
4. Should sentence analysis be exposed through the wasm API, or remain
   internal to `state.rs`? Start internal; expose only if a caller needs
   spans.
5. How much Finnish abbreviation data is worth carrying? The plan starts
   with a closed list; a data file can replace it later without changing
   the analyzer API.
6. How much valency data should live in the engine? The first slice uses
   part of speech, transitivity hints, and a small verb table. A larger
   valency lexicon (which verbs take partitive objects, which take
   complements) is a separate data project.
7. Should role analysis expose a confidence score to callers, or only a
   three-level `RoleConfidence`? Start with the three levels.
8. How should coordination share roles when the second verb has its own
   object but no subject? Start with a shared subject and a fresh object;
   revisit ellipsis if a real text disagrees.
