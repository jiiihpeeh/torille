# PLAN_SYNTAX_AST_REST — the remaining surface-tree work

Status: **Phase 5b landed; the rest is proposal.** Phases 0–5a plus the wasm
`analyze_ast` export are in place. This document details the remaining 3b, 4,
5c and 6 work. The base design is in
[`PLAN_SYNTAX_AST.md`](PLAN_SYNTAX_AST.md).

This document covers what is left:

| phase | work | recommendation |
|---|---|---|
| 3b | nested, mixed, clause and predicate lists | do only when a consumer needs it |
| 4 | coordination nodes | **defer**; the list arena already covers most of it |
| 5b | wasm `analyze_ast` export | **landed** |
| 5c.1 | AST-backed attribute agreement | parity audit first, then migrate |
| 5c.2 | word-order diagnostics | **defer**; suggestion-only, no safe rule yet |
| 6 | API cleanup and helper migration | last, after the consumers settle |

The order in the table is the recommended order. Every phase is a separate
commit and the checker output must stay byte-identical until a phase explicitly
adds a diagnostic.

## 1. Current baseline

`src/syntax/ast/` now has:

```text
mod.rs        Ast, NodeId, Node, NodeKind, ClauseKind, PhraseKind,
              CoordKind, Constituent, build, analyze, validate
seed.rs       Document, logical Sentence, Word leaves
phrases.rs    one contiguous Phrase per phrase root
clauses.rs    one Clause per flat clause; relative host merge
lists.rs      top-level nominal List/ListItem nodes
attach.rs     Parenthetical and Quote containers
validate.rs   the invariants
```

The current limitations are deliberate:

- lists attach only when they are top-level `Nominal` lists with `Word` or
  `Phrase` items; nested, mixed, clause and predicate lists stay flat;
- a list's conjunction is a `Word` child of the `List`, not a `ListItem`;
- quotes are split at a sentence boundary; nested quote levels are one run;
- there is no `Coordination` node;
- the wasm surface has no AST export;
- the checker still reads the flat analysis only;
- `Ast::clause_constituents` is a view over the flat clause index; a host is
  seen segment by segment, not as one order unit.

The base plan’s invariants and tests are the contract:
`cargo test --release --workspace`, the 122-fixture AST corpus test, the 23
unit tests and `syntax_tree --check` are green.

## 2. Decision summary

| question | decision |
|---|---|
| nested lists | phase 3b; do not start without a consumer |
| mixed lists | keep flat permanently unless a consumer names a safe shape |
| clause/predicate lists | attempt only after 3b’s nested-list machinery exists |
| coordination | deferred; list arena is the first representation |
| wasm | additive `analyze_ast` / `analyze_ast_with_state` |
| checker | parity audit for attribute agreement before any migration |
| word-order diagnostics | no runtime rule now; the constituent view is the API |
| public enums | consider `#[non_exhaustive]` when the wasm shape lands |
| AST caching | no cache until a measured consumer needs one |

## 3. Phase 3b — nested and non-nominal lists

### 3.1 Goal

Represent the list arena faithfully, not just its top-level nominal subset:

```text
Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.
                                      ^ nested list
```

and, if a consumer needs it, clause/predicate lists such as
`Sami söi, joi ja nukkui`.

### 3.2 Data already available

`syntax_list` gives:

- `List.parent: Option<usize>`, with the child list at a **smaller** index than
  its parent (post-order);
- `ListItemKind::List(Vec<usize>)` naming the child list indices;
- `ListItemKind::Clause` for an item that the parser recognised as a clause;
- `ListItem.scan` including trailing separators/brackets.

No new list parser is needed. The AST mirrors this arena.

### 3.3 Attachability

Replace the current `eligible(list)` with a recursive rule:

```text
attachable(L):
  L.parent is None or the parent list is attachable
  L.kind == Nominal
  every item is one of:
    Word or Phrase           (current)
    List(indices)            (all children attachable)
    Clause                   (only in phase 3b.2)
  every item has a candidate child plan
  the list is covered by items + conjunction
```

Two policies to decide before coding:

1. **Parent failure.** If a parent list is not attachable, do not attach its
   child lists either. A nested child floating under the clause without its
   parent is worse than leaving the whole subtree flat.
2. **Mixed lists.** Keep `ListKind::Mixed` flat. Its items can be clause
   fragments; attaching a mixed list over the predicate would make the tree
   lie about what the clause is. The arena remains available to a diagnostic
   consumer.

### 3.4 Algorithm

1. Compute `attachable` bottom-up over the post-order arena: a child list is
   considered before its parent.
