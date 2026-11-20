# TODO — grammar, syntax and style roadmap

A roadmap for the opt-in grammar (`syntax_check`), syntax analysis (`syntax`)
and style (`style_check`) layers. The design records are
[`docs/syntax_checker.md`](../docs/syntax_checker.md),
[`docs/style_checker.md`](../docs/style_checker.md),
[`PLAN_SYNTAX.md`](PLAN_SYNTAX.md),
[`PLAN_SYNTAX_CHECKER.md`](PLAN_SYNTAX_CHECKER.md) and
[`PLAN_STYLE.md`](PLAN_STYLE.md).

Status on 2026-11-19: the grammar, syntax and style phases are landed except
the explicitly gated items below (cold-page performance and the held `PREFER`
rule), and the targeted suites are green.
`page-2000` is at **5 adjudicated diagnostics with 0 false positives**
(1 true positive, 3 accepted, 1 excerpt artefact), and the reviewed-prose gate
is **266 clean rows / 6,349 words**, 2 true positives and 0 known false
positives. The precision plan
[`PLAN_GRAMMAR_PRECISION.md`](PLAN_GRAMMAR_PRECISION.md) is complete. This
file is the next roadmap, not a list of known red tests.

## Principles

1. **A wrong `Error` on correct prose is worse than a missing advisory rule.**
   Precision on real Finnish outranks rule count.
2. **Fix interpretation before adding rules.** A checker with hundreds of rules
   is unreliable if the morphology readings or syntactic roles are wrong.
   Prefer role disambiguation, reading consistency and confidence over another
   surface pattern.
3. **Keep the layers separate.** `syntax` understands structure, `syntax_check`
   evaluates correctness, `style_check` suggests expression. The checker must
   consume the syntax analysis, not grow a second parser beside it.
4. **Respect legitimate variation.** Word order, optional commas, colloquial
   possessive omission and copula complement case are not automatic errors.
5. **Detection and correction are different claims.** A finding can be valid
   even when no safe automatic replacement is available.
6. **Adjudicated prose is the release metric.** Constructed fixtures prove
   rules; reviewed prose proves the checker is trustworthy.

## Roadmap at a glance

| Stage | Work | Why |
|---|---|---|
| P0 | Morphological-ambiguity regression suite | Protect the interpretation fixes |
| P0 | Adjudicated Finnish prose corpus | Establish real-world precision |
| P0 | Remaining false positives | Every new one gets a regression first |
| P1 | Diagnostic contract, confidence, explainability, correction confidence | One contract before the rule expansion |
| P1 | Agreement subsystem | Highest-frequency missing errors |
| P1 | Case-government subsystem | Reuses the existing role and clause data |
| P1 | Sentence-structure subsystem | Conservative structural diagnostics |
| P1 | Morphology gap: adjective degree | Only if a correction rule needs it |
| P2 | Extra comma, duplicate word, style vocabulary | Lower-severity coverage |
| P2 | UI ignore/dismiss and combined command | Usability and rule-level suppression |
| P2 | Cold-page performance | Separate engineering target |
| P3 | Long-distance syntax roadmap | Keep visible; do not rush |

## P0 — protect the foundation

These come before any new rule family. They are where the current checker is
most likely to lose trust.

### P0-A — Morphological ambiguity and role-selection regressions

The recent fixes were all interpretation bugs, not missing rules. Make that
class of bug a permanent test category.

- [x] **Add an ambiguity fixture** — landed as
  `tests/fixtures/syntax_ambiguity.json` + `tests/syntax_ambiguity.rs`.
  Each case pins the intended interpretation and at least one competing
  interpretation that must not win.
- [x] **Require the pair for every new role rule.** A rule that says "this is
  a subject" must also say which nearby nominal is *not* the subject. Add the
  pair to the fixture in the same commit.
- [x] **Cover the readings that actually collide:**
  - noun vs verb (`tuli`, `kirjaan`, `sen`);
  - pronoun vs adverb (`sen`, `ne`);
  - preposition/postposition vs noun (`ilman`, `ennen`);
  - nominative vs genitive/accusative on the same surface (`sinut`);
  - determiner vs relative pronoun (`joka`);
  - subject vs adverbial (`joka päivä`).
- [x] **Gate the suite in the same way as the exact-output fixture.** A role
  change that silently flips a competing reading fails the suite.

Suggested cases:

| Construction | Intended reading | Competing reading to reject |
|---|---|---|
| `Joka päivä juon kahvia.` | frequency adverbial | `päivä` as subject |
| `Joka mies tietää sen.` | `mies` as subject | `joka` as determiner phrase |
| `Joka päivä on kaunis.` | `päivä` as subject | frequency adverbial |
| `Ilman rahaa en tule.` | preposition complement | `Ilman` as subject |
| `Ilman sinua minä en tule.` | `minä` as subject | `sinua` as attribute |
| `Tarvitsen sen.` | `sen` as object | `sen` as adverb |
| `Hän pitää puheen.` | total object | partitive-only rection error |
| `Autan sinut sisään.` | `sinut` as accusative object | `sinut` as nominative subject |
| `Pidän sinusta.` | elative rection | total-object silence |
| `Merkitse joka esine kirjaan.` | one clause, determiner | relative clause split |
| `Mies, joka tuli, söi.` | relative clause | determiner |

Area: `tests/fixtures/`, `tests/syntax_check.rs`, `src/syntax/roles.rs`,
`src/syntax/predicates.rs`, `src/syntax/segments.rs`.

### P0-B — Adjudicated Finnish prose corpus and release metrics

The clean gates are constructed sentences. `tests/fixtures/perf/page-2000.txt`
reports grammar diagnostics, but they are counted, not judged. Add a small
public-domain prose sample with reviewed findings and provenance.

- [x] **Choose a small sample with provenance.** Seeded with a reviewed
  Project Gutenberg excerpt via `tests/fixtures/perf/page-2000.txt` (ebook
  9082, public domain). The first slice is 381 adjudicated correct words plus
  one true-positive sentence. Grow it before treating the rate as stable.
- [x] **Adjudicate every diagnostic** as true positive, false positive, or
  deliberate/acceptable. The seed is
  `tests/fixtures/prose/adjudicated.json`: 15 clean rows, 5 known
  false-positive rows, and 1 true-positive row.
- [x] **Separate rows by severity/status:** `clean`, `errors`, and
  `known_false_positives`. A fix to a known row makes the suite fail so it can
  be moved to `clean` or `errors`.
- [x] **Add the ambiguity corpus** from P0-A to the same run.
- [x] **Keep the constructed exact-output fixtures** as the per-rule gate.
- [x] **Grow the prose sample to at least 1,000 correct words** and keep the
  known-false-positive rate per 1,000 words as the baseline. The sample is now
  **266 clean rows / 6,349 words**, 2 true positives and 0 known false positives;
  the precision phases fixed the known findings and grew the sample from the
  original 1,021 words. The 2026-11-19 extensions added 61 clean Finlex
  paragraphs (modern formal Finnish), 86 clean novel paragraphs from seven
  Projekti Lönnrot works and 40 clean factual paragraphs from three Doria
  books; the raw sources are gitignored and the provenance is in
  `tests/fixtures/prose/README.md`. New rows come from the same public-domain
  sources and were admitted only after `check_syntax` returned zero
  diagnostics; the seed note in `tests/fixtures/prose/adjudicated.json` records
  that rule.

Track these metrics:

| Metric | Purpose |
|---|---|
| `Error` false positives per 1,000 tokens | Release blocker |
| false positives per 1,000 tokens by severity | Overall precision |
| precision and recall per `DiagnosticKind` | Which subsystem is noisy |
| percentage of sentences with ≥1 finding | How intrusive the checker is |
| correction acceptance / correction validity | Detection vs correction quality |

Release gate:

- **zero `Error` on adjudicated correct prose**;
- no regression in the warning/suggestion per-1,000-token baseline;
- every new rule lands with constructed positive/negative cases and, where it
  is a high-frequency construction, an adjudicated prose row.

Do this **before** implementing most of P1, so rule work has a real precision
ceiling.

### P0-C — Remaining false positives and reading bugs

Every new false positive found in prose goes here first and gets a regression
before the next rule lands.

- [x] **`joka`/`jokainen` as a determiner before a time noun** — landed.
  `Joka päivä juon kahvia.`, `Joka aamu juon kahvia.`,
  `Joka vuosi matkustan Lappiin.`, `Jokainen päivä menen kouluun.` used to emit
  `subject_predicate_agreement` **Error/High**. The role pass treats a
  determiner plus a closed set of temporal nouns as a frequency adverbial when
  the clause's finite reading is 1st/2nd person. `Joka mies tietää sen.` and
  `Joka päivä on kaunis.` keep their subject readings.
  Area: `src/syntax/roles.rs`, `src/syntax/segments.rs`,
  `src/syntax/predicates.rs`.

- [x] **Verify `koko` + temporal noun** — verified, no pass extension.
  `koko päivä` is a valid subject (`Koko päivä meni hukkaan.`), while
  `koko päivän` is the duration adverbial (`Koko päivän juoksin.`). `koko` is
  therefore **not** added to the frequency determiners; the ambiguity fixture
  pins the subject reading and the clean corpus keeps both correct forms quiet.
  `Koko päivä juoksin.` no longer offers the 3sg `juoksi`: a 1st/2nd-person
  predicate with a nominal subject is a deliberate false negative, pinned by
  `koko_temporal_noun_does_not_offer_a_third_person_verb`.
  Evidence: attested subject use `Koko päivä meni tavaroiden kanniskeluun.`
  (suomisanakirja.fi/kanniskella) and adverbial use `Hän kanniskeli
  kauppakasseja koko päivän.` on the same page.

- [x] **Prepositions and their complements** — landed.
  `Ilman rahaa en tule.`, `Ilman sinua en tule.`, `Ilman sinua minä en tule.`
  and `Ennen minua hän tuli.` are clean. `mark_adpositions` handles
  `Preposition` (including the lexical `adverbi, prepositio` row for `ennen`)
  and marks the following nominal as its complement. Nominal case selection
  requires one reading to be both nominal and in the case, so a preposition's
  nominative reading cannot make it a subject.
  Area: `src/syntax/roles.rs`, `src/syntax/predicates.rs`.

- [x] **Rection must not treat a partial valency table as exhaustive** —
  stopped with an aspectual/total-object guard.
  `Hän pitää puheen.`, `Hän pitää kokouksen.`, `Hän pitää kirjan.`,
  `Katson elokuvan.` and `Autan sinut sisään.` are clean; `Pidän sinusta.`,
  `Katson elokuvaa.` and `Luotan sinuun.` stay clean.
  `ASPECTUAL_OBJECT_VERBS` in `src/syntax_check/rection.rs` is a stopgap. The
  proper follow-up is a `frame kind` column in `data/verb_rection.csv`
  distinguishing lexical government from aspectual partitive/total
  alternation. Do not add an “anything” frame; that silences the verb.

