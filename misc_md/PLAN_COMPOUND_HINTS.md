# PLAN_COMPOUND_HINTS — a precomputed lexical-compound split table

Status: **prototyped, measured and parked** · 2026-11-22 · baseline
`ee7b7f9` (after Phase 3 of
[`PLAN_UNKNOWN_FAST.md`](PLAN_UNKNOWN_FAST.md))

## 0. Result

The idea was implemented end-to-end and measured. The lexical table is real:
**48,973 splits from 56,487 classless lemmas (87 %)**, 48,973 unique rows,
**173 KB gzip** (2.4 MB CSV). The runtime hint was correct — parity with the
naming gate was 0 on all corpora and the unknown set was unchanged — but it
did not meet the go gate:

| corpus | hint hits / calls | share |
|---|---:|---:|
| `page-2000` | 21 / 88 | 24 % |
| 8 %-error page | 19 / 168 | 11 % |
| book `45448` | 884 / 3,358 | 26 % |
| book `7000` | 186 / 9,008 | 2 % |

`page-2000` improved from ~131 ms to ~119 ms (about 9 %), inside the noise of
the cold-start tool and below the ≥10 % gate. The top tail
(`lempiviikkomme`, `perunkirjoittamista`) is novel, not lexical, and is not
covered. The reader-shaped 7,000 book gets almost no hits.

The go gate required ≥30 % of compound-DP calls and a win on both books; it
failed, so the runtime hook, generator, CSV and build table were reverted.
This file is the measured record. A future attempt needs a richer source
(common novel compounds or modifier-frequency evidence) or a cheaper miss
path, not only the classless rows.

The idea: many Finnish compounds are not discoveries. The Kotus *sanalista*
already lists 56,486 classless rows, and Kotus omits the class precisely
because the word declines like its final component. For those words the
modifier/head boundary is a lexical fact. A small generated CSV can record
that boundary, and the first pass can try the known split before running the
dynamic-programming segmenter.

This is a **hint**, not a new acceptance rule. The runtime still verifies the
head form with the engine; a hit can skip the search, never the proof.

## 1. Why it might pay

After Phase 3 the `page-2000` first pass is **84 ms** at the state level, but
the remaining tail is compounds:

| surface | first-pass time | analysis |
|---|---:|---|
| `lempiviikkomme` | ~23 ms | `lempi` + `viikko` + possessive |
| `perunkirjoittamista` | ~11 ms | `perun` + `kirjoittamista` |
| `tuomioistuimen` | ~5 ms | lexical compound |
| `koulumestarin` | ~3.5 ms | lexical compound |
| `arkihuoneessa` | ~2 ms | lexical compound |

The callgrind profile of the naming gate still puts `is_known_compound_dp`
at ~25 % and `modifier_is_known` at ~27 % of instructions. The DP asks the
same question for every prefix of every surface: is this prefix a known
modifier, and can the rest be a head? For a lexical compound the answer is
already known from the classless row.

A precomputed split table can turn the common case into:

```text
prefix lookup -> one known head -> verify the head form -> done
```

and leave the DP for genuinely novel compounds.

## 2. What the data is

The source is the classless sanalista rows. A row such as `tuomioistuin`
has no inflection class and declines through its head `istuin`; the modifier
`tuomio` is fixed. The CSV records the morphological head boundary, **not**
the semantic decomposition. `compound::analyze` is not the generator oracle:
it reports `tuomioistuimen` as `tuomio` + `istua` + `imeä`, which is a
lexical explanation, not the inflectional split. The oracle is the same
function the engine uses to inflect a classless compound
(`classless_heads` / `head_inflections` / `compound_head_from_lemma`).

A deterministic sample of 2,233 classless lemmas (every 25th row) found
**1,986 exact splits (89 %)** with `modifier_is_known` plus
`compound_head_from_lemma`. The split table is therefore a plausible
lexicon-sized cache; the unknown is how often running text uses those
lexemes, which §5 measures.

Proposed `data/compound_splits.csv`:

```text
compound<TAB>split<TAB>modifier<TAB>head<TAB>head_class<TAB>kind<TAB>source
```

- `compound` — the classless sanalista lemma (`tuomioistuin`);
- `split` — byte offset of the head (`tuomio` is 6 bytes);
- `modifier` — the fixed prefix (`tuomio`);
- `head` — the morphological head lemma (`istuin`);
- `head_class` — the head's class expression (`1`, `9*B`, ...), or empty for
  a derived/bound head;
- `kind` — `ordinary`, `genitive-initial`, `both-parts`, `lexicalised`,
  `bound`, `verb-head`; the runtime uses it to decide whether the split is
  safe to try;
- `source` — `classless` for the initial rows; future rows can name a
  hand-audited exception.

A first version should include only rows where:

1. the lemmas is classless in the sanalista;
2. a split exists at a char boundary;
3. `modifier_is_known(modifier)` is true;
4. `compound_head_from_lemma(modifier, head)` names the whole classless
   lemma;
5. the head is a content head;
6. the whole lemma's engine forms are consistent with the head's paradigm.

The generator should **skip** ambiguous rows rather than guess: class 51
(both parts inflect), plural-only compounds, hyphenated lexemes, and bound
heads need their own rules and would make the first table less exact.

## 3. Runtime shape

Add a small table module, `src/expand/compound_hints.rs`:

```text
modifier -> [head entry]
head entry = { head, class, kind, split }
```

The lookup is:

1. Reject a hyphenated surface; the writing-rule guard runs first.
2. For each char-boundary prefix of the lowercased surface, longest first:
   - probe the modifier map;
   - for each head entry, verify the remainder is a form of `head` under
     `class` (use `emits_target_form`/the stem map, not a generic
     `is_known_word`), and that the head is a content head;
   - return the head on the first verified hit.
