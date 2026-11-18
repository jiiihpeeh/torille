# Word wizard plan

Status: **implemented.** `src/class_wizard.rs` has the sheet, the parsers, the
tail-weighted inference (nominal and verb), the eval hook and the saved-word
editing; `tools/src/bin/word_wizard.rs` is the CLI; `data/class_examples.tsv` is
the generated 76-class sheet; the wasm exposes the wizard and user-word
functions; `torille-wasm/demo/wizard.html` stores in `localStorage`, and the
Firefox and Chromium **Omat sanat** pages store in `browser.storage.local` and
teach the spell worker the saved forms (accepted and suggested within two
edits). The oracle curve is measured in `tools/wizard_eval.py`; only optional
window dressing remains. This plan covers the
**user-interaction phase**: a browser tool that finds a word's Kotus class from
example forms. The build-phase CLI is the same core with a different front end;
it is a reuse, not the first deliverable.

## The problem

A word missing from the sanalista — a name, a new loanword, a dialect word, a
compound the dictionary does not list — has no paradigm. To add it to a
personal word list, or to the dictionary, someone has to name its Kotus class
(`5*A`, `12`, ...). The class is a number plus a gradation letter. A normal
user does not know it, and getting it wrong produces systematically wrong
forms.

The existing reverse tool, `tools/classify_lemma.py`, solves this for a
contributor with Omorfi + Voikko: it generates the full oracle paradigm and
scores every class against it. It is the right tool for the build phase and the
wrong tool for a user. It needs two GPL C++ libraries, takes about a second per
word, needs the Python binding, and cannot run in a browser.

The wizard asks only for forms the user already knows. It compares those to
what the engine would generate under each candidate class and returns the
class, the evidence, and the paradigm.

## What the user does

1. Type the word; choose noun, adjective, numeral, ordinal, pronoun, proper
   noun, or verb.
2. The wizard says whether the word is already in the dictionary and, if so,
   its recorded class.
3. It asks 1–5 questions, each with an example: *genitive singular? `talo` →
   `talon`*. The user types `mökin`.
4. The candidate list narrows live. A **Browse examples** panel lists the
   classes with their model paradigms, for a user who would rather compare
   than answer.
5. The wizard shows the winning class, the confidence, which answers matched,
   the rival classes and where they differ, and the full paradigm of the
   user's word.
6. **Save** writes a line to the user's word list in browser storage.
   **Export** downloads the same text.

### Worked example

`mökki` is `5*A`: genitive `mökin`, partitive `mökkiä`, illative `mökkiin`.
Suppose it were not in the dictionary.

| question | answer | class 5 predicts | class `5*A` predicts |
|---|---|---|---|
| `gen_sg` | `mökin` | `mökkin` (miss) | `mökin` (hit) |
| `par_sg` | `mökkiä` | `mökkiä` (hit) | `mökkiä` (hit) |
| `ill_sg` | `mökkiin` | `mökkiin` (hit) | `mökkiin` (hit) |

The first answer already separates the gradation. The next two confirm the
class. The result is `5*A`, with the full `mökki` paradigm rendered by the
engine.

## Constraints from the request

- **User phase, browser.** The primary surface is the demo page first, then the
  extension's options page. The CLI is a test/debug front end over the same
  core.
- **Text-based formats.** Every input and stored artefact is a small TSV. No
  JSON on disk, no SQLite in the browser, no binary formats.
- **Read as hashmaps.** The sheet is `Map<class, Map<key, value>>`; answers are
  `Map<cell, form>`; saved words are `Map<word, {class, pos}>`. Parse once, query
  by key.
- **Browser storage.** `localStorage` for the demo and `browser.storage.local`
  for the extension, with the same key and the same text on both. The text is
  exportable and importable, so the user owns the data.

## Non-goals

- **No new inflection logic.** The engine is the only authority. The wizard
  asks `expand::inflect_in` what each class would generate and compares
  strings. It never appends an ending itself.
- **No change to `check`, `suggest`, `inflect` or the reverse index.** The
  wizard is additive. The extension's custom-word layer sits above the wasm.
