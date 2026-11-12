# PLAN_SYNTAX_CHECKER — grammar and punctuation diagnostics

Status: Phases 0–5 are landed, including nested lists, missing/extra argument
warnings, the word-order decision record and aspect-based object-case
suggestions (singular completive; plural deferred). Phase 6 has deterministic
ranking, the wasm word cache, the evaluation baseline, punctuation phase 2,
the Leptos diagnostic panel with edit application, the browser-extension
grammar check and the checker docs. The style layer is deliberately out of
scope; deferred edge cases are listed in `docs/syntax_checker.md`. The analysis layer it builds on is `PLAN_SYNTAX.md` and is already landed
on the `syntax` branch.

The remaining work is indexed in §16; each open item has enough detail to be
implemented without reopening the whole design.

The goal is a **correction checker**, not another parser: detect a small set of
high-confidence grammatical errors, explain them, and produce corrections from
the existing morphology engine. It sits on top of `syntax::analyze`; it does
not replace it and it does not run in the keystroke path.

Finnish examples are kept in Finnish. The rules and messages live in the engine;
UI copy is the caller's concern.

## 1. Scope

Priority order:

| priority | rule | status |
|---|---|---|
| 1 | subject–predicate agreement | done |
| 2 | attribute–noun agreement | done |
| 3 | verb rection | done |
| 4 | case selection | done through rection |
| 5 | object case | negative case done; aspect suggestions in phase 5B |
| 6 | missing/extra arguments | phase 5C, warning only |
| 7 | word order | deferred; decision record in phase 5D |
| 8 | list structure and list separators | done, including nested lists |
| 9 | comma/clause punctuation | commas done; semicolons/quotes in phase 6B |

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

**Lists are an implemented extension to the analysis layer.** `SyntaxAnalysis`
has no phrase parser, so it cannot tell a list from clause coordination from
clauses alone. The checker adds `lists: Vec<List>`, built by a focused
phrase-level pass (`syntax_list`), and uses it for list punctuation. Nested
lists are a post-order child arena: `ListItemKind::List(index)` and
`List.parent` walk from the parent to the child. It is not a full
phrase-structure parser, but it is a real syntactic structure, not a comma
heuristic.

Clause kinds and relative-clause extent are now derived in
`src/syntax_check/punctuation.rs`; the analysis layer still keeps linear
clause ranges. A relative clause inside its host still splits the host
(`Mies, joka tuli, söi` becomes three ranges), which is enough for the comma
rules but not for a UI that wants the host clause as one unit. Host nesting and
antecedent linking are deferred analysis refinements in `PLAN_SYNTAX.md`, not
blockers for the checker.

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

Module layout (actual plus the planned phase-5C module):

