# PLAN_SYNTAX_AST — a typed surface tree over `syntax`

Status: **Phases 0–2 landed.** The skeleton, head-driven phrases and the
multi-segment relative host are in `src/syntax/ast/`: `Ast`, `Node`, `NodeKind`,
the `seed`/`group`/`chunk` passes, the validator and the `syntax_tree` CLI. The
flat `syntax::SyntaxAnalysis` stays the authority; the tree is an additive view
built on demand. This plan defines the node kinds, the
construction pipeline, the invariants and the phase gates.

## Decision summary

| question | choice |
|---|---|
| representation | one append-only `Vec<Node>` arena with typed `NodeKind`s and `NodeId` links |
| authority | the flat model; the tree copies roles and never re-derives them |
| kinds | `Document`, `Sentence`, `Clause`, `Phrase`, `List`, `ListItem`, `Coordination`, `Parenthetical`, `Quote`, `Word` |
| pipeline | `seed` → `group` → `chunk` → `attach` → `coordinate` → `validate` |
| multi-word constituents | contiguous phrases only; one multi-segment host exception for a relative clause |
| first consumer | the word-order features (recommended; see open question 1) |
| phasing | six separate commits, zero diagnostic diff until the consumer phase |

## 1. Why a tree

The advisory syntax layer is deliberately flat. `SyntaxAnalysis` has four
fields:

```rust
pub struct SyntaxAnalysis {
    pub sentences: Vec<SentenceSpan>,
    pub clauses: Vec<Clause>,
    pub lists: Vec<syntax_list::List>,
    pub tokens: Vec<TokenSyntax>,
}
```

`TokenSyntax` carries the chosen role, its confidence, the governor token
(`head`), the case and every reading. `Clause` carries the token range and the
**token indices** of its predicate, subject, object and complement.
`syntax_list` already builds a real post-order arena with nested items.

That model answers "what is this word doing?" but not "what is this
constituent?":

- A multi-word object (`sen vanhan kirjan`) has no identity. Rules pass a
  token range and re-derive where it starts and ends.
- `Clause.tokens` is one contiguous range. A relative clause splits its host
  (`Mies, joka tuli, söi`) into three clauses, and the host cannot be asked for
  as one unit.
- `ListItemKind::Phrase` is a label, not a phrase node. A list item that is a
  numeral phrase looks the same as a bare word.
- The word-order specification ([`../docs/word_order.md`](../docs/word_order.md))
  talks about fronted constituents and interrogative phrases. The features on
  `Clause::features` name the construction, but they cannot point at the
  constituent that moved.
- `syntax_check` has helpers (`clause_words`, `nominal_case`, `first_word`,
  `last_word`, `is_marker`, …) that reconstruct clause and phrase structure at
  every call site. The helpers are correct; the missing layer is the structure
  itself.

An additive tree closes that gap without changing a single role, case or
diagnostic.

## 2. What “AST” means here

This is a **surface constituency tree**, not a compiler AST and not a semantic
representation:

- the leaves are word tokens, not characters or morphemes;
- the internal nodes are sentences, clauses, phrases, lists and coordination;
- morphology supplies the word readings and the role evidence;
- no word sense, no anaphora resolution, no ellipsis reconstruction, no
  semantic roles beyond `SyntacticRole`.

It is a **derived view**:

```rust
let analysis = syntax::analyze(&tokens);
let ast = syntax::ast::build(&tokens, &analysis);
```

The builder never searches the lexicon, never re-runs the role resolver and
never invents a role. Every structural claim it adds is either (a) a grouping
of tokens the flat model already linked with a `head` edge, or (b) a grouping
licensed by an existing flat structure (`Clause`, `List`, `SentenceSpan`).
When the flat model says `Unknown`, the tree has an unattached `Word` leaf, not
a guessed phrase.

The name `ast` is the user-facing one. In the documentation it is “the surface
tree”; the module path is `syntax::ast` for brevity.

## 3. Non-goals

- **No parser rewrite.** No statistical model, no external dependency, no
  grammar formalism, no chart parser.
- **No replacement of the flat model.** `SyntaxAnalysis` keeps every field and
  every consumer. The tree is a second view, not a new authority.
- **No diagnostic change in the tree phases.** `check_analysis` output stays
  byte-identical. A rule moves onto the tree only in a separate phase with a
  fixture that fails on the old path.
- **No keystroke-path work.** The builder is on-demand, like `syntax::analyze`.
- **No full UD/DG representation.** No empty categories, no traces, no
  movement chains, no discourse structure below the sentence.
- **No ambiguity packing.** One tree. Alternative readings stay in
  `TokenSyntax.readings`; an ambiguous edge gets `RoleConfidence::Weak`, not a
  forest.
- **No new data tables.** The tree is built from tokens, morphology and the
  existing flat analysis.

