# Grammar and syntax — complete feature reference

This document is the feature-level reference for the morphology engine, the
syntax analysis layer and the grammar checker. It lists the grammatical
categories the engine understands, the syntax structures it builds, and every
diagnostic kind the checker can emit.

It is deliberately **not** a list of the 104k sanalista lemmas or every Kotus
class. Those are data; the class table is in `data/catalogue.csv` and the rule
model is in [`morfologia.yaml`](morfologia.yaml). The detailed checker contract
is [`syntax_checker.md`](syntax_checker.md); the word-order specification is
[`word_order.md`](word_order.md). The flat analysis model is
[`../misc_md/PLAN_SYNTAX.md`](../misc_md/PLAN_SYNTAX.md); the AST design is
[`../misc_md/PLAN_SYNTAX_AST.md`](../misc_md/PLAN_SYNTAX_AST.md).

## Contents

- The pipeline
- Part I — Morphology and inflection
- Part II — Syntax analysis
- Part III — Grammar checker
- Part IV — Contracts and guarantees
- Part V — File map
- Diagnostic mapping (Part III)
- Coverage matrix and API contract (Part IV)
- Terminology

## Terminology

| avoid | use |
|---|---|
| grammar engine (for inflection) | morphology engine |
| grammar (for both layers) | morphology and inflection vs grammar checker |
| confidence (unqualified) | form / role / diagnostic / correction confidence |
| parser | flat syntax analysis / role resolver |
| “100% accurate” | fixture-level exactness on the named corpus |
| “word order is correct” | `PREFER` / `ALLOW` information-structure decision |

## 0. The pipeline

```text
text
  │
  ├─ tokenise                 word/separator runs with byte spans
  ├─ sentence::spans          sentences, paragraphs, quote depth
  ├─ morphology::analyze      readings, lemmas, case/number, depth
  ├─ syntax::analyze_syntax   clauses, roles, features, lists
  ├─ syntax::ast::build       Document/Sentence/Clause/Phrase/List/…
  └─ syntax_check             diagnostics, explanations, corrections
```

The morphology engine is the only stage that searches the lexicon. Syntax is
advisory and on demand; the checker is opt-in and never on the keystroke path.
`torille::check_all` runs grammar and style over one morphology pass.

The **flat `SyntaxAnalysis` is authoritative**. The AST is a derived,
additive view: it groups the flat roles, clauses and lists and never re-resolves
a role or searches the lexicon. Diagnostics read the flat analysis; the AST
serves structure, UI and the word-order constituent view.

---

# Part I — Morphology and inflection

The morphology engine recognises a surface word and generates the forms of a
lemma. It is dictionary-driven: the Kotus sanalista supplies the lemmas and
class codes, the catalogue supplies the eight core slots, and the hand-written
class engines plus the heuristic grammar fill the rest.

## 1. Word classes

`PartOfSpeech` is the sanalista's own inventory, not a simplified school list.

| value | meaning | example |
|---|---|---|
| `Noun` | common or proper noun | `talo`, `Sami` |
| `Adjective` | adjective, including degrees | `punainen`, `punaisempi` |
| `Pronoun` | personal, demonstrative, relative, interrogative | `hän`, `tämä`, `joka` |
| `Numeral` | cardinal | `kaksi`, `kolme` |
| `Ordinal` | ordinal | `toinen`, `kolmas` |
| `Verb` | verb, finite or non-finite | `sanoa`, `sanoo`, `sanottu` |
| `NegativeVerb` | the negation auxiliary | `en`, `ei`, `älä` |
| `Adverb` | adverb | `nopeasti`, `eilen` |
| `Conjunction` | coordinating or subordinating | `ja`, `että` |
| `Interjection` | interjection | `hei`, `voi` |
| `Preposition` | preposition | `ilman`, `ennen` |
| `Postposition` | postposition | `alla`, `kanssa` |
| `Other` | an unknown code or a `+` compound of classes | — |

A lemma can carry more than one class (`adjektiivi, substantiivi`). The first
class is the prominent one; every reading is retained on the token.

## 2. Nominal categories

### 2.1 Case

`Tags.case` has 16 values. The table lists the productive core first; the
comitative, prolative and instructive forms are more restricted (typically
plural, participial or lexical).

| case | example | notes |
|---|---|---|
| nominative | `talo` | subject, nominative plural object |
| accusative | `talo` / `talon` | object tag; the surface is shared with nominative/genitive |
| genitive | `talon` | possession, total singular object, modifiers |
| partitive | `taloa` | partial/negative object, quantity |
| inessive | `talossa` | “in” |
| elative | `talosta` | “out of” |
| illative | `taloon` | “into” |
| adessive | `talolla` | “at/on”, possessor with `on` |
| ablative | `talolta` | “off/from” |
| allative | `talolle` | “onto/to” |
| essive | `talona` | state “as” |
| translative | `taloksi` | change “into” |
| abessive | `talotta` | “without” |
| comitative | `taloineen` | “with”, typically plural with a possessive |
| prolative | `postitse` | rare/lexical “by way of” |
| instructive | `taloin` | plural/participial “by means of” |

Case is a property of a reading, not a guess from position. `Tags.case` is
absent when the readings disagree; `WordInfo.case` only exposes the agreed
case. The checker never rewrites a case from word order.

#### Case evidence and syncretism

Three levels must never silently become interchangeable:

| level | field | meaning |
|---|---|---|
| morphological reading | `Tags.case` on each reading | a case the form can express |
| agreed morphology | `TokenSyntax.case` | the case all selected readings agree on, or `None` |
| syntactic function | `Clause.object` / `Clause.features.object_case` | the function the clause resolved |

Morphology proposes; syntax selects. A nominative/accusative syncretic surface
can be an object, but the token’s `case` stays what the readings say.
`ObjectCase::Accusative` is produced only from a real `Case::Accusative`
reading, not from word order.