2. Create `List`/`ListItem` shells only for the attachable lists, or create
   them lazily in index order and discard a whole subtree if a parent fails.
3. Attach in this order:
   - a top-level list to the smallest containing `Clause` or `Phrase`;
   - a nested list to its parent `ListItem`;
   - clause/predicate items to a `Clause` node whose token range equals the
     item, or to the top-level nodes inside the item when no exact clause node
     exists.
4. Item children:
   - `ListItemKind::List(indices)`: the child `List` nodes, no generic
     candidates;
   - `Word`/`Phrase`: the current top-level child selection;
   - `Clause`: prefer the exact `Clause` node, then the current top-level
     selection; if a clause spans more than the item, skip the list subtree.
5. Coverage:
   - item words must be covered by the item’s children;
   - a parent item whose child is a list is covered by that child list;
   - the root list ignores `separators` and `introducer`, but **not** the
     conjunction, which is a `Word` child.
6. Sort children and run `validate`.

### 3.5 Edge cases

- A nested list inside a parenthetical: lists run before `attach.rs`; the
  parenthetical later reparents the top-level list node. The nested ownership
  must be settled before the wrapper pass.
- A nested list inside a quote: the quote pass must see the parent list node
  as an ordinary child.
- A list whose parent is a `ListItemKind::Clause`: if the item’s clause node is
  a host segment, use the segment range, not the host envelope.
- Two lists with overlapping token ranges: skip both; the flat arena stays.
- A list whose conjunction leaf is missing or already inside a phrase: skip the
  list subtree; do not orphan a `List` node.
- A list with a colon introducer and a conjunction: the introducer is skipped
  by coverage, the conjunction is a child.

### 3.6 Validation changes

- Every attached `List` node has one `ListItem` per flat item and one optional
  conjunction `Word` child.
- A `ListItemKind::List` item has child `List` nodes and no generic children.
- Nested list indices resolve and child token ranges are inside the item.
- Coverage of a parent item includes the child list’s descendants.
- No orphaned list subtree: every `List` node’s parent chain reaches the root.
- Ineligible lists are not required to have nodes; if a node appears for one,
  it is a violation (unlike phase 3a’s “at most one”).

### 3.7 Tests and gate

- the bracket + nested-list fixture;
- a colon list containing a nested list;
- a parent list with a failing child: the whole subtree stays flat;
- mixed list: no node;
- clause/predicate list fixtures once 3b.2 is enabled;
- `syntax_tree --check` over the whole fixture corpus;
- `cargo test --release --workspace`, no diagnostic diff.

Gate to start 3b.2 (clause/predicate lists): a named consumer. Without one,
3b.1 (nested nominal lists) is enough.

## 4. Phase 4 — coordination

Status: **deferred.** Do not implement until a rule or a UI asks for it.

### 4.1 Why defer

`src/syntax_list.rs` already represents most coordination:

- nominal lists own `ja`/`tai`/`sekä`;
- predicate and clause lists exist in the arena;
- the conjunction is a list child in the AST for attached nominal lists.

A separate `Coordination` node for the same span would duplicate the list
arena and create two owners for one conjunction. Clause coordination with a
shared subject is also already represented by the flat clause pair plus
`share_clause_roles`.

### 4.2 Trigger

Start phase 4 only when one of these is true:

- a checker rule needs one node for a coordinated subject/object that is not a
  list;
- the UI needs a single “coordination” highlight that is not a list;
- a word-order rule needs the coordinated constituent as one unit.

### 4.3 Design sketch (if triggered)

- Find a conjunction token (`ja`, `tai`, `vai`, `sekä`, `eli`) that is **not**
  in `analysis.lists`.
- Take the nearest complete sibling phrase/clause on each side under one
  parent.
- Same kind only: `Coordination(CoordKind)` with the conjuncts as children and
  the conjunction as `head`.
- Mixed kinds: no node unless a consumer explicitly wants `Mixed`.
- Clause coordination with a shared subject: the shared subject is a child of
  the `Coordination` node and the conjunct clauses omit it; require the flat
  `share_clause_roles` link.
- `span`/`tokens` envelope from the first to last conjunct; `segments` one
  range.
- Role is `None`; the parent clause keeps its own slot pointers.
- Validate: source-order children, one conjunction token as `head`, no overlap
  with a `List`, no double-owned conjunct.
- Tests: `Sami söi puuron ja joi maidon`, coordinated objects/adjectives not in
  a list, mixed stays flat; `word_order_allow` unchanged.

## 5. Phase 5b — wasm `analyze_ast`

### 5.1 Goal

Make the tree available to the demo and the extensions without changing any
existing export:

