// Tokeniser for the demo editor.
//
// Splits text into word and non-word runs. Finnish word chars include
// `ä`, `ö`, `å` and the hyphen-minus (for split compounds like
// `nuori-isäntä`). The regex is intentionally permissive; the wasm
// side decides what is and isn't a word.

export type Run =
  | { kind: 'word'; text: string; start: number; end: number }
  | { kind: 'sep'; text: string };

// The regex has three alternatives -- word-with-hyphen, word, and a
// separator catch-all -- but every match goes through the same
// capture group, so we cannot tell from the match object alone which
// alternative fired. The third alternative matches anything that is
// not a Finnish/ASCII letter, so we classify a match as a `word` if
// and only if it is a run of letters (with an optional trailing
// hyphen), and a `sep` otherwise.
//
// `[\p{L}]` matches any Unicode letter (so `ö`, `ä`, `å` are not
// lost); `[^\W\d_]` only matches ASCII letters and would silently
// drop the leading `ö` of `öihin`.
const TOKEN_RE = /([\p{L}]+-?|[\p{L}]+|[^a-zA-ZäöÄÖåÅ]+)/gu;
const WORD_TEST = /^[\p{L}-]+$/u;

export function tokenize(text: string): Run[] {
  const runs: Run[] = [];
  let i = 0;
  for (const m of text.matchAll(TOKEN_RE)) {
    const at = m.index ?? i;
    if (at > i) runs.push({ kind: 'sep', text: text.slice(i, at) });
    const matched = m[0];
    runs.push({
      kind: WORD_TEST.test(matched) ? 'word' : 'sep',
      text: matched,
      start: at,
      end: at + matched.length,
    });
    i = at + matched.length;
  }
  if (i < text.length) runs.push({ kind: 'sep', text: text.slice(i) });
  return runs;
}

export function countWords(text: string): number {
  let n = 0;
  for (const tok of tokenize(text)) if (tok.kind === 'word') n++;
  return n;
}