| example | morphological evidence | syntactic result |
|---|---|---|
| `näen pojan` | genitive/accusative total object | object; `object_case` genitive or accusative |
| `näen poikaa` | partitive | object; `object_case` partitive |
| `poika tuli` | nominative | subject |
| `pojan tulo` | genitive | modifier |
| `Pöydällä on kirja` | nominative | existential subject |
| `Pöydällä on kirjaa` | partitive | existential subject |

The contract tests in `tests/syntax_contract.rs` pin these distinctions.


### 2.2 Number

`Singular`, `Plural`, `Collective`. The collective is the collective reading of
a singular form, not a third agreement number for subjects.

### 2.3 Possessive suffixes

Five suffixes: first singular `-ni`, second singular `-si`, third `-nsa/-nsä`,
first plural `-mme`, second plural `-nne`. The third person covers singular and
plural possessors. A possessive is a form dimension, so `taloni` is
legitimately case-less when the readings disagree.

### 2.4 Clitics

Five post-inflectional particles:

| clitic | surface | function |
|---|---|---|
| `Ko` | `-ko/-kö` | question/focus |
| `Kin` | `-kin` | “also, too, even” |
| `Kaan` | `-kaan/-kään` | negative additive “either” |
| `Han` | `-han/-hän` | reminder/softener |
| `Pa` | `-pa/-pä` | emphasis/prompt |

A clitic does not trigger gradation. It sits on top of the host form and is
exposed separately so a rule can find the focus host.

### 2.5 Degree and polarity

`Degree` is `Comparative` or `Superlative` for adjectives/adverbs; the positive
is absence, not a third value. `Polarity` is `Affirmative` or `Negative`; the
negative appears in the analytic construction (`en mene`) and in negative
forms. `Tags` also carries `Voice`, `Mood`, `Tense`, `Person` where relevant.

## 3. Verbal categories

| dimension | values |
|---|---|
| person | first, second, third |
| number | singular, plural |
| tense | present, past, perfect, pluperfect |
| mood | indicative, conditional, potential, imperative, instructive, infinitive |
| voice | active, passive, middle |
| polarity | affirmative, negative |

The finite paradigm includes present, past, conditional, potential and
imperative. The non-finite system includes the infinitive and the present/past
participles; a participle can carry case and number as well as tense/voice.
The negative construction is analytic: the auxiliary carries person/number,
the main verb is the connegative.

## 4. Inflection model

### 4.1 Data and classes

- `data/nykysuomensanalista.csv`: 104k lemmas with homonymy, POS and inflection
  info (`9`, `9*B`, `9, 50`, `(21)`).
- `data/catalogue.csv`: eight slots per class — singular nominative, genitive,
  partitive, illative, then the same four plural. The oracle seeds the slots
  once; `build_catalogue` rewrites them from the engine's own output until they
  are self-consistent.
- Kotus classes **1–101**, including lettered subclasses; verbs use
  **52–78**. Class 50/51 are compound markers; class 99 is indeclinable; class
  101 is the irregular/hand-written family. The per-class engines live in
  `src/expand/nominals/` and `src/expand/verbs/`; a class is authoritative when
  it is verified against the noun/verb lexicon. Otherwise its forms are
  additive to the heuristic engine.

### 4.2 Gradation

Consonant gradation is a strong/weak alternation. The model in
[`morfologia.yaml`](morfologia.yaml) lists:

```text
kk:k  pp:p  tt:t  k:∅  p:v  t:d  nk:ng  mp:mm  nt:nn
lt:ll  rt:rr  kk:∅  kV:v
```

A lexeme carries the strong grade in the infinitive/nominative and the weak
grade in the parts the class fixes as weak. The engine reads the grade site from
the class and the stem; an alternation read off the genitive is marked
`Repaired`, not `Plain`.

### 4.3 Vowel harmony

Back vowels `a o u`, front vowels `ä ö y`, neutral `e i`. An ending takes the
back or front spelling from the stem's last non-neutral vowel; a stem with only
neutral vowels takes the back spelling. `-lle` has no harmonic vowel.

### 4.4 Stems, endings and confidence

A form is built from a class stem, an ending and the class's spelling rules.
The catalogue's eight slots are the anchor; the grammar derives the remaining
cases and the non-finite forms. A form carries:

| confidence | meaning | may be suggested? |
|---|---|---|
| `Plain` | plain attachment, no repair | **yes** |
| `Repaired` | built by repairing the lemma (gradation, epenthesis, an alternation read off the genitive) | no |
| `Guessed` | type-rule guess with no catalogue form behind it | no |

This split is why a valid but repaired form is accepted as a word and never
offered as a correction.

### 4.5 `Form`, `Tags` and ambiguity

A `Form` has a surface, one or more `Tags` readings and a `Confidence`.
`Form::union` is the intersection of what several readings agree on: a
dimension absent across readings means “they disagree”, not “unknown”.
`Tags` is the explainable unit a syntax rule consumes.

## 5. Word families and special constructions

| family | support |
|---|---|
| common nouns | full case/number paradigm, gradation, harmony, possessives/clitics |
| proper names | name table / proper-name forms; capitalisation retained; `erisnimi` metadata |
| adjectives | positive/comparative/superlative; attributive and predicative readings |
| numerals / ordinals | cardinal government, ordinal inflection, numeral phrases |
| pronouns | personal, demonstrative, relative, interrogative; closed paradigms |
| verbs | finite and non-finite; classes 52–78; negative construction; passive/middle |
| adverbs | lexical adverbs and `-sti` forms; degree where applicable |
| adpositions | pre/postpositions with governed-case frames (curated) |
| conjunctions / interjections | closed classes; no inflection |
| compounds | class 50/51 part-by-part inflection; modifier + head; bound tails |
| derivations | productive suffixes (`-ja`, `-minen`, `-nen`, …); lexicalised forms |
| feminine derivations | `-tar/-tär` productive; `-nna/-ska/-kko` lexical tables |
| indeclinable | class 99: the lemma is accepted, no generated forms |
| lexical adverbs | exact `-päin`/`-pikaa` spellings and productive genitive + `puoli` |

The engine does **not** treat all feminine suffixes as one rule; the detailed
policy is in `AGENTS.md` and `src/expand/derived_forms/agent.rs`.

