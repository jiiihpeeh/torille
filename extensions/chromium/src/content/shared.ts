import { computeIgnoredRanges, type IgnoredRange } from './ignore-ranges';

// §1  Debug + browser shim
// ─────────────────────────────────────────────────────────────────────

// Enable with `localStorage.setItem('torille-debug', '1')` (per-page
// override) or by toggling the "Debug-loki" checkbox on the
// extension's options page (writes to `browser.storage.local` and
// propagates here via storage.onChanged).
let TORILLE_DEBUG: boolean = (() => {
  try { return localStorage.getItem('torille-debug') === '1'; }
  catch (_) { return false; }
})();
export function dbg(label: string, ...args: unknown[]): void {
  if (!TORILLE_DEBUG) return;
  const head = label ? `[torille:${label}]` : '[torille]';
  const fn = (args[0] instanceof Error) ? console.error : console.log;
  fn(head, ...args);
}
export function setDebugFlag(on: boolean): void { TORILLE_DEBUG = on; }

dbg('content', 'content.js loaded', { host: location.hostname });

/**
 * Fire `fn` on a tap (touch/pen) but not on a scroll or drag.
 * `touchend` is used rather than `click` so the popup can open
 * immediately, and `preventDefault()` suppresses the synthetic mouse
 * events that would otherwise open it a second time. Movement over
 * ~12px is treated as a scroll and leaves the gesture alone.
 */
export function addTapHandler(
  el: HTMLElement, fn: (x: number, y: number) => void,
): void {
  let tracking = false;
  let sx = 0;
  let sy = 0;
  el.addEventListener('touchstart', (event: Event) => {
    const e = event as TouchEvent;
    if (e.touches.length !== 1) {
      tracking = false;
      return;
    }
    sx = e.touches[0].clientX;
    sy = e.touches[0].clientY;
    tracking = true;
  }, { passive: true });
  el.addEventListener('touchcancel', () => {
    tracking = false;
  }, { passive: true });
  el.addEventListener('touchend', (event: Event) => {
    if (!tracking) return;
    tracking = false;
    const e = event as TouchEvent;
    const t = e.changedTouches[0];
    if (!t) return;
    if (Math.hypot(t.clientX - sx, t.clientY - sy) > 12) return;
    e.preventDefault();
    fn(t.clientX, t.clientY);
  }, { passive: false });
}

export const api: typeof browser = (typeof browser !== 'undefined') ? browser : chrome;

// Appearance/popup settings come from the options page. CSS custom
// properties are set on :root so they inherit through every shadow root;
// no overlay has to rebuild its stylesheet when a colour changes.
export const UI_SETTINGS = {
  underlineStyle: 'torille-underline-style',
  underlineColor: 'torille-underline-color',
  suggestionsMax: 'torille-suggestions-max',
  popupTrigger: 'torille-popup-trigger',
  popupBg: 'torille-popup-bg',
  popupFg: 'torille-popup-fg',
  popupAccent: 'torille-popup-accent',
  popupAccentFg: 'torille-popup-accent-fg',
  ignoreCodeBlocks: 'torille-ignore-code-blocks',
  ignoreInlineCode: 'torille-ignore-inline-code',
  ignoreBbcode: 'torille-ignore-bbcode',
  ignoreUrls: 'torille-ignore-urls',
  ignoreCodeEditors: 'torille-ignore-code-editors',
} as const;

export type UiSettings = {
  underlineStyle: string;
  underlineColor: string;
  suggestionsMax: number;
  popupTrigger: 'click' | 'hover';
  popupBg: string;
  popupFg: string;
  popupAccent: string;
  popupAccentFg: string;
  ignoreCodeBlocks: boolean;
  ignoreInlineCode: boolean;
  ignoreBbcode: boolean;
  ignoreUrls: boolean;
  ignoreCodeEditors: boolean;
};

export const uiSettings: UiSettings = {
  underlineStyle: 'wavy',
  underlineColor: '#cc3333',
  suggestionsMax: 8,
  popupTrigger: 'click',
  popupBg: '#ffffff',
  popupFg: '#222222',
  popupAccent: '#0077cc',
  popupAccentFg: '#ffffff',
  ignoreCodeBlocks: true,
  ignoreInlineCode: true,
  ignoreBbcode: true,
  ignoreUrls: true,
  ignoreCodeEditors: true,
};