- **No Omorfi/Voikko at runtime.** The oracle stays in `tools/`, GPL, for
  evaluation.
- **No automatic edit of `data/nykysuomensanalista.csv`.** The build CLI prints
  the row and the `add_sanalista_word.py` command; a human chooses the anchor.
- **No statistical guesser.** The result is a ranked explanation, not a
  probability.

## Text formats

All three are UTF-8 TSV. A line whose first field starts with `#` is a comment;
blank lines are ignored; fields are trimmed but not unescaped (a tab cannot
appear in a field). The first line is a version comment.

### `data/class_examples.tsv` — the read-only class sheet

```tsv
# torille-class-examples v1
# class	key	value
1	model	äly
1	pos	nominal
1	lemmas	2151
1	nom_sg	äly
1	gen_sg	älyn
1	par_sg	älyä
1	ill_sg	älyyn
1	nom_pl	älyt
1	gen_pl	älyjen
1	par_pl	älyjä
1	ill_pl	älyihin
5	model	aari
5	gen_sg	aarin
...
```

- One section per class. Nominal classes 1–49 use the eight catalogue slots;
  verb classes 52–78 use the six finite-form cells (`pres_1sg`, `pres_3sg`,
  `past_3sg`, `cond_3sg`, `imp_2sg`, `pres_1pl`). 50/51 are compound markers and
  are omitted. Classes 76–78 have no catalogue model, so they use a short
  representative lemma (`tietää`, `vipajaa`, `raksaa`). Classes 77/78 are
  defective sound verbs and have only the third-person present and conditional.
- `model` is `Catalogues::model()`; `pos` is `nominal` or `verb`; `lemmas` is
  the count of lemmas with that base class, for ordering the exemplar list. The
  count is informational: a sanalista addition does not force a regeneration.
- The form rows are what the engine generates for the model in the class's
  cells. A cell with several alternants shows the first plain one. The sheet is
  therefore the engine's rendering, not a second paradigm source; the
  catalogue's own correctness is `tests/catalogue.rs`'s job. This is what the
  wizard will actually generate, so the UI and the inference cannot disagree.
- Generated, committed, and checked. `--check-sheet` re-renders the sheet and
  compares the parsed rows; a class-engine or lexicon change turns it red until
  the file is regenerated, exactly like the derived blobs.
- About 780 rows, ~20 KB. Embedded with `include_str!` and exposed by the wasm,
  so there is one copy and no fetch.

### `answers.tsv` — one wizard session

```tsv
# torille-wizard-answers v1
gen_sg	mökin
par_sg	mökkiä
ill_sg	mökkiin
```

- Two columns: cell key, form. An empty value means the question was skipped.
- The browser keeps the same text in `localStorage["torille-wizard-session"]`
  while a session is open.
- The CLI accepts `--answers FILE` or repeated `--form gen_sg=mökin`.

### `user-words.tsv` — the saved list

```tsv
# torille-user-words v1
mökki	5*A	substantiivi
Helsinki	5	erisnimi
kahdeksas	45	järjestysluku
taipumaton sana	99	substantiivi
```

- Three columns: word, class expression, part of speech. The class may be a dual
  reading (`9, 10`) and is read by `kotus::parse_inflections`. The part-of-speech
  label is kept as the user chose it (`erisnimi`, `numeraali`, ...); `erisnimi`
  inflects as a noun.
- The last line for a word wins; a delete rewrites the blob without that word.
- Stored as one string under `torille-user-words`; export/import uses the same
  bytes.
- Matching is case-insensitive; the capitalization is preserved for display.

### Parsing rules

- Split on `\t`. A line with the wrong column count is rejected with its line
  number, not silently skipped.
- Strip `\r`, so a CRLF file from a text editor works.
- `#` is a comment only at the start of the first field; it is not an inline
  comment.
- Duplicate keys: last wins (an answer can be corrected; a word can be
  replaced). The sheet checker reports duplicates as a warning.
- Normalize NFC and trim. Matching lowercases both sides; display keeps the
  original.
- Caps: 10,000 lines, 256 bytes per line, 64 characters per word. Reject control
  characters. The stored blob is user input; it must not become a memory
  problem.
