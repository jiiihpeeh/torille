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

## Reading this document

The reference is written single-column and plain-text-first so the same source
survives a Markdown viewer, a GitHub/GitLab page, `pandoc -f gfm -t plain` and
a single-column PDF/HTML print. Long material is a list, not a wide table:
a rule keeps its id, severity, correction and example on adjacent lines.
Short enumerations may stay tabular; the diagnostic index is the one table
whose rows a reader scans by name.

The render-safe layout rules and their gates are recorded in
[`../misc_md/PLAN_GRAMMAR_SYNTAX_LAYOUT.md`](../misc_md/PLAN_GRAMMAR_SYNTAX_LAYOUT.md).
The review-driven content plan is
[`../misc_md/PLAN_GRAMMAR_SYNTAX_REFERENCE.md`](../misc_md/PLAN_GRAMMAR_SYNTAX_REFERENCE.md).

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

- Avoid **grammar engine** (for inflection); use **morphology engine**.
- Avoid **grammar** (for both layers); use **morphology and inflection** vs
  **grammar checker**.
- Avoid an unqualified **confidence**; use **form / role / diagnostic /
  correction confidence**.
- Avoid **parser**; use **flat syntax analysis / role resolver**.
- Avoid **“100% accurate”**; use **fixture-level exactness on the named
  corpus**.
- Avoid **“word order is correct”**; use the **`PREFER` / `ALLOW`
  information-structure decision**.

## 0. The pipeline

Text enters at the top and leaves as diagnostics:

1. `tokenise` — word/separator runs with byte spans.
2. `sentence::spans` — sentences, paragraphs, quote depth.
3. `morphology::analyze` — readings, lemmas, case/number, depth.
4. `syntax::analyze_syntax` — clauses, roles, features, lists.
5. `syntax::ast::build` — `Document`/`Sentence`/`Clause`/`Phrase`/`List`/…
6. `syntax_check` — diagnostics, explanations, corrections.

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

- **`Noun`** — common or proper noun. `talo`, `Sami`.
- **`Adjective`** — adjective, including degrees. `punainen`, `punaisempi`.
- **`Pronoun`** — personal, demonstrative, relative, interrogative. `hän`,
  `tämä`, `joka`.
- **`Numeral`** — cardinal. `kaksi`, `kolme`.
- **`Ordinal`** — ordinal. `toinen`, `kolmas`.
- **`Verb`** — verb, finite or non-finite. `sanoa`, `sanoo`, `sanottu`.
- **`NegativeVerb`** — the negation auxiliary. `en`, `ei`, `älä`.
- **`Adverb`** — adverb. `nopeasti`, `eilen`.
- **`Conjunction`** — coordinating or subordinating. `ja`, `että`.
- **`Interjection`** — interjection. `hei`, `voi`.
- **`Preposition`** — preposition. `ilman`, `ennen`.
- **`Postposition`** — postposition. `alla`, `kanssa`.
- **`Other`** — an unknown code or a `+` compound of classes; example: —.

A lemma can carry more than one class (`adjektiivi, substantiivi`). The first
class is the prominent one; every reading is retained on the token.

## 2. Nominal categories

### 2.1 Case

`Tags.case` has 16 values. The productive core comes first; the comitative,
prolative and instructive forms are more restricted (typically plural,
participial or lexical).

Core cases:

- **nominative** — `talo`; subject, nominative plural object.
- **accusative** — `talo` / `talon`; object tag, the surface is shared with
  nominative/genitive.
- **genitive** — `talon`; possession, total singular object, modifiers.
- **partitive** — `taloa`; partial/negative object, quantity.

Local cases:

- **inessive** — `talossa`; “in”.
- **elative** — `talosta`; “out of”.
- **illative** — `taloon`; “into”.
- **adessive** — `talolla`; “at/on”, possessor with `on`.
- **ablative** — `talolta`; “off/from”.
- **allative** — `talolle`; “onto/to”.

Marginal cases:

- **essive** — `talona`; state “as”.
- **translative** — `taloksi`; change “into”.
- **abessive** — `talotta`; “without”.
- **comitative** — `taloineen`; “with”, typically plural with a possessive.
- **prolative** — `postitse`; rare/lexical “by way of”.
- **instructive** — `taloin`; plural/participial “by means of”.

Case is a property of a reading, not a guess from position. `Tags.case` is
absent when the readings disagree; `WordInfo.case` only exposes the agreed
case. The checker never rewrites a case from word order.

#### Case evidence and syncretism

Three levels must never silently become interchangeable:

- **Morphological reading** — `Tags.case` on each reading; a case the form can
  express.
- **Agreed morphology** — `TokenSyntax.case`; the case all selected readings
  agree on, or `None`.
- **Syntactic function** — `Clause.object` /
  `Clause.features.object_case`; the function the clause resolved.