## 4. The kinds

One arena, one enum per structural layer. The “different kinds” are the
variants of `NodeKind`, refined by `ClauseKind` and `PhraseKind`.

| kind | payload | what it represents | what it must not claim |
|---|---|---|---|
| `Document` | — | the whole text | paragraphs; there is no paragraph node |
| `Sentence` | — | one `SentenceSpan` | a clause; a sentence may hold several |
| `Clause` | `ClauseKind` | one flat `Clause`, or a merged host of several | a new clause split |
| `Phrase` | `PhraseKind` | a head plus its intra-phrase dependents | a new role; the head's role is copied |
| `List` | `{ index }` | one `syntax_list::List` | a new list parse |
| `ListItem` | `{ list, item }` | one `syntax_list::ListItem` | a phrase; the item's children are phrases |
| `Coordination` | `CoordKind` | same-kind conjuncts joined by `ja`/`tai`/… | a list; lists own their coordination |
| `Parenthetical` | — | a balanced `(...)`, `[...]`, `{...}` | a clause; it may *contain* clauses |
| `Quote` | — | a quoted run, using `TokenContext::quote_depth` | reported speech semantics |
| `Word` | — | exactly one word token | a separator, a punctuation mark |

```rust
pub enum NodeKind {
    Document,
    Sentence,
    Clause(ClauseKind),
    Phrase(PhraseKind),
    List { index: usize },
    ListItem { list: usize, item: usize },
    Coordination(CoordKind),
    Parenthetical,
    Quote,
    Word,
}
```

`ClauseKind` is **structural only**:

```rust
pub enum ClauseKind {
    Main,
    Relative,
    Subordinate,
    Infinitive,
    Coordination,
}
```

`ClauseType`, `Construction`, `QuestionType`, `SubjectPosition`,
`ObjectCase`, `FocusType`, `TopicPosition` and `Register` stay on
`Clause::features` and are read through `Node::clause`. The tree does not copy
them: `Clause(main)` + `clause = 3` + `analysis.clauses[3].features` is the
whole answer.

`PhraseKind` is lexical:

```rust
pub enum PhraseKind {
    Noun,
    Adjective,
    Pronoun,
    Numeral,
    Adverb,
    Postposition,
    Preposition,
    Verb,
    Infinitive,
    Participle,
}
```

`Numeral` is a phrase whose head is a numeral (`Kaksi` in `Kaksi miestä`) or
a nominal head with a numeral dependent. The head is the token the flat model
made the clause slot; in the current model the counted noun is a `Modifier`
dependent of the numeral (`miestä` -> `Kaksi`), and the phrase records that the
whole constituent behaves like a numeral (singular agreement, partitive
object).

`CoordKind` is the category of the conjuncts:

```rust
pub enum CoordKind {
    Nominal,
    Verbal,
    Clausal,
    Mixed,
}
```

`Mixed` exists so a coordination the builder cannot type is still visible to a
consumer that wants to stay quiet on it.

