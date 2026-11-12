# PLAN_NESTED_LISTS — true list-in-list support

Status: plan only. This is the remaining Phase 3 item from
`PLAN_SYNTAX_CHECKER.md`. Do not implement this as another comma heuristic.

The colon-introduced list already works:

```text
Ostin kolme asiaa: omenoita, appelsiineja ja banaaneja.
```

The missing case is a **list inside a list item** (or inside a parenthetical
inside one), for example:

```text
Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.
```

The parent list has three items; the second item contains a nested list of two
adjectives. The nested list must be a real child in the list arena, not four
extra top-level items.

## 1. Why the current attempt failed

The first implementation was tried and reverted. It added:

- a `scan` range to `ListItem` that included trailing separators;
- a bracket-pair scanner inside an item;
- a recursive `parse_clause` for the inside of each bracket pair;
- child indexes in `ListItemKind::List(index)` and `List.parent`.

The list parser can do all of that, but the analysis layer had already split
the sentence before the list parser ran. `src/sentence.rs`'s
`separator_ends_sentence` treats opening and closing brackets as sentence
boundaries (so a capital after `(` is accepted by the case layer). For the
example above, the tokens are effectively grouped as:

```text
sentence 0: Ostin omenoita, appelsiineja
sentence 1: pieniä, makeita
sentence 2: ja banaaneja
```

`src/syntax.rs` builds `sentence_words` from those sentence indices, so the
list parser sees the top-level comma and the comma inside the parentheses as
siblings. It produces the wrong items before the nested pass ever runs.

The fix is therefore **not** in `syntax_list.rs` alone. The syntax layer needs
a clause grouping that keeps a parenthetical together, while the sentence
layer keeps its current spans for the case checker.

## 2. Design decision

Keep `sentence.rs` unchanged. Its bracket boundaries are there for
capitalisation after `(` and are covered by case tests.

Add a **syntax-only clause grouping** in `src/syntax.rs`:

```rust
fn clause_groups(
    tokens: &[Token],
    contexts: &[sentence::TokenContext],
) -> Vec<Vec<usize>>;
```

The groups are what `analyze_with` feeds into `segment_clause_words`. The
public `SyntaxAnalysis.sentences` stays the original sentence spans; a
`Clause` may span two sentence spans when the boundary was a parenthetical
bracket. Document that in `Clause`.

### Merging rule

Build the ordinary sentence groups first, then merge two adjacent groups when
the separator between their last and first word is a **bracket-only boundary**:

- contains `(`, `[` or `{`, and no `.`, `?` or `!`;
- or contains `)`, `]` or `}`, and no `.`, `?` or `!`.

Examples:

| text | groups |
|---|---|
| `Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.` | one group |
| `Teksti (tämä) jatkuu.` | one group |
| `(Tämä on uusi virke.) Toinen virke.` | two groups; `". "` after `)` is terminal |
| `Teksti (tämä). Jatkuu.` | two groups; the period after `)` is terminal |
| `Teksti (tämä) jatkuu. Toinen virke.` | the period after `jatkuu` starts a new group |
| `Koira, joka nukkuu sohvalla, on minun.` | unchanged |
| `Menin kotiin, koska olin väsynyt.` | unchanged |

Implementation can be a small union-find/iterative merge over sentence groups;
the boundary separator tokens are already available between the two groups'
word token ranges. A separator may contain both `)` and `.`; the period wins.

Quotes are deliberately not merged. Direct speech and citation punctuation are
not parenthetical lists, and merging them would change sentence semantics more
broadly.

## 3. Nested list parser

After the grouping fix, restore the nested list work. The shape is already in
the public types; this phase only populates it.

### `ListItem.scan`

`ListItem.tokens` stays the word range used by diagnostics. Add:

```rust
pub(crate) scan: Range<usize>, // tokens + trailing separators
```

`scan` starts at the first word and runs past the last word over separators
until the next word or the clause end. It is what lets the bracket scanner see
the closing `)` after `makeita`:

```text
appelsiineja ( pieniä , makeita )
^ tokens start                       ^ tokens end
^ scan start                                  ^ scan end
```

### Recursive parse

For each item:

1. Scan `item.scan` for balanced `()`, `[]`, `{}` pairs.
2. For each inside range with at least two words and a top-level comma, build a
   synthetic `Clause` for that range.
3. Call `parse_clause` recursively. The child list is appended to the arena.
4. Record the child index. After all children are appended, set
   `list.parent = Some(child_parent_index)` and set the parent item's kind to
   `ListItemKind::List(child_index)`.
5. Push the parent list after its children, so `analysis.lists` is post-order:
   children before parents. Indices remain stable.

The parent list's `tokens`/`span` still include the parentheses because the
item is a phrase. Only the child's range excludes the brackets.

### Parent punctuation

`missing_separators` must skip an item whose kind is `ListItemKind::List`.
Its words include the nested item's words, and the child's own commas are not
missing parent separators.

## 4. Diagnostics

The child list is a normal `List` in the arena, so `list_diagnostics` applies
the existing rules to it:

- `ListSeparatorMissing` inside the child;
- `ListSeparatorDuplicate` inside the child;
- `ListCommaBeforeConjunction` inside the child;
- `ListIntroducerComma` for a colon-introduced child.

