// Ignore ranges for code blocks, markup and markup payloads.
//
// The engine has no idea which parts of an attached field are prose and
// which are code. This module is the content-script-side answer: it
// finds the regions the checker must not inspect, merges them, and can
// replace them with a length-preserving mask before the text reaches
// `analyze-text`/`update-state`.
//
// Everything here works on JavaScript string indices (UTF-16 code
// units), which is exactly what the wasm binding reports after its
// byte-offset conversion. Never count codepoints: `ä`, emoji and CRLF
// would shift every later offset.

export type IgnoredRange = { start: number; end: number };

export type IgnoreOptions = {
  /** ```` ``` ```` / `~~~` fenced blocks and raw HTML code elements. */
  codeBlocks?: boolean;
  /** Paired backtick spans on a single line. */
  inlineCode?: boolean;
  /** BBCode tags plus code/URL/image/email payloads. */
  bbcode?: boolean;
  /** Raw HTML tag syntax and code-like element contents. */
  html?: boolean;
  /** Bare URLs and email addresses in prose. */
  urlsAndEmails?: boolean;
};

const DEFAULT_OPTIONS: Required<IgnoreOptions> = {
  codeBlocks: true,
  inlineCode: true,
  bbcode: true,
  html: true,
  urlsAndEmails: true,
};

// BBCode whose *contents* are not prose. `[quote]`, `[b]`, `[list]` and
// friends keep their text; only their tag syntax is masked.
const BBCODE_CODE_TAGS = new Set([
  'code', 'icode', 'php', 'html', 'sql', 'xml',
]);
const BBCODE_OPAQUE_TAGS = new Set([
  'url', 'img', 'email', 'media',
]);

// HTML elements whose text is code, not prose. Inline formatting tags
// (`<b>`, `<a>`, `<em>`, ...) only have their tag syntax masked.
const HTML_CODE_TAGS = new Set([
  'code', 'pre', 'kbd', 'samp', 'script', 'style',
]);

// Tag names we are willing to treat as HTML in a plain-text field. The
// allow-list keeps prose like `x < y and z > w` from being mistaken for
// a tag. Raw HTML in a textarea is an unusual input; a false negative
// here is much cheaper than masking entire prose sentences.
const HTML_KNOWN_TAGS = new Set([
  'a', 'abbr', 'address', 'article', 'aside', 'audio', 'b', 'bdi', 'bdo',
  'blockquote', 'br', 'button', 'canvas', 'caption', 'cite', 'code',
  'col', 'colgroup', 'data', 'dd', 'del', 'details', 'dfn', 'dialog',
  'div', 'dl', 'dt', 'em', 'figcaption', 'figure', 'footer', 'form',
  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'header', 'hr', 'i', 'iframe',
  'img', 'input', 'ins', 'kbd', 'label', 'legend', 'li', 'link', 'main',
  'mark', 'menu', 'meta', 'nav', 'noscript', 'object', 'ol', 'optgroup',
  'option', 'output', 'p', 'picture', 'pre', 'q', 'rp', 'rt', 'ruby',
  's', 'samp', 'script', 'section', 'select', 'small', 'source', 'span',
  'strong', 'style', 'sub', 'summary', 'sup', 'table', 'tbody', 'td',
  'template', 'textarea', 'tfoot', 'th', 'thead', 'time', 'title', 'tr',
  'track', 'u', 'ul', 'var', 'video', 'wbr',
]);

/**
 * Merge overlapping or touching ranges into sorted, disjoint ranges.
 * The input is not mutated.
 */
export function mergeRanges(ranges: IgnoredRange[]): IgnoredRange[] {
  if (ranges.length <= 1) return ranges.map((r) => ({ ...r }));
  const sorted = ranges
    .map((r) => ({ start: Math.max(0, r.start), end: Math.max(0, r.end) }))
    .filter((r) => r.end > r.start)
    .sort((a, b) => a.start - b.start || a.end - b.end);
  const out: IgnoredRange[] = [];
  for (const r of sorted) {
    const last = out[out.length - 1];
    if (last && r.start <= last.end) {
      if (r.end > last.end) last.end = r.end;
    } else {
      out.push(r);
    }
  }
  return out;
}

