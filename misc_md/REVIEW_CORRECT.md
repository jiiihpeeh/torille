# REVIEW_CORRECT — public-boundary correctness and robustness

Status: **Phase 1 landed 2026-11-23; Phases 2–5 open.** Baseline `0c06175`
(2026-11-22). The review method was: read the engine, run the workspace gates,
then probe the public entry points with deterministic adversarial inputs. The
gates are clean (`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --release --workspace`); every finding below is about behaviour the
gates do not currently exercise.

## Progress

### 2026-11-23 — Phase 1 (C1–C3) implemented

The three panic paths and the class-engine recursion are closed on the working
tree, with regression tests:

- **C1** — `LexicalCompoundAnalyzer::analyze` now requires the lowercased and
  original strings to agree on byte length as well as character count; `ẞabc`,
  `STRAẞE` and a seeded 1,000-case Unicode fuzz are in `tests/hyphenation_fi.rs`.
- **C2** — `levenshtein_with_ops` falls back to a heap buffer above the
  256-char stack size; long ASCII, multibyte and at-limit cases are in
  `src/suggest/mod.rs` tests, and `decode_into`'s contract is corrected.
- **C3** — the concrete stem unwraps fail closed (`class_28J`, `class_7`,
  `class_33D`, `class_41`, `class_41D`, `class_48`, `class_9`, `class_10`,
  `class_11`, `class_12`, `class_13`, `verbs/utility`, `grammar/verb`);
  `class_9`'s `tt` plural-only rewrite no longer recurses; the wizard rejects
  a non-word surface with `WizardError::NotAWord`; and
  `every_class_survives_an_ill_shaped_surface` runs every lexicon class,
  including the lettered subtypes, over ill-shaped words and all three
  parts of speech.
- Deferred: the generated-vs-hand-written `expand/nominals/` documentation
  conflict is in C7.5; it did not block the engineering fix because the
  checked-in files are the hand-maintained engines.

Verified: clippy clean, `cargo test --release --workspace` green,
`tools/build/compare_engine_outputs.py --strict` green (no lost forms),
`tools/test_binding_agrees_with_binary.py` green (28 lemmas identical),
`tools/test_illegal_evidence.py` green (every illegal row explained), and
`tools/test_reverse_lookup_recall.py` unchanged at 99.95% base recall. The
plan file itself was checked with `tools/check_docs.py`.

The short verdict: the morphology and grammar layers are in good shape, but the
**public boundaries are not total**. Three entry points panic on valid UTF-8 or
long input, the word wizard — a user-facing path — panics on short or
non-letter words, and two checker entry points accept fabricated class-50/51
forms that the full path rejects. All of these are narrow, fixable defects, not
architectural problems; the risk is that they are invisible until a browser
worker or a binding caller hits them.

Order of work: the panic fixes first (they are independent, small, and carry
the clearest regression tests), then the paradigm/lexicon-source fix (one
structural change closes a class of wrong answers), then the case-handling
fixes. The API and maintainability items in Phase 4 and Phase 5 are separate
commits and do not block the correctness work.

## Priority

| id | finding | severity |
|---|---|---|
| C1 | `hyphenation::positions` panics on valid UTF-8 (`ẞ`) | critical, panic |
| C2 | `levenshtein_with_ops` panics above 256 characters | critical, panic |
| C3 | word wizard panics on short or non-letter input | critical, user-facing |
| C4 | class-50/51 forms are fabricated when the lexicon is omitted | high, wrong answer |
| C5 | ASCII-only lowercasing loses `Ä`/`Ö`/`Å` in suggest/check | high, wrong answer |
| C6 | `check` rejects some capitalized forms `check_fast` accepts | high, divergence |
| C7 | public API and documentation drift | medium |
| C8 | duplicated browser extension source and dead modules | medium |
| C9 | cache eviction, hashing and ranking polish | low |

## Evidence

The findings were reproduced with a scratch `examples/` binary and deleted
afterward; the tree is clean. The snippets below are the minimal reproducers
and are what the regression tests should encode.

### E1 — hyphenation panic

`torille::hyphenation::positions` panics for words where lowercasing preserves
the character count but not the byte length. `ẞ` (3 bytes) lowercases to `ß`
(2 bytes), so the guard at `src/hyphenation/compounds.rs:83-86` passes and the
segmenter indexes the lowercased string with the original byte length at
`src/hyphenation/compounds.rs:146`.

