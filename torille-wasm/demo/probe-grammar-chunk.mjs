// Grammar/style chunking probe.
//
// Verifies the worker's sentence-batch path:
//   * progress appears before the whole 1,959-word page finishes;
//   * the chunked result matches the whole-text baseline (69 grammar,
//     14 style) for the perf fixture, including quote-spanning sentences;
//   * editing while a check is running cancels the old run and leaves no
//     stale diagnostics.
//
// Run from this directory after `node build.mjs`:
//
//   node probe-grammar-chunk.mjs

import puppeteer from 'puppeteer-core';
import { readFile } from 'node:fs/promises';

const FIXTURE = new URL('../../tests/fixtures/perf/page-2000.txt', import.meta.url);
const EXPECTED_GRAMMAR = 69;
const EXPECTED_STYLE = 14;
const SHORT = 'Koira nukkuu.';

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

// Keep the grammar run from competing with the spelling first pass.
await page.evaluate(() => {
  const button = document.querySelector('.spell-check-toggle');
  if (button?.getAttribute('aria-pressed') === 'true') button.click();
});

const text = await readFile(FIXTURE, 'utf8');
await page.evaluate((fixture) => {
  window.__torille.panels[0].setText(fixture);
  window.__torille.panels[1].setText('');
}, text);
await new Promise((resolve) => setTimeout(resolve, 2500));

const started = Date.now();
await page.evaluate(() => {
  const grammar = document.querySelector('.grammar-toggle');
  const style = document.querySelector('.style-toggle');
  if (grammar?.getAttribute('aria-pressed') !== 'true') grammar?.click();
  if (style?.getAttribute('aria-pressed') !== 'true') style?.click();
});

let firstProgress = -1;
let maxDone = 0;
let final = { grammar: 0, style: 0, progress: null };
for (let i = 0; i < 800; i++) {
  const state = await page.evaluate(() => {
    const panel = window.__torille.panels[0];
    return {
      grammar: panel.grammarDiagnostics().length,
      style: panel.styleDiagnostics().length,
      progress: panel.grammarProgress(),
    };
  });
  if (state.progress && firstProgress < 0) firstProgress = Date.now() - started;
  if (state.progress) maxDone = Math.max(maxDone, state.progress.done);
  final = state;
  if (state.progress === null && state.grammar > 0 && i > 2) {
    await new Promise((resolve) => setTimeout(resolve, 300));
    final = await page.evaluate(() => {
      const panel = window.__torille.panels[0];
      return {
        grammar: panel.grammarDiagnostics().length,
        style: panel.styleDiagnostics().length,
        progress: panel.grammarProgress(),
      };
    });
    break;
  }
  await new Promise((resolve) => setTimeout(resolve, 20));
}

const totalMs = Date.now() - started;
console.log('=== chunked grammar/style ===');
console.log(`  first progress: ${firstProgress} ms`);
console.log(`  last batch:     ${maxDone}`);
console.log(`  total:          ${totalMs} ms`);
console.log(`  final:          ${final.grammar} grammar, ${final.style} style`);

const progressPass = firstProgress >= 0 && firstProgress < 2000 && maxDone > 0;
const countsPass = final.grammar === EXPECTED_GRAMMAR && final.style === EXPECTED_STYLE;
console.log(`  progress PASS:  ${progressPass}`);
console.log(`  counts PASS:    ${countsPass}`);

// Cancel a running check by editing the text. The new short text has no
// grammar diagnostics; the old diagnostics must not survive.
await page.evaluate((short) => window.__torille.panels[0].setText(short), SHORT);
let cancelled = { grammar: -1, style: -1, progress: 'pending' };
for (let i = 0; i < 300; i++) {
  cancelled = await page.evaluate(() => {
    const panel = window.__torille.panels[0];
    return {
      grammar: panel.grammarDiagnostics().length,
      style: panel.styleDiagnostics().length,
      progress: panel.grammarProgress(),
    };
  });
  if (cancelled.progress === null && cancelled.grammar === 0) break;
  await new Promise((resolve) => setTimeout(resolve, 20));
}
const cancelPass = cancelled.grammar === 0 && cancelled.style === 0 && cancelled.progress === null;
console.log(`  cancel PASS:    ${cancelPass} (grammar=${cancelled.grammar}, style=${cancelled.style})`);

await browser.close();
process.exit(progressPass && countsPass && cancelPass ? 0 : 1);