```rust
#[wasm_bindgen]
pub fn analyze_ast(text: &str) -> js_sys::Object;

#[wasm_bindgen]
pub fn analyze_ast_with_state(idx: u32, text: &str) -> js_sys::Object;
```

### 5.2 Shape

Keep the existing `analyze_syntax` object as a `syntax` field and add an `ast`
field, so a consumer can migrate one part at a time:

```json
{
  "syntax": { "sentences": [], "clauses": [], "lists": [], "tokens": [] },
  "ast": {
    "root": 0,
    "nodes": [
      {
        "id": 0,
        "kind": "document",
        "detail": null,
        "span": [0, 21],
        "tokens": [0, 7],
        "segments": [[0, 7]],
        "parent": null,
        "children": [1],
        "head": null,
        "governor": null,
        "role": null,
        "confidence": "likely",
        "antecedent": null,
        "clause": null
      }
    ],
    "clauses": [
      {
        "index": 0,
        "constituents": [
          {
            "node": 6,
            "kind": "phrase",
            "role": "subject",
            "tokens": [0, 1],
            "span": [0, 4]
          }
        ],
        "first": "subject",
        "fronted": null
      }
    ]
  }
}
```

`detail` is the clause kind, phrase kind, coordination kind, list index or
`list.item` string; `null` for a word/document/sentence/container. `role` and
`confidence` use the existing names. `segments` is an array of `[start, end]`
pairs. `clauses` is keyed by the **flat** clause index, using
`Ast::clause_constituents`.

### 5.3 Implementation notes

- Factor the existing `syntax_object` so `analyze_ast` can call it and then
  attach `ast`; do not duplicate the 100-line sentence/clause/token/list
  writer.
- `analyze_ast` runs `syntax::analyze` then `ast::build`.
- `analyze_ast_with_state` reuses `state.syntax_cache` and the same
  disambiguation sidecar as `analyze_syntax_with_state`.
- No new state field and no AST cache in phase 5b. Measure first.
- The export is on-demand; it must not enter the keystroke path.

### 5.4 UI

Optional debug consumer:

- a small `torille-wasm/demo/ast.html` page that calls `analyze_ast` and
  renders the indented tree plus the `--order` sequence;
- no change to the production demo/extension panel until a product decision;
- the page is a developer tool, not a shipped feature.

### 5.5 Tests and gate

- `cargo check -p torille-wasm --target wasm32-unknown-unknown`;
- `cargo clippy -p torille-wasm --target wasm32-unknown-unknown -- -D warnings`;
- `wasm-pack build --release --target web` (or the existing demo build
  script);
- `torille-wasm/tests/ast-shape.mjs` with
  `node --experimental-wasm-modules tests/ast-shape.mjs` from
  `torille-wasm/` after a wasm-pack build:
  it asserts the `{ syntax, ast }` envelope, the node fields and the
  `object / predicate / adverbial` constituent order for
  `Kirjan luin eilen.`, then repeats through `analyze_ast_with_state`;
- no change to `analyze_syntax`, `check_syntax`, `check_style` or their
  existing consumers.

**Phase 5b landing.** `analyze_ast` and `analyze_ast_with_state` are in
`torille-wasm/src/lib.rs`. The object is `{ syntax, ast }`; `ast.nodes` is a
pre-order node array with `id`, `kind`, `detail`, `span`, `tokens`, `segments`,
`parent`, `children`, `head`, `governor`, `role`, `confidence`, `antecedent`
and `clause`; `ast.clauses` carries the flat clause index, its constituents,
`first` and `fronted`. The state path reuses `syntax_cache` and the
disambiguation sidecar. `wasm-pack build --release` and
`tests/ast-shape.mjs` (stateless and stateful, `Kirjan luin eilen.`) pass; no
existing export changed.

## 6. Phase 5c — checker consumers

### 6.1 Attribute agreement (5c.1)

`src/syntax_check/agreement.rs` already has a careful attribute rule. It finds
the head through the flat `TokenSyntax.head` first and a forward clause scan
second. The AST has the same relation as a `Phrase` head plus `Modifier`
children.

Do this as a **parity refactor**, not a new rule:

1. Add an internal `attribute_pairs` helper:
   ```rust
   struct AttributePair { modifier: usize, head: usize, phrase: NodeId }
   fn phrase_attribute_pairs(ast: &Ast, analysis: &SyntaxAnalysis) -> Vec<AttributePair>
   ```
   For every `Phrase`, take its head and its `Modifier` children whose POS is
   adjective, ordinal or demonstrative. Keep the existing exclusions
   (relative pronoun, invariant quantifier, cardinal phrase, quantity head).