## 6. Morphology API

| API | purpose |
|---|---|
| `morphology::analyze_morphology(tokens, cache)` | one `WordInfo` per word: readings, lemmas, case/number, POS, clitic, possessive, derivation, depth |
| `morphology::MorphologyCache` | per-text cache; bounded |
| `morphology::WordInfo` | resolved token; `Depth` records `Direct`, `StemMap`, `HeadsTails`, `Prefix`, `Clitic`, `Possessive`, `Compound`, `ProperName`, `Unknown` |
| `expand::inflect(lemma)` / `inflect_all` | one/all generated forms with `Tags` and `Confidence` |
| `expand::form_of` / `forms_of` | ask for a specific shape |
| `expand::head_inflections` | class a compound head declines as |
| `compound::analyze` / `explain` | split a compound and explain each part |
| `derivation` | productive reverse-derivation lookup |
| `sentence::analyze` / `spans` | sentence boundaries and `TokenContext` |
| `class_wizard` | infer a Kotus class from example forms |
| `check` / `suggest` | spell recognition and corrections (separate from grammar) |

`Tags` dimensions: `part_of_speech`, `case`, `number`, `person`, `tense`,
`mood`, `voice`, `possessive`, `degree`, `clitic`, `polarity`.

---

# Part II — Syntax analysis

The syntax layer is advisory: it resolves roles, builds clauses and lists, and
exposes features for the checker and UI. It is deliberately conservative; an
unknown role is preferable to a guessed one. See
[`../misc_md/PLAN_SYNTAX.md`](../misc_md/PLAN_SYNTAX.md) for the analysis plan.

## 1. Tokenisation

`tokenise::tokenise` splits text into alternating word and separator runs. A
word is a maximal run of Unicode letters/digits with single internal hyphens
(`nuori-isäntä`, `20-vuotias`). A pure number is a separator. Each `Token`
carries the literal text and a byte span; words also expose `lower_word` and
`is_capitalised`. The tokeniser is shared by the engine, the demo and the wasm.

## 2. Sentence layer

`sentence::spans` and `sentence::analyze` provide:

- `SentenceSpan { start, end, index, paragraph_start }`;
- `TokenContext { sentence_index, sentence_start, paragraph_start, quote_depth }`.

Boundaries include `.`, `?`, `!`, a colon, a line break, opening/closing
brackets and quote marks, with an abbreviation guard. The quote depth is a
context hint, not a dialogue parser. The syntax layer may merge bracket-only
sentence boundaries into one clause while the case layer keeps the raw spans.

## 3. Clause layer

`Clause` is the sentence's structural unit:

| field | meaning |
|---|---|
| `start`, `end`, `tokens` | byte span and word-token range |
| `predicate` | main finite verb; in a compound tense the auxiliary links to it |
| `subject`, `object`, `complement` | resolved argument tokens |
| `negative` | contains a negative auxiliary |
| `existential`, `passive` | construction flags |
| `features` | word-order features (below) |
| `parent`, `children`, `antecedent` | relative-clause tree and antecedent token |

Segmentation covers finite predicates, subordinators, relative pronouns, a
comma before a finite verb, coordination, infinitival clauses and bracket
merges. The linear clause list stays the source of truth; the relative tree is
additive. A multi-segment host (`Mies, joka tuli, söi`) is exposed by the AST,
not by a non-contiguous `Clause` range.

### 3.1 Segmentation precedence

The order in `src/syntax/segments.rs` is the specification:

| priority | cue | effect |
|---|---|---|
| 1 | top-level semicolon | split |
| 2 | `sillä` clause opener | split |
| 3 | relative pronoun / relative opener | split |
| 4 | comparison opener followed by a predicate | split |
| 5 | subordinator (`että`/`vaikka` guarded) | split |
| 6 | comma + capitalised word before a finite predicate | split at the capitalised word |
| 7 | comma + personal pronoun + finite predicate | split at the pronoun |
| 8 | coordinating conjunction before a finite predicate | split at the conjunction |
| 9 | infinitive after a finite predicate | split at the infinitive |
| 10 | comma + finite predicate after a finite predicate | split at the predicate |
| 11 | no reliable cue | **no split** |

`että` after `sekä` is not a clause opener; `vaikka` is only a subordinator when
a predicate follows; a bracket-only sentence boundary is merged for clauses but
not for the raw case spans. When cues compete, the later cue is not applied;
when none is reliable, the run stays one clause and the role resolver returns
`Unknown` rather than inventing a boundary.

| example | result |
|---|---|
| `Tiedän, että hän tulee.` | main + subordinate |
| `Mies, joka tuli, söi.` | host segment, relative, continuation |
| `Kun tulin kotiin, söin.` | subordinate + main |
| `Hän sanoi, ettei tule.` | main + negative subordinate |
| `Sami ja Liisa söivät.` | one clause (coordinated subject) |
| `Sami söi ja joi.` | two coordinated clauses |
| `(Tämä on uusi virke.) Toinen virke.` | two sentences |
| `omena (pieni, makea) ja banaani` | one clause; bracket boundary merged |

The clause fixtures in `tests/fixtures/syntax_cases.json` and
`syntax_ambiguity.json` are the oracle for these decisions.

## 4. Roles

`SyntacticRole` is what a token is doing in its clause:

| role | meaning |
|---|---|
| `Predicate` | finite main verb or folded predicate |
| `Auxiliary` | negation/tense auxiliary governed by the predicate |
| `Subject` | clause subject |
| `Object` | clause object |
| `Adverbial` | free adverbial/locative |
| `Complement` | predicative or governed complement |
| `Modifier` | attribute, genitive modifier, title/apposition |
| `Unknown` | no defensible role |

`RoleConfidence` is `Likely` or `Weak`. `TokenSyntax` also stores `head` (the
governor token), `case`, `lemma`, every `lemma` alternative and every reading.
A role is assigned only when morphology and a positional cue agree; a token
that is both a noun and a verb form stays `Unknown`/`Weak` until the clause
pass resolves it.

