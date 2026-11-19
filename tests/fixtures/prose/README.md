# Adjudicated prose fixture

`adjudicated.json` is the reviewed-prose gate for the grammar checker. Run it
with:

```bash
cargo test --release -p torille --test syntax_prose
```

The test prints the current clean-row and word counts. `clean` rows must
produce no diagnostics, `errors` rows must produce exactly the listed kinds,
and `known_false_positives` rows pin reviewed bugs: a fix makes the entry fail
so the row must be moved deliberately.

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
- **Projekti Lönnrot** — seven public-domain novels and story collections by
  Pentti Haanpää, Maiju Lassila, Joel Lehtonen, Volter Kilpi and Maria Jotuni.
  The authors died by 1955, so the works are public domain in Finland and the
  EU. Download each zip under `sources/lonnrot/`:

  ```bash
  mkdir -p tests/fixtures/prose/sources/lonnrot
  for id in 3717 3715 2508 0988 0886 0559 0779; do
    curl -L --fail -o "tests/fixtures/prose/sources/lonnrot/$id.zip" \
      "https://lonnrot.net/kirjat/$id.zip"
  done
  ```

  | id | author | work | year |
  |---|---|---|---|
  | 3717 | Pentti Haanpää | *Iisakki vähäpuheinen* | 1953 |
  | 3715 | Pentti Haanpää | *Karavaani ja muita juttuja* | 1930 |
  | 2508 | Maiju Lassila | *Pirttipohjalaiset* | 1911 |
  | 0988 | Joel Lehtonen | *Punainen mylly* | 1913 |
  | 0886 | Joel Lehtonen | *Sorron lapset (Punainen mies I)* | 1923 |
  | 0559 | Volter Kilpi | *Bathseba* | 1900 |
  | 0779 | Maria Jotuni | *Tyttö ruusutarhassa* | 1927 |

  Some transcriptions are ISO-8859-1 rather than UTF-8; the extractor decodes
  UTF-8 first and falls back to ISO-8859-1. The rows are narrative and
  dialogue paragraphs from the middle of each work, admitted only after the
  individual check.
- **Doria (Kansalliskirjasto)** — three Public Domain Mark 1.0 factual books
  from 1900, chosen to give the checker expository prose rather than fiction:

  | item | author | work |
  |---|---|---|
  | [101475](https://doria.fi/handle/10024/101475) | Ragnar Hult | *Suomen maantiede* |
  | [127971](https://doria.fi/handle/10024/127971) | E. N. Setälä | *Äidinkielen oppikirja kansakouluja varten, yleinen osa* |
  | [127972](https://doria.fi/handle/10024/127972) | E. N. Setälä | *Äidinkielen oppikirja kansakouluja varten, toinen osa* |

  Fetch the PDFs and extract the OCR layer:

  ```bash
  mkdir -p tests/fixtures/prose/sources/doria
  curl -L --fail -o tests/fixtures/prose/sources/doria/Suomen_maantiede.pdf \
    'https://doria.fi/bitstream/handle/10024/101475/Suomen_maantiede.pdf?sequence=1&isAllowed=y'
  curl -L --fail -o tests/fixtures/prose/sources/doria/Aidinkielen_oppikirja_kansakouluja_varten_Yleinen_osa.pdf \
    'https://doria.fi/bitstream/handle/10024/127971/Aidinkielen_oppikirja_kansakouluja_varten_Yleinen_osa.pdf?sequence=1&isAllowed=y'
  curl -L --fail -o tests/fixtures/prose/sources/doria/Aidinkielen_oppikirja_kansakouluja_varten-Toinen_osa_Vuorokurssit.pdf \
    'https://doria.fi/bitstream/handle/10024/127972/Aidinkielen_oppikirja_kansakouluja_varten-Toinen_osa_Vuorokurssit.pdf?sequence=1&isAllowed=y'
  for f in tests/fixtures/prose/sources/doria/*.pdf; do
    pdftotext -layout "$f" "${f%.pdf}.txt"
  done
  ```

  The OCR layer has the usual confusions. Only **explicit, reviewed**
  corrections are applied; there is no general OCR normalizer. The corrections
  used for the admitted rows are:

  | OCR | corrected |
  |---|---|
  | `spenaattia` | `pinaattia` |
  | `rabarbcria` | `rabarbia` |
  | `parsaheiuää` | `parsaheinää` |
  | `sei61` | `sellaisia` |
  | `lisalmeen` | `Iisalmeen` |
  | `tarvotaan` | `tarvitaan` |
  | `kuusain` | `kuusama` |
  | `pyssy miehiä` | `pyssymiehiä` |

  A paragraph is admitted only after the corrected text passes the individual
  check; an uncorrected or uncertain paragraph is left out.

## Admission rule

A row is added to `clean` only after `check_syntax` reports zero diagnostics
for that exact text. A row with a diagnostic is either:

- added to `errors` when the diagnostic is a true positive;
- added to `known_false_positives` when it is a reviewed checker bug, with the
  exact kinds; or
- left out of the fixture until the bug is fixed.

The Finlex selection was taken from complete `<p>` paragraphs that passed the
individual check. Definition-list fragments, headings and section references
were excluded.

## Current baseline

As of 2026-11-19: **246 clean rows / 5,789 words**, 1 true positive and 0 known
false positives. The exact count is whatever `syntax_prose` prints; update
`docs/syntax_checker.md` and `docs/grammar_syntax.md` when it moves.