## 5. Node shape and the arena

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    /// Byte envelope for diagnostics and UI highlighting.
    pub span: Range<usize>,
    /// Word-token envelope. Separators are not covered.
    pub tokens: Range<usize>,
    /// Word-token ranges this node owns directly. Usually one; a host clause
    /// split by a relative clause has two or more.
    pub segments: Vec<Range<usize>>,
    pub parent: Option<NodeId>,
    /// Source order, never empty for an internal node.
    pub children: Vec<NodeId>,
    /// The token this node is about: the phrase head, the clause predicate,
    /// the list head, or the conjunction of a coordination. `None` for the
    /// document, a sentence and a list item.
    pub head: Option<usize>,
    /// The flat `TokenSyntax::head` of `head`: the governor edge. It may
    /// repeat the parent link for an attached modifier; it is the only copy of
    /// a clause-level edge the tree keeps.
    pub governor: Option<usize>,
    /// The role the flat model chose for `head`, copied, never re-derived.
    pub role: Option<SyntacticRole>,
    pub confidence: RoleConfidence,
    /// For a relative clause, the nominal the relative pronoun modifies.
    pub antecedent: Option<usize>,
    /// Index into `SyntaxAnalysis::clauses` for the node's head token. A
    /// multi-segment host uses the flat clause of its predicate.
    pub clause: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ast {
    /// Append-only arena. Ids are creation order, not source order.
    pub nodes: Vec<Node>,
    /// The `Document` node. `seed` creates it first, so it is id 0.
    pub root: NodeId,
}
```

Why an append-only arena:

- the builder appends as it discovers structure, so ids are stable and no
  index shifts;
- parent and child links are plain `u32` indices, cheap to clone and
  serialise;
- a pass may create a node after its future parent (a list attached to an
  already-built phrase, a relative clause attached to a host), which a strict
  post-order arena would forbid;
- `children` is kept in source order, so a recursive walk visits siblings in
  text order. `Ast::walk()` returns a pre-order id list (parent before
  children, siblings by `tokens.start`).

There is no id-order invariant. Validation checks parent/child reciprocity and
acyclicity, not that a child id is larger than its parent.

`Ast::node(id)`, `Ast::children(id)`, `Ast::span(id)` and
`Ast::nodes_of_kind(...)` are read-only helpers. `span` is the byte envelope;
a consumer that highlights must use `segments` when it needs to leave a
relative clause out.

### Why both `head` and `governor`

The flat model has one edge, `TokenSyntax.head`, and it means “governor”, not
“lexical head”:

- in `Talo on kaunis`, `kaunis` has `head = on` (the copula governs the
  complement), but the adjective is the head of its own adjective phrase;
- in `kaksi miestä`, `kaksi` has `head = miestä` (the numeral modifies the
  counted noun), and the noun is the phrase head;
- in `Pöydän alla on kissa`, `alla` has `head = on` (the predicate governs the
  adverbial), but the postposition is the head of its own phrase.

If the tree reused one field for both, a verb phrase would swallow its object
and a copular clause would make the predicative a child of the verb. The tree
therefore stores both: `head` for the phrase-internal head, `governor` for the
flat edge. Only `Modifier` and `Auxiliary` edges become parent/child edges;
every other `head` edge stays a clause-level `governor` link.

## 6. The pipeline

```text
tokens
  │
  ├─ morphology::analyze_morphology            (existing)
  ├─ sentence::analyze                         (existing)
  └─ syntax::analyze_syntax                    (existing)
        │
        ▼
  ast::build(tokens, &analysis)
        │
        ├─ 1. seed       Document → Sentence → Word leaves
        ├─ 2. group      flat clauses → Clause nodes (host merge, kinds)
        ├─ 3. chunk      Word leaves → Phrase nodes (head + modifiers/auxiliaries)
        ├─ 4. attach     lists, parentheticals, quotes
        ├─ 5. coordinate same-kind conjuncts (optional, consumer-gated)
        └─ 6. validate   invariants + flat-agreement checks
        │
        ▼
  Ast { nodes, root }
