# PLAN_SYNTAX — making the advisory syntax layer trustworthy

Status: audit and fixture complete; Phase 1 landed (S1, S3, S4; S2 partial).
Work happens on the `syntax` branch off `master`.

The syntax layer (`src/syntax.rs`) is the second, on-demand layer over the
morphology engine. It is **not** in the keystroke path; its only shipping
consumer is `torille_wasm::analyze_syntax` (`torille-wasm/src/lib.rs:198`).
The sentence layer (`src/sentence.rs`) is separate and stays untouched.

The goal of this branch is not a full parser. It is to make the output a
caller can trust for the cases the feature is for: highlighting subjects,
objects and predicates, resolving the lemma of an ambiguous surface, and
splitting a sentence into clauses. Where morphology alone cannot decide, the
existing contract is to say `Unknown` rather than guess. That contract stays.

## Reproducing the audit

```bash
cargo test --release -p torille --lib syntax::   # 24 passed, ~30 s
cargo run --release -p torille --example syntax_probe
SYNTAX_READINGS=1 cargo run --release -p torille --example syntax_probe
cargo run --release -p torille --example syntax_probe -- "Hän tuli tupaan, jossa roihusi tuli."
```

`examples/syntax_probe.rs` prints clauses, roles, confidence, case, head, lemma
and (with `SYNTAX_READINGS=1`) every morphological reading. Its default corpus
is `examples/syntax_probe_sentences.txt`. The corpus is the seed of the
regression fixture in phase 0.

## What the layer does today

- One `Clause` per sentence, split at subordinators, relative pronouns,
  `ja joi`-style coordination, a comma before a finite verb, and infinitives
  (`segment_clause_words`, `syntax.rs:668`).
- A finite predicate, a nominative subject, an object, a copula/change-verb
  complement, an existential, a passive, a necessity construction, coordinated
  subjects/objects, modifiers, and adverbials from local cases.
- `Clause.negative` and `Clause.existential` flags.
- `RoleConfidence::{Certain, Likely, Weak}`. `Certain` is currently never
  produced; `Likely` and `Weak` carry the whole contract.

The 24 tests cover the happy paths of those rules. The audit below covers the
paths they do not.

## Audit findings

Ordered by how much they corrupt downstream output, not by difficulty.
`S<n>` ids are for commit messages and test names.

### P0 — reading fidelity (every later rule inherits these)

**S1. The base search inflects every lemma as a noun.**
`find_bases_with_inner` sets `let pos = PartOfSpeech::Noun`
(`src/expand/find_bases.rs:769`, and the same default at `:525` and `:635`).
`inflect_all` therefore tags generated forms with nominal parts of speech
even when the lemma is an adverb, conjunction or pronoun. `resolve_word`
keeps a non-`None` part of speech (`syntax.rs:304`), so:

| surface | sanalista lemma | reading today | expected |
|---|---|---|---|
| `eilen` | `eilen` adverbi | noun nominative | adverb |
| `kun` | `kun` konjunktio | noun nominative | conjunction |
| `missä` | `missä` pronomini | noun nominative | pronoun inessive |
| `sinä` | `sinä` pronomini | noun nominative | pronoun nominative |
| `hän` | `hän` pronomini | noun nominative | pronoun nominative |

This is why `Eilen`, `Kun`, `Missä`, `älä` and `hän` become subjects or
unknowns. It also strips the pronoun/adverb cue that `classify` needs.

Fix direction: when a reading comes from a `SourcedForm`, replace its part of
speech with the source lemma's own part of speech (`lemma_pos(source.lemma)`)
rather than filling only when `None`. Do not change `find_bases` itself in
this phase: suggestion ranking reaches it, and the hard-coded `Noun` is
intentional there (surface generation ignores POS for most classes). Keep the
correction at the syntax boundary and cover it with the table above.

**S2. Proper names lose their case.**
Names are not in the sanalista. `resolve_word` fails to find a base, then the
capitalised fallback in `analyze_with` (`syntax.rs:177`) installs a synthetic
**nominative** reading (`proper_name_reading`, `syntax.rs:579`). Every name is
therefore a nominative nominal:

| surface | expected | today |
|---|---|---|
| `Helsingissä` | inessive adverbial | noun nominative, often subject |
| `Matin` | genitive modifier | genitive only when the common-word `matti` happens to resolve |
| `Liisalle` | allative recipient | noun nominative, often object |
| `Helsingissä asuva mies` | `Helsingissä` adverbial, `mies` subject | `Helsingissä` subject |

`data/proper_name_forms.csv` is generated from Omorfi's singular case cells
(`tools/oracle/build_proper_name_forms.py`, `CASES = [NOM, GEN, PAR, ...]`), but
the emitted table is `(form, name)` only; the case is discarded. Fix direction:
emit `(form, name, case)` and add a `proper_names::case(form)` lookup (a small
parallel map is enough), then prefer that reading over the synthetic
nominative. This is a data + build-script change and should be its own commit.

**S2 interim (Phase 1).** The syntax layer now uses the name table's lemma
(`Helsingissä` -> `Helsing`) and assigns the nominative only when the surface
equals that lemma. An inflected name therefore has a noun reading and no
case, which keeps it out of subject/object selection instead of inventing a
nominative. This fixes `Matin auto` and stops `Helsingissä` becoming a
subject, but it does **not** recover the inessive/allative.
`proper_name_adessive` and `recipient_vs_theme` remain defect rows for that
reason.

**S3. Clitics are not resolved in the syntax layer.**
`find_bases_with_form_filter` peels a clitic to find the base but retains no
surface form for `onko`, so `resolve_word` falls through to the direct-lemma
fallback and produces a bare `Verb` reading with no mood:

```
Onko  verb, mood None   -> not a predicate
```

`Eikö` works only because `negation_base` handles `-kö` on the auxiliary.
`Onko sinulla kirjaa?` then chooses `kirjaa` (the verb `kirjata`) as the
predicate and calls the clause negative. Same failure for `Onko`, `Eikö hän`,
`Tuleeko`, `Menetkö`.

Fix direction: in `resolve_word`, if the ordinary resolution yields no mood
(or no reading at all), strip one clitic with
`expand::clitic::strip_clitic`, resolve the host, and copy its readings while
recording the clitic. Do not generalise this into a suffix rule; use the
existing clitic table.

**S4. Connegative readings and clause negation are conflated.**
A connegative verb form legitimately carries `Polarity::Negative` as a word
(`syö` in `en syö`, `mennä` in `ei mennä`), and the engine emits that reading
for the bare surface. `resolve_clause` sets

```rust
clause.negative = ... || predicate.readings.iter().any(polarity == Negative)
```

(`syntax.rs:805`), so **affirmative** clauses become negative:

| sentence | today | expected |
|---|---|---|
| `Syö puuro!` | negative, `puuro` not an object | affirmative, object |
| `Anna kirja minulle.` | negative, `kirja` not an object | affirmative, object |
| `Hänen pitää mennä.` | `mennä` negative | affirmative |

The negative form in a clause is the **auxiliary** (`en`, `ei`, `älä`,
`eikö`, fused `enkä`), not a connegative main verb. `älä` is currently not
recognised as a negative auxiliary at all (`negation_base("älä")` is `None`),
so `Älä syö puuroa` makes `Älä` a nominative subject.

Fix direction: `clause.negative` is true iff the clause contains a token with
a `NegativeVerb` reading. Add `älä` (and the imperative plural `älkää`) to the
closed negation table or to a syntax-local closed set. Do not use a
connegative reading as the trigger.

### P1 — clause segmentation

**S5. Only the citation form of a relative pronoun starts a clause.**
`is_relative` matches `joka`, `mikä`, `ken` (`syntax.rs:726`). Inflected
relatives are ordinary forms:

- `jossa`, `jonka`, `jota`, `josta`, `johon`, `jolla`, `jolta`, `jolle`,
  `jotka`, `joiden`, `joita`, ...