- [x] **`sen`/`ne` pronoun objects read as adverbs** — landed.
  `Tarvitsen sen.` and `Käytän sen.` used to trigger `missing_argument`.
  `classify` now lets a pronoun genitive/accusative object reading win over the
  adverb homonym; object selection takes the genitive/accusative case.
  Area: `src/syntax/roles.rs` (`classify`), `arguments.rs`.

- [x] **`joka` + noun must not split a relative clause** — landed.
  `Merkitse joka esine kirjaan.` is one clause: `esine` is the object and
  `kirjaan` is not read as a predicate. `joka` is a determiner when followed by
  a nominative noun and not preceded by a comma.
  Area: `src/syntax/segments.rs`, `src/syntax/predicates.rs`.

Prose-corpus findings were pinned as `known_false_positives` in
`tests/fixtures/prose/adjudicated.json`. All four remaining rows have now
landed and moved to `clean`; the corpus has no known false positives. Two of
the older rows had already landed:

- [x] **Modifier vs head:** landed. `Nämä hurskaan miehen sanat` agrees `Nämä`
  with `sanat`; `attribute_head` walks past intervening modifier words.
- [x] **Punctuation boundaries:** landed. Sentence-initial `Ja kun ...`, a
  final relative clause (`..., joka ei ...`), `--kun ...` and the `jos`
  conditional no longer ask for an extra comma.

The four remaining rows, with the cause measured from a token/role dump:

- [x] **Apposition and subject phrase:** landed. `Me suuremmat istumme ...` is
  quiet. The personal pronoun now matches the predicate's person/number and
  heads the apposition; class 101 tags `me`/`te`/`he` as plural. Originally it
  produced
  `attribute_agreement`, `subject_predicate_agreement` and `object_case`.
  `me` is tagged nominative **singular** by the class-101 engine, so `Me`
  cannot agree with plural `suuremmat`; the subject pass then takes the last
  preverbal nominative (`suuremmat`) as the subject and demotes `Me` to a
  weak modifier. `häntä` is the noun “tail” before it is the pronoun `hän`,
  and `ettei` is not treated as a clause opener.
- [x] **Experiencer and total-object verbs:** landed. `ettei häntä pelottaisi`,
  `he eivät uupuneet sitä kuuntelemasta`, and `Jumala hellytti pyövelien
  sydämet` are quiet. `ettei` now opens its own clause, and the pronoun
  homonym of `häntä` wins over the noun “tail”. Originally the latter two were
  clean already; `pelottaa` fired because `ettei` did not split its clause and
  `häntä`'s noun nominative homonym became the subject.
- [x] **Coordination scope:** landed. `Kenoveeva kehräsi ja lauloi, Sikri
  soitti ...` is quiet. Unknown capitalized names are role-only subjects, a
  comma before a capitalized subject starts the clause at that word, and a
  relative clause no longer inherits the main clause subject. Originally it
  reported plural agreement on three singular predicates: the unknown
  capitalized names had no readings, so the subject pass missed them and
  `share_clause_roles` copied `molemmat` into every following 3rd-person
  clause. `ja lauloi, Sikri soitti` was also one clause because the boundary
  was before `Sikri`, not directly before the finite verb.
- [x] **Postposition and adverbial:** landed. `rakkaan puolisonsa jälkeen`,
  `koko päivänä` and `Eräänä iltana` are quiet. A postposition classifies as
  `Adverbial` before the noun-case fallback, and temporal essives are
  adverbials rather than attributes or subjects. Originally it reported an
  `object_case` error on the genitive governed by `jälkeen`. `jälkeen` has a
  postposition reading plus a noun illative homonym, so `classify` gives it
  the noun case and `mark_adpositions` skips it; the genitive
  `puolisonsa` then becomes the subject. `koko päivänä` is read as an essive
  attribute and corrected to `kokona`; `koko` is an invariant determiner and
  `päivänä` is a temporal essive adverbial, not an attribute head.

#### P0-C landing plan — the four remaining rows

Land the morphology first: it removes diagnostics from more than one row and
keeps the later role rules from compensating for a bad reading.

1. **Pronoun morphology** — `src/expand/nominals/class_101.rs`,
   `src/morphology/mod.rs`.
   - Give `Pron` an explicit `number`; `me`, `te` and `he` are plural, while
     `minä`, `sinä` and `hän` stay singular. Use it for every cell, including
     the accusative (`meidät`, `teidät`, `heidät`).
   - When a direct resolution succeeds (`häntä` is a noun), also add the
     closed-class pronoun paradigm's readings for `pronoun_base(lower)`.
     Keep both homonyms; do not overwrite the noun.
   - Make `nominal_has_case` and `nominal_case` prefer pronoun readings when
     a token has both a pronoun and a noun reading, so the “tail” nominative
     cannot make `häntä` a subject.
   - Tests: class-101 number tags for every personal pronoun; `häntä` has a
     pronoun partitive reading; `Me` is nominative plural.

2. **Apposition** — `src/syntax/roles.rs`.
   - Before the default subject search, when a preverbal personal pronoun
     (`minä`/`sinä`/`hän`/`me`/`te`/`he`) matches the finite predicate's
     person and number, make the pronoun the subject.
   - Mark a following nominative nominal/adjective phrase as an apposition
     `Modifier` headed by the pronoun. Require no coordinator between the
     two, so `Minä ja Liisa tulemme` still uses coordinated subjects.
   - Tests: `Me suuremmat istumme.`, `Minä yksin tulen.`,
     `Te kaikki tiedätte.`, `Me suuremmat emme istu.`; keep
     `Minä ja Liisa tulemme.` clean.

3. **Fused negative subordinators** — `src/syntax/predicates.rs`,
   `src/syntax/segments.rs`.
   - `ettei` is `että + ei`. Recognise fused negative conjunctions through
     `expand::negation::negation_base`: if the base is a subordinator
     (`että`, `jotta`, `vaikka`, `ellei`, ...), treat the surface as one.
   - This splits `ettei häntä pelottaisi`; with the pronoun preference from
     step 1, `häntä` is the partitive object and the negative-object rule is
     quiet.
   - Tests: `ettei häntä pelottaisi`; `Hän sanoi ettei tule.` keeps its
     comma behaviour; `Ei, sillä Jumala hellytti ...` and
     `he eivät uupuneet ...` stay clean.

4. **Comparison marker and attribute heads** — `src/syntax/predicates.rs`,
   `src/syntax/roles.rs`, `src/syntax_check/agreement.rs`.
   - Add `kuin` as a function word (`is_clause_marker`, `is_marker`). Its
     noun homonym is `kuu` in the instructive plural, which is what
     `attribute_head` currently finds.
   - `attribute_head` accepts only a nominal head whose role can head a
     phrase (`Subject`, `Object`, `Complement`, `Unknown`); skip `Modifier`
     and `Adverbial` and continue to the next candidate.
   - Tests: `Hän on yhtä viaton ja herttainen kuin kaunis.` is quiet;
     `Punainen auto` still agrees; `Nämä hurskaan miehen sanat` stays clean.

5. **Unknown names and coordination scope** —
   `src/syntax/roles.rs`, `src/syntax/segments.rs`.
   - Role-only fallback for an unknown capitalized word immediately followed
     by the clause predicate/auxiliary: mark it as a nominative subject
     without inventing morphology or a lemma. Do not apply when it is
     followed by a nominal (`Kenoveevan laulua`), so it stays a modifier.
   - Start a new clause at a comma followed by a capitalized word that is
     itself followed by a finite predicate. This splits `, Sikri soitti`.
   - In `share_clause_roles`, only inherit the previous subject when the new
     predicate's person/number is compatible with it, or when the previous
     subject's number is unknown. A plural subject must not be copied into a
     singular predicate.
   - Tests: the full row is quiet; `Sami söi ja joi.` keeps a shared
     singular subject; `Sami ja Liisa söivät ja joivat.` stays plural;
     `Kun hän tuli, söin.` still does not share.

6. **Postpositions and temporal essives** — `src/syntax/roles.rs`.
   - `classify` gives a preposition/postposition reading `Adverbial` with no
     case, before the noun-case fallback.
   - `mark_adpositions` handles a postposition already classified
     `Adverbial`, marks the preceding genitive as `Modifier` (head =
     postposition), and clears the postposition's case. Standalone `jälkeen`
     stays safe because no genitive precedes it.
   - Mark essive temporal nouns (`päivä`, `ilta`, `aamu`, `yö`, `kesä`,
     `talvi`, `kevät`, `syksy`, `vuosi`, `viikonloppu`, `hetki`) as
     `Adverbial` before subject/object selection. `koko` stays an invariant
     determiner; the temporal head is no longer an attribute head.
   - Tests: the full row is quiet; `Illallisen jälkeen ...` has no object;
     `koko päivänä` is quiet; `Eräänä iltana` stays quiet;
     `Pöydän alla on kissa` is unchanged.

7. **Corpus, metrics and gates** — `tests/fixtures/prose/adjudicated.json`,
   `tests/syntax_prose.rs`, `TODO_GRAMMAR_STYLE.md`, `TODO.md`.
   - Move each row to `clean` only when it has zero diagnostics. All four
     moved: the suite reported 66 clean rows and **1,021 clean words** at that
     point, with 0 known-FP words and 0 known errors. The later precision
     phases raised the live baseline to **248 clean rows / 5,862 words**; see
     the work log and [`PLAN_GRAMMAR_PRECISION.md`](PLAN_GRAMMAR_PRECISION.md).
   - Run `cargo test --release --test syntax_prose`, then
     `cargo test --release --workspace`, clippy, the strict engine-output
     comparison, the n=5 evidence gate and the binding agreement test.
   - The class-101 change retags existing surfaces; it does not add or remove
     a surface. Verify `the_pronoun_blob_matches_compute` and
     `cargo run -p torille-maps -- status`; regenerate/publish only if a
     surface changed.
   - Do not add `koko päivä` (nominative) to the clean set, a universal
     postposition rule, or a suffix entry in `is_known_word`. Keep
     `Koira nukkuvat.`, `Minä menee kotiin.` and `En lue kirja.` as true
     positives.

The order is deliberate: steps 1 and 3 fix `Me suuremmat ...`; step 4 fixes
`herttainen`; step 5 fixes the coordinated names; step 6 fixes the postposition
and `koko` row. Each step carries its own unit regression, so a later role
change cannot silently reintroduce an earlier false positive.

Landing the four rows exposed three more role bugs, fixed in the same pass so
the clean set stays quiet:

- A demoted weak predicate (`siitä` is the elative pronoun/adverb and a form of
  `siittää`) is re-read from its non-verb readings instead of becoming a bare
  `Unknown`; otherwise the attribute pass treats the adverbial as an attribute
  of the next nominal.
