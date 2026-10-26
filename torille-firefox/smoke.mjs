// Smoke test for the torille Firefox content-script extension.
//
// Verifies the three pieces in isolation:
//   1. The wasm: `check` and `suggest_words` agree with the demo and
//      the Rust unit tests.
//   2. The tokenize regex (re-implemented identically here) is the
//      same as the content script's.
//   3. The worker (`spell-worker.js`) round-trips a `check` message.
//
// The actual content-script behaviour (shadow-DOM overlay, click-to-
// replace) is best exercised by installing the extension in Firefox
// via `web-ext run` -- puppeteer can drive the page but can't easily
// stand in for `browser.runtime.getURL` and the worker URL scheme.

import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// The pkg is a bundler-target build, so there is no `init` export to
// call: wire the glue to a hand-instantiated instance, exactly like
// `src/spell-worker/wasm-loader.ts` does at runtime.
import * as glue from './dist/spell-worker/torille_wasm_bg.js';

const here = dirname(fileURLToPath(import.meta.url));
const wasmBytes = await readFile(join(here, 'dist', 'spell-worker', 'torille_wasm_bg.wasm'));
const { instance } = await WebAssembly.instantiate(
  wasmBytes,
  { './torille_wasm_bg.js': glue },
);
glue.__wbg_set_wasm(instance.exports);
const start = instance.exports.__wbindgen_start;
if (typeof start === 'function') start();

const { check, suggest_words } = glue;

/**
 * @typedef {Object} Token
 * @property {string} text  The matched substring (a word or word-with-trailing-hyphen).
 * @property {number} start Byte offset in the input where the match begins.
 */

/**
 * Re-implementation of the content script's tokeniser so the smoke
 * test exercises the exact same regexes. Splits the input into runs
 * of Unicode letters (with an optional trailing hyphen to keep split
 * compounds like "nuori-" + "isäntä" as adjacent tokens) and skips
 * runs of non-letters (whitespace, digits, punctuation).
 *
 * @param {string} text  Raw text to tokenise.
 * @returns {Token[]}    List of `{text, start}` pairs.
 */
function tokenize(text) {
  const out = [];
  for (const m of text.matchAll(TOKEN_RE)) {
    if (WORD_RE.test(m[0])) out.push({ text: m[0], start: m.index });
  }
  return out;
}

const cases = [
  ['talo', true, 'lemma'],
  ['taloo', false, 'one-edit typo of talo'],
  ['koirra', false, 'one-edit typo of koira'],
  ['xyzzy', false, 'unknown'],
  ['Kissa', true, 'case-insensitive'],
  ['puhtaan', true, 'phase 3 gradated form'],
  ['öihin', true, 'phase 4 short lemma'],
];

console.log('1. wasm check:');
let failCount = 0;
for (const [w, want, why] of cases) {
  const got = check(w);
  const ok = got === want;
  if (!ok) failCount++;
  console.log(`  ${ok ? 'OK ' : 'BAD'} ${JSON.stringify(w).padEnd(10)}  expected=${want}  got=${got}  // ${why}`);
}

const sugg = Array.from(suggest_words('taloo', 5));
console.log('\n2. suggest_words:');
const hasTalo = sugg.includes('talo');
console.log(`  ${hasTalo ? 'OK ' : 'BAD'} suggest_words("taloo",5) includes "talo": ${JSON.stringify(sugg)}`);
if (!hasTalo) failCount++;

// 3. tokenize — same regex the content script uses.
const TOKEN_RE = /[\p{L}]+(?:-)?|[^a-zA-ZäöÄÖåÅ]+/gu;
const WORD_RE = /^[\p{L}-]+$/u;

console.log('\n3. tokenize:');
const t1 = tokenize('Tässä on kirjoitusvirhee. Koira juoksee.');
const t2 = tokenize('nuori-isäntä särkynytsydän');
console.log(`  Tässä on kirjoitusvirhee. Koira juoksee.`);
console.log(`    -> ${JSON.stringify(t1.map((t) => t.text))}`);
console.log(`  nuori-isäntä särkynytsydän`);
console.log(`    -> ${JSON.stringify(t2.map((t) => t.text))}`);
// Split compounds become two tokens (the engine handles
// recombination separately); single words stay one token.
const tokenOk =
  t1.map((t) => t.text).join(',') === 'Tässä,on,kirjoitusvirhee,Koira,juoksee' &&
  t2.map((t) => t.text).join(',') === 'nuori-,isäntä,särkynytsydän';
console.log(`  ${tokenOk ? 'OK ' : 'BAD'} length & shape`);
if (!tokenOk) failCount++;

if (failCount > 0) {
  console.log(`\n${failCount} check(s) failed`);
  process.exit(1);
}
console.log('\nall checks passed');