| input | result |
|---|---|
| `positions("ẞabc")` | panic, end byte index 6 > len 5 |
| `positions("STRAẞE")` | panic |
| `positions("Straße")` | ok, no points |

A 4,000-case seeded fuzz over the public entry points produced 1,241 panics,
all in this function. The same code backs the wasm `hyphenate` export when
`compound_breaks` is set.

### E2 — `levenshtein_with_ops` panic

`src/suggest/levenshtein.rs:193-198` decodes into fixed `[u32; 256]` buffers
without checking the length. `decode_into` (`:162`) has no bound check and its
doc incorrectly says `s.len() <= dst.len()` (bytes, not chars).

| input | result |
|---|---|
| `levenshtein_with_ops(&"a".repeat(256), "b")` | ok |
| `levenshtein_with_ops(&"a".repeat(257), "b")` | panic |
| `levenshtein_with_ops(&"ä".repeat(300), "b")` | panic |

The function is public and is called from `src/syntax_check/correction.rs:135`.
It also allocates a full `(n+1) x (m+1)` matrix, so an unbounded input is both a
panic and an allocation hazard.

### E3 — wizard panic

`class_wizard::infer` tries every candidate class through `inflect_in` without
validating the surface. The class engines assume a well-formed stem and use
about fifty `chars().last().unwrap()` / `next_back().expect(...)` sites.

| word | pos | result |
|---|---|---|
| `nä` | noun, verb, adjective | panic; noun hits `class_28J.rs:52` |
| `aä`, `rä`, `la` | noun, verb, adjective | panic |
| `🙂`, `ẞ` | noun, adjective | panic |
| `talo`, `kissa`, `xyzzy` | any | ok |

The panic is reachable from `wizard_infer` in `torille-wasm/src/lib.rs:1238`,
which is what the extension options page calls. The UI only rejects an empty
string. A 30,000-case fuzz over `(word, class, pos)` also hit
`src/expand/verbs/utility.rs:259` for several verb classes.

### E4 — fabricated class-50/51 forms

`inflect_all` takes `lexicon: Option<&LexiconIndex>` (`src/expand/mod.rs:733`).
For a class-50/51 lemma the compound splitter needs that index; with `None` it
falls through to the class-50 catalogue model, which lands gradation inside the
compound. The cheap recognition path passes `None` (`lemma_emit`,
`src/expand/find_bases.rs:757-770`), while the full walk passes
`Some(&index)`.

| call | forms |
|---|---|
| `inflect_all("kylmätila", …, None)` | 394 |
| `inflect_all("kylmätila", …, Some(&index))` | 155 |
| `inflect_all("nuori-isäntä", …, None)` | 0 |
| `inflect_all("nuori-isäntä", …, Some(&index))` | 176 |

The fabricated forms are `Plain` confidence: `kylmädilillä`,
`syväasfaltdeiksi`, `ahkeraldin`. The result is that the same word gets four
different answers:

```text
check("kylmädilillä")             Unknown
check_fast("kylmädilillä", …)     Some(Inflected(["kylmätila"]))
check_normal("kylmädilillä")      Inflected { base: "kylmätila", gate: "engine" }
check_normal_fast("kylmädilillä") Unknown
```

The sanalista has 653 class-50 rows and 93 class-51 rows (50 dual-class), so
the blast radius is not a single special case.

### E5 — ASCII-only lowercasing

`src/suggest/candidates.rs:216` and `src/suggest/state.rs:138` use
`to_ascii_lowercase`; so do `src/expand/find_bases.rs:242,420-421,833`. The
comment at `candidates.rs:206-216` says this keeps the distance case-stable; it
does not for Finnish capitals. The prefix walks key on the first characters, so
an `Ä`/`Ö` initial misses the whole neighbourhood.

| input | suggestions at distance 1 or 2 |
|---|---|
| `äitj` | `["äiti"]` |
| `Äitj` | `[]` at distance 1 and distance 2 |
| `öljy` | `["öljy", "öljyn", …]` |
| `Öljy` | `[]` |
| `äidiss` | `["äidissä", …]` |
| `Äidiss` | `[]` |

### E6 — capitalized forms split between paths