- A relative clause (`poika, joka oli syntynyt ...`) does not inherit the main
  clause subject, so `share_clause_roles` cannot make the singular `oli` agree
  with plural `he`.
- The comma-plus-capitalized-subject split ignores a vocative/apposition
  (`sinä, Heintsi, sano ...`), where a comma separates the name from the
  predicate, and the negative-object rule skips the clause subject in a merged
  clause.

#### Modern formal prose findings (2026-11-19)

The Finlex 729/2018 sweep admitted 61 clean paragraphs and left 78 candidates
with diagnostics. They are not in `known_false_positives`; they are the seed
for a future precision round. Representative examples:

- `case_selection` on a partitive object after a participle:
  `... avustettava loukkaantuneita ...`.
- `subject_predicate_agreement` on a coordinated singular subject:
  `Valvottu jalankulkijaryhmä ja järjestäytynyt kulkue saa ...`.
- `attribute_agreement` on an adverbial or quantifier modifier:
  `... muullakin kuin sille tarkoitetulla tien osalla ...` and
  `... ajosuunnat erottava koroke ...`.
- `connegative` on a passive connegative:
  `... jos tässä laissa ei toisin säädetä.`
- `verb_rection` / `case_selection` on an infinitive or participial frame:
  `... saa ajoneuvolla poistua risteyksestä sopivinta ajokaistaa käyttäen ...`.
- `comma_missing` before `ellei`:
  `... väistettävä oikealta lähestyvää ajoneuvoa ellei 2–4 momentissa toisin
  säädetä.`

Each needs an ambiguity pair (the intended role plus the rejected reading)
before a rule change. Do not add one to `known_false_positives` without the fix
or a deliberate baseline move.

The Projekti Lönnrot novel sweep shows the same classes in narrative and
dialogue form: `comma_missing` before `ja vaikka` / `että`,
`subject_predicate_agreement` on dialogue fragments, `verb_rection` on a
conversational frame, and `object_case` on an imperative or dialogue object.
These are candidates, not confirmed false positives; each still needs an
ambiguity pair before a rule change.

The Doria factual sweep adds expository prose. Its rejected windows are mostly
comma and subject findings on OCR-damaged text; uncorrected text is not
admitted, so only explicitly corrected, clean rows enter the corpus.

Deeper gaps exposed by the curated rows, recorded rather than guessed at:

- **Plural-only classless compounds.** `olosuhteet` and `ajosuunnat` are read
  as singular nominatives, so `Erityiset olosuhteet vaativat` and
  `oleva ajosuunnat erottava koroke` get false agreement findings. The fix
  belongs in the classless compound/head morphology, not in the agreement rule.
- **List continuation after a postposition.** `... kohtuutonta melua, ilman
  pilaantumista ...` is missegmented into an extra clause, so the comma rule
  wants a second comma after `kohtuuttonta`.
- **Singular class-noun predicate.** `Sukset olivat kulkuneuvo` is flagged by
  the predicative-noun rule even though the singular class reading is normal.

## P1 — shared diagnostic contract, confidence and explainability

This is cross-cutting and should land before the rule expansion. It is the
place to stop future false positives by construction rather than by patching
one word at a time.

### P1-A — One diagnostic contract for all three layers

The exact Rust shape can stay close to today's `SyntaxDiagnostic`, but every
layer should follow the same contract:

```rust
Diagnostic {
    rule_id: RuleId,          // stable kind/name, usable for ignore/dismiss
    severity: Severity,       // Error / Warning / Suggestion
    confidence: Confidence,   // Certain / Likely / Ambiguous
    span: Span,               // the actual problematic construction
    message: MessageKey,      // stable, localisable
    explanation: Option<Explanation>,
    suggestions: Vec<Suggestion>, // optional; empty is valid
}
```

Rules:

- stable rule identifiers — ignore by **rule**, not by text span;
- span points at the responsible word or construction;
- severity and confidence are independent;
- suggestions are optional;
- an explanation is available without a correction;
- `style_check` never emits `Error` or `Warning`.

Landed in `src/diagnostic.rs`: the shared `Severity`, `Confidence`, `RuleId`,
`Explanation`, `Correction` (with `license` and `confidence`), and the `Finding`
trait. `SyntaxDiagnostic` and `StyleDiagnostic` both carry `rule_id` and
`explanation`; `CheckAll::findings()` returns both streams through the same
trait. The old `DiagnosticConfidence` name and `High`/`Medium`/`Low` constants
remain as compatibility aliases.

### P1-B — Structural confidence and suppression

Introduce a small number of explicit structural states rather than numerical
probabilities:

- **Certain** — one clear reading; an `Error` is allowed.
- **Likely** — one preferred reading with a known competitor; use `Warning` or
  `Suggestion`, or suppress for high-risk rules.
- **Ambiguous** — competing readings; suppress the diagnostic.

An agreement error should require an unambiguous subject–predicate relation.
An unknown word, an ambiguous role, a numeral phrase, a coordinated subject
whose person cannot be resolved, or a missing comma that changes the parse
should suppress rather than guess.

- [x] Add the confidence states and route every existing rule through them.
  `Confidence::{Certain, Likely, Ambiguous}` replaces the numeric labels;
  `Confidence::from_role` maps an unambiguous role to `Certain` and a weak role
  to `Likely`.
- [x] Pin suppression cases in the ambiguity fixture. Added
  `ambiguous_tuli_subject` (`Tuli sammui.`); the ranker also has a synthetic
  suppression test.
- [x] Do not emit `Error` from a rule whose preconditions are only `Likely`.
  Enforced centrally in `rank::resolve`: an `Error` with `Likely` is downgraded
  to `Warning`, and an `Ambiguous` finding is dropped before it reaches a
  caller. Existing rules now build `Certain`/`Error` only from a clear role.

### P1-C — Detection confidence vs correction confidence

A diagnostic can be valid even when no safe rewrite exists.

- [x] Make correction confidence explicit, or make corrections fully optional
  per diagnostic. Corrections are optional (empty is valid) and now carry a
  `CorrectionLicense` (`Inflection`, `Curated`, `Punctuation`, `Other`) and a
  `Confidence`; the wasm wire shape exposes both.
- [x] Keep curated replacements for style and safe case/form changes. Style
  phrase replacements are `Curated`/`Likely`; inflection forms are
  `Inflection`/`Certain`; punctuation edits are `Punctuation`/`Certain`.
- [x] Do not invent a word, a subject or an argument just to fill a suggestion.
  The helpers still take every replacement from `forms_of`/the curated tables;
  a diagnostic with no safe rewrite keeps an empty correction list.
- [x] For numeral/adposition/object rules, detect confidently and propose only
  forms licensed by the inflection engine and the curated frame. The numeral
  and adposition corrections are built through `case_corrections`, which
  requires a `Plain` engine form.

Example: `Kaksi lapsi tuli.` can be detected confidently, but a suggestion is
only safe when the engine can name the partitive singular without ambiguity.

### P1-D — Explainability

Every finding should be able to say:

- which word or phrase is considered wrong;
- which grammatical relation triggered it (subject–predicate, attribute–head,
  verb frame, numeral head–complement, adposition–complement, punctuation);
- what the expected category is (case, number, person, form);
- why the proposed correction is licensed by the analysis.

`expected` and `message` are useful for debugging, but the UI needs a stable
explanation separate from the correction. Add explanation keys before the
first new rule family.

Landed: every finding carries `rule_id` (the message key) and an `Explanation`
with a stable `Relation` (`subject_predicate`, `attribute_head`, `verb_frame`,
`adposition_complement`, `numeral_complement`, `punctuation`, ...) and a
stable key. The syntax and style panels show the rule id and the relation; the
wasm diagnostic object exposes `rule_id` and `explanation: { relation, key }`.
`expected` remains the category (case/number/person/form).

## P1 — grammar rule subsystems

Once P0 and the contract are in place, add rules one subsystem at a time. A
subsystem lands with its ambiguity cases, constructed positives/negatives, and
prose-corpus review.

### Agreement

- [x] **Predicative adjective agreement** — landed for number.
  `Talot ovat punainen.` and `Talo on punaisia.` now emit
  `predicative_agreement`; `Talot ovat punaisia.`, `Talot ovat punaiset.` and
  `Talo on punainen.` are clean. The rule is number-only and adjective-only:
  `Talot ovat punaisia` (partitive plural) and `Talot ovat punaiset`
  (nominative plural) are both legitimate, so case is not a correctness claim.
  Area: `src/syntax_check/agreement.rs`; `Clause.complement`.

- [x] **Predicative noun/participle complements** — landed.
  `Lapset ovat väsynyt.` is covered by the separate compound-participle rule
  (`väsyneet`); `He ovat opettaja.` now emits `predicative_agreement` /
  `predicative_noun_agreement` as a `Warning` with both licensed plural forms
  (`opettajia`, `opettajat`). The noun rule is deliberately separate from the
  adjective number rule: it only fires on a clear nominative number mismatch
  (plural subject + nominative singular, or singular subject + nominative
  plural). Partitive predicates (`He ovat opettajia.`, `Ne ovat vettä.`) and
  agreeing forms stay quiet; the correction confidence is `Likely` because the
  partitive/nominative choice is semantic.
  Area: `src/syntax_check/agreement.rs`.

- [x] **Connegative main verb after the negative auxiliary** — landed.
  `En menen.`, `Hän ei menee.` and the negative past (`En menin.` →
  `mennyt`) now emit `connegative`. `En mene.`, `Hän ei mene.`, `En ole.`,
  `En mennyt.`, `En tullut.`, `Älä mene.`, `Älkää menkö.` and `En voi mennä.`
  are clean. The rule checks only the verb adjacent to the auxiliary, accepts
  a negative reading (present connegative or past participle), and treats the
  3sg conditional as the conditional connegative. Imperative findings are
  label-only; the potential is left quiet.
  Area: `src/syntax_check/connegative.rs`, `src/syntax_check/correction.rs`,
  `src/syntax/predicates.rs`.

- [x] **Compound-tense participle agreement** — landed.
  `Me olemme lukenut.`, `Minä olen lukeneet.` and `Hän on lukeneet.` now emit
  `subject_predicate_agreement` with the corrected participle (`lukeneet`,
  `lukenut`). The rule compares the subject's number with the active past
  participle when an `olla` auxiliary is present. Passive compound tenses
  (`Talo on rakennettu`, `Talot on rakennettu`) are skipped. Active/passive
  homonyms (`mennyt`, `tullut`) are filtered to the active reading; an
  adjective homonym whose chosen lemma is not the verb (`väsynyt`) falls back
  to the verb lemma in `lemmas`. Clean rows cover all three example errors
  fixed, the negative compound (`Me emme ole lukeneet.`), `He ovat väsyneet.`,
  `He ovat väsyneitä.` and the passive forms.
  Area: `src/syntax_check/agreement.rs`, `src/syntax_check/correction.rs`.

