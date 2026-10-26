// Minimal browser smoke test for the Leptos demo.
// Loads the index, waits for the Leptos app to mount, then
// reads the misspell list and reports what it sees. We expect
// the same hard-coded misspellings the wasm-demo / SolidJS
// demo use: taloo, koirra, xyzzy (plus possibly puhtaan if
// the lemmatiser treats 'puhtaan' as known).

import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox'],
});
const page = await browser.newPage();
const consoleLines = [];
const pageErrors = [];
page.on('console', (m) => consoleLines.push(`[${m.type()}] ${m.text()}`));
page.on('pageerror', (e) => pageErrors.push(String(e)));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });

// Wait for the Leptos app to mount and run its effect.
await page.waitForFunction(
  () => {
    const misspell = document.querySelector('.misspell-list ul');
    return !!misspell;
  },
  { timeout: 30000 },
).catch(() => {});

const result = await page.evaluate(() => {
  const heading = document.querySelector('h1')?.textContent;
  const items = Array.from(document.querySelectorAll('.misspell-list li button'))
    .map((b) => b.textContent);
  return { heading, items };
});
console.log(JSON.stringify(result, null, 2));
console.log('\n=== CONSOLE ===');
for (const l of consoleLines) console.log(l);
console.log('\n=== PAGE ERRORS ===');
for (const e of pageErrors) console.log(e);
await browser.close();