`check_common` computes a Unicode `lower` but passes the original-case `word`
to `find_bases_for_check` (`src/suggest/mod.rs:149`), and the walks use ASCII
lowercasing, so a rare capitalized form can be lost by the full path while the
fast path finds it:

```text
check("ääkeihinsä")      Inflected(["ääk"])
check("Ääkeihinsä")      Unknown
check_fast("Ääkeihinsä") Some(Inflected(["ääk"]))
check_normal("Ääkeihinsä") Unknown
```

The same case is true for other prefix-walk-only recoveries; sentence-initial
`Ä`/`Ö` words are the common shape.

## Phase 1 — stop the panics

Each slice is one commit and lands its regression test in the same change.
Tests use a seeded LCG, not a new dependency; the repo has no `proptest` and
this plan does not add one.

### C1 — hyphenation byte/char identity

- [x] In `LexicalCompoundAnalyzer::analyze`, require
  `lower.len() == word.len()` as well as the existing char-count guard, and
  keep the character-count floor of four.
- [x] Add an assertion or a comment at the segmenter entry that `start`/`end`
  are byte offsets into `lower`, and that the part offsets are reused on
  `word`.
- [x] Add `ẞabc`, `STRAẞE`, and `aẞbcdä` to `tests/hyphenation_fi.rs` as
  no-panic cases with the expected (empty or pattern) break list.
- [x] Add a seeded fuzz test over short random Unicode strings asserting no
  panic and that every returned `byte_offset` is a char boundary.

Acceptance: the E1 table has no panics; the fuzz test is green.

### C2 — `levenshtein_with_ops` length handling

- [x] Decode through a `Vec<u32>` when either side exceeds the stack buffer,
  or reuse the stack/heap split from `levenshtein` (`:44-48`).
- [x] Fix the `decode_into` doc to say "the character count must be
  `<= dst.len()`", and make the helper's contract explicit.
- [x] Decide the policy for very long inputs. The matrix is needed for the edit
  script, so either document an explicit maximum length or return the distance
  with an empty ops list above a cap. A silent panic is not acceptable.
- [x] Add `256`/`257` and a multibyte `300 x ä` case to the `suggest` tests.

Acceptance: the E2 table has no panics; the public API has a documented bound.

### C3 — class engines and the wizard

The policy question is whether `expand::inflect(word, class, …)` promises a
total function or only a well-formed stem. The Python binding documents that a
`klass` is enough to inflect a word the sanalista has never seen, so arbitrary
input is part of the contract in practice. Treat it as total.

- [x] Guard `class_wizard::infer`: trim, return a `WizardError` for an empty
  word or a surface that is not alphabetic (emoji, punctuation, symbols), and
  decide a minimum length for candidate generation. A short word can be valid
  in Finnish, so a length guard must still return a report (likely
  "indeclinable" or "no candidate"), not a panic.
- [x] Fix the concrete panic sites: `class_28J.rs:52` (`nth(1)`) and
  `verbs/utility.rs:259` (`next_back`). Then sweep the
  `chars().last().unwrap()` family in `expand/nominals/` and
  `expand/verbs/` and return no forms when the stem cannot exist.
- [ ] Reconcile the generated-vs-hand-written claim before editing. The
  `src/expand/nominals/mod.rs` header and `AGENTS.md:534` say the tree is
  produced by `tools/oracle/extract_class_rules.py --rust`, but the class
  files themselves say "written by hand as a logical engine", and the
  generator's own default output is `target/class-rules`. If the checked-in
  files are hand-maintained, `AGENTS.md` must say so; if a generated subset
  exists, the fix belongs in the generator template and not in the output.
- [x] Add a class-matrix robustness gate over every lexicon class (including
  the lettered subtypes), ill-shaped words and all three parts of speech. The
  matrix covers every class the random-seeded variant would reach, so no
  allowlist was needed.

Acceptance: `infer("nä")` returns a `Report` (no panic), `infer("🙂")` and
`infer("ẞ")` return `WizardError::NotAWord`, the class matrix is green, and the
wizard still answers `näyttelijä`/`kissa` as before.

## Phase 2 — one paradigm source of truth

### C4.1 — make the lexicon index implicit

- [ ] Add a convenience `inflect_all(lemma, inflections, cat, pos, possessives)`
  that uses `LexiconIndex::new(crate::kotus::lemmas())`, and rename the current
  `Option` form to `inflect_all_in` (or `inflect_all_with_lexicon`). Follow the
  same shape for `form_of`/`forms_of` if it keeps the two families consistent.