- [x] **Possessive suffix person/number** — landed.
  `Minun kirjasi on uusi.` and `Hänen kirjani on uusi.` now emit
  `possessive_agreement` as a `Warning`; `Minun kirja on uusi.` emits
  `possessive_missing` as a `Suggestion`. Never an `Error`, and no automatic
  correction: colloquial Finnish drops the suffix. The rule matches a genitive
  personal pronoun with the adjacent head noun, through adjective/adverb
  modifiers; standalone suffixes (`Kirjani on uusi.`) and matching suffixes
  stay quiet. A genitive pronoun before a necessive verb (`Teidän täytyy ...`)
  is the verb's subject, not a possessor, and is skipped.
  Area: `src/syntax_check/possessive.rs`.

### Case government

- [x] **Numeral government** — landed.
  `Kaksi lapsi tuli.`, `Kaksi lapsia tuli.` and `Ostin kaksi kirja.` now emit
  `case_selection` with the partitive singular. `Kaksi lasta tuli.`,
  `Kaksi miestä tuli.`, `Yksi lapsi tuli.`, `Kahdet kengät.`,
  `Kaksi pientä lasta.`, `Kaksi kolmesta.`, `Sata autoa.`,
  `Kaksikymmentä lasta.` and `Ostin kaksi kirjaa.` are clean. The rule is
  narrow: only a nominative complement or partitive plural is flagged; `yksi`,
  plural numerals, `toinen` and other non-cardinals are skipped. The adjective
  guard also removes the old `Kaksi pientä lasta` attribute false positive.
  Area: `src/syntax_check/numeral.rs`, `src/syntax_check/agreement.rs`.

- [x] **Adposition case government** — landed.
  `Pöytä alla on kissa.` now emits `case_selection` on `Pöytä` with
  `Pöydän`; `Lähdin ilman raha.` does the same with `rahaa`. The table is
  `data/adposition_cases.csv` (`adposition,case,example`), embedded as
  `maps/adposition_cases.bin.gz`; the rule does not infer a case from the
  surface. `Pöydän alla on kissa.`, `Lähdin ilman rahaa.`,
  `Ennen minua hän lähti.`, `Tien varrella kasvoi koivuja.`,
  `Lain mukaan se on sallittua.` and the standalone adverb `Kissa on alla.`
  are clean. A genitive after a partitive-governing preposition is left quiet
  because it may modify a following participle (`ilman Jumalan tahtomatta`);
  that prose false positive was found during the corpus review and removed.
  Area: `src/syntax_check/adposition.rs`, `src/expand/adposition_cases.rs`,
  `data/adposition_cases.csv`, `build.rs`, `build/map_builders/front_coded.rs`.

- [x] **Existential subject case** — landed.
  `Pöydällä on kirjan.` now emits `case_selection` on `kirjan`; the subject is
  nominative or partitive, never genitive. The clause's `existential` flag now
  accepts a genitive post-predicate nominal, so a wrong-case subject does not
  hide the construction. Detection is `Certain`/`Error`, but the rewrite is
  `Likely`: the engine offers both licensed forms (`kirja`, `kirjaa`) because
  boundedness and count/mass choose between them. `Pöydällä on kirja.`,
  `Pöydällä on kirjoja.`, `Pöydällä on Liisan kirja.`,
  `Pöydällä on minun kirjani.`, `Minun on mentävä.` and
  `Täällä on minun mentävä.` are clean. The rule fires only on a
  sentence-final common noun/adjective; a clause split at a line break or an
  unpunctuated coordinator (`... laulujen\nja kanteleen maa`) stays quiet.
  Area: `src/syntax_check/existential.rs`, `src/syntax/roles.rs`.

- [x] **Bare nominative object** — landed.
  `Minä näen kirja.` now emits `object_case` on `kirja` with both licensed
  repairs (`kirjan`, `kirjaa`). The total object is genitive singular or
  nominative plural; a nominative **singular** object is correct only in an
  imperative, passive or necessive clause. The rule is deliberately narrow:
  it requires an overt preverbal subject, a postverbal object, a singular
  common noun with no possessive and no competing nominal case, and the object
  must be the clause's final word. `Näen kirjat.`, `Syö puuro!`,
  `Syötiin puuro.`, `Minun täytyy lukea kirja.`, `Söin aamupuuronsa.` and
  `Hän puhui kuten asiantuntija.` are clean; the page-2000 sweep is quiet.
  Area: `src/syntax_check/object_case.rs`.

### Sentence structure

- [x] **Missing finite predicate** — landed.
  `Koska minä väsynyt.` now emits `clause_structure`/`missing_predicate` as a
  `Warning`, with no correction. The rule requires a sentence-final
  declarative clause that starts with a subordinator, has no finite predicate,
  and contains a personal-pronoun subject followed by a predicative
  adjective/participle. `Koska minä olen väsynyt.`, questions, headings and
  fragments without a period stay quiet.
  Area: `src/syntax_check/structure.rs`.

- [x] **Incomplete coordination** — landed.
  `Hän tuli ja.` now emits `clause_structure`/`incomplete_coordination` as a
  `Warning` on the trailing coordinator, with no correction. The clause must
  have a predicate and end at a declarative period; `Hän tuli ja lähti.` and a
  fragment without the period stay quiet.

- [x] **Extra comma** — landed.
  `Minä, menen kotiin.` now emits `comma_extra` as a `Warning` with a
  `Likely` comma-removal correction. The rule is deliberately narrow: only a
  personal-pronoun subject directly followed by a comma and then its finite
  predicate. Appositions and parentheticals (`Minä, yksin, tulen.`,
  `Minä, kaikesta huolimatta, menen kotiin.`), noun-subject addresses
  (`Liisa, tule tänne.`) and any intervening word stay quiet. A deliberate
  pause is the remaining competitor, so this is never an `Error`.
  Area: `src/syntax_check/punctuation/comma.rs`.

### Morphology

- [x] **Adjective degree** — landed.
  `Punaisempi autot ovat hienoja.` now emits `attribute_agreement` on
  `Punaisempi` (expected nominative plural). `Tags` gained
  `degree: Option<Degree>` (`Comparative`/`Superlative`), the morphology codec
  carries it, and `degree_forms` tags every generated degree reading.
  `attribute_agreement` and `predicative_agreement` compare degree forms like
  any adjective; an untagged degree surface is still skipped rather than
  replaced with a positive. Degree forms are derived `Repaired` words, so the
  diagnostic is detection-only: `Punaisemmat autot ovat hienoja.`,
  `Punaisempi auto on hieno.` and `Talot ovat punaisempia.` are clean.
  Recorded decision: `PLAN_SYNTAX_CHECKER.md` §14 item 4.

## Recommended implementation order

The rule order is deliberately interleaved across subsystems so each step
reuses the previous one and the prose corpus grows with it.

| Order | Work | Reason |
|---|---|---|
| 0 | Ambiguity regression suite | Protect the interpretation fixes |
| 0 | Adjudicated prose corpus | Establish real-world precision |
| 1 | Diagnostic contract + structural confidence | Stop future false positives by construction |
| 2 | Predicative complement agreement | High-frequency, reuses `Clause.complement` |
| 3 | Connegative main verb | Finnish-specific and tightly scoped |
| 4 | Numeral government | High-frequency, constrained |
| 5 | Adposition case government | Natural extension of the preposition work |
| 6 | Existential subject case | Reuses the existing clause flags |
| 7 | Compound-tense participle agreement | Extends the agreement subsystem |
| 8 | Bare nominative object | Important; needs the imperative/passive/necessive guards |
| 9 | Clause structure | Conservative missing-predicate/incomplete-coordination rule |
| 10 | Possessive suffix | `Warning`/`Suggestion` only |
| 11 | Extra comma | P2 punctuation coverage |
| 12 | Duplicate words | Low-risk style suggestion |
| 13 | Style vocabulary expansion | Incremental table growth |
| 14 | UI ignore/dismiss + combined command | Rule-level suppression and usability |
| 15 | Cold-page performance | Separate engineering objective |
| 16 | Word-order specification and features | Construction-aware, suggestion-only |

## P2 — punctuation, style and surfaces

### Conservative duplicate-word detection

Adjacent repetition is not always accidental:

- `Se oli hyvin, hyvin vaikeaa.`
- `Ei, ei se ollut niin.`
- `Hän sanoi, että että tämä oli väärin.` may be a transcription artifact, but
  context matters.

- [x] **Landed.** `style_duplicate_word` is a `Suggestion` for two identical
  surfaces with only whitespace between them. The correction removes the
  separator and the second word. A comma, dash or any other punctuation breaks
  the pair; the explicit rhetorical list (`hyvin hyvin`, `kauan kauan`,
  `pian pian`, ...) and phrase-table spans stay quiet. The duplicate takes
  precedence over the near-repeat metric, so one span never carries both.
  Page-2000 has no `style_duplicate_word` findings.
  Area: `src/style_check/repetition.rs`.

### Other P2 items

- [x] **Grow the style tables carefully** — landed.
  Added reviewed rows: `periaatteessa`, `tosiasiassa`, `sinänsä` (filler),
  `joka tapauksessa` (filler), `jollain tavalla` → `jotenkin` (wordy) and
  `jossain määrin` (wordy, label only). `niin kuin` and `ikään kuin` are
  deliberately **not** rows: they are standard comparators, not fillers, and
  the exact-match table cannot see whether a given use is redundant. The
  exact-match contract is unchanged; the new rows were checked against the
  style fixtures and the page-2000 prose corpus (none of the new terms occur
  there, so its style baseline is unchanged).

- [x] **Dead reserved kinds** — resolved.
  `CaseSelection`, `ClauseStructure` and `CommaExtra` now have emitting rules;
  `WordOrder` is the only deliberately reserved kind left.

- [x] **Ignore/dismiss and a combined command** — landed.
  Every grammar/style finding has an **älä huomauta** action that hides its
  stable `rule_id`; the per-page set is persisted in
  `torille-ignored-rules` and the options page can clear it. The Rust API is
  `SyntaxCheck`/`StyleAnalysis`/`CheckAll::without_rules` and
  `ignore_rules`. The context menu adds **Tarkista kielioppi ja tyyli**, which
  runs the existing combined wasm/background pass and shows one ranked panel;
  the separate grammar and style commands remain. Both browser extensions
  stay source-identical and pass their typecheck and unit tests.

