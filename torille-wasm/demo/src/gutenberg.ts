// Finnish Project Gutenberg books for the demo's reader panel.
//
// The corpus is downloaded once by `tools/fetch_finnish_books.py` into
// `books/`: one UTF-8 `<id>.txt` per book plus `catalog.json`. The reader
// fetches both from the demo server, same-origin -- no gutendex, no
// gutenberg.org, no CORS, no `/proxy`.
//
// Regenerate with:
//   python3 tools/fetch_finnish_books.py

import { countWords } from './tokenize';

export interface Chapter {
  /** 0-based chapter index inside the book. */
  index: number;
  /** The header line as it appears in the source (e.g. "I.", "LUKU I"). */
  heading: string;
  /** The body of the chapter, after trim(). */
  body: string;
}

export interface Book {
  id: number;
  title: string;
  author: string;
  chapters: Chapter[];
}

export interface CatalogEntry {
  id: number;
  title: string;
  author: string;
}

/** Directory the downloader writes, relative to the served page. */
export const BOOKS_DIR = 'books';
/** The catalogue file the downloader writes. */
export const CATALOG_URL = `${BOOKS_DIR}/catalog.json`;
/** The UTF-8 text of book `id`. */
export function bookTextUrl(id: number): string {
  return `${BOOKS_DIR}/${id}.txt`;
}

/**
 * Load the catalogue (`books/catalog.json`). Throws when it is missing,
 * which is the "you have not run the downloader" case.
 */
export async function loadCatalog(signal?: AbortSignal): Promise<CatalogEntry[]> {
  const response = await fetch(CATALOG_URL, { signal });
  if (!response.ok) {
    throw new Error(
      `${CATALOG_URL}: HTTP ${response.status}. Run ` +
        '`python3 tools/fetch_finnish_books.py` to download the corpus.',
    );
  }
  const data: unknown = await response.json();
  if (!Array.isArray(data)) throw new Error(`${CATALOG_URL}: not an array`);
  const out: CatalogEntry[] = [];
  for (const raw of data as Record<string, unknown>[]) {
    const id = Number(raw?.id);
    if (!Number.isFinite(id)) continue;
    const title = String(raw?.title ?? '').trim();
    const author = String(raw?.author ?? '').trim();
    out.push({ id, title: title || `(#${id})`, author });
  }
  return out;
}

/**
 * Fold a string for search: lowercase and strip diacritics, so `lonnrot`
 * finds `Lönnrot` and `saastamala` is not needed to find `Sastamala`.
 */
function fold(value: string): string {
  return value
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase();
}

/**
 * Case- and diacritic-insensitive filter over the catalogue: a book
 * matches when its title or author contains `query`, or when `query` is
 * a Gutenberg id (digits, optionally prefixed with `#`). An empty query
 * returns the whole catalogue.
 */
export function filterCatalog(
  catalog: CatalogEntry[],
  query: string,
): CatalogEntry[] {
  const raw = query.trim();
  if (raw.length === 0) return catalog;
  const q = fold(raw);
  const idQuery = raw.startsWith('#') ? raw.slice(1) : raw;
  const isId = /^\d+$/.test(idQuery);
  return catalog.filter(
    (c) =>
      fold(c.title).includes(q) ||
      fold(c.author).includes(q) ||
      (isId && String(c.id).includes(idQuery)),
  );
}

/**
 * Load + parse one book from the local corpus. `entry` supplies the
 * title/author for the reader header.
 */
export async function loadBook(
  entry: CatalogEntry,
  signal?: AbortSignal,
): Promise<Book> {
  const url = bookTextUrl(entry.id);
  const response = await fetch(url, { signal });
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  const raw = await response.text();
  const body = stripBoilerplate(raw);
  // Chapters first, then pages: a chapter within the word budget stays a
  // single entry, a long one becomes several consecutive entries
  // ("LUKU I — osa 2/5"). `paginateChapter` copies the source chapter's
  // `index`, so the flattened list is renumbered here.
  const chapters = splitChapters(body)
    .flatMap((ch) => paginateChapter(ch))
    .map((ch, i) => ({ ...ch, index: i }));
  return {
    id: entry.id,
    title: entry.title,
    author: entry.author,
    chapters,
  };
}

