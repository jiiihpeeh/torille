// Verify the live-underline bug fix: type new text, check
// the new misspellings get confirmed by the wasm pool, and
// the inflection forms (talossa, juoksee) stay clean.
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);

const events = [];
page.on('console', (m) => events.push(`[page ${m.type()}] ${m.text()}`));
page.on('pageerror', (e) => events.push('[pageerror] ' + e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
await page.waitForSelector('.editor textarea', { timeout: 30000 });

// Wait for first render to settle.
await new Promise((r) => setTimeout(r, 4000));

const baselineMisspells = await page.$$eval(
  '.editor .underlay .misspell',
  (els) => els.map((e) => e.textContent),
);
console.log('=== BASELINE misspellings in underlay ===');
console.log(' ', baselineMisspells);

const baselineCounter = await page.$eval('h1 small', (e) => e.textContent);
console.log('=== BASELINE counter ===', baselineCounter);

// Now type a new sentence that should be mostly clean.
const ta = await page.$('.editor textarea');
await ta.click({ clickCount: 3 });  // select all
await page.keyboard.press('Backspace');
await ta.type('Tämä on uusi lause talossa. Kissa juoksee puistossa. Hän menee sinne huomenna.');

await new Promise((r) => setTimeout(r, 5000));

const afterTypeMisspells = await page.$$eval(
  '.editor .underlay .misspell',
  (els) => els.map((e) => e.textContent),
);
console.log('=== AFTER TYPE-1 (clean) misspellings ===');
console.log(' ', afterTypeMisspells);

const afterTypeCounter = await page.$eval('h1 small', (e) => e.textContent);
console.log('=== AFTER TYPE-1 counter ===', afterTypeCounter);

// Now type a sentence with deliberate misspellings.
await ta.click({ clickCount: 3 });
await page.keyboard.press('Backspace');
await ta.type('Minä mennee kauppaan. Koira laulaba puistossa. xyzzy taloo koirra.');

await new Promise((r) => setTimeout(r, 6000));

const afterType2Misspells = await page.$$eval(
  '.editor .underlay .misspell',
  (els) => els.map((e) => e.textContent),
);
console.log('=== AFTER TYPE-2 (intentional misspellings) misspellings ===');
console.log(' ', afterType2Misspells);

const afterType2Counter = await page.$eval('h1 small', (e) => e.textContent);
console.log('=== AFTER TYPE-2 counter ===', afterType2Counter);

console.log('\n=== Filtered page-side log (demo + findMisspellings) ===');
for (const l of events) {
  if (l.includes('[demo]') || l.includes('[findMisspellings]')) {
    console.log('  ' + l);
  }
}

await browser.close();
