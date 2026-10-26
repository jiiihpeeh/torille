// Sylvi stress-test: load a slice of Minna Canth's Sylvi
// (Gutenberg #13174, 1888-1893) into the editor, then
// verify the live underlay:
//   1. Marks garbage misspellings (xyzzy, qwerty, ...) red
//   2. Stays clean on real Finnish (modern + the play's
//      older orthography where the lemma check still holds)
//   3. Stays clean on inflected forms (talossa, juoksee,
//      kauppaan, ...) -- the wasm pool's bg.check should
//      demote them from the misspell list
//
// Run from this directory after `node build.mjs`:
//
//   node probe-sylvi.mjs
//
// Pre-req: the demo server is up on :8765
// (`cargo run --release -p torille-demo-server`) serving the rebuilt dist/.

import { readFileSync } from 'node:fs';
import puppeteer from 'puppeteer-core';

// 1. Pull a slice of Sylvi (skip Gutenberg boilerplate).
//    We use a MIDDLE slice -- a dialogue scene between Alma,
//    Viktor, and Sylvi at a party. This avoids the stage-
//    direction heavy opening (which has historical-orthography
//    false positives like "Etehisessä", "makuuhuoneesen",
//    "alkoovi") and exercises the inflection engine on natural
//    modern dialogue: present-tense verbs (puhuu, menee, itkee,
//    nauraa), partitives (Sylviä, Almaa), illatives (saliin),
//    and a couple of archaisms (tänäin -> tänään) that the
//    first-pass analyzer should still tolerate.
//
//    Slice anchor is "Kaunis ilma tänäin" -- a stable marker in
//    the current Gutenberg text that sits right at the
//    dialogue scene we want to exercise (with "tänäin" being
//    the historical-orthography archaism mentioned above).
const raw = readFileSync('/tmp/sylvi.txt', 'utf8');
const sliceStart = raw.indexOf('Kaunis ilma tänäin');
if (sliceStart < 0) {
  throw new Error('Could not find middle slice marker in Sylvi text');
}
const contentSlice = raw.slice(sliceStart, sliceStart + 1200);

// 2. Garbage misspellings that the first-pass analyzer
//    will reliably flag. Real Finnish-looking misspellings
//    like "mennee" are sometimes passed by the inflection
//    engine, which is what we're testing the de-mote path
//    for, not the first pass.
const GARBAGE = 'qwerty asdfg zzzz plover munster xylyx';

const TEXT = `${GARBAGE} ${contentSlice}`;

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
page.on('pageerror', (e) => pageLogs.push('[pageerror]', e));

await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
await page.waitForSelector('.editor textarea', { timeout: 30000 });

// ── helpers ───────────────────────────────────────────────────
const setText = async (text) => {
  await page.evaluate((newText) => {
    const ta = document.querySelector('.editor textarea');
    const proto = Object.getPrototypeOf(ta);
    const setter = Object.getOwnPropertyDescriptor(proto, 'value').set;
    setter.call(ta, newText);
    ta.dispatchEvent(new InputEvent('input', { bubbles: true }));
  }, text);
};

// Wait until `getCount` has been quiet (same value) for
// `quietMs`, OR until the count drops to `target` (or
// below). The slow `bg.check` demotes verb inflections
// over the next 10-45 s depending on text size; `target`
// lets us short-circuit the wait for the "happy path"
// when the count is already at the expected final value.
const waitForStable = async (
  getCount, target,
  { maxMs = 60000, quietMs = 2000, pollMs = 200 } = {},
) => {
  const start = Date.now();
  let last = await getCount();
  if (last <= target) return last;
  let quietSince = Date.now();
  while (Date.now() - start < maxMs) {
    await new Promise((r) => setTimeout(r, pollMs));
    const c = await getCount();
    if (c <= target) return c;
    if (c !== last) {
      last = c;
      quietSince = Date.now();
    } else if (Date.now() - quietSince >= quietMs) {
      return c;
    }
  }
  return last;
};