## 5. Clause features for word order

`SyntacticRole` is a syntactic function, not a construction. Construction-
specific behaviour is carried by `ClauseFeatures`, so the role inventory does
not have to grow variants such as “existential subject” or “necessive subject”.

`ClauseFeatures` is computed after role resolution:

| field | values |
|---|---|
| `clause_type` | declarative, interrogative, imperative, subordinate |
| `construction` | ordinary, existential, passive, copular, possessive, necessive |
| `subject_kind` | nominative, partitive, genitive_necessive, existential, omitted, other |
| `question_type` | wh, yes/no, alternative, echo |
| `subject_position` | preverbal, postverbal, omitted |
| `object_case` | partitive, accusative, genitive, nominative |
| `focus_type` | neutral, contrastive |
| `topic_position` | initial, non-initial |
| `register` | neutral, colloquial |

`subject_kind` is a construction/function feature: `Subject` remains the role,
while the shape (nominative, partitive, genitive-necessive, existential,
omitted, other) and the agreement behaviour are read here. A genitive subject
is the necessive shape; a passive clause has no syntactic subject; an
existential has the dedicated kind.

| construction | subject/possessor | agreement behaviour |
|---|---|---|
| ordinary | nominative subject | subject–predicate person/number |
| existential | postverbal nominative/partitive subject | singular predicate; no agreement error |
| passive | no syntactic subject | no subject agreement |
| copular | nominative subject | complement number agreement |
| possessive | adessive possessor + `on` | the possessed nominal is the subject |
| necessive | genitive subject + `täytyy`/`pitää`/`on` | 3sg predicate; no nominative subject agreement |

`Number::Collective` is not a plural: the subject-agreement bearer currently
returns no subject for a collective reading, so it never drives a
subject–predicate comparison. Middle voice is a morphological reading; no
syntax rule currently branches on it.

Deliberately **not** produced: narrow/broad focus (needs prosody), formal
register (no model), and a `FORCE` constraint level (no current hard rule).
`object_case` always comes from morphology, never from position.
`clause.features` is the input to [`word_order.md`](word_order.md), whose
`PREFER`/`ALLOW` decisions are pinned by `tests/word_order_allow.rs`.

## 6. Lists

`syntax_list` is the only list parser. A list needs a top-level comma or a colon
introducer and is classified as:

| `ListKind` | example |
|---|---|
| `Nominal` | `omena, appelsiini ja banaani` |
| `Clause` | `Sami söi, Liisa joi` |
| `Predicate` | `Sami söi, joi ja nukkui` |
| `Mixed` | analysis only; no automatic punctuation |

`ListItemKind` is `Word`, `Phrase`, `Clause` or `List(indices)`. The arena is
post-order: a nested list has a smaller index than its parent, `List.parent`
links upward, and `ListItem::scan` keeps trailing separators for bracket
nesting. Diagnostics use `List.tokens`/`items`; the AST mirrors the arena.

## 7. Surface tree (AST)

`syntax::ast` is an additive, derived tree over the flat analysis. It never
re-searches the lexicon or re-resolves a role.

```rust
let tree = torille::syntax::ast::analyze(&tokens);
let clauses = tree.clause_constituents(0);
```

### 7.1 Node kinds

`NodeKind`:

| kind | payload | meaning |
|---|---|---|
| `Document` | — | whole text |
| `Sentence` | — | logical sentence; bracket spans merged when one clause touches them |
| `Clause` | `ClauseKind` | flat clause or multi-segment relative host |
| `Phrase` | `PhraseKind` | head + intra-phrase dependents |
| `List` | `{ index }` | mirrored `syntax_list` entry |
| `ListItem` | `{ list, item }` | mirrored item |
| `Coordination` | `CoordKind` | reserved; no current pass |
| `Parenthetical` | — | balanced `(...)`, `[...]`, `{...}` |
| `Quote` | — | quote-depth run |
| `Word` | — | one word token |

`ClauseKind`: `Main`, `Relative`, `Subordinate`, `Infinitive`, `Coordination`.
`PhraseKind`: `Noun`, `Adjective`, `Pronoun`, `Numeral`, `Adverb`,
`Postposition`, `Preposition`, `Verb`, `Infinitive`, `Participle`.
`CoordKind`: `Nominal`, `Verbal`, `Clausal`, `Mixed`.

`NodeKind::Coordination` is a phrase-level coordination node and
`ClauseKind::Coordination` is a clause-level structural kind. Both are
**reserved and unemitted**; no current pass creates either. When a consumer
needs coordination, the phrase-level node owns same-kind conjuncts and the
clause-level kind is used only if a clause wrapper is required.

A `Node` carries `span`, `tokens`, `segments`, `parent`, `children`, `head`,
`governor`, `role`, `confidence`, `antecedent` and `clause`. `head` is the
lexical head; `governor` is the flat `TokenSyntax.head`. Only `Modifier` and
`Auxiliary` edges become phrase-internal children; an object is not a child of
the verb phrase merely because the verb governs it.

### 7.2 Built phases

| phase | content |
|---|---|
| 0 | Document, logical Sentence, Word leaves, one Clause per flat clause |
| 1 | head-driven phrases, `head`/`governor`, adposition complements, numeral phrases, predicate-less shape phrases |
| 2 | multi-segment relative host, nested relative node, antecedent |
| 3 | top-level nominal lists + items + conjunction; parentheticals; quotes; sentence-span union |
| 4 | word-order constituent view (`clause_constituents`, `first_constituent`, `fronted_constituent`) |
| 5 | wasm `analyze_ast` (`{ syntax, ast }`, nodes, clauses, hosts) |

Deliberately deferred: nested/mixed/clause/predicate lists, `Coordination`
nodes, `Clause.sentence`, flat `ClauseKind` unification. The detailed plan is
[`../misc_md/PLAN_SYNTAX_AST_REST.md`](../misc_md/PLAN_SYNTAX_AST_REST.md).

### 7.3 Constituents