/** True when `range` overlaps any ignored range. */
export function isIgnored(
  range: IgnoredRange, ignored: IgnoredRange[],
): boolean {
  // Binary search for the last range starting at or before `range.end`.
  let lo = 0;
  let hi = ignored.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const r = ignored[mid];
    if (r.start >= range.end) {
      hi = mid - 1;
    } else if (r.end <= range.start) {
      lo = mid + 1;
    } else {
      return true;
    }
  }
  return false;
}

/**
 * Replace ignored code units with spaces while preserving the string
 * length and any line breaks. A mask is safe to hand to the engine:
 * every hit offset still points at the same character in the original.
 */
export function maskIgnored(text: string, ignored: IgnoredRange[]): string {
  if (ignored.length === 0) return text;
  let out = '';
  let cursor = 0;
  for (const range of ignored) {
    const start = Math.max(cursor, Math.min(text.length, range.start));
    const end = Math.max(start, Math.min(text.length, range.end));
    if (start > cursor) out += text.slice(cursor, start);
    const piece = text.slice(start, end);
    out += piece.replace(/[^\r\n]/g, ' ');
    cursor = end;
  }
  if (cursor < text.length) out += text.slice(cursor);
  return out;
}

/** True when `index` is preceded by an odd number of backslashes. */
function isEscaped(text: string, index: number): boolean {
  let n = 0;
  for (let i = index - 1; i >= 0 && text[i] === '\\'; i--) n++;
  return n % 2 === 1;
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Find the matching close tag for a BBCode opener, honouring nesting of
 * the same tag name. Returns the close tag's start/end or null.
 */
function findBbcodeClose(
  text: string, name: string, from: number,
): IgnoredRange | null {
  const re = new RegExp(
    `\\[(\\/?)${escapeRegExp(name)}(?:=[^\\]\\n]*|\\s[^\\]\\n]*)?\\]`,
    'gi',
  );
  re.lastIndex = from;
  let depth = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m[1] === '/') {
      if (depth === 0) {
        return { start: m.index, end: m.index + m[0].length };
      }
      depth--;
    } else {
      depth++;
    }
  }
  return null;
}

/**
 * Mask BBCode. All tag syntax is ignored (`[quote]`, `[color=red]`, ...);
 * the contents of code-like and URL/image/email/media tags are ignored
 * too. Prose tags keep their text checkable.
 */
export function parseBbcode(text: string): IgnoredRange[] {
  if (!text.includes('[')) return [];
  const ranges: IgnoredRange[] = [];
  const tag = /\[(\/?)([A-Za-z][A-Za-z0-9*]*)((?:=[^\]\n]*|\s[^\]\n]*)?)\]/g;
  let i = 0;
  while (i < text.length) {
    tag.lastIndex = i;
    const m = tag.exec(text);
    if (m === null) break;
    const tagStart = m.index;
    const tagEnd = m.index + m[0].length;
    if (isEscaped(text, tagStart)) {
      i = tagEnd;
      continue;
    }
    const name = m[2].toLowerCase();
    if (m[1] === '/') {
      ranges.push({ start: tagStart, end: tagEnd });
      i = tagEnd;
      continue;
    }
    if (BBCODE_CODE_TAGS.has(name) || BBCODE_OPAQUE_TAGS.has(name)) {
      const close = findBbcodeClose(text, name, tagEnd);
      if (close === null) {
        // Unclosed code-like tag: ignore the rest rather than risk
        // spellchecking a code payload as prose.
        ranges.push({ start: tagStart, end: text.length });
        break;
      }
      ranges.push({ start: tagStart, end: close.end });
      i = close.end;
      continue;
    }
    ranges.push({ start: tagStart, end: tagEnd });
    i = tagEnd;
  }
  return ranges;
}

type LineInfo = { start: number; end: number; text: string };

