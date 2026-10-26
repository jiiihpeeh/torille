// Capture every console event from page + workers, with the worker
// URL prepended so we can tell which worker posted which log.
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);

const lines = [];
const attachWorkerLogs = (worker, label) => {
  worker.on('console', (m) => {
    lines.push(`[worker:${label}] ${m.text()}`);
  });
};
page.on('workercreated', (w) => {
  // URL like 'http://127.0.0.1:8765/src/spell-worker.js'
  const u = String(w.url());
  const base = u.split('/').pop() || '?';
  attachWorkerLogs(w, base);
});
page.on('console', (m) => {
  lines.push(`[page] ${m.text()}`);
});
page.on('pageerror', (e) => lines.push('[pageerror] ' + e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
await page.waitForSelector('.editor textarea', { timeout: 30000 });

// Default demo text — wait long enough for first render to settle.
await new Promise((r) => setTimeout(r, 8000));

console.log('=== ALL LOG (full case) ===');
for (const l of lines) console.log('  ' + l);

await browser.close();