2. Add a test-only audit over the syntax fixtures and prose corpus. For every
   modifier token that `attribute_head` finds, assert the phrase pair has the
   same head, and for every phrase pair assert the flat rule agrees or the
   token is in an explicit exception list. The exception list is the design
   record; it must be small and reasoned.
3. Only if the audit is clean, add:
   ```rust
   pub fn check_analysis_with_tree(
       tokens: &[Token],
       analysis: &SyntaxAnalysis,
       ast: &Ast,
   ) -> Vec<SyntaxDiagnostic>;
   ```
   `check_attributes` takes an optional `&Ast`; when the token is inside a
   `Phrase` with an unambiguous head, that pair wins; otherwise the flat
   `attribute_head` fallback stays.
4. `check_analysis` may build the AST once:
   ```rust
   let ast = ast::build(tokens, analysis);
   check_analysis_with_tree(tokens, analysis, &ast)
   ```
   This is a deliberate cost on the on-demand grammar path. Measure it before
   landing; if it is too large, build the AST only when a clause contains a
   candidate attribute. Do not put it in the keystroke path.
5. Keep `DiagnosticKind::AttributeAgreement`, severity, corrections and JSON
   exactly as they are.

Gate:

- every fixture and prose diagnostic is byte-identical;
- the parity audit passes;
- `perf_grammar_style` records the new AST cost, with a documented budget
  (target: no more than 5% on the grammar path; the tree is on-demand);
- `cargo test --release --workspace` and the Python gates that read syntax
  diagnostics.

### 6.2 Word-order diagnostics (5c.2)

Do **not** add a runtime `WordOrder` rule yet.

`docs/word_order.md` is explicit: SVO is a `PREFER`, never an error; object,
topic, temporal and locative fronting, contrastive focus, passive and
existential orders are `ALLOW`; echo questions and contrastive questions are
allowed. A rule without a reliable cue would be a false-positive generator.

The consumer that lands now is the API, not a diagnostic:

- `Ast::clause_constituents`, `first_constituent`, `fronted_constituent`;
- `syntax_tree --order`;
- the wasm AST export.

If a rule is later started:

- one rule at a time, suggestion-only (`Severity::Suggestion`), never
  `Error`/`Warning`, no correction;
- a `PREFER` fixture and an `ALLOW` counterpart in
  `tests/fixtures/word_order_cases.json`;
- `word_order_allow.rs` must stay green;
- no statistical model and no new dependency.

### 6.3 List and UI consumers (5c.3)

- The checker’s list diagnostics already read `syntax_list`; do not rewrite
  them to read the AST. The AST is a view, not a second list parser.
- A UI “show structure” panel can read the wasm AST export; it must degrade to
  the existing `analyze_syntax` shape if the AST is absent.
- A list consumer that needs nested lists is the trigger for phase 3b, not for
  a new list parser.

## 7. Phase 6 — cleanup and API

### 7.1 Public API

- Add `#[non_exhaustive]` to `NodeKind`, `ClauseKind`, `PhraseKind` and
  `CoordKind` when the wasm export makes them a cross-language contract.
  This is a 0.x breaking change for an exhaustive `match`; do it in the same
  commit as the export, document it, and add the wildcard arm in every in-repo
  match.
- Keep `Node` fields public. Consumers do not construct nodes; adding a field
  is still a 0.x break, so batch future fields.
- Keep `build`, `analyze`, `validate`, `Ast`, `Node`, `NodeId`, `Constituent`
  and the kind enums re-exported from `syntax::ast`.
- Do not put `Ast` inside `SyntaxAnalysis`: it would change `PartialEq`, the
  wasm shape and every caller.

### 7.2 Checker helpers

- Do not bulk-delete `syntax_check/analysis.rs` helpers. They serve many
  token-level rules that do not need a tree.
- Migrate one helper only when an AST-backed rule has the same output and the
  old caller can be removed in the same commit.
- `attribute_head` is the first candidate (5c.1); `first_word`, `last_word`,
  `clause_words` and `nominal_case` are not.

### 7.3 `syntax_list`

- Keep `syntax_list.rs` as the only list parser.
- The AST list nodes point at the arena; they do not replace it.
- If 3b lands, the AST’s nested list indices and `List.parent` must be checked
  against the arena, not re-derived.

### 7.4 Documentation

- Update `PLAN_SYNTAX_AST.md` status and this document’s status as each phase
  lands.
- Add `syntax::ast` to the public API inventory in `AGENTS.md` when the wasm
  export lands.
- Add the `syntax_tree --order` and wasm `analyze_ast` rows to `README.md` and
  `tools/README.md` only when they exist (they do for `--order`).
