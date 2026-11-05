# Evidence refresh TODO

Status: **closed (2026-11-04).** `tools/test_illegal_evidence.py` exits 0:
every `illegal` row matches exactly one sourced rule and no rule is stale.
The multiple-base work and the rest of the gates are green as well.

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

After removing the six stale rules, fixing `kyy`, filtering multi-class
reports, and adding the dual-class `appi`/`vyyhti` evidence, the gate reported
**0 stale rules and 50 illegal rows with no evidence rule** (down from 440).
The last 50 were then fixed at the class level, and the gate now exits 0 with
477 illegal rows, all explained by 46 live rules.

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

## The last 50 rows (resolved)

The 50 unmatched rows from the n=5 gate clustered on:

| class | rows | lemmas | resolution |
|---|---:|---|---|
| `50` | 26 | `villikaniini`, `kevytsinko` | sourced evidence: both are prefix + head, and the compound oracle missed the head's real e-plural / weak grade |
| `22` | 23 | `nougat`, `parfait`, `gourmet` | class engine: the consonant-final plural stays on the `i`-stem (`nougatilta`), plus the lexical illative vowel |
| `49` | 1 | `autere` | sourced evidence: Wiktionary's askel paradigm gives both `autereisiin` and `autereihin`; Voikko lists only the first |

`viisi`/`pommi` (class 99) and the class-15 adjectives were fixed earlier: the
class-99 delegations now use class 27/5, and the class-15 long partitive is
plain only for the 131 lemmas the oracle confirms.

## Tasks

- [x] Run `python3 tools/report_by_class.py -n 5 -j 8 -o reports/noun-classes-current.json`
      and confirm the totals above.
- [x] Remove the six stale rules listed above from
      `data/oracle_exceptions.json`.
- [x] Triage `kyy` class 18: the class-17 `isiin` alternant was plain for
      `kyy`; it is now rare (`kyisiin` remains accepted, not plain). The
      `lemma-18-pii` rule is therefore removed too.
- [x] Apply the class filter to every multi-class lemma, not only noun/verb
      homonyms; this removed the `lahti`/`kuori` cross-class rows.
- [x] Add sourced `appi` (5B/7B) and `vyyhti` (7F/5F) evidence rules.
- [x] Fix the remaining 50 rows: class 50 compounds (`villikaniini`,
      `kevytsinko`), class 22 loans (`nougat`, `parfait`, `gourmet`) and
      `autere`; then re-run the n=5 gate.
- [x] Re-run `python3 tools/test_illegal_evidence.py`; it exits 0, with
      every illegal row matching exactly one rule and no stale rules.
- [x] Update `data/oracle_exceptions.json` (`about`/version unchanged; the
      rule set grew to 46 live rules).
- [x] Re-run the normal gates:
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
