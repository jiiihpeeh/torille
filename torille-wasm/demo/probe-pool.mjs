// Verify the suggestion pool is firing in background by
// capturing all [torille-sug] / [torille-demo] console lines
// and checking for wasm:suggest-for-misspell / wasm:check-known
// log entries fired BEFORE any click happens.
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);

const consoleLines = [];
page.on('console', (m) => {
  const t = m.text();
  if (t.includes('torille-sug') || t.includes('wasm:suggest-for-misspell') || t.includes('wasm:check-known') || t.includes('in:recv') || t.includes('boot:ready')) {
    console.log('[browser]', t);
  }
});
page.on('pageerror', (e) => console.log('[pageerror]', e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });

// Wait 8 seconds -- the demo boots, debounces 150ms, fires the
// first-pass analyze-misspells, fans out bg.check + suggest to
// the pool, and settles. Capture everything that landed.
await new Promise((r) => setTimeout(r, 8000));

console.log('\n=== console captured above ===');
await browser.close();
