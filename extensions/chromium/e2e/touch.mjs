// E2E: tapping a misspelling opens the suggestion dropdown, and tapping
// a suggestion replaces the word. Exercises both the textarea overlay
// and the contenteditable CSS Highlight path with a touch viewport.
//
// Serve extensions/chromium/ on :8767, then:
//   node e2e/touch.mjs

import puppeteer from 'puppeteer-core';

const PORT = process.env.TORILLE_PORT ?? '8767';
const URL = `http://localhost:${PORT}/e2e/test-page.html`;

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
});

function assert(cond, message) {
  if (!cond) throw new Error(message);
}

try {
  const page = await browser.newPage();
  page.on('pageerror', (err) => console.error('[pageerror]', err.message));
  await page.setViewport({
    width: 390, height: 844, isMobile: true, hasTouch: true, deviceScaleFactor: 2,
  });
  await page.goto(URL, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__e2e?.ready === true, { timeout: 5000 });

  // ── textarea overlay ──────────────────────────────────────────────
  await page.waitForFunction(() => {
    const host = document.querySelector('.torille-overlay-host');
    const overlay = host?.shadowRoot?.querySelector('.overlay');
    return overlay && overlay.textContent === document.querySelector('#t').value
      && overlay.querySelector('.misspell');
  }, { timeout: 15000 });

  const anchor = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const span = host.shadowRoot.querySelector('.misspell');
    const r = span.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2, word: span.textContent };
  });
  await page.touchscreen.tap(anchor.x, anchor.y);
  await new Promise((r) => setTimeout(r, 600));

  const popup = await page.evaluate(() => {
    const host = document.querySelector('.torille-overlay-host');
    const menu = host.shadowRoot.querySelector('.suggestions');
    const button = menu?.querySelector('button');
    const r = button?.getBoundingClientRect();
    return {
      visible: menu && getComputedStyle(menu).display !== 'none',
      count: menu ? menu.querySelectorAll('button').length : 0,
      button: r ? {
        x: r.x + r.width / 2, y: r.y + r.height / 2, text: button.textContent,
      } : null,
    };
  });
  assert(popup.visible, 'tap on a textarea misspelling did not open the popup');
  assert(popup.button, 'popup has no suggestion button');
  await page.touchscreen.tap(popup.button.x, popup.button.y);
  await new Promise((r) => setTimeout(r, 600));

  const textareaValue = await page.evaluate(() => document.querySelector('#t').value);
  assert(!textareaValue.includes(anchor.word),
    `tapping a suggestion did not replace ${anchor.word}: ${textareaValue}`);
  assert(textareaValue.includes(popup.button.text),
    `replacement ${popup.button.text} is missing: ${textareaValue}`);
  console.log('textarea tap: ok');

  // ── contenteditable CSS Highlight path ────────────────────────────
  await page.waitForFunction(() => {
    const hl = window.CSS?.highlights?.get('torille-misspell');
    return hl && Array.from(hl).length > 0;
  }, { timeout: 15000 });

  const ceAnchor = await page.evaluate(() => {
    const hl = window.CSS.highlights.get('torille-misspell');
    const range = Array.from(hl)[0];
    const r = range.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2, word: range.toString() };
  });
  await page.touchscreen.tap(ceAnchor.x, ceAnchor.y);
  await new Promise((r) => setTimeout(r, 600));

  const cePopup = await page.evaluate(() => {
    const host = document.querySelector('.torille-ce-host');
    const button = host?.querySelector('button');
    const r = button?.getBoundingClientRect();
    return {
      visible: host && host.style.display === 'block',
      button: r ? {
        x: r.x + r.width / 2, y: r.y + r.height / 2, text: button.textContent,
      } : null,
    };
  });
  assert(cePopup.visible, 'tap on a contenteditable misspelling did not open the popup');
  assert(cePopup.button, 'contenteditable popup has no suggestion button');
  await page.touchscreen.tap(cePopup.button.x, cePopup.button.y);
  await new Promise((r) => setTimeout(r, 600));

  const ceText = await page.evaluate(() => document.querySelector('#ce').textContent);
  assert(!ceText.includes(ceAnchor.word),
    `tapping a suggestion did not replace ${ceAnchor.word}: ${ceText}`);
  assert(ceText.includes(cePopup.button.text),
    `replacement ${cePopup.button.text} is missing: ${ceText}`);
  console.log('contenteditable tap: ok');
} finally {
  await browser.close();
}