- [ ] **Cold page target**
  `PLAN_GRAMMAR_STYLE_PERF.md` §Phase 11 landed the serial numeral upgrade:
  only a nominative cardinal governs a partitive, so the deep walk that ran
  after every inflected numeral (`kahtena päivänä`, `kolmesta miehestä`) is
  gone. Measured 2026-11-19: native single-thread cold **1,330 → ~1,190 ms**,
  native 4-worker **450 → 319–348 ms** (inside the 500 ms budget), and the
  wasm post-seed final pass **615 → 72 ms**, which is where the pool's
  755–762 ms lived. The browser pool now measures **364–395 ms** for a fresh
  textarea state on a warm pool (`e2e/pool-timing.mjs`), so the ≤500 ms
  per-page target is met; the ~950 ms first check of a session is the four
  worker spawns and wasm instantiations, paid once. The exact table is priced,
  not rejected: the **grouped** two-level layout (`stem → base → endings`, the
  base written once per stem) is **3.15 MB gzip for nouns + verbs**, or
  ~3.7 MB with a reading byte per entry, against ~8.1 MB for the flat
  `surface → base` table. The naive two-level layout was 2.85 MB for nouns
  alone — worse than flat — which is what made the first draft of Phase 10
  call the idea dead. Only the reading byte pays for itself, because an
  ending-only table still leaves `form_readings` building the paradigm. It is
  a download-budget decision now that the per-page target is met; the
  zero-byte alternative is the class-level `accepts` test, which requires
  `expand/verbs/utility.rs::paradigm` restated as data. Performance remains a
  separate engineering objective, not a rule-correctness gate.

- [ ] **Sentence-cache path finds four more diagnostics than the whole-document
  pass on `page-2000`.** Found while measuring the wasm pool: the shipping
  `check_grammar_style_with_state` (sentence cache) returns **9** grammar
  diagnostics where `check_syntax` and `syntax_check::check` return **5**. The
  four extras are at offsets 236 (`existential_subject_case`), 711
  (`comma_missing`), 9357 and 9882 (`question_mark_missing`); they are the
  same on a second call, so it is not stale state. The parity test
  `the_sentence_cache_matches_the_whole_text_pass` passes only on ten short
  texts, so the divergence is specific to sentence boundaries in a long
  document — most likely the neighbour context a sentence is checked with.
  Either the extra four are real (and the whole-document pass is missing
  them) or the sentence path is over-firing; adjudicate before changing
  either, and add the found rows to the corpus once decided.

## P3 — future syntax roadmap

### Word-order specification

- [x] **Write the construction-aware specification.** The rules, decisions and
  examples are in [`docs/word_order.md`](../docs/word_order.md) and the
  machine-readable fixture `tests/fixtures/word_order_rules.json`; the schema
  is pinned by `tests/word_order_spec.rs`. It covers wh-questions, `-ko`
  questions, imperatives, neutral SVO, existentials, passives, subordinate
  clauses and the information-structure `ALLOW` table. The decision vocabulary
  is `FORCE`/`PREFER`/`ALLOW`; SVO is a `PREFER`, never an error.
- [x] **Add the clause features** (`clause_type`, `construction`,
  `question_type`, `subject_position`, `object_case`, `focus_type`,
  `topic_position`, `register`) to the analysis. They live on
  `Clause::features` in `src/syntax/features.rs`, are pinned in
  `tests/fixtures/syntax_ambiguity.json` and the new
  `tests/fixtures/word_order_cases.json`, and are exposed by `analyze_syntax`
  in the wasm surface. `Clause` also gained `passive`, which the construction
  feature needs. The existential detection was extended to intransitive
  existence/appearance verbs with a postverbal partitive subject
  (`Talossa asuu ihmisiä`), and a postverbal coordinated subject is now marked
  (`Naapurissa asuvat Liisa ja Pekka`).
- [x] **Build the `ALLOW` regression net.** `tests/word_order_allow.rs` checks
  every example in `docs/word_order.md`: the exact diagnostic list (empty), the
  clause features, that each case really is an example on the side of the rule
  it names, and that `WordOrder` is never emitted. It immediately found two
  real false positives, both fixed: `Tuleeko hän?` and `Talossa asuu ihmisiä`
  were rewritten by the rection rule.
- [ ] **Implement the first `PREFER` suggestion**, now that the net is green;
  keep `WordOrder` suggestion-only and attach no automatic correction.
  **Held deliberately:** §6.1 of `docs/word_order.md` shows that every
  non-preferred example in the spec is also listed as an allowed alternative by
  the same rule, so a suggestion would fire on a sentence pinned clean. No
  `WordOrder` rule until there is a **structurally impossible** order (a
  genuine `FORCE`), not a merely less neutral one.
  Re-audited 2026-11-19: the six rules in §6.1 still list each non-preferred
  example as `ALLOW`, `tests/word_order_allow.rs` is green, and no `FORCE`
  construction has been identified. The item stays open by design, not blocked.

Keep the rest visible; do not implement it before the local agreement and case
work is trustworthy. The current analysis is deliberately local and advisory.

- relative clauses with complex, coordinated or distant antecedents;
- coordination with shared subjects and long-distance agreement;
- embedded clauses and nested subordination;
- infinitive and participial constructions as clause participants;
- long-distance agreement relationships;
- a non-contiguous host span for interrupted relatives;
- `ClauseKind` promotion, `Clause.sentence`, and antecedent-aware list roles —
  only when a second consumer needs them.

Area: `src/syntax/`, `src/syntax_list.rs`, `PLAN_SYNTAX.md` §“Remaining
analysis refinements”.

## Follow-up plan — neural reading disambiguation

- [x] **Audit the neural reranker integration.** The architecture audit is
  [`docs/neural_disambiguation.md`](../docs/neural_disambiguation.md). It proposes a
  per-analysis `Disambiguation` sidecar between `analyze_morphology` and
  `analyze_syntax`, preserving every existing `WordInfo.readings` candidate.
  The smallest viable milestone is an M0 candidate-preference MLP behind a
  feature flag, evaluated on UD Finnish-FTB and a frozen hard-ambiguity set;
  the BiLSTM/compact-Transformer choice comes only after that baseline.
- [x] **Phase 1 data and coverage harness.** `tools/src/ud/` reads CoNLL-U,
  maps UD morphology to versioned `fi-ud-tags-1` `Tags`, reports
  exact/partial/ambiguous/missing/unavailable coverage by POS, case and depth,
  and validates the initial manual ambiguity fixture format. No model is
  trained and no production grammar code is changed.
  Run `cargo run --release -p torille-tools --bin ud_coverage -- --file
  corpus.conllu`.
- [x] **Phase 2 exact no-op sidecar.** `src/disambiguation.rs` adds
  `Disambiguation`/`ReadingDecision`/`CandidateView` with a disabled
  `disambiguate`; the syntax, combined, sentence-cache and wasm owner paths
  consume it. `tests/disambiguation_noop.rs` proves identical `TokenSyntax`,
  clauses, diagnostics, spans, corrections, lists, style output and combined
  output with neural inference off. The sentence-cache key includes the
  disambiguation cache key (model/schema/activation). No decisions affect
  roles or severities yet.
- [x] **Phase 3 M0 experiments.** `tools/src/ud/m0.rs`,
  `tools/src/bin/ud_m0_features.rs` and `tools/neural/train_m0.py` compare
  morphology-only, local-context and sentence-context MLPs on UD Finnish-FTB.
  Three seeds: morph test top-1 0.8886 mean, local 0.8864, sentence 0.8774,
  first-candidate baseline 0.5074. Local wins dev but not test; sentence
  context is worse. Decision: do not start BiLSTM/Transformer on context gain.
  The tags and deterministic-context stack weights are embedded only behind
  the off-by-default `neural-m0` / `neural-m0-stack` features and are not
  activated.
- [x] **M0 runtime/product evaluation (implemented; negative).** The tags and
  deterministic-context stack models are embedded behind `neural-m0` and
  `neural-m0-stack`. The aggressive consumer reached UD candidate top-1
  0.8867 and decided-token role accuracy 0.6697 -> 0.6899, but added an
  `Error` `object_case` on adjudicated clean prose. The weak-hint consumer
  keeps confident and adverbial baseline roles and re-reads only weak
  predicates; it returns adjudicated clean prose to 0 diagnostics. The stack
  variant reaches candidate top-1 0.9043 and role accuracy 0.6644 -> 0.6696
  overall (0.6697 -> 0.6885 decided), with 127 role changes (71 improve, 26
  worsen). Page-2000 diagnostics still move 65 -> 69 with 44 role changes.
  Decision: do not activate. The next step is role/clause hardening, not a
  larger model.
- [x] **First role/clause hardening.** `sillä` now starts a clause when the
  next word is a personal pronoun, a non-bare negative auxiliary, or a
  nominative nominal. `sillä on väliä`, `sillä ei ole väliä`, `sillä välin`,
  `sillä tavalla` and `sillä aikaa` stay one clause. This fixed the two
  reviewed `sillä` rows (moved to `clean`) and page-2000 diagnostics 67 -> 65.
  The connegative rule now accepts a past participle after a negative
  auxiliary even when the class engine gives it no negative-polarity reading
  (`eivät uupuneet`), removing a false positive the new split exposed.
- [x] **Remaining role/clause hardening — complete.** The `sillä` split and the
  negative-past participle are fixed, and so is the argument-selection half of
  the role pass (2026-11-18, below): a subject that agrees with its verb, a
  prenominal attribute, a postposed attribute and a participle complement are
  no longer rewritten by a rection frame. On `page-2000` that removes 7 false
  positives (54 → 47 diagnostics).
  The agreement and attribute half is fixed too: a finite form that is also an
  instructive/infinitive is not an agreement bearer, `kaikki` never decides
  the number, a comma plus a personal pronoun plus its verb starts a clause,
  and only a `Modifier` in a clause with a predicate is an attribute candidate.
  On `page-2000` the two rounds together remove 20 false positives
  (54 → 34 diagnostics) with none added.
  The later precision phases fixed the remaining role/clause cues: absolute
  constructions (`lapsi sylissä`, `Päivän vaietessa`, `niitä seuraamaan`), a
  vocative that is not an argument, the fixed `sen kun` adverbial, the
  infinitive's object, reported questions and the negated-clause object.
  `page-2000` is now at **5 diagnostics**, every removal adjudicated and none
  added; all five are adjudicated with 0 false positives:
  - 2 `question_mark_missing` (1 accepted, 1 true positive);
  - 2 `subject_predicate_agreement` on colloquial `ne on` (accepted);
  - 1 `unbalanced_quote` where the excerpt ends inside a quote.
  The exact rows, their adjudication, the fixture that pins each one and the
  phase that fixed it are in
  [`PLAN_GRAMMAR_PRECISION.md`](PLAN_GRAMMAR_PRECISION.md). The
  known-false-positive list is empty; a new false positive must be added there
  first, so a fix makes the suite fail until the row is deliberately moved to
  `clean`.
  These are grammar-checker bugs independent of M0. Re-run `ud_m0_eval` and
  `m0_prose_eval` after each role change; a larger model remains blocked
  meanwhile.

## Deliberate boundaries

