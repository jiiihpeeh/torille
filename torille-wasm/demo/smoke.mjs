// Puppeteer-driven smoke test for the torille-wasm demo.
//
// Captures all console messages (page + workers), reports the DOM
// state at fixed checkpoints, and exercises a typing flow to confirm
// the spell-check feedback loop ends up redrawing the page.

import puppeteer from 'puppeteer-core';

const URL = 'http://localhost:8765/demo/';
const TIMEOUT = 30_000;

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
  defaultViewport: { width: 900, height: 600 },
});

try {
  const page = await browser.newPage();

  // Capture every console message -- page or worker, with the source.
  page.on('console', (msg) => {
    const loc = msg.location();
    console.log(`[console:${msg.type()}] ${msg.text()} (${loc.url}:${loc.lineNumber})`);
  });
  page.on('pageerror', (err) => console.error('[pageerror]', err.message));
  page.on('requestfailed', (req) =>
    console.error('[requestfailed]', req.url(), req.failure()?.errorText));

  console.log(`navigating to ${URL}`);
  await page.goto(URL, { waitUntil: 'domcontentloaded' });

  // Wait up to TIMEOUT for the editor textarea to appear.
  console.log('waiting for textarea…');
  await page.waitForSelector('textarea', { timeout: TIMEOUT });

  // Snapshot the editor text + misspell count.
  const snap1 = await page.evaluate(() => ({
    text: document.querySelector('textarea')?.value?.slice(0, 80),
    misspellCount: document.querySelectorAll('.misspell').length,
    misspellWords: Array.from(document.querySelectorAll('.misspell')).map(s => s.textContent),
    h1Texts: Array.from(document.querySelectorAll('h1 small')).map(s => s.textContent),
    loadSplash: document.getElementById('app')?.textContent?.includes('loading wasm'),
  }));
  console.log('after textarea mounted:', JSON.stringify(snap1));

  // Type something new to trigger a debounced re-check.
  console.log('typing into textarea…');
  await page.click('textarea');
  await page.keyboard.press('End');
  await page.keyboard.type('  qwxyz');
  await new Promise((r) => setTimeout(r, 1500));

  const snap2 = await page.evaluate(() => ({
    text: document.querySelector('textarea')?.value?.slice(-40),
    misspellCount: document.querySelectorAll('.misspell').length,
    misspellWords: Array.from(document.querySelectorAll('.misspell')).map(s => s.textContent),
    h1Texts: Array.from(document.querySelectorAll('h1 small')).map(s => s.textContent),
  }));
  console.log('after typing:', JSON.stringify(snap2));

  // Click the first misspelled word; the SuggestionsPopup should appear.
  console.log('clicking the first misspelled word…');
  const clicked = await page.evaluate(() => {
    const span = document.querySelector('.editor .underlay .misspell');
    if (!span) return null;
    const r = span.getBoundingClientRect();
    return { word: span.textContent, x: r.left + r.width / 2, y: r.top + r.height / 2 };
  });
  console.log('  target:', JSON.stringify(clicked));
  if (clicked) {
    await page.mouse.move(clicked.x, clicked.y);
    await page.mouse.down();
    await page.mouse.up();
    // give suggestAsync (worker round-trip) + rAF time to land.
    await new Promise((r) => setTimeout(r, 600));
  }

  const snap3 = await page.evaluate(() => ({
    popupVisible: !!document.querySelector('.suggestions'),
    popupButtons: Array.from(document.querySelectorAll('.suggestions button')).map(b => b.textContent),
    popupEmpty: !!document.querySelector('.suggestions .empty'),
  }));
  console.log('after click:', JSON.stringify(snap3));

  await page.screenshot({ path: '/tmp/opencode/demo.png', fullPage: true });
  console.log('screenshot saved');
} finally {
  await browser.close();
}