```text
src/syntax_list.rs  List, ListItem, ListKind, parse (top-level + nested)
src/syntax_check/
  mod.rs          public API, orchestration
  diagnostic.rs   DiagnosticKind, Severity, Diagnostic, Edit, Correction
  agreement.rs    subject–predicate and attribute–noun
  rection.rs      valency frames and case selection
  object_case.rs  negative-clause partitive, aspect suggestions (phase 5B)
  arguments.rs    missing/extra arguments (phase 5C)
  punctuation.rs  clause/list punctuation and clause-kind derivation
  correction.rs   build corrections with forms_of
  rank.rs         scoring, dedup, deterministic ordering
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

`ListItem` also carries a crate-private `scan` range that runs over trailing
separators; it exists only so the nested parser can see the closing bracket
after the item's last word. It is not serialised.

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

`kind` is `nominal`, `clause`, `predicate` or `mixed`. An item's `kind` is
`word`, `phrase`, `clause` or `list`; when it is `list`, the item also carries
`list`, the index of the child in the same `lists` array. The array is
post-order, so a child index is always smaller than its parent's.

The extension should render:

- `Error` as a grammar underline;
- `Warning` with a weaker hint;
- `Suggestion` only in an optional style mode;
- corrections via the edit list, not by replacing the whole text.

## 10. Data files

| file | content | producer |
|---|---|---|
| `data/verb_rection.csv` | `lemma,case,frame,priority,note` | curated, reviewed against Kielitoimiston ohjepankki |
| `data/verb_arguments.csv` | `lemma,slot,case,requirement,note` (phase 5C) | curated, reviewed against Kielitoimiston ohjepankki |
| `data/comma_phrases.csv` | fixed phrases where a comma is not free | curated, only when observed |
| `maps/verb_rection.bin.gz` | `lemma -> cases` | `build.rs` / `build/map_builders` |
| `maps/verb_arguments.bin.gz` | `lemma/slot -> requirement` (phase 5C) | `build.rs` / `build/map_builders` |

The rection map is a `StemMap` (lemma → case names), like the other small
lexical maps. Keep the argument-completeness table in its own blob so a change
in “must the argument exist” cannot silently change the case choices. Update
`data/README.md` and `maps/README.md`. `build.rs` gets a `rerun-if-changed`
line for each CSV.

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

### Evaluation corpus

The exact-output fixture proves individual rules; it does not measure how
often the checker is wrong on real prose. `tests/fixtures/syntax_eval.json` is
the second, larger corpus for aggregate evaluation:

- 159 sentences at the current baseline: 109 clean and 50 with expected
  diagnostics;
- one row per sentence, with the expected kind(s);
- every emitted kind represented, plus the guards named in §13;
- run by `tests/syntax_eval.rs`, which prints per-kind precision/recall and the
  clean-corpus `Error` count.

The release gate is **zero `Error` on clean rows**. Suggestion counts are
recorded, not gated, until the baseline is stable. The exact-output fixture
remains the per-case gate.

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

### Phase 3 — lists, comma rules and clause kinds — done

Lists first, because a comma-separated nominal run must not be mistaken for
clause coordination.

- [x] `src/syntax_list.rs`: `List`, `ListItem`, `ListKind`,
      `ListItemKind`, and `parse`.
- [x] `SyntaxAnalysis.lists` and wasm serialisation.
- [x] Nominal / Clause / Predicate / Mixed classification.
- [x] Top-level comma splitting, final conjunction, colon introducer.
- [x] Nested list arena and parent index: `parent`, `ListItemKind::List` and
      a post-order child arena, built after `src/syntax.rs` merges
      bracket-only sentence boundaries.
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

Nested lists are done. `src/sentence.rs` still treats brackets as sentence
boundaries for the case layer, so `src/syntax.rs`'s `clause_groups` merges
bracket-only boundaries before clause segmentation; `src/syntax_list.rs` then
parses the parenthetical recursively and appends a post-order child arena.
`Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.` is one parent
list with three items and one child list with two. The design record is
[`PLAN_NESTED_LISTS.md`](PLAN_NESTED_LISTS.md).

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

### Phase 5 — object case, arguments, word order

#### Phase 5A — negative-clause partitive (done)

- [x] Negative-clause partitive as an error.
- [x] Rection outranks the general negative-object rule on the same span
      (`En pidä sinä` offers `sinusta`/`sinua`, not only `sinua`).
- [x] A valid partitive object stays quiet.

Acceptance (passing):

```text
En lue kirja.      → En lue kirjaa.        (error)
Sami ei syö puuro. → Sami ei syö puuroa.   (error)
En pidä sinä.      → En pidä sinusta.      (rection wins)
```

No error for `En lue kirjaa`, `En syö puuroa`, `En pidä sinusta`,
`En mene kouluun`.

#### Phase 5B — aspect-based object case, suggestion only

Goal: make the total/partitive choice visible without pretending the checker
knows the speaker's aspect. This phase must never produce `Error`.

Design:

- Only affirmative clauses; the negative rule in 5A owns negation.
- Only a clause whose `Clause.object` is set, or whose first post-predicate
  nominal is unambiguous enough to be an object.
- Only a nominal with a single retained case reading. `kirjaa` may be weak
  (it is also a verb form), but it must not be an unknown word.
- Two closed cue lists in `src/syntax_check/object_case.rs`, matched as token
  runs so the multiword cues work:
  - `PROGRESSIVE_CUES`: `parhaillaan`, `juuri nyt`, `tällä hetkellä`,
    `kesken`, `paraikaa`.
  - `COMPLETIVE_CUES`: `loppuun`, `valmiiksi`, `kokonaan`, `viimeistään`
    only when it modifies the object's completion, not a deadline. If that
    reading is not certain, leave `viimeistään` out.
- A progressive cue plus a total object (genitive singular, nominative plural,
  or accusative) suggests the partitive.
- A completive cue plus a partitive object suggests the total object. For a
  singular object the target is the **genitive**. Plural is deferred: a
  partitive plural is often homonymous (`kirjoja` belongs to both `kirja` and
  `kirjo`), so the syntax layer's chosen lemma can produce a different word
  (`kirjot`). Stay quiet until the reading carries per-lemma candidates.
- The diagnostic is `Suggestion`, `DiagnosticConfidence::Medium`, one
  correction, never an `Error`.
- If the cue and the object are in different clauses, stay quiet.
- If the object is a numeral phrase, a proper name without a case, or an
  unknown word, stay quiet.

Acceptance:

```text
Luen parhaillaan kirjan.   → suggestion: Luen parhaillaan kirjaa.
Luin kirjaa loppuun.       → suggestion: Luin kirjan loppuun.
Luen parhaillaan kirjaa.   # clean
Luin kirjan loppuun.       # clean
Luen kirjan.               # clean: no aspect cue
Luen kirjaa.               # clean: no aspect cue
Luin kirjoja loppuun.      # clean: plural completive deferred
```

The clean corpus must stay free of `Error`; suggestions are allowed only on
the two cue rows above.

Tasks:

- [x] Add the two closed cue tables and the total-object target.
- [x] Emit only `Suggestion`/`Medium`; add all guards above.
- [x] Add the two suggestion cases and the clean rows to the fixture.
- [x] Add unit tests for the cues, the matching cases, the parenthetical guard
      and the no-cue path.
- [x] Defer plural completive with an explicit guard and test.

Implementation notes:

- Cues are matched as word runs, so `juuri nyt` and `tällä hetkellä` work.
  `viimeistään` is omitted: its “at the latest” reading is not completion.
- A cue inside a parenthetical is ignored (`Luen (parhaillaan) kirjan.`).
- The object is taken from `Clause.object`; a clause without one stays quiet.
- The plural completive path is deliberately silent until a partitive plural
  can name its lemma without the homonym risk.

#### Phase 5C — missing and extra arguments, warning only

Goal: catch the small set of frames where an argument is obligatory, and the
small set of verbs that take none. This is not a general valency model.

Data:

- Add `data/verb_arguments.csv` with the schema
  `lemma,slot,case,requirement,note`.
  - `slot`: `object` or `recipient`.
  - `case`: a case name, or `any` when the frame accepts several.
  - `requirement`: `obligatory` or `forbidden`.
- Build it into `maps/verb_arguments.bin.gz` with a writer in
  `build/map_builders/front_coded.rs`; read it through a small
  `src/expand/verb_arguments.rs` runtime module.
- Keep it separate from `verb_rection.csv`. Rection answers “which case does
  this argument take”; this table answers “must the argument exist”. Mixing
  them would silently make existing frames obligatory.
- Start with 10–20 verbs and only frames verified against Kielitoimiston
  ohjepankki or a dictionary example. Do not infer transitivity from a class
  or a suffix.

Missing-argument rule:

- Fire only for an `obligatory` row whose predicate is in the table.
- The clause must have no candidate nominal in any accepted case for that
  slot; a candidate in the wrong case already belongs to rection.
- Do not fire for passive clauses (the argument may be suppressed), for a
  clause with no subject in a context that permits ellipsis, or when the
  clause ends in an ellipsis marker.
- Emit `MissingArgument`, `Warning`, `DiagnosticConfidence::Medium`, with no
  automatic correction. The message names the expected slot/case.

Extra-argument rule:

- Fire only for a `forbidden` row (a closed intransitive table) and an
  object-role nominal with a clear case reading.
- Do not fire for a local-case adverbial, a modifier, or a numeral complement.
- Emit `ExtraArgument`, `Warning`, `Medium`, with no automatic correction.
- Do not add `sataa` to the forbidden table: `Sataa vettä` is idiomatic.

Acceptance:

```text
Hän luottaa.        → missing_argument warning (illative)
Luotan sinuun.      # clean
Nukun kirja.        → extra_argument warning
Nukun hyvin.        # clean
Sataa.              # clean
Sataa vettä.        # clean
Minä nukun.         # clean
```

Tasks:

- [x] Add `data/verb_arguments.csv` and its build writer/blob (`maps/verb_arguments.bin.gz`).
- [x] Add `src/expand/verb_arguments.rs` with a lookup per lemma/slot.
- [x] Implement `MissingArgument` with the passive/ellipsis guards.
- [x] Implement `ExtraArgument` with the adverbial/numeral guards.
- [x] Add fixtures for one warning of each kind and the clean rows above.
- [x] Confirm rection and argument checks never both fire on the same span.

Implementation notes:

- The table has 17 rows: 11 obligatory frames and 6 forbidden-object verbs.
- `slot` has a third value, `argument`, for case-governed complements
  (`luottaa johonkin`). The plan's original two values were too narrow.
- A post-predicate nominative can be read as a subject (`Nukun kirja`); it is
  an extra argument only when the predicate's person/number cannot agree with
  it. A genuine presentational subject (`Herää mies`) agrees and stays quiet.
- A wrong-case argument is left to rection; the missing rule only fires when
  the predicate's own segment has no nominal candidate at all.
- A one-word clause (`Rakastan.`) and a clause ending in `...` stay quiet.

#### Phase 5D — word order decision record

No word-order rules are planned. Finnish order is information structure, and
`Söi Sami puuron`, `Puuron söi Sami` and `Sami söi puuron` are all grammatical.

Tasks:

- [x] Add `Söi Sami puuron.` and `Puuron söi Sami.` to the clean corpus.
- [x] Add a comment on `DiagnosticKind::WordOrder` saying the kind is reserved
      and not emitted, so a future reader does not assume a missing rule.
- [x] Keep `WordOrder` out of the severity/ranking examples; the rank test now
      uses `CommaMissing` as its suggestion placeholder.

This closes the deferral without shipping a rule.

### Phase 6 — punctuation phase 2, UI, ranking, evaluation

#### Phase 6A — ranking and conflict resolution (done)

- [x] Deterministic ordering: severity, confidence, start, end, kind.
- [x] One diagnostic per span; rection wins over the general object-case rule.
- [x] Corrections sorted by score, then note, then replacement.

#### Phase 6B — punctuation phase 2

The current punctuation module owns commas and list separators. This phase
adds the remaining punctuation the tokeniser already exposes. It stays a
separate rule library fed by clauses and list spans.

**Semicolons**

- A semicolon is a clause boundary for scanning, never an `Error`.
- Do not suggest replacing a comma with a semicolon or vice versa; Finnish
  usage is stylistic.
- Task: make `syntax_list` and clause-comma rules treat `;` as a boundary, so
  a semicolon cannot make two clauses look like one list.

**Question marks**

- Trigger when a sentence's first clause starts with an interrogative word
  (`kuka`, `mikä`, `missä`, `mistä`, `mihin`, `milloin`, `miten`, `kuinka`,
  `miksi`, `ken`, `kumpi`, `montako`) or its predicate carries the `-ko`/`-kö`
  clitic.
- Require the clause to be sentence-initial and the only main clause before
  the terminal separator. This excludes indirect questions such as
  `Kysyin, missä hän asuu.`
- If the sentence ends with `.` or no terminal mark, use a new
  `QuestionMarkMissing` kind. Emit `Error`/`High` only when the trigger is
  unambiguous; otherwise stay silent.
- Correction: replace the final `.` with `?`, or insert `?` at the last word's
  end. When the question is quoted, the correction goes before the closing
  quote.
- Skip dialogue fragments and any sentence whose terminal separator already
  contains `?`.

**Quotes and dialogue dashes**

- Use `sentence::TokenContext.quote_depth` to detect an unbalanced quote at
  the end of the document. Emit one `Warning`, not a per-line error.
- Do not correct quote style, dialogue dashes, or dash spacing. Those are
  editorial choices.
- A quoted question still satisfies the question-mark rule when the `?` is
  inside the quote; the final separator is not the only place to look.

**Parentheticals and appositions**

- Comma pairs around clear parentheticals (`esimerkiksi`, `kuten`, short
  appositions) are advisory. At most a `Suggestion`.
- Use a small closed exception list; do not infer a parenthetical from every
  adverb or every comma pair.
- A missing pair is never an `Error`; the existing relative-clause rule is the
  only pair rule allowed to emit one.

**Colon and direct speech**

- The colon introducer rule stays list-only. `Hän sanoi: …` is not a list and
  must stay quiet.
- Semicolon/colon scanning must not cross a quote or a parenthetical.

Tests:

```text
Missä sinä asut.          → Missä sinä asut?
Onko sinulla kirjaa.      → Onko sinulla kirjaa?
Kysyin, missä hän asuu.   # clean: indirect question
Hän sanoi: »Missä sinä asut?»  # clean
»Missä sinä asut.»        → »Missä sinä asut?»   (inside the quote)
Hän tuli; minä lähdin.    # clean: semicolon style
```

Tasks:

- [x] Treat `;` as a clause/list boundary in scanning; emit no diagnostic for
      the semicolon itself.
- [x] Add `QuestionMarkMissing` (or the chosen kind) to `DiagnosticKind`, with
      the sentence-initial/only-main-clause guard.
- [x] Add the unbalanced-quote warning from `quote_depth`; no style edits.
- [x] Add the advisory parenthetical pair with the closed cue list.
- [x] Add the six rows above and their clean/dirty split to the fixture.
- [x] Confirm indirect questions and quoted questions stay quiet.

Implementation notes:

- A top-level `;` splits clauses in `syntax.rs`; a semicolon inside a
  parenthetical or quotation does not. No semicolon diagnostic is emitted.
- The question rule fires only when a sentence has one clause and its first
  word is interrogative or carries `-ko`/`-kö`. It replaces the final `.`, or
  inserts `?` before a closing quote/bracket. Indirect questions and
  `kuka tahansa`-style indefinites stay quiet.
- The parenthetical pair is `Suggestion` only and currently limited to
  `esimerkiksi`. `kuten` is deferred because its parenthetical is the following
  phrase, not the cue word.
- An unbalanced quote is one `Warning` at the end of the document; quote style
  and dialogue dashes are never corrected.

#### Phase 6C — UI and edit application

The wasm layer already returns clauses, lists and diagnostics with byte spans
and `Edit` arrays. No rule work is needed.

- Add a diagnostic panel to `torille-wasm-leptos` that lists diagnostics by
  rank and shows the rule name, severity and correction.
- Apply a correction by replaying its edits **from the end of the text
  backwards**, so earlier offsets stay valid.
- Add an opt-in grammar toggle to the extensions; do not run `check_syntax`
  on every keystroke. Debounce or run on blur.
- Acceptance: applying all edits from the fixture produces the expected
  corrected sentence; the panel order matches `rank::resolve`.

Tasks:

- [x] Add the diagnostic panel to `torille-wasm-leptos`
      (`components/syntax.rs`), with Finnish labels, severity classes and
      correction buttons.
- [x] Add reverse-order edit replay and tests for a replacement, a two-edit
      insertion pair and a deletion.
- [x] Add the opt-in grammar call to the browser extensions. The context menu
      item **Tarkista kielioppi** runs on one input; a floating panel lists
      diagnostics and applies corrections to textarea/input fields.
- [x] Add an integration test for the stateless fallback when the state handle
      is stale, and for UTF-16 span/edit offsets after non-ASCII text.

Implementation notes:

- The extension panel is reached from the context menu, so it is opt-in and
  never runs on a keystroke.
- The worker converts the checker's UTF-8 byte offsets to UTF-16 before the
  content script slices a JS string; the integration test uses `Äiti …` to
  catch a drift.
- Corrections are applied only to `<textarea>` and `<input>`; a
  contenteditable editor shows the diagnostics read-only because a range-aware
  writer is a separate problem.

#### Phase 6D — evaluation corpus

- Add `tests/fixtures/syntax_eval.json` (or a TSV if it gets long) with at
  least 100 sentences: roughly 60 clean and 40 with expected diagnostics.
- Cover every emitted kind and every guard: negation, passive, existential,
  necessity, numerals, proper names, relative clauses, brackets, lists,
  nested lists, semicolons and questions.
- Metrics: per-kind precision/recall, total `Error` count on clean rows, and
  the number of suggestions on clean rows.
- Gate: **zero `Error` on clean rows**. Record the first measured baseline in
  the plan or a short `docs/syntax_checker.md`; do not gate precision until
  the baseline is stable.
- Keep the existing per-case fixture as the exact-output gate; the evaluation
  corpus is the aggregate measure, not a replacement.

Tasks:

- [x] Add the 100+ sentence corpus (`tests/fixtures/syntax_eval.json`, 91
      clean + 41 cases at the first baseline).
- [x] Add the metrics integration test (`tests/syntax_eval.rs`) and print the
      per-kind table.
- [x] Record the first baseline here.
- [x] Gate zero `Error` on clean rows; keep suggestions ungated at first.

First baseline (2026-11-12):

```text
clean rows 109: 0 errors, 0 warnings, 0 suggestions
cases 50: every emitted kind precision 1.00, recall 1.00
```

The baseline is a snapshot, not a target: a new rule that adds an extra
diagnostic to a case lowers precision and should be explained in its commit,
but the gate is still zero `Error` on clean rows.

#### Phase 6E — style layer decision

Decision: do not start a style layer in this plan. If one is built later:

- it is a separate analyzer with its own module, `DiagnosticKind`s, severity
  and opt-in UI;
- it must not reuse grammar kinds or run in the same pass by default;
- it must not make the clean corpus noisier.

Tasks:

- [x] Keep the decision in this plan; no code until a real style request
      arrives.
- [x] Record the reopening condition: a separate plan, separate module and
      opt-in UI before any style rule is added.

#### Phase 6F — checker documentation

- Add `docs/syntax_checker.md`: rule catalogue, severity policy, false-positive
  policy, wasm shape, known limitations and the evaluation baseline.
- Link it from `README.md` and `AGENTS.md` only after the document exists.
- Keep the plan as the design record; the new doc is the user-facing contract.

Tasks:

- [x] Write `docs/syntax_checker.md` using the landed rules and the §16 index.
- [x] Add the README/AGENTS links after the document exists.

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
- A semicolon is never an error. It splits clauses for scanning, and a
  semicolon inside a parenthetical or quotation does not split the host.
- A missing question mark is an `Error` only when the sentence has one clause
  and its first word is interrogative or carries `-ko`/`-kö`. Indirect
  questions, `kuka tahansa`-style indefinites and sentences with a second
  clause stay quiet.
- An unbalanced quote is a `Warning`, never an `Error`; quote style and
  dialogue dashes are not corrected.
- A parenthetical comma pair is a `Suggestion`, never an `Error`, and only a
  closed cue list may trigger it.
- `Warning` for constructions with standard variation (coordinated subjects,
  colloquial number, optional commas).
- `Suggestion` for style/optional punctuation.
- Skip a rule when the token has `readings.is_empty()` (unknown word), when
  the role is `Unknown`, or when the agreement target is ambiguous.
- Never infer a valency frame from a suffix or from a dictionary class; use
  the curated table.
- A clean corpus failure is a release blocker.

## 14. Decisions recorded

1. **List parser placement.** Landed as the sibling `src/syntax_list.rs`.
   Move it into a `syntax/` directory only when a second phrase-level analysis
   needs the same home.
2. **Punctuation policy.** Finnish-only. No other language policy is planned.
3. **Clause kinds and relative extent.** `ClauseKind` stays in
   `syntax_check::punctuation` and is derived on demand. Promote it to
   `syntax::Clause` only when a second consumer needs it.
4. **Adjective degree.** `Tags` still has no degree. The checker uses the
   surface `expand::degree_base` guard to skip comparatives/superlatives; add a
   degree dimension only when a correction rule needs it.
5. **Rection data.** 32 frames across 30 verbs are landed. Grow by verified
   additions only, 30–50 high-frequency verbs, two frames maximum per verb, no
   semantic guesses.
6. **Comma norm.** Implemented as decided: only unbalanced pairs and
   subordinate clauses are errors; relative commas are suggestions unless an
   opening comma is already present.
7. **Style layer.** Out of scope for this plan; phase 6E records the conditions
   for reopening it.

## 15. Recommended first release

The first release is exactly the user's "contextual inflection error detection
and correction":

```text
Minä menee        → Minä menen
Koira nukkuvat    → Koira nukkuu
Punainen autot    → Punaiset autot
Lapset leikkii    → Lapset leikkivät
```

It needed phase 0, phase 1 and phase 2. All three are landed, followed by
lists/punctuation (phase 3), rection (phase 4) and the negative-object rule
(phase 5A). A checker that is right about four constructions is more useful
than one that is unsure about twenty.

Lists, including nested lists, are part of the landed checker. The checker
surface in §16 is complete. Deferred edge cases remain documented in
`docs/syntax_checker.md`: plural completive object case, `kuten` phrase
extent, linear relative clauses and one child list per item. Word order stays
deferred.

## 16. Remaining work index

Ordered by user-facing value. Each item points at the phase that specifies it.

1. [x] **Missing and extra arguments** — phase 5C. `data/verb_arguments.csv`
   plus `MissingArgument` / `ExtraArgument` warnings.
2. [x] **Aspect-based object case** — phase 5B. Suggestion only, closed cue
   tables, never an `Error`; singular completive, plural deferred.
3. [x] **Punctuation phase 2** — phase 6B. Semicolon boundaries, question-mark
   diagnostics, unbalanced quotes, advisory parenthetical pairs, no
   quote-style corrections.
4. [x] **Evaluation corpus** — phase 6D. 159 sentences, zero `Error` on clean
   rows, per-kind baseline recorded.
5. [x] **UI and edit application** — phase 6C. Leptos panel, reverse-order edit
   replay, and the opt-in browser-extension grammar check with a floating
   diagnostics panel.
6. [x] **Word-order decision record** — phase 5D. Clean rows and a comment on
   the reserved kind; no rule.
7. [x] **Checker documentation** — phase 6F. `docs/syntax_checker.md` with the
   rule catalogue and limitations; README and AGENTS link to it.
8. [x] **Style layer decision** — phase 6E. Recorded as out of scope; only a
   separate opt-in analyzer would reopen it.

Deferred analysis refinements live in `PLAN_SYNTAX.md` §“Remaining analysis
refinements”: relative-clause nesting, antecedent linking, and a multi-child
list shape. None is a blocker for the checker work above.
