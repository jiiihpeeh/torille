// Verify the surgical-diff optimization in findMisspellings.
// The probe loads a baseline text with N misspellings, then
// types a single character at the end and checks that the
// next render reports (reused=N, toCheck=0). Then it adds a
// brand-new misspelling and checks (reused=N, toCheck=1).
//
// Run from this directory after `node build.mjs`:
//
//   node probe-surgical.mjs

import puppeteer from 'puppeteer-core';

const browser = await puppeteer.launch({
  executablePath: '/usr/bin/google-chrome',
  headless: 'new',
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
  protocolTimeout: 600000,
});
const page = await browser.newPage();
page.setDefaultTimeout(600000);

const pageLogs = [];
page.on('console', (m) => pageLogs.push(`[${m.type()}] ${m.text()}`));
page.on('pageerror', (e) => pageLogs.push('[pageerror] ' + e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
await page.waitForSelector('.editor textarea', { timeout: 30000 });

const setText = async (text) => {
  await page.evaluate((newText) => {
    const ta = document.querySelector('.editor textarea');
    const proto = Object.getPrototypeOf(ta);
    const setter = Object.getOwnPropertyDescriptor(proto, 'value').set;
    setter.call(ta, newText);
    ta.dispatchEvent(new InputEvent('input', { bubbles: true }));
  }, text);
};
const readDispatchLines = () => {
  // Pull the latest [findMisspellings] dispatch line(s) from
  // the captured page logs.
  return pageLogs
    .filter((l) => l.includes('[findMisspellings] dispatch'))
    .slice(-1)[0] || '';
};
const settle = (ms) => new Promise((r) => setTimeout(r, ms));

// ── baseline: 4 misspellings, no other changes ─────────────────
const BASELINE = 'taloo koirra xyzzy asdfg ja talo talossa kisso';
await setText(BASELINE);
await settle(8000);
const baseline = readDispatchLines();
console.log('=== baseline ===');
console.log('  text:    ', JSON.stringify(BASELINE));
console.log('  dispatch:', baseline);
const baseRe = /reused=(\d+) toCheck=(\d+)/;
const baseMatch = baseline.match(baseRe);
const baseReused = baseMatch ? parseInt(baseMatch[1]) : -1;
const baseToCheck = baseMatch ? parseInt(baseMatch[2]) : -1;
console.log('  parsed:  reused=', baseReused, ' toCheck=', baseToCheck);
if (baseReused !== 0) {
  console.log('*** FAIL: baseline should report reused=0 ***');
  process.exit(1);
}

// ── type a single space: 0 changed words ───────────────────────
// All baseline misspellings should be reused, 0 pool calls.
await setText(BASELINE + ' ');
await settle(2500);
const afterSpace = readDispatchLines();
console.log('\n=== after typing one space ===');
console.log('  dispatch:', afterSpace);
const spaceMatch = afterSpace.match(baseRe);
const spaceReused = spaceMatch ? parseInt(spaceMatch[1]) : -1;
const spaceToCheck = spaceMatch ? parseInt(spaceMatch[2]) : -1;
console.log('  parsed:  reused=', spaceReused, ' toCheck=', spaceToCheck);

if (spaceReused === baseToCheck && spaceToCheck === 0) {
  console.log('  *** PASS: all ' + spaceReused +
              ' baseline misspellings reused, 0 pool calls ***');
} else {
  console.log('  *** FAIL: expected reused=' + baseToCheck + ' toCheck=0 ***');
  process.exit(2);
}

// ── type a new misspelling: should reuse all, dispatch 1 ───────
await setText(BASELINE + ' munster');
await settle(8000);
const afterNew = readDispatchLines();
console.log('\n=== after adding new misspelling "munster" ===');
console.log('  dispatch:', afterNew);
const newMatch = afterNew.match(baseRe);
const newReused = newMatch ? parseInt(newMatch[1]) : -1;
const newToCheck = newMatch ? parseInt(newMatch[2]) : -1;
console.log('  parsed:  reused=', newReused, ' toCheck=', newToCheck);

if (newReused === baseToCheck && newToCheck === 1) {
  console.log('  *** PASS: ' + newReused + ' reused + 1 new pool dispatch ***');
} else {
  console.log('  *** FAIL: expected reused=' + baseToCheck + ' toCheck=1 ***');
  process.exit(3);
}

// ── live underlay snapshot ─────────────────────────────────────
const ta = await page.$('.editor textarea');
const taValue = await ta.evaluate((el) => el.value);
const underlay = await page.$$eval(
  '.editor .underlay .misspell',
  (els) => els.map((e) => e.textContent),
);
console.log('\n=== final live underlay ===');
console.log('  textarea:', JSON.stringify(taValue));
console.log('  misses (' + underlay.length + '):');
for (const w of underlay) console.log('   ', JSON.stringify(w));
const expected = new Set(['taloo', 'koirra', 'xyzzy', 'asdfg', 'munster']);
const got = new Set(underlay);
let allOk = true;
for (const w of expected) {
  if (!got.has(w)) {
    console.log('  *** MISSING: ' + w + ' ***');
    allOk = false;
  }
}
if (allOk) {
  console.log('  *** PASS: all 5 misspellings visible in underlay ***');
} else {
  console.log('  *** FAIL ***');
  process.exit(4);
}

await browser.close();