- A version newer than the parser's major is refused with a message, not
  misread.

### Hash-map shapes

Rust:

```rust
type Sheet = HashMap<String, HashMap<String, String>>; // class -> key -> value
type Answers = HashMap<String, String>;               // cell -> form
struct UserWord { word: String, class: String, pos: PartOfSpeech }
type UserWords = HashMap<String, UserWord>;           // lowercased word -> entry
```

JS:

```js
const sheet = new Map();   // class -> Map(key -> value)
const answers = new Map(); // cell -> form
const words = new Map();   // lowercased word -> { word, class, pos }
```

The raw maps are the parse output. A typed layer (`Exemplar`, `Candidate`,
`UserWord`) is built once and used by the rest; the maps are not passed around
stringly-typed.

The core's parser is canonical. The browser needs the sheet and the saved list
for display, so it has a small mirror of those two parsers; the answer text is
always parsed by the wasm. The shared fixtures under
`tests/fixtures/text_formats/` are the contract between the two
implementations. If the mirror grows beyond a split on tabs, move it into the
wasm.

### Cell keys

Nominal: `nom gen par ill ine ela ade abl all ess tra abe com pro ins` ×
`_sg` / `_pl`. The nominal question pool is the catalogue's eight slots minus
the nominative: `gen_sg`, `par_sg`, `ill_sg`, `nom_pl`, `gen_pl`, `par_pl`,
`ill_pl`. `acc_sg` is excluded because it is ambiguous with the
nominative/genitive.

Verb: `pres_1sg`, `pres_3sg`, `past_3sg`, `cond_3sg`, `imp_2sg` — the five the
wizard asks — plus `pres_1pl` and the other persons in the sheet. A verb cell
requires the active voice and affirmative polarity, so a passive or the
analytic negative (`en puhu`) does not answer it.

The key-to-`Tags` table lives in the core (`Cell::tags()`), is tested against
`inflect` output, and is the only place a key becomes grammar.

## Inference

### Candidate space

- Enumerate the **parsed** `Inflection`s in `kotus::lemmas()`: roughly 150
  nominal readings and 120 verbal readings in the current lexicon. Do not split
  the raw class strings; `kotus::parse_inflections` already handles `(1)`,
  `8*(C)` and `9, 50`.
- Exclude 50/51 (compound markers, not paradigms) and 100/101 from the main set.
  Add 99 as the explicit "does not inflect" result.
- Keep the user's POS family: `grammar::is_verb_class` for verbs, the rest for
  nominals. Pass the user's `PartOfSpeech` to `inflect_in`, so adjectives and
  pronouns get the right reading.
- For each base class, add the gradation letters whose `Gradation::site(word)`
  finds a strong pair, plus `None`. The observed set supplies the prior; the
  synthesized variants cover a reading the lexicon does not happen to list for
  this exact combination.
- The per-expression lemma count is the prior, used only to break ties.

### Matching

For each candidate, build `inflect_in(word, c, cat, Some(&index), pos, false)`
once per `(word, pos)` session. The index is built once; `inflect` per
candidate would rebuild it every time.

For each answered cell, the predicted forms are the `Form.word`s that have a
reading **strictly** in that cell:

```rust
fn claims(tags: &Tags, cell: &Cell) -> bool {
    tags.case == Some(cell.case)
        && tags.number == Some(cell.number)
        // possessive / mood / tense / person only when the cell names them
}
```

Do **not** use `Tags::matches`: it treats a `None` dimension as a wildcard, so
a case-less form would "hit" every case. The binding already reports case-less
possessive forms; the wizard runs with possessives off, but the strict check is
the contract, not an accident.

Compare lowercased, NFC-normalized, trimmed strings. A user's answer may be any
alternant the class emits; the engine emits alternants as separate forms, so
set membership is enough. Never apply gradation or harmony to the user's
answer.

### Scoring

```
hits    = answered cells where the answer is in the predicted set
misses  = answered cells where the predicted set is non-empty and lacks it
unknown = answered cells where the class emits nothing
```

