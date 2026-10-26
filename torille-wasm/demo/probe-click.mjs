// Click a confirmed misspell and confirm the popup has
// candidates pre-populated by the *background* dispatch
// (no fresh wasm call should be needed).
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
  if (t.includes('wasm:suggest-for-misspell') || t.includes('wasm:check-known')
      || t.includes('out:post') || t.includes('cache:suggest-for-misspell:hit')
      || t.includes('Pick:')) {
    console.log('[browser]', t);
  }
});
page.on('pageerror', (e) => console.log('[pageerror]', e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });

// Wait for the pool's bg.suggest_for-misspell to land.
await new Promise((r) => setTimeout(r, 3500));

const beforeClick = await page.evaluate(() => {
  // Inspect the popup's content if any.
  const popup = document.querySelector('.suggestions');
  const popupItems = popup?.querySelectorAll('button') ?? [];
  const misspellSpans = document.querySelectorAll('.editor .underlay .misspell');
  const misspellTexts = Array.from(misspellSpans).map((s) => s.textContent);
  return {
    visibleMisspells: misspellTexts,
    popupItemCount: popupItems.length,
    popupText: popup?.textContent ?? null,
  };
});
console.log('\n=== STATE BEFORE CLICK ===');
console.log(JSON.stringify(beforeClick, null, 2));

// Click the first misspell directly. Editor listens on `mousedown`,
// not `click`, so dispatch the right event.
const clickResult = await page.evaluate(() => {
  const span = document.querySelector('.editor .underlay .misspell');
  if (!span) return { ok: false, reason: 'no misspell' };
  const t = span.textContent ?? '';
  span.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
  return { ok: true, word: t };
});
console.log('mousedown dispatched:', clickResult);

// Wait a tick for the popup to render.
await new Promise((r) => setTimeout(r, 800));

const afterClick = await page.evaluate(() => {
  const popup = document.querySelector('.suggestions');
  const items = popup?.querySelectorAll('button') ?? [];
  const itemTexts = Array.from(items).map((b) => b.textContent);
  return {
    popupPresent: !!popup,
    itemCount: items.length,
    items: itemTexts,
    emptyShown: !!popup?.querySelector('.empty'),
  };
});
console.log('\n=== STATE AFTER CLICK ===');
console.log(JSON.stringify(afterClick, null, 2));

console.log('\n=== CONSOLE LOG ===');
for (const l of consoleLines) console.log(l);

await browser.close();