/** Split into lines with offsets; `end` excludes the newline. */
function lines(text: string): LineInfo[] {
  const out: LineInfo[] = [];
  let start = 0;
  for (let i = 0; i <= text.length; i++) {
    if (i === text.length || text[i] === '\n') {
      const end = i;
      out.push({ start, end, text: text.slice(start, end).replace(/\r$/, '') });
      start = i + 1;
    }
  }
  return out;
}

/** Markdown fenced code blocks (``` and ~~~), including their fences. */
export function parseFencedCode(text: string): IgnoredRange[] {
  if (!text.includes('```') && !text.includes('~~~')) return [];
  const ranges: IgnoredRange[] = [];
  const ls = lines(text);
  const open = /^( {0,3})(`{3,}|~{3,})(.*)$/;
  for (let i = 0; i < ls.length; i++) {
    const m = open.exec(ls[i].text);
    if (m === null) continue;
    const fence = m[2];
    const ch = fence[0];
    const len = fence.length;
    let close = -1;
    for (let j = i + 1; j < ls.length; j++) {
      const c = ls[j].text;
      const cm = /^( {0,3})(`{3,}|~{3,})[ \t]*$/.exec(c);
      if (cm === null) continue;
      if (cm[2][0] === ch && cm[2].length >= len) {
        close = j;
        break;
      }
    }
    if (close === -1) {
      // Unclosed fence: the rest is inside the block.
      ranges.push({ start: ls[i].start, end: text.length });
      break;
    }
    ranges.push({ start: ls[i].start, end: ls[close].end });
    i = close;
  }
  return ranges;
}

/** Paired single-line backtick spans; unmatched backticks are prose. */
export function parseInlineCode(text: string): IgnoredRange[] {
  if (!text.includes('`')) return [];
  const ranges: IgnoredRange[] = [];
  for (const line of lines(text)) {
    const s = line.text;
    let i = 0;
    while (i < s.length) {
      if (s[i] !== '`' || isEscaped(s, i)) {
        i++;
        continue;
      }
      let run = 1;
      while (i + run < s.length && s[i + run] === '`') run++;
      let j = i + run;
      let found = -1;
      while (j < s.length) {
        if (s[j] !== '`') {
          j++;
          continue;
        }
        let closeRun = 1;
        while (j + closeRun < s.length && s[j + closeRun] === '`') closeRun++;
        if (closeRun === run) {
          found = j;
          break;
        }
        j += closeRun;
      }
      if (found === -1) {
        i += run;
        continue;
      }
      ranges.push({
        start: line.start + i,
        end: line.start + found + run,
      });
      i = found + run;
    }
  }
  return ranges;
}

/**
 * Find the matching close for a raw HTML code-like element, honouring
 * nesting. Returns the close tag's start/end or null.
 */
function findHtmlClose(
  text: string, name: string, from: number,
): IgnoredRange | null {
  const re = new RegExp(`<\\s*(\\/?)${escapeRegExp(name)}\\b[^>]*>`, 'gi');
  re.lastIndex = from;
  let depth = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const selfClosing = /\/>$/.test(m[0]) ||
      /^(script|style)$/i.test(name) && m[1] === '';
    if (m[1] === '/') {
      if (depth === 0) {
        return { start: m.index, end: m.index + m[0].length };
      }
      depth--;
    } else if (!selfClosing) {
      depth++;
    }
  }
  return null;
}

/**
 * Mask raw HTML. Code-like element contents are ignored; all other
 * known tag syntax is masked but their text stays prose.
 */