Morphology proposes; syntax selects. A nominative/accusative syncretic surface
can be an object, but the token’s `case` stays what the readings say.
`ObjectCase::Accusative` is produced only from a real `Case::Accusative`
reading, not from word order.

- `näen pojan` — genitive/accusative total object → object;
  `object_case` genitive or accusative.
- `näen poikaa` — partitive → object; `object_case` partitive.
- `poika tuli` — nominative → subject.
- `pojan tulo` — genitive → modifier.
- `Pöydällä on kirja` — nominative → existential subject.
- `Pöydällä on kirjaa` — partitive → existential subject.

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

- **`Ko`** (`-ko/-kö`) — question/focus.
- **`Kin`** (`-kin`) — “also, too, even”.
- **`Kaan`** (`-kaan/-kään`) — negative additive “either”.
- **`Han`** (`-han/-hän`) — reminder/softener.
- **`Pa`** (`-pa/-pä`) — emphasis/prompt.

A clitic does not trigger gradation. It sits on top of the host form and is
exposed separately so a rule can find the focus host.

### 2.5 Degree and polarity

`Degree` is `Comparative` or `Superlative` for adjectives/adverbs; the positive
is absence, not a third value. `Polarity` is `Affirmative` or `Negative`; the
negative appears in the analytic construction (`en mene`) and in negative
forms. `Tags` also carries `Voice`, `Mood`, `Tense`, `Person` where relevant.

## 3. Verbal categories

- **person** — first, second, third.
- **number** — singular, plural.
- **tense** — present, past, perfect, pluperfect.
- **mood** — indicative, conditional, potential, imperative, instructive,
  infinitive.
- **voice** — active, passive, middle.
- **polarity** — affirmative, negative.

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

- **`Plain`** — plain attachment, no repair. **May be suggested.**
- **`Repaired`** — built by repairing the lemma (gradation, epenthesis, an
  alternation read off the genitive). Not suggested.
- **`Guessed`** — type-rule guess with no catalogue form behind it. Not
  suggested.

This split is why a valid but repaired form is accepted as a word and never
offered as a correction.

### 4.5 `Form`, `Tags` and ambiguity

A `Form` has a surface, one or more `Tags` readings and a `Confidence`.
`Form::union` is the intersection of what several readings agree on: a
dimension absent across readings means “they disagree”, not “unknown”.
`Tags` is the explainable unit a syntax rule consumes.

## 5. Word families and special constructions

- **common nouns** — full case/number paradigm, gradation, harmony,
  possessives/clitics.
- **proper names** — name table / proper-name forms; capitalisation retained;
  `erisnimi` metadata.
- **adjectives** — positive/comparative/superlative; attributive and
  predicative readings.
- **numerals / ordinals** — cardinal government, ordinal inflection, numeral
  phrases.
- **pronouns** — personal, demonstrative, relative, interrogative; closed
  paradigms.
- **verbs** — finite and non-finite; classes 52–78; negative construction;
  passive/middle.
- **adverbs** — lexical adverbs and `-sti` forms; degree where applicable.
- **adpositions** — pre/postpositions with governed-case frames (curated).
- **conjunctions / interjections** — closed classes; no inflection.
- **compounds** — class 50/51 part-by-part inflection; modifier + head; bound
  tails.
- **derivations** — productive suffixes (`-ja`, `-minen`, `-nen`, …);
  lexicalised forms.
- **feminine derivations** — `-tar/-tär` productive; `-nna/-ska/-kko`
  lexical tables.
- **indeclinable** — class 99: the lemma is accepted, no generated forms.
- **lexical adverbs** — exact `-päin`/`-pikaa` spellings and productive
  genitive + `puoli`.

The engine does **not** treat all feminine suffixes as one rule; the detailed
policy is in `AGENTS.md` and `src/expand/derived_forms/agent.rs`.

## 6. Morphology API

- `morphology::analyze_morphology(tokens, cache)` — one `WordInfo` per word:
  readings, lemmas, case/number, POS, clitic, possessive, derivation, depth.
- `morphology::MorphologyCache` — per-text cache; bounded.
- `morphology::WordInfo` — resolved token; `Depth` records `Direct`,
  `StemMap`, `HeadsTails`, `Prefix`, `Clitic`, `Possessive`, `Compound`,
  `ProperName`, `Unknown`.
- `expand::inflect(lemma)` / `inflect_all` / `inflect_all_in` — one/all
  generated forms with `Tags` and `Confidence`. `inflect_all` supplies the
  embedded lexicon; `inflect_all_in` takes the caller's. A class-50/51
  compound paradigm needs it, so `None` means "cannot answer", never "use the
  catalogue model".
- `expand::form_of` / `form_of_in` / `forms_of` / `forms_of_in` — ask for a
  specific shape, with the same lexicon rule.
