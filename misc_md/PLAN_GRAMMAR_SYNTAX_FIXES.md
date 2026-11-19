# PLAN_GRAMMAR_SYNTAX_FIXES — the remaining grammar/syntax work

Status: **proposal.** No code in this plan. It follows the completed
`PLAN_GRAMMAR_PRECISION.md` and the landed AST phases in
`PLAN_SYNTAX_AST.md` / `PLAN_SYNTAX_AST_REST.md`.

The checker is precise: 79 adjudicated clean prose rows / 1,233 words, 1 true
positive and 0 known false positives; `page-2000` is at 5 diagnostics. The
remaining work is **structural coverage and narrow recall**, not another
precision sweep.

## 1. Evidence

| finding | evidence | kind |
|---|---|---|
| predicate-less NP has no AST phrase | the 5c.1 parity audit’s one exception: `... kivuloinen ja kalpea tyttö ...` | AST structure |
| wh constituent is invisible | `Missä hän asuu?` → `first=unknown`, `fronted=-` | AST/word-order view |
| host has no single order | `clause_constituents` is per flat clause; `Mies, joka tuli, söi` has no host-level order | AST/API |
| missing relative closing comma is quiet | `Mies, joka tuli söi.` → 0 diagnostics | checker false negative |
| imperative connegative has no correction | `Älä menet.` → `connegative` with `corrections: []` | correction gap |
| conditional/potential connegative | conditional 3sg accepted; potential left quiet by design | morphology-gated |
| docs baseline is stale | `docs/syntax_checker.md` says 66 clean rows / 1,021 words; fixture is 79 / 1,233 | documentation |

Current AST baseline: 24 unit tests, the 122-sentence fixture corpus, the
wasm AST shape test and `syntax_tree --check`.

## 2. Non-goals

Do not add or change these in this plan:

- a general `WordOrder` diagnostic;
- `focus_type` narrow/broad or formal register;
- corrections for `Repaired`/`Guessed` degree forms;
- possessive severity or correction policy;
- broad numeral or adposition government;
- nested/mixed/clause lists or a `Coordination` node without a consumer;
- the extension’s opt-in trigger (context menu only);
- a statistical model, a parser rewrite or a new dependency.

`Clause.sentence` and a flat `ClauseKind` are listed as optional cleanup; they
are not required for the four user-facing fixes.

## 3. Recommended order

| phase | work | risk | user value |
|---|---|---|---|
| 1 | predicate-less nominal phrases in the AST | low | removes the parity exception; headings/captions |
| 2 | relative closing-comma diagnostic | medium | real grammar false negative |
| 3 | imperative connegative corrections | low | real correction gap |
| 4 | interrogative and host constituent views | low | word-order API/UI |
| 5 | conditional/potential connegative | gated | recall, only with morphology evidence |
| 6 | docs/API cleanup | low | maintainability |

Each phase is a separate commit. Phases 1 and 4 are AST-only and must not
change a diagnostic. Phases 2 and 3 change checker output and need exact
fixtures.

## 4. Phase 1 — predicate-less nominal phrases

### Goal

Give a predicate-less clause a shape-based `Phrase` for an attributive run:

```text
Punainen auto.
kalpea tyttö
```

The head is the noun, the preceding adjective/ordinal/demonstrative is the
modifier, and the phrase confidence is `Weak`. No role is invented.

### Current behaviour

`src/syntax/ast/phrases.rs` roots a phrase on a clause slot, an `Adverbial`,
or the target of a flat `Modifier` edge. In a predicate-less fragment the flat
model often has no such edge (`kalpea` → `tyttö` in the prose exception), so
the AST has no phrase and the 5c.1 audit cannot compare the head.

### Design

- Run only when `clause.predicate.is_none()`.
- Scan the clause’s words in source order for a **shape run**:
  - zero or more modifier-shape words immediately before a nominal head;
  - modifier shape: `Adjective`, `Ordinal`, or a demonstrative pronoun
    (`se`, `tämä`, `tuo`, `ne`, `nämä`, `nuo`, `sama`);
  - head: `Noun` (including a proper name read as a noun);
  - no coordinator, unknown word, comma or other separator between the run’s
    words except ordinary whitespace.
- The head is the phrase head; the run’s other words are its children. A run
  with no modifier is not a phrase: a bare word stays a leaf.
- Do not include a genitive noun modifier in phase 1. `koulumestarin` is a
  separate word; the parity case only needs `kalpea tyttö`.
- Do not claim a word already claimed by an existing phrase, list item or
  wrapper. If the shape run overlaps one, skip it.