function applyUiSettings(): void {
  const root = document.documentElement;
  root.style.setProperty('--torille-underline-line',
    uiSettings.underlineStyle === 'none' ? 'none' : 'underline');
  root.style.setProperty('--torille-underline-style', uiSettings.underlineStyle);
  root.style.setProperty('--torille-underline-color', uiSettings.underlineColor);
  root.style.setProperty('--torille-popup-bg', uiSettings.popupBg);
  root.style.setProperty('--torille-popup-fg', uiSettings.popupFg);
  root.style.setProperty('--torille-popup-accent', uiSettings.popupAccent);
  root.style.setProperty('--torille-popup-accent-fg', uiSettings.popupAccentFg);
}

export async function loadUiSettings(): Promise<void> {
  const cur = await api.storage.local.get(Object.values(UI_SETTINGS));
  const style = String(cur[UI_SETTINGS.underlineStyle] ?? 'wavy');
  const trigger = String(cur[UI_SETTINGS.popupTrigger] ?? 'click');
  const max = Number(cur[UI_SETTINGS.suggestionsMax] ?? 8);
  uiSettings.underlineStyle = ['wavy', 'solid', 'dashed', 'dotted', 'none'].includes(style)
    ? style : 'wavy';
  uiSettings.underlineColor = /^#[0-9a-f]{6}$/i.test(String(cur[UI_SETTINGS.underlineColor]))
    ? String(cur[UI_SETTINGS.underlineColor]) : '#cc3333';
  uiSettings.suggestionsMax = Number.isFinite(max)
    ? Math.min(20, Math.max(3, Math.round(max))) : 8;
  uiSettings.popupTrigger = trigger === 'hover' ? 'hover' : 'click';
  uiSettings.popupBg = /^#[0-9a-f]{6}$/i.test(String(cur[UI_SETTINGS.popupBg]))
    ? String(cur[UI_SETTINGS.popupBg]) : '#ffffff';
  uiSettings.popupFg = /^#[0-9a-f]{6}$/i.test(String(cur[UI_SETTINGS.popupFg]))
    ? String(cur[UI_SETTINGS.popupFg]) : '#222222';
  uiSettings.popupAccent = /^#[0-9a-f]{6}$/i.test(String(cur[UI_SETTINGS.popupAccent]))
    ? String(cur[UI_SETTINGS.popupAccent]) : '#0077cc';
  uiSettings.popupAccentFg = /^#[0-9a-f]{6}$/i.test(String(cur[UI_SETTINGS.popupAccentFg]))
    ? String(cur[UI_SETTINGS.popupAccentFg]) : '#ffffff';
  const bool = (key: string, fallback: boolean): boolean => {
    const value = cur[key];
    return typeof value === 'boolean' ? value : fallback;
  };
  uiSettings.ignoreCodeBlocks = bool(UI_SETTINGS.ignoreCodeBlocks, true);
  uiSettings.ignoreInlineCode = bool(UI_SETTINGS.ignoreInlineCode, true);
  uiSettings.ignoreBbcode = bool(UI_SETTINGS.ignoreBbcode, true);
  uiSettings.ignoreUrls = bool(UI_SETTINGS.ignoreUrls, true);
  uiSettings.ignoreCodeEditors = bool(UI_SETTINGS.ignoreCodeEditors, true);
  applyUiSettings();
}

/** Ignore ranges for the current content-script settings. */
export function ignoredRangesFor(text: string): IgnoredRange[] {
  return computeIgnoredRanges(text, {
    codeBlocks: uiSettings.ignoreCodeBlocks,
    inlineCode: uiSettings.ignoreInlineCode,
    bbcode: uiSettings.ignoreBbcode,
    html: uiSettings.ignoreCodeBlocks,
    urlsAndEmails: uiSettings.ignoreUrls,
  });
}

