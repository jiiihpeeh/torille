// Measure the time-to-first-underline for a freshly-typed
// misspelling. The slow `bg.check` used to gate the underline
// (50-1100 ms per word, in parallel across the 3-worker pool
// but still up to ~1-2 s for the slowest word). The new
// "candidate first, demote later" flow should show the
// underline in single-digit ms.
//
// Run from this directory after `node build.mjs`:
//
//   node probe-timing.mjs

import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);
page.on('pageerror', (e) => console.error('[pageerror]', e));

const pageLogs = [];
page.on('console', (m) => pageLogs.push(`[${m.type()}] ${m.text()}`));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
await page.waitForSelector('.editor textarea', { timeout: 30000 });

// Wait for the first render to settle, so the pool is warm.
await new Promise((r) => setTimeout(r, 6000));

const SET_TEXT_JS = (text) => `
  (() => {
    const ta = document.querySelector('.editor textarea');
    const proto = Object.getPrototypeOf(ta);
    const setter = Object.getOwnPropertyDescriptor(proto, 'value').set;
    setter.call(ta, ${JSON.stringify(text)});
    ta.dispatchEvent(new InputEvent('input', { bubbles: true }));
    ta.focus();
  })()
`;

// Measure the latency from "set text" to "first underline
// appears" for a fresh text with several misspellings. We
// poll the underlay every 5 ms.
const measureUnderlineLatency = async (text) => {
  pageLogs.length = 0;  // reset
  const t0 = Date.now();
  await page.evaluate(SET_TEXT_JS(text));
  // Poll the underlay.
  let firstUnderline = -1;
  for (let i = 0; i < 200; i++) {
    const count = await page.evaluate(() =>
      document.querySelector('.editor').querySelectorAll('.underlay .misspell').length,
    );
    if (count > 0 && firstUnderline < 0) {
      firstUnderline = Date.now() - t0;
      break;
    }
    await new Promise((r) => setTimeout(r, 5));
  }
  // Wait long enough for the slow check to settle so we can
  // also measure the final state.
  await new Promise((r) => setTimeout(r, 4000));
  const finalCount = await page.evaluate(() =>
    document.querySelector('.editor').querySelectorAll('.underlay .misspell').length,
  );
  const finalText = await page.evaluate(() => {
    const els = document.querySelector('.editor').querySelectorAll('.underlay .misspell');
    return Array.from(els).map((e) => e.textContent);
  });
  // Pull the relevant page-side log lines.
  const dispatchLog = pageLogs
    .filter((l) => l.includes('[findMisspellings] dispatch'))
    .slice(-1)[0] || '';
  const demoteLogs = pageLogs
    .filter((l) => l.includes('demote misspell') || l.includes('demote (slow'))
    .slice(-5);
  return { firstUnderline, finalCount, finalText, dispatchLog, demoteLogs };
};

// ── test 1: clean text + 4 fresh misspellings ─────────────────
const T1 = 'talo taloo koirra xyzzy asdfg ja kissa istuu talossa';
const r1 = await measureUnderlineLatency(T1);
console.log('=== test 1: fresh misspellings + inflected form ===');
console.log('  text:        ', JSON.stringify(T1));
console.log('  first underline (ms):', r1.firstUnderline);
console.log('  dispatch log:', r1.dispatchLog);
console.log('  final misspell count:', r1.finalCount);
console.log('  final words:  ', r1.finalText);
console.log('  demote log:');
for (const l of r1.demoteLogs) console.log('   ', l);
if (r1.firstUnderline < 200) {
  console.log('  *** PASS: underline appeared in <200 ms (was 1-2 s before) ***');
} else {
  console.log('  *** FAIL: underline took ' + r1.firstUnderline + ' ms ***');
  process.exit(1);
}
// Inflected form `istuu` should be demoted by the slow check.
if (r1.finalText.includes('istuu')) {
  console.log('  *** FAIL: `istuu` should be demoted (verb inflection) ***');
  process.exit(2);
}
if (r1.finalText.includes('talossa')) {
  console.log('  *** FAIL: `talossa` should never have been flagged ***');
  process.exit(3);
}
console.log('  *** PASS: `istuu` demoted, `talossa` never flagged ***');

// ── test 2: just garbage, no inflections ──────────────────────
const T2 = 'qwerty asdfg plover munster zzzz xylyx';
const r2 = await measureUnderlineLatency(T2);
console.log('\n=== test 2: pure garbage ===');
console.log('  text:        ', JSON.stringify(T2));
console.log('  first underline (ms):', r2.firstUnderline);
console.log('  final misspell count:', r2.finalCount);
console.log('  final words:  ', r2.finalText);
if (r2.firstUnderline < 200) {
  console.log('  *** PASS: underline in <200 ms ***');
} else {
  console.log('  *** FAIL: took ' + r2.firstUnderline + ' ms ***');
  process.exit(4);
}
if (r2.finalCount === 6) {
  console.log('  *** PASS: all 6 garbage words still flagged after slow check ***');
} else {
  console.log('  *** FAIL: expected 6 final, got ' + r2.finalCount + ' ***');
  process.exit(5);
}

await browser.close();