export function parseHtmlCode(text: string): IgnoredRange[] {
  if (!text.includes('<')) return [];
  const ranges: IgnoredRange[] = [];
  const re = /<(\/?)([A-Za-z][A-Za-z0-9:-]*)(?:\s[^<>]*?)?\s*\/?>/g;
  let i = 0;
  while (i < text.length) {
    re.lastIndex = i;
    const m = re.exec(text);
    if (m === null) break;
    const tagStart = m.index;
    const tagEnd = m.index + m[0].length;
    const name = m[2].toLowerCase();
    if (!HTML_KNOWN_TAGS.has(name)) {
      i = tagEnd;
      continue;
    }
    if (m[1] === '/') {
      ranges.push({ start: tagStart, end: tagEnd });
      i = tagEnd;
      continue;
    }
    if (HTML_CODE_TAGS.has(name)) {
      if (/\/>$/.test(m[0])) {
        ranges.push({ start: tagStart, end: tagEnd });
        i = tagEnd;
        continue;
      }
      const close = findHtmlClose(text, name, tagEnd);
      if (close === null) {
        ranges.push({ start: tagStart, end: text.length });
        break;
      }
      ranges.push({ start: tagStart, end: close.end });
      i = close.end;
      continue;
    }
    ranges.push({ start: tagStart, end: tagEnd });
    i = tagEnd;
  }

  // HTML comments are markup too.
  const comment = /<!--[\s\S]*?(?:-->|$)/g;
  let cm: RegExpExecArray | null;
  while ((cm = comment.exec(text)) !== null) {
    ranges.push({ start: cm.index, end: cm.index + cm[0].length });
  }
  return ranges;
}

/**
 * Bare URLs and email addresses. Links are never Finnish prose, and the
 * tokeniser would otherwise split `https://example.fi/talo` into words
 * and underline the path. Trailing sentence punctuation is left out so
 * the full stop after a link still belongs to the sentence.
 */
export function parseUrlsAndEmails(text: string): IgnoredRange[] {
  if (!text.includes('://') && !text.includes('www.') && !text.includes('@')) {
    return [];
  }
  const ranges: IgnoredRange[] = [];
  const url = /\b(?:https?:\/\/|ftp:\/\/|www\.)[^\s<>"'`]+/gi;
  let m: RegExpExecArray | null;
  while ((m = url.exec(text)) !== null) {
    let end = m.index + m[0].length;
    // Drop sentence punctuation and an unmatched closing bracket.
    while (end > m.index && /[.,;:!?]/.test(text[end - 1])) end--;
    if (text[end - 1] === ')') {
      const slice = text.slice(m.index, end);
      const opens = (slice.match(/\(/g) ?? []).length;
      const closes = (slice.match(/\)/g) ?? []).length;
      if (closes > opens) end--;
    }
    if (end > m.index) ranges.push({ start: m.index, end });
  }
  const email = /[\p{L}\p{N}._%+-]+@[\p{L}\p{N}.-]+\.[\p{L}]{2,}/gu;
  while ((m = email.exec(text)) !== null) {
    ranges.push({ start: m.index, end: m.index + m[0].length });
  }
  return ranges;
}

/** Compute the merged ignore mask for a whole text. */
export function computeIgnoredRanges(
  text: string, options: IgnoreOptions = {},
): IgnoredRange[] {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  const ranges: IgnoredRange[] = [];
  if (opts.codeBlocks) ranges.push(...parseFencedCode(text));
  if (opts.inlineCode) ranges.push(...parseInlineCode(text));
  if (opts.bbcode) ranges.push(...parseBbcode(text));
  if (opts.html) ranges.push(...parseHtmlCode(text));
  if (opts.urlsAndEmails) ranges.push(...parseUrlsAndEmails(text));
  return mergeRanges(ranges);
}

/** HTML elements whose contents are never prose. */
export const CODE_DOM_TAGS: ReadonlySet<string> = new Set([
  'CODE', 'PRE', 'KBD', 'SAMP', 'SCRIPT', 'STYLE',
]);

/**
 * True when `node` is inside a code-like element between it and
 * `editorRoot` (inclusive). Used by the contenteditable path, where the
 * DOM already knows what is code.
 */
export function isInsideCodeDom(
  node: Node | null, editorRoot: Element,
): boolean {
  let n: Node | null = node;
  while (n) {
    if (n.nodeType === 1) {
      const tag = (n as Element).tagName.toUpperCase();
      if (CODE_DOM_TAGS.has(tag)) return true;
    }
    if (n === editorRoot) return false;
    n = n.parentNode;
  }
  return false;
}