- Use the flat head token’s role (`Unknown` in the fragment), the flat
  confidence (`Weak`), and the phrase kind from the head POS.
- Attach the phrase under the clause node and reparent the run’s leaves.
- Extend the parity audit’s exception list to empty; this is the acceptance.

### Edge cases

- `Punainen autot.` is still a shape phrase; number agreement is the
  checker’s job, not the tree’s.
- `Koko päivänä` is an invariant determiner; the phrase may form, but no
  diagnostic follows because the rule keeps its invariant guard.
- A heading with no punctuation and no predicate is the target case.
- A run containing a relative pronoun, subordinator or conjunction is not a
  phrase; leave it flat.
- A run that crosses an existing list/phrase is skipped.

### Files

`src/syntax/ast/phrases.rs`; the shape helper and unit tests in
`src/syntax/ast/mod.rs`; the parity exception in
`src/syntax_check/agreement.rs`.

### Tests and gate

- `Punainen auto.`, `kalpea tyttö`, the full prose exception sentence;
- a heading with no predicate and a multi-modifier run;
- `syntax_tree --check` over the corpus;
- `attribute_head_matches_the_ast_phrase_head` has **zero** exceptions;
- `cargo test --release --workspace`; no diagnostics changed.

## 5. Phase 2 — relative closing comma

### Goal

Catch the missing closing comma in a non-restrictive relative clause whose
host predicate was swallowed into the relative by the flat parser:

```text
Mies, joka tuli söi.        → comma_pair_unbalanced after `tuli`
Mies, joka tuli, söi.      # clean
Mies joka tuli söi.        # clean: restrictive relative, no opening comma
Mies, joka tuli.           # clean: no continuation
```

### Current behaviour

`src/syntax_check/punctuation/comma.rs` has `relative_comma`, but it only
inspects `finite_predicate_words` inside the relative. In the missing-comma
sentence the flat parser produces one relative clause containing a subject
`tuli` and a predicate `söi`; there is no second clause for the host, and the
rule stays quiet.

### Design

Add a narrow `relative_closing_comma` check (or extend `relative_comma`):

- Find a `ClauseKind::Relative` clause `R`.
- Let `antecedent = previous_word(R.first)`. Require a comma or dash between
  the antecedent and `R.first`; a restrictive relative with no opening comma
  is left alone.
- Collect the finite predicates in `R` by role (`Predicate`/`Auxiliary`).
  If there is no predicate, stop.
- Look for a finite word **before** the first predicate whose role is not
  `Predicate`/`Auxiliary` (the likely relative predicate, as `tuli` in the
  example). If there is no such word, stop.
- If there is already a comma or dash between that word and the predicate,
  the parser has split the boundary; stop.
- Emit `CommaPairUnbalanced` at the end of that finite word (after `tuli`),
  with no automatic correction unless an exact punctuation edit is safe.
- Do not fire when:
  - the relative is final in the sentence;
  - the opening delimiter is absent;
  - the continuation starts with a coordinator (`ja`, `tai`, `vai`, `sekä`,
    `eli`) or a line break;
  - the finite word is the clause predicate/auxiliary, or is a negative
    auxiliary;
  - a terminal mark, dash or existing comma separates the two words.

Use the flat `Clause.parent`/`children` and `same_sentence` helpers; the AST
host is not required for this rule. The AST host remains the UI/order view.

### Tests

Add exact-output cases to `tests/fixtures/syntax_diagnostics.json`:

- missing closing comma (`Mies, joka tuli söi.`);
- clean with comma;
- clean restrictive without opening comma;
- clean final relative (`Mies, joka tuli.`);
- clean dash;
- a relative with a coordinated predicate (`Mies, joka tuli ja söi.`) stays
  clean.

Run `syntax_prose` and `page-2000`; no new false positive. If the heuristic
finds a prose row, add it to the adjudicated fixture as clean or record the
exception before landing.

### Files

`src/syntax_check/punctuation/comma.rs`, `tests/fixtures/syntax_diagnostics.json`,
`tests/fixtures/prose/adjudicated.json` if a new clean row is needed.

### Gate

- exact diagnostics fixture;
- 79 clean prose rows still quiet;
- `page-2000` count does not rise;
- `cargo test --release --workspace`.

## 6. Phase 3 — imperative connegative corrections

### Goal

Give the existing imperative connegative finding a correct repair:

```text
Älä menet.      → mene
Älkää menette.  → menkö
Älköön menette. → menkö (if the catalogue has the 3sg imperative connegative)
```