`Constituent { node, kind, role, tokens, start, end, interrogative }` is a
direct child of a clause or host. `interrogative` is true for a wh word in a
wh/echo question and for a `-ko`/`-kö` host in a yes/no question.
`fronted_constituent` returns the first object/adverbial/complement before the
predicate; a subject is neutral, and an unknown/modifier word is not asserted
to be moved.

`host_constituents` returns the order of a multi-segment host, including the
relative clause as a constituent. `syntax_tree --order` prints `q:` and
`host:` lines; wasm `analyze_ast` exposes `interrogative` and `hosts`.

### 7.4 Validation

`syntax::ast::validate` checks root, parent/child reciprocity, word coverage,
segment shape, partition/coverage, clause agreement, list agreement, role
agreement, phrase heads and acyclicity. `build` runs it under
`debug_assert!`; `syntax_tree --check` runs it explicitly.

---

# Part III — Grammar checker

`torille::syntax_check` is the opt-in grammar and punctuation checker. It is
not a parser, a style checker, or part of the keystroke path.

## 1. Contract

`check(text)` tokenises, runs `syntax::analyze`, applies the rules and returns
`SyntaxCheck { analysis, diagnostics }`. `check_with_cache` uses a caller cache;
`check_analysis` runs over an existing analysis. Each diagnostic carries:

- a stable `DiagnosticKind` name and stable `RuleId` (message key);
- `Severity`: `Error`, `Warning`, `Suggestion`;
- `Confidence`: `Certain`, `Likely`, `Ambiguous`;
- a byte span, `expected` value and `Explanation` with a `Relation`;
- zero or more `Correction`s, each with `Edit`s, a score, a note,
  `CorrectionLicense` (`Inflection`, `Curated`, `Punctuation`, `Other`) and its
  own `Confidence`.

`Ambiguous` findings are suppressed; an `Error` whose confidence is only
`Likely` is downgraded to `Warning`. Corrections are applied from the end of the
text backwards. `without_rules`/`ignore_rules` filter by stable rule id.

## 2. Every diagnostic kind

| kind | rule | severity | correction | example |
|---|---|---|---|---|
| `subject_predicate_agreement` | subject and finite predicate agree in person/number; active past participle agrees in number | `Error`, or `Warning` for a weak role | inflection | `Koira nukkuvat.` → `nukkuu` |
| `connegative` | main verb after a negative auxiliary is connegative | `Error`, `Warning` when weak | inflection; 2sg imperative only | `En menen.` → `mene` |
| `attribute_agreement` | attributive adjective/demonstrative agrees with its head | `Error` for number, `Warning` for case | inflection | `Punainen autot.` → `punaiset` |
| `predicative_agreement` | predicative adjective/participle agrees in number; clear noun-predicate mismatch | `Error`/`Warning` | inflection/curated | `Talot ovat punainen.` → `punaiset` |
| `possessive_agreement` | possessive suffix matches an adjacent genitive pronoun | `Warning`/`Suggestion` | none | `Minun kirjasi on uusi.` |
| `verb_rection` | curated valency frame requires a case the argument lacks | `Error`/`Warning` | inflection/curated | `Pidän sinä.` → `sinusta` |
| `case_selection` | numeral government, curated adposition frame, existential subject case | `Error`/`Warning` | inflection/curated | `Kaksi lapsi tuli.` → `lasta` |
| `object_case` | negative clause needs partitive; bare nominative object; aspect cue | `Error`/`Suggestion` | inflection | `En lue kirja.` → `kirjaa` |
| `missing_argument` | obligatory slot from `verb_arguments.csv` is absent | `Warning` | none | `Hän luottaa.` |
| `extra_argument` | closed intransitive frame has an object | `Warning` | none | `Nukun kirja.` |
| `clause_structure` | declarative clause lacks a predicate or ends in an uncoordinated conjunction | `Warning` | none | `Koska minä väsynyt.` |
| `word_order` | **reserved**; no rule emits it | — | — | spec only |
| `comma_missing` | subordinate clause boundary; `esimerkiksi` parenthetical pair | `Error`/`Suggestion` | punctuation | `Menin kotiin koska olin väsynyt.` |
| `comma_extra` | comma between personal-pronoun subject and finite predicate | `Warning` | removal | `Minä, menen kotiin.` |
| `comma_pair_unbalanced` | opened relative-clause comma pair is not closed | `Error` | punctuation | `Koira, joka nukkuu sohvalla on minun.` |
| `question_mark_missing` | a clear direct question ends with `.` or no mark | `Error` | punctuation | `Missä sinä asut.` → `?` |
| `unbalanced_quote` | quotation still open at end of text | `Warning` | none | `Hän sanoi: »Elä tule.` |
| `list_separator_missing` | adjacent list items with no separator | `Error` | punctuation | `ostin omenoita appelsiineja` |
| `list_separator_duplicate` | item separated twice | `Error` | removal | `omenoita,, ja banaaneja` |
| `list_comma_before_conjunction` | comma before final `ja`/`tai`/`sekä` | `Suggestion` | removal | `omenoita, ja banaaneja` |
| `list_introducer_comma` | comma directly after a list-introducing colon | `Error` | removal | `kolme asiaa:, omenoita` |

`WordOrder` is reserved: the design is in [`word_order.md`](word_order.md), and
any future rule must be suggestion-only. `clause_type`, `construction`,
`question_type`, `subject_position`, `object_case`, `focus_type`,
`topic_position` and `register` are already computed for it.

### Stable rule ids and correction contract

The `RuleId` used by ignore lists is the diagnostic's message key. A kind may
have more than one rule id; the correction license is part of the contract.

