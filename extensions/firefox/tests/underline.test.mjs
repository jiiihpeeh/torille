// Unit tests for the underline partition used by the textarea mirror.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  partitionUnderlines,
  underlineClass,
} from '../src/content/underline.ts';

test('a spelling underline is the existing misspell class', () => {
  assert.equal(
    underlineClass({ start: 0, end: 3, layer: 'spelling' }),
    'misspell',
  );
});

test('grammar severities get distinct classes', () => {
  assert.equal(
    underlineClass({ start: 0, end: 3, layer: 'grammar', severity: 'error' }),
    'grammar-error',
  );
  assert.equal(
    underlineClass({ start: 0, end: 3, layer: 'grammar', severity: 'warning' }),
    'grammar-warning',
  );
  assert.equal(
    underlineClass({ start: 0, end: 3, layer: 'grammar', severity: 'suggestion' }),
    'grammar-suggestion',
  );
});

test('style always uses the style class', () => {
  assert.equal(
    underlineClass({ start: 0, end: 3, layer: 'style' }),
    'style-suggestion',
  );
});

test('plain text is one unstyled region', () => {
  assert.deepEqual(partitionUnderlines(5, []), [
    { start: 0, end: 5, classes: [] },
  ]);
});

test('a single underline splits the text into three regions', () => {
  assert.deepEqual(
    partitionUnderlines(10, [{ start: 2, end: 5, layer: 'spelling' }]),
    [
      { start: 0, end: 2, classes: [] },
      { start: 2, end: 5, classes: ['misspell'] },
      { start: 5, end: 10, classes: [] },
    ],
  );
});

test('overlapping layers keep both classes', () => {
  const regions = partitionUnderlines(10, [
    { start: 2, end: 6, layer: 'spelling' },
    { start: 4, end: 8, layer: 'grammar', severity: 'error' },
  ]);
  assert.deepEqual(regions, [
    { start: 0, end: 2, classes: [] },
    { start: 2, end: 4, classes: ['misspell'] },
    { start: 4, end: 6, classes: ['grammar-error', 'misspell'] },
    { start: 6, end: 8, classes: ['grammar-error'] },
    { start: 8, end: 10, classes: [] },
  ]);
});

test('adjacent regions with the same classes are merged', () => {
  assert.deepEqual(
    partitionUnderlines(10, [
      { start: 2, end: 4, layer: 'style' },
      { start: 4, end: 6, layer: 'style' },
    ]),
    [
      { start: 0, end: 2, classes: [] },
      { start: 2, end: 6, classes: ['style-suggestion'] },
      { start: 6, end: 10, classes: [] },
    ],
  );
});

test('out-of-range spans are clamped and empty spans are ignored', () => {
  assert.deepEqual(
    partitionUnderlines(5, [
      { start: -2, end: 2, layer: 'style' },
      { start: 3, end: 99, layer: 'spelling' },
      { start: 4, end: 4, layer: 'grammar', severity: 'error' },
    ]),
    [
      { start: 0, end: 2, classes: ['style-suggestion'] },
      { start: 2, end: 3, classes: [] },
      { start: 3, end: 5, classes: ['misspell'] },
    ],
  );
});