Rank by `hits` desc, then `misses` asc, then `recorded` (the sanalista's exact
expression) desc, then **`tail_prior` desc**, then the whole-lexicon `prior`
desc, then class number asc (stable output). `unknown` is not negative evidence;
the question pool is chosen so it does not happen.

The tail prior is the tie-break the request asked for: it counts lemmas in the
lexicon that end in the same three characters and carry the exact expression.
`mökki` and the invented `pöräkki` both end in `-kki`, where class `5*A` has 406
lemmas and the equivalent `5*D`/`5*L`/`5*M` variants have none, so the class
weighted towards the word's tail wins. It is a tie-break, not evidence: hits and
misses always come first, and a recorded expression still beats it.

Report the **base class** and the **full expression** separately: the user cares
about `5*A`, and the evaluation metric is usually the base class.

### Asking the next question

- Live set: candidates with the maximum `hits` and zero `misses`.
- For each unanswered cell in the pool, group the live candidates by their
  predicted form. `gain = live - largest group`. Pick the largest gain; tie-break
  by the canonical order (`gen_sg`, `par_sg`, `ill_sg`, `nom_pl`, `gen_pl`,
  `par_pl`, `ill_pl`).
- Do not ask a cell where every live candidate predicts the same form
  (`gain == 0`), or where more than half emit nothing.
- Stop when one candidate remains, the top has 3+ hits and no rival can tie on
  the remaining cells, 5 questions have been asked, or no cell has gain.
- If an answer matches no candidate, keep it (it penalizes every candidate
  equally), say so, and ask the next cell; do not clear the answer
  automatically.

Genitive singular is the first question: it reveals the stem and the weak grade,
and every class has it. Partitive and illative follow because they separate the
vowel and consonant patterns the genitive leaves tied.

### Ties, compounds, names, indeclinables

- **Tie.** If the top classes agree on hits, misses, the recorded flag, the
  tail prior and the whole-lexicon prior, and no remaining cell separates them,
  return both, best first, and say they are indistinguishable for this word.
  The class number and gradation order are not evidence. Saving a dual
  expression (`9, 10`) is allowed; the parser already reads it.
- **Compound.** Run `compound::analyze` before the questions. If it splits, show
  the parts and the head's class and say the class comes from the head; the user
  usually does not need a new entry for a regular compound. The questions still
  work if they continue.
- **Name.** The same flow with `erisnimi`; save the surface as typed, match
  case-insensitively, and capitalize the preview from the surface. The checker's
  suggestion path restores case later. A numeral/ordinal uses the same nominal
  cells with `numeraali`/`järjestysluku` as the reading.
- **Indeclinable.** A **taipumaton** checkbox in the pages and
  `--indeclinable` in the CLI return class `99` and skip the questions. The
  sheet has no 99 (it is not a catalogue class); the UI hard-codes the
  explanation. A saved 99 word accepts the lemma only: `wizard_user_forms`
  does not run class 99's loan machinery, so it cannot invent forms.

## Surfaces

### Core (`src/class_wizard.rs`)

```rust
pub const SHEET: &str = include_str!("../data/class_examples.tsv");
pub const CORE_KEYS: [&str; 8] = [...];
pub const QUESTION_POOL: [&str; 7] = [...];
pub const VERB_KEYS: [&str; 6] = [...];
pub const VERB_QUESTION_POOL: [&str; 5] = [...];

pub fn parse_sheet(text: &str) -> Result<Sheet, ParseError>;
pub fn parse_answers(text: &str) -> Result<Answers, ParseError>;
pub fn parse_user_words(text: &str) -> Result<UserWords, ParseError>;
pub fn format_user_words(words: &UserWords) -> String;
pub fn add_user_word(text: &str, word: &str, class: &str, pos: &str) -> Result<String, ParseError>;
pub fn remove_user_word(text: &str, word: &str) -> Result<String, ParseError>;
pub fn pos_from_name(name: &str) -> Option<PartOfSpeech>;
pub fn render_sheet(cat: &Catalogues) -> String;
pub fn check_sheet(cat: &Catalogues) -> Result<Vec<SheetMismatch>, ParseError>;
pub fn infer(lemma: &str, pos: PartOfSpeech, answers: &Answers,
             cat: &Catalogues) -> Result<Report, WizardError>;
pub fn infer_with(lemma: &str, pos: PartOfSpeech, answers: &Answers,
                  cat: &Catalogues, use_recorded: bool)
    -> Result<Report, WizardError>;
```