| DiagnosticKind | RuleId(s) | Typical severity | Correction |
|---|---|---|---|
| `SubjectPredicateAgreement` | `subject_predicate_agreement`, `participle_agreement` | Error / Warning (weak role) | Inflection |
| `Connegative` | `connegative` | Error / Warning | Inflection; 2sg imperative only |
| `AttributeAgreement` | `attribute_agreement` | Error (number) / Warning (case) | Inflection |
| `PredicativeAgreement` | `predicative_agreement`, `predicative_noun_agreement` | Error / Warning | Inflection/curated |
| `PossessiveAgreement` | `possessive_agreement`, `possessive_missing` | Warning / Suggestion | none |
| `VerbRection` | `verb_rection` | Error / Warning | Inflection/curated |
| `CaseSelection` | `numeral_case`, `adposition_case`, `existential_subject_case` | Error / Warning | Inflection/curated |
| `ObjectCase` | `object_case`, `bare_nominative_object` | Error / Suggestion | Inflection |
| `MissingArgument` | `missing_argument` | Warning | none |
| `ExtraArgument` | `extra_argument` | Warning | none |
| `ClauseStructure` | `incomplete_coordination`, `missing_predicate` | Warning | none |
| `WordOrder` | **none emitted** | — | — |
| `CommaMissing` | `comma_missing`, `parenthetical_comma` | Error / Suggestion | Punctuation |
| `CommaExtra` | `comma_extra` | Warning | Punctuation |
| `CommaPairUnbalanced` | `comma_pair_unbalanced` | Error | Punctuation |
| `QuestionMarkMissing` | `question_mark_missing` | Error | Punctuation |
| `UnbalancedQuote` | `unbalanced_quote` | Warning | none |
| `ListSeparatorMissing` | `list_separator_missing` | Error | Punctuation |
| `ListSeparatorDuplicate` | `list_separator_duplicate` | Error | Punctuation |
| `ListCommaBeforeConjunction` | `list_comma_before_conjunction` | Suggestion | Punctuation |
| `ListIntroducerComma` | `list_introducer_comma` | Error | Punctuation |

A caller can dismiss one rule without hiding the rest of the checker. Some
kinds have more than one message key because they cover distinct sub-rules
(`case_selection`, `object_case`, `predicative_agreement`, `possessive_agreement`,
`clause_structure`). The contract test
`tests/syntax_contract.rs::every_fixture_diagnostic_has_a_contract_entry`
checks every emitted pair against this table.

## 3. Rule families

### 3.1 Agreement

- **Subject–predicate**: finite bearer (main verb, `olla` auxiliary, negative
  auxiliary), person/number; existential, passive, necessive and numeral
  constructions skipped.
- **Attribute**: prenominal adjective/ordinal/demonstrative; number mismatch is
  an error, case mismatch a warning; genitive modifiers, cardinals, quantity
  heads and relative pronouns skipped.
- **Predicative**: number agreement for adjectives/participles; noun predicate
  only for a clear number mismatch; partitive semantics stay quiet.
- **Possessive**: adjacent genitive pronoun vs suffix; warning/suggestion only.
- **Connegative**: negative auxiliary + main verb; past participle and
  conditional/potential accepted; 2sg imperative gets `mene`, plural/third
  imperative connegatives stay uncorrected (`Repaired`).

The agreement checks are separate, not interchangeable:

| check | controller / bearer | construction | correction |
|---|---|---|---|
| subject–predicate | subject person/number; finite main verb or `olla` auxiliary | ordinary, copular | inflection |
| negative auxiliary | negative auxiliary carries person/number; main verb is connegative | negative | connegative selection; 2sg imperative only |
| compound tense | active past participle agrees in number with subject | ordinary/copular | inflection |
| predicate complement | adjective/participle number; noun predicate only clear number mismatch | copular | inflection/curated |
| attribute | adjective/ordinal/demonstrative agrees with its nominal head | any | inflection |
| possessive | suffix matches adjacent genitive pronoun | any | none |
| passive | no subject agreement; passive participle is not a subject target | passive | none |
| existential | singular predicate; postverbal nominative/partitive subject | existential | none |
| necessive | genitive subject; 3sg predicate | necessive | none |
| experiencer | case-marked experiencer; no subject–predicate reuse | experiencer (gated) | rule-specific |

The controller, the agreement bearer and the connegative selection are three
different questions. A rule that changes one must not silently change another.

### 3.2 Rection, case and objects

- **Verb rection** reads `data/verb_rection.csv`; a curated frame is a lexical
  fact, never inferred from position.
- **Case selection** reads `data/adposition_cases.csv` and numeral rules;
  existential subject case is detected but may not choose the repair.
- **Object case** covers negative partitive, bare nominative object, and
  aspect cues (progressive/completive); aspect suggestions only.
- **Arguments** read `data/verb_arguments.csv`: obligatory slots and closed
  intransitive frames; no automatic correction.

### 3.3 Punctuation, lists, questions, quotes

- Subordinate/relative boundaries use commas, dashes and terminal marks.
- Missing relative closing comma is detected when the parser swallowed the
  host predicate; restrictive relatives without an opening comma stay quiet.
- Parenthetical commas are suggestions from a closed cue list.
- Questions: `-ko`/`-kö` and interrogative first words; reported/echo questions
  are guarded.
- Lists: missing/duplicate separators, comma before conjunction, colon
  introducer.
- Quotes: unbalanced quote warning; quote style/dialogue dashes are not
  corrected.

### 3.4 Structure

- Missing predicate: only a sentence-final subordinate clause with a personal
  pronoun subject and a predicative word but no finite verb.
- Incomplete coordination: clause with a predicate ends in a coordinator at a
  declarative period.
- Neither carries a correction.

## 4. Correction policy

- `Error` requires a clear cue, a likely role and a single relevant reading.
- `Plain` inflection forms are the only inflection corrections; `Repaired` and
  `Guessed` are accepted words but never suggestions.
- Unknown words, unknown roles, ambiguous agreeing targets and numeral phrases
  suppress the rule.
- Parenthetical commas are suggestions; a final conjunction without a comma is
  never an error; a semicolon is never an error.
- An unbalanced quote is a warning; no quote-style correction.
- A `WordOrder` diagnostic, if it ever lands, is suggestion-only.

### 4.1 Confidence layers

Do not use an unqualified “confidence”:

| layer | values | owner |
|---|---|---|
| form | `Plain`, `Repaired`, `Guessed` | morphology generation |
| role | `Likely`, `Weak` | syntax |
| diagnostic | `Certain`, `Likely`, `Ambiguous` | checker |
| correction | `Certain`, `Likely`, `Ambiguous` | correction |

The propagation policy is:

| input evidence | diagnostic ceiling | correction |
|---|---|---|
| single relevant reading + strong role | `Certain` | `Plain` form only |
| single relevant reading + weak role | `Likely` | `Plain`, likely confidence |
| competing readings | `Ambiguous` → suppressed | none |
| repaired form | warning-level if the rule uses it | **never** an inflection suggestion |
| guessed form | suppressed for correction | never |
| curated table row | as the table states | `Curated` |
| mechanical punctuation | `Certain` for a clear boundary | `Punctuation` |

`Ambiguous` is suppressed before a caller sees it; an `Error` whose confidence
is only `Likely` is downgraded to `Warning` in `rank::resolve`. The `Plain`
restriction applies to **inflection** corrections; curated corrections come
from a reviewed table, and punctuation edits are mechanical. They are separate
licenses, not exceptions to the inflection rule.

A rule may have a stronger lexical cue than the role confidence. The attribute
number rule is the documented exception: an unambiguous attribute shape and an
unambiguous head number can be `Certain`/`Error` even when the modifier role is
`Weak`. This is a rule-specific certainty, not a licence to ignore role
confidence everywhere; every new rule must state which evidence supplies its
`Certain`.

The current syntax layer does not carry `Form.confidence` per reading in
`WordInfo`; diagnostics consume readings and correction generation filters for
`Plain`. The chosen policy is the explicit split: **a repaired reading may
support a diagnostic, but it may never support an inflection correction.** If
the audit finds a repaired reading causing a false `Error`, add a
`form_confidence` field to `WordInfo` and cap the diagnostic; do not claim the
current model already does that.

## 5. Data files

| file | content |
|---|---|
| `data/verb_rection.csv` | `lemma,case,frame,priority,note` valency frames |
| `data/verb_arguments.csv` | `lemma,slot,case,requirement,note` argument completeness |
| `data/adposition_cases.csv` | `adposition,case,example` governed-case rows |
| `maps/verb_rection.bin.gz` | `(verb lemma → governed cases)` |
| `maps/verb_arguments.bin.gz` | `(verb lemma → argument frames)` |
| `maps/adposition_cases.bin.gz` | `(adposition → governed case)` |

## 6. Evaluation baseline

As of 2026-11-19:

```text
syntax_eval clean rows 154: 0 errors, 0 warnings, 0 suggestions
syntax_eval cases 60: every emitted kind precision 1.00, recall 1.00
syntax_diagnostics: 75 exact-output cases, 272 clean rows
prose: 79 clean rows / 1,233 words, 1 true positive, 0 known false positives
page-2000: 5 diagnostics
```

Run the gates with:

```bash
cargo test --release -p torille \
  --test syntax_check --test syntax_eval --test syntax_ambiguity --test syntax_prose
```

The exact-output fixture is `tests/fixtures/syntax_diagnostics.json`; the
adjudicated prose is `tests/fixtures/prose/adjudicated.json`.

### 6.1 What the numbers mean

These are **fixture-level** results on a curated corpus, not a claim of general
Finnish accuracy.

- `syntax_eval` clean rows are sentences with no expected diagnostic;
  `syntax_eval` cases are reviewed expected-kind rows.
- `syntax_diagnostics` is the exact-output fixture: the same kind, severity,
  span and first correction.
- `prose/adjudicated.json` separates reviewed clean rows, true positives and
  known false positives. A known false positive is pinned so a fix must move
  the row.
- A larger independently adjudicated corpus is future work; a 1.00 on this
  corpus means the curated rows match, not that the checker is perfect.

### 6.2 Performance

Measured on the committed fixture (`page-2000`, 1,959 words) and the demo/wasm
shape:

| workload | measurement |
|---|---|
| warm grammar rules | ~1.4 ms |
| warm style | ~2.5–3 ms |
| warm grammar + style | ~7–9 ms |
| one edited sentence, native | 2.6–3.7 ms |
| cold page, release wasm single-thread | ~3.0 s |
| cold page, 4-worker pool | ~1.4 s |
| cold-page target | ≤500 ms, still open |

The morphology cache is per textarea and bounded; the sentence cache is keyed
by sentence text; there is no AST cache. The wasm module carries the 104k-lemma
sanalista (roughly 3–4 MB compressed), so a document is checked on demand, not
in the keystroke loop.

## 7. Surfaces

- **CLI**: `check_syntax`, `check_style`, `dump_diagnostics`.
- **Leptos demo**: `SyntaxPanel` runs `check_syntax` on demand and applies
  corrections.
- **Firefox/Chromium**: context-menu **Tarkista kielioppi** (or grammar+style);
  opt-in, one input at a time.
- **wasm**: `analyze_syntax`, `check_syntax`, `check_style`, `analyze_ast`
  (tree + constituent order) with cache-aware variants.
- **Shared caches**: `MorphologyCache` for readings; `SentenceCache` for
  per-sentence diagnostics; no AST cache yet.

## 8. Deliberate limits

These are not bugs; they are conservative boundaries:

- no general word-order rule (`PREFER`/`ALLOW`, never an error);
- narrow/broad focus and formal register are not modelled;
- predicative agreement is number-only; noun predicates only for clear number
  mismatch;
- degree corrections for `Repaired` forms are diagnostic-only;
- inverted order, infinitive agreement, 1st/2nd-person subjects and `vaikka`
  boundaries have documented narrow cues;
- only a `Modifier` role is an attribute in a clause with a predicate;
  predicate-less fragments use the shape rule;
- compound-tense agreement covers the active past participle only;
- numeral/adposition government is a curated set, not a general case engine;
- rection targets a genuine argument, not a subject/attribute;
- existential subject case detects but does not choose the repair;
- bare nominative object is a narrow rule;
- extra commas are detected only in the pronoun-subject slot;
- possessive agreement is warning/suggestion only;
- ambiguous partitive plurals stay quiet;
- relative clauses are linked but the checker does not reparse the host as one
  non-contiguous span (the AST exposes host segments; the closing comma is
  checked from the linear pair);
