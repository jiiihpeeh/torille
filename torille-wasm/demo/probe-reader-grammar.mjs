// Reader grammar/style probe.
//
// Loads the demo, waits for the Gutenberg reader's chapter, then sets both the
// free-text panel and the reader to the same 1,959-word fixture. With grammar
// and style on, the two panels must produce identical counts; the reader must
// expose progress and clear its diagnostics when the text is replaced.
//
// Run from this directory after `node build.mjs`:
//
//   node probe-reader-grammar.mjs

import puppeteer from 'puppeteer-core';
import { readFile } from 'node:fs/promises';

const FIXTURE = new URL('../../tests/fixtures/perf/page-2000.txt', import.meta.url);
const SHORT = 'Koira nukkuu.';
const EXPECTED_GRAMMAR = 69;
const EXPECTED_STYLE = 14;

const text = await readFile(FIXTURE, 'utf8');
const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);
page.on('pageerror', (e) => console.error('[pageerror]', e.message));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
await page.waitForFunction(
  () => Boolean(window.__torille?.ready) && window.__torille.panels.length >= 2,
  { timeout: 30000 },
);

// Let the reader load its default chapter before replacing the text; the
// chapter-sync effect would otherwise overwrite the fixture.
await page.waitForFunction(
  () => {
    const area = document.querySelector('.gutenberg-reader textarea');
    return area instanceof HTMLTextAreaElement && area.value.length > 200;
  },
  { timeout: 60000 },
);

// Isolate grammar/style from the spelling pass.
await page.evaluate(() => {
  const button = document.querySelector('.spell-check-toggle');
  if (button?.getAttribute('aria-pressed') === 'true') button.click();
});
await page.evaluate((fixture) => {
  window.__torille.panels[0].setText(fixture);
  window.__torille.panels[1].setText(fixture);
}, text);
await new Promise((resolve) => setTimeout(resolve, 500));

const started = Date.now();
await page.evaluate(() => {
  const grammar = document.querySelector('.grammar-toggle');
  const style = document.querySelector('.style-toggle');
  if (grammar?.getAttribute('aria-pressed') !== 'true') grammar?.click();
  if (style?.getAttribute('aria-pressed') !== 'true') style?.click();
});

let firstProgress = -1;
let final = [
  { grammar: 0, style: 0, progress: null },
  { grammar: 0, style: 0, progress: null },
];
for (let i = 0; i < 800; i++) {
  const state = await page.evaluate(() => window.__torille.panels.map((panel) => ({
    grammar: panel.grammarDiagnostics().length,
    style: panel.styleDiagnostics().length,
    progress: panel.grammarProgress(),
  })));
  if (state.some((s) => s.progress) && firstProgress < 0) {
    firstProgress = Date.now() - started;
  }
  final = state;
  if (state.every((s) => s.progress === null && s.grammar > 0) && i > 2) {
    await new Promise((resolve) => setTimeout(resolve, 300));
    final = await page.evaluate(() => window.__torille.panels.map((panel) => ({
      grammar: panel.grammarDiagnostics().length,
      style: panel.styleDiagnostics().length,
      progress: panel.grammarProgress(),
    })));
    break;
  }
  await new Promise((resolve) => setTimeout(resolve, 20));
}

const readerBatches = await page.evaluate(
  () => window.__torille.panels[1].grammarBatchTimes(),
);
const spans = await page.evaluate(() => ({
  grammar: document.querySelectorAll(
    '.gutenberg-reader .underlay [class*="grammar-"]').length,
  style: document.querySelectorAll(
    '.gutenberg-reader .underlay .style-suggestion').length,
}));

const areas = await page.evaluate(() => [...document.querySelectorAll('textarea')].map((a) => ({ len: a.value.length, head: a.value.slice(0, 24) })));
console.log('textareas', areas);
console.log('=== reader grammar/style ===');
console.log(`  first progress: ${firstProgress} ms`);
console.log(`  free text:      ${final[0].grammar} grammar, ${final[0].style} style`);
console.log(`  reader:         ${final[1].grammar} grammar, ${final[1].style} style`);
console.log(`  reader batches: ${readerBatches.length} (p90 ${readerBatches.length
  ? [...readerBatches].sort((a, b) => a - b)[Math.floor(readerBatches.length * 0.9)].toFixed(0)
  : 0} ms)`);
console.log(`  reader spans:   ${spans.grammar} grammar, ${spans.style} style`);

const countsPass = final[0].grammar === EXPECTED_GRAMMAR &&
  final[0].style === EXPECTED_STYLE &&
  final[1].grammar === EXPECTED_GRAMMAR &&
  final[1].style === EXPECTED_STYLE;
const progressPass = firstProgress >= 0 && readerBatches.length > 0;
const spansPass = spans.grammar > 0 && spans.style > 0;
console.log(`  counts PASS:    ${countsPass}`);
console.log(`  progress PASS:  ${progressPass}`);
console.log(`  spans PASS:     ${spansPass}`);

// Replace the reader text while a fresh check is starting. The old
// diagnostics must not survive the new short text.
await page.evaluate((short) => window.__torille.panels[1].setText(short), SHORT);
let cancelled = { grammar: -1, style: -1, progress: 'pending' };
for (let i = 0; i < 300; i++) {
  cancelled = await page.evaluate(() => {
    const panel = window.__torille.panels[1];
    return {
      grammar: panel.grammarDiagnostics().length,
      style: panel.styleDiagnostics().length,
      progress: panel.grammarProgress(),
    };
  });
  if (cancelled.progress === null && cancelled.grammar === 0) break;
  await new Promise((resolve) => setTimeout(resolve, 20));
}
const cancelPass = cancelled.grammar === 0 && cancelled.style === 0 &&
  cancelled.progress === null;
console.log(`  cancel PASS:    ${cancelPass} (grammar=${cancelled.grammar}, style=${cancelled.style})`);

await browser.close();
process.exit(countsPass && progressPass && spansPass && cancelPass ? 0 : 1);