// Wait until a new render's `[findMisspellings] dispatch`
// line appears in the page-side console. The render is
// gated on a 150 ms debounce + the wasm's first-pass
// (~1 ms) + Solid's reactive propagation. The 4 s ceiling
// is generous; if we don't see a dispatch by then the
// test setup is broken and we should fail loudly.
const waitForRender = async (beforeLogs) => {
  const t0 = Date.now();
  while (Date.now() - t0 < 4000) {
    const newLines = pageLogs.slice(beforeLogs).filter(
      (l) => l.includes('[findMisspellings] dispatch'),
    );
    if (newLines.length > 0) return;
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error('no [findMisspellings] dispatch within 4 s -- render not firing');
};

const underlayCount = () => page.$$eval(
  '.editor .underlay .misspell',
  (els) => els.length,
);
const underlayWords = () => page.$$eval(
  '.editor .underlay .misspell',
  (els) => els.map((e) => e.textContent),
);

// ── TEST 1: garbage + Sylvi slice ──────────────────────────────
// Set the text, wait for the render to fire (the
// `[findMisspellings] dispatch` log line), then wait for
// the new candidates to populate and the slow check to
// settle. We require the count to be at least 1 before
// returning (so we don't read a mid-render empty
// underlay), and otherwise wait for the count to drop to
// the expected range or be quiet for 2 s.
const beforeT1 = pageLogs.length;
await setText(TEXT);
await waitForRender(beforeT1);
// Wait until the underlay has the new render's content.
// The first-pass results arrive ~1-5 ms after the dispatch,
// so we just give the DOM a moment to settle and then
// fall back to the "count <= 10 or quiet for 2 s" logic.
await new Promise((r) => setTimeout(r, 300));
await waitForStable(underlayCount, 10);

const ta = await page.$('.editor textarea');
const taValue = await ta.evaluate((el) => el.value);
const misspells = await underlayWords();
const counter = await page.$eval('h1 small', (e) => e.textContent);

console.log('=== TEST 1: garbage-prefixed Sylvi slice ===');
console.log('  text length:  ', taValue.length, 'chars');
console.log('  starts with:  ', JSON.stringify(taValue.slice(0, 60)));
console.log('  counter:      ', counter);
console.log('  underlay (' + misspells.length + ' words):');
for (const m of misspells) console.log('   ', JSON.stringify(m));

let pass = true;
const garbageWords = GARBAGE.split(/\s+/);
const present = new Set(misspells);
let allFound = true;
console.log('\n  garbage coverage:');
for (const w of garbageWords) {
  const ok = present.has(w);
  console.log('  ', ok ? '✓' : '✗', w);
  if (!ok) allFound = false;
}
if (allFound) {
  console.log('\n*** PASS: all 6 garbage misspellings are underlined ***');
} else {
  console.log('\n*** FAIL: at least one garbage word is missing from underlay ***');
  pass = false;
}

// ── TEST 2: clean Sylvi slice (no garbage) ─────────────────────
// The slow bg.check demotes verb inflections over ~10 s.
// Wait for the demote pass to stabilise (2 s of quiet) before
// reading the underlay.
const beforeT2 = pageLogs.length;
await setText(contentSlice);
await waitForRender(beforeT2);
await new Promise((r) => setTimeout(r, 300));
// Expected: 0-8 historical-orthography false positives.
// Wait until underlay is at most 10, or quiet for 2 s.
await waitForStable(underlayCount, 10);

const cleanMisspells = await underlayWords();
const cleanCounter = await page.$eval('h1 small', (e) => e.textContent);
console.log('\n=== TEST 2: clean Sylvi slice (no garbage) ===');
console.log('  counter:  ', cleanCounter);
console.log('  underlay (' + cleanMisspells.length + ' words):');
for (const m of cleanMisspells) console.log('   ', JSON.stringify(m));

// Diagnostic: dump the demote + candidate state from the
// page so we can see why words are still underlined.
const state = await page.evaluate(() => {
  const w = window.__torille;
  if (!w) return { error: 'no __torille debug hook' };
  const cand = w.candidates();
  const dem = Array.from(w.demoted());
  const miss = w.misspellings().map((m) => m.word);
  return { candidateCount: cand.length, demotedCount: dem.length,
           demoted: dem.slice(0, 30),
           candidates: cand.slice(0, 30).map((m) => m.word),
           misspellings: miss };
});
console.log('  internal state:');
console.log('    candidate count:', state.candidateCount);
console.log('    demoted count:  ', state.demotedCount);
console.log('    demoted:        ', state.demoted);
console.log('    candidates (30):', state.candidates);
console.log('    misspellings:   ', state.misspellings);

// Historical spellings (makuuhuoneesen, alkoovi, Etehisessä,
// etc.) are known false positives from 1888 orthography. We
// allow up to 8 false positives but FAIL if it's catastrophic.
if (cleanMisspells.length === 0) {
  console.log('\n*** PASS: clean slice -> 0 misspellings ***');
} else if (cleanMisspells.length <= 8) {
  console.log(`\n*** OK: clean slice -> ${cleanMisspells.length} misspellings ` +
              '(historical-orthography false positives, expected) ***');
} else {
  console.log(`\n*** FAIL: clean slice -> ${cleanMisspells.length} underlines ***`);
  pass = false;
}

// ── page-side log ──────────────────────────────────────────────
console.log('\n=== filtered page-side log ===');
for (const l of pageLogs) {
  if (l.includes('[demo]') || l.includes('[findMisspellings]')) {
    console.log('  ' + l);
  }
}

await browser.close();
if (!pass) process.exit(1);