/**
 * Strip the Project Gutenberg license block + transcriber's notes + footer.
 * The standard markers are
 *   `*** START OF (THE|THIS) PROJECT GUTENBERG ... ***`
 *   `*** END OF (THE|THIS) PROJECT GUTENBERG ... ***`
 * but a few old texts use `_START_` / `_END_` or `***START***` so the
 * regex is forgiving.
 */
export function stripBoilerplate(text: string): string {
  const startRe = /\*\*\*\s*START OF (THE|THIS)?\s*PROJECT GUTENBERG[^*]*\*\*\*/i;
  const endRe = /\*\*\*\s*END OF (THE|THIS)?\s*PROJECT GUTENBERG[^*]*\*\*\*/i;
  let body = text;
  const startMatch = body.match(startRe);
  if (startMatch?.index !== undefined) {
    body = body.slice(startMatch.index + startMatch[0].length);
  }
  const endMatch = body.match(endRe);
  if (endMatch?.index !== undefined) {
    body = body.slice(0, endMatch.index);
  }
  return body.trim();
}

/**
 * Heuristic: is `line` (already trimmed) a chapter header?
 *
 * Finnish Gutenberg texts vary widely. Common patterns we recognise:
 *   * Roman-numeral chapters: `I.`, `II.`, ..., `L.` (Kalevala's 50 songs
 *     are numbered this way.)
 *   * Named chapters: `LUKU I`, `TOINEN LUKU`, `ENSIMMÄINEN NÄYTÖS`,
 *     `OSA II`, `CHAPTER III`, `ACT I`.
 *
 * The check is conservative on length (< 80 chars) so random body text
 * that happens to contain the word `LUKU` is not misclassified.
 */
const ROMAN_CHAPTER = /^(?:[IVXLCDM]+)\.?\s*$/;
const KEYWORD_CHAPTER =
  /^(?:LUKU|OSA|NÄYTÖS|CHAPTER|ACT|LUVUN)\s+(?:[IVXLCDM]+|\d+|[A-ZÅÄÖ]+\b)?\.?\s*$/i;
const ORDINAL_CHAPTER =
  /^(?:ENSIMMÄINEN|TOINEN|KOLMAS|NELJÄS|VIIDES|KUUDES|SEITSEMÄS|KAHDEKSAS|YHDEKSÄS|KYMMENES|ENSIMMÄISEN|TUNNIN|JOHDASTO)\s+(?:LUKU|OSA|NÄYTÖS)\b.*$/i;

function looksLikeChapter(line: string): boolean {
  if (line.length === 0 || line.length > 80) return false;
  return (
    ROMAN_CHAPTER.test(line) ||
    KEYWORD_CHAPTER.test(line) ||
    ORDINAL_CHAPTER.test(line)
  );
}

/**
 * Split a Gutenberg body into chapters. Anything before the first detected
 * chapter is bundled under "Alku" (preface / front matter). Books with no
 * detectable chapter structure come back as a single chapter. A chapter
 * still too long for one reader page is split further by `paginateChapter`.
 */
export function splitChapters(body: string): Chapter[] {
  const lines = body.split('\n');
  const out: Chapter[] = [];
  let heading = 'Alku';
  let buf: string[] = [];

  const flush = (): void => {
    const text = buf.join('\n').trim();
    if (text.length > 0) {
      out.push({ index: out.length, heading, body: text });
    }
  };

  for (const line of lines) {
    const trimmed = line.trim();
    if (looksLikeChapter(trimmed)) {
      flush();
      heading = trimmed;
      buf = [];
    } else {
      buf.push(line);
    }
  }
  flush();
  // No chapter heading at all: the whole body is one chapter (possibly
  // empty). A *leading* empty preface is NOT emitted -- when the body
  // starts with a heading (`AGNES`), an empty `Alku` before it would put a
  // blank page first.
  if (out.length === 0) {
    out.push({ index: 0, heading: 'Alku', body: '' });
  }
  return out;
}

/**
 * Word budget for a single reader page.
 *
 * A chapter of a novel can run several thousand words, and the reader
 * spell-checks whatever is in the editor; the cost scales with the
 * distinct first-pass hits. Splitting a page at ~2000 words caps one
 * render's dispatch while keeping pages long enough that chapter
 * navigation stays usable.
 */