- Word order is not checked; Finnish order is information structure.
- The style layer never emits `Error` or `Warning`.
- Grammar and style are on-demand and never run in the keystroke path.
- No numerical probabilities; structural confidence states only.
- No automatic rewrite without a licensed correction.
- Relative clauses are linked additively; a non-contiguous host span is not
  modelled.
- The checker consumes `syntax`; it must not build a second parser.

## Verification

Current gates:

```bash
cargo test --release -p torille --test syntax_check --test syntax_eval --test syntax_ambiguity --test syntax_prose --test style_check --test style_eval
cargo test --release -p torille
cargo clippy -p torille --all-targets -- -D warnings
./target/release/check_syntax "Joka päivä juon kahvia."
./target/release/check_syntax "Ilman rahaa en tule."
./target/release/check_style "Tämä on on hyvä."
```

Run the prose suite with `-- --nocapture` to print the adjudicated word count
and the known false-positive rate. The ambiguity gate prints each
intended/competing interpretation pair that fails.

## Work log

- 2026-11-15: audit written; landed the first P0 slice:
  determiner `joka`/`jokainen` + temporal noun, preposition complements,
  reading-consistent nominal case selection, the rection total-object guard,
  and the `sen`/`ne` pronoun-object override. Added clean fixture rows and
  unit tests for the role changes.
  `cargo test --release -p torille` and `cargo clippy -p torille --all-targets
  -- -D warnings` are green.
- 2026-11-15: revised this roadmap after review. Added P0 ambiguity and
  adjudicated-prose gates, a shared diagnostic/confidence contract,
  detection-vs-correction confidence, explainability, subsystem grouping,
  conservative duplicate-word policy, and a future long-distance syntax list.
- 2026-11-15: landed P0-A. Added `tests/fixtures/syntax_ambiguity.json` and
  `tests/syntax_ambiguity.rs`; the suite pins the intended role, the rejected
  competing role, and the clean diagnostic list for thirteen ambiguous
  constructions.
- 2026-11-15: landed the P0-B seed. Added
  `tests/fixtures/prose/adjudicated.json` and `tests/syntax_prose.rs`:
  15 clean rows, 5 known-false-positive rows and 1 true-positive row
  (381 adjudicated correct words, 17 known false-positive errors, 44.6 per
  1,000 words at this seed size). The suite fails if a clean row stops being
  quiet or a known row changes, so a fix must move the row rather than leave a
  stale baseline.
  Moved adjudicated prose before the P1 rule expansion.
- 2026-11-15: landed the first P1 rule, predicative adjective agreement.
  `Talot ovat punainen.` and `Talo on punaisia.` emit
  `predicative_agreement` with adjective number corrections; the clean corpus
  adds `Talot ovat punaisia.`, `Talot ovat punaiset.`, `Talo on punainen.`,
  `Lapset ovat väsyneitä.`, `Hän on väsynyt.` and `He ovat opettajia.`.
  Added exact-output and ambiguity cases, updated the UI labels and the rule
  catalogue. The prose corpus recorded no new false positives.
- 2026-11-15: landed the connegative rule. `En menen.` and `Hän ei menee.`
  now emit `connegative` with `mene`; `En menin.` offers `mennyt`. Clean rows
  cover `En mene.`, `Hän ei mene.`, `En ole.`, `En mennyt.`, `En tullut.`,
  `Älä mene.`, `Älkää menkö.` and `En voi mennä.` The rule required two prose
  corrections during development: it now checks only the verb adjacent to the
  auxiliary (so `Ei, sillä ...` is not an auxiliary) and accepts the 3sg
  conditional as the conditional connegative (`ettei häntä pelottaisi`).
- 2026-11-15: landed numeral government. `Kaksi lapsi tuli.`,
  `Kaksi lapsia tuli.` and `Ostin kaksi kirja.` now emit `case_selection`
  with the partitive singular. Clean rows cover `Kaksi lasta tuli.`,
  `Yksi lapsi tuli.`, `Kahdet kengät.`, `Kaksi pientä lasta.`,
  `Kaksi kolmesta.`, `Sata autoa.` and `Kaksikymmentä lasta.`. The same work
  added a numeral-phrase guard to `attribute_agreement`, removing the
  `Kaksi pientä lasta` false positive.
- 2026-11-15: implemented Phase 1 of the neural-disambiguation plan.
  Added the CoNLL-U reader, versioned `fi-ud-tags-1` UD→`Tags` mapping,
  candidate/lemma-provenance adapter, coverage report by POS/case/depth, the
  manual ambiguity fixture format, and `ud_coverage`. No model was trained and
  no production grammar code or public API changed. The committed sample
  reports 11 word tokens / 100% exact+partial coverage; a UD Finnish-FTB smoke
  run reports 13,904 word tokens / 80.6% coverage, with mapping and candidate
  categories reported separately.
- 2026-11-15: implemented Phase 2 of the neural-disambiguation plan.
  Added the disabled `Disambiguation`/`ReadingDecision`/`CandidateView` seam,
  threaded it through syntax, the combined pass, `SentenceCache` and the wasm
  owner paths, and added the exact no-op gate over tokens, clauses,
  diagnostics, spans, corrections, lists, style and combined output. The
  sentence cache now includes the model/schema/activation key. No model is
  active and no role, severity or diagnostic changed.
- 2026-11-15: implemented Phase 3 M0 experiments. Added M0 feature extraction,
  a numpy-only three-variant trainer and `tools/neural/m0_report.json`.
  UD Finnish-FTB results across three seeds: morph test top-1 0.8886 mean,
  local 0.8864, sentence 0.8774, first-candidate baseline 0.5074. Local
  improves dev but not test, so the project stops before BiLSTM/Transformer.
- 2026-11-15: implemented the optional M0 runtime/product evaluation. Added the
  tags-only `models/m0-tags-v1.bin`, the `neural-m0` feature with embedded
  inference, `classify_with_preference`, `ud_m0_eval` and `m0_prose_eval`.
  The aggressive consumer added an `Error` on adjudicated clean prose; the
  weak-hint consumer fixed that and kept adjudicated prose at baseline, with
  UD role accuracy 0.6644 -> 0.6662 overall (0.6697 -> 0.6803 decided). The
  page-2000 smoke still moves 65 -> 70, so the model stays off by default and
  the default build remains an exact no-op.
- 2026-11-15: added the deterministic-context stack variant. `train_m0.py`
  now supports `--variants stack`, `combine_m0.py` measures reading-order
  prior fusion (it hurts: model 0.8867 -> 0.8829 test at best), and
  `models/m0-stack-v1.bin` is embedded behind `neural-m0-stack`. Three-seed
  stack test top-1 is 0.9027 mean (0.9015–0.9043), and the weak-hint consumer
  reaches UD role accuracy 0.6696 overall / 0.6885 decided. Adjudicated prose
  stays at baseline; page-2000 diagnostics move 65 -> 69. Still not activated.
- 2026-11-15: first role/clause hardening. `sillä` starts a clause only when
  the next word can open one, so `sillä välin` and `sillä tavalla` stay
  adverbial. The split moves two reviewed `sillä` rows from known false
  positives to clean and reduces page-2000 diagnostics 67 -> 65. The
  connegative rule now accepts the negative past/perfect participle
  (`eivät uupuneet`) even when the class engine omits the negative-polarity
  reading, removing the false positive the split exposed.
- 2026-11-17: landed the four remaining P0-C prose false positives. Class 101
  tags `me`/`te`/`he` plural; a personal pronoun heads its apposition; `ettei`
  opens a clause; `kuin` is a comparison marker; unknown capitalized names get
  a role-only subject; relative clauses do not inherit the main subject;
  postpositions and temporal essives are adverbials. The landing also re-reads
  a demoted weak predicate from its non-verb readings and makes the
  negative-object rule skip the clause subject. All four rows moved to
  `clean`; the prose fixture is 66 rows / 1,021 words with 0 known false
  positives and 0 known errors.
- 2026-11-17: landed adposition case government. Added
  `data/adposition_cases.csv`, its build emitter and embedded map, and
  `src/syntax_check/adposition.rs`. `Pöytä alla` now offers `Pöydän` and
  `ilman raha` offers `rahaa`; the standalone adverb (`Kissa on alla`) and a
  genitive modifier after a partitive-governing preposition
  (`ilman Jumalan tahtomatta`) stay quiet. The full page-2000 sweep found
  exactly that one false positive before the guard; the adjudicated prose and
  clean syntax corpora are unchanged.
- 2026-11-17: landed the shared diagnostic contract (P1-A..D). Added
  `src/diagnostic.rs` with `RuleId`, `Confidence::{Certain,Likely,Ambiguous}`,
  `Explanation`/`Relation`, `CorrectionLicense`, and the `Finding` trait.
  `SyntaxDiagnostic` and `StyleDiagnostic` now carry `rule_id` and
  `explanation`; `CheckAll::findings()` exposes both streams through one
  contract. `rank::resolve` suppresses `Ambiguous` findings and downgrades an
  `Error` whose confidence is only `Likely` to `Warning`. Corrections carry a
  license and their own confidence; the wasm object exports `rule_id`,
  `explanation`, and the correction license/confidence. The syntax and style
  panels show the rule id and relation. Added `ambiguous_tuli_subject` to the
  ambiguity fixture and a cross-layer contract test. All workspace tests and
  clippy are green; the checked prose baseline is unchanged.
- 2026-11-17: landed existential subject case. `Pöydällä on kirjan.` emits
  `case_selection` with both engine-licensed repairs (`kirja`, `kirjaa`); the
  diagnostic is `Certain` while each correction is `Likely`, so detection and
  correction confidence stay separate. The existential flag now accepts a
  genitive post-predicate nominal. The rule is limited to a sentence-final
  common noun/adjective and skips pronouns, proper names, and clauses split at
  a line break; the page-2000 sweep is quiet. Added diagnostic, clean and
  ambiguity cases.
- 2026-11-17: landed compound-tense participle agreement. The rule compares the
  subject's number with the active past participle of an `olla` compound tense
  (`Me olemme lukenut.` -> `lukeneet`, `Minä olen lukeneet.` -> `lukenut`).
  It filters the passive homonyms of `mennyt`/`tullut`, skips passive compound
  tenses, and falls back through `lemmas` when the chosen base is an adjective
  (`väsynyt`). Added diagnostic, clean and ambiguity cases; the page-2000
  sweep has no `participle_agreement` findings.
- 2026-11-17: landed bare nominative object. `Minä näen kirja.` emits
  `object_case` with `kirjan`/`kirjaa`. The rule requires an overt preverbal
  subject, a postverbal singular common-noun object with no possessive or
  competing nominal case, and a clause-final object; it skips imperative,
  passive, necessive and comparison constructions. The first prose sweep
  flagged 16 role-path false positives (coordinated subjects, vocatives,
  `lasta`, comparisons); the final guards bring page-2000 to zero.
