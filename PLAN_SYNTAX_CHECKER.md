# PLAN_SYNTAX_CHECKER — grammar and punctuation diagnostics

Status: Phases 0–4 landed and Phase 5 started (negative-clause object case);
nested lists are still open. Phase 6 is planned. The analysis layer it builds
on is `PLAN_SYNTAX.md` and is already landed on the `syntax` branch.

The goal is a **correction checker**, not another parser: detect a small set of
high-confidence grammatical errors, explain them, and produce corrections from
the existing morphology engine. It sits on top of `syntax::analyze`; it does
not replace it and it does not run in the keystroke path.

Finnish examples are kept in Finnish. The rules and messages live in the engine;
UI copy is the caller's concern.

## 1. Scope

Priority order:

| priority | rule | first release |
|---|---|---|
| 1 | subject–predicate agreement | yes |
| 2 | attribute–noun agreement | yes |
| 3 | verb rection | phase 4 |
| 4 | case selection | phase 4 |
| 5 | object case | phase 5, advisory |
| 6 | missing/extra arguments | phase 5, warning |
| 7 | word order | deferred |
| 8 | list structure and list separators | phase 3 |
| 9 | comma/clause punctuation | phase 3 |

In scope, in this order:

1. **Contextual inflection errors** — subject–predicate agreement,
   attribute–noun agreement, and the finite auxiliary in compound/negative
   clauses.
2. **Verb rection and case selection** — a curated valency table, only for
   verbs where the required case is a lexical fact (`pitää` + elative,
   `mennä` + illative, ...).
3. **Object case** — partitive vs. total object, only where a clear cue exists
   (negation, existential, explicit quantity).
4. **Punctuation driven by clause and list structure** — list separators,
   colon-introduced lists, subordinate-clause commas, relative-clause comma
   pairs, coordinated clauses, parentheticals.
5. **Missing/extra arguments** — only for a small table of obligatory or
   strictly intransitive frames, and only as warnings.
6. **Word order** — deferred; Finnish word order is flexible and most
   "wrong order" examples are style, not grammar.

Out of scope for this plan:

- a full dependency parser or a statistical model;
- style advice (repetition, clumsy phrasing, register);
- rewriting sentences for clarity;
- spell checking and word-level suggestions (already in `suggest`);
- punctuation beyond commas, colons and semicolons in the first release
  (quotes and dialogue marks are a later punctuation-engine phase).

The style layer, if built later, is a **separate** analyzer. It must not share
diagnostic kinds with grammar errors, and it must be opt-in in the UI.

## 2. What the analysis layer already gives us

`torille::syntax` returns:

- `sentences: Vec<SentenceSpan>` — byte spans;
- `clauses: Vec<Clause>` — `predicate`, `subject`, `object`, `complement`,
  `negative`, `existential`, and the token range;
- `tokens: Vec<TokenSyntax>` — role, confidence, clause, head, case, lemma,
  and every morphological reading;
- `RoleConfidence::{Likely, Weak}` — no `Certain`; a role is deliberately
  weak when a word has competing readings.

That is enough for the first rules:

| rule | uses |
|---|---|
| subject–predicate agreement | `Clause.subject`, `Clause.predicate`, `Auxiliary` head, readings |
| attribute agreement | `Modifier` + `head`, head's case/number |
| verb rection | `Predicate` lemma + `Object`/`Adverbial` case |
| comma before a subordinate clause | clause spans + first token (`että`, `koska`, ...) |
| comma pair around a relative clause | relative clause start + host predicate |
| list separators | a list structure built from item spans and commas |

**Lists are a planned extension to the analysis layer.** The current
`SyntaxAnalysis` has no phrase parser, so it cannot tell a list from a clause
coordination. The checker plan adds `lists: Vec<List>` to `SyntaxAnalysis`,
built by a focused phrase-level pass (`syntax_list`), and uses it for list
punctuation. It is not a full phrase-structure parser, but it is a real
syntactic structure, not a comma heuristic.

The missing pieces are **clause kinds and relative-clause extent**. The current
clause ranges are linear and a relative clause inside its host splits the host
(`Mies, joka tuli, söi` becomes three ranges). Comma rules need a small amount
of structure on top; see phase 3. That is a targeted extension, not a full
parser.

## 3. Design principles

1. **Grammar errors first, style second.** The first release is the four
   contextual inflection corrections the user asked for. Style is a separate
   analyzer.
2. **A correction is a set of edits, not a string.** A diagnostic may need one
   replacement (`nukkuvat` → `nukkuu`), two commas around a relative clause, or
   a deletion. The API carries byte spans and exact replacements.
3. **Never guess at a role.** An `Error` requires `RoleConfidence::Likely` for
   the relevant roles and a single relevant reading. `Weak` roles produce at
   most a `Suggestion`; `Unknown` roles suppress the rule.
4. **A correct sentence must stay quiet.** Every rule ships with a clean
   corpus that asserts no `Error` diagnostics. False negatives are cheaper
   than false positives.
5. **Use morphology for corrections.** `expand::forms_of` with a `Tags`
   filter is the inflection engine; do not write a second one.
6. **Only plain forms are offered.** A `Repaired`/`Guessed` form may be
   accepted as a word but must never be a correction. This is the same rule
   the suggestion path follows.
7. **One diagnostic per conflicting span.** Rule output is deduplicated and
   ranked before it reaches the UI.
8. **Data over ad-hoc word lists.** Rection frames live in a CSV table built
   into the maps; subordinator/relative/coordinator sets are shared with
   `syntax.rs`, not copied.

## 4. Architecture