export const PAGE_WORD_BUDGET = 2000;

/** A run of text plus the separator that followed it in the source. */
interface Separated {
  text: string;
  sep: string;
}

/**
 * Split `text` on `re` while keeping the separator. `re` must contain
 * exactly one capture group around the separator; `String.split`
 * interleaves the captured separators with the pieces, and we walk the
 * even (text) / odd (separator) slots. None of the callers' patterns can
 * match the empty string, so the alignment is stable.
 */
function chunkBy(text: string, re: RegExp): Separated[] {
  const parts = text.split(re);
  const out: Separated[] = [];
  for (let i = 0; i < parts.length; i += 2) {
    out.push({ text: parts[i], sep: parts[i + 1] ?? '' });
  }
  return out;
}

/**
 * Greedily pack separated chunks into pages of at most `budget` words.
 * Packing is lossless: `pages.join('')` equals the chunks joined.
 */
function packPages(chunks: Separated[], budget: number): string[] {
  const pages: string[] = [];
  let cur = '';
  let curWords = 0;
  for (const c of chunks) {
    const w = countWords(c.text);
    if (curWords > 0 && curWords + w > budget) {
      pages.push(cur);
      cur = c.text + c.sep;
      curWords = w;
    } else {
      cur += c.text + c.sep;
      curWords += w;
    }
  }
  if (cur.length > 0) pages.push(cur);
  return pages;
}

/** Separator levels, coarsest first. */
const PAGE_SEPARATORS: RegExp[] = [
  /(\n{2,})/, // paragraph break
  /([.!?…]+\s*)/, // sentence end
  /(\n)/, // line break (verse / hard-wrapped text)
];

/**
 * Split `text` into pages of at most `budget` words. Tries the coarsest
 * separator first and only descends to a finer one for the pieces that are
 * still too large, so prose breaks between paragraphs and verse between
 * lines.
 */
function paginateBody(text: string, budget: number, level = 0): string[] {
  if (countWords(text) <= budget) return [text];
  if (level >= PAGE_SEPARATORS.length) return hardCut(text, budget);
  const chunks = chunkBy(text, PAGE_SEPARATORS[level]);
  if (chunks.length <= 1) {
    return paginateBody(text, budget, level + 1);
  }
  const out: string[] = [];
  for (const page of packPages(chunks, budget)) {
    out.push(...paginateBody(page, budget, level + 1));
  }
  return out;
}

/**
 * Last-resort split: cut on whitespace into `budget`-word pieces. Only
 * reached by a single run longer than the whole budget with no paragraph,
 * sentence or line break to cut on. Whitespace between words is preserved.
 */
function hardCut(text: string, budget: number): string[] {
  const parts = text.split(/(\s+)/);
  const pages: string[] = [];
  let cur = '';
  let curWords = 0;
  for (let i = 0; i < parts.length; i += 2) {
    if (curWords >= budget) {
      pages.push(cur);
      cur = '';
      curWords = 0;
    }
    cur += parts[i] + (parts[i + 1] ?? '');
    if (parts[i].length > 0) curWords++;
  }
  if (cur.length > 0) pages.push(cur);
  return pages;
}

/**
 * Split one chapter into reader-sized pages. A chapter whose body already
 * fits `budget` is returned unchanged, so the common short chapter costs a
 * single word count. Splitting is purely structural -- the page bodies
 * concatenate back to the original body -- and the heading carries the
 * part number so the chapter dropdown stays readable ("LUKU I — osa 2/5").
 *
 * Callers must re-index the result: every page of a split chapter keeps
 * the source chapter's `index`; `loadBook` renumbers the flattened list.
 */
export function paginateChapter(
  chapter: Chapter,
  budget: number = PAGE_WORD_BUDGET,
): Chapter[] {
  if (budget <= 0 || countWords(chapter.body) <= budget) return [chapter];
  const bodies = paginateBody(chapter.body, budget);
  if (bodies.length <= 1) return [chapter];
  return bodies.map((body, i) => ({
    index: chapter.index,
    heading: `${chapter.heading} — osa ${i + 1}/${bodies.length}`,
    body,
  }));
}