```

| pass | input | output | refuses |
|---|---|---|---|
| `seed` | tokens, `SyntaxAnalysis` | one `Document`, one `Sentence` per span, one `Word` leaf per word token | clauses, phrases, lists |
| `group` | flat `Clause` list, `parent`/`antecedent`, clause roles | one `Clause` node per flat clause, or one multi-segment host in place of the merged pieces; `ClauseKind` | a new clause split; a host merge that fails a condition |
| `chunk` | each tree clause's flat words + `TokenSyntax.head` | phrase nodes with a lexical head, attached under their clause | a phrase over a list, a non-contiguous phrase, a phrase with no head |
| `attach` | `analysis.lists`, bracket pairs, quote depth | `List`, `ListItem`, `Parenthetical`, `Quote` under the smallest containing node | re-parsing a list; guessing an unbalanced bracket |
| `coordinate` | sibling phrases/clauses + `ja`/`tai`/`vai`/`sekä`/`eli` | `Coordination` nodes | a mixed-category coordination; a list (lists already own their coordination) |
| `validate` | tokens, analysis, ast | `Result<(), Vec<Violation>>` | silent acceptance of a broken tree |

The passes are pure functions over the arena. They may share a private
`Builder` that owns the `Vec<Node>` and the token→leaf map; the public API is
still `build` and `validate`.

The order matters for one reason: a merged host is decided **before** any
clause node is emitted, so the tree never contains an orphaned piece of a host.
`group` emits the clause nodes; `chunk` fills them. `attach` runs last because
a list's owner is the smallest phrase that contains it.

`seed` attaches each `Word` leaf to its sentence. `group` reparents it under
the clause that owns it, and `chunk` reparents it again under its phrase.
Reparenting is a field update; the arena is append-only.

## 7. Construction rules

### 7.1 Phrase roots

A token is a **phrase root** when at least one of these holds:

1. it is the flat clause's `predicate`, `subject`, `object` or `complement`;
2. it is a word the clause marks `Adverbial` (its own adverbial phrase);
3. it is the target of at least one `Modifier` edge and is not itself a
   `Modifier` or `Auxiliary` dependent;
4. it is the head of a list item that is not itself a list.

Everything else stays a `Word` leaf: conjunctions, interjections, unknown
tokens, and separators (which are never leaves). A `Modifier` or `Auxiliary`
dependent is always attached to its governor; if the governor is outside the
clause, the dependent stays a leaf and its `head` edge remains a `governor`
link.

A phrase root owns:

- every `Modifier` dependent whose `head` points at it, recursively;
- every `Auxiliary` whose `head` points at the predicate, for a verb phrase.

A phrase root does **not** own its `governor`. That is the whole point of the
`head`/`governor` split: `kaunis` is governed by `on` but headed by itself.

If the root has no dependents and does not fill a clause slot, no phrase is
created; the word stays a leaf. A one-word phrase is allowed only when it fills
a slot, is a list item head, or is a coordination conjunct, because a clause
tree should have constituents at every argument position and every list item.

### 7.2 Phrase kind

| cue on the head / phrase | kind |
|---|---|
| nominal POS (noun, unknown nominal) | `Noun` |
| pronoun | `Pronoun` |
| adjective | `Adjective` |
| numeral head, or a nominal head with a numeral dependent | `Numeral` |
| adverb | `Adverb` |
| postposition | `Postposition` |
| preposition | `Preposition` |
| finite verb | `Verb` |
| verb with `Mood::Infinitive` | `Infinitive` |
| verb with a participle reading | `Participle` |

If the head has no nominal, verbal, adposition or adverb POS, no phrase is
created. This is the same conservatism as `nominal_pos` and `is_finite`: a
closed table beats a guess.

### 7.3 Contiguity

A phrase is built only when its root and every recursively attached dependent
form one contiguous token run, ignoring separators. If a dependent is separated
from its head by a relative clause, a parenthetical or another clause, the
phrase is **not** built; its words stay leaves or become separate phrases, and
the flat `head` edge remains available as `governor`.

This keeps the tree a tree. Non-contiguous constituents are handled only by
the host-segment rule in §7.6, which is a clause-level exception, not a
phrase-level one.

### 7.4 Clause grouping and children

`group` decides the tree's clause nodes before `chunk` fills them. For a flat
clause that is not merged into a host, one `Clause` node is emitted with the
flat clause's `tokens` and `span`, and its `clause` field is that flat index.
For a merged host, one `Clause` node replaces the two pieces (§7.6).

After `chunk`, a `Clause` node's children are, in source order:

- the predicate phrase, when `Clause.predicate` is `Some`;
- the subject, object and complement phrases;
- every adverbial phrase;
- every `List` or `Parenthetical`/`Quote` attached to the clause;
- every relative clause node whose `parent` is this clause;
- any remaining `Word` leaves.

`ClauseKind`:

| condition | kind |
|---|---|
| `clause.parent.is_some()` | `Relative` |
| first word is a subordinator (`is_subordinator`) and not relative | `Subordinate` |
| predicate has only non-finite readings (`Mood::Infinitive`) | `Infinitive` |
| a conjunct of a §7.7 coordination | `Coordination` |
| otherwise | `Main` |

The flat `ClauseType` already says `Subordinate` for a relative clause; the
tree refines that into `Relative` vs `Subordinate`. It does not contradict the
feature, it names the structural subtype.

`Node::confidence` for a clause is `Likely` when it has a predicate and
`Weak` when it is a fragment. `Sentence` and `Document` are `Likely`. A
`List` inherits `Likely` for `Nominal` and `Weak` for `Mixed`. A `Phrase` and a
`Word` copy the flat `RoleConfidence` of their head. A `ListItem`,
`Coordination`, `Parenthetical` or `Quote` is `Likely` when the cue is
explicit (a list item boundary, a conjunction, a balanced bracket, a quote
delimiter) and `Weak` when the builder had to infer adjacency. These are the
same two levels the rest of the model uses; the tree introduces no third
scale.

### 7.5 Lists

`syntax_list::parse` is the only list parser. The tree mirrors its arena, it
does not re-scan commas.

- one `List { index }` node per `analysis.lists[index]`;
- `span`, `tokens`, `head`, `role` and `clause` copied from the list;
- one `ListItem { list, item }` child per item, in item order;
- the item's children are the top-level phrase/word/clause nodes whose token
  ranges fall inside `item.tokens` and whose parent is not also inside the item;
- `ListItemKind::List(indices)` makes the child `List` nodes the item's
  children, preserving the post-order arena;
- separators stay out of the tree; the list's `separators` field is still the
  punctuation source.

**Ownership rule.** A list attaches to the smallest node whose token range
contains the whole list:

- if the list is exactly a clause slot (`list.role` names the slot and the
  list's token range equals the slot's phrase range), the `List` is a direct
  child of the clause with that role, and no phrase is built around it;
- if the list sits inside a phrase (`punainen, sininen ja vihreä talo`), the
  `List` is a child of the noun phrase, and the phrase head stays `talo`;
- if no containing node exists, the list is a child of its clause.

The `ListItem` nodes own the item phrases. The tree never has both a
slot-spanning phrase and a list for the same range.

### 7.6 Relative clauses and host segments

The flat model builds a relative tree on `Clause` (`parent`, `children`,
`antecedent`) but keeps the host in contiguous pieces. `group` may merge the
pieces into one host node with **segments** when all of these hold:

1. relative clause `R` has `R.parent == Some(A)`, `A` is top-level, `A`'s
   only child is `R`, and `R` has no children of its own;
2. the next flat clause `B` is in the same sentence as `A` and is the **last**
   clause of that sentence (a coordinated continuation is left flat);
3. `B` has no subject of its own, or `share_clause_roles` made it share `A`'s
   subject; when `A` has a subject and `B` does not share it, `B`'s predicate
   must still fit that subject in person and number;
4. `R` lies between `A` and `B` in token order;
5. `R.antecedent` is in `A` or `B` (or `None` when the relative word is
   unambiguous).

Then the host is one `Clause(kind)` with:

- `segments = [A.tokens, B.tokens]`, `tokens = A.tokens.start..B.tokens.end`,
  `span = A.start..B.end`;
- `kind` is `Subordinate` when `A` opens with a subordinator, `Infinitive`
  when either segment's predicate is non-finite, and `Main` otherwise;
- `head` is `B.predicate`, and `clause` is `B` (the flat clause that holds the
  host predicate);
- children from `A`, the relative `Clause(Relative)`, then children from `B`;
- the relative's `parent` is the host, `antecedent` copied to `Node.antecedent`.

`Mies, joka tuli, söi.` becomes one host clause with segments `[Mies]` and
`[söi]`, containing the relative clause between them. If any condition fails,
the tree keeps the flat three-clause shape. A split span is never guessed.

### 7.7 Coordination

Coordination is the one optional pass. It starts after lists, because a
nominal list already owns its `ja`/`tai` coordination and must not be wrapped
twice.

- Find a conjunction token (`ja`, `tai`, `vai`, `sekä`, `eli`) that is not a
  list separator.
- Take the nearest complete phrase or clause on each side in the same clause.
- If both have the same `PhraseKind` or are both `Clause`, create a
  `Coordination(kind)` whose children are the conjuncts and whose `head` is
  the conjunction token.
- If the conjuncts differ, create `Coordination(Mixed)` only when a consumer
  asks for it; otherwise leave the conjunction as a `Word` leaf.
- For clause coordination with a shared subject, the shared subject is a child
  of the `Coordination` node and the conjunct clauses omit it. This is allowed
  only when the flat `share_clause_roles` pass marked the sharing; otherwise
  no coordination node is built.

Until a consumer needs coordination, the pass is off. The enum variant stays
so adding it later is not a breaking match change.

## 8. Invariants

`ast::validate(tokens, analysis, &ast)` checks all of these. The builder runs
it under `debug_assert!`; tests and the CLI run it explicitly.

1. **Root.** `root` points at a `Document`, it has no parent, and it is the
   only node with no parent.
2. **Parent/child reciprocity.** Every child id is listed exactly once by its
   parent, and the child's `parent` points back. No child appears under two
   parents.
3. **Word coverage.** Every word token appears in exactly one `Word` leaf, and
   every leaf covers exactly one word token. Separators appear in no leaf.
4. **Segment shape.** `segments` are non-empty, sorted, disjoint and inside
   `tokens`; `tokens` is the envelope; `span` contains every token in
   `tokens`.
5. **Partition.** A node's `segments` plus the token ranges of its interrupting
   children partition `tokens`. “Interrupting” means a child not contained in
   any segment; in v1 this is only a relative clause inside a host.
6. **Clause agreement.** Every flat `Clause` belongs to exactly one tree
   `Clause` node: either that node's own `tokens`/`clause`, or a segment of a
   multi-segment host. Every `Clause` node points back to an existing flat
   clause, and a host's `segments` are exactly the token ranges of the flat
   clauses it merged.
7. **List agreement.** Every `analysis.lists[i]` has exactly one
   `List { index: i }` node with the same `tokens` and `span`; every item has
   one `ListItem { list: i, item: j }`; nested list indices resolve.
8. **Role agreement.** For a `Word` or `Phrase`, `role` and `confidence` equal
   the flat `TokenSyntax` values of `head`. For a `List`, they come from the
   flat `syntax_list::List`. Every other kind has `role = None`. The tree never
   invents a role.
9. **Phrase heads.** A `Phrase` has a head; its children are the attached
   modifiers/auxiliaries and their sub-phrases, plus any `List` or
   `Parenthetical` that belongs inside the phrase. No clause-level argument is
   a child of a phrase. A one-word phrase is legal only when it fills a clause
   slot, is a list item head, or is a coordination conjunct.
10. **Acyclicity.** Following `parent` from any node reaches the root; no
    cycle exists.

A violation is a bug in the builder, not a diagnostic. `validate` returns all
violations so a test can print them at once.

## 9. Integration

### 9.1 Module layout

```text
src/syntax/ast/
  mod.rs           Ast, NodeId, Node, NodeKind, ClauseKind, PhraseKind,
                   CoordKind, public build/validate
  seed.rs          Document/Sentence/Word skeleton
  phrases.rs       phrase roots, head resolution, kind, contiguity
  clauses.rs       clause grouping, ClauseKind, host segments
  attach.rs        lists, parentheticals, quotes (phase 3)
  coordination.rs  the optional same-kind pass (phase 4)
  validate.rs      the invariants in §8