- `mitä`, `minkä`, `missä`, `mistä`, `mihin`, ...
- `kuka`, `kuten` (when relative), ...

In the audit sentence the clause is not split at `jossa`:

```
Hän tuli tupaan, jossa roihusi tuli.
  one clause; predicate tuli; jossa adverbial head=tuli; the second tuli
  is also a Predicate with lemma tulla
```

Expected:

```
clause 0: Hän(subject) tuli(predicate, tulla) tupaan(adverbial, illative)
clause 1: jossa(adverbial, joka) roihusi(predicate, roihuta) tuli(subject, tuli)
```

Fix direction: segment on the **lemma** when the token has a pronoun reading
(`out[index].lemma` is `joka`/`mikä`/`ken`/`kuka`), plus a closed surface set
for the pronouns whose forms do not reach the base (`mitä`, `missä`). Keep
the set in one function, `is_relative_word(index, out)`, and test every case
form.

**S6. `että` is both a subordinator and the second half of `sekä ... että`.**
`Sekä Sami että Liisa tulivat.` splits at `että`, leaving `Sami` in a
predicate-less clause and making `Liisa` the only subject. Fix direction: do
not start a clause at `että` when the clause already contains `sekä` before
it (or when the previous word is a coordinated nominal and no finite verb
precedes). Treat `joko ... tai` the same way; `joko`, `sekä` and the pair
markers currently become subjects (`Joko` is subject in the audit).

**S7. Finite/infinitive ambiguity changes segmentation.**
`classify` returns `Unknown` when **any** reading is an infinitive
(`syntax.rs:495`), even when a finite reading also exists. `pitää` and
`näyttää` are both infinitives and finite 3sg forms. Consequences:

- `Hänen pitää mennä.` — `pitää` is not a predicate candidate during
  segmentation, so `mennä` never starts a clause; later `choose_predicate`
  promotes `pitää` through the infinitive fallback anyway.
- `Hän näyttää väsyneeltä.` — `näyttää` is promoted by the same fallback,
  not because it has a finite reading.

Fix direction: do not discard the finite readings. Keep the infinitive and
finite readings, and let `choose_predicate` prefer the finite one when the
clause already has a subject or the word is clause-initial. The
infinitive-only fallback (`syntax.rs:760`) remains for genuinely non-finite
clauses (`Haluan syödä`, `Sami haluaa syödä puuron`).

### P2 — predicate and auxiliary resolution

**S8. The negative auxiliary remains a second predicate.**
`En syö puuroa.` marks both `En` and `syö` as `Predicate`; `Sami ei syö`
marks `Sami`'s `ei` as a weak predicate too. `choose_predicate` excludes a
bare negative auxiliary when a main verb exists, but nothing demotes the
auxiliary's already-assigned role.

Fix direction: decide the API shape first. Either

- add `SyntacticRole::Auxiliary` and mark the negation auxiliary (and the
  `olla` of a compound tense) as auxiliary with `head = main predicate`; or
- keep the single `Predicate` role but make `Clause.predicate` the **phrase
  head** and demote every other predicate candidate with no coordinating
  conjunction to `Unknown`/nominal.

The second is less API churn but loses the auxiliary in the token stream.
Recommendation: add `Auxiliary`; the wasm JSON already passes `role.name()`
and consumers can ignore an unknown name.

**Phase 1 landing.** The demotion half is in: after the clause predicate is
chosen, any other finite reading that also has a nominal reading is demoted to
`Unknown`, so `kirjaa` in `Luen kirjaa` can be the object again. Negative
auxiliaries are deliberately excluded, so `En syö` still has two `Predicate`
tokens until phase 3 adds `Auxiliary`.

**S9. `olla` + participle is always a complement.**
`Olen lukenut kirjan.` produces `Olen` predicate, `lukenut` complement,
`kirjan` unknown. In a compound tense the participle is part of the predicate,
and the object belongs to the whole phrase.

Fix direction: classify the `olla` + participle construction by voice:

- active past participle (`lukenut`, voice active, nominative) -> compound
  tense; predicate covers both tokens, then search for the object after it.
