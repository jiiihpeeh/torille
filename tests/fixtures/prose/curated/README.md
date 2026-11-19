# Curated prose fixtures

Each `*.txt` file here is manually curated clean prose. The test reads every
non-empty line as one paragraph and requires it to produce **zero** grammar
diagnostics. Drop a new UTF-8 file here and re-run:

```bash
cargo test --release -p torille --test syntax_prose
```

Rules:

- one paragraph per line; do not wrap a paragraph across lines;
- a line starting with `#` is a comment and is skipped;
- a paragraph that produces a diagnostic is not admitted here until either the
  checker is fixed or the paragraph is adjudicated in
  [`../adjudicated.json`](../adjudicated.json);
- keep the source and licence of a new file in this README or in the commit
  message. The raw downloads under `../sources/` are gitignored; only the
  curated text is committed.

The committed files are excerpts, not new works. The authors died by 1955, the
works are public domain in Finland and the EU, and the transcriptions come from
the named volunteers' projects.

## `lonnrot.txt` — Projekti Lönnrot novels

86 narrative and dialogue paragraphs from seven public-domain novels and story
collections. Some transcriptions are ISO-8859-1 rather than UTF-8; the source
text was decoded accordingly before curation.

| id | author | work | year |
|---|---|---|---|
| 3717 | Pentti Haanpää | *Iisakki vähäpuheinen* | 1953 |
| 3715 | Pentti Haanpää | *Karavaani ja muita juttuja* | 1930 |
| 2508 | Maiju Lassila | *Pirttipohjalaiset* | 1911 |
| 0988 | Joel Lehtonen | *Punainen mylly* | 1913 |
| 0886 | Joel Lehtonen | *Sorron lapset (Punainen mies I)* | 1923 |
| 0559 | Volter Kilpi | *Bathseba* | 1900 |
| 0779 | Maria Jotuni | *Tyttö ruusutarhassa* | 1927 |

Raw zips: `https://lonnrot.net/kirjat/<id>.zip`, saved locally under
`../sources/lonnrot/`.

The Iisakki rows use the reviewed modern correction `laiteille` → `laidoille`.
The final `Sukset olivat kulkuneuvo` sentence is left out because the
predicative-noun rule flags the singular class-noun predicate; it is recorded
for the next precision round rather than forced into the clean set.

## `doria.txt` — Doria factual prose

40 expository paragraphs from three **Public Domain Mark 1.0** books from 1900.

| item | author | work |
|---|---|---|
| [101475](https://doria.fi/handle/10024/101475) | Ragnar Hult | *Suomen maantiede* |
| [127971](https://doria.fi/handle/10024/127971) | E. N. Setälä | *Äidinkielen oppikirja kansakouluja varten, yleinen osa* |
| [127972](https://doria.fi/handle/10024/127972) | E. N. Setälä | *Äidinkielen oppikirja kansakouluja varten, toinen osa* |

Fetch the PDFs from the item's bitstream and extract the OCR layer with
`pdftotext -layout`. The OCR has the usual confusions; only explicit, reviewed
corrections are applied — there is no general OCR normalizer. The corrections
used for the admitted rows are:

| OCR | corrected |
|---|---|
| `spenaattia` | `pinaattia` |
| `rabarbcria` | `rabarbia` |
| `parsaheiuää` | `parsaheinää` |
| `sei61 laisia` | `sekalaisia` |
| `lisalmeen` | `Iisalmeen` |
| `tarvotaan` | `tarvitaan` |
| `kuusain` | `kuusama` |
| `pyssy miehiä` | `pyssymiehiä` |

A paragraph is admitted only after the corrected text passes the individual
check; an uncorrected or uncertain paragraph is left out.

### Re-OCRing a PDF

The source PDFs carry a publisher OCR layer. If a page is too damaged and a
working `ocrmypdf` is available, re-OCR it with the Finnish Tesseract model:

```bash
ocrmypdf -l fin --force-ocr --output-type pdf input.pdf output.pdf
pdftotext -layout output.pdf output.txt
```

Keep the re-OCR output under `../sources/` (gitignored) and commit only the
reviewed corrections in the curated rows.

A full `ocrmypdf -l fin --force-ocr` pass was run on *Suomen maantiede*. It
fixes the diacritics and many word breaks (`parsaheinää`, `rabarberia`) but
still leaves layout spacing and occasional slips (`Kanpan` for `Kaupan`), so
the reviewed corrections above remain the authority; the re-OCR is a source,
not an automatic replacement.
