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
- `construction`: existential, passive, copular, possessive, other;
- `subject_position`: preverbal, postverbal, omitted;
- `object_case`: partitive, accusative, genitive, nominative (from morphology,
  never from position);
- `question_type`: wh, yes/no, alternative, echo;
- `focus_type`: neutral, contrastive, narrow, broad;
- `topic_position`: initial, non-initial;
- `register`: formal, neutral, colloquial;
- `constraint_level`: hard, preferred, optional.

`object_case` in particular must be resolved before the order rule runs:
`Kirjan luin` is object fronting, not a subject + verb.

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
- Before any rule lands, the clause features in §2 are added to the analysis
  and pinned in `tests/fixtures/syntax_ambiguity.json` and the prose corpus.
- The first implementation phase should start with the `ALLOW` table (a
  regression net that proves the checker stays quiet) and only then add a
  `PREFER` suggestion for an unambiguous cue.

## 7. Non-goals

- Do not enforce SVO on neutral declaratives.
- Do not infer object case from position.
- Do not rewrite an existential or passive clause into SVO.
- Do not treat an echo question, contrastive focus or spoken-language order as
  an error.
- Do not add a universal word-order correction.

## 8. Implementation map

| future piece | location |
|---|---|
| clause features (`clause_type`, `construction`, `question_type`, ...) | `src/syntax/` and `src/syntax_check/` |
| rule engine and decision model | `src/syntax_check/word_order.rs` (new) |
| machine-readable rules | `tests/fixtures/word_order_rules.json` |
| ambiguity and prose regressions | `tests/fixtures/syntax_ambiguity.json`, `tests/fixtures/prose/adjudicated.json` |
| explanation relation | `Relation::WordOrder` (new; `DiagnosticKind::WordOrder` already exists) |

The spec is deliberately separate from `PLAN_SYNTAX_CHECKER.md`'s old “no
word-order rules” decision; that decision is superseded here.
