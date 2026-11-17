# Word-order specification

Finnish word order is not an SVO grammar. Grammatical roles come primarily
from case marking, and most departures from neutral order are information
structure, not errors. This document is the specification for any future
`WordOrder` diagnostics and for the analysis features they need. It replaces
the old “word order is deferred” decision with a construction-aware model.

The machine-readable copy is
[`tests/fixtures/word_order_rules.json`](../tests/fixtures/word_order_rules.json).
It is a specification fixture: no runtime rule reads it yet.

## 1. Decisions

Every rule returns one of three decisions, never a bare “wrong order”:

| decision | meaning |
|---|---|
| `FORCE` | the construction imposes a genuine structural requirement |
| `PREFER` | one order is the neutral or natural choice; the alternative is grammatical |
| `ALLOW` | multiple orders are grammatical, with a different information structure |

The diagnostic layer treats `PREFER` as a suggestion-only observation and
`ALLOW` as silence. `FORCE` is reserved for constructions that are
unambiguously structural; no current rule sets it. In particular, **SVO is a
`PREFER` for neutral declaratives, never an error**.

## 2. What must be analysed first

Word order cannot be judged from positions alone. The validator needs:

- `clause_type`: declarative, interrogative, imperative, subordinate;
- `construction`: existential, passive, copular, possessive, ordinary;
- `subject_position`: preverbal, postverbal, omitted;
- `object_case`: partitive, accusative, genitive, nominative (from morphology,
  never from position);
- `question_type`: wh, yes/no, alternative, echo;
- `focus_type`: neutral, contrastive;
- `topic_position`: initial, non-initial;
- `register`: neutral, colloquial;
- `constraint_level`: hard, preferred, optional — the rule's own decision, not
  a clause feature.

`object_case` in particular must be resolved before the order rule runs:
`Kirjan luin` is object fronting, not a subject + verb.

### 2.1 Implementation status

The features are implemented on `Clause::features`
(`src/syntax/features.rs`) and computed after role resolution, so every field
reads the final case and role assignment:

| feature | cue |
|---|---|
| `clause_type` | imperative mood + no subject or `!`; `?` or a `-ko` clitic; else a subordinator or a relative parent; else declarative |
| `construction` | `clause.passive`; an adessive personal possessor + `olla`; `clause.existential`; `olla` + complement; else ordinary |
| `question_type` | `vai` + a clitic → alternative; a clitic → yes/no; a clause-initial interrogative → wh; a trailing interrogative clause → echo |
| `subject_position` | the subject token against the predicate token |
| `object_case` | the resolved object's case |
| `focus_type` | an all-caps word the lexicon knows |
| `topic_position` | whether a constituent precedes the finite verb |
| `register` | a closed colloquial surface list |

Three values the specification lists are deliberately **not** produced, because
the analysis has no cue for them and a guess would defeat the purpose:

- `focus_type` `narrow`/`broad` needs prosody or discourse;
- `register` `formal` needs a register model;
- a `FORCE`-level `constraint_level`, because no current rule is a hard
  structural requirement.

