// E2E: code blocks, inline code, BBCode and rich-editor <pre><code> must
// never receive an underline, while the prose around them still does.
//
// Serve extensions/chromium/ on :8767 first:
//   python3 -m http.server 8767
//   node e2e/code-blocks.mjs

import puppeteer from 'puppeteer-core';

const URL = 'http://localhost:8767/e2e/code-blocks.html';
const CODE_WORDS = ['koirra', 'xyzzy'];

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
  await page.goto(URL, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.__e2e?.ready === true, { timeout: 5000 });
  await page.waitForFunction(() => {
    const ta = document.querySelector('#t');
    const overlay = document.querySelector('.torille-overlay-host')
      ?.shadowRoot?.querySelector('.overlay');
    return ta && overlay && overlay.textContent === ta.value
      && overlay.querySelectorAll('.misspell').length > 0;
  }, { timeout: 15000 });

  const state = await page.evaluate(() => {
    const overlay = document.querySelector('.torille-overlay-host')
      ?.shadowRoot?.querySelector('.overlay');
    const spans = overlay ? Array.from(overlay.querySelectorAll('.misspell')) : [];
    const hl = window.CSS?.highlights?.get('torille-misspell');
    return {
      textareaMisspells: spans.map((s) => s.textContent),
      overlayText: overlay ? overlay.textContent : null,
      ceText: document.querySelector('#ce')?.textContent ?? null,
      ceMisspells: hl ? Array.from(hl).map((r) => r.toString()) : [],
    };
  });
  console.log(JSON.stringify(state, null, 2));

  assert(state.textareaMisspells.includes('taloo'),
    'prose misspelling taloo is missing from the textarea overlay');
  for (const word of CODE_WORDS) {
    assert(!state.textareaMisspells.includes(word),
      `code word ${word} was underlined in the textarea overlay`);
  }
  assert(!state.ceMisspells.includes('koirra'),
    'code word koirra was highlighted in the contenteditable');
  assert(!state.ceMisspells.includes('xyzzy'),
    'code word xyzzy was highlighted in the contenteditable');
  assert(state.ceMisspells.some((s) => s.includes('taloo')),
    'prose misspelling taloo is missing from the contenteditable highlight');

  // Turning the code-block setting off must restore checking inside the
  // fenced block.
  await page.evaluate(async () => {
    await window.browser.storage.local.set({ 'torille-ignore-code-blocks': false });
  });
  await page.waitForFunction(() => {
    const overlay = document.querySelector('.torille-overlay-host')
      ?.shadowRoot?.querySelector('.overlay');
    return overlay && Array.from(overlay.querySelectorAll('.misspell'))
      .some((s) => s.textContent === 'koirra');
  }, { timeout: 15000 });
  console.log('toggle off: fenced code is checked again');
} finally {
  await browser.close();
}