- `expand::head_inflections` — class a compound head declines as.
- `compound::analyze` / `explain` — split a compound and explain each part.
- `derivation` — productive reverse-derivation lookup.
- `sentence::analyze` / `spans` — sentence boundaries and `TokenContext`.
- `class_wizard` — infer a Kotus class from example forms.
- `check` / `suggest` — spell recognition and corrections (separate from
  grammar).

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

- `start`, `end`, `tokens` — byte span and word-token range.
- `predicate` — main finite verb; in a compound tense the auxiliary links to
  it.
- `subject`, `object`, `complement` — resolved argument tokens.
- `negative` — contains a negative auxiliary.
- `existential`, `passive` — construction flags.
- `features` — word-order features (below).
- `parent`, `children`, `antecedent` — relative-clause tree and antecedent
  token.

Segmentation covers finite predicates, subordinators, relative pronouns, a
comma before a finite verb, coordination, infinitival clauses and bracket
merges. The linear clause list stays the source of truth; the relative tree is
additive. A multi-segment host (`Mies, joka tuli, söi`) is exposed by the AST,
not by a non-contiguous `Clause` range.

### 3.1 Segmentation precedence

The order in `src/syntax/segments.rs` is the specification; the numbers are
the precedence order:

1. **Top-level semicolon** — split.
2. **`sillä` clause opener** — split.
3. **Relative pronoun / relative opener** — split.
4. **Comparison opener followed by a predicate** — split.
5. **Subordinator** (`että`/`vaikka` guarded) — split.
6. **Comma + capitalised word before a finite predicate** — split at the
   capitalised word.
7. **Comma + personal pronoun + finite predicate** — split at the pronoun.
8. **Coordinating conjunction before a finite predicate** — split at the
   conjunction.
9. **Infinitive after a finite predicate** — split at the infinitive.
10. **Comma + finite predicate after a finite predicate** — split at the
    predicate.
11. **No reliable cue** — **no split**.

`että` after `sekä` is not a clause opener; `vaikka` is only a subordinator when
a predicate follows; a bracket-only sentence boundary is merged for clauses but
not for the raw case spans. When cues compete, the later cue is not applied;
when none is reliable, the run stays one clause and the role resolver returns
`Unknown` rather than inventing a boundary.

- `Tiedän, että hän tulee.` — main + subordinate.
- `Mies, joka tuli, söi.` — host segment, relative, continuation.
- `Kun tulin kotiin, söin.` — subordinate + main.
- `Hän sanoi, ettei tule.` — main + negative subordinate.
- `Sami ja Liisa söivät.` — one clause (coordinated subject).
- `Sami söi ja joi.` — two coordinated clauses.
- `(Tämä on uusi virke.) Toinen virke.` — two sentences.
- `omena (pieni, makea) ja banaani` — one clause; bracket boundary merged.

The clause fixtures in `tests/fixtures/syntax_cases.json` and
`syntax_ambiguity.json` are the oracle for these decisions.

## 4. Roles

`SyntacticRole` is what a token is doing in its clause:

- **`Predicate`** — finite main verb or folded predicate.
- **`Auxiliary`** — negation/tense auxiliary governed by the predicate.
- **`Subject`** — clause subject.
- **`Object`** — clause object.
- **`Adverbial`** — free adverbial/locative.
- **`Complement`** — predicative or governed complement.
- **`Modifier`** — attribute, genitive modifier, title/apposition.
- **`Unknown`** — no defensible role.

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

- **`clause_type`** — declarative, interrogative, imperative, subordinate.
- **`construction`** — ordinary, existential, passive, copular, possessive,
  necessive.
- **`subject_kind`** — nominative, partitive, genitive_necessive,
  existential, omitted, other.
- **`question_type`** — wh, yes/no, alternative, echo.
- **`subject_position`** — preverbal, postverbal, omitted.
- **`object_case`** — partitive, accusative, genitive, nominative.
- **`focus_type`** — neutral, contrastive.
- **`topic_position`** — initial, non-initial.
- **`register`** — neutral, colloquial.

`subject_kind` is a construction/function feature: `Subject` remains the role,
while the shape (nominative, partitive, genitive-necessive, existential,
omitted, other) and the agreement behaviour are read here. A genitive subject
is the necessive shape; a passive clause has no syntactic subject; an
existential has the dedicated kind.

- **ordinary** — nominative subject; subject–predicate person/number.
- **existential** — postverbal nominative/partitive subject; singular
  predicate, no agreement error.
- **passive** — no syntactic subject; no subject agreement.
- **copular** — nominative subject; complement number agreement.
- **possessive** — adessive possessor + `on`; the possessed nominal is the
  subject.
- **necessive** — genitive subject + `täytyy`/`pitää`/`on`; 3sg predicate, no
  nominative subject agreement.

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

`syntax_list` is the only list parser. A list needs a top-level comma or
semicolon, or a colon introducer, and is classified by `ListKind`:

- **`ListKind::Nominal`** — `omena, appelsiini ja banaani`.
- **`ListKind::Clause`** — `Sami söi, Liisa joi`.
- **`ListKind::Predicate`** — `Sami söi, joi ja nukkui`.
- **`ListKind::Mixed`** — analysis only; no automatic punctuation.

The clause layer splits at every top-level semicolon, so the parser joins the
adjacent clauses of one sentence that a semicolon separates back into a single
list whose items are the semicolon groups:

- `Ostin omenoita; banaaneja; appelsiineja.` is a `Nominal` list of three items,
  because the first group is trimmed at its predicate.
- `Kokoukseen osallistuivat Virtanen, toimitusjohtaja; Laine,
  projektipäällikkö; …` keeps each group as one `Phrase` item, so an internal
  comma is not a list separator.
- `List.separators` holds every top-level separator token and
  `List.separator_kinds` says whether each is a comma or a semicolon. The
  comma-only diagnostics read the comma characters in the gap, so a semicolon
  boundary never triggers one.

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

- **`Document`** — whole text.
- **`Sentence`** — logical sentence; bracket spans merged when one clause
  touches them.
- **`Clause`** (`ClauseKind`) — flat clause or multi-segment relative host.
- **`Phrase`** (`PhraseKind`) — head + intra-phrase dependents.
- **`List`** (`{ index }`) — mirrored `syntax_list` entry.
- **`ListItem`** (`{ list, item }`) — mirrored item.
- **`Coordination`** (`CoordKind`) — reserved; no current pass.
- **`Parenthetical`** — balanced `(...)`, `[...]`, `{...}`.
- **`Quote`** — quote-depth run.
- **`Word`** — one word token.

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

- **Phase 0** — `Document`, logical `Sentence`, `Word` leaves, one `Clause`
  per flat clause.
- **Phase 1** — head-driven phrases, `head`/`governor`, adposition
  complements, numeral phrases, predicate-less shape phrases.
- **Phase 2** — multi-segment relative host, nested relative node, antecedent.
- **Phase 3** — top-level nominal lists + items + conjunction; parentheticals;
  quotes; sentence-span union.
- **Phase 4** — word-order constituent view (`clause_constituents`,
  `first_constituent`, `fronted_constituent`).
- **Phase 5** — wasm `analyze_ast` (`{ syntax, ast }`, nodes, clauses, hosts).

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

This is the lookup index. Each kind appears once; the full description,
correction policy and example are in the family section below. `RuleId` is the
message key used by ignore lists; a kind may have more than one.

| kind | rule id(s) | severity |
|---|---|---|
| `subject_predicate_agreement` | `subject_predicate_agreement`, `participle_agreement` | Error / Warning |
| `connegative` | `connegative` | Error / Warning |
| `attribute_agreement` | `attribute_agreement` | Error / Warning |
| `predicative_agreement` | `predicative_agreement`, `predicative_noun_agreement` | Error / Warning |
| `possessive_agreement` | `possessive_agreement`, `possessive_missing` | Warning / Suggestion |
| `verb_rection` | `verb_rection` | Error / Warning |
| `case_selection` | `numeral_case`, `adposition_case`, `existential_subject_case` | Error / Warning |
| `object_case` | `object_case`, `bare_nominative_object` | Error / Suggestion |
| `missing_argument` | `missing_argument` | Warning |
| `extra_argument` | `extra_argument` | Warning |
| `clause_structure` | `incomplete_coordination`, `missing_predicate` | Warning |
| `word_order` | none emitted | — |
| `comma_missing` | `comma_missing`, `parenthetical_comma` | Error / Suggestion |
| `comma_extra` | `comma_extra` | Warning |
| `comma_pair_unbalanced` | `comma_pair_unbalanced` | Error |
| `semicolon_misuse` | the five `semicolon_*` rule ids (see §3.4) | Suggestion |
| `semicolon_missing` | `semicolon_list_hierarchy` | Suggestion |
| `question_mark_missing` | `question_mark_missing` | Error |
| `unbalanced_quote` | `unbalanced_quote` | Warning |
| `list_separator_missing` | `list_separator_missing` | Error |
| `list_separator_duplicate` | `list_separator_duplicate` | Error |
| `list_comma_before_conjunction` | `list_comma_before_conjunction` | Suggestion |
| `list_introducer_comma` | `list_introducer_comma` | Error |

`DiagnosticKind::name()` returns the kind shown in the first column. The Rust
variant is given in parentheses in the family sections. The `Error`/`Warning`
split is refined there: an `Error` whose confidence is only `Likely` is
downgraded to `Warning`.

## 3. Rule families

### 3.1 Agreement

- **`subject_predicate_agreement`** (`SubjectPredicateAgreement`) — a subject
  and a finite predicate agree in person/number; the active past participle
  agrees in number. Severity: `Error`, or `Warning` for a weak role.
  Correction: inflection. `Koira nukkuvat.` → `nukkuu`. The rule id
  `participle_agreement` covers the participle case.