`Report` carries the ranked candidates, per-answer hit/miss/unknown, the next
question, the tie flag, and the recorded class when the lemma is in the
sanalista. `infer` uses one `LexiconIndex` and `inflect_in`, not `inflect` per
candidate. It lowercases the headword for the engine and keeps the surface for
display; a capitalized name therefore finds its lower-case paradigm.

### CLI (`tools/src/bin/word_wizard.rs`)

```
word_wizard [WORD] [--pos POS] [--form CELL=FORM ...] [--answers FILE]
            [--indeclinable] [--json] [--emit sanalista|custom]
            [--write-sheet PATH] [--check-sheet]
```

- No `--form` / `--answers`: an interactive text REPL, the same questions as the
  browser.
- `--answers`: batch mode for scripts and the build phase.
- `--indeclinable`: return class 99 and skip the questions; only the lemma is
  accepted for the saved entry.
- `--emit sanalista`: print the CSV row (`word, homonym, pos, class`) and the
  `add_sanalista_word.py` command with a placeholder anchor.
- `--emit custom`: print the user-words row.
- `--write-sheet`: regenerate `data/class_examples.tsv` from the embedded
  catalogue.
- `--check-sheet`: verify the committed sheet against `inflect`; the CI gate.
- `--json`: the report as JSON; the browser receives the same shape from the
  wasm.

### wasm (`torille-wasm/src/lib.rs`)

```rust
#[wasm_bindgen] pub fn wizard_sheet() -> String;
#[wasm_bindgen] pub fn wizard_infer(word: &str, pos: &str, answers_tsv: &str) -> js_sys::Object;
#[wasm_bindgen] pub fn wizard_paradigm(word: &str, class: &str, pos: &str) -> js_sys::Object;
#[wasm_bindgen] pub fn wizard_user_forms(user_words_tsv: &str) -> js_sys::Array;
#[wasm_bindgen] pub fn wizard_user_words(user_words_tsv: &str) -> js_sys::Object;
#[wasm_bindgen] pub fn wizard_user_words_add(text: &str, word: &str, class: &str, pos: &str) -> js_sys::Object;
#[wasm_bindgen] pub fn wizard_user_words_remove(text: &str, word: &str) -> js_sys::Object;
```

The sheet is embedded (`SHEET`), so the page does not fetch a file.
`wizard_infer` parses the answer text in Rust and returns the report object;
`wizard_paradigm` returns the eight-cell preview; `wizard_user_forms` returns
the lowercased surfaces for the extension's custom-word set; the three
`wizard_user_words*` functions are the one parser/formatter the demo and the
options page both call, so the saved text is never parsed in JS.

### Demo page (Phase 3)

- `torille-wasm/demo/wizard.html` plus `src/wizard.ts` (bundled to
  `dist/wizard.js`), linked from `index.html`.
- Uses the existing wasm loader. The sheet is read once via `wizard_sheet()` and
  parsed into a `Map`.
- Two tabs: **Find a class** (questions, candidates, paradigm) and **Browse
  examples** (the sheet, sorted by `lemmas`, searchable by class or model).
- Storage: `localStorage["torille-user-words"]` and
  `localStorage["torille-wizard-session"]`; import/export the text.
- No build-step change beyond the new esbuild entry.

### Extension (Phase 4)

- A separate **`options/wizard.html`** page, linked from the options page. It has
  the same find/browse tabs and saved list; it does not load the wasm.
- It messages the background:
  - `torille:wizard-sheet` → the sheet text from the worker;
  - `torille:wizard-infer` → the report;
  - `torille:wizard-paradigm` → the preview;
  - `torille:wizard-user-words` / `-add` / `-remove` → the parsed list and the
    edited text.