- the extension grammar check is context-menu only;
- grammar and style are separate layers and never mix.

---

# Part IV — Contracts and guarantees

## 1. Diagnostic lifecycle

```text
text
  → tokenise
  → morphology
  → syntax analysis
  → rule evaluation
  → ambiguity filtering
  → severity assignment (Error→Warning when Likely)
  → correction generation
  → rank (severity, confidence, span, kind)
  → diagnostics
```

## 2. API guarantees

| guarantee | contract |
|---|---|
| spans | half-open UTF-8 byte ranges `start..end` |
| token indices | zero-based; refer to the full token stream, including separators |
| word leaves | `Word` nodes cover only word tokens; separators are not leaves |
| clause/list indices | zero-based; absent values are `Option` |
| clause token ranges | endpoints are words; interior separators are included |
| diagnostic order | severity, diagnostic confidence, `span.start`, `span.end`, kind; one diagnostic per span |
| correction order | score, then note; edits applied from the end backwards |
| cache reuse | same analysis and diagnostics; only cache/depth counters differ |
| stable | `DiagnosticKind` names, `RuleId` strings, correction licenses and span semantics |
| 0.x unstable | AST node kinds/fields, `Clause` fields and JSON shapes; additive changes are documented |

The contract tests are `tests/syntax_contract.rs`; the exact diagnostics live
in `tests/fixtures/syntax_diagnostics.json`. Run
`python3 tools/check_docs.py` to check the Markdown tables, links, heading
anchors and code fences in this document.

## 3. Coverage matrix

| feature | recognition | syntax integration | checking | correction |
|---|---|---|---|---|
| nominal inflection | yes | yes | case/rection/object | yes, confidence-dependent |
| verb inflection | yes | yes | agreement/connegative | yes, confidence-dependent |
| possessive suffixes | yes | partial | possessive agreement | no |
| clitics | yes | question detection | question marks | limited |
| compounds | yes | partial | limited | no |
| derivations | yes | partial | style/spell separate | no |
| numerals | yes | numeral phrases | numeral case | yes |
| proper names | yes | subject/object | limited | no |
| relative clauses | yes | host/AST | punctuation | limited |
| lists | yes | list arena/AST | punctuation | yes |
| parentheticals/quotes | yes | AST | punctuation | no |
| word order | features | constituent view | reserved | no |
| agreement | yes | roles/features | yes | yes |
| rection | curated | roles | yes | curated |
| argument structure | curated | roles | warnings | no |

“Partial” means the feature is recognised but the syntax integration or the
rule set is deliberately narrower than the morphology. The exact code path and
the fixture for each row belong in the feature-specific sections above.

---

# Part V — File map

| topic | files |
|---|---|
| tokens, sentence layer | `src/tokenise.rs`, `src/sentence.rs` |
| morphology cache / readings | `src/morphology/` |
| tags and form model | `src/tags.rs` |
| inflection grammar | `src/grammar/`, `src/expand/` |
| per-class engines | `src/expand/nominals/`, `src/expand/verbs/` |
| compounds / derivations | `src/compound.rs`, `src/derivation.rs` |
| syntax analysis | `src/syntax/` |
| AST | `src/syntax/ast/` |
| lists | `src/syntax_list.rs` |
| grammar checker | `src/syntax_check/` |
| checker diagnostics | `src/syntax_check/diagnostic.rs`, `src/diagnostic.rs` |
| checker corrections | `src/syntax_check/correction.rs` |
| checker data | `data/verb_rection.csv`, `data/verb_arguments.csv`, `data/adposition_cases.csv` |
| checker tests | `tests/fixtures/syntax_*.json`, `tests/fixtures/prose/adjudicated.json` |
| AST tests | `src/syntax/ast/`, `tests/syntax_tree.rs` |
| word order | [`word_order.md`](word_order.md), `src/syntax/features.rs` |
| AST design | [`../misc_md/PLAN_SYNTAX_AST.md`](../misc_md/PLAN_SYNTAX_AST.md), [`../misc_md/PLAN_SYNTAX_AST_REST.md`](../misc_md/PLAN_SYNTAX_AST_REST.md) |
| grammar/syntax fixes | [`../misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md`](../misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md) |
| style layer | [`style_checker.md`](style_checker.md) |
| hyphenation | [`hyphenation.md`](hyphenation.md) |
| verification | [`harness.md`](harness.md), [`../misc_md/TODO.md`](../misc_md/TODO.md) |

## Not grammar or syntax

- **Spell checking / suggestions** — `check`, `suggest`, `suggest_detailed`,
  `levenshtein_with_ops`; see `README.md`.
- **Style** — repetition, long sentences, clichés, filler, wordy phrases and
  colloquialisms; separate opt-in `style_check`, see `style_checker.md`.
- **Hyphenation** — line-break patterns and policy; see `hyphenation.md`.
- **Neural reranking** — optional `neural-m0` reading disambiguation; off by
  default.
- **Word wizard** — Kotus-class inference from example forms; separate
  `class_wizard`/wasm pages.

The review-driven hardening plan for this document is
[`../misc_md/PLAN_GRAMMAR_SYNTAX_REFERENCE.md`](../misc_md/PLAN_GRAMMAR_SYNTAX_REFERENCE.md).
It records the editorial restructuring, case-evidence model, construction
features, segmentation precedence, confidence propagation, API contract and
evaluation methodology still to be verified.

The render-safe layout plan is
[`../misc_md/PLAN_GRAMMAR_SYNTAX_LAYOUT.md`](../misc_md/PLAN_GRAMMAR_SYNTAX_LAYOUT.md).
It covers the collapsed tables and pipeline in exported/plain-text copies:
replace the ASCII pipeline with a numbered list, convert long table cells to
definition lists, split the diagnostic mega-table into a compact index plus
family subsections, and add `pandoc`/`check_docs.py` render checks.