- **`connegative`** (`Connegative`) — the main verb after a negative auxiliary
  is connegative. Severity: `Error`, `Warning` when the reading is weak.
  Correction: inflection, 2sg imperative only. `En menen.` → `mene`.
- **`attribute_agreement`** (`AttributeAgreement`) — an attributive
  adjective/demonstrative agrees with its head. Severity: `Error` for number,
  `Warning` for case. Correction: inflection. `Punainen autot.` → `punaiset`.
- **`predicative_agreement`** (`PredicativeAgreement`) — a predicative
  adjective/participle agrees in number; a clear noun-predicate mismatch is
  reported. Severity: `Error`/`Warning`. Correction: inflection/curated.
  `Talot ovat punainen.` → `punaiset`. The rule id
  `predicative_noun_agreement` covers the noun predicate.
- **`possessive_agreement`** (`PossessiveAgreement`) — the possessive suffix
  matches an adjacent genitive pronoun. Severity: `Warning`/`Suggestion`.
  Correction: none. `Minun kirjasi on uusi.` The rule id `possessive_missing`
  covers the missing suffix.

Implementation notes:

- **Subject–predicate** — finite bearer (main verb, `olla` auxiliary, negative
  auxiliary), person/number; existential, passive, necessive and numeral
  constructions skipped.
- **Attribute** — prenominal adjective/ordinal/demonstrative; number mismatch
  is an error, case mismatch a warning; genitive modifiers, cardinals, quantity
  heads and relative pronouns skipped.
- **Predicative** — number agreement for adjectives/participles; noun predicate
  only for a clear number mismatch; partitive semantics stay quiet.
- **Possessive** — adjacent genitive pronoun vs suffix; warning/suggestion
  only.
- **Connegative** — negative auxiliary + main verb; past participle and
  conditional/potential accepted; 2sg imperative gets `mene`, plural/third
  imperative connegatives stay uncorrected (`Repaired`).

The agreement checks are separate, not interchangeable:

- **Subject–predicate** — controller: subject person/number; bearer: finite
  main verb or `olla` auxiliary; construction: ordinary, copular; correction:
  inflection.
- **Negative auxiliary** — the negative auxiliary carries person/number; the
  main verb is the connegative; construction: negative; correction:
  connegative selection, 2sg imperative only.
- **Compound tense** — the active past participle agrees in number with the
  subject; construction: ordinary/copular; correction: inflection.
- **Predicate complement** — adjective/participle number; noun predicate only
  for a clear number mismatch; construction: copular; correction:
  inflection/curated.
- **Attribute** — adjective/ordinal/demonstrative agrees with its nominal
  head; any construction; correction: inflection.
- **Possessive** — the suffix matches an adjacent genitive pronoun; any
  construction; correction: none.
- **Passive** — no subject agreement; the passive participle is not a subject
  target; construction: passive; correction: none.
- **Existential** — singular predicate; postverbal nominative/partitive
  subject; construction: existential; correction: none.
- **Necessive** — genitive subject; 3sg predicate; construction: necessive;
  correction: none.
- **Experiencer** — case-marked experiencer; no subject–predicate reuse;
  construction: experiencer (gated); correction: rule-specific.

The controller, the agreement bearer and the connegative selection are three
different questions. A rule that changes one must not silently change another.

### 3.2 Rection, case and objects

- **`verb_rection`** (`VerbRection`) — a curated valency frame requires a case
  the argument lacks. Severity: `Error`/`Warning`. Correction:
  inflection/curated. `Pidän sinä.` → `sinusta`.
- **`case_selection`** (`CaseSelection`) — numeral government, a curated
  adposition frame, or the existential subject case. Severity:
  `Error`/`Warning`. Correction: inflection/curated. `Kaksi lapsi tuli.` →
  `lasta`. The rule ids are `numeral_case`, `adposition_case` and
  `existential_subject_case`.
- **`object_case`** (`ObjectCase`) — a negative clause needs the partitive, a
  bare nominative object, or an aspect cue. Severity: `Error`/`Suggestion`.
  Correction: inflection. `En lue kirja.` → `kirjaa`. The rule id
  `bare_nominative_object` covers the bare nominative object.

Implementation notes:

- **Verb rection** reads `data/verb_rection.csv`; a curated frame is a lexical
  fact, never inferred from position.
- **Case selection** reads `data/adposition_cases.csv` and numeral rules;
  existential subject case is detected but may not choose the repair.
- **Object case** covers negative partitive, bare nominative object, and aspect
  cues (progressive/completive); aspect suggestions only.
- **Arguments** read `data/verb_arguments.csv`: obligatory slots and closed
  intransitive frames; no automatic correction.

### 3.3 Arguments and structure