- Background: read `browser.storage.local["torille-user-words"]` on the first
  check and on change; send the whole text to each spell worker
  (`set-user-words`). The text is small; no partial updates.
- Worker (`spell-core.ts`):
  - new inbound `set-user-words` (text), the `wizard-*` calls;
  - on `set-user-words`, call `wizard_user_forms(text)` and build a
    `Set<string>` of lowercased surfaces (the lemma included);
  - `handleCheck` / `handleCheckFast` / `handleCheckBatch` consult `userForms`
    before the wasm; `analyze-text` filters custom words out of the hit list;
  - suggestions are unchanged in v1: saved words are accepted, not offered. Say
    so in the UI.
- `storage.onChanged` for the key re-broadcasts, so a save in one tab or worker
  is live everywhere.
- Import validates the file through `wizard-user-words` and then replaces the
  stored text; export uses the same text file.

## Storage

| surface | key | value |
|---|---|---|
| demo | `torille-user-words` | the TSV blob |
| demo | `torille-wizard-session` | the in-progress answers TSV |
| extension | `torille-user-words` | the same TSV blob |
| extension | `torille-wizard-session` (optional) | the same |

- Text, not JSON, and not IndexedDB: the list is a few KB, `localStorage` and
  `browser.storage.local` are enough for a settings page, and the text *is* the
  export format.
- Firefox's `storage.local` quota is measured in a few MB; the cap above is far
  below it.
- Read-modify-write on every save (re-read, update the map, format) avoids
  clobbering a change from another tab.
- Formatting sorts by word (Unicode order) and always writes the version
  header, so exports diff cleanly.
- The exported file is `torille-user-words.txt`, UTF-8, with the same header.

## Validation

- **Sheet staleness** (`src/class_wizard.rs`'s `the_sheet_is_current` and
  `--check-sheet`): re-render the sheet from the engine and compare the model
  and the eight forms, skipping the informational lemma count. A class-engine
  change turns this red until the sheet is regenerated, exactly like the derived
  blobs. The catalogue's own correctness stays `tests/catalogue.rs`'s gate.
- **Parsers** (Rust and JS, same fixtures under
  `tests/fixtures/text_formats/`): comments, blanks, CRLF, duplicate keys, wrong
  column count, empty values, Unicode, version refusal, caps.
- **Recovery** (unit/integration): sample sanalista lemmas, generate answers
  from the recorded class, hide the class, infer; top-3 must contain the
  recorded base class. This is a scoring regression test, not an accuracy claim
  (the probe and the hypothesis use the same engine).
- **Oracle curve** (`tools/wizard_eval.py`, GPL, dev-only): read `ok='both'`
  cells from `data/kotus_nomines.sqlite`, sample nouns, and report
  top-1/top-3 base class and exact expression at 1–5 answers, adaptive versus
  the fixed canonical order. The binding method `Engine.class_candidates`
  passes `use_recorded=False`, because the sample words are in the sanalista
  and ranking the recorded class first would make the measurement circular.
  The 500-sample run (412 nouns, seed 1) is: adaptive top-1 base 90.3% at one
  answer, 94.2% at two, 99.8% top-3; fixed 90.3% until 94.7% at five. The
  adaptive picker therefore pays off from the second answer. A `--limit 100`
  run is under a minute; `--limit 500` is about eight minutes.
- **Question picker**: the adaptive order must be at least as good as the fixed
  order at 3 answers. If it is not, ship the fixed order and record why.
- **Browser/worker**: `torille-wasm/demo/wizard-smoke.mjs` drives the demo page
  through the five questions and a save/reload; `extensions/firefox/wizard-page-smoke.mjs`
  does the same for the options page with a stubbed runtime and the real worker
  core; `extensions/firefox/wizard-core-smoke.mjs` proves `set-user-words` accepts
  a generated form, `analyze-text` filters it out and clearing the list rejects
  it; `extensions/firefox/smoke.mjs` covers the wasm functions. `npm test` keeps
  the source-sync copies identical; run `npm run build` first.