```text
text
 │
 ├─ tokenise ────────────────► tokens (byte spans, separators)
 │
 ├─ syntax::analyze ─────────► SyntaxAnalysis (clauses, roles, readings)
 │      └─ syntax_list::parse ► lists (items, separators, conjunction)
 │
 ├─ syntax_check::check ─────► rule engine
 │      ├─ agreement
 │      ├─ rection / case
 │      ├─ arguments
 │      ├─ lists
 │      └─ punctuation
 │
 ├─ correction::generate ────► candidates from forms_of
 │
 └─ rank::resolve ───────────► SyntaxCheck { analysis, diagnostics }
```

Module layout (new sibling of `src/syntax.rs`):

```text
src/syntax_list.rs  List, ListItem, ListKind, parse
src/syntax_check/
  mod.rs          public API, orchestration, conflict resolution
  diagnostic.rs   DiagnosticKind, Severity, Diagnostic, Edit, Correction
  agreement.rs    subject–predicate and attribute–noun
  rection.rs      valency frames and case selection
  arguments.rs    missing/extra arguments (advisory)
  lists.rs        list punctuation and list/clause discrimination
  punctuation.rs  clause-boundary comma rules and clause-kind derivation
  correction.rs   build corrections with forms_of
  rank.rs         scoring, dedup, conflict resolution
```

`SyntaxAnalysis` gains `pub lists: Vec<List>`; the wasm analysis object
serialises it alongside clauses and tokens.

`src/lib.rs` gets `pub mod syntax_check;`.

The shared word-class predicates in `src/syntax.rs` (`is_subordinator`,
`is_coordinating`, `is_relative_word`) become `pub(crate)` so the punctuation
module uses the same lists. Do not create a second subordinator list.

### Clause kinds

Add a local enum in `syntax_check` first, then promote it to `syntax.rs` if a
second consumer appears:

```rust
pub enum ClauseKind {
    Main,
    Subordinate, // first word is että, koska, kun, jos, vaikka, ...
    Relative,    // first word is joka/mikä/kuka in any case
    Infinitive,  // first word is an infinitive
    Coordinate,  // starts with ja, mutta, tai, ...
}
```

Derive the kind from the clause's first word and its `TokenSyntax` lemma. A
relative clause that is interrupted by the host predicate needs its extent
computed separately; see phase 3.

## 5. Data model