- **`missing_argument`** (`MissingArgument`) — an obligatory slot from
  `verb_arguments.csv` is absent. Severity: `Warning`. Correction: none.
  `Hän luottaa.`
- **`extra_argument`** (`ExtraArgument`) — a closed intransitive frame has an
  object. Severity: `Warning`. Correction: none. `Nukun kirja.`
- **`clause_structure`** (`ClauseStructure`) — a declarative clause lacks a
  predicate or ends in an uncoordinated conjunction. Severity: `Warning`.
  Correction: none. `Koska minä väsynyt.` The rule ids are
  `incomplete_coordination` and `missing_predicate`.

Implementation notes:

- **Missing predicate** — only a sentence-final subordinate clause with a
  personal pronoun subject and a predicative word but no finite verb.
- **Incomplete coordination** — a clause with a predicate ends in a
  coordinator at a declarative period.
- Neither carries a correction.

### 3.4 Punctuation, lists, questions, quotes

- **`comma_missing`** (`CommaMissing`) — a subordinate clause boundary or an
  `esimerkiksi` parenthetical pair. Severity: `Error`/`Suggestion`. Correction:
  punctuation. `Menin kotiin koska olin väsynyt.` The rule id
  `parenthetical_comma` covers the parenthetical pair.
- **`comma_extra`** (`CommaExtra`) — a comma between a personal-pronoun
  subject and a finite predicate. Severity: `Warning`. Correction: removal.
  `Minä, menen kotiin.`
- **`comma_pair_unbalanced`** (`CommaPairUnbalanced`) — an opened
  relative-clause comma pair is not closed. Severity: `Error`. Correction:
  punctuation. `Koira, joka nukkuu sohvalla on minun.`
- **`semicolon_misuse`** (`SemicolonMisuse`) — a semicolon in a position the
  specification forbids: a main/subordinate or coordinated-complement boundary,
  a subject/predicate or verb/object/complement split, a sentence-final
  semicolon, or a short parallel list that wants commas. Severity:
  `Suggestion`. Correction: punctuation (`;` → `,`, deleted with its following
  space, or `;` → `.`). `Kun tulin kotiin; kaikki nukkuivat.` The rule ids are
  `semicolon_subordinate`, `semicolon_coordination`, `semicolon_phrase`,
  `semicolon_terminal` and `semicolon_simple_list`.
- **`semicolon_missing`** (`SemicolonMissing`) — a complex list where the first
  group and an existing semicolon establish a repeated group shape, but an
  outer comma still separates two groups. Severity: `Suggestion`. Correction:
  punctuation; replace every outer comma with `;` in one edit set
  (`semicolon_list_hierarchy`). A fully comma-separated list stays quiet.
- **`question_mark_missing`** (`QuestionMarkMissing`) — a clear direct
  question ends with `.` or no mark. Severity: `Error`. Correction:
  punctuation. `Missä sinä asut.` → `?`. A dash-interrupted or line-wrapped
  question gets no mark at the break (`Muistatko--muistatko …`,
  `Voitteko …\ntehdä …?`); a correlative relative (`Minkä …, se …`) and a
  rhetorical conditional question (`Mitä … tulisinkaan …, jos …`) are not
  direct questions and stay quiet.
- **`unbalanced_quote`** (`UnbalancedQuote`) — a quotation is still open at the
  end of the text. Severity: `Warning`. Correction: none.
  `Hän sanoi: »Elä tule.`
- **`list_separator_missing`** (`ListSeparatorMissing`) — adjacent list items
  have no separator. Severity: `Error`. Correction: punctuation.
  `ostin omenoita appelsiineja`
- **`list_separator_duplicate`** (`ListSeparatorDuplicate`) — an item is
  separated twice. Severity: `Error`. Correction: removal.
  `omenoita,, ja banaaneja`
- **`list_comma_before_conjunction`** (`ListCommaBeforeConjunction`) — a comma
  before the final `ja`/`tai`/`sekä`. Severity: `Suggestion`. Correction:
  removal. `omenoita, ja banaaneja`
- **`list_introducer_comma`** (`ListIntroducerComma`) — a comma directly after
  a list-introducing colon. Severity: `Error`. Correction: removal.
  `kolme asiaa:, omenoita`

Implementation notes:

- Subordinate/relative boundaries use commas, dashes and terminal marks.
- Missing relative closing comma is detected when the parser swallowed the
  host predicate; restrictive relatives without an opening comma stay quiet.
- Parenthetical commas are suggestions from a closed cue list.
- Questions: `-ko`/`-kö` and interrogative first words; reported/echo questions
  are guarded.
- Lists: missing/duplicate separators, comma before conjunction, colon
  introducer.
- Semicolons: a valid main-main semicolon stays quiet; only structural misuse
  is reported. A semicolon-separated list is one list whose items are the
  semicolon groups, so a complex list keeps its internal commas and a short
  parallel list is offered commas. Spacing typography is a separate pass.
