// Byte-offset ↔ UTF-16 index conversion.
//
// `torille-wasm` reports hit ranges as UTF-8 *byte* offsets (Rust's
// `str` indices). JavaScript strings are indexed by UTF-16 code unit,
// so every `ä`, `ö`, `å` (2 bytes, 1 unit) before a hit shifts the
// range by one, and an emoji (4 bytes, 2 units) shifts it by two --
// placing the underline on the wrong characters and making the popup
// replace the wrong substring.
//
// `buildByteToUtf16Map(text)` returns one entry per byte offset
// (length `byteLength + 1`); `map[byteOffset]` is the UTF-16 index of
// the character containing that byte. Misspell ranges always fall on
// character boundaries, so indexing with `start` / `end` is exact.

/**
 * Build a byte-offset → UTF-16-index lookup table for `text`.
 *
 * The table has `utf8ByteLength(text) + 1` entries. All bytes that
 * belong to one character map to that character's starting UTF-16
 * index, which is what a range boundary needs.
 */
export function buildByteToUtf16Map(text: string): Uint32Array {
  // Pass 1: exact UTF-8 length, so the table is neither under- nor
  // over-allocated for large pastes.
  let byteLen = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (c < 0x80) {
      byteLen += 1;
    } else if (c < 0x800) {
      byteLen += 2;
    } else if (
      c >= 0xd800 && c <= 0xdbff &&
      i + 1 < text.length &&
      text.charCodeAt(i + 1) >= 0xdc00 &&
      text.charCodeAt(i + 1) <= 0xdfff
    ) {
      byteLen += 4;
      i++; // low surrogate consumed
    } else {
      byteLen += 3;
    }
  }

  // Pass 2: fill. Each char's bytes map to the char's first UTF-16
  // index; the char then advances the UTF-16 cursor by 1 or 2.
  const map = new Uint32Array(byteLen + 1);
  let b = 0;
  let u = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    let bytes: number;
    let units = 1;
    if (c < 0x80) {
      bytes = 1;
    } else if (c < 0x800) {
      bytes = 2;
    } else if (
      c >= 0xd800 && c <= 0xdbff &&
      i + 1 < text.length &&
      text.charCodeAt(i + 1) >= 0xdc00 &&
      text.charCodeAt(i + 1) <= 0xdfff
    ) {
      bytes = 4;
      units = 2;
      i++; // low surrogate consumed
    } else {
      bytes = 3;
    }
    for (let k = 0; k < bytes; k++) map[b + k] = u;
    b += bytes;
    u += units;
  }
  map[byteLen] = u;
  return map;
}

/**
 * Convert one UTF-8 byte offset to a UTF-16 index.
 *
 * Intended for call sites that only have a single offset; prefer
 * `buildByteToUtf16Map` when converting a whole hit list (one pass
 * per text instead of one per hit).
 */
export function byteToUtf16Offset(text: string, byteOffset: number): number {
  const map = buildByteToUtf16Map(text);
  return map[Math.max(0, Math.min(byteOffset, map.length - 1))];
}
