// Unit tests for the code/BBCode ignore layer.
//
// Everything here works on JS string indices (UTF-16 code units), so the
// non-ASCII cases are part of the contract: a mask must never change the
// length of the text the wasm sees.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  computeIgnoredRanges,
  isInsideCodeDom,
  isIgnored,
  maskIgnored,
  mergeRanges,
  parseBbcode,
  parseFencedCode,
  parseHtmlCode,
  parseInlineCode,
  parseUrlsAndEmails,
} from '../src/content/ignore-ranges.ts';

const covered = (text, ranges, word) => {
  const at = text.indexOf(word);
  assert.notEqual(at, -1, `missing test word ${word}`);
  return isIgnored({ start: at, end: at + word.length }, ranges);
};

test('BBCode [code] content is ignored, [quote] prose is not', () => {
  const text = '[quote]talo on kiva[/quote] [code]xyzzy()[/code]';
  const ranges = parseBbcode(text);
  assert.equal(covered(text, ranges, 'xyzzy'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
  assert.equal(covered(text, ranges, 'kiva'), false);
  // Tag syntax itself is never checked.
  assert.equal(covered(text, ranges, '[quote]'), true);
  assert.equal(covered(text, ranges, '[/code]'), true);
});

test('BBCode is case-insensitive and supports attributes', () => {
  const text = '[CODE=php]$x = 1;[/CODE] [color=red]talo[/color]';
  const ranges = parseBbcode(text);
  assert.equal(covered(text, ranges, '$x'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
});

test('nested [code] inside [quote] masks only the code', () => {
  const text = '[quote]talo [code]xyzzy[/code] kiva[/quote]';
  const ranges = parseBbcode(text);
  assert.equal(covered(text, ranges, 'xyzzy'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
  assert.equal(covered(text, ranges, 'kiva'), false);
});

test('nested same-name [code] blocks stay masked', () => {
  const text = '[code]a [code]b[/code] c[/code] talo';
  const ranges = parseBbcode(text);
  assert.equal(covered(text, ranges, 'a'), true);
  assert.equal(covered(text, ranges, 'b'), true);
  assert.equal(covered(text, ranges, 'c'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
});

test('unclosed [code] ignores to end of text', () => {
  const text = 'talo [code]xyzzy never closes';
  const ranges = parseBbcode(text);
  assert.equal(covered(text, ranges, 'xyzzy'), true);
  assert.equal(covered(text, ranges, 'never'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
});

test('URL/image/email payloads are ignored', () => {
  const text = [
    'katso [url]https://example.fi/talo[/url]',
    '[img]kuva.png[/img]',
    '[email]matti@example.fi[/email]',
    'talo',
  ].join(' ');
  const ranges = parseBbcode(text);
  assert.equal(covered(text, ranges, 'example.fi/talo'), true);
  assert.equal(covered(text, ranges, 'kuva.png'), true);
  assert.equal(covered(text, ranges, 'matti@example.fi'), true);
  assert.equal(covered(text, ranges, 'katso'), false);
});

test('escaped BBCode opener is prose', () => {
  const text = 'kirjoita \\[code] nain';
  const ranges = parseBbcode(text);
  assert.equal(covered(text, ranges, 'code'), false);
  assert.equal(covered(text, ranges, 'nain'), false);
});

test('fenced code with language tag masks the fence and body', () => {
  const text = 'talo\n```js\nconst x = 1;\n```\nkiva';
  const ranges = parseFencedCode(text);
  assert.equal(covered(text, ranges, 'const'), true);
  assert.equal(covered(text, ranges, '```js'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
  assert.equal(covered(text, ranges, 'kiva'), false);
});

test('tilde fences and longer closing fences', () => {
  const text = '~~~python\nprint(1)\n~~~~\ntalo';
  const ranges = parseFencedCode(text);
  assert.equal(covered(text, ranges, 'print'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
});

test('unclosed fence ignores to end of text', () => {
  const text = 'talo\n```\nconst x = 1;';
  const ranges = parseFencedCode(text);
  assert.equal(covered(text, ranges, 'const'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
});

test('inline code requires a paired backtick on the same line', () => {
  const text = 'talo `xyzzy` kiva `unmatched';
  const ranges = parseInlineCode(text);
  assert.equal(covered(text, ranges, 'xyzzy'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
  assert.equal(covered(text, ranges, 'unmatched'), false);
});

test('double-backtick spans can contain a single backtick', () => {
  const text = 'talo ``xyz` z`` kiva';
  const ranges = parseInlineCode(text);
  assert.equal(covered(text, ranges, 'xyz'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
  assert.equal(covered(text, ranges, 'kiva'), false);
});

test('apostrophes are not inline code', () => {
  const text = "tama on minun talo's";
  assert.deepEqual(parseInlineCode(text), []);
});

test('raw HTML code elements are ignored, anchor text is prose', () => {
  const text = '<pre><code>xyzzy</code></pre> <a href="/x">talo</a>';
  const ranges = parseHtmlCode(text);
  assert.equal(covered(text, ranges, 'xyzzy'), true);
  assert.equal(covered(text, ranges, '<pre>'), true);
  assert.equal(covered(text, ranges, 'href'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
});

test('script/style contents are ignored', () => {
  const text = '<script>const x = 1;</script><style>.a{color:red}</style>talo';
  const ranges = parseHtmlCode(text);
  assert.equal(covered(text, ranges, 'const'), true);
  assert.equal(covered(text, ranges, 'color'), true);
  assert.equal(covered(text, ranges, 'talo'), false);
});

test('comparison signs in prose are not HTML tags', () => {
  const text = 'jos x < y ja z > w niin talo';
  assert.deepEqual(parseHtmlCode(text), []);
});

test('bare URLs are ignored, including Finnish paths', () => {
  const text = 'katso https://example.fi/talo?q=koirra. sitten jatka';
  const ranges = parseUrlsAndEmails(text);
  assert.equal(covered(text, ranges, 'https://example.fi/talo?q=koirra'), true);
  assert.equal(covered(text, ranges, 'katso'), false);
  assert.equal(covered(text, ranges, 'jatka'), false);
  // The sentence full stop stays prose.
  assert.equal(isIgnored({ start: text.indexOf('. sitten'), end: text.indexOf('. sitten') + 1 }, ranges), false);
});

test('bare email addresses are ignored', () => {
  const text = 'mailaa matti.koirra@example.fi tai talo@example.fi nyt';
  const ranges = parseUrlsAndEmails(text);
  assert.equal(covered(text, ranges, 'matti.koirra@example.fi'), true);
  assert.equal(covered(text, ranges, 'talo@example.fi'), true);
  assert.equal(covered(text, ranges, 'mailaa'), false);
});

test('www links and trailing parentheses', () => {
  const text = 'www.example.fi/talo ja (https://example.fi/koirra)';
  const ranges = parseUrlsAndEmails(text);
  assert.equal(covered(text, ranges, 'www.example.fi/talo'), true);
  assert.equal(covered(text, ranges, 'https://example.fi/koirra'), true);
  assert.equal(isIgnored({ start: text.length - 1, end: text.length }, ranges), false);
});

test('adjacent and overlapping ranges coalesce', () => {
  assert.deepEqual(
    mergeRanges([
      { start: 5, end: 10 },
      { start: 0, end: 5 },
      { start: 3, end: 7 },
      { start: 12, end: 14 },
    ]),
    [{ start: 0, end: 10 }, { start: 12, end: 14 }],
  );
});

test('mask preserves UTF-16 length, newlines and non-ASCII offsets', () => {
  const text = 'äiti 😀\n[code]xyzzy[/code]\nkoirra';
  const ignored = computeIgnoredRanges(text);
  const masked = maskIgnored(text, ignored);
  assert.equal(masked.length, text.length);
  assert.equal(masked.slice(0, text.indexOf('[code]')), text.slice(0, text.indexOf('[code]')));
  assert.equal(masked.includes('\n'), true);
  // The code words are now spaces, but the prose after still occupies
  // the same offsets.
  const codeStart = text.indexOf('[code]');
  const codeEnd = text.indexOf('[/code]') + '[/code]'.length;
  const maskedCode = masked.slice(codeStart, codeEnd);
  assert.equal(maskedCode.length, codeEnd - codeStart);
  assert.match(maskedCode, /^[\s]+$/);
  assert.equal(masked.slice(text.indexOf('koirra')), 'koirra');
});

test('prose immediately around code keeps its offsets and checking', () => {
  const text = 'taloo `xyzzy` koirra';
  const ignored = computeIgnoredRanges(text);
  const masked = maskIgnored(text, ignored);
  assert.equal(masked.length, text.length);
  assert.equal(masked.indexOf('taloo'), 0);
  assert.equal(masked.indexOf('koirra'), text.indexOf('koirra'));
  assert.equal(isIgnored({ start: 0, end: 5 }, ignored), false);
  assert.equal(
    isIgnored({ start: text.indexOf('xyzzy'), end: text.indexOf('xyzzy') + 5 }, ignored),
    true,
  );
});

test('isIgnored treats partial overlap as ignored', () => {
  const ignored = [{ start: 10, end: 20 }];
  assert.equal(isIgnored({ start: 0, end: 5 }, ignored), false);
  assert.equal(isIgnored({ start: 5, end: 12 }, ignored), true);
  assert.equal(isIgnored({ start: 18, end: 25 }, ignored), true);
  assert.equal(isIgnored({ start: 20, end: 25 }, ignored), false);
});

test('DOM ancestor walk finds code inside the editor', () => {
  const root = { nodeType: 1, tagName: 'DIV', parentNode: null };
  const pre = { nodeType: 1, tagName: 'PRE', parentNode: root };
  const code = { nodeType: 1, tagName: 'CODE', parentNode: pre };
  const text = { nodeType: 3, parentNode: code };
  const prose = { nodeType: 3, parentNode: root };
  assert.equal(isInsideCodeDom(text, root), true);
  assert.equal(isInsideCodeDom(prose, root), false);
  assert.equal(isInsideCodeDom(null, root), false);
});
