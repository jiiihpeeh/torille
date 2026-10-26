# Plan: replace the lcp-derived stem with a genitive-anchored class stem

## Goal

Kill the recurring `lcp_len` **stem-undercount** bug at its source.

`variant_of` (and `past_stem`) infer the model's stem per form with
`lcp_len(model, form)`, which stops one letter short whenever a form changes
the stem's final consonant:

```
nainen   n a i n e n
naisia   n a i s i a
         ^^^^^   <- lcp stops at "nai" (3), but the real stem is "nais" (4)
```

Cutting the lemma at that short length truncates it: `aakkonen` cut to `kuu`
+ `sia` gives the garbage `kuusia` instead of `aakkosia`. This one flaw has
surfaced repeatedly:

- `aakkonen` / `aakkoset`: whole plural oblique missing (`aakkosissa`,
  `aakkosilla`, ...) because the plural stem is read as garbage.
- verb `past_stem`: conditional base undercounted (`teroittisi` for
  `teroittaisi`).
- the `-ta/-tä` and type-1 conditional and participle stems.

## Principle

A stem is a **property of the class**, not of any one form. It should be read
once from the model's **genitive** (the canonical stem), never re-derived per
form with lcp. `noun.rs` already models stems this way (`desc.stems` /
`desc.genitive`); the refactor makes `variant_of` agree with it instead of
duplicating the fragile lcp heuristic.

## Architecture

### Today (two divergent systems)

```
inflect.rs::variant_of          noun.rs (oblique)
  stem = lcp_len(model, form)     stem = class table (desc.stems/desc.genitive)
  -> undercounts -> truncates     -> correct, class-anchored
```

### After

```
variant_of:
  1. model_stem  = model genitive minus its -n ending, computed once
     ("naisen" -> "nais")
  2. ending      = slot form aligned against model_stem, where the stem's
     final consonant may change (the s of "naisia" is part of the stem)
  3. lemma_form  = lemma's genitive stem + ending
     ("aakkos" + "ia" -> "aakkosia")
```

- The plural stem is a distinct stem (`nais` vs the singular `naise`) and is
  read from the model's plural partitive the same way, not by stripping a
  marker-length off an arbitrary form.
- `past_stem` adopts the same genitive-anchored stem so verbs agree with nouns.

## Harness / verification

- `python3 tools/check_nouns_with_omorfi.py` — full catalogue must stay **0
  missing**; the "not offered" count must not regress.
- `python3 tools/check_verbs_with_voikko.py` — 933 plain forms, **0
  unrecognised**.
- `cargo test -p torille` — all lib + integration tests.
- Add regression tests: `aakkonen`/`aakkoset` plural oblique
  (`aakkosissa`, `aakkosilla`, `aakkosina`, `aakkosiksi`),
  `nainen` plural unchanged (`naisissa`, not `naisiissa`), `teraittaa`
  conditional `teroittaisi`, `aakkonen` plural partitive `aakkosia`.

## Steps

1. **Read the model's genitive stem once** in `variant_of` (genitive slot minus
   its `-n` ending) as the canonical stem, replacing the per-form `lcp_len`.
2. **Align each slot's ending** against that stem, allowing the stem's final
   consonant to change (the divergent consonant is part of the stem, not the
   start of the ending).
3. **Read the plural stem** from the model's plural partitive the same way, and
   use it for the plural oblique.
4. **Apply to `past_stem`** so verbs share the same genitive-anchored stem.
5. **Add the regression tests** above.
6. **Run all harnesses and the full catalogue**; fix any class that regresses
   (this touches every class's lexical cases, so expect to iterate).


## Risks / watch-outs

- `variant_of` is generic across all 8 slots and every class; a wrong stem
  length will ripple through the lexical cases. Regression-test the whole
  catalogue, not just the fixed words.
- The plural stem differs from the singular stem; do not reuse the singular
  stem for plural slots.
- Some classes have multiple stem readings (class 7 `ovi`/`joki`); the
  genitive-anchored stem must not collapse the alternatives the class legitimately
  offers.
- The earlier `marker_len`-only fix broke `nainen` (`naisiissa` for `naisissa`)
  because the plural oblique *adds* the `i`; the plural stem must stay the
  consonant stem `nais`, not `naisi`.

## Outcome

Done. Steps 1-4 landed as `class_stem` (`grammar::noun`), `class_plural_stem`
and `plural_stems` (`expand::inflect`); `past_stem` reads its stem the same way.

Step 5 found one case the plan had not anticipated, and fixing it needed a
second bug fixed underneath. `aakkoset` ended in a consonant, so the class's
stem rule could not do what it does for the model: class 38 cuts `nainen` to
`naise` by turning the last `n` of the `nain` it keeps into an `s`, and
`aakkos` has no `n` there to change. `apply_stem` copes with that — its
substitution is a no-op when the letter is absent, and the cut and the added
`e` still stand, giving the right `aakkose`. But `rule_applies` rejected the
reading outright, on the reasoning that the rule "mirrors that replacement one
for one", which it did not: the guard was stricter than the code it claimed to
describe. The reading was therefore thrown away, `class_stem` fell back to the
whole lemma, `class_plural_stem` never fired, and the word had no plural
oblique at all — every plural case came out on `aakkoset-` instead of
`aakkos-`. The guard now only rejects when the substitution is the rule's
*whole* edit, which is the one case where nothing is left to do.

Harnesses, before and after, unchanged:

- `check_verbs_with_voikko.py` — 933 plain forms, **0 unrecognised**.
- `check_nouns_with_omorfi.py` — 740 Voikko-confirmed forms, **0 missing**,
  **41 generated but not offered** (no regression).
- `aakkoset` on its own: 14 Omorfi forms, 0 missing, 0 not offered.
- `cargo test` — 141 lib + 26 integration.

Regression tests added in `expand::tests`:

- `the_plural_oblique_is_built_on_the_class_plural_stem` — `aakkonen`
  plural oblique and partitive.
- `a_consonant_final_member_takes_the_same_plural_stem` — the same for
  `aakkoset`. This is the one that fails without the guard fix; the other
  two pass either way, since they hold the behaviour this plan was written to
  keep.
- `the_plural_oblique_does_not_take_the_singular_i` — `naisissa` and not
  `naisiissa`, the watch-out above.
- `teroittaisi` was already pinned in `grammar::verb::tests`.