- [ ] Keep the `_in` variant for the genuinely custom-lexicon callers (the
  binding and the tools already hold their own `LexiconIndex`).
- [ ] Document the invariant: a class-50/51 paradigm is only meaningful with a
  lexicon index; `None` is not "use the catalogue model", it is "cannot
  answer".
- [ ] Update the public docs and the README's Rust API section, since this is a
  pre-1.0 breaking change.

### C4.2 — pass the index through the cheap recognition path

- [ ] Give `lemma_emit` a `&LexiconIndex` parameter and pass
  `Some(index)` to `emits_standalone_form`.
- [ ] Update `find_any_base_with`, `find_best_base_with`, and the
  `cheap_recognition` branch of `find_bases_with_inner`. All three already
  construct the index at the top of the walk.
- [ ] Audit the remaining `inflect_all(..., None)` call sites and classify
  each one. Production sites: `suggest/known/lemmas.rs:21`,
  `morphology/mod.rs:1102,1134`, `expand/check_normal.rs:313`,
  `expand/prefix_index.rs:117`, `expand/stem_index.rs:206`; test-only sites
  follow the same rule. Replace with the embedded index unless the lemma is
  known not to be class 50/51 (and then say so in a comment).

### C4.3 — the parity test

- [ ] Add a test that walks every class-50/51 lemma, inflects it with the
  embedded index, and asserts that `check`, `check_fast`, `check_normal`,
  and `is_known_word` agree on every generated form.
- [ ] Add the four-row E4 table as a direct regression test, with the expected
  answer (`Unknown` for `kylmädilillä`) pinned.
- [ ] Assert the form counts for `kylmätila` and `nuori-isäntä` with and
  without the index (or, after the API change, that the public function has no
  `None` variant).

Acceptance: the E4 table is all `Unknown`; the parity test is green; the form
counts do not change when the caller supplies the same index. Run
`python3 tools/build/compare_engine_outputs.py --strict` and the benchmark
correctness fingerprint; the change affects only the paths that were passing
`None`, so the expected diff is "fabricated forms disappear", not a new form
set.

## Phase 3 — case handling

### C5 — one Unicode lowercase helper

- [ ] Promote the `morphology::lowercase` `Cow` helper
  (`src/morphology/mod.rs:378`) to a shared crate-private utility, or expose it
  as `pub(crate)` from a small `text` module.
- [ ] Replace the ASCII-only calls in `suggest/candidates.rs:216`,
  `suggest/state.rs:138`, and `expand/find_bases.rs:242,420-421,833`. Keep
  `to_ascii_lowercase` only where the input is guaranteed ASCII (for example a
  lemma that is documented as lowercase ASCII), and say why.
- [ ] Use the same helper in `find_bases_for_analysis` and `shares_first_two`,
  or pass an already-lowercased word so the helpers never see case.
- [ ] Pass `&lower` to `find_bases_for_check` in `check_common`
  (`src/suggest/mod.rs:149`).

### C6 — the regression table

- [ ] Add the E5 table: `suggest_detailed("Äitj", 5, 2)` equals the
  case-restored `suggest_detailed("äitj", 5, 2)` result, and likewise for
  `Öljy`, `Äidiss`, and `Åbo`.
- [ ] Add the E6 table: `check("Ääkeihinsä")` equals
  `check("ääkeihinsä")` and `check_fast`; the same for one noun and one
  adjective.
- [ ] Verify the suggestion ranking tests and the candidate-generation golden
  list still pass. The case fix changes the candidate pool for capitalized
  words, so a golden list that includes a capitalized query may move; update
  it intentionally, not by rerunning blindly.

Acceptance: the E5 and E6 tables are green; no ASCII-only lowercasing remains
in the Finnish surface paths.

## Phase 4 — API and documentation hygiene (separate commits)

### C7.1 — root exports

- [ ] Re-export `check`, `suggest_detailed`, and `is_known_word` from
  `src/lib.rs`, or correct `AGENTS.md:414`, which lists `check` and
  `suggest_detailed` as crate-root today. The former is the better fix: `check`
  is the first thing the README advertises.

### C7.2 — `Generated` vs `Form`