Keep detection and correction separate, as the user proposed.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,      // standard written Finnish is violated
    Warning,    // likely wrong, or a construction with variation
    Suggestion, // optional style/punctuation improvement
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    SubjectPredicateAgreement,
    AttributeAgreement,
    VerbRection,
    CaseSelection,
    ObjectCase,
    MissingArgument,
    ExtraArgument,
    ClauseStructure,
    WordOrder,
    CommaMissing,
    CommaExtra,
    CommaPairUnbalanced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Byte span in the source text. `start == end` is an insertion.
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Correction {
    /// Applying every edit produces the corrected sentence.
    pub edits: Vec<Edit>,
    /// Lower is better. Populated by `rank`.
    pub score: f32,
    /// Stable explanation key, not a localized sentence.
    pub note: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxDiagnostic {
    pub kind: DiagnosticKind,
    pub severity: Severity,
    pub confidence: DiagnosticConfidence,
    /// The smallest span the UI should underline.
    pub span: Range<usize>,
    /// Stable message key; the caller localizes it.
    pub message: &'static str,
    /// What the rule expected, for diagnostics/debugging.
    pub expected: Option<String>,
    pub corrections: Vec<Correction>,
}

pub struct SyntaxCheck {
    pub analysis: SyntaxAnalysis,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}
```

### List structures

Lists are a first-class part of the analysis, not a clause substitute. The
arena keeps nesting cheap and serialisable:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    /// Byte span of the whole list, introducer excluded.
    pub span: Range<usize>,
    /// Token range, introducer excluded.
    pub tokens: Range<usize>,
    /// Clause the list belongs to (index into `analysis.clauses`; for a
    /// clause-coordination list, its first clause).
    pub clause: usize,
    /// The list's grammatical function, when the surrounding clause makes
    /// it clear (`Object`, `Adverbial`, ...).
    pub role: Option<SyntacticRole>,
    /// Governing token: the predicate or the head the list belongs to.
    pub head: Option<usize>,
    pub kind: ListKind,
    pub items: Vec<ListItem>,
    /// Comma tokens that separate top-level items.
    pub separators: Vec<usize>,
    /// Final `ja`/`sekä`/`tai`/`vai`, when present.
    pub conjunction: Option<usize>,
    /// Colon that introduces the list (`Ostin kolme asiaa: ...`).
    pub introducer: Option<usize>,
    /// Parent list when this list is nested inside an item.
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub tokens: Range<usize>,
    pub span: Range<usize>,
    pub kind: ListItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Nominal,   // omenoita, appelsiineja ja banaaneja
    Clause,    // Ostin omenoita, Liisa osti banaaneja
    Predicate, // Sami söi, joi ja nukkui (shared subject)
    Mixed,     // do not apply list punctuation automatically
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListItemKind {
    Word,
    Phrase,
    Clause,
    /// Nested list, index into `analysis.lists`.
    List(usize),
}
```

The user-proposed `LIST`, `LIST_ITEM`, `LIST_SEPARATOR` and
`LIST_CONJUNCTION` nodes are represented by `List`, `ListItem`, the
`separators` vector and the `conjunction` field. There is no separate
`ListSeparator` node because the separator is already a token; its index is
enough for punctuation edits.

Public entry points:

```rust
pub fn check(text: &str) -> SyntaxCheck;
pub fn check_with_cache(text: &str, cache: &mut syntax::AnalysisCache) -> SyntaxCheck;
pub fn check_analysis(tokens: &[Token], analysis: &SyntaxAnalysis) -> Vec<SyntaxDiagnostic>;
```

`check` tokenises once, runs `syntax::analyze_with_cache`, then
`check_analysis`; `check_analysis` is the testable core. `check_with_cache`
lets the wasm textarea reuse its `AnalysisCache`.

## 6. Rule catalogue

### 6.1 Subject–predicate agreement (priority 1)

Detect when the finite agreement bearer and the subject disagree in person or
number.

**Agreement bearer.** Find the finite token, not necessarily
`Clause.predicate`:

- a plain finite verb: `Clause.predicate`;
- a compound tense: the `Auxiliary` whose `head` is the predicate
  (`olen lukenut`, `on rakennettu` when active agreement applies);
- a negative clause: the `NegativeVerb` auxiliary (`en`, `ei`, ...), because
  the connegative main verb has no person;
- passive: no subject, so no rule.

**Subject number/person.**

- `Clause.subject` usually points at the head. Collect coordinated subjects in
  the clause (`role == Subject`, joined by `ja`/`sekä`) and use plural.
- A numeral head (`kaksi miestä`) uses the numeral's number, which is singular;
  verbs with numeral subjects are singular.
- Pronouns are read from their `Tags` (`minä` 1sg, `me` 1pl, ...).
- If the subject has several readings and they disagree, skip (or `Suggestion`
  at most).

**Suppress the rule for constructions where agreement is neutralized:**

| construction | example | why |
|---|---|---|
| existential | `Pöydällä on kirjat.` | plural subject, singular `on` is standard |
| passive | `Talo on rakennettu.` | no agreeing subject |
| necessive | `Minun täytyy mennä.` | genitive subject, 3sg fixed |
| infinitive clause | `Haluan syödä.` | no finite subject agreement |
| impersonal/weather | `Sataa.` | no subject |
| numeral phrase | `Kaksi miestä tuli.` | singular verb with numeral head |
| collective noun | `Kansa tulee.` | singular head |
| parenthetical reporting | `Hän sanoi, että...` | clause-local; do not cross |

**Correction.** Use `forms_of` with the verb lemma and the sanalista's
inflections, preserving tense, mood, voice and polarity, and changing only
person/number. Plain forms only.

Examples:

| wrong | diagnostic | correction |
|---|---|---|
| `Koira nukkuvat.` | 3pl verb, 3sg subject | `Koira nukkuu.` |
| `Minä menee kotiin.` | 3sg verb, 1sg subject | `Minä menen kotiin.` |
| `Hän antoivat kirjan.` | 3pl verb, 3sg subject | `Hän antoi kirjan.` |
| `Lapset leikkii.` | 3sg verb, 3pl subject | `Lapset leikkivät.` |
| `Me menee.` | 3sg verb, 1pl subject | `Me menemme.` |
| `Minä on lukenut.` | auxiliary 3sg, subject 1sg | `Minä olen lukenut.` |
| `Hän en tule.` | negative auxiliary 1sg, subject 3sg | `Hän ei tule.` |

Do **not** flag `Pöydällä on kirjat`, `Talo on rakennettu`,
`Minun täytyy mennä`, `Kaksi miestä tuli`, `Sami ja Liisa söivät` (plural verb),
`Sami söi puuron` (singular subject).

### 6.2 Attribute–noun agreement (priority 2)

An attributive adjective/ordinal/numeral/demonstrative agrees with its head in
case and number.

```text
Punainen autot.       Punaiset autot.
Punaiset auto.        Punainen auto.
```

- Only `Modifier` tokens whose `head` is a nominal in the same clause.
- Only POS `Adjective`, `Ordinal`, or a demonstrative pronoun. **Cardinal
  numerals are not agreement attributes**: `kaksi miestä` is a numeral +
  partitive government, not a mismatch.
- Skip genitive modifiers (`Matin auto`), titles, appositions, and
  lexicalized adjectives that are part of a name (`Sami Ruotsalainen`).
- `punaisen auto` is ambiguous: it can be the genitive of a nominalized
  adjective ("the red one's car"). Do not flag it; the rule only fires when
  the modifier reading is unambiguously attributive.
- Use the head's selected case/number; if the head is ambiguous, skip.
- Correction with `forms_of(adj, inflections, case, number)`; preserve the
  adjective's degree.

**Known gap:** `Tags` has no `Degree` dimension. In v1 only positive-degree
attributes are corrected; a comparative/superlative is detected from the
surface/degree tables and skipped rather than risking `punaisin auto` →
`punainen auto`. Adding a degree dimension is a separate change.

### 6.3 Verb rection and case selection (priority 3)

A small valency table supplies the case a verb governs.

```csv
lemma,case,frame,priority,note
pitää,elative,like,1,"pitää jostakin"
pitää,partitive,hold,2,"pitää jotakin"
mennä,illative,go-to,1,"mennä johonkin"
mennä,allative,go-to-event,2,"mennä jollekin"
luottaa,illative,trust,1,"luottaa johonkin"
... 
```

Rules:

- Fire only when the predicate lemma is in the table.
- The candidate is an `Object` or a case-bearing `Adverbial` that the verb
  governs, not a subject or a free adverbial.
- If the candidate's case matches **any** frame, no error. A verb can have
  several frames (`pitää` like/hold); never call a valid frame wrong.
- If it matches none, emit `VerbRection`, with one correction per frame case,
  ranked by frame priority then by edit distance.
- Do not fire when the argument is unknown, a proper name without a case, or
  the clause is passive (the logical object may be in a different case).

Examples:

| wrong | expected corrections |
|---|---|
| `Pidän sinä.` | `Pidän sinusta.` (elative, first), `Pidän sinua.` (partitive) |
| `Menen koulu.` | `Menen kouluun.` (illative, first), `Menen koululle.` |
| `Luotan sinä.` | `Luotan sinuun.` |
| `Kuuntelen sinä.` | `Kuuntelen sinua.` |

The table starts with 30–50 high-frequency verbs and grows only when a
verified frame is added. It is not a general semantic valency model.

### 6.4 Object case (priority 5, advisory)

Total vs. partitive object is aspect/negation/quantity-sensitive:

- negative clause: partitive (`En lue kirjaa`);
- existential/quantity: partitive;
- completed/result: total (genitive/accusative/nominative);
- ongoing/indefinite: partitive.

There is often no local cue. In v1:

- flag only a clear negative clause where the object is not partitive
  (`En lue kirja` → `kirjaa`);
- otherwise emit a `Suggestion` with both partitive and total corrections, or
  no diagnostic if the cue is absent;
- never emit `Error` for `Luen kirja` / `Luen kirjaa` from aspect alone.

### 6.5 Missing/extra arguments (priority 6, warning only)

Use the valency table, not a general parser:

- missing obligatory argument only for frames explicitly marked
  `obligatory` (e.g. `luottaa` needs an illative argument);
- extra argument only for a small intransitive table (`nukkua`, `sataa`, ...);
- no diagnostic for a dropped subject (Finnish permits it);
- no diagnostic for an omitted recipient when the verb is optionally
  transitive.

### 6.6 Word order (priority 7, deferred)

Finnish word order is information structure, not a fixed SVO grammar. Do not
flag `Söi Sami puuron` or `Puuron söi Sami` as errors. Word order belongs to the
style layer or to very specific constructions (question word order) and is not
part of the first correction release.

### 6.7 Punctuation (priority 8, but high value)

Punctuation is a **separate rule library** fed by syntax, not code inside the
parser.

**Clause kinds:** main, subordinate, relative, infinitive, coordinate (see
§4). The first two rules:

#### Subordinate-clause comma

A subordinate clause introduced by `että`, `koska`, `kun`, `jos`, `vaikka`,
`jotta`, `mikäli`, `ellei` is separated from the main clause by a comma, in
both orders:

```text
Menin kotiin koska olin väsynyt.     → Menin kotiin, koska olin väsynyt.
Koska olin väsynyt menin kotiin.     → Koska olin väsynyt, menin kotiin.
```

Implementation detail: insert the comma at the **end of the previous word**
(not before the following space), so `kotiin, koska` is produced. If the
subordinate clause is first, insert after its last word.

#### Relative-clause comma

A relative clause introduced by `joka`/`mikä` can be restrictive (no comma) or
non-restrictive (comma pair). Do **not** add a comma to a restrictive relative
by default:

```text
Koira joka nukkuu sohvalla on minun.      # restrictive: no error
Koira, joka nukkuu sohvalla, on minun.    # non-restrictive: both commas
Tapasin koiran, joka asuu naapurissa.     # final relative: opening comma only
```

Rules:

- If an opening comma is present before the relative clause, require the
  closing comma when the host predicate follows; `CommaPairUnbalanced`.
- If there is no opening comma, emit at most a `Suggestion` to add the pair
  (style), never an `Error`.
- A final relative with an opening comma needs no closing comma.
- A relative clause at the very end of a sentence needs only its opening comma.

#### Coordinated clauses

Comma before `ja`/`mutta`/`tai` is optional in Finnish and depends on whether
the clauses have separate subjects and predicates. Do not make it an `Error`:

- `Menin kauppaan ja ostin maitoa.` — shared subject/predicate structure, no
  comma;
- `Menin kauppaan, ostin maitoa ja tulin kotiin.` — a series of main clauses;
  commas are acceptable, not obligatory;
- `Sami söi puuron, ja Liisa joi maidon.` — separate subjects; comma is a
  suggestion, not an error.

#### Parentheticals and appositions

Comma pairs around clear parentheticals (`esimerkiksi`, `kuten`, appositions)
are advisory. Add a small exception list; do not infer parentheticals from
every adverb.

#### Comma edits

- **Insertion:** `Edit { start: previous_word_end, end: previous_word_end,
  replacement: "," }`.
- **Removal:** cover only the comma character, not the following space.
- A pair is one diagnostic with two edits, so a single correction inserts or
  removes both.
- Do not touch commas inside quotes, code, or fixed phrases; the tokeniser
  already gives separator spans, so those checks are cheap.

Future punctuation engine: colon, semicolon, question marks, quotation marks,
dialogue dashes. Keep the `Punctuation` module independent of the grammar
rules; it consumes `ClauseKind` and token spans.

### 6.8 Lists and list punctuation

A list is a **syntactic structure**, not a sequence of clauses. The parser
must recognise it before the punctuation checker decides whether a comma is
required, optional or wrong.

#### Recognition

1. Take the maximal span inside one clause/sentence that ends at a clause
   boundary, period, semicolon, closing parenthesis or quote.
2. Split on **top-level** commas only. Commas inside parentheses, quotes,
   relative clauses or nested lists are not separators.
3. Attach a final conjunction (`ja`, `sekä`, `tai`, `vai`) to the last item;
   it does not need a preceding comma in Finnish.
4. Detect an introducer: a colon followed by the list
   (`Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.`).
5. Classify every item:
   - all nominal phrases → `Nominal`;
   - all independent clauses (finite predicate and their own/recoverable
     subject) → `Clause`;
   - all verb phrases sharing a subject → `Predicate`;
   - anything else → `Mixed`.
6. If `Mixed`, keep the structure for analysis but do **not** apply list
   punctuation rules automatically.
7. Parse nested lists recursively inside an item and store the child list
   index in `ListItemKind::List`.

A list must have at least two items and a real cue (a top-level comma, a
final conjunction, or a colon introducer). A single comma is not a list.

#### List vs. clause coordination

| sentence | classification | punctuation rule |
|---|---|---|
| `Ostin omenoita, appelsiineja ja banaaneja.` | `Nominal` | list separators |
| `Ostin omenoita, Liisa osti banaaneja ja Matti osti päärynöitä.` | `Clause` | clause punctuation, not list separators |
| `Sami söi, joi ja nukkui.` | `Predicate` | commas optional; no missing-separator error |
| `Ostin omenoita, appelsiineja ja Liisa osti banaaneja.` | `Mixed` | no automatic correction |

A two-predicate coordination is not a list: `Sami söi ja joi.` has one
subject and two predicates, and needs no comma. `Predicate` lists require
three parallel predicates or a clear comma cue before the last conjunction.

The same structures in the Finnish examples above:

```text
Ostin omenoita, appelsiineja, banaaneja ja päärynöitä.          Nominal
Ostin omenoita, Liisa osti banaaneja ja Matti osti päärynöitä.  Clause
```

Do not classify by punctuation alone. Require item parallelism and a
grammatical function for the whole span. This is what keeps the checker from
treating `Kuitenkin, ostin omenoita, jotka olivat kalliita, eilen.` as a list:
the commas mark an introductory expression and a relative clause, and the
items are not parallel. Both commas are clause/relative boundaries, not list
separators.

#### Finnish-only list punctuation policy

`ListPunctuationPolicy` is fixed to Finnish. No other language policy exists
in this project:

| rule | Finnish policy |
|---|---|
| comma between list items | required (`Error` when missing) |
| duplicate comma | `Error` |
| comma before final `ja`/`sekä` | not preferred; `Suggestion` at most, never `Error` |
| comma before a two-item conjunction | `Suggestion`/no error |
| comma after a colon introducer | `Error` |

The standard Finnish list is `omena, appelsiini ja banaani`, not
`omena, appelsiini, ja banaani`. A comma before the final conjunction is a
stylistic choice, not a grammar error, so it stays a suggestion. Do not add
examples or policies for other languages.

#### List diagnostics

Add these kinds:

```rust
ListSeparatorMissing,
ListSeparatorDuplicate,
ListCommaBeforeConjunction,
ListIntroducerComma,
```

Rules:

- **Missing separator:** two parallel nominal items are adjacent without a
  comma and without being a lexical compound. The correction is an insertion
  at the end of the preceding item. High confidence only when the item boundary
  is unambiguous.
- **Duplicate comma:** two comma separators in a row (`omena,, appelsiini`).
  Remove one comma only, not the following space.
- **Comma before final conjunction:** Finnish `Suggestion` (remove or keep),
  never `Error`. No other language policy makes it mandatory or valid here.
- **Colon introducer:** a colon introduces a list; a comma immediately after
  the colon is an error (`ListIntroducerComma`). Do not treat the colon as a
  list separator.
- **Nested list:** edits are applied to the child list; the parent does not
  duplicate them. One diagnostic per child list.

Examples:

```text
Ostin omenoita, appelsiineja, banaaneja ja päärynöitä.   # valid Finnish
Ostin omenoita appelsiineja, banaaneja ja päärynöitä.    # missing separator
Ostin omenoita, appelsiineja,, ja banaaneja.             # duplicate comma
Ostin omenoita, appelsiineja, ja banaaneja.              # suggestion only
Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.  # introduced list
Ostin kolme asiaa:, omenoita, appelsiineja ja banaaneja. # introducer comma
```

## 7. Correction generation

`src/syntax_check/correction.rs` turns a rule finding into candidates:

```rust
fn correct_verb_agreement(lemma, inflections, cat, person, number, original)
    -> Vec<Correction>;

fn correct_attribute(lemma, inflections, cat, case, number, original)
    -> Vec<Correction>;

fn correct_case(lemma, inflections, cat, case, original)
    -> Vec<Correction>;
```

All use `expand::forms_of` with a `Tags` filter:

- subject–verb: preserve `tense`, `mood`, `voice`, `polarity`; set `person`,
  `number`;
- attribute: preserve degree if available (v1: positive only); set `case`,
  `number`;
- rection: set `case`, preserve `number`/`possessive`;
- object case: partitive or total variants, both offered when the cue is not
  decisive.

Filter `Confidence::Plain`. If there is no plain form, emit the diagnostic with
an empty correction list rather than inventing a word. For the negative
auxiliary use the closed negation table (`en`, `et`, `ei`, `emme`, `ette`,
`eivät`), not `inflect_all` unless that path is verified.

`Tags::matches` treats a missing dimension as open (a form whose number was
never settled matches any number request). After `forms_of`, re-check the
candidate's readings for the exact person/number/case that the rule wants
before offering it. The candidate's `Confidence` must also be `Plain`.

List and punctuation corrections are **edit-only**: no `forms_of`. Insertion is
a zero-length `Edit` at the end of the preceding item; removal covers only the
comma character. A comma pair is one diagnostic with two edits, so one
correction applies both.

## 8. Ranking and conflict resolution

This is the layer that keeps the checker from sounding confident about the
wrong thing.

1. **Rule confidence:** `High` only when the construction is unambiguous and
   the roles are `Likely`; `Medium` for coordinated/colloquial; `Low` for
   suggestions.
2. **Edit cost:** fewer edits first; use `suggest::levenshtein_with_ops` on
   the changed token(s), not on the whole sentence.
3. **Meaning preservation:** prefer an inflection-only correction over a
   different lemma; prefer the same tense/mood/voice.
4. **Frame priority:** rection candidates are ordered by the CSV priority, so
   `Pidän sinä` offers `sinusta` before `sinua`.
5. **List confidence:** only a `Nominal`/`Clause`/`Predicate` list with clear
   item boundaries can produce a separator `Error`; `Mixed` lists produce no
   correction, and a comma before the final conjunction is a `Suggestion`.
6. **Conflict:** one diagnostic per token/span. A punctuation insertion and a
   token replacement at the same boundary cannot both win; compare severity,
   confidence, then edit cost.
7. **Deduplication:** two rules that propose the same edit produce one
   diagnostic. Two rules that propose different edits for the same span keep
   the higher-confidence one and attach the other as an alternative only if it
   is genuinely compatible.
8. **Deterministic order:** severity desc, confidence desc, start asc, kind
   stable. Golden tests depend on this.

## 9. Public API and wasm

Rust:

```rust
pub fn check(text: &str) -> SyntaxCheck;
pub fn check_with_cache(text: &str, cache: &mut syntax::AnalysisCache) -> SyntaxCheck;
pub fn check_analysis(tokens: &[Token], analysis: &SyntaxAnalysis)
    -> Vec<SyntaxDiagnostic>;
```

Wasm (new exports, no change to `analyze_syntax`):

```text
check_syntax(text) -> {
  analysis: { sentences, clauses, tokens, lists },
  diagnostics: [{
    kind, severity, start, end, message, confidence, expected,
    corrections: [{ edits: [{start, end, replacement}], score, note }]
  }]
}

check_syntax_with_state(idx, text) -> same, through the textarea's cache
```

The existing `analyze_syntax_with_state` stays the analysis-only export. Once
`SyntaxAnalysis.lists` lands, both analysis exports carry the same `lists`
array; `check_syntax` only adds the diagnostics. The diagnostic panel calls
`check_syntax_with_state`; it does not re-run the analysis separately.

`lists` is serialised as:

```text
lists: [{
  start, end, tokensStart, tokensEnd, clause, kind,
  items: [{ start, end, tokensStart, tokensEnd, kind }],
  separators: [tokenIndex], conjunction, introducer, parent
}]
```

`kind` is `nominal`, `clause`, `predicate` or `mixed`; an item's `kind` is
`word`, `phrase`, `clause` or a child list index.

The extension should render:

- `Error` as a grammar underline;
- `Warning` with a weaker hint;
- `Suggestion` only in an optional style mode;
- corrections via the edit list, not by replacing the whole text.

## 10. Data files

| file | content | producer |
|---|---|---|
| `data/verb_rection.csv` | `lemma,case,frame,priority,note` | curated, reviewed against Kielitoimiston ohjepankki |
| `data/comma_phrases.csv` | fixed phrases where a comma is not free | curated, only when observed |
| `maps/verb_rection.bin.gz` | `lemma -> cases` | `build.rs` / `build/map_builders` |

The rection map is a `StemMap` (lemma → case names), like the other small
lexical maps. Update `data/README.md` and `maps/README.md`. `build.rs` gets a
`rerun-if-changed` line for the CSV.

Do not put the subordinator/relative/coordinator lists in a new file; they
already exist in `src/syntax.rs`. Make them `pub(crate)` and reuse them.

## 11. Tests and evaluation

### Diagnosis fixture

`tests/fixtures/syntax_diagnostics.json`:

```json
{
  "cases": [
    {
      "id": "sv_agreement_3pl_subject",
      "text": "Koira nukkuvat.",
      "diagnostics": [
        {
          "kind": "subject_predicate_agreement",
          "severity": "error",
          "start": 6,
          "end": 14,
          "corrections": ["nukkuu"]
        }
      ]
    }
  ]
}
```

The runner:

- asserts exactly the expected diagnostics, ordered deterministically;
- applies each correction to the source and checks the resulting text;
- has a `clean` list of sentences where **no `Error`** is allowed.

`tests/syntax_check.rs` is the runner, with one `#[test]` per case (as the
syntax corpus does, so cases run in parallel).

### Clean corpus

At least the following must produce no `Error`:

```text
Pöydällä on kirjat.
Pöydällä on kirja.
Talo on rakennettu.
Talo on kaunis.
Minun täytyy mennä.
Hänen pitää mennä.
Kaksi miestä tuli.
Hänellä on kaksi lasta.
Sami ja Liisa söivät.
Sami söi puuron.
Lapset leikkivät.
Koirat nukkuvat.
Koira nukkuu.
Minä menen kotiin.
Hän antoi kirjan.
En lue kirjaa.
Luen kirjaa.
Luen kirjan.
Koira, joka nukkuu sohvalla, on minun.
Koira joka nukkuu sohvalla on minun.
Punaisen auto on punainen.   # ambiguous genitive, no error
Ostin omenoita, appelsiineja ja banaaneja.
Ostin omenoita, appelsiineja, banaaneja ja päärynöitä.
Ostin omenoita, appelsiineja, ja banaaneja.   # suggestion, not Error
Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.
Ostin omenoita, Liisa osti banaaneja ja Matti osti päärynöitä.
Kuitenkin, ostin omenoita, jotka olivat kalliita, eilen.
```

The two relative-clause variants are both acceptable: the comma is a
restrictive/non-restrictive choice, not an error. `Punaisen auto` is ambiguous
(genitive of a nominalized adjective) and must stay quiet.

### List fixture

`tests/fixtures/syntax_diagnostics.json` gets a list section:

```json
{
  "id": "list_missing_separator",
  "text": "Ostin omenoita appelsiineja, banaaneja ja päärynöitä.",
  "diagnostics": [
    {
      "kind": "list_separator_missing",
      "severity": "error",
      "start": 14,
      "end": 14,
      "corrections": [","]
    }
  ]
}
```

Cases:

```text
valid:       Ostin omenoita, appelsiineja ja banaaneja.
valid:       Ostin omenoita, appelsiineja, banaaneja ja päärynöitä.
missing:     Ostin omenoita appelsiineja, banaaneja ja päärynöitä.
duplicate:   Ostin omenoita, appelsiineja,, ja banaaneja.
suggestion:  Ostin omenoita, appelsiineja, ja banaaneja.
introduced:  Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.
introducer,: Ostin kolme asiaa:, omenoita, appelsiineja ja banaaneja.
not a list:  Kuitenkin, ostin omenoita, jotka olivat kalliita, eilen.
clauses:     Ostin omenoita, Liisa osti banaaneja ja Matti osti päärynöitä.
mixed:       Ostin omenoita, appelsiineja ja Liisa osti banaaneja.
```

Unit tests live beside each rule module, but the fixture is the gate. A
hand-written list is not enough.

### Manual/CLI

`tools/src/bin/check_syntax.rs` prints tokens, clauses and diagnostics for one
sentence or a file. This is the human view during rule development. It is not
a gate.

### Gates

- `cargo test --release --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- clean corpus must stay clean;
- the existing syntax corpus and engine gates must stay green.

## 12. Phases

Each phase is a separate commit with its fixture rows.

### Phase 0 — contract, fixtures, no rules — done

- [x] `src/syntax_check/` skeleton and public types (`diagnostic`, `agreement`,
      `correction`, `rank`).
- [x] `check`, `check_analysis`, `check_with_cache`.
- [x] `tests/fixtures/syntax_diagnostics.json`, `tests/syntax_check.rs` and the
      clean corpus.
- [x] `tools/src/bin/check_syntax.rs`.
- [x] Wasm exports `check_syntax` / `check_syntax_with_state`.

### Phase 1 — subject–predicate agreement (the first release) — done

- [x] Finite verb, compound auxiliary, and negative auxiliary agreement.
- [x] Coordinated subjects and numeral subjects.
- [x] Exceptions: existential, passive, necessive, infinitive, impersonal.
- [x] Corrections from `forms_of`, plain forms only.

Acceptance (passing):

```text
Koira nukkuvat.       → Koira nukkuu.
Minä menee kotiin.    → Minä menen kotiin.
Hän antoivat kirjan.  → Hän antoi kirjan.
Lapset leikkii.       → Lapset leikkivät.
Me menee.             → Me menemme.
Minä on lukenut.      → Minä olen lukenut.
Hän en tule.          → Hän ei tule.
```

The clean corpus has no diagnostics at all, including existential, passive,
necessive, numeral and coordinated constructions.

### Phase 2 — attribute–noun agreement — done

- [x] Positive-degree attributive adjectives.
- [x] Demonstratives/ordinals where the case/number is unambiguous.
- [x] Skip genitive modifiers, titles, names, and non-positive degrees.
- [x] Predicate-less phrases (`Punainen autot`) use the adjacent nominal as
      the head; full clauses use the parser's `head` link.
- [x] Cardinal numerals stay out (they govern the partitive, not agreement).

Acceptance (passing):

```text
Punainen autot.       → Punaiset autot.
Punaiset auto.        → Punainen auto.
Vanhat talo.          → Vanha talo.
Nämä kirja on hyvä.   → Tämä kirja on hyvä.
Tämä kirjat ovat hyviä. → Nämä kirjat ovat hyviä.
Punaisessa auto.      → Punainen auto. (warning: case)
```

No errors for `punaisen auton`, `punaisen auto` (ambiguous genitive),
`Vanha talo`, `Sami Ruotsalainen`, `kaksi miestä`, `Punaisempi auto`,
`Punaisessa talossa` or `Hän tuli iloisena kotiin`.

### Phase 3 — lists, comma rules and clause kinds — mostly done

Lists first, because a comma-separated nominal run must not be mistaken for
clause coordination.

- [x] `src/syntax_list.rs`: `List`, `ListItem`, `ListKind`,
      `ListItemKind`, and `parse`.
- [x] `SyntaxAnalysis.lists` and wasm serialisation.
- [x] Nominal / Clause / Predicate / Mixed classification.
- [x] Top-level comma splitting, final conjunction, colon introducer.
- [ ] Nested list arena and parent index: the fields and `ListItemKind::List`
      exist, but the parser still produces only top-level lists.
- [x] List punctuation rules and the Finnish default policy.
- [x] `ClauseKind` derivation.
- [x] Relative-clause extent heuristics for the interrupted-host case.
- [x] Subordinate comma in both orders, including the merged
      `Koska olin väsynyt menin kotiin` case.
- [x] Relative comma pair only when the opening comma exists; no error for a
      restrictive relative without commas.
- [x] Unbalanced pair detection.
- [x] Coordinate comma advisory only (no automatic diagnostic).

Acceptance (passing):

```text
Ostin omenoita, appelsiineja ja banaaneja.        # valid list
Ostin omenoita appelsiineja, banaaneja ja päärynöitä.
                                                   → missing separator
Ostin omenoita, appelsiineja,, ja banaaneja.       → duplicate comma
Ostin omenoita, appelsiineja, ja banaaneja.        # suggestion only
Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.  # introduced list
Ostin kolme asiaa:, omenoita, appelsiineja ja banaaneja. → introducer comma
Ostin omenoita, Liisa osti banaaneja ja Matti osti päärynöitä.
                                                   # clause coordination, no list error
Menin kotiin koska olin väsynyt.   → Menin kotiin, koska olin väsynyt.
Koska olin väsynyt menin kotiin.   → Koska olin väsynyt, menin kotiin.
Koira, joka nukkuu sohvalla on minun.
                                   → Koira, joka nukkuu sohvalla, on minun.
Menin kauppaan ja ostin maitoa.    # no error
Sami söi puuron, ja Liisa joi maidon. # no error; comma is optional
```

No error for a restrictive relative without commas, and no error for
`Kuitenkin, ostin omenoita, jotka olivat kalliita, eilen.` (not a list).

Nested lists are the one remaining list item. The structure has `parent` and
`ListItemKind::List(idx)`, but `parse` currently builds the top-level arena
only. That is the next list commit, not a blocker for the diagnostics above.

### Phase 4 — verb rection and case selection — done

- [x] `data/verb_rection.csv` + `verb_rection.bin.gz`.
- [x] Rection rule and multi-frame corrections, in CSV priority order.
- [x] 32 curated frames across 30 verbs.
- [x] Candidates are post-predicate objects or post-predicate nominative
      subjects (`Pidän sinä`); free adverbials (`Odotan kotona`,
      `Asun kadulla`, `Katson ikkunasta`) are left alone.

Acceptance (passing):

```text
Pidän sinä.       → Pidän sinusta. (sinua as an alternative)
Menen koulu.      → Menen kouluun. (koululle as an alternative)
Luotan sinä.      → Luotan sinuun. (warning: weak predicate)
Kuuntelen sinä.   → Kuuntelen sinua.
Hän rakastaa sinä. → Hän rakastaa sinua.
```

No error for `Pidän sinusta`, `Pidän sinua`, `Menen kouluun`,
`Menen koululle`, `Luotan sinuun`, `Kuuntelen sinua`, `Minä opiskelen suomea`,
`Odotan kotona`, `Asun kadulla`, `Opiskelen koulussa` or
`Katson ikkunasta`.

### Phase 5 — object case, arguments, word order — partial

- [x] Negative-clause partitive as an error.
- [x] Rection outranks the general negative-object rule on the same span
      (`En pidä sinä` offers `sinusta`/`sinua`, not only `sinua`).
- [ ] Aspect-based object case as a suggestion only.
- [ ] Missing/extra arguments for explicitly marked frames.
- [ ] Word order: no rules; document the deferral.

Acceptance (passing):

```text
En lue kirja.      → En lue kirjaa.        (error)
Sami ei syö puuro. → Sami ei syö puuroa.   (error)
En pidä sinä.      → En pidä sinusta.      (rection wins)
```

No error for `En lue kirjaa`, `En syö puuroa`, `En pidä sinusta`,
`En mene kouluun`.

### Phase 6 — UI, ranking, evaluation

- [ ] Conflict resolution and deterministic ordering.
- [ ] Extension/hover rendering with edit application.
- [ ] Diagnostic cache in wasm (reuse `AnalysisCache`; rules are cheap).
- [ ] Measure on a 100-sentence corpus; no `Error` on the clean corpus.
- [ ] Decide whether the style layer starts.

## 13. False-positive policy

Ship fewer rules, not more guesses.

- For role-based rules, `Error` only with `RoleConfidence::Likely` on both the
  agreeing token and its controller, and a single reading for the relevant
  dimension.
- For punctuation, `Error` requires a clear clause boundary and a known clause
  kind; an ambiguous relative-clause reading is a `Suggestion`.
- A list requires at least two parallel items plus a real cue (top-level comma,
  final conjunction or colon introducer). A comma sequence broken by a
  relative clause, a subordinate clause or an introductory phrase is not a
  list.
- `Mixed` lists never produce an automatic punctuation correction. A final
  conjunction without a comma is never an error in Finnish, and a comma before
  it is only a suggestion. No other language policy applies.
- `Warning` for constructions with standard variation (coordinated subjects,
  colloquial number, optional commas).
- `Suggestion` for style/optional punctuation.
- Skip a rule when the token has `readings.is_empty()` (unknown word), when
  the role is `Unknown`, or when the agreement target is ambiguous.
- Never infer a valency frame from a suffix or from a dictionary class; use
  the curated table.
- A clean corpus failure is a release blocker.

## 14. Open questions

1. **List parser placement.** `src/syntax_list.rs` as a sibling of
   `syntax.rs`, or convert `syntax.rs` into a directory and add
   `syntax/list.rs`? Recommendation: the sibling module first; move it into a
   `syntax/` directory only when more phrase-level analyses appear.
2. **List punctuation policy.** Confirm Finnish-only punctuation; no other
   language policy is planned.
3. **Clause kinds and relative extent.** Do we extend `syntax::Clause` with
   `kind`/`parent`, or derive them in `syntax_check`? Recommendation: derive
   first, promote only when a second consumer needs them.
4. **Adjective degree.** `Tags` has no degree; do we add it to the grammar
   model, or detect degree from the surface and skip non-positive corrections?
   Recommendation: skip in v1, add a degree dimension only if the checker
   needs to correct comparatives.
5. **Rection data.** How large should the curated table grow, and who reviews
   it? Recommendation: 30–50 high-frequency verbs, two frames maximum per
   verb, no semantic guesses.
6. **Comma norm.** Which relative clauses are non-restrictive? Recommendation:
   only unbalanced pairs and subordinate clauses are errors; relative commas
   are suggestions unless an opening comma is already present.
7. **Style layer.** Keep it separate. It shares the analysis but has its own
   `DiagnosticKind`, severity, and opt-in UI.

## 15. Recommended first release

The first release is exactly the user's "contextual inflection error detection
and correction":

```text
Minä menee        → Minä menen
Koira nukkuvat    → Koira nukkuu
Punainen autot    → Punaiset autot
Lapset leikkii    → Lapset leikkivät
```

It needs phase 0, phase 1 and phase 2 only. All three are landed. Then add
lists/punctuation (phase 3) and rection (phase 4). A checker that is right
about four constructions is more useful than one that is unsure about twenty.

Lists are planned now but are not part of the first release. Their structure
is a prerequisite for phase 3 punctuation, not a prerequisite for the four
agreement rules.