```

`src/syntax/mod.rs` gains `pub mod ast;` and re-exports `Ast`, `Node`,
`NodeId`, `NodeKind`, `ClauseKind`, `PhraseKind`, `CoordKind`, `build` and
`validate` at `syntax::ast::*`. No existing public item moves.

### 9.2 API

```rust
pub fn build(tokens: &[Token], analysis: &SyntaxAnalysis) -> Ast;
pub fn validate(tokens: &[Token], analysis: &SyntaxAnalysis, ast: &Ast)
    -> Result<(), Vec<Violation>>;

pub struct Violation {
    pub node: Option<NodeId>,
    pub rule: &'static str,
    pub detail: String,
}

/// Convenience for tools and wasm: analysis plus tree from one morphology pass.
pub fn analyze(tokens: &[Token]) -> SyntaxTree;

pub struct SyntaxTree {
    pub analysis: SyntaxAnalysis,
    pub ast: Ast,
}
```

`build` is pure and does not allocate a morphology cache. `analyze` reuses
`syntax::analyze`'s cache path and then calls `build`; it is the only function
that pays for both.

`attach` needs the quote-depth sidecar for `Quote` nodes, so `build` calls
`sentence::analyze(tokens)` once. That is a second sentence pass, but it is
cheap and on-demand; a later phase can thread the contexts through
`syntax::analyze` without changing the public tree shape.

### 9.3 Checker

- Phases 0–5 leave `syntax_check` untouched. `check_analysis` does not build
  the tree.
- A rule that needs the tree calls `ast::build` once in `check_analysis` and
  receives `&Ast`; the first such rule gets a `check_analysis_with_tree`
  variant rather than changing the existing signature.
- The first candidates are:
  - **word-order features**: a fronted object/topic is the phrase node before
    the predicate, not a token scan;
  - **noun-phrase agreement**: adjective/numeral agreement inside a `Phrase`,
    which the flat token roles cannot express;
  - **list items**: an item that is a `Numeral` phrase or a `Noun` phrase,
    instead of a token range with a `Phrase` label.

### 9.4 CLI and wasm

- `tools/src/bin/syntax_tree.rs`:
  - `syntax_tree "text"` prints the indented tree;
  - `--json` prints a stable node array;
  - `--check` runs `validate` and exits non-zero on a violation.
- `torille-wasm` gains an additive `analyze_ast(text)` and
  `analyze_ast_with_state(idx, text)` in phase 5, with the same node fields as
  the Rust struct. No existing export changes.
- The demo/extension UI is a later consumer; the CLI is the first debugger.

### 9.5 Textual dump

The CLI and the golden fixtures share one format:

```text
Document[0..21]
  Sentence[0..21]
    Clause(main)[0..21] tokens=0..7 segs=0..1,6..7 head=söi
      Word[0..4] "Mies"
      Clause(relative)[6..15] parent=host antecedent=Mies
        Word[6..10] "joka"
        Phrase(verb)[11..15] head=tuli role=predicate
          Word[11..15] "tuli"
      Phrase(verb)[17..21] head=söi role=predicate
        Word[17..21] "söi"
