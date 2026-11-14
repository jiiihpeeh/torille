// Wasm sentence-cache probe.
//
// Calls the combined grammar/style export three times through one textarea
// state: the cold page, the same page again (all sentence hashes hit), and a
// page with one sentence edited (one miss, the rest hits). The edited result
// must match a fresh state's whole-text result exactly.
//
// Run from this directory after `node build.mjs`:
//
//   node probe-sentence-cache.mjs

import { readFile } from 'node:fs/promises';

const pkg = new URL('../pkg/', import.meta.url);
const glue = await import(new URL('torille_wasm_bg.js', pkg).href);
const bytes = await readFile(new URL('torille_wasm_bg.wasm', pkg));
const { instance } = await WebAssembly.instantiate(bytes, {
  './torille_wasm_bg.js': glue,
});
glue.__wbg_set_wasm(instance.exports);
if (typeof instance.exports.__wbindgen_start === 'function') {
  instance.exports.__wbindgen_start();
}

const fixture = new URL('../../tests/fixtures/perf/page-2000.txt', import.meta.url);
const text = await readFile(fixture, 'utf8');

function editOneSentence(source) {
  const boundary = source.indexOf('. ', Math.floor(source.length / 2));
  const start = boundary < 0 ? Math.floor(source.length / 2) : boundary + 2;
  return source.slice(0, start) + 'x' + source.slice(start);
}

const key = (diagnostic) =>
  `${diagnostic.kind}|${diagnostic.start}|${diagnostic.end}|${diagnostic.message}`;

const state = glue.create_text_area_state();
let started = performance.now();
const cold = glue.check_grammar_style_with_state(state, text, true, true);
const coldMs = performance.now() - started;

started = performance.now();
const warm = glue.check_grammar_style_with_state(state, text, true, true);
const warmMs = performance.now() - started;

const edited = editOneSentence(text);
started = performance.now();
const changed = glue.check_grammar_style_with_state(state, edited, true, true);
const editMs = performance.now() - started;

const fresh = glue.create_text_area_state();
const expected = glue.check_grammar_style_with_state(fresh, edited, true, true);

const same = (left, right) =>
  JSON.stringify(left.map(key).sort()) === JSON.stringify(right.map(key).sort());
const warmMatches = same(cold.diagnostics, warm.diagnostics) &&
  same(cold.style, warm.style);
const editMatches = same(changed.diagnostics, expected.diagnostics) &&
  same(changed.style, expected.style);

console.log('=== wasm sentence cache ===');
console.log(`  cold:  ${coldMs.toFixed(1)} ms  (${cold.diagnostics.length} grammar, ${cold.style.length} style)`);
console.log(`  warm:  ${warmMs.toFixed(1)} ms  (all sentence hashes hit)`);
console.log(`  edit:  ${editMs.toFixed(1)} ms  (${changed.diagnostics.length} grammar, ${changed.style.length} style)`);
console.log(`  warm result matches cold: ${warmMatches}`);
console.log(`  edited result matches fresh whole-text check: ${editMatches}`);

glue.destroy_text_area_state(state);
glue.destroy_text_area_state(fresh);
process.exit(warmMatches && editMatches ? 0 : 1);
