// Browser-level e2e for the real content script + panel + background pool.
//
// Loads the compiled background and content script in one page, routes
// `runtime.sendMessage` to the real background, and routes the background's
// `tabs.sendMessage` to the real content listeners. Verifies the context-menu
// grammar and style commands, progress, the 4-worker pool, and cancellation
// on an edit.
//
// Run from this directory after `npm run build`:
//
//   node e2e/pool-content.mjs

import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import puppeteer from 'puppeteer-core';

const PORT = 8769;
const PAGE = `http://127.0.0.1:${PORT}/e2e/pool-content.html`;
const ROOT = new URL('..', import.meta.url).pathname;
const FIXTURE = new URL('../../../tests/fixtures/perf/page-2000.txt', import.meta.url);
const EXPECTED_GRAMMAR = 68;
const EXPECTED_STYLE = 14;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function waitFor(page, predicate, label, timeout = 30000) {
  const started = Date.now();
  while (Date.now() - started < timeout) {
    if (await page.evaluate(predicate)) return;
    await sleep(50);
  }
  throw new Error(`timed out waiting for ${label}`);
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
  await waitFor(page, () => window.__e2e?.ready === true, 'background ready');
  await waitFor(
    page,
    () => {
      const host = document.querySelector('.torille-overlay-host');
      const overlay = host?.shadowRoot?.querySelector('.overlay');
      return Boolean(host && overlay);
    },
    'content overlay',
  );

  const text = await readFile(FIXTURE, 'utf8');

  async function setText(value) {
    await page.evaluate((next) => {
      const textarea = document.querySelector('#t');
      textarea.value = next;
      textarea.dispatchEvent(new Event('input', { bubbles: true }));
    }, value);
    await waitFor(
      page,
      () => window.__e2e.overlayText() === document.querySelector('#t').value,
      'overlay text',
    );
  }

  async function contextMenu(menuItemId) {
    await page.evaluate(() => {
      const textarea = document.querySelector('#t');
      const rect = textarea.getBoundingClientRect();
      textarea.dispatchEvent(new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: rect.left + 20,
        clientY: rect.top + 20,
      }));
    });
    await page.evaluate((item) => window.__e2e.contextMenu(item), menuItemId);
  }

  await setText(text);
  assert(await page.evaluate(() => window.__e2e.morphologyWorkerCount()) === 0,
    'a morphology worker was created before the grammar check');

  await contextMenu('torille-check-grammar');
  let progressText = null;
  let grammarPanel = 0;
  let grammarUnderlines = 0;
  for (let attempt = 0; attempt < 1200; attempt++) {
    const state = await page.evaluate(() => ({
      toast: window.__e2e.toastText(),
      panel: window.__e2e.panelCount(),
      underlines: window.__e2e.grammarUnderlines(),
    }));
    if (state.toast && state.toast.includes('/')) progressText = state.toast;
    grammarPanel = state.panel;
    grammarUnderlines = state.underlines;
    if (grammarPanel === EXPECTED_GRAMMAR && grammarUnderlines > 0) break;
    await sleep(20);
  }
  const grammarWorkers = await page.evaluate(() =>
    window.__e2e.morphologyWorkerCount());
  console.log(
    `grammar: panel ${grammarPanel}, underlines ${grammarUnderlines}, ` +
    `progress ${JSON.stringify(progressText)}, workers ${grammarWorkers}`,
  );
  assert(grammarPanel === EXPECTED_GRAMMAR,
    `grammar panel has ${grammarPanel} rows, expected ${EXPECTED_GRAMMAR}`);
  assert(grammarUnderlines > 0, 'no grammar underlines were painted');
  assert(progressText !== null, 'no group progress reached the content toast');
  assert(grammarWorkers === 4,
    `expected 4 morphology workers, got ${grammarWorkers}`);

  await contextMenu('torille-check-style');
  let stylePanel = 0;
  let styleUnderlines = 0;
  for (let attempt = 0; attempt < 1200; attempt++) {
    const state = await page.evaluate(() => ({
      panel: window.__e2e.panelCount(),
      underlines: window.__e2e.styleUnderlines(),
    }));
    stylePanel = state.panel;
    styleUnderlines = state.underlines;
    if (stylePanel === EXPECTED_STYLE && styleUnderlines > 0) break;
    await sleep(20);
  }
  console.log(
    `style: panel ${stylePanel}, underlines ${styleUnderlines}`,
  );
  assert(stylePanel === EXPECTED_STYLE,
    `style panel has ${stylePanel} rows, expected ${EXPECTED_STYLE}`);
  assert(styleUnderlines > 0, 'no style underlines were painted');

  // Cancellation: start the long grammar check, then edit the field. The
  // panel and underlines must clear and the old result must not land.
  await setText(text);
  await contextMenu('torille-check-grammar');
  let sawProgress = false;
  for (let attempt = 0; attempt < 400; attempt++) {
    const toast = await page.evaluate(() => window.__e2e.toastText());
    if (toast) { sawProgress = true; break; }
    await sleep(20);
  }
  assert(sawProgress, 'the long check never showed its progress toast');

  await setText('Koira nukkuu.');
  for (let attempt = 0; attempt < 600; attempt++) {
    const state = await page.evaluate(() => ({
      toast: window.__e2e.toastText(),
      panel: window.__e2e.panelPresent(),
      underlines: window.__e2e.grammarUnderlines(),
      text: document.querySelector('#t').value,
      overlay: window.__e2e.overlayText(),
    }));
    if (state.toast === null && !state.panel && state.underlines === 0) {
      console.log(`cancel: panel cleared, text ${JSON.stringify(state.text)}`);
      assert(state.text === 'Koira nukkuu.', 'the textarea text changed');
      assert(state.overlay === 'Koira nukkuu.', 'the overlay text changed');
      console.log('content-panel pool e2e PASS');
      break;
    }
    if (attempt === 599) {
      throw new Error(
        `cancel did not clear: ${JSON.stringify(state)}`,
      );
    }
    await sleep(20);
  }
} finally {
  if (browser) await browser.close();
  server.kill('SIGTERM');
}