/** True when the field is a known whole-editor code surface. */
export function isCodeEditorField(el: HTMLElement): boolean {
  if (!uiSettings.ignoreCodeEditors) return false;
  let n: HTMLElement | null = el;
  for (let i = 0; n && i < 4; i++, n = n.parentElement) {
    const cls = typeof n.className === 'string' ? n.className : '';
    if (/\b(cm-editor|monaco-editor|CodeMirror|ace_editor)\b/.test(cls)) {
      return true;
    }
    if (n.hasAttribute('data-code-editor')) return true;
  }
  return false;
}

// ─────────────────────────────────────────────────────────────────────
// §2  Constants
// ─────────────────────────────────────────────────────────────────────

// Text inputs we attach to. `input` without an explicit type also
// implies text per HTML; url/search/email are the ones a Finnish
// user is most likely to type Finnish in. `[contenteditable]` catches
// rich-text editors like Gmail's compose window or Notion's pages.
export const TARGET_SELECTOR = [
  'textarea',
  'input[type="text"]',
  'input[type="search"]',
  'input[type="email"]',
  'input[type="url"]',
  'input:not([type])',
  '[contenteditable="true"]',
  '[contenteditable=""]',
  '[role="searchbox"]',
].join(', ');

// Search boxes are opt-in. People type URLs, product codes, names and
// half-remembered queries into them, all of which look like
// misspellings. The options page stores `torille-searchboxes`; the
// default (missing key) is off.
export function isSearchField(el: HTMLElement): boolean {
  if (el.getAttribute('role') === 'searchbox') return true;
  if (!(el instanceof HTMLInputElement)) return false;
  if (el.type === 'search' || el.getAttribute('inputmode') === 'search') {
    return true;
  }
  const id = el.id.toLowerCase();
  const name = el.name.toLowerCase();
  if (id === 'q' || name === 'q') return true;
  if (/(^|[-_])search($|[-_])/.test(id) || /(^|[-_])search($|[-_])/.test(name)) {
    return true;
  }
  const hint = (
    el.getAttribute('placeholder')
    || el.getAttribute('aria-label')
    || ''
  ).toLowerCase();
  return /\b(search|haku|etsi)\b/.test(hint);
}

// When debug logging is on, the overlay's text is also rendered with
// a low-opacity color so the user can visually verify the alignment
// of the mirror against the underlying textarea. Off by default.
//
// Toggle by setting this to `true` and rebuilding. The overlay's
// text shows in faint blue; the underline still shows in red on
// misspellings. If the blue text overlaps the textarea's text, the
// mirror is aligned. If it's offset, the box-sizing or font
// copying is wrong. Keep this off in builds users run: it paints a
// translucent duplicate of every input's text over the page.
const DEBUG_VISIBLE_OVERLAY = false;