- passive past participle (`luettu`, voice passive) -> passive construction
  (`Kirja on luettu`), logical object before the auxiliary.
- present/agent participle (`on mentävä`) -> necessity complement (already
  handled).
- anything else after `olla` -> copula complement (already handled).

**S10. Necessive/modal genitive subjects are missing.**
`Minun täytyy mennä.` and `Hänen pitää mennä.` leave the genitive pronoun
`Unknown`; `täytyy`/`pitää` get no subject, and the infinitive clause inherits
none. The existing necessity rule only fires for `olla`
(`Minun on mentävä`).

Fix direction: a closed set of necessive predicates (`täytyä`, `pitää`,
`kannattaa`, `sopia`, `olla pakko`, `olla paras`) takes a genitive subject.
Prefer a small table in `syntax.rs`; do not teach the morphology engine a
new case rule. Once the genitive is the subject, `share_clause_roles` gives it
to the following infinitive clause (`lukea`, `mennä`) as it already does for
`Sami haluaa syödä`.

**S11. The infinitive fallback can promote the wrong word.**
`Onko sinulla kirjaa?` promotes `kirjaa` (verb `kirjata`, imperative) because
`Onko` has no mood. This is S3 plus the fallback. After S3 the fallback should
also require that the token has no finite reading with a subject-compatible
person, and that no earlier predicate exists in the clause.

`Eikö hän tullut?` also has two predicate candidates (`Eikö`, `tullut`). The
negative auxiliary is the finite head; `tullut` is the participle. Fold with
S8/S9.

### P3 — nominal roles

**S12. Subject selection ignores case and existing roles for proper names.**
`resolve_clause` tries `before.iter().find(is_proper_name)` before the
nominative search (`syntax.rs:843`). `is_proper_name` checks only
capitalisation, a noun reading and the name set — not case and not whether the
token already has a role. Therefore:

- `Matin auto on punainen.` -> `Matin` (genitive) is subject, `auto` unknown.
- `Helsingissä asuva mies on opettaja.` -> `Helsingissä` is subject.
- `Vanha talo Helsingissä on kaunis.` -> `Vanha` is subject (the name table
  also contains the surname `Vanha`) and `Helsingissä` becomes a modifier
  through the synthetic nominative reading of S2.
- `Kun hän tuli, söin.` -> `Kun` is subject (the name table contains `Kun`).

Fix direction: a subject candidate must be nominative/partitive, must not
already be `Adverbial`/`Modifier`/`Object`, and a genitive can only be a
subject in the necessive construction (S10). Do not special-case a proper
name before the ordinary nominative scan; use the same case gate, with the
name only as a tie-breaker.

**Phase 1 landing.** The case-blind shortcut is gone, and every candidate
search now requires `role == Unknown`. A capitalized lemma with a
nominative/partitive case is the only name tie-breaker (`Sami` over
`Ruotsalainen`; `Vanha` has a lowercase lemma). `Matin auto` and
`Vanha talo` now produce the right subject. The general rules (genitive
subject, recipient ranking, existential word order) remain phases 3–4.

**S13. Postpositional locatives do not count as existential locatives.**
`Pöydän alla on kissa.` currently makes `alla` the subject (S1 gives it a noun
reading) and `kissa` a complement. Even after S1, `alla` is an adverb with no
case, so `locative_before` (which requires `case.is_some_and(is_local_case)`)
stays false and the existential is missed. The same positional rule misses
`Onko sinulla kirjaa?`, where the locative follows the predicate; that leaves
`clitic_question` as a defect row even after the clitic itself resolves.

Fix direction: an adverbial whose lemma is a postposition/adverb in the local
set (`alla`, `alla`, `päällä`, `sisällä`, `luona`, `vieressä`, ...) counts as
the locative. Prefer a small closed set or the `PartOfSpeech::Postposition` /
`Adverb` reading, not a suffix rule.

**S14. Numeral phrases pick the counted noun as the head.**
`Hänellä on kaksi lasta.` makes `kaksi` subject and `lasta` complement; the
partitive `lasta` also has a spurious nominative reading (`lasta` = splinter),
which `best_case` prefers.