### Current behaviour

`src/syntax_check/connegative.rs` detects the error but deliberately returns
`Vec::new()` when `is_imperative_auxiliary` is true. `is_imperative_auxiliary`
only checks `älä`; the plural/third-person auxiliaries are not covered by the
correction path, and `connegative_corrections` requests the erroneous
affirmative reading’s mood (indicative for `menet`), not the imperative.

### Design

- Extend `is_imperative_auxiliary` to the closed auxiliary set the negation
  table knows (`älä`, `älkää`, `älköön`, `älkööt`), using
  `expand::negation::negation_base` and the auxiliary’s `Tags`.
- Read the auxiliary’s person/number from its negative-verb reading. If it is
  missing or ambiguous, keep the diagnostic and leave corrections empty.
- Add an imperative branch to the correction helper:
  ```text
  want = Verb, Active, Negative, Imperative,
         Person and Number from the auxiliary
  ```
  request forms through `cached_forms`, require an exact form reading and a
  single token, and apply the same `Correction::new` / `Inflection` license.
- If no exact imperative connegative exists in the catalogue, return no
  correction; do not fall back to the indicative `mene`.
- Keep the diagnostic kind, severity and span unchanged.

### Tests

- `Älä menet.` → correction `mene`;
- `Älkää menette.` → `menkö`;
- `Älköön menette.` → `menkö` if generated, otherwise the test records the
  empty correction as the accepted current behaviour;
- valid `Älä mene.`, `Älkää menkö.`, `Älköön menkö.` stay quiet;
- the existing `connegative_imperative` exact fixture is updated, not
  duplicated.

Probe the catalogue first with `inflect mennä` and the existing form set. If
the imperative connegative is not generated, phase 3 reduces to extending
`is_imperative_auxiliary` and leaving corrections empty; document that.

### Files

`src/syntax_check/connegative.rs`, `src/syntax_check/correction.rs`,
`tests/fixtures/syntax_diagnostics.json`.

### Gate

- exact correction fixture;
- no new diagnostics on clean prose;
- `cargo test --release --workspace`.

## 7. Phase 4 — interrogative and host constituents

### Goal

Make the word-order view see the constituent that the question is about and
the host as one order:

```text
Missä hän asuu?
  first: interrogative (Missä)
  fronted: -

Mies, joka tuli, söi.
  host order: Mies, [relative joka tuli], söi
```

### Design

#### Interrogative flag

- Add `pub interrogative: bool` to `Constituent`.
- A constituent is interrogative when:
  - the clause’s `features.question_type` is `Wh` or `Echo` and its head
    surface is in `syntax::INTERROGATIVE_SURFACES`; or
  - the clause is `YesNo`/`Alternative` and a reading of the constituent’s
    head has `Clitic::Ko` (or the surface fallback already used by
    `question_type_of`).
- `fronted_constituent` stays object/adverbial/complement-only. Add
  `Ast::interrogative_constituent(analysis, clause)` (or let a caller use
  `first_constituent().interrogative`); do not change the meaning of
  “fronted”.
- `syntax_tree --order` prints `q:` on the constituent; `--json --order`
  adds `"interrogative": true`.
- `torille-wasm`’s constituent object adds the same field.

#### Host order

- Add `Ast::host_constituents(host: NodeId)` returning the host’s direct
  children in source order, including the nested `Clause(Relative)` as a
  constituent with `role = None` and `kind = clause`.
- Add `SyntaxTree::host_constituents(clause_index)` which finds the host by
  the segment whose token range is the flat clause range.
- `syntax_tree --order` prints a `host` line when a clause belongs to a
  multi-segment host:
  ```text
  host: Mies[0..4] relative[6..15] söi[17..21]
  ```
- The wasm `ast` object gains an additive `hosts` array:
  ```json
  [{ "node": 2, "clause": 2, "segments": [[0,1],[6,7]],
     "constituents": [...] }]
  ```

### Tests

- `Missä hän asuu?` first constituent has `interrogative = true`;
- `Hän asuu missä?` has a post-predicate interrogative constituent, but
  `fronted` remains `-`;
- `Tuleeko hän?` marks the predicate constituent interrogative;
- `Mies, joka tuli, söi.` host order is `Mies`, relative, `söi`;
- `syntax_tree --order` and `--json --order` snapshots;
- wasm `tests/ast-shape.mjs` checks the new fields.

### Files

`src/syntax/ast/mod.rs`, `tools/src/bin/syntax_tree.rs`,
`torille-wasm/src/lib.rs`, `torille-wasm/tests/ast-shape.mjs`.