```

Byte spans are omitted from the stable golden file when they would make the
fixture noisy; the fixture stores the token surfaces and the structural fields
(`kind`, `head`, `role`, `segments`, `parent`) and the test compares those.

## 10. Phases and gates

Each phase is a separate commit. A phase that adds only structure must produce
zero diagnostic diff.

| phase | deliverable | gate |
|---|---|---|
| 0. Skeleton | types, `seed`, `group` (one Clause per flat clause, no host merge), `validate`, CLI dump | **landed**: `Ast` builds for every text in `tests/fixtures/syntax_cases.json`; all invariants hold; `cargo test --release --workspace` green; no diagnostic change |
| 1. Phrases | phrase roots, kinds, contiguity, `head`/`governor` | **landed**: phrase tests for NP/AP/NumP/PP/VP; every `Modifier`/`Auxiliary` flat edge appears as a tree edge; multi-word object and `Kaksi miestä` covered |
| 2. Clause grouping | clause kinds, predicate phrase, slot children, host merge | **landed**: `Mies, joka tuli, söi` keeps three flat clauses but the tree has one host with two segments and the relative nested between them; `Talo on kaunis` has a separate AdjectivePhrase complement |
| 3. Attach | lists, list items, parentheticals, quotes | list and nested-list fixtures; a clause-slot list replaces the slot phrase; unbalanced brackets leave the flat shape |
| 4. Coordination (optional) | same-kind phrase/clause coordination | `Sami söi puuron ja joi maidon`; mixed coordination stays flat; `word_order_allow` unchanged |
| 5. Consumers | one checker rule or the word-order feature reads the tree; wasm export | the new rule's fixture fails on the flat path and passes on the tree; every existing diagnostic is byte-identical |
| 6. Cleanup | migrate ad hoc helpers only where the tree is equivalent | no helper is deleted until its last caller moves; the flat API stays |

Phase 0 must name its first consumer. If nobody reads the tree, do not build
it; the current workarounds are documented and the flat model is correct.

**Phase 0 landing.** `src/syntax/ast/` now has `mod.rs`, `seed.rs`,
`clauses.rs` and `validate.rs`. `syntax_tree` prints the skeleton, writes
`--json` and runs `--check`; `tests/syntax_tree.rs` validates all 122 fixture
sentences. Relative clauses are typed as `Clause(Relative)` but stay direct
children of their sentence until phase 2's host merge, because the flat host
range does not contain the relative yet.

**Phase 1 landing.** `phrases.rs` builds a contiguous phrase per phrase root:
clause slots and adverbials become one-word phrases, `Modifier`/`Auxiliary`
edges become phrase children, an adposition's complement is internal, and a
numeral head (or a numeral dependent) makes a `Numeral` phrase. Phrases that
would cross a relative clause, a parenthetical or a list are refused; their
words stay leaves and keep the flat `governor`. The phrase tests cover NP, VP,
AP, NumP, postposition and preposition phrases, a modifier chain and a fronted
object.

**Phase 2 landing.** `clauses.rs` merges a relative host with its continuation
when the flat model links them and the continuation is the last clause of the
sentence, so `Mies, joka tuli, söi` is one `Clause(main)` with segments
`[Mies]` and `[söi]`, the relative nested between them and its `antecedent`
copied to the node. If there is no continuation, the continuation is
coordinated, or any agreement/antecedent condition fails, the tree keeps the
flat three-clause shape. The validator now checks a flat clause by
segment-membership rather than by a one-to-one node.

## 11. Tests

### 11.1 Unit tests

In `src/syntax/ast/`, beside the code:

- `seed`: sentence and leaf counts, leaf coverage, separator exclusion;
- `phrases`: one test per kind, one for the head/governor split, one for
  contiguity refusal;
- `clauses`: main/relative/subordinate/infinitive, predicate phrase, copular
  complement, existential adverbial, host merge;
- `attach`: list ownership, nested list, unbalanced bracket;
- `coordination`: same-kind, mixed, shared subject;
- `validate`: one test per invariant with a deliberately broken arena.

### 11.2 Corpus invariants

`tests/syntax_tree.rs`:

- load `tests/fixtures/syntax_cases.json`,
  `tests/fixtures/syntax_ambiguity.json` and
  `tests/fixtures/word_order_cases.json`;
- for every text, `ast::analyze` then `validate`; a violation names the case
  id and the node id;
- assert that `analysis.clauses.len()` and `analysis.lists.len()` are
  unchanged from the flat pass.

This is the data-driven regression net the repository already uses. A new
construction goes into the fixtures, not into a hand-written list of strings.

### 11.3 Golden trees

`tests/fixtures/syntax_tree.json` holds the compact dump from §9.5 for ~20
sentences covering:

- ordinary transitive and intransitive;
- existential, copular, passive, possessive;
- yes/no, wh and imperative;
- relative host, coordinated clauses, coordinated phrases;
- nominal list, numeral list, nested list;
- postposition phrase, genitive modifier chain;
- parenthetical and quote;
- an `Unknown` word that stays a leaf.

The test compares the structural fields, not byte offsets, so an unrelated
tokeniser change does not churn the fixture.

### 11.4 Baseline and performance

- `cargo test --release --workspace` plus the existing syntax fixtures are the
  no-change gate.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- A `syntax_tree` group in `benches/torille.rs` (or a new bench) measures
  build time on 100 and 1,000 tokens. Target: under 1 ms and 5 ms in release,
  and no measurable change to `check_syntax` because the checker does not call
  it.
- `cargo check -p torille-wasm --target wasm32-unknown-unknown` once the wasm
  export lands.

## 12. Risks and open questions

| risk | mitigation |
|---|---|
| The tree duplicates the flat model and drifts | `validate` checks clause/list/role agreement; the flat model stays the only authority |
| Non-contiguous constituents become a parser project | v1 is contiguous-only; host segments are the single, conservative exception |
| Phrase boundaries look arbitrary to a user | every phrase needs a flat `head` edge or a clause slot; no phrase from adjacency alone |
| Public `NodeKind` grows | additive variants within 0.x; consumers match with a `_` arm; document the stability rule |
| Coordination needs shared-subject ownership | defer until a consumer exists; `Mixed` keeps the tree honest |
| The CLI dump becomes a second contract | the golden test compares fields, not the pretty-printed text; the text is for debugging |
| The tree tempts a rule rewrite | phase 5 is one rule at a time, each with a failing fixture on the old path |
| Multi-segment spans confuse highlighting | `span` is the envelope; `segments` is the highlight unit; document this on `Node` |

Open questions:

1. **First consumer.** Word-order features, NP agreement, or the UI? The answer
   decides whether phase 1 needs `Adverb`/`Postposition` phrases or only
   nominal and verbal ones. Recommendation: word-order features first, because
   the spec is already written and the flat model's gap is largest there.
2. **Clause coordination.** Is a shared-subject `Coordination` worth the
   complexity, or is the flat clause pair enough? Recommendation: defer; lists
   already cover the common nominal case, and the checker's clause rules read
   the flat pair correctly.
3. **Naming.** `syntax::ast` vs `syntax::tree`. Recommendation: `ast` in the
   public path (the user-facing name), “surface tree” in prose.
4. **`Node::span` for a split host.** Is the envelope (`Mies` through `söi`)
   the right byte span, or should the host expose only `segments`? Recommendation:
   both; `span` is for “where is this clause”, `segments` is for “what does it
   own”.
5. **Should `SyntaxAnalysis` own the tree?** Recommendation: no. It would
   change `PartialEq`, the wasm shape and every caller; `SyntaxTree` is the
   opt-in wrapper.

## Appendix A — worked trees

`Sami söi aamupuuronsa.`

```text
Clause(main)
  Phrase(noun, head=Sami, role=subject)
    Word Sami
  Phrase(verb, head=söi, role=predicate)
    Word söi
  Phrase(noun, head=aamupuuronsa, role=object, governor=söi)
    Word aamupuuronsa