- 2026-11-17: landed the conservative clause-structure warnings in
  `src/syntax_check/structure.rs`. `Koska minä väsynyt.` emits
  `missing_predicate` and `Hän tuli ja.` emits `incomplete_coordination`, both
  `clause_structure`/`Warning` with no correction. The rule only fires at a
  declarative period; questions, headings, comma-continued clauses and
  period-less fragments stay quiet. The page-2000 sweep has no
  `clause_structure` findings.
- 2026-11-17: landed possessive-suffix agreement in
  `src/syntax_check/possessive.rs`. A wrong suffix is a `Warning`
  (`Minun kirjasi ...`), a missing suffix a `Suggestion` (`Minun kirja ...`);
  neither is ever an `Error` and neither carries a correction. Matching
  suffixes, standalone suffixes, and necessive genitive subjects
  (`Teidän täytyy ...`) stay quiet. Added diagnostic, clean and ambiguity
  cases; the page-2000 sweep has no possessive findings.
- 2026-11-17: landed the extra-comma rule. `Minä, menen kotiin.` emits
  `comma_extra` as a `Warning` with a comma-removal `Likely` correction. Only
  a personal-pronoun subject directly before its finite predicate is checked;
  appositions, parentheticals, noun-subject addresses and intervening words
  stay quiet. The page-2000 sweep has no `comma_extra` findings.
- 2026-11-17: landed `style_duplicate_word` in `src/style_check/repetition.rs`.
  Adjacent identical surfaces with only whitespace between them are a
  `Suggestion` with a delete-the-second-word correction. Punctuation, the
  rhetorical list (`hyvin hyvin`, `kauan kauan`, ...) and phrase-table spans
  stay quiet; the duplicate takes precedence over the near-repeat metric. The
  page-2000 sweep has no duplicate-word findings.
- 2026-11-17: grew the curated style tables with reviewed rows:
  `periaatteessa`, `tosiasiassa`, `sinänsä`, `joka tapauksessa` (filler),
  `jollain tavalla` -> `jotenkin` (wordy) and `jossain määrin` (wordy, label
  only). Left `niin kuin` and `ikään kuin` out because they are standard
  comparators rather than fillers. Added exact-output style cases; the clean
  style corpus and page-2000 remain quiet.
- 2026-11-17: landed rule-level dismiss and the combined command. Grammar and
  style diagnostics now carry a stable `rule_id` through the wasm wire; the
  panels expose an "älä huomauta" action, persist the ignored ids in
  `torille-ignored-rules`, and the options page can clear them. The Rust
  `without_rules`/`ignore_rules` API filters both streams. The context menu
  adds "Tarkista kielioppi ja tyyli" over the existing combined
  `check-grammar-style` pass and renders one ranked panel. Firefox and
  Chromium sources remain identical; both typecheck and extension tests pass.
- 2026-11-17: added the adjective-degree dimension. `Tags.degree`
  (`Comparative`/`Superlative`), the morphology codec and `degree_forms` now
  carry it, and `attribute_agreement`/`predicative_agreement` compare degree
  forms like ordinary adjectives. `Punaisempi autot ovat hienoja.` is now a
  number-agreement error. Degree forms are derived `Repaired` words, so the
  finding is diagnostic-only; the clean rows cover the agreeing degree forms.
- 2026-11-17: landed predicative noun complements. `He ovat opettaja.` emits
  `predicative_agreement`/`predicative_noun_agreement` as a `Warning` with
  `opettajia`/`opettajat`; `Hän on opettajat.` suggests `opettaja`. The rule is
  separate from the adjective number rule and only fires on a nominative
  number mismatch; partitive predicates (`He ovat opettajia.`,
  `Ne ovat vettä.`) stay quiet. `Lapset ovat väsynyt.` was already covered by
  the compound-participle rule (`väsyneet`). Page-2000 has no noun-complement
  findings.
- 2026-11-17: wrote the construction-aware word-order specification in
  `docs/word_order.md`, with the machine-readable rule set in
  `tests/fixtures/word_order_rules.json` and the schema gate in
  `tests/word_order_spec.rs`. The model is `FORCE`/`PREFER`/`ALLOW`; SVO is a
  `PREFER`, never an error, and object/topic/temporal/locative fronting,
  passive, existential and contrastive orders are `ALLOW`. The rule engine
  stays deferred until the clause features are present.
- 2026-11-17: implemented the word-order clause features on `Clause::features`
  (`src/syntax/features.rs`): `clause_type`, `construction`, `question_type`,
  `subject_position`, `object_case`, `focus_type`, `topic_position`,
  `register`. Added `Clause::passive`, extended the existential detection to
  intransitive existence verbs with a partitive postverbal subject, and marked
  a postverbal coordinated subject. Every example in `docs/word_order.md` is
  now a green `ALLOW`/preferred regression case in
  `tests/fixtures/word_order_cases.json` + `tests/word_order_allow.rs`, which
  found and closed two rection false positives (`Tuleeko hän?`,
  `Talossa asuu ihmisiä`). The wasm clause object exposes the features.
  Narrow/broad focus, a formal register and any `FORCE` rule remain
  unimplemented because there is no cue for them.
- 2026-11-18: cut the cold morphology pass by sharing one paradigm build. The
  `emits_form` verification now keeps the readings of the surface it was asked
  about, so `form_readings` answers from that probe instead of walking the
  paradigm a second time (609 of 1,398 extractions on the perf fixture); the
  verification walk skips `split::dedup` and folds the matched copies with
  `Form::absorb`, which is the same merge; `modifier_is_known` is memoised per
  modifier. Cold single-thread 2,016 → 1,490 ms, fresh-cache style ~1,410 →
  ~1,140 ms, 4 native workers 753 → 537 ms, warm unchanged. The attribution
  experiment (verb stem map, cheap fallback, both) is recorded in
  `PLAN_GRAMMAR_STYLE_PERF.md`.
- 2026-11-18: role/argument hardening. The rection rule now requires a
  *genuine* argument: a `Subject`-role token is skipped when the finite
  predicate agrees with it (person and number for a pronoun, 3rd person and
  number for a nominal), when the clause is existential, or when it has
  coordinated subjects. A partitive adjective after a nominal is that
  nominal's attribute (`jotain erinomaista`). The role pass marks prenominal
  attributes and demonstrative determiners before subject selection (so
  `Rinteellä tuli hirvittävä susi` names `susi`, and `se kesy hirvi` names
  `hirvi`), marks a partitive extent before the `-päin` direction adverbs as
  their complement, and restricts the participle complement to `olla`. `jäädä`
  and `pysyä` joined the essive/translative complement table. Measured on
  `page-2000`: 54 → 47 diagnostics, all seven removed rows adjudicated as
  false positives, none added; the clean corpus, the ambiguity fixture and
  `m0_prose_eval` (66 rows, 0 diagnostics) stay green.
- 2026-11-18: agreement and attribute hardening (second round of the same
  item). The agreement rule skips a bearer that is also an instructive or
  infinitive (`ihastellen`, `elää`, `ihmetellen`) and never takes `kaikki` as
  the number cue. A comma followed by a personal pronoun and then a finite
  predicate starts a clause (`kun häneltä kysyttiin, hän vastasi`). Only a
  `Modifier` in a clause with a predicate is an attribute candidate, and among
  the pronouns only the demonstratives are attributive, so `minulle`,
  `niille`, `häntä` and `mutta` are no longer compared with the following
  noun. Measured on `page-2000`: 47 → 34 diagnostics, all removed rows
  adjudicated false positives, none added; 11 new clean rows and two ambiguity
  cases pin the behaviour.
- 2026-11-18: inverted order and non-finite subjects (third round). A finite
  predicate followed by a nominative personal pronoun takes it as its subject
  (`Sen sanoi hän.`, `Kysyi hän sitten.`), which also stops the quoted
  vocative (`..., Heintsi', sanoi hän`) from becoming one; a postverbal
  coordinated subject must be a noun, so a coordinated adjective no longer
  makes `oli hän yhtä viaton ja herttainen` plural. An infinitive clause
  agrees through the governing verb (`alkoi tulla … puute` is quiet,
  `Haluan mennä koulu` still fires). A 1st/2nd-person predicate takes only a
  pronoun as its subject, and `vaikka` opens a clause only when a predicate
  follows. `page-2000`: 34 → 27 diagnostics, all removals adjudicated, none
  added; four clean rows and three ambiguity cases added.
- 2026-11-18: wrote `PLAN_GRAMMAR_PRECISION.md`, the phase plan for the 21
  remaining false positives on `page-2000` (27 diagnostics: 21 FPs, 5 accepted,
  1 excerpt artefact). Phase 0 landed: `tools/src/bin/dump_diagnostics.rs` is
  the review tool, and the 13 rows that reproduce outside the page (14
  findings) are pinned in `known_false_positives`, so the suite now reports
  11.4 findings per 1,000 words and fails when a fix lands without moving its
  row. The plan also records two structural root causes: a newline ends a
  sentence (`sentence::separator_ends_sentence`), and a comma is invisible to
  the boundary tests when a quote or a relative clause sits beside it.
- 2026-11-18: precision Phase 1 (inherited subject keeps its number). The
  `tuuli` agreement row was not a bearer problem: the page wraps
  `tuuli\npuhalteli`, so the newline sentence split leaves `tuuli` as a
  one-word predicate, and `share_clause_roles` copied `tähdet` into that
  clause because `clause_subject_person_number` looked only at subject tokens
  inside the clause and returned unknown for an inherited one. It now includes
  `clause.subject`, so a mismatching subject is not shared; the clause has no
  controller and stays quiet. `page-2000`: 27 → 26, exactly the one row; the
  original line break is a clean fixture in `syntax_diagnostics.json`.
  Agreement true positives (`Koira nukkuvat.`, `Minä menee kotiin.`,
  `Hän antoivat kirjan.`) still fire.
- 2026-11-18: precision Phase 2 (a comma already at the boundary counts).
  Three mechanisms, one family: the `jolloin` relative clause merged into the
  main `Clause` no longer gets a second comma (`comma_before_relative`, which
  knows the relative adverb but does not change global clause segmentation);
  `same_sentence` no longer reaches across an unspanned terminal mark like
  `. 'Jos` (a terminal mark followed by an apostrophe quote); and `is_finite`
  rejects a finite reading that carries a possessive suffix, so the
  postposition `yllään` is not a relative clause's closing host. Rows 2 and 3
  moved from `known_false_positives` to `clean`; row 4's `yllään` sentence is
  a clean fixture. `page-2000`: 26 → 23, exactly these three. The
  `subordinate_comma_*` and `relative_comma_pair` fixtures still fire.