- [ ] `src/expand/mod.rs:73` publicly aliases `tags::Form` as `Generated`;
  public signatures use both names. Pick one for the public surface, document
  the alias as deprecated, or keep it and add a doc-test that shows the two are
  the same type. Do not add a second copy.

### C7.3 — diagnostic identity

- [ ] `SyntaxDiagnostic` and `StyleDiagnostic` store both `rule_id` and
  `message` and construct the first from the second
  (`src/syntax_check/diagnostic.rs:126-152`,
  `src/style_check/diagnostic.rs:68-93`). Both fields are public, so a caller
  can make them disagree. Derive `rule_id()` from `message`, or make the fields
  private and expose one accessor.
- [ ] While there, decide whether the diagnostic enums should be
  `#[non_exhaustive]` before 1.0. `Severity`, `Confidence`, `CheckOutcome`,
  `DiagnosticKind`, and `StyleKind` all lack it.

### C7.4 — `CheckOutcome` ergonomics

- [ ] Derive `Clone` and `PartialEq`; consider `Vec<Cow<'static, str>>` for the
  base list so a learned proper name can join the engine's static lemmas.
- [ ] The doc comment on `is_known` says compounds route as `Unknown`; the
  code's `is_known_word` fallback recognises compounds
  (`kahvikekkereitä` -> `Inflected(["kekkerit"])`). Correct the comment and the
  module doc in `src/suggest/mod.rs` at the same time.

### C7.5 — small code hygiene

- [ ] `src/check.rs:24-31`: the `Finding` contract paragraph belongs to
  `findings()`, not `without_rules()`. Move it and give `findings()` its own
  doc.
- [ ] `src/expand/prefix_index.rs` has no callers outside its own tests and
  re-parses the full sanalista on every `inflection_candidate_for` call;
  `src/expand/stem_index.rs` is superseded by `stem_endings` and
  `STEM_BUCKETS_NOUN`. Remove both, or mark them `#[deprecated]` with a
  migration note. They are still `pub mod` at `src/expand/mod.rs:1315,1318`.
- [ ] The root `Cargo.toml:6-7` says the whole `tools` crate is
  GPL-3.0-or-later; `tools/Cargo.toml` uses `license.workspace = true` and
  `LICENSE`/`tools/README.md` say tools are MIT/Apache except the
  Omorfi/Voikko files. Fix the comment. Also use `resolver = "3"` (or omit it)
  with edition 2024.
