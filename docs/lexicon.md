# The noun lexicon (`data/kotus_nomines.sqlite`)

`tools/build_nomine_lexicon.py` builds a **materialised paradigm** for every
classed noun-like word: `data/kotus_nomines.sqlite`. It is the answer to the
oracle question the [verification harness](harness.md) asks of Omorfi and
Voikko every time it runs, written down once, so the same verdict can be read
back with a query instead of live morphology calls.

This is the reference the noun half of the harness compares torille against.
Read it, then rebuild it when the oracle or the source data changes.

## Why it exists

The run-time harness asks two C++ libraries per cell, per lemma, every run:

* **Omorfi** which forms *exist*;
* **Voikko** which of those are *real*.

That is authoritative but slow and not reproducible without both libraries in
the environment. The lexicon asks the same two libraries once, stores the
answer, and lets the harness read `ok='both'` back in milliseconds with no
Omorfi or Voikko loaded. It is the same authority, saved — not a second
opinion. The live path stays in `omorfi_harness.py` for verbs and for anything
the lexicon does not cover; the lexicon covers the nominal grid
(nouns → pronouns), which is the biggest share of the work.

## The two tables

The schema is deliberately exactly two tables. One is the reference for a
word, the other is the paradigm, and everything about a word is written once
in the first and joined to in the second.

```
base:   lemma │ homonym │ part_of_speech │ class              (one row per word)
forms:  lemma │ homonym │ form │ case │ number │ possessive │ ok
                                                              (one row per form per cell)
```

* **`base`** is one row per word. `lemma` is the headword; `homonym` is the
  source's Homonymia index (empty for the sole word); `part_of_speech` is the
  whole Sanaluokka column, which is often several words (`toinen` is `numeraali,
  järjestysluku, pronomini, adjektiivi`); `class` is the whole Taivutustiedot
  column, verbatim, so it can hold the rich shapes — `9, 50` is two classes,
  `8, (21)` declines its plural like class 21. The primary key is
  `(lemma, homonym)`, because the source is: `ahtaus` is one spelling that is
  class 40 *and* class 39, two words with one surface form.
* **`forms`** is one row per *form per cell*. A cell is
  `(case, number, possessive)` and the three are already columns spelled out —
  `nominative`, `singular`, `first person singular` — so there is no code table
  and no third table for the grammar. `ok` says whether both oracles agreed the
  word is real (`both`) or only Omorfi asserted it (`omorfi`). A form answering
  two cells — `kissani` is both nominative and genitive — is one row per cell,
  because it is one word and two cells, and the primary key is
  `(lemma, homonym, form, case, number, possessive)`.

The current build: **25 401 words, 4 261 603 form rows, 245 classes**,
657 MB. 3 500 922 rows are `ok='both'`; 760 681 are `ok='omorfi'`.

## Building it

```bash
python3 tools/build_nomine_lexicon.py -o data/kotus_nomines.sqlite
```

Defaults to sqlite, over the nominal parts of speech
(`substantiivi,numeraali,adjektiivi,pronomini`), the full 13-case × 2-number ×
6-possessive grid. Notable flags:

* `-n N` — a sample of N lemmas per class (a 30-second smoke build).
* `--classes 9,9*B,27` — a subset of classes; `--classes numeric` limits to
  the 51 plain Kotus classes; `--classes all` (default) keeps the `*` variants
  separate, so `9` and `9*B` — a gradation — stay apart.
* `--pos ...` — the Sanaluokka values to include; `--pos numeraali` gets the
  numerals, which are recorded under the same noun classes as nouns.
* `--possessives ''` — the bare case/number grid only.
* `--engine` — after the Omorfi + Voikko grid, add the engine's own forms and
  missing bases (including classless compounds). This is the default and needs
  the `torille` Python binding; `--no-engine` leaves an oracle-only file.
  `--engine-only` runs just that pass against an existing sqlite file.