The parent list keeps its own top-level diagnostics. Conflict resolution stays
one diagnostic per span; a child and parent should never produce the same
span, but the ranker's span dedup is the safety net.

## 5. Tests

### Clause grouping

A `syntax::clause_groups` test table:

```text
Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.  # one group
Teksti (tämä) jatkuu.                                        # one group
(Tämä on uusi virke.) Toinen virke.                          # two groups
Teksti (tämä). Jatkuu.                                        # two groups
Teksti (tämä) jatkuu. Toinen virke.                           # two groups
```

Also assert the existing sentence spans are unchanged and the case-layer
tests still pass.

### List arena

```text
Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.
```

- parent list has 3 items;
- second item kind is `List(child)`;
- child list has 2 items and `child.parent == Some(parent)`;
- lists are post-order: child index < parent index;
- parent item punctuation does not report the child comma as missing.

### Diagnostics

```text
valid:        Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.
missing:      Ostin omenoita appelsiineja (pieniä, makeita), banaaneja ja päärynöitä.
duplicate:    Ostin omenoita, appelsiineja (pieniä,, makeita) ja banaaneja.
not a list:   Kuitenkin, ostin omenoita (jotka olivat kalliita) eilen.
not a list:   (Tämä on uusi virke.) Toinen virke.
```

The `not a list` rows must produce no list diagnostics. The `missing` row
reports the parent boundary, not the child.

### Regression

- `Koira, joka nukkuu sohvalla, on minun.` unchanged.
- `Koira, joka nukkuu sohvalla on minun.` still `CommaPairUnbalanced`.
- `Menin kotiin koska olin väsynyt.` still `CommaMissing`.
- The case-layer bracket tests (`opening_brackets_start_a_sentence`,
  `closing_brackets_start_a_sentence`) must stay green.

## 6. API and wasm

- `SyntaxAnalysis.lists` is already serialized.
- `ListItemKind::List(index)` already serializes a `list` index; the wasm
  consumer can walk the arena.
- `ListItem.scan` is internal and is not serialized.
- `Clause` may now span sentence spans; add a note to the `Clause` docs and
  the wasm shape comment. No new field is required unless a UI needs the
  original sentence index; if it does, add `Clause.sentence: usize` in a
  separate change.
- `SyntaxAnalysis.sentences` remains unchanged.

## 7. Phases

### Phase 0 — audit fixture (already done)

- [x] Reproduce the sentence split at `(` and `)`.
- [x] Record the failed nested attempt and revert it.
- [ ] Add the two regression examples above to a temporary audit test (can be
      folded into Phase 1).

### Phase 1 — bracket-merged clause grouping

- [ ] Add `clause_groups` in `src/syntax.rs`; use it instead of raw
      `sentence_words` for clause segmentation.
- [ ] Merge only bracket-only boundaries; periods/questions/exclamations after
      the bracket stay real boundaries.
- [ ] Keep `sentence::spans` and `TokenContext` unchanged.
- [ ] Test the five grouping rows and the existing case tests.

### Phase 2 — nested list arena

- [ ] Add the internal `scan` range.
- [ ] Bracket-pair scanner + recursive `parse_clause`.
- [ ] Set `List.parent` and `ListItemKind::List(index)`.
- [ ] Skip parent missing-separator checks for nested items.
- [ ] Test the arena and the post-order indices.

### Phase 3 — nested punctuation and diagnostics

- [ ] Apply the existing list rules to children.
- [ ] Test valid, missing, duplicate and not-a-list cases.
- [ ] Confirm conflict resolution and deterministic order.

### Phase 4 — integration

- [ ] Add wasm/consumer docs for parent/child list traversal.
- [ ] Add the clean corpus rows.
- [ ] Run `cargo test --release --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`, and the Python
      gates as for the other phases.

## 8. Open questions

1. **Multiple nested lists in one item.** `ListItemKind::List(usize)` supports
   one child. If an item contains two parenthetical lists, do we add
   `Lists(Vec<usize>)` or leave the second unrepresented? Recommendation:
   support one child now; add a vector only when a real sentence needs it.
2. **Brackets vs. parentheses.** Treat `()`, `[]` and `{}` the same for
   grouping and nesting. This is a syntax decision, not a case-layer one.
3. **Parent list role.** The parent list's `role`/`head` should still come from
   the host clause; the child list inherits the same clause but not the parent
   role unless a later semantic layer says so.
4. **Quotes.** Do not merge across quote boundaries in this phase. A list
   inside a quotation is still parsed if its brackets keep it together; the
   quote itself is not a nested-list cue.
5. **Clause spans across sentences.** Is a clause spanning two
   `SyntaxAnalysis.sentences` acceptable, or should the UI get a separate
   `Clause.sentence`? Recommendation: document the span and add the field only
   when a consumer needs it.

## 9. Definition of done

```text
Ostin omenoita, appelsiineja (pieniä, makeita) ja banaaneja.
```

produces:

- one parent `List` with three items;
- one child `List` with two items;
- `child.parent == Some(parent)`;
- no `ListSeparatorMissing` for the child's comma at the parent level;
- the existing top-level list, clause-comma and agreement tests unchanged.

And these remain true:

```text
(Tämä on uusi virke.) Toinen virke.        # two clause groups
Teksti (tämä). Jatkuu.                      # two clause groups
Koira, joka nukkuu sohvalla, on minun.     # unchanged
```