Fix direction: when a numeral (`kaksi`, `kolme`, `monta`, ...) is followed by
a partitive nominal, the numeral is the subject/object and the counted noun is
a `Modifier` with `head = numeral` and `case = Partitive`. Use the numeral's
POS (S1) and the partitive reading, not the first case in the array.

**S15. Valency complements are only recognised after `olla` and `tulla`.**
`Hän näyttää väsyneeltä.` -> `väsyneeltä` is adverbial (ablative); expected
complement. `Talo maalattiin punaiseksi.` -> `punaiseksi` stays unknown;
expected complement (resultative translative).

Fix direction: a small predicate-valency table for the productive cases:

- `näyttää`, `tuntua`, `vaikuttaa`, `kuulostaa`, `haisahtaa`, `maistua` +
  ablative -> complement.
- `maalata`, `värittää`, `kutsua`, `nimittää`, `valita`, `tehdä` +
  translative -> complement.
- `pitää` + essive/partitive (`pitää häntä hulluna`) -> complement.

Keep it a table in `syntax.rs`; do not rewrite the case classifier. The
adverbial-case fallback remains for verbs the table does not know.

**S16. The first nominal after the predicate becomes the object, even when it
is a recipient.**
`Sami antoi Liisalle kirjan.` makes `Liisalle` the object (and, via S2, calls
it nominative) and leaves `kirjan` unknown. `Sami antoi kirjan Liisalle.` gets
the object right but leaves the recipient unknown. After S2, the allative will
at least be an adverbial; the object search must still skip allative/ablative
recipients and prefer the genitive/partitive theme.

Fix direction: object selection ranks cases (accusative/genitive > partitive >
nominative) and skips allative/ablative when a later theme candidate exists.
Recipient marking can stay `Adverbial` with a `head` of the predicate; a
dedicated `IndirectObject` role is out of scope for this branch.

**S17. Subject sharing is limited to coordination and non-finite clauses.**
`Sami söi puuron, koska oli nälkäinen.` leaves the second clause subjectless;
`Hän sanoi, että tulee.` likewise. Finnish drops a repeated subject in
subordinate clauses. But `Kun hän tuli, söin.` must **not** inherit `hän`
into `söin`.

Fix direction: share the previous clause's subject when the new clause has no
subject, the previous clause is in the same sentence, and the new predicate
agrees in person/number (third person singular/plural, or a non-finite
predicate). Do not share when the predicate is first/second person and the
previous subject is third. This needs `Person`/`Number` on the selected
reading, not just the lemma.

**S18. Function words leak into nominal roles.**
`Kun`, `Missä`, `Eilen`, `Joko`, `Sekä`, `ja`, `että` all have nominal
readings from S1 and can become subjects, objects or modifiers. `classify`
turns `ja` into `Unknown` only because it has no case; the coordination code
then still lets it be a nominal in some paths.

Fix direction: one `is_function_word(index, out)` gate for conjunction,
subordinator, relative, postposition, and the closed adverb/pronoun set. Use
it in subject/object/complement/modifier selection. Reuse the existing
`is_clause_marker` and extend it, rather than adding a second list.

**S19. Clause and token API gaps.**
`Clause` has `predicate`, `subject`, `object` but no `complement` or
`adverbials`, even though tokens can carry those roles. `RoleConfidence::
Certain` is never constructed. Decide before phase 4:

- add `Clause.complement: Option<usize>` and maybe `Clause.adverbials:
  Vec<usize>`; or
- document that only the core three are on the clause and consumers must scan
  tokens for the rest.

Either is fine; leaving an unused `Certain` while claiming a confidence model
is not. Recommend: add `complement`, keep `adverbials` out (they are many and
order matters), and either produce `Certain` for the unambiguous finite
predicate/subject cases or remove the variant in a breaking-change commit.

**S20. A sentence costs seconds, not milliseconds.**
The audit measured `syntax::analyze` on the probe corpus:

