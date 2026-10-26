// Regression tests for the UTF-8 byte → UTF-16 index conversion.
//
// `torille-wasm` reports hit ranges as Rust byte offsets. JavaScript
// slices strings by UTF-16 code unit, so without the conversion an
// underline after any `ä`/`ö`/`å` lands one character to the left.
// The existing e2e used only ASCII, which is why it never caught this.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildByteToUtf16Map,
  byteToUtf16Offset,
} from '../src/spell-worker/offsets.ts';

const utf8Len = (s) => new TextEncoder().encode(s).length;

test('ASCII offsets are unchanged', () => {
  const text = 'talo taloo';
  const map = buildByteToUtf16Map(text);
  assert.equal(map.length, text.length + 1);
  for (let i = 0; i <= text.length; i++) assert.equal(map[i], i);
  assert.equal(map[text.length], text.length);
});

test('ä/ö/å shift the hit by one per character', () => {
  const text = 'äiti taloo';
  const map = buildByteToUtf16Map(text);
  assert.equal(map.length, utf8Len(text) + 1);

  // "taloo" is bytes 6..11 and UTF-16 units 5..10.
  const start = 6;
  const end = 11;
  assert.equal(map[start], 5);
  assert.equal(map[end], 10);
  // The scoping bug was visible exactly here: byte-slicing returned
  // the wrong substring.
  assert.equal(text.slice(map[start], map[end]), 'taloo');
  assert.notEqual(text.slice(start, end), 'taloo');
});

test('mixed Finnish text keeps every word boundary exact', () => {
  const text = 'Älä syö ääkkösiä taloo';
  const map = buildByteToUtf16Map(text);
  assert.equal(map.length, utf8Len(text) + 1);
  assert.equal(map[map.length - 1], text.length);

  const word = 'taloo';
  const byteStart = utf8Len(text.slice(0, text.indexOf(word)));
  assert.equal(
    text.slice(map[byteStart], map[byteStart + utf8Len(word)]),
    word,
  );
});

test('an emoji shifts by two UTF-16 units', () => {
  const text = '😀 koirra';
  const map = buildByteToUtf16Map(text);
  assert.equal(map.length, utf8Len(text) + 1);

  // "koirra" is bytes 5..11 and UTF-16 units 3..9.
  assert.equal(map[5], 3);
  assert.equal(map[11], 9);
  assert.equal(text.slice(map[5], map[11]), 'koirra');
});

test('every character boundary maps to its own UTF-16 start', () => {
  const text = 'aä😀öZ';
  const map = buildByteToUtf16Map(text);
  let byte = 0;
  let unit = 0;
  for (const ch of text) {
    assert.equal(map[byte], unit, `byte ${byte} -> unit ${unit}`);
    byte += utf8Len(ch);
    unit += ch.length;
  }
  assert.equal(map[byte], unit);
  assert.equal(byte, map.length - 1);
});

test('byteToUtf16Offset is bounds-safe', () => {
  const text = 'äiti taloo';
  assert.equal(byteToUtf16Offset(text, 0), 0);
  assert.equal(byteToUtf16Offset(text, 6), 5);
  assert.equal(byteToUtf16Offset(text, 10_000), text.length);
  assert.equal(byteToUtf16Offset(text, -1), 0);
});
