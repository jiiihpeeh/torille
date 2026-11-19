// Steady-state timings for the 4-worker morphology pool.
//
// `pool.mjs` measures the *first* check of a session, which pays the four
// worker spawns and their wasm instantiation. This probe warms the pool with
// a one-sentence check, then times the full page on a fresh textarea state;
// that is the cold-page cost a reader pays for each new article while the
// browser session is alive.
//
// Run from this directory after `npm run build`:
//
//   node e2e/pool-timing.mjs
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import puppeteer from 'puppeteer-core';

const PORT = 8769;
const PAGE = `http://127.0.0.1:${PORT}/e2e/pool.html`;
const ROOT = new URL('..', import.meta.url).pathname;
const FIXTURE = new URL('../../../tests/fixtures/perf/page-2000.txt', import.meta.url);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const server = spawn('python3',
  ['-m', 'http.server', String(PORT), '--directory', ROOT], { stdio: 'ignore' });

let browser;
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(PAGE)).ok) break; } catch {}
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
  await page.goto(PAGE, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__e2e?.ready === true, { timeout: 30000 });

  const { idx } = await page.evaluate(() =>
    window.__e2e.send({ type: 'torille:create-state' }));
  const text = await readFile(FIXTURE, 'utf8');
  await page.evaluate(() => window.__e2e.setThreads(4));

  async function run(label, value, state = idx) {
    await page.evaluate(() => window.__e2e.clearProgress());
    const started = Date.now();
    const result = await page.evaluate(({ idx, value }) => window.__e2e.send({
      type: 'torille:check-grammar-style',
      inputId: 1, idx, text: value, syntax: true, style: true,
    }), { idx: state, value });
    console.log(
      `${label}: ${Date.now() - started} ms, ` +
      `${result.diagnostics.length} grammar / ${result.style.length} style`);
    return result;
  }

  const fresh = async () => (await page.evaluate(() =>
    window.__e2e.send({ type: 'torille:create-state' }))).idx;
  await run('warm-up 1 sentence', 'Koira nukkuu.');
  await run('full page #1 (same state)', text);
  for (const index of [1, 2, 3]) {
    const state = await fresh();
    await run(`full page, fresh state #${index}`, text, state);
  }
} finally {
  if (browser) await browser.close();
  server.kill('SIGTERM');
}
