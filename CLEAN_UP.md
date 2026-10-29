# CLEAN_UP — engine `src/` cleanup plan

A working checklist for cleaning up the `torille` engine crate. Ordered by
risk/return: each phase is independently landable, and the later phases depend
on the earlier ones being green.

`src/` (the engine) is **not** the browser-extension source; that lives in
`extensions/firefox/src` and `extensions/chromium/src`. This file is only about the
crate at the repository root.

Baseline measured on 2026-10-23 (before this pass):

| check | result |
|---|---|
| `cargo check -p torille --all-targets` | **fails** (2 stale dev targets) |
| warnings, `cargo check -p torille` | **330** total, 314 from the lib |
| warnings in generated nominal/verb modules | ~300 |
| `#[allow(dead_code)]` occurrences | 454 (almost all generated files) |
| `#[path]` swaps into `nominals-refactored/` | 99 |
| classes still in the old `nominals/` tree | 53 |

After the first pass (P0–P2):

| check | result |
|---|---|
| `cargo check -p torille --all-targets` | **passes**, 0 warnings |
| `cargo clippy -p torille --all-targets -- -D warnings` | **passes** |
| `cargo check --workspace` | passes (15–25 warnings remain in other crates, depending on targets) |
| nominal trees | two → **one** (`nominals/`, ported shape) |
| `#[path]` swaps into `nominals-refactored/` | 99 → **0** |
| engine lib tests | 681 passed (old tree's duplicate tests removed) |

`tests/candidate_generation.rs` was green after the P5 split; its old
golden list (and `data/rerank/*.jsonl`) was regenerated for the
keyboard-adjacency tie-break, and the score assertion now allows the
documented 0.05 bonus. The engine cleanup itself did not change ranking.

## P0 — debris and broken targets

- [x] Moved the nested worktrees out of the source trees. Both were clean,
      detached checkouts: `src/.kilo/worktrees/walnut-toaster` (**298 MB**)
      and `extensions/chromium/.kilo/worktrees/petite-hamster` (36 MB). Removed
      with `git worktree remove` and pruned; the empty `.kilo/` directories
      are gone. `**/.kilo/` stays in `.gitignore` so fresh clones ignore any
      future agent worktrees.
- [x] Delete `src/bin/torille.rs` (the whole `src/bin/`); it is `fn main() {}`.
      The real binaries live in `tools/`.
- [x] Delete `examples/test-jooksee.rs`. It imports the private
      `kotus::WORD_ORDER` and is a scratch probe; it breaks
      `cargo check --all-targets`.
- [x] Fix `benches/torille.rs`:
      - `OnceLock<Catalogues>` vs `Catalogues::embedded() -> &'static Catalogues`.
        Use `OnceLock<&'static Catalogues>` or call `embedded()` directly.
      - remove the unused `Lemma as LemmaT` import.
- [x] Remove `[profile.release]` from `torille-wasm/Cargo.toml` and
      `torille-wasm-leptos/Cargo.toml`. Cargo ignores profiles in non-root
      packages; the root `[profile.release]` is the only one that applies.

## P1 — make the warning gates meaningful

- [x] Fix the hand-written warnings:
      - `src/expand/mod.rs:81` — unused `crate::kotus::Lemma` import
      - `src/expand/mod.rs:705` — unnecessary `mut`
      - `src/suggest.rs:671` — unused `STEM_BUCKETS_VERB` import
      - `src/suggest.rs:1654`, `:1764`, `:1765` — unnecessary `mut`
      - `src/expand/clitic.rs:122` — dead `clitic_variant_pair`
      - `src/expand/static_table.rs` — `RESTART` import, `StaticMap` alias
        (format constants `MAGIC`/`SET_MAGIC`/`HEADER` are used by the test
        encoders, so keep them with an `allow` rather than deleting them)
      - `src/map_reader.rs:81,156` — dead `restart_count` / `contains`
      - `build.rs:963,978` — unnecessary `mut`
      Also fixed the 55 Clippy lints this surfaced in hand-written code
      (needless borrows, collapsible `if`, `map_or` → `is_some_and`, etc.).
- [x] Silence generated-module noise at the module boundary, not per file:
      `#[allow(unused_imports, unused_mut, unused_variables,
      unused_attributes, duplicate_macro_attributes, non_snake_case,
      dropping_copy_types, clippy::all)]` on `mod nominals;`,
      `pub mod nominals_refactored;`, and `mod verbs;` in
      `src/expand/mod.rs`. `dead_code` stays as the per-file inner allow so
      the two do not conflict.
      Long term, teach `tools/extract_class_rules.py --rust` to emit these and
      drop the 454 per-file `#![allow(dead_code)]` attributes.
- [x] `cargo clippy -p torille --all-targets -- -D warnings` now passes.
      Enable it in CI. `cargo check -p torille --all-targets` is warning-free.

## P2 — stop compiling the refactored tree in release

- [x] Put `#[cfg(test)]` on `pub mod nominals_refactored;` in
      `src/expand/mod.rs`. It is referenced only by
      `expand::tests::the_two_nominal_trees_agree_form_for_form`, but is
      currently compiled into every release build (149 modules). The
      `#[path]`-swapped files under `nominals/` still compile in release, which
      is the intended state.
- [x] Keep the two-tree gate running under `cargo test`; only the standalone
      module is test-only.

## P3 — finish the two-tree consolidation (planned work)

- [x] Port the remaining 53 classes from `expand/nominals/` to
      `expand/nominals-refactored/` per `PLAN_REFACTOR.md`. The two-tree gate
      passed for every shared class before the swap.
- [x] Delete the losing tree: the old 53 per-class files, then move
      `nominals-refactored/` into `expand/nominals/`.
- [x] Replace the 206 `#[path]` declarations with ordinary `mod` declarations
      (the surviving `nominals/mod.rs` has none).
- [x] Retire the two-tree equivalence test, which would now compare each file
      with itself, and collapse
      `a_word_without_possessives_gets_no_possessive_column` to the single tree.
- [x] Copy `AUTHORITATIVE_CLASSES` / `is_authoritative` into the surviving
      `mod.rs` (the refactored tree never carried them).
- [x] Update `PLAN_REFACTOR.md` and `AGENTS.md` with the outcome.

## P4 — single sources of truth

- [x] `src/case.rs` and `docs/morfologia.yaml` are hand-synced copies with no
      drift test. Added `case::tests::the_yaml_spec_matches_the_case_table`,
      a dependency-free parser that compares every field of the
      `noun.cases` rows. It found two real gaps (translative and abessive
      `after_s_n` allomorphs); the YAML now matches the engine table.
- [x] `src/compound.rs` (split an unknown word) and `src/expand/compound.rs`
      (expand a compound lemma) share a name. Both now carry a module header
      that says which one is which and points at the other.
- [ ] `src/harmony.rs` and `src/grammar/harmony.rs` are intentionally different
      (primitives vs ending harmonisation). Keep both; grouping them under one
      `harmony/` directory is cosmetic and was not done.

## P5 — split the largest files (mechanical, review-heavy)

- [x] `src/suggest.rs` → `src/suggest/mod.rs` (1061 lines) plus
      `known.rs` (the lemma set and compound detector), `candidates.rs`
      (`suggest` / `suggest_detailed` / the pipeline), `state.rs`
      (`check_fast` / `suggest_with_state`) and `levenshtein.rs`. Every public
      path is re-exported from `suggest`, so callers and `lib.rs` are unchanged.
- [x] `src/expand/mod.rs` (1748 lines): the derived-form layer moved to
      `src/expand/derived_forms.rs` (compound heads, `-inen`, `-lainen`,
      agent nouns, numerals, adjective degrees). The pipeline and dispatch
      stay in `mod.rs`, which is still hand-formatted; `cargo fmt` was not run
      on it.
- [x] `build.rs` (393 lines): the two code generators moved to
      `build/sanalista.rs`, `build/front_coded.rs` and `build/huffman.rs`.
      They are reached with `#[path = "build/…"]` because a build-script
      module would otherwise resolve in the repo root, not `build/`.
- [x] **Did not split** `src/grammar/noun.rs` or `src/grammar/verb.rs`:
      TODO A/B/C are still open and the harness is the guard.

## P6 — API surface

- [x] Modules with no consumers outside the crate are now `pub(crate)`:
      `case`, `gradation`, `harmony`, `wordset`. `pronoun` stays `pub` because
      it is a documented closed-paradigm layer (only its own tests use it
      today). Removed the unused duplicate `case::Harmony` enum this exposed.
- [x] Keep `tokenise`, `derivation`, `compound` public (used by `tools` /
      `torille-wasm`).
- [x] Keep `suggest::check_fast` and `suggest::suggest_with_state` public:
      `torille-wasm` uses them.

## P7 — process

- [x] Add `rust-toolchain.toml` (pins 1.99.0, edition 2024, clippy + rustfmt).
- [x] CI: `.github/workflows/ci.yml` runs `cargo check --workspace
      --all-targets`, the engine clippy gate, the lib tests, `catalogue` and
      `candidate_generation` and `rerank_parity`. The Python harness is
      separate (needs Omorfi/Voikko/maturin).
- [ ] `cargo fmt --check` is deliberately **not** a gate yet: generated
      modules are not rustfmt-clean, and `src/expand/mod.rs` /
      `src/expand/build.rs` are hand-formatted.
- [x] Add `**/.kilo/` to `.gitignore` so a fresh clone does not name nested
      worktrees as content (they are currently excluded only in the local
      `.git/info/exclude`).

### Size after P5

| file | before | after |
|---|---|---|
| `src/suggest.rs` | 3007 | `suggest/mod.rs` 1061 + `candidates` 418, `known` 988, `state` 161, `levenshtein` 390 |
| `src/expand/mod.rs` | 2446 | 1748 + `derived_forms.rs` 635 |
| `build.rs` | 1371 | 393 + `build/` 1026 across three modules |

Public paths are unchanged (`suggest::*`, `expand::derived_*`,
`expand::modifier_is_known`); the lib test count is still 681.

## Notes from the first pass

- Touched engine files were formatted with `rustfmt --edition 2024` *except*
  `src/expand/mod.rs` and `src/expand/build.rs`, which AGENTS says must stay
  hand-formatted. If the formatting churn is unwanted, revert it separately
  from the logical fixes.
- The generated-module `#[allow]` is deliberately broad (`clippy::all`). The
  sustainable fix is in `tools/extract_class_rules.py --rust`; until then the
  allow is scoped to `expand::{nominals, verbs}` and does not hide anything in
  hand-written code.
- The two-tree retirement removed the old tree's duplicate `mod tests`; the
  lib test count dropped from 796 to 680 for that reason, not from lost
  coverage. The ported files' own tests, the fixture and `tests/catalogue.rs`
  (26 cases) all pass.
- `cargo check --workspace` still reports warnings (15 without `--all-targets`,
  25 with) from the other crates (`torille-wasm`, `torille-wasm-leptos`, ...).
  `tools` is now warning-free under
  `cargo clippy -p torille-tools --all-targets -- -D warnings`.


## Tools pass (2026-10-24)

- [x] Remove local `__pycache__`/`*.pyc` (already ignored by git).
- [x] `pyflakes tools` is clean: unused imports/locals removed, and the
      uninitialised `forms` use in `build_proper_name_forms.py` fixed.
- [x] `cargo clippy -p torille-tools --all-targets -- -D warnings` is clean.
- [x] Make the one-off `wait_and_post.sh` find the repository root instead of
      hard-coding `/home/j-p/torille`.
- [x] Fold the oracle patch scripts into the builders that own the output:
      `update_nomine_forms.py` -> `build_nomine_lexicon.py --engine`;
      `update_verb_forms.py` -> `build_verb_lexicon.py --engine`;
      `fill_verb_gaps.py` -> the verb grid's comitative/negative-mood cells;
      `extend_verb_connectives.py` and `build_verb_connegatives.py` -> the
      verb builder's post-passes. The five standalone scripts are gone.
- [x] Add `tools/README.md` with the binary/harness inventory, run conventions,
      and the license scope.
- [x] License split: engine, bindings, browser code and most tools are
      `MIT OR Apache-2.0`; only the files tied to Omorfi or Voikko are
      `GPL-3.0-or-later`, with SPDX headers. `data/` remains separate.

## What not to "clean up"

- Do **not** hand-edit `expand/nominals*` or `expand/verbs`; regenerate.
- Do **not** simplify `is_known_word` in `src/suggest/known.rs`; AGENTS is explicit
  that it is a bypass and the fix belongs in `find_bases_with` / the class
  engine.
- Do **not** touch stem/possessive behaviour as part of a cleanup commit; TODO
  A/B/C need harness runs and baseline checks.