- [ ] Reconcile `AGENTS.md:534` ("Do not hand-edit generated files.
  `expand/nominals/` is produced by `extract_class_rules.py --rust`") with the
  per-file "written by hand as a logical engine" headers and the generator's
  `target/class-rules` default. This blocks a safe C3 fix until it is clear
  which files are generated.
- [ ] Add `torille-firefox-0.5.0-source.zip` to `.gitignore` (or delete it);
  it is an untracked build artifact in the repository root.

## Phase 5 — maintainability and performance follow-ups

These are lower priority and do not block the correctness work. Each is its
own commit and its own measurement.

### C8.1 — extension duplication

`extensions/firefox/src` and `extensions/chromium/src` are identical except
for `background/background.ts`, where the only differences are two adapter
lines (`api.storage.onChanged` vs `api.storage?.onChanged?.`). That is about
4,000 duplicated lines plus their tests. Put the common source in one package
and copy or symlink it at build time; keep only the manifest, background
adapter, and packaging per browser.

### C8.2 — `find_bases_with_inner`

`src/expand/find_bases.rs:805` takes ten positional booleans and every call
site repeats `/* flag */` comments. Replace the flags with a `WalkOptions`
struct or named enums (`Recognition::BasesOnly`, `CheapGate::On`, ...). This
removes four `#[allow(clippy::too_many_arguments)]` sites and the risk of a
transposed flag.

### C8.3 — correction boilerplate

`src/syntax_check/correction.rs` repeats the lemma -> `(inflections, pos)`
lookup six times (`:96, :169, :243, :324, :417, :477`), and the forms cache key
includes the full inflection list even though it is a function of the lemma.
Extract a `lemma_plan()` helper and key the cache on `(lemma, want, pos)`.

### C9.1 — lowercase allocations

There are about 100 `to_lowercase()` call sites, most of them in
`syntax_check/punctuation/*` where the token is already lowercase. Reusing the
`Cow` helper from C5 avoids the allocation for the common case and is worth
trying against the `page-2000` grammar/style baseline (`19.29 ms` warm).

### C9.2 — cache eviction

`MorphologyCache` (`src/morphology/mod.rs:283,316`), `SentenceCache`
(`src/syntax_check/sentence_cache.rs:224`), and `TextAreaState` all clear the
whole map at their cap. On a long document the words already seen are
re-resolved after the first eviction. A two-generation cache or an LRU keeps
the working set for a few lines more.

### C9.3 — sentence-cache collisions

`SentenceCache` keys a unit by a `u64` hash only
(`src/syntax_check/sentence_cache.rs:34-100`). `len`, `core`, and `quote_depth`
guard most collisions, but two different texts of the same length and hash
reuse diagnostics. Store the unit text (bounded at 2,000 entries) or compare it
after a hash hit.

### C9.4 — suggestion ranking

`suggest("Kisssa")` returns `["Kissaa", "Kisassa", "Kissa", …]`: the
keyboard-adjacent substitution `s -> a` beats the deletion that produces the
lemma. For an accidental repeat, the lemma is usually meant. Add a golden test
and decide a tie-break: lemma before an inflected form, or a preference for
deletion/insertion over a substitution that changes the inflection.

## Gates

Run after every slice where the change can affect the engine:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
python3 tools/test_binding_agrees_with_binary.py
python3 tools/test_reverse_lookup_recall.py
python3 tools/build/compare_engine_outputs.py --strict
python3 tools/test_illegal_evidence.py
```

Notes:

- The Python gates need the binding built first:
  `(cd bindings && env -u CONDA_PREFIX maturin develop --release)`.
- `test_illegal_evidence.py` is the slow gate; it is still the gate for the
  curated oracle exceptions in `data/oracle_exceptions.json`.
- Phase 2 additionally runs the benchmark suite (`python3 tools/bench.py run
  --profile quick` and `compare` against the reference) because it changes
  which forms the cheap checker accepts. The correctness fingerprint must
  match, or the diff must be explained as a fabricated-form removal.
- `cargo fmt --check` is deliberately not a gate. Do not let the formatter
  rewrite `src/expand/mod.rs` or `src/expand/build.rs`; they are hand-formatted
  around a few calls.
- If this plan is edited later, `python3 tools/check_docs.py
  misc_md/REVIEW_CORRECT.md` should stay green.

## Non-goals

- No new dictionary data, no morphology-coverage target, and no class
  promotion. This plan changes how existing paradigms are reached, not which
  forms they contain.
- No new dependency. The fuzz tests use a seeded LCG; the repo deliberately
  avoids `proptest`/`quickcheck`.
- No rewrite of the suggestion ranker beyond C9.4; the keyboard bonus and the
  curated correction table stay.
- No change to the grammar/style diagnostic contract beyond C7.3.

## Open questions

1. **Is `inflect_all(..., None)` part of anyone's public contract?** The
   pre-1.0 version allows a breaking rename, but the Python binding and the
   tools are in-repo and can be updated. Decide before C4.1.
2. **Which layer owns totality?** If the class engines promise total input, C3
   is an engine fix. If they promise a well-formed stem, the wizard must
   validate and so must the binding. The safest answer is both: validate at
   the user-facing entry point and make the generated stems fail closed.
3. **Which nominals files are generated?** The headers and `AGENTS.md`
   disagree, and C3 must edit the right side of that boundary.
4. **Where do the maps' provenance hashes belong?** The build fetches a
   force-pushed orphan branch at compile time. This review did not open that
   box, but a published crate would benefit from a pinned commit or a content
   hash in `REQUIRED`. It is out of scope here.

## Done when

- The three panic evidence tables (E1–E3) and the two case tables (E5, E6)
  are green in CI.
- The E4 table is `Unknown` on all four rows, and the class-50/51 parity test
  covers every lemma in those classes.
- The class matrix and the hyphenation fuzz run in the normal
  `cargo test --release --workspace` suite; a random-seeded variant is optional
  once the matrix covers every lexicon class.
- The standard gates in the previous section pass, and the benchmark
  correctness fingerprint is unchanged or the change is explained.
- Phase 4 and Phase 5 are either landed as separate commits or explicitly
  deferred with a line in `misc_md/TODO.md`.