### Gate

- no diagnostic change (AST/wasm only);
- `syntax_tree --check` corpus;
- `cargo check -p torille-wasm --target wasm32-unknown-unknown`;
- `node --experimental-wasm-modules tests/ast-shape.mjs`;
- `cargo test --release --workspace`.

## 8. Phase 5 — conditional/potential connegative (gated)

The conditional connegative is accepted as the 3sg conditional surface; the
potential is left quiet because the engine does not tag it reliably.

Do not add a rule until a morphology fixture shows:

- the potential or conditional negative surface has an unambiguous negative
  reading;
- the auxiliary + main verb pair is adjacent and the main verb is not a
  legitimate affirmative form.

If that evidence appears, add a narrow `connegative` case with the same
`Warning`/`Likely` policy as the existing uncertain paths. Otherwise close the
item as **won’t-fix**: quiet is better than a false positive.

## 9. Phase 6 — optional structural cleanup

These are not blockers for the four fixes above.

### `Clause.sentence`

Add `Clause.sentence: usize` from `sentence::TokenContext`; use it in
`punctuation::same_sentence` and the UI instead of byte-span scanning. It is
an additive Rust field and a wasm clause field. Do it only when a rule or the
UI needs the sentence identity; it changes `Clause` construction and
`PartialEq`.

### Flat clause kind

The AST has `ClauseKind`; the checker recomputes a local `ClauseKind` in
`punctuation/clause.rs`. Do not unify until the checker consumes the AST;
duplicating the enum is less risky than a cross-layer dependency today.

### Docs

`docs/syntax_checker.md` still records the 2026-11-15/17 baseline. Refresh:

- 79 clean adjudicated rows / 1,233 words;
- 1 true positive;
- 0 known false positives;
- `page-2000` 5 diagnostics;
- the relative-host limitation and the imperative-connegative gap after their
  phases.

Update `PLAN_SYNTAX_AST_REST.md` / `PLAN_SYNTAX_AST.md` status when phase 1
lands; the parity exception should disappear.

## 10. Test matrix

For every phase:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
cargo run --release -p torille-tools --bin syntax_tree -- --check "..."
```

Checker changes (phases 2, 3, 5):

```bash
cargo test --release -p torille \
  --test syntax_check --test syntax_eval --test syntax_ambiguity --test syntax_prose
cargo run --release -p torille-tools --bin check_syntax -- "..."
```

AST changes (phases 1, 4):

```bash
cargo test -p torille --test syntax_tree
cargo test --release -p torille --lib syntax::ast
cargo test --release -p torille --lib syntax_check::agreement::tests
```

Wasm changes (phase 4):

```bash
cargo check -p torille-wasm --target wasm32-unknown-unknown
cargo clippy -p torille-wasm --target wasm32-unknown-unknown -- -D warnings
cd torille-wasm && node --experimental-wasm-modules tests/ast-shape.mjs
```

No Python gate is needed unless a data table or morphology changes.

## 11. Risks and mitigations

| risk | mitigation |
|---|---|
| shape phrases invent structure in headings | only predicate-less clauses; `Weak` confidence; no role; parity + tree validation |
| relative comma heuristic false-positives | opening delimiter required; coordinator/dash/line-break guards; prose/page gate; exact fixture |
| imperative corrections name the wrong person | read auxiliary person/number; exact catalogue form; empty correction if ambiguous |
| conditional/potential rule guesses | morphology fixture first; otherwise won’t-fix |
| constituent field breaks wasm consumers | additive field only; update `ast-shape.mjs`; no existing field removed |
| host order duplicates flat order | host view is additive; `clause_constituents` stays per flat clause |
| docs drift again | refresh the baseline in the same commit as the last diagnostic change |

## 12. Definition of done

- predicate-less `kalpea tyttö` has an AST phrase and the 5c.1 parity audit
  has no exceptions;
- `Mies, joka tuli söi.` emits `comma_pair_unbalanced`, while the clean
  variants stay quiet;
- `Älä menet.` and its plural/third-person variants offer a correct
  imperative connegative correction, or the catalogue limitation is
  documented and no wrong form is offered;
- `Missä hän asuu?` and `Mies, joka tuli, söi.` expose their constituents in
  `syntax_tree --order` and the wasm AST;
- conditional/potential is either evidence-backed or explicitly closed;
- docs show 79 clean rows / 1,233 words, 1 true positive, 0 false positives,
  page-2000 5;
- clippy, the workspace suite, the AST corpus and the wasm check are green.