* `-j N` — worker processes over classes. The complete build takes ~40 minutes
  at `-j 4` and ~4 GB of RAM; see [Cost](#cost) below for why that is.

`-f csv|tsv|json` writes the same facts as flat files or a nested document; the
sqlite form is the one to use as a reference.

The Omorfi boundary marker (U+2019, written where a stem meets its ending) is
stripped as the lexicon is built — `keskiko'okseen` is the same word as
`keskikookseen`, and no Finnish word contains an apostrophe.

## Using it as the harness reference

The confirmed paradigm of a word is the set of forms whose `ok` is `both`:

```sql
SELECT DISTINCT form FROM forms
  WHERE lemma = 'talo' AND ok = 'both';
```

That set is exactly what `Harness.confirmed_forms()` returns at run time: every
cell the oracle answers, every spelling of it that Voikko reads as the lemma.
torille's recall and precision passes compare against this same set whether it
comes from a live `confirmed_forms()` call or from this query. A harness pass
that reads the lexicon:

```python
def reference_set(conn, lemma):
    return {r[0] for r in conn.execute(
        "SELECT DISTINCT form FROM forms WHERE lemma=? AND ok='both'", (lemma,))}
```

which is precise enough and about three orders of magnitude cheaper than the
live path — no Omorfi load (7.5 s), no Voikko, no 156 generate calls per word.

The cell-facing question is equally direct — "what is the genitive singular
1st-person plural of `talo`":

```sql
SELECT form FROM forms
  WHERE lemma = 'talo'   AND [case]='genitive'
    AND number='singular' AND possessive='first person plural';
-- talomme
```

## How the harness should read it

| thing to check | the query |
|---|---|
| the paradigm torille must offer as `plain` | `SELECT DISTINCT form FROM forms WHERE lemma=? AND ok='both'` |
| which cells produced that form | rows of `forms` for that lemma, grouped by `case,number,possessive` |
| is `taloixni` (engine offered it) a real word of `talo` | `SELECT ok FROM forms WHERE lemma='talo' AND form='taloixni'` — no row ⇒ not a word |
| the class that owns a form | `JOIN base USING (lemma,homonym)`, read `class` |

The homonym key is load-bearing: join `forms` to `base` on *both* `lemma` and
`homonym`, never `lemma` alone, or `ahtaus`'s two words are fused. `forms` does
not repeat `class`; it is the price of keeping one canonical place for a word's
own facts.

## Caveats

1. **The oracle reaches only as far as Omorfi's dictionary.** A word the
   sanalista classes but Omorfi has no morphology for produces no oracle forms.
   The default `--engine` pass fills those from the engine and adds a `base`
   row; `--no-engine` leaves the gap. `kahdeksan` is a numeral in class 10 with
   a perfectly regular paradigm, but it is not in Omorfi's dictionary, so only
   the engine pass can put it in the lexicon. That is the harness's existing
   `no confirmed form` skip (`iso` in Omorfi's case) — a word with no oracle to
   disagree with is skipped, not reported as a mass of missing. Build scope
   means nouns dominate; 25 401 words survive out of 32 485 dispatched, and the
   gap is words Omorfi does not know.
2. **`ok='omorfi'` is not authoritative.** Those rows are forms Omorfi asserts
   that Voikko's finite dictionary does not back. The reference set is `both`
   only — the same rule `confirmed_forms()` uses. Every `instructive`/`comitative`
   possessive in the file is `omorfi` and never `both`, which is the lexicon
   agreeing with the harness's "instructive takes no possessive" rule from the
   other direction.
3. **The grid is the one the harness checks**: 13 cases × 2 numbers × 6
   possessives, which is exactly `noun_cells(ALL_CASES, ["SG","PL"],
   ALL_POSSESSIVES)`. Build with `--possessives ''` if the bare case/number
   grid is the question.
4. **Scope is nominal.** Verbs and their finite forms — and infinitives — are a
   different grid (mood/tense/person) and live in the live harness, not here.

## Cost

Why a one-shot build is heavy, and why that is a fair price:

* the work is the C++ morphology libraries doing ~5 million cell generations,
  on 4 cores for ~40 minutes;
* the driver process and four workers each hold one Omorfi + Voikko (~4 GB
  total on this machine's 12 GB);
* it is a bounded batch that produces a 657 MB closed database — no processes
  remain when it finishes;
* the *reader* pays none of that: a query is milliseconds with nothing loaded.

## Rebuilding

Rebuild when the source data (`data/nykysuomensanalista.csv`) or the oracle
versions change:

```bash
rm -f data/kotus_nomines.sqlite   # a partial file would be read as complete
python3 tools/build_nomine_lexicon.py -o data/kotus_nomines.sqlite -j 4
```

The file is a generated artifact; if it is committed, the `.base` convention,
schema changes and adds are all captured by this one command.