`construction` describes the clause's *order and frame*, so `Kirja on
pöydällä` is `ordinary` while `Pöydällä on kirja` is `existential`; the two have
the same proposition and different information structure, which is exactly what
the order rule must see.

`existential` itself was extended for this: an intransitive verb of existence or
appearance with a locative and a postverbal **partitive** now counts
(`Talossa asuu ihmisiä`, `Puutarhassa kasvaa kukkia`). The partitive is the
cue, because a nominative subject agrees with its verb (`Naapurissa asuvat
Liisa ja Pekka`) and must keep going through the agreement rule.

## 3. Rules

### FI_QUESTION_001 — wh-question fronting

A direct wh-question normally starts with its interrogative phrase.

| example | reading |
|---|---|
| `Missä hän asuu?` | neutral, correct |
| `Hän asuu missä?` | marked / echo question |
| `Miksi tulit tänne?` | neutral, correct |
| `Tulit tänne miksi?` | marked / echo question |

Decision: `PREFER`, priority 100, `hard_constraint: false`.
Exceptions: echo question, contrastive focus, spoken language. Without a
reliable echo/focus cue, the checker stays quiet.

### FI_QUESTION_002 — yes/no questions with `-ko`/`-kö`

The constituent carrying the question clitic normally precedes the rest of the
question. The clitic attaches to the focused constituent, not necessarily the
verb.

| example | reading |
|---|---|
| `Tuleeko hän?` | neutral, correct |
| `Oletko valmis?` | neutral, correct |
| `Helsingissäkö hän asuu?` | locative focus, correct |
| `Hän tuleeko?` | marked/contrastive, not a neutral question |

Decision: `PREFER`, priority 95, `hard_constraint: false`.
Exceptions: echo question, contrastive focus, spoken language.

### FI_IMPERATIVE_001 — imperative

The finite imperative verb normally precedes its complements in a neutral
command. An understood subject pronoun is normally omitted.

| example | reading |
|---|---|
| `Tule tänne!` | neutral, correct |
| `Avaa ovi!` | neutral, correct |
| `Lukekaa kirja!` | neutral, correct |
| `Tänne tule!` | marked/contrastive |

Decision: `PREFER`, priority 80, `hard_constraint: false`.
Exceptions: contrastive focus, discourse context.

### FI_ORDER_001 — neutral declarative

The neutral, unmarked declarative is subject–finite verb–object.

| example | reading |
|---|---|
| `Minä ostin auton.` | neutral |
| `Hän lukee kirjaa.` | neutral |
| `Lapset pelaavat jalkapalloa.` | neutral |
| `Auton ostin.` | object fronting, allowed |
| `Eilen kävin kaupassa.` | temporal fronting, allowed |

Decision: `PREFER`, priority 10, `hard_constraint: false`.
This is a generation preference, not a correctness rule. Exceptions:
topic fronting, focus fronting, existential, passive, stylistic inversion.

### FI_EXISTENTIAL_001 — existential

An existential construction normally puts the location/existence context
before the finite verb and introduces the entity after it.

| example | reading |
|---|---|
| `Pöydällä on kirja.` | neutral existential |
| `Talossa asuu ihmisiä.` | neutral existential |
| `Puutarhassa kasvaa kukkia.` | neutral existential |
| `Kirja on pöydällä.` | grammatical, different information structure |

Decision: `PREFER`, priority 90, `hard_constraint: false`.
Do not rewrite an existential into an ordinary SVO sentence.

### FI_PASSIVE_001 — passive

A Finnish passive clause does not require an explicit grammatical subject.

| example | reading |
|---|---|
| `Täällä puhutaan suomea.` | neutral passive |
| `Talossa rakennetaan uutta huonetta.` | neutral passive |
| `Asia on jo käsitelty.` | passive perfect |
| `Täällä suomea puhutaan.` | marked object fronting, allowed |

Decision: `ALLOW`, priority 90, `hard_constraint: false`.
Never add a subject or force SVO onto a passive clause.

### FI_SUBORDINATE_001 — subordinate clause

A subordinator introduces its clause. Finnish does not use English-style
subject–auxiliary inversion in subordinate clauses.

| example | reading |
|---|---|
| `Tiedän, että hän tulee.` | neutral subordinate |
| `Luulen, että asia on tärkeä.` | neutral subordinate |
| `Menen kotiin, koska olen väsynyt.` | neutral subordinate |
| `Tiedän, että tulee hän.` | marked, but not an agreement error |

Decision: `PREFER`, priority 85, `hard_constraint: false`.
Exceptions: stylistic inversion, poetry, contrastive focus.

### FI_ALLOW_001 — information-structure fronting

The following are grammatical and must not be reported as order errors:

| construction | example |
|---|---|
| object fronting | `Kirjan luin eilen.` |
| topic fronting | `Minä en sitä tiedä.` |
| temporal fronting | `Eilen kävin kaupassa.` |
| locative fronting | `Kotona hän viihtyy hyvin.` |
| contrastive focus | `JUURI HÄN teki sen.` |
| passive | `Asia käsiteltiin eilen.` |
| existential | `Pöydällä on kahvikuppi.` |

Decision: `ALLOW`, priority 0.

## 4. Priority order

When rules disagree, apply:

1. syntactic construction constraints — interrogatives, imperatives,
   subordinate clauses, passive, existential;
2. information structure — topic, focus, contrast, given/new;
3. discourse context — continuity, reference, emphasis;
4. neutral default order — SVO for ordinary declaratives;
5. stylistic optimisation — naturalness, rhythm, register.

A lower-numbered layer wins. `ALLOW` from an information-structure layer
suppresses a `PREFER` from the neutral SVO layer.

## 5. Machine-readable shape

The fixture holds a rule list with this shape:

```json
{
  "id": "FI_QUESTION_001",
  "name": "wh_question_fronting",
  "category": "interrogative",
  "condition": { "clause_type": "interrogative", "question_type": "wh" },
  "constraint": { "target": "interrogative_phrase", "position": "initial" },
  "decision": "PREFER",
  "priority": 100,
  "hard_constraint": false,
  "exceptions": ["echo_question", "contrastive_focus", "spoken_language"],
  "examples": {
    "preferred": ["Missä hän asuu?"],
    "allowed": ["Hän asuu missä?"]
  }
}
```

`decision` is one of `FORCE`, `PREFER`, `ALLOW`. `hard_constraint` is true only
for a true `FORCE`. Every rule names its exceptions and both a preferred and an
allowed example; the fixture test checks the schema and stable ids.

## 6. Diagnostic contract when this is implemented

- `DiagnosticKind::WordOrder` stays **suggestion-only**. It must never emit
  `Error` or `Warning`: even high-priority rules are construction-sensitive
  preferences.
- No correction is attached to a word-order suggestion initially. Rewriting
  order changes information structure, and the engine cannot name a single
  safe replacement.
- `ALLOW` cases produce no diagnostic. In particular, object/topic/temporal/
  locative fronting, contrastive focus, passive and existential orders are
  clean.
- The clause features in §2 are added to the analysis and pinned in
  `tests/fixtures/syntax_ambiguity.json` and the prose corpus. **Done** — see
  §2.1.
- The `ALLOW` regression net is in place before any rule:
  `tests/fixtures/word_order_cases.json` carries every example in §3, and
  `tests/word_order_allow.rs` asserts the exact diagnostic list (empty), the
  clause features, and that `WordOrder` is never emitted. **Done.**
- Only after that net is green may a `PREFER` suggestion be added, and only for
  an unambiguous cue.

### 6.1 Why no `WordOrder` rule is emitted yet

Building the net (§6) settled the question for now. Every sentence the
specification marks as a non-preferred order is also listed by the same rule as
an allowed alternative:

| rule | non-preferred example | the rule's own verdict |
|---|---|---|
| `FI_QUESTION_001` | `Hän asuu missä?` | echo question, `ALLOW` |
| `FI_QUESTION_002` | `Hän tuleeko?` | contrastive, `ALLOW` |
| `FI_IMPERATIVE_001` | `Tänne tule!` | contrastive, `ALLOW` |
| `FI_ORDER_001` | `Auton ostin.`, `Eilen kävin kaupassa.` | fronting, `ALLOW` |
| `FI_EXISTENTIAL_001` | `Kirja on pöydällä.` | `ALLOW` |
| `FI_SUBORDINATE_001` | `Tiedän, että tulee hän.` | `ALLOW` |

So a rule that fires on any of them would report a sentence this project pins
clean, and every one of those orders carries a different information structure
the engine cannot name. `Söi Sami puuron.` is in the same position: it is in the
clean corpus, and there is no `PREFER` rewrite the engine could defend.

A `WordOrder` diagnostic is therefore not written until there is a construction
with a **structurally impossible** order — a genuine `FORCE` — rather than a
merely less neutral one. The features and the net are the groundwork that makes
that decision cheap to revisit.

## 7. Non-goals

- Do not enforce SVO on neutral declaratives.
- Do not infer object case from position.
- Do not rewrite an existential or passive clause into SVO.
- Do not treat an echo question, contrastive focus or spoken-language order as
  an error.
- Do not add a universal word-order correction.

## 8. Implementation map

| piece | location |
|---|---|
| clause features (`clause_type`, `construction`, `question_type`, ...) | `src/syntax/features.rs`, on `Clause::features` |
| machine-readable rules | `tests/fixtures/word_order_rules.json`, pinned by `tests/word_order_spec.rs` |
| the `ALLOW` regression net | `tests/fixtures/word_order_cases.json`, `tests/word_order_allow.rs` |
| ambiguity regressions | `tests/fixtures/syntax_ambiguity.json` (`features` key) |
| clause features in the browser surface | `torille-wasm/src/lib.rs` `analyze_syntax` |
| rule engine and decision model | `src/syntax_check/word_order.rs` (not implemented) |
| explanation relation | `Relation::WordOrder` (not implemented; `DiagnosticKind::WordOrder` already exists) |

The spec is deliberately separate from `PLAN_SYNTAX_CHECKER.md`'s old “no
word-order rules” decision; that decision is superseded here.