- Keep the `syntax_tree` CLI and the golden fixture format in sync.

## 8. Performance and caching

The AST is on-demand. Never call `ast::build` from the keystroke path.

Measurements to take before deciding on a cache:

| scenario | what to measure | budget |
|---|---|---|
| 100-word text | `ast::build` alone | < 1 ms release |
| 1,000-word text | `ast::build` alone | < 5 ms release |
| grammar check | `check_analysis` with and without AST | < 5% regression |
| wasm `analyze_ast` | analysis + tree vs `analyze_syntax` | document the delta |

Do **not** add an AST to `TextAreaState` in phase 5b. The existing
`state.syntax_cache` already makes morphology cheap; the tree is rebuilt only
when the panel asks. If a consumer re-renders the tree on every keystroke, the
consumer is wrong, not the builder.

A per-sentence AST cache inside `SentenceCache` is a later option. It is
awkward because clause/host/segment indices are document-global; do not start
it without a measured hot path.

## 9. Test matrix for the remaining work

For every phase:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
cargo run --release -p torille-tools --bin syntax_tree -- --check "..."
```

Phase-specific:

- 3b: nested-list fixtures, parent-failure subtree test, mixed stays flat;
- 4: coordination fixtures only if triggered, `word_order_allow` unchanged;
- 5b: `cargo check -p torille-wasm --target wasm32-unknown-unknown`,
  wasm-pack/demo build, JSON field smoke test;
- 5c.1: attribute parity audit, all syntax diagnostics byte-identical,
  `perf_grammar_style`;
- 5c.2: `word_order_allow.rs`, no `WordOrder` error/warning;
- 6: public API docs, wildcard arms, no dead code.

The existing Python gates are unaffected unless an engine table changes. Do
not run them as a substitute for the AST corpus test; they do not see the AST.

## 10. Risks and open questions

| risk | mitigation |
|---|---|
| 3b nested lists duplicate the list arena | child indices come from `ListItemKind::List`; validator cross-checks |
| mixed lists make the tree lie | keep them flat; revisit only with a consumer |
| coordination duplicates lists | defer; use the arena until a concrete need |
| AST-backed attribute rule changes output | parity audit first; byte-identical gate; fallback to flat |
| grammar path slows down | measure; build lazily or per candidate clause; no keystroke |
| wasm JSON shape drifts | one writer for nodes/order; native helper test; document fields |
| public enum growth breaks matches | `#[non_exhaustive]` + in-repo wildcard arms at the export commit |
| quote nesting is one run | document; a depth-aware quote is a separate phase only if a consumer needs it |
| sentence grouping differs from `SentenceSpan` count | document; AST consumers use `Sentence` nodes, flat consumers keep spans |
| host clause is not one order unit | `clause_constituents` is per flat clause; a `host_constituents` view is separate work |
| `fronted_constituent` is silent on unknown wh words | intentional; a rule needs a positive cue, not a guess |

Open questions:

1. **Wasm shape.** Is `{ syntax, ast }` the right envelope, or should
   `analyze_ast` return only the AST and leave `analyze_syntax` for the flat
   view? Recommendation: the envelope, so one call gives the panel both.
2. **3b consumer.** Does any UI actually need nested lists? Recommendation:
   no code until the “show structure” panel exists.
3. **Attribute migration.** Is `check_attributes` worth migrating at all if
   the flat rule is already correct? Recommendation: only if the parity audit
   finds a real phrase-only case; otherwise leave it and document the tree as
   the future path.
4. **Coordination.** Is `ListKind::Clause/Predicate` enough? Recommendation:
   yes until a rule needs one node for a coordinated argument.
5. **`#[non_exhaustive]` timing.** With the wasm export or at 1.0?
   Recommendation: with the export, because that is when external matches
   appear.

## 11. Recommended sequence

1. **5b wasm export** — **landed**: additive, low risk, makes the tree usable
   and creates the first external shape contract.
2. **5c.1 parity audit** — no behavior change; decide whether the checker
   migration is worth it.
3. **3b nested nominal lists** — only if the panel or a rule needs them.
4. **4 coordination** — only on a named trigger.
5. **6 cleanup** — after the consumers settle; `#[non_exhaustive]` with 5b.

Definition of done for this plan:

- the wasm export is documented and tested;
- the attribute parity audit is recorded (pass or documented exceptions);
- any 3b/4 work has a named consumer and its fixtures;
- the flat model and every diagnostic are unchanged unless a phase explicitly
  says otherwise;
- `cargo test --release --workspace`, clippy, the AST corpus and the wasm32
  check are green.
