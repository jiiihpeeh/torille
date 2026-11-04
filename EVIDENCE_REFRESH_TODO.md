# Evidence refresh TODO

Status: **open.** The multiple-base work is otherwise green; this file covers
the one red gate, `tools/test_illegal_evidence.py`.

The gate compares the default per-class report against
`data/oracle_exceptions.json`. A rule that stops matching is stale and must be
removed; an `illegal` row that matches nothing is a new defect that needs
triage. Reproduce with:

```bash
python3 tools/test_illegal_evidence.py
```

Current result (2026-11-04): **7 stale rules**, 751 illegal rows, 155 missing,
3,880 unsupported. The same result reproduces with the tools from commit
`04434b2`, before the class-report filtering change, so this is an existing
baseline refresh rather than a regression from `MULTIPLE_BASE_FIX_TODO.md`.

After removing the six stale rules and fixing `kyy`, the gate now reports
**0 stale rules but 440 illegal rows with no evidence rule**. The removal was
correct, but it exposed the real remaining precision work: the default report
still has 745 illegal rows, and most are not covered by any rule.

## Current rule disposition

| rule | class | current report | disposition |
|---|---|---|---|
| `34c-nom-possessive-genitive-stem` | `34*C` | 0 illegal | remove; engine and oracle now agree |
| `plural-only-33d-short-imet` | `33*D` | 0 illegal | remove; engine and oracle now agree |
| `plural-only-33j-short-imet` | `33*J` | 0 illegal | remove; engine and oracle now agree |
| `plural-only-41c-short-aat` | `41*C` | 0 illegal | remove; engine and oracle now agree |
| `plural-only-41i-short-aat` | `41*I` | 0 illegal | remove; engine and oracle now agree |
| `lemma-32d-ien` | `32*D` | 0 illegal, 134 missing | remove from the **illegal** evidence file; the `ien` gap is a missing-direction item, not an illegal one. Track it in `TODO_MISSING.md` if it is still real. |
| `lemma-18-pii` | `18` | 6 illegal, all `kyy` | replace or fix. The rule names `pii`/`tii`/`pe-tsai`, none of which produce an illegal row now; `kyy` is a different lemma in the same class. |

`lemma-41e-havas` is **not** stale in the current report; keep it.

## Class 18 `kyy` rows

Current findings:

```text
kyy  kyisiin    expected kyihin   got kyisiin
kyy  kyisiinsä  expected kyihinsä got kyisiinsä
... 4 more possessive variants
```

Before adding an evidence rule, check whether this is an engine bug. Class 18
is the abbreviation/letter-name class; `kyy` should follow the same illative
pattern as the class's model word. If `kyihin` is the right form, fix the class
18 engine (`src/expand/nominals/class_18.rs`) rather than whitelisting the
wrong `kyisiin`. Only if Wiktionary/Kielitoimisto confirms `kyisiin` should the
rule be extended, with a `source` and `note` for `kyy`.

## Remaining unmatched rows after the first pass

The 440 unmatched rows from the n=5 gate cluster on a few classes:

| class | rows | lemmas |
|---|---:|---|
| `7*F` | 142 | `lahti`, `vyyhti` |
| `5*B` | 71 | `appi` |
| `7*B` | 71 | `appi` |
| `26` | 71 | `kuori` |
| `15` | 35 | `vihreä`, `kauhea`, `makea`, `korkea`, `pehmeä` |
| `50` | 26 | `villikaniini`, `kevytsinko` |
| `22` | 23 | `nougat`, `parfait`, `gourmet` |
| `49` | 1 | `autere` |

These are precision defects in possessive/plural stems, not evidence gaps. A
rule should not be added until the class engine is checked: `appi` has both a
`5*B` and a `7*B` row, and the engine emits the same `apeitta` for both while
the oracle confirms `apitta`; `lahti`/`vyyhti` and `kuori` have the same shape
of plural-possessive disagreement. The larger `-n 100` class-18 report also
shows 884 illegal rows outside the n=5 gate sample, so fixing only the gate's
sample is not enough.

## Tasks

- [x] Run `python3 tools/report_by_class.py -n 5 -j 8 -o reports/noun-classes-current.json`
      and confirm the totals above.
- [x] Remove the six stale rules listed above from
      `data/oracle_exceptions.json`.
- [x] Triage `kyy` class 18: the class-17 `isiin` alternant was plain for
      `kyy`; it is now rare (`kyisiin` remains accepted, not plain). The
      `lemma-18-pii` rule is therefore removed too.
- [ ] Fix the unmatched class defects above, starting with the shared
      possessive/plural-stem pattern in `appi` / `lahti` / `vyyhti` / `kuori`;
      then re-run the n=5 gate.
- [ ] Re-run `python3 tools/test_illegal_evidence.py`; it must exit 0, with
      every illegal row matching exactly one rule and no stale rules.
- [ ] Update `data/oracle_exceptions.json`'s `about`/version if the rule count
      or policy changes, and update `docs/harness.md` if it quotes the old
      rule set.
- [ ] Re-run the normal gates:
      `cargo test --release --workspace`,
      `cargo clippy --all-targets -- -D warnings`,
      `python3 tools/test_binding_agrees_with_binary.py`,
      `python3 tools/test_reverse_lookup_recall.py`.

## Acceptance

- `tools/test_illegal_evidence.py` exits 0.
- The default `report_by_class.py -n 5 -j 8` report has 0 illegal rows, or every
  remaining illegal row is explained by exactly one sourced rule.
- No stale evidence rule remains.
- The fix is in the class engine for `kyy` if the engine is wrong, not a new
  suffix shortcut in `is_known_word`.

## Related

- `TODO_ILLEGAL_REMOVAL_PLAN.md` — the older precision sweep and its evidence
  policy.
- `docs/harness.md` — finding kinds and known defects.
- `MULTIPLE_BASE_FIX_TODO.md` — the multiple-base work; its gate-status section
  points here.
