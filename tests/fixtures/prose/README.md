# Adjudicated prose fixture

`adjudicated.json` is the reviewed-prose gate for the grammar checker. The
manually curated novel and factual excerpts live in [`curated/`](curated/) and
join the same gate. Run both with:

```bash
cargo test --release -p torille --test syntax_prose
```

The test prints the current clean-row and word counts for each input and the
combined total. `clean` rows and every curated paragraph must produce no
diagnostics, `errors` rows must produce exactly the listed kinds, and
`known_false_positives` rows pin reviewed bugs: a fix makes the entry fail so
the row must be moved deliberately.

## Sources

The committed rows are excerpts, not new works. The raw downloads live under
`sources/`, which is gitignored; only the adjudicated rows are committed.

- **Project Gutenberg ebook 9082** — Juhani Aho, *Muistatko—? Kukkia
  Keväiseltä Niityltä*. Public domain. The committed slice is
  [`../perf/page-2000.txt`](../perf/page-2000.txt).
- **Finlex statute 729/2018** — *Tieliikennelaki* (Road Traffic Act), a
  consolidated statute. Finnish statutes and official decisions are not
  subject to copyright. Download the Akoma Ntoso XML with:

  ```bash
  mkdir -p tests/fixtures/prose/sources
  curl -L --fail -o tests/fixtures/prose/sources/tieliikennelaki.xml \
    https://www.finlex.fi/files/assets/tieliikennelaki.xml
  ```

  The sample is extracted from the `<p>` elements of the statute body. It is
  modern formal Finnish from a professionally edited legal text, which gives
  the checker a genre the 19th-century prose excerpt does not cover.
- **Projekti Lönnrot** and **Doria** — manually curated novel and factual
  excerpts live in [`curated/`](curated/). See
  [`curated/README.md`](curated/README.md) for the source IDs, licences and the
  explicit OCR corrections. Each file is one paragraph per line and is checked
  by the same `syntax_prose` gate.

## Admission rule

A row is added to `clean` or to a `curated/*.txt` file only after
`check_syntax` reports zero diagnostics for that exact text. A row with a
diagnostic is either:

- added to `errors` when the diagnostic is a true positive;
- added to `known_false_positives` when it is a reviewed checker bug, with the
  exact kinds; or
- left out of the fixture until the bug is fixed.

The Finlex selection was taken from complete `<p>` paragraphs that passed the
individual check. Definition-list fragments, headings and section references
were excluded.

## Current baseline

As of 2026-11-19: **140 adjudicated rows + 114 curated rows = 254 clean rows /
6,034 words**, 2 true positives and 0 known false positives. The exact count is
whatever `syntax_prose` prints; update `docs/syntax_checker.md` and
`docs/grammar_syntax.md` when it moves.