- **No engine regression**: the full gates from `AGENTS.md` (`clippy`,
  `cargo test --release --workspace`, the binding and harness gates,
  `test_illegal_evidence.py`). The wizard changes no generated form, so the
  harness baseline must not move.

## Performance

- A quick measurement inflected all 1,092 class × gradation expressions in
  about 1 s through the Python binding. The Rust core over the ~270 parsed
  readings, pruned by POS and gradation site, should be an order of magnitude
  cheaper.
  The budget is **<100 ms for the first answer in the worker and <20 ms for each
  later answer**, measured with a new bench.
- The index is built once per session; `inflect_in` takes it as an argument. If
  the measurement still misses the budget, cache the generated paradigms per
  `(word, pos)` in the session and only re-score on each answer.
- The sheet parse is one-time. The worker's custom-form set is rebuilt only when
  the storage text changes.

## Phases

| phase | deliverable | exit criterion |
|---|---|---|
| 0 | format doc and fixtures | the three TSVs, cell keys, version headers and parser rules are agreed and pinned by fixtures |
| 1 | sheet and parsers in `src/class_wizard.rs`; `--write-sheet` / `--check-sheet` | the sheet is generated, committed and staleness-checked; parser tests green |
| 2 | inference and CLI; `tools/wizard_eval.py`; binding method | batch and interactive CLI work; the oracle curve and the adaptive-versus-fixed comparison are recorded — **done** |
| 3 | wasm exports, demo page, `localStorage`, import/export | a user can find and save a word in the demo; the page survives reload; a Puppeteer smoke test passes — **done** |
| 4 | extension options wizard, `browser.storage.local`, worker custom-word set | a saved word is accepted by the checker on the next text; a deleted word is not; source-sync is green — **done for Firefox and Chromium** |
| 5 | custom-word suggestions (**done**), verb sheet and cells (**done**), session restore (**done**), name handling and the indeclinable switch (**done**) | measured, not promised |

Phases 0–5 and the oracle curve are implemented (2026-11-18): the demo, both
extensions, nominal and verb inference, the tail prior, saved-word acceptance
and suggestions, and the measured adaptive-versus-fixed accuracy.

## Risks and open questions

- **Engine defects are inherited.** If a class engine emits a wrong form, the
  wizard can prefer the wrong class. The oracle curve separates "class
  ambiguity" from "engine disagreement"; the latter is a harness bug, not a
  wizard bug.
- **Gradation letters can be equivalent for a word.** `mökki`'s genitive,
  partitive, illative, nominative plural and genitive plural are reproduced by
  `5*A`, `5*D`, `5*L`, `5*M` and `10*A`; the engine makes several letters
  no-ops for this stem, and class 10's singular coincides. The tail prior
  prefers `5*A` (`-kki` words are overwhelmingly `5*A`), and a recorded
  expression ranks first when the word is known. If even the tail prior ties,
  the UI says the classes generate the same forms rather than pretending one is
  certain.
- **The eight-slot pool may tie.** The sheet has only the catalogue's eight
  slots. If the oracle curve shows ties that a ninth or tenth case would break,
  extend the sheet with engine-generated essive/translative rows; the format
  already allows it.
- **Verbs are a second grid.** The verb cells are finite forms, not cases; the
  sheet and question pool switch on `pos`. Classes 77/78 are defective sound
  verbs with only the third-person present and conditional, so a user who
  answers a first-person form of one gets fewer hits for that class. The sheet
  and the UI show only the cells the engine emits.
- **Storage is per browser.** `localStorage` and `browser.storage.local` do not
  sync across machines; export/import is the bridge. Do not add a sync backend
  in this plan.
- **Options page versus worker.** The options page must not load the wasm; all
  engine calls go through the background/worker. If the worker is not ready, the
  wizard shows a short "starting" state and retries.
- **The JS mirrors can drift.** The fixtures are the contract; keep the JS
  parsers to a split on tabs. If that stops being true, move parsing into the
  wasm and return an object.

### Remaining work

1. **Window dressing.** A result is saved with the surface as typed; a custom
   word's capitalization in suggestions follows the existing suggestion-case
   path. Nothing in the plan is blocked.