- Quotes: unbalanced quote warning; quote style/dialogue dashes are not
  corrected.

### 3.5 Reserved

- **`word_order`** (`WordOrder`) — **reserved**; no rule emits it (`spec only`,
  no example). Severity and correction: —. The design is in
  [`word_order.md`](word_order.md), and any future rule must be
  suggestion-only. `clause_type`, `construction`,
  `question_type`, `subject_position`, `object_case`, `focus_type`,
  `topic_position` and `register` are already computed for it.

A caller can dismiss one rule without hiding the rest of the checker. Some
kinds have more than one message key because they cover distinct sub-rules
(`case_selection`, `object_case`, `predicative_agreement`,
`possessive_agreement`, `clause_structure`). The contract test
`tests/syntax_contract.rs::every_fixture_diagnostic_has_a_contract_entry`
checks every emitted pair against this catalogue.

## 4. Correction policy

- `Error` requires a clear cue, a likely role and a single relevant reading.
- `Plain` inflection forms are the only inflection corrections; `Repaired` and
  `Guessed` are accepted words but never suggestions.
- Unknown words, unknown roles, ambiguous agreeing targets and numeral phrases
  suppress the rule.
- Parenthetical commas are suggestions; a final conjunction without a comma is
  never an error; a semicolon is never an error. A valid main-main semicolon
  stays quiet; a structurally forbidden semicolon is a suggestion-only
  correction.
- An unbalanced quote is a warning; no quote-style correction.
- A `WordOrder` diagnostic, if it ever lands, is suggestion-only.

### 4.1 Confidence layers

Do not use an unqualified “confidence”:

- **form** — `Plain`, `Repaired`, `Guessed`; owner: morphology generation.
- **role** — `Likely`, `Weak`; owner: syntax.
- **diagnostic** — `Certain`, `Likely`, `Ambiguous`; owner: checker.
- **correction** — `Certain`, `Likely`, `Ambiguous`; owner: correction.

The propagation policy is:

- **single relevant reading + strong role** — diagnostic ceiling `Certain`;
  correction `Plain` form only.
- **single relevant reading + weak role** — diagnostic ceiling `Likely`;
  correction `Plain`, likely confidence.
- **competing readings** — `Ambiguous` → suppressed; no correction.
- **repaired form** — warning-level if the rule uses it; **never** an
  inflection suggestion.
- **guessed form** — suppressed for correction; never.
- **curated table row** — as the table states; correction `Curated`.
- **mechanical punctuation** — `Certain` for a clear boundary; correction
  `Punctuation`.

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

- `data/verb_rection.csv` — `lemma,case,frame,priority,note` valency frames.
- `data/verb_arguments.csv` — `lemma,slot,case,requirement,note` argument
  completeness.
- `data/adposition_cases.csv` — `adposition,case,example` governed-case rows.
- `maps/verb_rection.bin.gz` — `(verb lemma → governed cases)`.
- `maps/verb_arguments.bin.gz` — `(verb lemma → argument frames)`.
- `maps/adposition_cases.bin.gz` — `(adposition → governed case)`.

## 6. Evaluation baseline

As of 2026-11-19:

```text
syntax_eval clean rows 154: 0 errors, 0 warnings, 0 suggestions
syntax_eval cases 60: every emitted kind precision 1.00, recall 1.00
syntax_diagnostics: 75 exact-output cases, 272 clean rows
prose: 266 clean rows / 6,349 words, 2 true positives, 0 known false positives
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

Measured on the committed fixture (`page-2000`, 1,959 words), the native
release build and the demo/wasm shape:

- **cold morphology, native single-thread** — 674 ms.
- **cold morphology, native 4-worker** — 202 ms.
- **warm morphology** — 0.46 ms.
- **warm syntax analysis** — 5.41 ms.
- **warm grammar rules** — 12.34 ms.
- **warm style** — 2.50 ms.
- **warm grammar + style (full pass)** — 19.29 ms.
- **one edited sentence, native** — 3.22 ms.
- **cold page, release wasm, 4 workers alive** — 364–395 ms.
- **first check of a browser session** — ~950 ms (worker start-up).
- **cold-page target** — ≤500 ms per page, met once the workers exist.

The native rows are the benchmark reference; the wasm rows are the last
recorded browser measurement. The first check of a session pays the four
worker spawns and wasm instantiations once.

The morphology cache is per textarea and bounded; the sentence cache is keyed
by grammar-unit text; there is no AST cache. The wasm module carries the 104k-lemma
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

1. tokenise.
2. morphology.
3. syntax analysis.
4. rule evaluation.
5. ambiguity filtering.
6. severity assignment (`Error` → `Warning` when `Likely`).
7. correction generation.
8. rank (severity, confidence, span, kind).
9. diagnostics.

## 2. API guarantees

- **spans** — half-open UTF-8 byte ranges `start..end`.
- **token indices** — zero-based; refer to the full token stream, including
  separators.
- **word leaves** — `Word` nodes cover only word tokens; separators are not
  leaves.
- **clause/list indices** — zero-based; absent values are `Option`.
- **clause token ranges** — endpoints are words; interior separators are
  included.
- **diagnostic order** — severity, diagnostic confidence, `span.start`,
  `span.end`, kind; one diagnostic per span.
- **correction order** — score, then note; edits applied from the end
  backwards.
- **cache reuse** — same analysis and diagnostics; only cache/depth counters
  differ.
- **stable** — `DiagnosticKind` names, `RuleId` strings, correction licenses
  and span semantics.
- **0.x unstable** — AST node kinds/fields, `Clause` fields and JSON shapes;
  additive changes are documented.

The contract tests are `tests/syntax_contract.rs`; the exact diagnostics live
in `tests/fixtures/syntax_diagnostics.json`. Run
`python3 tools/check_docs.py` (add `--render` for the pandoc plain-text export)
to check the Markdown tables, column and cell limits, links, heading spacing
and anchors, code fences and the exported text of this document.

## 3. Coverage matrix

- **nominal inflection** — recognition: yes; syntax integration: yes;
  checking: case/rection/object; correction: yes, confidence-dependent.
- **verb inflection** — recognition: yes; syntax integration: yes; checking:
  agreement/connegative; correction: yes, confidence-dependent.
- **possessive suffixes** — recognition: yes; syntax integration: partial;
  checking: possessive agreement; correction: no.
- **clitics** — recognition: yes; syntax integration: question detection;
  checking: question marks; correction: limited.
- **compounds** — recognition: yes; syntax integration: partial; checking:
  limited; correction: no.
- **derivations** — recognition: yes; syntax integration: partial; checking:
  style/spell separate; correction: no.
- **numerals** — recognition: yes; syntax integration: numeral phrases;
  checking: numeral case; correction: yes.
- **proper names** — recognition: yes; syntax integration: subject/object;
  checking: limited; correction: no.
- **relative clauses** — recognition: yes; syntax integration: host/AST;
  checking: punctuation; correction: limited.
- **lists** — recognition: yes; syntax integration: list arena/AST; checking:
  punctuation; correction: yes.
- **parentheticals/quotes** — recognition: yes; syntax integration: AST;
  checking: punctuation; correction: no.
- **word order** — recognition: features; syntax integration: constituent
  view; checking: reserved; correction: no.
- **agreement** — recognition: yes; syntax integration: roles/features;
  checking: yes; correction: yes.
- **rection** — recognition: curated; syntax integration: roles; checking:
  yes; correction: curated.
- **argument structure** — recognition: curated; syntax integration: roles;
  checking: warnings; correction: no.

“Partial” means the feature is recognised but the syntax integration or the
rule set is deliberately narrower than the morphology. The exact code path and
the fixture for each row belong in the feature-specific sections above.

---

# Part V — File map

- **tokens, sentence layer** — `src/tokenise.rs`, `src/sentence.rs`.
- **morphology cache / readings** — `src/morphology/`.
- **tags and form model** — `src/tags.rs`.
- **inflection grammar** — `src/grammar/`, `src/expand/`.
- **per-class engines** — `src/expand/nominals/`, `src/expand/verbs/`.
- **compounds / derivations** — `src/compound.rs`, `src/derivation.rs`.
- **syntax analysis** — `src/syntax/`.
- **AST** — `src/syntax/ast/`.
- **lists** — `src/syntax_list.rs`.
- **grammar checker** — `src/syntax_check/`.
- **checker diagnostics** — `src/syntax_check/diagnostic.rs`,
  `src/diagnostic.rs`.
- **checker corrections** — `src/syntax_check/correction.rs`.
- **checker data** — `data/verb_rection.csv`, `data/verb_arguments.csv`,
  `data/adposition_cases.csv`.
- **checker tests** — `tests/fixtures/syntax_*.json`,
  `tests/fixtures/prose/adjudicated.json`.
- **AST tests** — `src/syntax/ast/`, `tests/syntax_tree.rs`.
- **word order** — [`word_order.md`](word_order.md),
  `src/syntax/features.rs`.
- **AST design** — [`../misc_md/PLAN_SYNTAX_AST.md`](../misc_md/PLAN_SYNTAX_AST.md),
  [`../misc_md/PLAN_SYNTAX_AST_REST.md`](../misc_md/PLAN_SYNTAX_AST_REST.md).
- **grammar/syntax fixes** —
  [`../misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md`](../misc_md/PLAN_GRAMMAR_SYNTAX_FIXES.md).
- **style layer** — [`style_checker.md`](style_checker.md).
- **hyphenation** — [`hyphenation.md`](hyphenation.md).
- **verification** — [`harness.md`](harness.md),
  [`../misc_md/TODO.md`](../misc_md/TODO.md).

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
