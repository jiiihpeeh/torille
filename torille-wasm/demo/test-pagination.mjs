// Regression test for the Gutenberg reader's chapter pagination.
//
// The demo has no JS test runner (the other `*.mjs` files are
// puppeteer probes against a live page), so this test bundles
// `src/gutenberg.ts` with the esbuild the demo already depends on
// and imports the result as a data: URL -- the same mechanism
// `build.mjs` uses, minus the browser.
//
// Run:  node test-pagination.mjs   (or `npm test`)
//
// What it pins:
//   * a chapter within the budget is returned untouched;
//   * a longer chapter is split at paragraph / sentence / line
//     boundaries, and every page stays within the word budget
//     (except where a single unbreakable run makes that
//     impossible, which none of the fixtures do);
//   * splitting is lossless: the page bodies concatenate back to
//     the original body, so no character is dropped or duplicated.

import * as esbuild from 'esbuild';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

const bundle = await esbuild.build({
  entryPoints: [join(__dirname, 'src/gutenberg.ts')],
  bundle: true,
  write: false,
  format: 'esm',
  platform: 'node',
  target: 'es2022',
});
const source = bundle.outputFiles[0].text;
const dataUrl =
  'data:text/javascript;base64,' + Buffer.from(source).toString('base64');
const { filterCatalog, paginateChapter, splitChapters, PAGE_WORD_BUDGET } =
  await import(dataUrl);

let failures = 0;
const ok = (cond, msg) => {
  if (cond) console.log('ok   ', msg);
  else {
    failures++;
    console.log('FAIL ', msg);
  }
};

// Count words here rather than importing the bundler output's own
// copy, so the test would catch a tokeniser change that shifts the
// budget too.
const countWords = (text) => (text.match(/[\p{L}]+-?|[\p{L}]+/gu) ?? []).length;

const chapter = (body, heading = 'LUKU I') => ({ index: 0, heading, body });
const lossless = (pages, body) => pages.map((p) => p.body).join('') === body;
const bounded = (pages, budget) =>
  pages.every((p) => countWords(p.body) <= budget);

// 1. A chapter already within the budget is returned unchanged.
{
  const c = chapter('Kissa istuu matolla.');
  const pages = paginateChapter(c);
  ok(pages.length === 1 && pages[0] === c, 'short chapter unchanged');
}

// 2. Long prose: the ordinary case, split between paragraphs.
{
  const para = Array.from(
    { length: 120 },
    (_, i) => `Sana${i} juoksee metsään ja palaa kotiin illalla.`,
  ).join(' ');
  const body = Array.from({ length: 40 }, () => para).join('\n\n');
  const pages = paginateChapter(chapter(body));
  ok(pages.length > 1, `prose split into ${pages.length} pages`);
  ok(lossless(pages, body), 'prose pages concatenate to the original');
  ok(bounded(pages, PAGE_WORD_BUDGET), 'prose pages within budget');
  ok(
    pages.every(
      (p, i) => p.heading === `LUKU I — osa ${i + 1}/${pages.length}`,
    ),
    'prose pages numbered in the heading',
  );
}

// 3. Long prose with no blank lines: falls back to sentence breaks.
{
  const body = 'Tämä on pitkä lause joka jatkuu ja jatkuu. '.repeat(4000).trim();
  const pages = paginateChapter(chapter(body));
  ok(pages.length > 1, `sentence-split prose into ${pages.length} pages`);
  ok(lossless(pages, body), 'sentence-split prose lossless');
  ok(bounded(pages, PAGE_WORD_BUDGET), 'sentence-split prose within budget');
}

// 4. Verse / no punctuation: falls back to line breaks.
{
  const body = Array.from({ length: 3000 }, (_, i) => `rivi ${i} laulaa`).join(
    '\n',
  );
  const pages = paginateChapter(chapter(body));
  ok(pages.length > 1, `line-split text into ${pages.length} pages`);
  ok(lossless(pages, body), 'line-split text lossless');
  ok(bounded(pages, PAGE_WORD_BUDGET), 'line-split text within budget');
}

// 5. A wall of text with no separator at all: last-resort word cut.
{
  const body = Array.from({ length: 9000 }, (_, i) => `sana${i}`).join(' ');
  const pages = paginateChapter(chapter(body));
  ok(pages.length > 1, `wall of text into ${pages.length} pages`);
  ok(lossless(pages, body), 'wall of text lossless');
  ok(bounded(pages, PAGE_WORD_BUDGET), 'wall of text within budget');
}

// 6. An empty chapter is not a special case for the splitter.
{
  const pages = paginateChapter(chapter(''));
  ok(pages.length === 1 && pages[0].body === '', 'empty chapter unchanged');
}

// 7. A non-default budget is honoured.
{
  const body = Array.from(
    { length: 30 },
    () => 'yksi kaksi kolme neljä viisi.',
  ).join('\n\n');
  const pages = paginateChapter(chapter(body), 20);
  ok(lossless(pages, body), 'custom budget lossless');
  ok(bounded(pages, 20), 'custom budget honoured');
}

// 8. A real book, when a local copy happens to be around. This is
//    the check that caught the 30k-word single-chapter case.
{
  const path = process.env.SYLVI_TXT ?? '/tmp/sylvi.txt';
  try {
    const raw = readFileSync(path, 'utf8');
    const chapters = splitChapters(raw);
    let pages = 0;
    let max = 0;
    for (const c of chapters) {
      const ps = paginateChapter(c);
      pages += ps.length;
      for (const p of ps) max = Math.max(max, countWords(p.body));
      if (!lossless(ps, c.body)) ok(false, `Sylvi ${c.heading} lossless`);
    }
    console.log(`info  Sylvi -> ${chapters.length} chapters, ${pages} pages, max ${max} words`);
    ok(max <= PAGE_WORD_BUDGET, 'real book pages within budget');
  } catch {
    console.log(`skip  no book at ${path}`);
  }
}

// 9. Catalogue search: case/diacritic folding and Gutenberg ids.
{
  const catalog = [
    { id: 7000, title: 'Kalevala', author: 'Elias Lönnrot' },
    { id: 10480, title: 'Kauppa-Lopo', author: 'Minna Canth' },
    { id: 7078, title: 'Kanteletar', author: 'Elias Lönnrot' },
  ];
  ok(filterCatalog(catalog, '').length === 3, 'empty query keeps all books');
  ok(
    filterCatalog(catalog, 'lonnrot').length === 2,
    'search folds ä/ö (Lönnrot)',
  );
  ok(filterCatalog(catalog, 'CANTH')[0]?.id === 10480, 'search is case-blind');
  ok(filterCatalog(catalog, '#10480')[0]?.id === 10480, 'search by #id');
  ok(filterCatalog(catalog, '10480')[0]?.id === 10480, 'search by id');
  ok(filterCatalog(catalog, 'kaleva')[0]?.id === 7000, 'search by title prefix');
}

console.log(failures === 0 ? '\nALL PASS' : `\n${failures} FAILURE(S)`);
process.exit(failures === 0 ? 0 : 1);