- one five-word sentence: about 1–2 s wall, about 10 s CPU;
- the 32-case corpus: about 45 s wall and 8 min CPU even with 16 test
  threads.

`resolve_word` calls `find_bases_with_form_filter` for every unique surface.
That entry point collects the **suggestion** payload — every generated form of
every head/tail/prefix candidate plus its neighbours — while the syntax layer
only needs the readings of the one surface. On an on-demand UI this is not
affordable for a paragraph, and it makes the regression suite expensive enough
that defects will be found late.

Fix direction: resolve with a cheaper recognition path
(`find_bases_for_check`) and inflect only the winning base, or add a
per-document cache in the wasm layer. Measure before and after on
`examples/syntax_probe_sentences.txt`; the target is tens of milliseconds per
sentence, not seconds. Do not make the keystroke path depend on this layer.

## Phases

Each phase is a separate commit with tests. Do not mix a data regeneration
with a role-rule change.

### Phase 0 — fixture and tooling (done on this branch)

- [x] `tests/fixtures/syntax_cases.json` holds the expected output: sentence,
      clause count, per-word role/confidence/case/lemma/head, and the
      clause-level `predicate`/`subject`/`object`/`negative`/`existential`.
- [x] `tests/syntax_corpus.rs` is the runner. A case with `defect` must
      currently mismatch at least one assertion, and the suite fails once it
      matches all of them, so an xfail cannot go stale.
- [x] One `#[test]` per case (macro at the bottom of the runner), so the
      expensive morphology runs in parallel and a failure names the sentence.
- [x] 34 cases: 18 pin behaviour that is already correct, 16 are
      defect-marked gates for the remaining phases.
- [x] `examples/syntax_probe.rs` remains the human view over a wider corpus,
      including sentences the fixture does not pin yet.

The fixture is now the gate; do not weaken a defect row to make a phase land.
When a fix turns a row green, remove its `defect` field in the same commit.

### Phase 1 — reading fidelity (S1–S4) — landed, S2 partial

- [x] Correct the part of speech from the source lemma in `resolve_word`.
      `reading_with_pos` overrides the noun default with the `SourcedForm`
      lemma's own part of speech.
- [x] Resolve a clitic through its host and preserve the host's readings.
      `onko` is now a finite predicate with lemma `olla` and mood indicative.
- [x] Add `älä`/`älkää` (and the other imperative auxiliaries) to the closed
      negation set.
- [x] Make `clause.negative` depend on a `NegativeVerb` token only.
- [x] Proper names: use the name table's lemma and only give a name the
      nominative when the surface is the citation spelling. An inflected name
      is case-less, so it cannot become a subject or object by accident.
- [x] Replace the case-blind proper-name subject shortcut with a case-gated
      tie-breaker for a capitalized lemma (`Sami` wins over `Ruotsalainen`,
      while `Vanha` has a lowercase lemma and does not).
- [ ] Emit the proper-name case column and replace the case-less reading.
      Deferred: regenerating `data/proper_name_forms.csv` needs a batched
      Omorfi run. Measured, 10 names take 7.3 s, so the current 53k names
      would take about 11 hours. This is a tooling task, not a syntax rule.

Acceptance now passing: `sentence_adverb` (`Eilen`), `subordinator_subject`
(`Kun`, `hän`), `imperative_affirmative`, `genitive_modifier_subject`
(`Matin`), `clitic_verb` (`Onko`), `negative_imperative` (`Älä`).
`clitic_question` still needs the existential word order of S13, not the
clitic rule.

### Phase 2 — clause segmentation (S5–S7)

- [ ] Segment on relative lemma and the closed inflected relative set.
- [ ] Neutralise `sekä ... että` / `joko ... tai`.
- [ ] Keep finite readings beside infinitive readings; only fall back to an
      infinitive when the clause has no finite candidate.
- [ ] Add the user sentence `Hän tuli tupaan, jossa roihusi tuli.` to the
      corpus with the exact expected roles.