```

`Pöydällä on kirja.`

```text
Clause(main, features.construction=existential)
  Phrase(noun, head=pöydällä, role=adverbial, governor=on)
    Word Pöydällä
  Phrase(verb, head=on, role=predicate)
    Word on
  Phrase(noun, head=kirja, role=subject, governor=on)
    Word kirja
```

A true postposition phrase is `Pöydän alla on kissa.`: `alla` is the phrase
head and the postposition, `Pöydän` is its genitive modifier, and the whole
phrase is the existential's adverbial.

`Talo on kaunis.` — the head/governor split:

```text
Clause(main, features.construction=copular)
  Phrase(noun, head=Talo, role=subject, governor=on)
  Phrase(verb, head=on, role=predicate)
  Phrase(adjective, head=kaunis, role=complement, governor=on)
```

`Mies, joka tuli, söi.` — a host with two segments:

```text
Clause(main) segments=[Mies] [söi]
  Word Mies        # the flat model leaves this one Unknown
  Clause(relative) parent=host antecedent=Mies
    Word joka
    Phrase(verb, head=tuli, role=predicate)
  Phrase(verb, head=söi, role=predicate)
```

`Ostin omenan, banaanin ja kaksi päärynää.` — a list owns its items:

```text
Clause(main)
  Phrase(verb, head=Ostin, role=predicate)
  List(nominal, role=object)
    ListItem
      Phrase(noun, head=omenan)
    ListItem
      Phrase(noun, head=banaanin)
    ListItem
      Phrase(numeral, head=päärynää)
        Word kaksi        # modifier, head=päärynää
```

`Kirjan luin eilen.` — a fronted object is a constituent:

```text
Clause(main)
  Phrase(noun, head=Kirjan, role=object, governor=luin)
  Phrase(verb, head=luin, role=predicate)
  Phrase(adverb, head=eilen, role=adverbial, governor=luin)
```

## Appendix B — relationship to the existing plans

- [`PLAN_SYNTAX.md`](PLAN_SYNTAX.md) built the flat role layer. This plan does
  not change a role; it groups the roles into constituents.
- [`PLAN_SYNTAX_REFACTOR.md`](PLAN_SYNTAX_REFACTOR.md) split the modules and
  added the relative-clause tree on `Clause`. The AST consumes that tree and
  adds phrases on top.
- [`PLAN_SYNTAX_CHECKER.md`](PLAN_SYNTAX_CHECKER.md) owns the diagnostics and
  the list arena. The AST mirrors the arena; it does not replace the rules.
- [`../docs/word_order.md`](../docs/word_order.md) is the first intended
  consumer: its “constituent” language becomes a node, and `Clause::features`
  remains the clause-level feature source.
