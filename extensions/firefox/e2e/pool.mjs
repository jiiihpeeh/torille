// Browser-level e2e for the Firefox morphology pool.
//
// Loads the real compiled background script in a page and stubs only the
// extension APIs. The spell worker, the morphology workers and the wasm are
// the built artefacts under dist/. Verifies:
//
//   * the 4-worker pool is created and the full page returns 68/14;
//   * progress arrives before the result and the group count is reported;
//   * grammarThreads=2 tears the 4-worker pool down and creates 2;
//   * grammarThreads=1 skips the pool and uses the sentence-chunked path;
//   * a newer check for the same input cancels the running pool check.
//
// Run from this directory after `npm run build`:
//
//   node e2e/pool.mjs

import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import puppeteer from 'puppeteer-core';

const PORT = 8768;
const PAGE = `http://127.0.0.1:${PORT}/e2e/pool.html`;
const ROOT = new URL('..', import.meta.url).pathname;
const FIXTURE = new URL('../../../tests/fixtures/perf/page-2000.txt', import.meta.url);
// The pool path goes through `SentenceCache::check_with_decisions`, which on
// this page reports 9 grammar findings where the whole-document
// `syntax_check::check` reports 5. That divergence is tracked in
// misc_md/TODO_GRAMMAR_STYLE.md, not papered over here: this constant pins
// the path the extension actually ships.
const EXPECTED_GRAMMAR = 9;
const EXPECTED_STYLE = 14;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const server = spawn(
  'python3',
  ['-m', 'http.server', String(PORT), '--directory', ROOT],
  { stdio: 'ignore' },
);

let browser;
try {
  for (let attempt = 0; attempt < 100; attempt++) {
    try {
      const response = await fetch(PAGE);
      if (response.ok) break;
    } catch {
      // server still starting
    }
    await sleep(100);
  }

  browser = await puppeteer.launch({
    executablePath: '/usr/bin/google-chrome',
    headless: 'new',
    args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
    protocolTimeout: 600000,
  });
  const page = await browser.newPage();
  page.setDefaultTimeout(600000);
  page.on('pageerror', (error) => console.error('[pageerror]', error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') console.error('[console]', message.text());
  });

  await page.goto(PAGE, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__e2e?.ready === true, { timeout: 30000 });

  const created = await page.evaluate(() =>
    window.__e2e.send({ type: 'torille:create-state' }));
  assert(created && typeof created.idx === 'number',
    'create-state did not return an idx');
  const idx = created.idx;

  const text = await readFile(FIXTURE, 'utf8');

  async function check(label, threads) {
    await page.evaluate(() => window.__e2e.clearProgress());
    await page.evaluate((value) => window.__e2e.setThreads(value), threads);
    const before = await page.evaluate(() =>
      window.__e2e.morphologyWorkerCount());
    const started = Date.now();
    const pending = page.evaluate(
      ({ idx, text }) => window.__e2e.send({
        type: 'torille:check-grammar-style',
        inputId: 1,
        idx,
        text,
        syntax: true,
        style: true,
      }),
      { idx, text },
    );
    let firstProgress = -1;
    for (let attempt = 0; attempt < 2000; attempt++) {
      const state = await page.evaluate(() => ({
        count: window.__e2e.progress.length,
        last: window.__e2e.progress.at(-1) ?? null,
      }));
      if (state.count > 0 && firstProgress < 0) firstProgress = Date.now() - started;
      if (state.last && state.last.done === state.last.total) break;
      await sleep(20);
    }
    const result = await pending;
    const ms = Date.now() - started;
    const after = await page.evaluate(() =>
      window.__e2e.morphologyWorkerCount());
    const progress = await page.evaluate(() => window.__e2e.progress);
    console.log(
      `${label}: ${ms} ms, first progress ${firstProgress} ms, ` +
      `${result.diagnostics.length} grammar / ${result.style.length} style, ` +
      `workers ${before}->${after}, progress ${progress.length} ` +
      `(total ${progress.at(-1)?.total ?? 0})`,
    );
    return { result, ms, firstProgress, before, after, progress };
  }

  const four = await check('4 workers', 4);
  assert(four.result.diagnostics.length === EXPECTED_GRAMMAR,
    `4-worker grammar count ${four.result.diagnostics.length}`);
  assert(four.result.style.length === EXPECTED_STYLE,
    `4-worker style count ${four.result.style.length}`);
  assert(four.after - four.before === 4,
    `expected 4 morphology workers, got ${four.after - four.before}`);
  assert(four.firstProgress >= 0, 'no progress before the 4-worker result');
  assert(four.progress.length > 0 && four.progress.at(-1).total > 1,
    'the 4-worker check did not report group progress');

  const two = await check('2 workers', 2);
  assert(two.result.diagnostics.length === EXPECTED_GRAMMAR,
    `2-worker grammar count ${two.result.diagnostics.length}`);
  assert(two.result.style.length === EXPECTED_STYLE,
    `2-worker style count ${two.result.style.length}`);
  assert(two.after - two.before === 2,
    `expected 2 new morphology workers, got ${two.after - two.before}`);

  const one = await check('1 worker (chunked)', 1);
  assert(one.result.diagnostics.length === EXPECTED_GRAMMAR,
    `chunked grammar count ${one.result.diagnostics.length}`);
  assert(one.result.style.length === EXPECTED_STYLE,
    `chunked style count ${one.result.style.length}`);
  assert(one.after === one.before,
    'grammarThreads=1 created a morphology worker');

  // Cancellation: start a long check, then send a short one for the same
  // input. The long run must return empty and the short run 0/0.
  await page.evaluate(() => {
    window.__e2e.clearProgress();
    window.__e2e.setThreads(4);
  });
  const longPending = page.evaluate(
    ({ idx, text }) => window.__e2e.send({
      type: 'torille:check-grammar-style',
      inputId: 1,
      idx,
      text,
      syntax: true,
      style: true,
    }),
    { idx, text },
  );
  for (let attempt = 0; attempt < 100; attempt++) {
    const count = await page.evaluate(() => window.__e2e.progress.length);
    if (count > 0) break;
    await sleep(20);
  }
  const shortResult = await page.evaluate(
    ({ idx }) => window.__e2e.send({
      type: 'torille:check-grammar-style',
      inputId: 1,
      idx,
      text: 'Koira nukkuu.',
      syntax: true,
      style: true,
    }),
    { idx },
  );
  const longResult = await longPending;
  console.log(
    `cancel: long ${longResult.diagnostics.length} grammar / ` +
    `${longResult.style.length} style, short ${shortResult.diagnostics.length} ` +
    `grammar / ${shortResult.style.length} style`,
  );
  assert(longResult.diagnostics.length === 0 && longResult.style.length === 0,
    'the cancelled long check still returned diagnostics');
  assert(shortResult.diagnostics.length === 0 && shortResult.style.length === 0,
    'the short check returned diagnostics');

  console.log('pool e2e PASS');
} finally {
  if (browser) await browser.close();
  server.kill('SIGTERM');
}
