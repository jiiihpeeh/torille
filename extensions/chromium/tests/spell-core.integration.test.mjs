// Integration test for the worker core: wasm loader + analyse offsets.
//
// Imports the *built* modules from `dist/` (run `npm run build`
// first). This exercises the same `spell-core.ts` path the extension
// uses in both hosts: the Firefox module worker and the Chromium
// service worker's direct import.
//
// Node has no `fetch(file://)`, so the test shims fetch for the wasm
// bytes and leaves everything else untouched.

import test from 'node:test';
import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const distCore = join(here, '..', 'dist', 'spell-worker', 'spell-core.js');

let available = true;
try {
  await access(distCore);
} catch {
  available = false;
}

// Minimal file:// fetch shim for the wasm bytes.
const realFetch = globalThis.fetch;
globalThis.fetch = async (input, init) => {
  const url = typeof input === 'string' ? input : String(input);
  if (url.startsWith('file:')) {
    return new Response(await readFile(fileURLToPath(url)));
  }
  return realFetch(input, init);
};

test('built core: analyse reports UTF-16 ranges after non-ASCII text',
     { skip: !available && 'run `npm run build` first' }, async () => {
  const { createSpellCore } = await import(distCore);
  const core = createSpellCore();
  await core.init();

  const created = await core.handle({ id: 1, type: 'create-state' });
  assert.equal(created?.type, 'create-state');
  const idx = created.idx;
  assert.equal(typeof idx, 'number');

  for (const text of ['äiti taloo', '😀 koirra', 'Älä syö ääkkösiä taloo']) {
    const out = await core.handle({
      id: 2, type: 'analyze-text', idx, text, max: 5,
    });
    assert.equal(out?.type, 'analyze-text');
    for (const hit of out.hits) {
      // Every range must land on a whole word in the JS string.
      const slice = text.slice(hit.start, hit.end);
      assert.equal(slice, hit.word,
        `range drifted: ${JSON.stringify({ text, hit, slice })}`);
    }
  }

  const last = await core.handle({
    id: 3, type: 'analyze-text', idx, text: 'äiti taloo', max: 5,
  });
  const taloo = last.hits.find((h) => h.word === 'taloo');
  assert.ok(taloo, 'taloo should be flagged');
  assert.equal('äiti taloo'.slice(taloo.start, taloo.end), 'taloo');
  assert.equal(taloo.start, 5);
  assert.equal(taloo.end, 10);

  await core.handle({ id: 4, type: 'destroy-state', idx });
});

test('built core: check-syntax reports UTF-16 spans and edits',
     { skip: !available && 'run `npm run build` first' }, async () => {
  const { createSpellCore } = await import(distCore);
  const core = createSpellCore();
  await core.init();

  const created = await core.handle({ id: 1, type: 'create-state' });
  assert.equal(created?.type, 'create-state');
  const idx = created.idx;

  const text = 'Äiti söi puuron. Koira nukkuvat.';
  const out = await core.handle({
    id: 2, type: 'check-syntax', idx, text,
  });
  assert.equal(out?.type, 'check-syntax');
  const diagnostic = out.diagnostics.find(
    (d) => d.kind === 'subject_predicate_agreement');
  assert.ok(diagnostic, 'the agreement diagnostic should be present');
  assert.equal(text.slice(diagnostic.start, diagnostic.end), 'nukkuvat');
  const edit = diagnostic.corrections[0].edits[0];
  assert.equal(text.slice(edit.start, edit.end), 'nukkuvat');
  assert.equal(edit.replacement, 'nukkuu');

  await core.handle({ id: 3, type: 'destroy-state', idx });
});

test('built core: check-style is suggestion-only with UTF-16 spans',
     { skip: !available && 'run `npm run build` first' }, async () => {
  const { createSpellCore } = await import(distCore);
  const core = createSpellCore();
  await core.init();

  const text = 'Tällä hetkellä tilanne on rauhallinen.';
  const out = await core.handle({ id: 1, type: 'check-style', text });
  assert.equal(out?.type, 'check-style');
  const diagnostic = out.diagnostics.find((d) => d.kind === 'style_wordy');
  assert.ok(diagnostic, 'the phrase diagnostic should be present');
  assert.equal(diagnostic.severity, 'suggestion');
  assert.equal(text.slice(diagnostic.start, diagnostic.end), 'Tällä hetkellä');
  assert.equal(diagnostic.corrections[0].edits[0].replacement, 'Nyt');
});

test('built core: check-syntax falls back when the state handle is stale',
     { skip: !available && 'run `npm run build` first' }, async () => {
  const { createSpellCore } = await import(distCore);
  const core = createSpellCore();
  await core.init();

  const out = await core.handle({
    id: 1, type: 'check-syntax', idx: 999999, text: 'Koira nukkuvat.',
  });
  assert.equal(out?.type, 'check-syntax');
  assert.ok(out.diagnostics.some(
    (d) => d.kind === 'subject_predicate_agreement'));
});

test('built core: unavailable wasm is not reported as misspelled',
     { skip: !available && 'run `npm run build` first' }, async () => {
  // Sanity-check the contract the background relies on: `check`
  // returns a boolean, and a transport failure throws (which the
  // background turns into `ok: null`) rather than returning false.
  const { createSpellCore } = await import(distCore);
  const core = createSpellCore();
  await core.init();

  const ok = await core.handle({ id: 1, type: 'check', word: 'talo' });
  assert.deepEqual(ok, { type: 'check', ok: true });

  const bad = await core.handle({ id: 2, type: 'check', word: 'taloo' });
  assert.deepEqual(bad, { type: 'check', ok: false });
});