3. Otherwise return `None`; the caller runs the existing DP.

The table is consulted **inside** the compound path, after the existing
guards:

- a known non-compound form wins before any compound analysis;
- a closed spelling of a hyphenated lexeme (`linjaauto`) is a misspelling,
  not a compound;
- the possessive belongs to the whole compound and is stripped before the
  hint, exactly as the DP path does now;
- a function word or bare connegative is not a head.

Integration points, in order of expected value:

- `is_known_compound_dp` — try the hint before `segment_compound_dp`;
- `is_known_compound_fast` — the engine-free first-pass probe can use the
  same lexical table after its length/hyphenation guards;
- `is_known_word_with_compound` — only if the first two show a real hit
  rate; the naming path already has the full DP fallback.

The table must not be used by `is_known_compound`'s **hyphenation** branch;
the split table is for closed compounds.

## 4. Generation and packaging

- `tools/src/bin/gen_compound_splits.rs` (or a `tools/oracle` Python
  generator) reads the classless rows and writes the CSV. It must use the
  engine's classless-head oracle, not the semantic `compound::analyze`.
- `build.rs` compiles the CSV into `maps/compound_splits.bin.gz`: a
  front-coded map keyed by modifier, with a head-entry list. The table is
  small: 56k lemmas, at most ~56k head entries; raw ~1–2 MB, gzip likely
  200–500 KB. Gate: **≤500 KB gzip** and no more than a 1 % wasm-size
  increase.
- Add the blob to `REQUIRED` and publish it on the `maps` branch, like the
  stem signatures.
- A release-only test reads the CSV and the blob, rebuilds a deterministic
  sample of rows with `classless_heads`, and checks the split/head/class
  agree. A stale engine change must turn the test red, not silently change
  a compound reading.
- The CSV is generated, never hand-edited. A new hand-audited row is a data
  exception with a source note, not a suffix rule.

## 5. Prototype and gates

Do the measurement before the integration. The first phase is a throwaway
generator plus a lookup harness:

| metric | why |
|---|---|
| classless rows | how many candidates the source actually has |
| rows with an exact split | how much of the lexicon is usable |
| rows by `kind` | how much is ordinary vs 51/bound/genitive |
| page-2000 hint hits | whether the table touches real text |
| hint hits on the top 20 slow words | whether it fixes the tail |
| DP calls avoided / total | the actual saving |
| hint time vs DP time | the lookup must be cheaper |
| CSV/blob size | the wasm budget |

**Go gate:** at least **30 %** of the compound-DP calls on `page-2000` and
the two books are answered by a verified hint, and the first pass improves
by **≥10 %**. The table must be ≤500 KB gzip.

**Stop gate:** if the hit rate is below 10 %, or the top tail words
(`lempiviikkomme`, `perunkirjoittamista`) are not covered, record the
negative result and do not ship. The page's novel compounds may dominate;
the table is a lexical cache, not a segmenter.

Correctness gates for a landing:

- every CSV row's split agrees with `classless_heads`/`head_inflections`;
- every hint hit is verified by the engine's head paradigm, so no new
  accept;
- `page-2000` diagnostics byte-identical (`dump_diagnostics` = 3);
- the typo and all-unknown corpora get no new accepts;
- `tests/catalogue.rs`, the compound tests and the classless-compound report
  unchanged;
- full release workspace tests and clippy green.

## 6. Risks and limits

- **Lexical coverage only.** Novel compounds (`lempiviikkomme`,
  `perunkirjoittamista`) still use the DP. The table can make the common
  lexical compounds cheap without removing the general segmenter.
- **Multi-part compounds.** The first split's head may itself be a compound
  (`tietokoneohjelmointi`). Store the outermost split and let the head be
  verified recursively, or store the first boundary and fall back to the DP
  when the head is not a simple form.
- **Class 51 and bound heads.** Both-parts-inflect and bound-suffix
  compounds do not follow the ordinary modifier/head split; exclude them
  from the first table and count them in the `kind` report.
- **Gradation at the boundary.** The modifier is fixed, but the head form
  can be weak/strong; verify with the engine's head paradigm, never with a
  string prefix alone.
- **Homonyms.** A head can have several classes; store every class that
  `head_inflections` returns, or skip the row if the head is ambiguous.
- **Staleness.** The table is a cache of the engine's classless analysis.
  The generator and the release-only parity test are the guard.
- **Duplicate source of truth.** The table must not become a second
  inflection dataset. It stores only the boundary and the head name; the
  paradigm still comes from the stem maps/engine.

## 7. What not to do

- Do not store every inflected surface. The exact form table was rejected
  at 3.15 MB gzip grouped / 8.1 MB flat; this table stores one lemma row
  per lexical compound, not millions of forms.
- Do not accept a hint on a prefix match alone. The head form must verify.
- Do not use the semantic `compound::analyze` split as the inflectional
  head; `tuomioistuimen` shows why.
- Do not hand-maintain the CSV. Generate it from the classless rows and the
  engine.
- Do not remove the DP. It is the fallback for novel compounds, and the
  table is only a shortcut.

## 8. First week

1. Write the throwaway generator over the classless rows; count exact
   splits by `kind` and inspect 50 random rows.
2. Run the hint-only lookup over `page-2000` and the two books; record hit
   rate, DP calls avoided, and the top-20 tail coverage.
3. If the go gate passes, add `src/expand/compound_hints.rs`, the build
   blob, and the `is_known_compound_dp` hook behind a feature flag.
4. Re-measure the first pass and the wasm size; only then generate the
   committed CSV and publish the blob.