// CSS Highlight API rule, used by the contenteditable path. The
// shadow-DOM mirror (textarea path) carries the same look via
// .misspell below; this is the canonical one for contenteditable.
//
// Note: a <style> appended to the document is enough -- Highlight
// pseudo-elements live in the top-level cascade, not inside a
// shadow root.
const HIGHLIGHT_STYLE_ID = 'torille-highlight-style';
export function ensureHighlightStyle(): void {
  if (document.getElementById(HIGHLIGHT_STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = HIGHLIGHT_STYLE_ID;
  style.textContent = `
    ::highlight(torille-misspell) {
      text-decoration-line: var(--torille-underline-line, underline);
      text-decoration-style: var(--torille-underline-style, wavy);
      text-decoration-color: var(--torille-underline-color, #c33);
      text-decoration-skip-ink: none;
      cursor: pointer;
    }
  `;
  document.head.appendChild(style);
}

// Styling lives inside an open shadow root so the page cannot reach
// it and we cannot reach the page's CSS.
export const OVERLAY_CSS = `
  :host { all: initial; }
  .mirror {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }
  .overlay {
    position: absolute;
    pointer-events: none;
    overflow: hidden;
    color: ${DEBUG_VISIBLE_OVERLAY ? 'rgba(0,128,255,0.35)' : 'transparent'};
    background: transparent;
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-wrap: break-word;
  }
  .token { display: inline; }
  .misspell {
    text-decoration-line: var(--torille-underline-line, underline);
    text-decoration-style: var(--torille-underline-style, wavy);
    text-decoration-color: var(--torille-underline-color, #c33);
    text-decoration-skip-ink: none;
    pointer-events: auto;
    cursor: pointer;
    touch-action: manipulation;
    -webkit-tap-highlight-color: transparent;
  }
  .suggestions {
    position: absolute;
    z-index: 2147483647;
    background: var(--torille-popup-bg, white);
    border: 1px solid #ccc;
    border-radius: 4px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.18);
    padding: 4px 0;
    font: 13px/1.4 -apple-system, system-ui, sans-serif;
    color: var(--torille-popup-fg, #222);
    pointer-events: auto;
    min-width: 8em;
    max-width: 22em;
    max-height: 14em;
    overflow-y: auto;
  }
  .suggestions button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    padding: 5px 12px;
    cursor: pointer;
    font: inherit;
    color: inherit;
    touch-action: manipulation;
    -webkit-tap-highlight-color: transparent;
  }
  .suggestions button:hover,
  .suggestions button.focused {
    background: var(--torille-popup-accent, #07c);
    color: var(--torille-popup-accent-fg, white);
  }
  .suggestions .empty {
    padding: 5px 12px;
    color: #888;
    font-style: italic;
  }
`;

// ─────────────────────────────────────────────────────────────────────
// §3  Input descriptor
// ─────────────────────────────────────────────────────────────────────

// Stable identity for a text input: enough to recognise the same
// element across log lines and reconnect it to a name in the DOM
// tree. The full element reference and its position only matter to
// a debugger anyway.
export function describeInput(el: HTMLElement | null): string | null {
  if (!el) return null;
  const tag = el.tagName ? el.tagName.toLowerCase() : '?';
  // type / name / placeholder are only present on HTMLInputElement
  // and HTMLTextAreaElement, not on the bare HTMLElement. Treat the
  // lookup defensively so contenteditable elements still describe.
  const inputLike = el as Partial<HTMLInputElement & HTMLTextAreaElement>;
  const type = inputLike.type ? ` type=${inputLike.type}` : '';
  const id = el.id ? ` id=${el.id}` : '';
  const name = inputLike.name ? ` name=${inputLike.name}` : '';
  const placeholder = inputLike.placeholder
    ? ` placeholder="${inputLike.placeholder.slice(0, 20)}"`
    : '';
  const cls = (() => {
    try { return el.getAttribute('class') || ''; }
    catch (_) { return ''; }
  })();
  const classTag = cls ? ` class="${cls.split(/\s+/).slice(0, 3).join('.')}"` : '';
  return `<${tag}${type}${id}${name}${placeholder}${classTag}>`;
}

// ─────────────────────────────────────────────────────────────────────
// §3b Native spellcheck suppression
// ─────────────────────────────────────────────────────────────────────

// We turn the browser's own checker off while our overlay is
// active, but the attribute belongs to the page: a blacklist or a
// per-textbox toggle must give the site back exactly what it had.
// Keep the original value (including "absent") and restore it.
export type NativeSpellAttrs = {
  spellcheck: string | null;
  autocorrect: string | null;
  autocapitalize: string | null;
};

export function disableNativeSpellcheck(el: HTMLElement): void {
  el.setAttribute('spellcheck', 'false');
  el.setAttribute('autocorrect', 'off');
  el.setAttribute('autocapitalize', 'off');
}

export function saveAndDisableNativeSpellcheck(el: HTMLElement): NativeSpellAttrs {
  const saved: NativeSpellAttrs = {
    spellcheck: el.getAttribute('spellcheck'),
    autocorrect: el.getAttribute('autocorrect'),
    autocapitalize: el.getAttribute('autocapitalize'),
  };
  disableNativeSpellcheck(el);
  return saved;
}

export function restoreNativeSpellcheck(el: HTMLElement, saved: NativeSpellAttrs): void {
  const attrs: (keyof NativeSpellAttrs)[] =
    ['spellcheck', 'autocorrect', 'autocapitalize'];
  for (const name of attrs) {
    const value = saved[name];
    if (value === null) el.removeAttribute(name);
    else el.setAttribute(name, value);
  }
}

export type DebugFn = typeof dbg;
export type DescribeInput = typeof describeInput;
