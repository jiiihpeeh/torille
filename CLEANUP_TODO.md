# Cleanup todo

Tasks identified during the `src/` review on 2026-10-12. Each item lists the
concrete defect, the smallest remediation, and the file/line range so the work
can be picked up without re-reading the diff.

- [x] **Remove the dead `harmonize` from the new crate-root `harmony` module.**

- [x] **Fix the inaccurate module doc on `src/harmony.rs`.**

- [x] **Resolve the `harmonize` (z) vs `harmonise` (s) split.** Auto-resolved
  by item 1: deleting the public `crate::harmony::harmonize` left
  `crate::grammar::harmonise` as the only public-ish entry point. The
  local `harmonize` closures in the generated class files
  (`class_50.rs:61`, `class_49D.rs:61`, `class_49G.rs:61`, `class_51.rs`)
  are a generator-internal pattern from
  `tools/oracle/extract_class_rules.py:321,324` and are out of scope for this
  cleanup. Decision recorded: don't reintroduce a public `harmonize`
  alias at the crate root.

- [x] **Decide on the `Builder::h` / `VerbBuilder::h` indirection.**
  Decision: inline the conditional. Reverted to
  `String::from(if self.front { front_twin } else { back })` in
  `src/expand/verbs/utility.rs:386` and
  `src/expand/nominals-refactored/utility.rs:486`. The free-function
  declension helpers in the same files (`singular_case`, `plural_case`,
  `passive_mood`) keep their `pick_harmony(...)` calls — the indirection
  is appropriate where there is no builder to carry the `front` flag,
  and inline is appropriate inside the `h()` method that already has
  `self.front` on hand.

## Verified clean

- `cargo build --features generated-nominals,generated-verbs` succeeds with
  no errors in the changed files. The 303 pre-existing warnings are all in
  generated per-class files and unrelated to this diff.
- `cargo test --features generated-nominals,generated-verbs --lib expand::`
  — 511/511 pass, including
  `expand::tests::the_two_nominal_trees_agree_form_for_form`, which proves
  the two `DISPATCH` tables and the authoritative / additive split still
  agree.
- `crate::harmony::front` is byte-identical to the `front` it replaced; the
  `loanword_harmony` re-root through `super::utility::front` →
  `crate::harmony::front` is behaviourally identical.

## Out of scope

- The pre-existing dual `crate::grammar::harmony` (returns `Harmony` enum)
  and `crate::harmony::front` (returns `bool`) modules coexist. Unifying
  them is a separate refactor and was not changed by this diff.
- The 128-entry `DISPATCH` table is duplicated byte-for-byte between
  `src/expand/nominals/mod.rs:300-428` and
  `src/expand/nominals-refactored/mod.rs:166-315`. The two-trees test guards
  drift, and the duplication is a known cost of the porting model
  (acknowledged in the `nominals/mod.rs:1-23` comment).