- 2026-11-18: precision Phase 3 (fixed multiword conjunctions). `NO_COMMA_PAIRS`
  stops the boundary comma inside `niin että` and `sen kun`; the merged path
  now skips whenever a comma already sits between its last two finite words,
  which also closes the second `sen kun` finding (`koko ajan` was read as a
  1sg predicate). `Luulen että hän tulee.` still reports the missing comma;
  `Luulen, että hän tulee.` and `Niin että hän tuli.` are clean. `page-2000`:
  23 → 20, exactly rows 5–7.
- 2026-11-18: precision Phase 4 (demonstrative subject, relative adverb).
  `jolloin` is a relative adverb and now opens its own clause, so `kaikki …
  kiittävät` is that clause's subject rather than an argument of `tulee`.
  For `… katsokaa, se peitäkse …`, rection skips a demonstrative pronoun that
  a comma separates from the predicate, and the negative-object rule now
  requires the negative auxiliary to precede the clause predicate (the `ei`
  belongs to `ikäänkuin ei voisi`). `Katsokaa se!` still fires. `page-2000`:
  20 → 18, exactly rows 8–9.
- 2026-11-18: precision Phase 5a (comparison clauses). `niinkuin`/`ikäänkuin`
  now open a clause when a predicate follows, so `… kerro, niinkuin se sitten
  kävi` makes `se` the subject of `kävi`. They are not subordinators: a comma
  before a comparison clause is stylistic, and the subordinator route added
  two false comma rows on the page. `page-2000`: 18 → 17, row 10 only; the
  `niinkuin` prose row moves to clean and `Meille on niinkuin ennenkin haettu
  apua.` is a clean fixture.
- 2026-11-18: precision Phase 5b (absolute phrases). A nominative nominal
  directly before an inessive nominal/adverb, after an overt clause subject,
  is a detached phrase (`Hän jäi lapsi sylissä istumaan`), not the object;
  it is marked Adverbial before the object pass, so the illative frame cannot
  rewrite it. `Hän istui tuolissa.` and `Kirja pöydällä on minun.` are clean
  guard fixtures. `page-2000`: 17 → 15, rows 11–12 only.
- 2026-11-18: precision Phase 5c (genitive subject of an impersonal
  non-finite verb). When a clause ends in a genitive nominal and the next word
  is `vaieta`/`hämärtää`/`sarastaa`/`koittaa`/`valjeta`/`pimetä` as a
  possessive-less infinitive, the genitive is that verb's subject
  (`Päivän vaietessa`) and is detached before the object pass.
  `Kirjan luettuaan` keeps `kirjan` as the object because the possessive
  suffix marks the agent; the role pair is pinned in `syntax_ambiguity.json`.
  `page-2000`: 15 → 14, row 13 only. Phase 5 is complete.
- 2026-11-18: precision Phase 6 (infinitive object; satisfied frame). A
  clause-final nominal followed by an infinitive clause belongs to the
  infinitive (`lähti niitä seuraamaan`), so the matrix rection search skips
  it. A frame is satisfied by another nominal in the clause in its case, or
  by the closed goal postposition `haltuun` (`uskon sinut … haltuun`); the
  predicate itself is excluded because its homonyms can carry an illative
  noun reading (`Vastaan sinä.` must keep firing). `page-2000`: 14 → 12,
  rows 14–15 only.
- 2026-11-18: precision Phase 7 (vocative; quantity head). A clause-final
  nominative followed by a comma and an imperative clause is an address when
  a colon or quote precedes it (`vastasi: 'Jumala, ole …`); it is marked
  Adverbial before the object pass, so the rection frame cannot rewrite it.
  An adjective directly after `paljon`/`vähän`/`runsaasti`/`tarpeeksi` is the
  head of the quantity phrase, not an attribute. `Jumala, ole …`,
  `Liisa, tule …` and `Kuinka paljon hyvää …` are clean; `Minä, menen kotiin.`
  stays comma_extra. `page-2000`: 12 → 10, rows 16–17 only.
- 2026-11-18: precision Phase 8 (list detection). `list_separator_missing`
  needs the adjacent pair to share a case: `lapsineen suojassa sateelta ja
  tuulelta` is four obliques, and `vaatteista puute` is elative next to
  nominative. The `list_*` and nested-list fixtures still fire.
  `page-2000`: 10 → 7, rows 18–20 only.
- 2026-11-18: precision Phase 9 (reported questions). The interrogative must
  open the sentence, not just its clause, and a comma before the sentence
  (possibly with a line break or quote) marks a continuation. Rows 21 and 23
  are reported questions; row 22 stays accepted as a direct `-ko` question.
  `page-2000`: 7 → 5. The precision plan is complete: 54 → 5 diagnostics, of
  which 0 false positives (1 true positive, 3 accepted, 1 excerpt artefact);
  the prefix counts are 20 words → 0, 96 → 0, 1,959 → 5.
- 2026-11-19: reconciled this roadmap with the completed precision plan. The
  live status now says `page-2000` is 5 diagnostics / 0 false positives and the
  reviewed-prose gate is 79 clean rows / 1,233 words, 1 true positive, 0 known
  false positives. The role/clause hardening item is closed: its stale "still
  open" list (absolute constructions, vocative, `sen kun`, indirect questions)
  all landed in `PLAN_GRAMMAR_PRECISION.md`. The only gated items left are the
  cold-page target, the `koko` temporal verification and the held `PREFER`
  rule. No engine or checker behaviour changed.
- 2026-11-19: extended the adjudicated prose corpus with 61 clean paragraphs of
  modern formal Finnish from Finlex statute 729/2018 (Tieliikennelaki). The raw
  Akoma Ntoso XML is gitignored under `tests/fixtures/prose/sources/`; only the
  adjudicated rows and their provenance (`tests/fixtures/prose/README.md`) are
  committed. Every row was admitted only after an individual `check_syntax`
  returned zero diagnostics. The suite is now **140 clean rows / 2,439 words**,
  1 true positive and 0 known false positives. The same sweep left 78 candidate
  paragraphs with diagnostics; the representative false-positive shapes are
  recorded under P0-C as the seed for the next precision round.
- 2026-11-19: closed the `koko` + temporal-noun item. `koko päivä` is a valid
  subject (`Koko päivä meni hukkaan.`) and `koko päivän` is the duration
  adverbial, so `koko` is not added to the `joka`/`jokainen` frequency
  determiners. The ambiguity fixture pins the subject reading, the clean corpus
  keeps both correct forms quiet, and a unit test keeps `Koko päivä juoksin.`
  from offering the 3sg `juoksi`.
- 2026-11-19: added a 64-bit suffix stem-shape mask beside the 256-bit prefix
  mask (`LSS3`, 40 bytes/row). The generator now unions `inflect_all` with
  `None` and `Some(&LexiconIndex)`: class-50 compound heads emit forms only on
  the index path, so the prefix-only mask could reject a real form. Native
  single-thread cold is 1,497 → **1,330 ms** and the native 4-worker path is
  **450 ms**, under the ≤500 ms budget. The wasm pool is still **755–762 ms**
  with 4 workers, so the cold-page item stays open.
- 2026-11-19: removed the serial numeral upgrade. The partitive upgrade fired
  after *any* numeral, so `kahtena päivänä`, `kolmesta miehestä`,
  `kahdella miehellä`, `menoon`, `kotona` and `suusta` each ran the head/tail
  deep walk on the owning thread after every pool worker had finished. Only a
  nominative cardinal governs a partitive; `numeral_governs_partitive` now
  checks the reading's case. Native single-thread cold **1,330 → ~1,190 ms**,
  native 4-worker **450 → 319–348 ms**, and the wasm post-seed final pass
  **615 → 72 ms**. Diagnostics unchanged (5 grammar / 14 style on the
  fixture); `a_numeral_upgrades_the_partitive_homonym` still passes and
  `an_inflected_numeral_does_not_trigger_the_partitive_upgrade` pins
  `deep_upgrades`. See `PLAN_GRAMMAR_STYLE_PERF.md` §Phase 11.
- 2026-11-19: extended the prose corpus with 77 clean paragraphs from seven
  Projekti Lönnrot novels and story collections (Haanpää, Lassila, Lehtonen,
  Kilpi, Jotuni; 1900–1953). The raw zips are gitignored and
  `tests/fixtures/prose/README.md` records the IDs and licences. Every row was
  admitted only after an individual `check_syntax` returned zero diagnostics.
  The suite is now **217 clean rows / 4,861 words**, 1 true positive and 0
  known false positives. The rejected novel paragraphs are dominated by
  `comma_missing`, `subject_predicate_agreement`, `verb_rection` and
  `object_case`; P0-C records the representative shapes for the next precision
  round.
- 2026-11-19: extended the prose corpus with 29 clean factual paragraphs from
  three Doria (Kansalliskirjasto) Public Domain Mark 1.0 books: Hult's
  *Suomen maantiede* and Setälä's *Äidinkielen oppikirja* (parts 1–2), all
  1900. The raw PDFs and OCR text are gitignored; README records the item IDs
  and the explicit OCR corrections (`parsaheiuää` → `parsaheinää`, etc.). A
  corrected row was admitted only after it passed `check_syntax`; uncorrected
  OCR text is not admitted. The suite is now **246 clean rows / 5,789 words**,
  1 true positive and 0 known false positives. This adds expository prose to
  the Gutenberg/Lönnrot fiction and Finlex legal text.
- 2026-11-19: made manually curated prose a first-class fixture.
  `adjudicated.json` keeps the Gutenberg/Finlex adjudicated rows;
  `tests/fixtures/prose/curated/*.txt` holds one curated paragraph per line and
  the suite checks every line. Moved the 106 Lönnrot/Doria rows there and added
  six reviewed Doria paragraphs and two Iisakki paragraphs. Added the 1900
  `ne voi` paragraph to `errors` as a second true positive. Suite: 140
  adjudicated + 126 curated = **266 clean rows / 6,349 words**, 2 true
  positives and 0 known false positives.
- 2026-11-19: precision fixes from the new corpora. `connegative` now accepts a
  passive connegative after an intervening adverb (`ei toisin säädetä`) but
  still stops at a coordinated negative (`eikä tarpeetonta estettä ...`).
  `adposition_case` skips a possessive complement (`kykynsä mukaan`), whose
  case the suffix hides. Both have regressions; the deeper
  `erityiset olosuhteet` agreement and `ilman` comma segmentation findings are
  recorded for the next round rather than guessed at.
- 2026-11-20: punctuation precision. `comma_missing` no longer fires across a
  `kuin` comparison (`niin kauan kuin kaikki ovat yhtä köyhiä`); `question_mark`
  skips a dash-interrupted question at the dash, a question continued on the
  next line, a correlative relative (`Minkä …, se …`) and a rhetorical
  conditional (`Mitä … tulisinkaan …, jos …`), while still marking the final
  period. `page-2000` is 5 → 3 diagnostics (the two agreement rows and the
  document-end open quote remain); the prose suite stays 266 rows / 6,349 words
  with 2 true positives and 0 known false positives.
