// The Chromium and Firefox extensions carry the same content script and
// ignore-range parser. Keep the copies byte-identical; a fix that lands
// in only one browser is worse than no fix.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const other = join(here, '..', '..', 'firefox', 'src');

for (const rel of [
  'content/content.ts',
  'content/shared.ts',
  'content/worker-client.ts',
  'content/overlay/index.ts',
  'content/overlay/textarea.ts',
  'content/overlay/contenteditable.ts',
  'content/overlay/popup.ts',
  'content/grammar-panel.ts',
  'content/context-menu.ts',
  'content/blacklist.ts',
  'content/scanner.ts',
  'content/ignore-ranges.ts',
  'options/index.tsx',
]) {
  test(`chromium and firefox agree on ${rel}`, async () => {
    const mine = await readFile(join(here, '..', 'src', rel), 'utf8');
    const theirs = await readFile(join(other, rel), 'utf8');
    assert.equal(mine, theirs);
  });
}