Acceptance: two clauses; `jossa` starts clause 1; `roihusi` is its predicate;
the second `tuli` is the **subject** with lemma `tuli` (noun), while the first
`tuli` is the predicate with lemma `tulla`.

### Phase 3 — predicate and auxiliaries (S8–S11)

- [ ] Add `Auxiliary` (preferred) or demote non-head predicate candidates.
- [ ] Fold negative auxiliaries and `olla` compound tenses.
- [ ] Necessive genitive subjects (`täytyä`, `pitää`, `kannattaa`, ...).
- [ ] Harden the infinitive fallback.

Acceptance: `En syö puuroa.` has one predicate phrase; `Olen lukenut kirjan.`
has `kirjan` as object; `Minun täytyy mennä.` has a genitive subject and an
infinitive clause that shares it; `Eikö hän tullut?` has `hän` as subject.

### Phase 4 — nominal roles (S12–S18)

- [ ] Case-gated subject selection; no proper-name shortcut.
- [ ] Postpositional existential.
- [ ] Numeral phrase.
- [ ] Valency complements table.
- [ ] Recipient vs theme object ranking.
- [ ] Agreement-based subject sharing.
- [ ] Extend the function-word gate.

Acceptance: `Matin auto on punainen.` -> `Matin` modifier, `auto` subject.
`Helsingissä asuva mies on opettaja.` -> `Helsingissä` adverbial, `mies`
subject, `asuva` modifier. `Hänellä on kaksi lasta.` -> `kaksi` subject,
`lasta` modifier. `Pöydän alla on kissa.` -> `kissa` subject.
`Sami antoi Liisalle kirjan.` -> `kirjan` object, `Liisalle` adverbial.
`Sami söi puuron, koska oli nälkäinen.` -> second clause shares `Sami`.

### Phase 5 — API and wasm contract (S19)

- [ ] Decide `Clause.complement` / `Auxiliary` / `Certain`.
- [ ] Update `analyze_syntax` and its TypeScript shape in
      `torille-wasm/src/lib.rs`.
- [ ] Add a wasm/integration check that the JSON keys still match the doc
      comment.

### Phase 6 — make it affordable before a UI uses it (S20)

- [ ] Replace the suggestion-payload base search with a recognition-only
      resolution plus one inflection of the winning base, or cache per
      document in the wasm layer.
- [ ] Record before/after wall and CPU time for
      `examples/syntax_probe_sentences.txt` in this file.
- [ ] Do not ship an interactive consumer until a paragraph is tens of
      milliseconds, not seconds.

## Non-goals

- No statistical model, no external parser, no new dependency.
- No general suffix stripping to recover a case; use the morphology table or
  a closed lexical set.
- No changes to the sentence layer or the keystroke path.
- No suggestion/ranking change. If S1 is fixed by changing `find_bases`
  instead of the syntax boundary, the suggestion gates
  (`test_reverse_lookup_recall.py`, `compare_engine_outputs.py`) must be run;
  the plan deliberately avoids that in phase 1.
- No attempt to resolve every ambiguous word. `Unknown`/`Weak` remains the
  correct answer when morphology and word order disagree.

## Open questions

1. Should `Clause.predicate` point at the finite auxiliary or the main verb in
   a compound tense? Recommendation: the main verb, with `Auxiliary` linked to
   it — that is what a UI wants to underline.
2. Relative clauses interrupt their host (`Mies, joka tuli, söi`). `Clause`
   ranges are contiguous, so the host predicate is forced into its own
   clause. Do we accept linear segments and document that, or add nesting
   (`parent`/`children`)? Recommendation: accept linear for now; the audit
   sentence has a final relative, so it works. Revisit when a consumer needs
   the host clause as one unit.
3. Should a relative clause's antecedent be linked (`head` of the relative
   pronoun -> antecedent), or only the clause roles? Recommendation: later;
   it needs an antecedent search across the clause boundary.
4. For proper names, is expanding `data/proper_name_forms.csv` with a case
   column acceptable as a data change, or should the syntax layer infer case
   from the name's final suffix? Recommendation: the column; inference would
   be a suffix stripper by another name.
