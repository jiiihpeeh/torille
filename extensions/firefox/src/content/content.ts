// Content script: discovers text inputs on the page, disables the
// browser's built-in spellcheck (`spellcheck="false"`), and renders
// torille's own red-wavy underline + suggestion popup on top of each
// one. The wasm itself runs in a module worker (`spell-worker.js`)
// so the main thread is never blocked by `find_bases`.
//
// Each text input gets a shadow-DOM overlay positioned exactly over
// it. The shadow root is open, so the page's CSS cannot leak in and
// our CSS cannot leak out -- underlines and the suggestion popup keep
// their style regardless of what styles the page applies to the
// textarea.
//
// File layout (single file, no build step):
//   §1  Debug + browser shim
//   §2  Constants (overlay CSS, selector)
//   §3  Input descriptor           (describeInput)
//   §3b Native spellcheck suppression
//   §4  Worker client              (checkFastAsync / analyzeTextAsync +
//                                  suggestWithStateAsync via runtime)
//   §5  Overlay factory            (createOverlay, render)
//   §6  Blacklist factory          (loadBlacklist, refreshOverlays)
//   §7  Per-textbox toggle factory (pendingInput, setOverlayVisible)
//   §8  Scanner + MutationObserver (scan, observe)
//   §9  Bootstrap                  (wire the factories together)

import {
  computeIgnoredRanges,
  isIgnored,
  isInsideCodeDom,
  maskIgnored,
  type IgnoredRange,
} from './ignore-ranges';

(() => {
  'use strict';

  // ─────────────────────────────────────────────────────────────────────
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
  function dbg(label: string, ...args: unknown[]): void {
    if (!TORILLE_DEBUG) return;
    const head = label ? `[torille:${label}]` : '[torille]';
    const fn = (args[0] instanceof Error) ? console.error : console.log;
    fn(head, ...args);
  }
  dbg('content', 'content.js loaded', { host: location.hostname });

  /**
   * Fire `fn` on a tap (touch/pen) but not on a scroll or drag.
   * `touchend` is used rather than `click` so the popup can open
   * immediately, and `preventDefault()` suppresses the synthetic mouse
   * events that would otherwise open it a second time. Movement over
   * ~12px is treated as a scroll and leaves the gesture alone.
   */
  function addTapHandler(
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

  const api: typeof browser = (typeof browser !== 'undefined') ? browser : chrome;

  // Appearance/popup settings come from the options page. CSS custom
  // properties are set on :root so they inherit through every shadow root;
  // no overlay has to rebuild its stylesheet when a colour changes.
  const UI_SETTINGS = {
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

  type UiSettings = {
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

  const uiSettings: UiSettings = {
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

  async function loadUiSettings(): Promise<void> {
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
  function ignoredRangesFor(text: string): IgnoredRange[] {
    return computeIgnoredRanges(text, {
      codeBlocks: uiSettings.ignoreCodeBlocks,
      inlineCode: uiSettings.ignoreInlineCode,
      bbcode: uiSettings.ignoreBbcode,
      html: uiSettings.ignoreCodeBlocks,
      urlsAndEmails: uiSettings.ignoreUrls,
    });
  }

  /** True when the field is a known whole-editor code surface. */
  function isCodeEditorField(el: HTMLElement): boolean {
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
  const TARGET_SELECTOR = [
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
  function isSearchField(el: HTMLElement): boolean {
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
  function ensureHighlightStyle(): void {
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
  const OVERLAY_CSS = `
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
  function describeInput(el: HTMLElement | null): string | null {
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
  type NativeSpellAttrs = {
    spellcheck: string | null;
    autocorrect: string | null;
    autocapitalize: string | null;
  };

  function disableNativeSpellcheck(el: HTMLElement): void {
    el.setAttribute('spellcheck', 'false');
    el.setAttribute('autocorrect', 'off');
    el.setAttribute('autocapitalize', 'off');
  }

  function saveAndDisableNativeSpellcheck(el: HTMLElement): NativeSpellAttrs {
    const saved: NativeSpellAttrs = {
      spellcheck: el.getAttribute('spellcheck'),
      autocorrect: el.getAttribute('autocorrect'),
      autocapitalize: el.getAttribute('autocapitalize'),
    };
    disableNativeSpellcheck(el);
    return saved;
  }

  function restoreNativeSpellcheck(el: HTMLElement, saved: NativeSpellAttrs): void {
    const attrs: (keyof NativeSpellAttrs)[] =
      ['spellcheck', 'autocorrect', 'autocapitalize'];
    for (const name of attrs) {
      const value = saved[name];
      if (value === null) el.removeAttribute(name);
      else el.setAttribute(name, value);
    }
  }

  // ─────────────────────────────────────────────────────────────────────
  // §4  Worker client
  // ─────────────────────────────────────────────────────────────────────

  // One module worker per page. It hosts the wasm and answers
  // check/suggest requests tagged with a numeric id so we can match
  // responses to callers.
  // MV3 worker plumbing: the wasm lives in the background script (which
  // is in the extension's own origin and *can* construct Workers against
  // `chrome-extension://` URLs -- content scripts can't, because they
  // share the page's origin). The content script asks the background for
  // `check` / `suggest` via `runtime.sendMessage`. The wasm worker is
  // shared across all tabs, so the ~150 ms boot cost is paid once.
  //
  // `inputId` is a small monotonic number assigned per textarea /
  // contenteditable on this page. It is auto-injected into every
  // batch payload (and verified on the response) so the renderer
  // never accidentally accepts a response meant for a different
  // input. Single-word ops don't carry the id -- they're used by the
  // per-click popup flow, which only asks for one word and never
  // races with another overlay.
  function createWorkerClient(opts: { dbg: typeof dbg; inputId: number }) {
    const o = opts;
    const { inputId } = o;

    function send<R>(payload: unknown): Promise<R | null> {
      return new Promise((resolve) => {
        try {
          const p = api.runtime.sendMessage(payload);
          if (p && typeof p.then === 'function') {
            p.then(resolve, (e: unknown) => {
              o.dbg('content', 'sendMessage rejected',
                    { inputId, err: String(e) });
              resolve(null);
            });
          } else {
            // Older Firefox (MV2) returns `undefined` from sendMessage
            // when the response isn't a promise; fall back to a default.
            resolve(null);
          }
        } catch (e) {
          o.dbg('content', 'sendMessage threw', { inputId, err: String(e) });
          resolve(null);
        }
      });
    }

    // One wasm-side slot for this overlay's lifetime. We
    // self-manage the index here so renderers and the popup's
    // suggest path share the same proper-noun set without having
    // to thread the idx through their signatures.
    //
    // `stateIdxP` is a memoised Promise so concurrent callers
    // awaiting the same idx don't fire two `create-state` round
    // trips. On failure (worker down) the cached promise stays
    // resolved to `null` and subsequent calls keep that null.
    let stateIdxP: Promise<number | null> | null = null;
    function stateIdx(): Promise<number | null> {
      if (stateIdxP === null) {
        stateIdxP = (async (): Promise<number | null> => {
          const r = await send<R_CreateStateResponse>({
            type: 'torille:create-state',
          });
          return typeof r?.idx === 'number' ? r.idx : null;
        })();
      }
      return stateIdxP;
    }

    // Fire-and-forget: when the overlay dies, the slot is freed.
    // The scanner doesn't expose a detach hook today; the slot
    // therefore stays alive until the worker is terminated (tab
    // close), bounded by the number of textareas seen in a
    // session.
    function tearDownStateFn(): void {
      stateIdx().then((idx) => {
        if (idx !== null) {
          api.runtime.sendMessage({
            type: 'torille:destroy-state', idx,
          } as R_DestroyState).catch(() => {});
        }
      }).catch(() => {});
    }

    return {
      inputId,
      async checkAsync(word: string): Promise<boolean | null> {
        const r = await send<R_CheckResponse>({ type: 'torille:check', word });
        // `null` = unavailable. Do not turn a dead worker into a
        // spelling verdict.
        return r && typeof r.ok === 'boolean' ? r.ok : null;
      },
      async checkFastAsync(word: string): Promise<boolean | null> {
        // First-pass verdict against this overlay's state (~1 ms).
        // Used by the contenteditable popup instead of `checkAsync`,
        // whose engine fallback can block the worker for seconds.
        const idx = await stateIdx();
        if (idx === null) return null;
        const r = await send<R_CheckFastResponse>({
          type: 'torille:check-fast', idx, word,
        });
        return r && typeof r.ok === 'boolean' ? r.ok : null;
      },
      // Batch counterpart of `checkAsync`. One runtime.sendMessage per
      // render instead of one per word. The result array is parallel
      // to the input; the caller zips it back to its own state. `gen`
      // is the caller's render generation; the response echoes it so
      // a slow batch can be discarded by the caller without using the
      // values.
      //
      // The `inputId` is injected here so every overlay's worker
      // client stamps its own identity on outgoing batch traffic
      // without the renderer having to remember. A response that
      // comes back with a different `inputId` is treated as a routing
      // error and discarded.
      async checkBatchAsync(
        words: string[], gen: number,
      ): Promise<(boolean | null)[] | null> {
        if (words.length === 0) return [];
        const r = await send<R_CheckBatchResponse>({
          type: 'torille:check-batch', inputId, gen, items: words,
        });
        if (!r || !Array.isArray(r.ok)) return null;
        if (r.inputId !== inputId) {
          o.dbg('content', 'checkBatchAsync:inputId-mismatch',
                { want: inputId, got: r.inputId });
          return null;
        }
        return r.ok;
      },
      async suggestAsync(word: string, max: number): Promise<string[]> {
        const r = await send<R_SuggestResponse>({ type: 'torille:suggest', word, max });
        return Array.isArray(r?.suggestions) ? r.suggestions : [];
      },
      // Batch counterpart of `suggestAsync`. Used by the render layer
      // if it wants to fetch suggestions for several misspellings in
      // one go (e.g. on a contenteditable refresh). The popup's per-
      // click flow still uses `suggestAsync` since it only ever asks
      // for one word.
      async suggestBatchAsync(
        items: { word: string; max: number }[], gen: number,
      ): Promise<string[][] | null> {
        if (items.length === 0) return [];
        const r = await send<R_SuggestBatchResponse>({
          type: 'torille:suggest-batch', inputId, gen, items,
        });
        if (!r || !Array.isArray(r.results)) return null;
        if (r.inputId !== inputId) {
          o.dbg('content', 'suggestBatchAsync:inputId-mismatch',
                { want: inputId, got: r.inputId });
          return null;
        }
        return r.results;
      },
      // ─── per-textbox state lifecycle ────────────────────────────
      //
      // The render path. `createStateAsync` returns the wasm-side
      // slot index the content script holds for the lifetime of
      // this overlay. `destroyStateAsync` on detach returns the
      // slot to the pool. Every render fires `updateState` then
      // `analyzeText` -- one wasm round-trip per render regardless
      // of token count.
      async createStateAsync(): Promise<number | null> {
        return stateIdx();
      },
      destroyStateAsync(idx: number): void {
        // Fire-and-forget; there's no reply envelope.
        api.runtime.sendMessage({
          type: 'torille:destroy-state', idx,
        } as R_DestroyState).catch(() => { /* background down -- ignore */ });
      },
      clearStateAsync(idx: number): void {
        api.runtime.sendMessage({
          type: 'torille:clear-state', idx,
        } as R_ClearState).catch(() => {});
      },
      updateStateAsync(idx: number, text: string): void {
        api.runtime.sendMessage({
          type: 'torille:update-state', idx, text,
        } as R_UpdateState).catch(() => {});
      },
      async analyzeTextAsync(
        text: string, max: number, gen: number,
      ): Promise<R_AnalyzeHit[] | null> {
        const ask = async (idx: number): Promise<R_AnalyzeTextResponse | null> => {
          // The proper-noun update is fired ahead of analyze so the
          // wasm has the latest text in hand. analyze-text awaits in
          // the same render, so the renderer's wall-clock cost is
          // still one round-trip.
          api.runtime.sendMessage({
            type: 'torille:update-state', idx, text,
          } as R_UpdateState).catch(() => {});
          return send<R_AnalyzeTextResponse>({
            type: 'torille:analyze-text', inputId, gen, idx, text, max,
          });
        };

        let idx = await stateIdx();
        if (idx === null) return null;
        let r = await ask(idx);
        if (r && r.inputId === inputId && r.stateValid === false) {
          // The wasm worker restarted and lost every per-textbox slot.
          // Allocate a fresh one and retry once; otherwise the render
          // sees an empty hit list and the underlines silently vanish.
          o.dbg('content', 'state stale; recreating', { inputId, idx });
          stateIdxP = null;
          idx = await stateIdx();
          if (idx === null) return null;
          r = await ask(idx);
        }
        if (!r || !Array.isArray(r.hits)) return null;
        if (r.inputId !== inputId) return null;
        return r.hits;
      },
      // Popup's per-click suggest. Uses the per-textbox proper-noun
      // set so words the user has typed before surface as
      // candidates when the popup shows up.
      async suggestWithStateAsync(
        word: string, max: number,
      ): Promise<string[]> {
        const idx = await stateIdx();
        if (idx === null) return [];
        const r = await send<R_SuggestWithStateResponse>({
          type: 'torille:suggest-with-state', idx, word, max,
        });
        return Array.isArray(r?.suggestions) ? r.suggestions : [];
      },
      // On-demand grammar check for the context menu. The worker
      // converts byte offsets to UTF-16 before returning.
      async checkSyntaxAsync(text: string): Promise<R_SyntaxDiagnostic[] | null> {
        const idx = await stateIdx();
        if (idx === null) return null;
        const r = await send<R_CheckSyntaxResponse>({
          type: 'torille:check-syntax', idx, text,
        });
        return Array.isArray(r?.diagnostics) ? r.diagnostics : null;
      },
      // Free the wasm-side slot. Called from the scanner's
      // detach path so a per-overlay slot doesn't outlive the
      // overlay.
      tearDownState(): void { tearDownStateFn(); },
      setDebug(on: boolean): void {
        // Fire-and-forget; the background's worker picks up the flag
        // immediately.
        const m: R_Debug = { type: 'torille:debug', on };
        api.runtime.sendMessage(m);
      },
    };
  }

  // ─────────────────────────────────────────────────────────────────────
  // §5  Overlay factory
  // ─────────────────────────────────────────────────────────────────────

  // Build the shadow-DOM overlay for one text input, plus the
  // suggestion popup, the debounced render, and the popup's open /
  // close / click-outside plumbing. The factory returns a small API
  // the boot section wires into the scanner and the toggle.
  function createOverlay(opts: {
    el: HTMLElement;
    workerClient: ReturnType<typeof createWorkerClient>;
    dbg: typeof dbg;
    describeInput: typeof describeInput;
  }): { host: HTMLElement; overlay: HTMLElement;
         workerClient: ReturnType<typeof createWorkerClient>;
         initialRender: () => void; refresh: () => void;
         setVisible: (visible: boolean) => void;
         setDebug: (on: boolean) => void; destroy: () => void } {
    if (opts.el.isContentEditable) {
      return createContentEditableOverlay(opts);
    }
    return createTextareaOverlay(opts);
  }

  // Overlay for `<textarea>` and `<input>`: shadow-DOM mirror over
  // the element with a red-wavy underline on misspellings.
  function createTextareaOverlay(opts: {
    el: HTMLElement;
    workerClient: ReturnType<typeof createWorkerClient>;
    dbg: typeof dbg;
    describeInput: typeof describeInput;
  }): { host: HTMLElement; overlay: HTMLElement;
         workerClient: ReturnType<typeof createWorkerClient>;
         initialRender: () => void; refresh: () => void;
         setVisible: (visible: boolean) => void;
         setDebug: (on: boolean) => void; destroy: () => void } {
    const { el, workerClient, dbg, describeInput } = opts;
    const input = el as HTMLInputElement | HTMLTextAreaElement;

    // Disable the browser's spellcheck so we don't double-underline.
    // We remember the page's original values so hiding the overlay
    // (blacklist / per-textbox toggle) hands the field back intact.
    const nativeAttrs = saveAndDisableNativeSpellcheck(el);
    dbg('content', 'attach', describeInput(el));

    const host = document.createElement('div');
    host.className = 'torille-overlay-host';
    host.setAttribute('aria-hidden', 'true');
    const shadow = host.attachShadow({ mode: 'open' });

    const style = document.createElement('style');
    style.textContent = OVERLAY_CSS;
    shadow.appendChild(style);

    // Clip only the text mirror to the textarea's box. The mirror is
    // translated upwards when the textarea scrolls; without this wrapper
    // the underlines for scrolled-out lines leak above the textarea and
    // paint over the page. The suggestion popup stays a direct child of
    // the host so it can still extend below/outside the input.
    const mirror = document.createElement('div');
    mirror.className = 'mirror';
    shadow.appendChild(mirror);

    const overlay = document.createElement('div');
    overlay.className = 'overlay';
    mirror.appendChild(overlay);

    const popup = document.createElement('div');
    popup.className = 'suggestions';
    popup.style.display = 'none';
    shadow.appendChild(popup);

    // Put the host *after* the input and leave it at the input's own
    // stacking level. A huge z-index made every underline paint over
    // sticky headers, dialogs and dropdowns that legitimately sit above
    // the field. A positioned sibling after the input still paints above
    // its text (including when the input creates a z-index: 0 context),
    // but page chrome with a higher z-index now wins.
    el.parentNode!.insertBefore(host, el.nextSibling);
    host.style.position = 'absolute';
    // Sit just above the input's own stacking level. A plain `auto` is
    // enough on most pages, but editors that stack an underlay/text
    // layer (`z-index: 1/2`, as the torille demo does) would paint over
    // the overlay and swallow the click. Stay below genuine page chrome:
    // only the parent's positioned children are considered.
    let stacking = 0;
    for (const sibling of Array.from(el.parentElement?.children ?? [])) {
      if (!(sibling instanceof HTMLElement) || sibling === host) continue;
      const z = Number.parseInt(getComputedStyle(sibling).zIndex, 10);
      if (Number.isFinite(z)) stacking = Math.max(stacking, z);
    }
    host.style.zIndex = String(stacking + 1);
    host.style.pointerEvents = 'none';

    // Mirror the textarea's box so the overlay text lines up.
    //
    // Two things drift if we just copy the rect: the box-sizing (UA
    // default for textarea is border-box; for our div it's
    // content-box) and the vertical scrollbar (the textarea eats
    // ~15px from its content width when content overflows; the
    // overlay doesn't, so its text wraps at a different width and
    // every underline lands a glyph-width off). The fix is to:
    //   * set host box-sizing to border-box and size it to the
    //     textarea's border-box rect (matches the textarea's own
    //     layout exactly),
    //   * set overlay box-sizing to border-box too, position it
    //     inside the host at the textarea's clientLeft / clientTop
    //     (so its border lines up with the textarea's text inset),
    //   * size the overlay to el.clientWidth / clientHeight so the
    //     scrollbar reservation from the textarea carries over,
    //   * copy the textarea's padding so the overlay's content
    //     box has the same effective width.
    //
    // `host` is absolute, so left/top are measured from its
    // containing block. That is the nearest positioned ancestor --
    // which is often not the document. The demo's `.editor` is
    // `position: relative`, for example; blindly adding
    // `window.scrollX/Y` there shifts the mirror by the editor's own
    // page offset (a double offset). Resolve the rect against
    // `host.offsetParent` instead.
    function hostPosition(rect: DOMRect): { left: number; top: number } {
      const parent = host.offsetParent as HTMLElement | null;
      if (!parent) {
        return {
          left: window.scrollX + rect.left,
          top: window.scrollY + rect.top,
        };
      }
      const parentStyle = getComputedStyle(parent);
      if (
        (parent === document.body || parent === document.documentElement) &&
        parentStyle.position === 'static'
      ) {
        // No positioned ancestor: the containing block is the initial
        // containing block, i.e. document coordinates.
        return {
          left: window.scrollX + rect.left,
          top: window.scrollY + rect.top,
        };
      }
      const pr = parent.getBoundingClientRect();
      return {
        left: rect.left - pr.left - parent.clientLeft + parent.scrollLeft,
        top: rect.top - pr.top - parent.clientTop + parent.scrollTop,
      };
    }

    // The overlay's base inset inside the host. `syncBox` computes it
    // from the border widths; the scroll handler must use the same value,
    // not `clientLeft`/`clientTop`, which Firefox reports inconsistently
    // for `<input>` (it includes the padding, unlike a `<textarea>`).
    let overlayInsetLeft = 0;
    let overlayInsetTop = 0;

    function syncBox(): void {
      const cs = getComputedStyle(el);
      // Set the font / text properties individually (not the shorthand)
      // so the parser doesn't reset line-height or letter-spacing to
      // "normal" if the textarea's computed value is unexpected.
      overlay.style.fontFamily = cs.fontFamily;
      overlay.style.fontSize = cs.fontSize;
      overlay.style.fontWeight = cs.fontWeight;
      overlay.style.fontStyle = cs.fontStyle;
      overlay.style.lineHeight = cs.lineHeight;
      overlay.style.letterSpacing = cs.letterSpacing;
      overlay.style.wordSpacing = cs.wordSpacing;
      overlay.style.textAlign = cs.textAlign;
      overlay.style.textIndent = cs.textIndent;
      overlay.style.textTransform = cs.textTransform;
      overlay.style.direction = cs.direction;
      overlay.style.writingMode = cs.writingMode;
      overlay.style.wordBreak = cs.wordBreak;
      overlay.style.overflowWrap = cs.overflowWrap;
      // The page's textarea may disable wrapping (`wrap="off"` or an
      // explicit `white-space: pre`). The overlay's stylesheet defaults to
      // `pre-wrap`, which would re-wrap every line and put the underlines
      // on the wrong rows. GitHub's read-only code textarea is the big
      // case: it is `white-space: pre` and scrolls horizontally, so
      // mirroring it with pre-wrap made the whole licence text one block
      // of wrapped lines while the visible code stayed on 55 rows.
      overlay.style.whiteSpace = cs.whiteSpace;
      overlay.style.hyphens = cs.hyphens;
      overlay.style.tabSize = cs.tabSize;
      overlay.style.fontVariant = cs.fontVariant;
      overlay.style.fontKerning = cs.fontKerning;
      overlay.style.padding = cs.padding;
      const r = el.getBoundingClientRect();
      host.style.boxSizing = 'border-box';
      const pos = hostPosition(r);
      host.style.left = pos.left + 'px';
      host.style.top = pos.top + 'px';
      host.style.width = r.width + 'px';
      host.style.height = r.height + 'px';
      // The overlay is positioned inside the host at the textarea's
      // border offset, sized to the textarea's content + padding
      // (clientWidth / clientHeight). Its content box has the same
      // effective width as the textarea's text area because both
      // share the same padding.
      overlay.style.boxSizing = 'border-box';

      // Firefox's `clientLeft`/`clientWidth` mean different things for a
      // replaced `<input>` than for a `<textarea>`: on an input they are
      // the content box and its padding-inclusive inset, while a textarea
      // reports the padding box (minus a scrollbar). Taking `clientLeft`
      // as the overlay's border-box inset therefore pushed input text one
      // padding-width to the right (`Layout edge cases`).
      //
      // Anchor the overlay at the element's border box and give it the
      // padding box. Its own padding then matches the input's padding,
      // and `box-sizing: border-box` leaves the same content width. Keep
      // the textarea path on `clientWidth`/`clientHeight` so a reserved
      // vertical scrollbar still narrows the mirror.
      const borderLeft = parseFloat(cs.borderLeftWidth) || 0;
      const borderTop = parseFloat(cs.borderTopWidth) || 0;
      const borderRight = parseFloat(cs.borderRightWidth) || 0;
      const borderBottom = parseFloat(cs.borderBottomWidth) || 0;
      overlayInsetLeft = borderLeft;
      overlayInsetTop = borderTop;
      const isTextarea = el.tagName === 'TEXTAREA';
      positionOverlay();
      // A non-wrapping textarea scrolls horizontally instead of growing
      // taller. Give the overlay the full scroll width so translating it
      // by `scrollLeft` does not leave the rightmost underlines clipped
      // by the overlay's own `overflow: hidden`.
      const baseWidth = isTextarea
        ? (el.clientWidth || r.width - borderLeft - borderRight)
        : r.width - borderLeft - borderRight;
      overlay.style.width = Math.max(baseWidth, input.scrollWidth || 0) + 'px';
      if (isTextarea) {
        syncOverlayHeight();
      } else {
        overlay.style.height = (r.height - borderTop - borderBottom) + 'px';
      }

      // A single-line `<input>` centres its one line box inside the
      // content box, while the mirror stacks lines from the top. When the
      // field is taller than its line-height (Ecosia's 38px search box with
      // a 19px line), the underline lands through the middle of the glyphs.
      // Give the input mirror a line box as tall as the content area so its
      // glyphs land on the same baseline. Textareas keep their own
      // line-height because they wrap and scroll.
      if (!isTextarea) {
        const contentHeight = Math.max(
          0,
          r.height - borderTop - borderBottom
            - (parseFloat(cs.paddingTop) || 0)
            - (parseFloat(cs.paddingBottom) || 0),
        );
        overlay.style.lineHeight = contentHeight + 'px';
        // An `<input>` never wraps: it scrolls horizontally. Mirror that
        // instead of letting the overlay turn a long query into several
        // visual lines whose later underlines sit under the wrong text.
        overlay.style.whiteSpace = 'pre';
        overlay.style.wordWrap = 'normal';
        overlay.style.overflowWrap = 'normal';
      }
    }

    // The mirror follows the input's own scroll offset as well as its
    // border. `syncBox` runs on window scroll/resize and must not undo the
    // textarea's internal scroll: without the subtraction, a page scroll
    // reset the mirror to the first line while the textarea stayed
    // scrolled down, leaving every underline on the wrong word.
    function positionOverlay(): void {
      overlay.style.left = (overlayInsetLeft - input.scrollLeft) + 'px';
      overlay.style.top = (overlayInsetTop - input.scrollTop) + 'px';
    }

    // A textarea's mirror must contain the *whole* text, not just the
    // visible height: the overlay itself has `overflow: hidden`, so a
    // fixed clientHeight clipped every line past the first screen and the
    // last words never received underlines. The mirror wrapper clips to
    // the textarea box; the overlay is scrolled inside it.
    function syncOverlayHeight(): void {
      if (el.tagName !== 'TEXTAREA') return;
      overlay.style.height =
        Math.max(el.clientHeight || 0, el.scrollHeight || 0) + 'px';
    }

    syncBox();

    let focused: string | null = null;
    let focusedEl: HTMLElement | null = null;

    function closePopup(): void {
      popup.style.display = 'none';
      popup.textContent = '';
      focused = null;
      focusedEl = null;
      el.removeAttribute('data-torille-focused');
    }

    // Put the popup under the clicked word, not merely somewhere inside
    // the field. The old code clamped `left` to the *host* width with a
    // hard-coded 220px guess, which moved the popup left of the word
    // whenever the misspelling was towards the right edge. Anchor it to
    // the word's rect and then clamp only to the viewport, using the
    // popup's measured width/height.
    function positionPopup(): void {
      const anchor = focusedEl;
      if (!anchor || popup.style.display === 'none') return;
      const hostRect = host.getBoundingClientRect();
      const missRect = anchor.getBoundingClientRect();
      const margin = 8;

      popup.style.left = (missRect.left - hostRect.left) + 'px';
      popup.style.top = (missRect.bottom - hostRect.top + 2) + 'px';
      const popupRect = popup.getBoundingClientRect();

      let left = missRect.left - hostRect.left;
      const maxLeft =
        window.innerWidth - margin - popupRect.width - hostRect.left;
      left = Math.max(margin - hostRect.left, Math.min(left, maxLeft));

      let top = missRect.bottom - hostRect.top + 2;
      if (popupRect.bottom > window.innerHeight - margin) {
        // Flip above the word when there is no room below.
        top = missRect.top - hostRect.top - popupRect.height - 2;
      }
      const minTop = margin - hostRect.top;
      const maxTop =
        window.innerHeight - margin - popupRect.height - hostRect.top;
      top = Math.max(minTop, Math.min(top, Math.max(minTop, maxTop)));

      popup.style.left = left + 'px';
      popup.style.top = top + 'px';
    }

    function openPopup(misspellEl: HTMLElement, word: string,
                     start: number, end: number): void {
      focused = word;
      focusedEl = misspellEl;
      el.setAttribute('data-torille-focused', word);
      popup.textContent = '';
      popup.style.display = 'block';

      // A provisional position until `fillPopup` has measured the real
      // content and calls `positionPopup` again.
      const hostRect = host.getBoundingClientRect();
      const missRect = misspellEl.getBoundingClientRect();
      popup.style.left = (missRect.left - hostRect.left) + 'px';
      popup.style.top = (missRect.bottom - hostRect.top + 2) + 'px';

      // Suggestions are read from `lastHits` -- the wasm-side render
      // already produced them, so the popup click doesn't need a
      // second round-trip. If the user clicked before the first
      // render finished, fall back to a state-aware suggest call.
      const hit = lastHits.find(
        (h) => h.start === start && h.end === end,
      );
      let suggestions: string[] = hit?.suggestions ?? [];
      if (suggestions.length === 0 && lastHitsText === input.value) {
        suggestions = [];
      }
      if (suggestions.length === 0) {
        // Fall back for the very-first-click-before-render race.
        workerClient.suggestWithStateAsync(word, uiSettings.suggestionsMax).then((fallback) => {
          if (focused !== word) return;
          fillPopup(word, start, end, fallback);
        }).catch(() => { /* worker down -- popup shows nothing */ });
        return;
      }
      fillPopup(word, start, end, suggestions);
    }

    function fillPopup(
      word: string, start: number, end: number, suggestions: string[],
    ): void {
      if (focused !== word) return; // popup closed before result
      if (!suggestions.length) {
        const empty = document.createElement('div');
        empty.className = 'empty';
        empty.textContent = 'ei ehdotuksia';
        popup.appendChild(empty);
        positionPopup();
        return;
      }
      let picked = 0;
      for (const sug of suggestions) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.textContent = sug;
        let chosen = false;
        const choose = (): void => {
          if (chosen) return;
          chosen = true;
          replaceWordAt(start, end, sug);
          closePopup();
        };
        btn.addEventListener('mousedown', (e: Event) => {
          e.preventDefault();
          choose();
        });
        addTapHandler(btn, choose);
        if (picked === 0) btn.classList.add('focused');
        picked++;
        popup.appendChild(btn);
      }
      positionPopup();
    }

    /**
     * Replace `input.value.slice(start, end)` with `good`. Uses the
     * exact character offsets the renderer recorded, not
     * `indexOf(bad)` -- so if the same misspelling appears twice, only
     * the one the user clicked gets replaced.
     */
    function replaceWordAt(start: number, end: number, good: string): void {
      const cur = input.value;
      if (start < 0 || end > cur.length || start > end) return;
      input.value = cur.slice(0, start) + good + cur.slice(end);
      // Trigger the page's input listeners.
      input.dispatchEvent(new Event('input', { bubbles: true }));
      // Render is already triggered by the input event we just fired.
    }

    // Per-input misspell cache removed: the per-textbox state wasm
    // API returns hits with byte ranges directly. The popup's
    // click handler reads from `lastHits` below -- the popup no
    // longer needs a per-word wasm call to fetch suggestions.
    let pendingRender: ReturnType<typeof setTimeout> | null = null;
    // Monotonic counter so a slow render() can tell when it has been
    // superseded by a newer render() started after the user kept
    // typing. The old render aborts its DOM write so we don't end up
    // with spans from two renders interleaved.
    let renderGen = 0;
    // Latest hits from the last successful render. The popup's click
    // handler reads from this instead of firing another wasm call.
    // `lastHitsStart` is the start byte of the focused hit so the
    // popup can tell whether a click is on the same word it last
    // rendered (and which suggests-list the popup should display).
    let lastHits: R_AnalyzeHit[] = [];
    let lastHitsText = '';

    async function render(): Promise<void> {
      const myGen = ++renderGen;
      const text = input.value;
      dbg('content', 'render:start', { gen: myGen, len: text.length });

      // Code and markup must not influence sentence context or proper-
      // noun learning, so send a masked copy to the engine. The mask is
      // length-preserving, so every returned offset still points at the
      // same character in the original live text.
      const ignored = ignoredRangesFor(text);
      const analyzed = maskIgnored(text, ignored);

      // One wasm round-trip per render, regardless of token count.
      // The workerClient handles the wasm-side slot internally; this
      // call lazy-creates the state on first render. The hits carry
      // UTF-16 ranges into the original `text`, so the JS-side
      // tokenise step we used to do here is now redundant.
      const rawHits = await workerClient.analyzeTextAsync(analyzed, uiSettings.suggestionsMax, myGen);
      if (myGen !== renderGen) {
        dbg('content', 'render:stale', { gen: myGen, current: renderGen });
        return;
      }
      if (rawHits === null) {
        // The checker could not answer. Keep the underlines from the
        // previous render rather than blanking the overlay or (worse)
        // painting the whole text red.
        dbg('content', 'render:unavailable', { gen: myGen });
        return;
      }
      // Defensive post-filter: even if the engine or a future tokeniser
      // returns a hit inside the mask, never underline it.
      const hits = ignored.length === 0
        ? rawHits
        : rawHits.filter((h) => !isIgnored({ start: h.start, end: h.end }, ignored));

      // Only now drop the previous spans: clearing before the await
      // made every keystroke blink the underlines off and on.
      overlay.textContent = '';

      // Build a partition of [0, text.length) into non-misspell and
      // misspell regions from the wasm-returned byte ranges, then
      // append each as a span. The overlay's textContent ends up
      // == text exactly, so the wrap matches the textarea's.
      type Region = { start: number; end: number; misspell: boolean };
      const regions: Region[] = [];
      let lastEnd = 0;
      if (hits) {
        for (const h of hits) {
          if (h.start > lastEnd) {
            regions.push({ start: lastEnd, end: h.start, misspell: false });
          }
          regions.push({
            start: h.start, end: h.end, misspell: true,
          });
          lastEnd = h.end;
        }
      }
      if (lastEnd < text.length) {
        regions.push({ start: lastEnd, end: text.length, misspell: false });
      }

      // Stash the latest hits at the factory scope so the popup's
      // click handler can read suggestions without a second wasm
      // round-trip. Cleared on each render so a stale click on a
      // previous render's spans doesn't see ghosts.
      lastHits = hits;
      lastHitsText = text;

      let misspellCount = 0;
      for (const r of regions) {
        const slice = text.slice(r.start, r.end);
        if (r.misspell) {
          misspellCount++;
          const span = document.createElement('span');
          span.className = 'misspell';
          span.textContent = slice;
          span.dataset.word = slice;
          span.dataset.start = String(r.start);
          span.dataset.end = String(r.end);
          if (uiSettings.popupTrigger === 'hover') {
            span.addEventListener('mouseenter', () => {
              openPopup(span, slice, r.start, r.end);
            });
          } else {
            span.addEventListener('mousedown', (e: Event) => {
              e.preventDefault();
              openPopup(span, slice, r.start, r.end);
            });
          }
          // Hover is meaningless on touch, so a tap always opens the
          // dropdown regardless of the popup trigger setting.
          addTapHandler(span, () => {
            openPopup(span, slice, r.start, r.end);
          });
          overlay.appendChild(span);
        } else {
          const span = document.createElement('span');
          span.className = 'token';
          span.textContent = slice;
          overlay.appendChild(span);
        }
      }

      // Sanity check: the overlay's reconstructed text should equal
      // the textarea's value, char for char. If it doesn't, the
      // underline is under a different word than the user sees.
      const overlayText = overlay.textContent ?? '';
      if (overlayText !== text) {
        dbg('content', 'render:drift',
            { textarea: text, overlay: overlayText,
              lenDiff: overlayText.length - text.length });
      }
      // The text may have grown taller than the textarea's viewport.
      // Resize the mirror before the next paint so scrolled-to lines keep
      // their underlines.
      syncOverlayHeight();

      dbg('content', 'render:done',
          { gen: myGen, regions: regions.length, misspellings: misspellCount,
            hits: hits?.length ?? 0 });
    }

    function scheduleRender(): void {
      if (pendingRender) {
        dbg('content', 'scheduleRender:skip (already pending)');
        return;
      }
      dbg('content', 'scheduleRender:arm');
      pendingRender = setTimeout(() => {
        dbg('content', 'scheduleRender:fire');
        pendingRender = null;
        render();
      }, 150);
    }

    let destroyed = false;
    let resizeObserver: ResizeObserver | null = null;

    function onInputScroll(): void {
      // The overlay has overflow: hidden, so its own scrollTop is
      // ignored. We instead shift the overlay up by the textarea's
      // scrollTop so the visible portion of the overlay text lines
      // up with the visible portion of the textarea. The mirror's
      // overflow: hidden clips the off-screen part.
      positionOverlay();
    }

    // Click outside the popup closes it. Composed-path check so we
    // see through the shadow boundary.
    function onDocumentMouseDown(e: Event): void {
      if (focused === null) return;
      const path = e.composedPath();
      if (path.includes(popup)) return;
      if (path.some((n) => (n as HTMLElement).classList &&
                         (n as HTMLElement).classList.contains('misspell'))) return;
      closePopup();
    }

    // If the page stacks its own editor above our shadow host (the
    // torille demo puts its underlay at z-index 2), the click lands on the
    // textarea, not on a `.misspell` span. Hit-test the overlay's spans by
    // their client rects so the dropdown still opens at the same word.
    function misspellAtPoint(x: number, y: number): HTMLElement | null {
      for (const span of overlay.querySelectorAll<HTMLElement>('.misspell')) {
        for (const rect of span.getClientRects()) {
          if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
            return span;
          }
        }
      }
      return null;
    }

    function openAtPoint(x: number, y: number): boolean {
      const span = misspellAtPoint(x, y);
      if (!span) {
        dbg('content', 'input tap: no misspell at point ' + JSON.stringify(
          { x, y, spans: overlay.querySelectorAll('.misspell').length }));
        return false;
      }
      const word = span.dataset.word ?? span.textContent ?? '';
      const start = Number(span.dataset.start ?? '-1');
      const end = Number(span.dataset.end ?? '-1');
      if (!word || start < 0 || end < 0) return false;
      dbg('content', 'input tap -> popup', { word, start, end });
      openPopup(span, word, start, end);
      return true;
    }

    function onInputMouseDown(event: Event): void {
      const e = event as MouseEvent;
      if (destroyed || e.button !== 0 || uiSettings.popupTrigger !== 'click') return;
      if (openAtPoint(e.clientX, e.clientY)) e.preventDefault();
    }

    input.addEventListener('input', scheduleRender);
    input.addEventListener('mousedown', onInputMouseDown, true);
    // The overlay can sit under a page editor that stacks above us; on
    // touch the tap lands on the input, so hit-test the mirror rects.
    addTapHandler(input, (x, y) => {
      if (!destroyed) openAtPoint(x, y);
    });
    input.addEventListener('scroll', onInputScroll);
    window.addEventListener('scroll', syncBox, { passive: true });
    window.addEventListener('resize', syncBox);

    // The textarea can change size for reasons that don't fire a
    // window resize: auto-grow via JS, CSS-driven reflow, font-load,
    // etc. Watch the element directly so the host stays glued to it.
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(() => syncBox());
      resizeObserver.observe(el);
    }
    // Web fonts can swap the rendered font long after our initial
    // syncBox(). Re-sync once the document reports all fonts loaded.
    if (typeof document.fonts !== 'undefined' && document.fonts.ready) {
      document.fonts.ready.then(() => {
        if (destroyed) return;
        dbg('content', 'fonts.ready:resync');
        syncBox();
      }).catch(() => {});
    }

    document.addEventListener('mousedown', onDocumentMouseDown, true);

    return {
      host,
      overlay,
      workerClient,
      initialRender() { void render(); },
      refresh() { scheduleRender(); },
      setVisible(visible: boolean) {
        host.style.display = visible ? '' : 'none';
        if (visible) {
          // `syncBox` uses `host.offsetParent`, which is null while
          // the host is display:none; re-place it as it comes back.
          syncBox();
          disableNativeSpellcheck(el);
        } else {
          closePopup();
          restoreNativeSpellcheck(el, nativeAttrs);
        }
      },
      setDebug(on: boolean) { workerClient.setDebug(on); },
      destroy() {
        if (destroyed) return;
        destroyed = true;
        if (pendingRender) {
          clearTimeout(pendingRender);
          pendingRender = null;
        }
        closePopup();
        restoreNativeSpellcheck(el, nativeAttrs);
        input.removeEventListener('input', scheduleRender);
        input.removeEventListener('scroll', onInputScroll);
        input.removeEventListener('mousedown', onInputMouseDown, true);
        window.removeEventListener('scroll', syncBox);
        window.removeEventListener('resize', syncBox);
        document.removeEventListener('mousedown', onDocumentMouseDown, true);
        resizeObserver?.disconnect();
        resizeObserver = null;
        workerClient.tearDownState();
        host.remove();
      },
    };
  }

  // Overlay for `[contenteditable]` elements: there's no single
  // text string we can mirror, so we don't render per-word underlines.
  // Instead we watch the cursor position via window.getSelection(),
  // tokenise the word to the left of the caret, and pop a suggestion
  // menu right below it when it's a misspelling. Click a suggestion
  // and the Range API replaces the word in place.
  //
  // The popup is a regular (non-shadow) <div> attached to
  // document.body with `position: absolute` so it floats over rich
  // text formatting without inheriting the editor's CSS.
  function createContentEditableOverlay(opts: {
    el: HTMLElement;
    workerClient: ReturnType<typeof createWorkerClient>;
    dbg: typeof dbg;
    describeInput: typeof describeInput;
  }): { host: HTMLElement; overlay: HTMLElement;
         workerClient: ReturnType<typeof createWorkerClient>;
         initialRender: () => void; refresh: () => void;
         setVisible: (visible: boolean) => void;
         setDebug: (on: boolean) => void; destroy: () => void } {
    const { el, workerClient, dbg, describeInput } = opts;
    const host = document.createElement('div');
    host.className = 'torille-ce-host';
    host.setAttribute('aria-hidden', 'true');
    host.style.position = 'absolute';
    host.style.zIndex = '2147483647';
    host.style.pointerEvents = 'auto';
    host.style.display = 'none';
    host.style.background = 'white';
    host.style.border = '1px solid #ccc';
    host.style.borderRadius = '4px';
    host.style.boxShadow = '0 2px 8px rgba(0,0,0,0.18)';
    host.style.padding = '4px 0';
    host.style.font = '13px/1.4 -apple-system, system-ui, sans-serif';
    host.style.color = '#222';
    host.style.minWidth = '8em';
    host.style.maxWidth = '22em';
    host.style.maxHeight = '14em';
    host.style.overflowY = 'auto';
    document.body.appendChild(host);

    const nativeAttrs = saveAndDisableNativeSpellcheck(el);
    dbg('content', 'attach contenteditable', describeInput(el));

    // The CSS Highlight API exists on `CSS.highlights` in Firefox 140+
    // and Chrome 105+. Where it's available we draw the wavy
    // underline directly on the contenteditable's text nodes -- no
    // shadow mirror, no alignment drift, no font/padding sync
    // headaches. The browser renders the highlight using the
    // element's own layout.
    const HIGHLIGHTS_SUPPORTED =
      typeof CSS !== 'undefined' &&
      typeof (CSS as { highlights?: unknown }).highlights !== 'undefined' &&
      typeof (globalThis as { Highlight?: unknown }).Highlight !== 'undefined';
    if (HIGHLIGHTS_SUPPORTED) ensureHighlightStyle();

    // Cache misspell regions so the click hit-test can map a click
    // coordinate back to the right word.
    type Misspell = { word: string; range: Range };
    const misspellRegions = new Map<string, Misspell>();

    type WordInfo = { word: string; node: Text; start: number; end: number };

    let pendingCheck: ReturnType<typeof setTimeout> | null = null;
    // Render-generation counter for the contenteditable highlight pass.
    // Bumped before every analyse call so a stale response can be
    // discarded without overwriting a newer render.
    let renderGen = 0;

    function closePopup(): void {
      host.style.display = 'none';
      host.textContent = '';
    }

    function positionPopup(): void {
      const sel = window.getSelection();
      if (!sel || !sel.rangeCount) return;
      const range = sel.getRangeAt(0);
      // Hide the popup if the selection is no longer inside our editor.
      if (!el.contains(range.commonAncestorContainer) &&
          range.commonAncestorContainer !== el) {
        closePopup();
        return;
      }
      const rects = range.getClientRects();
      // For a collapsed caret the rects array typically has one tiny
      // line representing the caret position. If we get nothing
      // measurable, the cursor isn't on a real text line and we'd
      // rather hide the popup than anchor it to some empty space
      // (which looks like "underline with no text below").
      let rect: DOMRect | undefined;
      for (const r of Array.from(rects)) {
        if (r.width > 0 || r.height > 0) { rect = r; break; }
      }
      if (!rect) {
        closePopup();
        return;
      }
      host.style.left = (window.scrollX + rect.left) + 'px';
      host.style.top = (window.scrollY + rect.bottom + 2) + 'px';
    }

    // Find the run of letters immediately to the left of the caret.
    // Returns null if the caret isn't inside a text node, or there's
    // no letter to its left, or the caret is at a selection boundary.
    function currentWordAtCursor(): WordInfo | null {
      const sel = window.getSelection();
      if (!sel || !sel.rangeCount) return null;
      if (!sel.isCollapsed) return null;
      const node = sel.focusNode;
      if (!node || node.nodeType !== Node.TEXT_NODE) return null;
      if (!el.contains(node)) return null;
      const textNode = node as Text;
      const text = textNode.textContent ?? '';
      let cursor = sel.focusOffset;
      // Walk backwards over letters (allow trailing hyphen for split
      // compounds, but only if there's at least one letter first).
      let start = cursor;
      let seenLetter = false;
      while (start > 0) {
        const ch = text.charAt(start - 1);
        if (/\p{L}/u.test(ch)) { seenLetter = true; start--; continue; }
        if (ch === '-' && seenLetter) { start--; continue; }
        break;
      }
      if (!seenLetter) return null;
      let end = cursor;
      while (end < text.length) {
        const ch = text.charAt(end);
        if (/\p{L}/u.test(ch) || (ch === '-' && end > start)) { end++; continue; }
        break;
      }
      const word = text.slice(start, end).replace(/-+$/, '');
      if (!word) return null;
      return { word, node: textNode, start, end };
    }

    async function checkCurrentWord(): Promise<void> {
      const info = currentWordAtCursor();
      if (!info) {
        dbg('content', 'checkCurrentWord:no-word');
        closePopup();
        return;
      }
      if (isInsideCodeDom(info.node, el)) {
        dbg('content', 'checkCurrentWord:in-code');
        closePopup();
        return;
      }
      const ignored = ignoredRangesFor(el.textContent ?? '');
      if (ignored.length > 0) {
        const at = textOffsetOf(info.node, info.start);
        if (at !== null
            && isIgnored({ start: at, end: at + info.word.length }, ignored)) {
          dbg('content', 'checkCurrentWord:in-ignored-range');
          closePopup();
          return;
        }
      }
      dbg('content', 'checkCurrentWord:start', { word: info.word });
      const ok = await workerClient.checkFastAsync(info.word);
      if (ok === null) {
        // Checker unavailable: leave the editor alone instead of
        // opening a "no suggestions" popup for every word.
        dbg('content', 'checkCurrentWord:unavailable', { word: info.word });
        return;
      }
      if (ok) {
        dbg('content', 'checkCurrentWord:ok', { word: info.word });
        closePopup();
        return;
      }
      const suggestions = await workerClient.suggestWithStateAsync(info.word, uiSettings.suggestionsMax);
      if (!suggestions.length) {
        dbg('content', 'checkCurrentWord:no-suggestions', { word: info.word });
        closePopup();
        return;
      }
      dbg('content', 'checkCurrentWord:misspell', {
        word: info.word, suggestions: suggestions.length,
      });
      host.textContent = '';
      for (const sug of suggestions) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.textContent = sug;
        btn.style.display = 'block';
        btn.style.width = '100%';
        btn.style.textAlign = 'left';
        btn.style.background = 'none';
        btn.style.border = '0';
        btn.style.padding = '5px 12px';
        btn.style.cursor = 'pointer';
        btn.style.font = 'inherit';
        btn.style.color = 'inherit';
        btn.addEventListener('mousedown', (e: Event) => {
          e.preventDefault();
          const range = document.createRange();
          range.setStart(info.node, info.start);
          range.setEnd(info.node, info.end);
          replaceRange(range, sug);
        });
        host.appendChild(btn);
      }
      host.style.display = 'block';
      positionPopup();
    }

    // Inverse of `textNodeAt`: the global textContent offset for a
    // (Text node, local offset) pair. Used to compare the caret against
    // the text-range ignore mask.
    function textOffsetOf(node: Text, localOffset: number): number | null {
      let total = 0;
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      let n: Node | null;
      while ((n = walker.nextNode())) {
        const t = n as Text;
        const len = (t.textContent ?? '').length;
        if (t === node) return total + Math.min(localOffset, len);
        total += len;
      }
      return null;
    }

    // Map a global character offset (relative to textContent) into a
    // concrete (Text node, localOffset) pair inside the contenteditable.
    // Returns null if the offset is out of range. Walks text nodes in
    // DOM order, matching the way `textContent` concatenates them.
    function textNodeAt(globalOffset: number): { node: Text; offset: number } | null {
      let remaining = globalOffset;
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      let node: Node | null;
      while ((node = walker.nextNode())) {
        const t = node as Text;
        const len = (t.textContent ?? '').length;
        if (remaining <= len) return { node: t, offset: remaining };
        remaining -= len;
      }
      return null;
    }

    /**
     * Re-analyse the editor and update CSS.highlights. Idempotent;
     * safe to call from any of the watchpoints (input, post-replace).
     * Bails out silently if the highlight API isn't available.
     *
     * Uses the same first-pass `analyze-text` as the textarea overlay:
     * one wasm call for the whole editor (~ms), UTF-16 ranges, no
     * per-word `check` (whose engine fallback can take seconds).
     */
    async function renderMisspellHighlights(): Promise<void> {
      if (!HIGHLIGHTS_SUPPORTED) return;
      const text = el.textContent ?? '';
      const ignored = ignoredRangesFor(text);
      const hits = await workerClient.analyzeTextAsync(maskIgnored(text, ignored), 8, ++renderGen);
      if (hits === null) {
        dbg('content', 'highlight:unavailable');
        return; // keep the previous ranges
      }
      const ranges: Range[] = [];
      misspellRegions.clear();
      for (const hit of hits) {
        if (ignored.length > 0 && isIgnored({ start: hit.start, end: hit.end }, ignored)) {
          continue;
        }
        const start = textNodeAt(hit.start);
        const end = textNodeAt(hit.end);
        if (!start || !end) continue;
        // Rich editors expose code as real DOM: skip any hit whose text
        // node lives inside <code>/<pre>/<kbd>/<samp>/<script>/<style>.
        if (isInsideCodeDom(start.node, el) || isInsideCodeDom(end.node, el)) {
          continue;
        }
        const range = document.createRange();
        try {
          range.setStart(start.node, start.offset);
          range.setEnd(end.node, end.offset);
        } catch (e) {
          dbg('content', 'highlight:range:setStart-failed', { err: String(e) });
          continue;
        }
        ranges.push(range);
        misspellRegions.set(hit.word.toLowerCase() + ':' + hit.start, {
          word: hit.word, range,
        });
      }
      // Highlight's signature is `(AbstractRange... initialRanges)` -- varargs
      // of an array. Wrapping in an array trips Chromium's runtime
      // type check, hence the spread below. Live Range instances work
      // fine as the argument (StaticRange is not required), so we
      // pass them straight through.
      const highlightCtor = (globalThis as unknown as {
        Highlight: new (...ranges: AbstractRange[]) => unknown;
      }).Highlight;
      const highlight = new highlightCtor(...ranges);
      (CSS as unknown as {
        highlights: { set: (k: string, h: unknown) => void };
      }).highlights.set('torille-misspell', highlight);
      dbg('content', 'highlight:set',
          { misspellings: ranges.length, totalChars: text.length });
    }

    // Hit-test: the CSS Highlight pseudo-element can't receive events,
    // so when the user clicks or taps the contenteditable we look up
    // which misspell Region's bounding rects contain the point.
    function openMisspellAtPoint(x: number, y: number): boolean {
      if (misspellRegions.size === 0) return false;
      for (const ms of misspellRegions.values()) {
        for (const r of Array.from(ms.range.getClientRects())) {
          if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
            dbg('content', 'tap:on-misspell', { word: ms.word });
            openPopupAtWord(ms.word, ms.range);
            return true;
          }
        }
      }
      return false;
    }

    function onEditorClick(e: MouseEvent): void {
      if (openMisspellAtPoint(e.clientX, e.clientY)) e.preventDefault();
    }
    el.addEventListener('click', onEditorClick);
    addTapHandler(el, (x, y) => {
      openMisspellAtPoint(x, y);
    });

    function openPopupAtWord(word: string, range: Range): void {
      const rect = range.getBoundingClientRect();
      if (!rect.width && !rect.height) return;
      host.textContent = '';
      workerClient.suggestWithStateAsync(word, 8).then((suggestions) => {
        if (!suggestions.length) return;
        for (const sug of suggestions) {
          const btn = document.createElement('button');
          btn.type = 'button';
          btn.textContent = sug;
          btn.style.display = 'block';
          btn.style.width = '100%';
          btn.style.textAlign = 'left';
          btn.style.background = 'none';
          btn.style.border = '0';
          btn.style.padding = '5px 12px';
          btn.style.cursor = 'pointer';
          btn.style.font = 'inherit';
          btn.style.color = 'inherit';
          btn.style.touchAction = 'manipulation';
          let chosen = false;
          const choose = (): void => {
            if (chosen) return;
            chosen = true;
            replaceRange(range, sug);
            closePopup();
            void renderMisspellHighlights();
          };
          btn.addEventListener('mousedown', (e: Event) => {
            e.preventDefault();
            choose();
          });
          addTapHandler(btn, choose);
          host.appendChild(btn);
        }
        host.style.display = 'block';
        // Keep the dropdown on-screen on a narrow phone viewport.
        const margin = 8;
        const width = host.offsetWidth;
        const height = host.offsetHeight;
        let left = window.scrollX + rect.left;
        let top = window.scrollY + rect.bottom + 2;
        left = Math.max(
          window.scrollX + margin,
          Math.min(left, window.scrollX + window.innerWidth - margin - width),
        );
        if (rect.bottom + 2 + height > window.innerHeight - margin
            && rect.top - height - 2 >= margin) {
          top = window.scrollY + rect.top - height - 2;
        }
        host.style.left = left + 'px';
        host.style.top = top + 'px';
      });
    }

    function replaceRange(range: Range, good: string): void {
      range.deleteContents();
      const textNode = document.createTextNode(good);
      range.insertNode(textNode);
      const sel = window.getSelection();
      if (sel) {
        sel.removeAllRanges();
        const newRange = document.createRange();
        newRange.setStart(textNode, textNode.length);
        newRange.setEnd(textNode, textNode.length);
        sel.addRange(newRange);
      }
      el.dispatchEvent(new InputEvent('input', { bubbles: true }));
    }

    function scheduleCheck(): void {
      if (pendingCheck) {
        dbg('content', 'scheduleCheck:skip (already pending)');
        return;
      }
      dbg('content', 'scheduleCheck:arm');
      pendingCheck = setTimeout(() => {
        dbg('content', 'scheduleCheck:fire');
        pendingCheck = null;
        checkCurrentWord();
      }, 150);
    }

    // We only re-check on actual content edits (`input`). `mouseup` /
    // `selectionchange` would fire on every mouse movement and arrow-key
    // nudge, making the popup flicker in and out for no reason -- the
    // user's complaint about "activates at random times". An `input`
    // event is enough: typing, paste, delete, IME compose-finalise all
    // fire it.
    function onDocumentInput(e: Event): void {
      if (e.target === el || (e.target instanceof Node && el.contains(e.target))) {
        dbg('content', 'input:hit', { target: (e.target as Node).nodeName });
        scheduleCheck();
        if (HIGHLIGHTS_SUPPORTED) void renderMisspellHighlights();
      }
    }

    // Close popup on click outside.
    function onDocumentMouseDown(e: Event): void {
      if (host.style.display === 'none') return;
      if (e.composedPath().includes(host)) return;
      dbg('content', 'click-outside:close');
      closePopup();
    }

    document.addEventListener('input', onDocumentInput, true);
    document.addEventListener('mousedown', onDocumentMouseDown, true);

    return {
      host,
      overlay: host,
      workerClient,
      initialRender() {
        if (HIGHLIGHTS_SUPPORTED) void renderMisspellHighlights();
      },
      refresh() {
        if (HIGHLIGHTS_SUPPORTED) void renderMisspellHighlights();
      },
      setVisible(visible: boolean) {
        if (visible) {
          disableNativeSpellcheck(el);
        } else {
          closePopup();
          restoreNativeSpellcheck(el, nativeAttrs);
        }
      },
      setDebug(on: boolean) { workerClient.setDebug(on); },
      destroy() {
        closePopup();
        restoreNativeSpellcheck(el, nativeAttrs);
        el.removeEventListener('click', onEditorClick);
        document.removeEventListener('input', onDocumentInput, true);
        document.removeEventListener('mousedown', onDocumentMouseDown, true);
        workerClient.tearDownState();
        host.remove();
      },
    };
  }

  // ─────────────────────────────────────────────────────────────────────
  // §6  Blacklist factory
  // ─────────────────────────────────────────────────────────────────────

  // Per-host blacklist. Loaded once on boot; refreshed whenever the
  // storage changes (options page edit or context-menu site toggle).
  // When a host comes off the list we re-scan the document so any
  // inputs the scanner skipped on first pass get attached.
  function createBlacklist(opts: {
    dbg: typeof dbg;
    refreshOverlays: (opts: { reScan: boolean }) => void;
  }): { load: () => Promise<void>; isSiteBlacklisted: () => boolean } {
    const { refreshOverlays } = opts;
    const myHostname = location.hostname;
    let blacklist = new Set<string>();

    function isSiteBlacklisted(): boolean {
      return blacklist.has(myHostname);
    }

    async function load(): Promise<void> {
      const cur = await api.storage.local.get('blacklist');
      blacklist = new Set(Array.isArray(cur.blacklist) ? cur.blacklist : []);
      opts.dbg('content', 'blacklist loaded',
          { count: blacklist.size, blacklisted: isSiteBlacklisted() });
      // First refresh: scan only if the host isn't blacklisted. If it
      // is, `refreshOverlays` will hide whatever the scanner found
      // via MutationObserver before the load resolved (unlikely but
      // possible on a SPA mount).
      refreshOverlays({ reScan: !isSiteBlacklisted() });
    }

    if (api.storage?.onChanged) {
      api.storage.onChanged.addListener((changes, area) => {
        if (area === 'local' && changes.blacklist) {
          blacklist = new Set((changes.blacklist.newValue as string[] | undefined) || []);
          opts.dbg('content', 'storage.onChanged blacklist',
              { newCount: blacklist.size });
          refreshOverlays({ reScan: !isSiteBlacklisted() });
        }
      });
    }

    return { load, isSiteBlacklisted };
  }

  // ─────────────────────────────────────────────────────────────────────
  // §6b  Grammar panel
  // ─────────────────────────────────────────────────────────────────────

  // The grammar check is opt-in through the context menu and runs on
  // one input at a time. It is not on the keystroke path.

  function syntaxKindLabel(kind: string): string {
    switch (kind) {
      case 'subject_predicate_agreement': return 'subjektin ja predikaatin kongruenssi';
      case 'attribute_agreement': return 'attribuutin kongruenssi';
      case 'verb_rection': return 'verbien rektio';
      case 'case_selection': return 'sijan valinta';
      case 'object_case': return 'objektin sija';
      case 'missing_argument': return 'puuttuva argumentti';
      case 'extra_argument': return 'ylimääräinen argumentti';
      case 'clause_structure': return 'lauserakenne';
      case 'word_order': return 'sanajärjestys';
      case 'comma_missing': return 'puuttuva pilkku';
      case 'comma_extra': return 'ylimääräinen pilkku';
      case 'comma_pair_unbalanced': return 'pilkupari on epätasapainossa';
      case 'question_mark_missing': return 'puuttuva kysymysmerkki';
      case 'unbalanced_quote': return 'epätasapainoinen lainausmerkki';
      case 'list_separator_missing': return 'puuttuva luettelopilkku';
      case 'list_separator_duplicate': return 'kaksinkertainen luettelopilkku';
      case 'list_comma_before_conjunction': return 'pilkku ennen konjunktiota';
      case 'list_introducer_comma': return 'pilkku kaksoispisteen jälkeen';
      default: return kind;
    }
  }

  function syntaxSeverityLabel(severity: string): string {
    if (severity === 'error') return 'virhe';
    if (severity === 'warning') return 'varoitus';
    return 'ehdotus';
  }

  function syntaxSeverityColor(severity: string): string {
    if (severity === 'error') return '#c33';
    if (severity === 'warning') return '#d80';
    return '#36c';
  }

  function syntaxCorrectionLabel(correction: R_SyntaxCorrection): string {
    const first = correction.edits[0];
    if (!first) return 'korjaa';
    return first.replacement === '' ? 'poista' : first.replacement;
  }

  function applySyntaxEdits(text: string, edits: R_SyntaxEdit[]): string {
    const ordered = [...edits].sort(
      (a, b) => (b.start - a.start) || (b.end - a.end));
    let out = text;
    for (const edit of ordered) {
      if (edit.start < 0 || edit.end > out.length || edit.start > edit.end) {
        continue;
      }
      out = out.slice(0, edit.start) + edit.replacement + out.slice(edit.end);
    }
    return out;
  }

  function canApplySyntax(input: HTMLElement): boolean {
    return input instanceof HTMLTextAreaElement
      || input instanceof HTMLInputElement;
  }

  function editorText(input: HTMLElement): string {
    if (input instanceof HTMLTextAreaElement
        || input instanceof HTMLInputElement) {
      return input.value;
    }
    return input.textContent ?? '';
  }

  function setEditorText(input: HTMLElement, text: string): void {
    if (input instanceof HTMLTextAreaElement
        || input instanceof HTMLInputElement) {
      input.value = text;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
  }

  function showGrammarPanel(
    input: HTMLElement,
    text: string,
    diagnostics: R_SyntaxDiagnostic[],
    onApply: (newText: string) => void,
  ): void {
    document.querySelector('.torille-grammar-panel')?.remove();
    const panel = document.createElement('div');
    panel.className = 'torille-grammar-panel';
    panel.style.cssText = [
      'position:fixed',
      'z-index:2147483647',
      'box-sizing:border-box',
      'max-height:60vh',
      'overflow:auto',
      'background:#fff',
      'color:#111',
      'border:1px solid #888',
      'border-radius:6px',
      'box-shadow:0 4px 18px rgba(0,0,0,.25)',
      'padding:10px',
      'font:13px/1.4 system-ui,sans-serif',
    ].join(';');

    const width = Math.max(240, Math.min(440, window.innerWidth - 16));
    const rect = input.getBoundingClientRect();
    const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
    const below = rect.bottom + 6;
    const top = below + 220 < window.innerHeight
      ? below
      : Math.max(8, rect.top - 226);
    panel.style.width = `${width}px`;
    panel.style.left = `${left}px`;
    panel.style.top = `${top}px`;

    const header = document.createElement('div');
    header.style.cssText =
      'display:flex;justify-content:space-between;align-items:center;gap:8px;margin-bottom:6px;';
    const title = document.createElement('strong');
    title.textContent = 'Kielioppi';
    const close = document.createElement('button');
    close.type = 'button';
    close.textContent = 'sulje';
    close.style.cssText = 'font:inherit;padding:0.15em 0.5em;cursor:pointer;';
    close.addEventListener('click', () => panel.remove());
    header.append(title, close);
    panel.append(header);

    if (diagnostics.length === 0) {
      const empty = document.createElement('p');
      empty.textContent = 'Ei huomautuksia.';
      empty.style.margin = '0.3em 0';
      panel.append(empty);
    } else {
      const list = document.createElement('ul');
      list.style.cssText = 'list-style:none;margin:0;padding:0;';
      for (const diagnostic of diagnostics) {
        const item = document.createElement('li');
        item.style.cssText =
          'border-left:3px solid ' + syntaxSeverityColor(diagnostic.severity)
          + ';padding:4px 6px;margin:6px 0;background:#fafafa;';
        const head = document.createElement('div');
        const label = document.createElement('strong');
        label.textContent = syntaxKindLabel(diagnostic.kind);
        const severity = document.createElement('span');
        severity.textContent = ' ' + syntaxSeverityLabel(diagnostic.severity);
        severity.style.color = '#666';
        head.append(label, severity);
        item.append(head);
        if (diagnostic.expected) {
          const expected = document.createElement('div');
          expected.textContent = 'odotettu: ' + diagnostic.expected;
          expected.style.cssText = 'color:#555;font-size:0.9em;margin-top:2px;';
          item.append(expected);
        }
        if (diagnostic.corrections.length > 0) {
          const actions = document.createElement('div');
          actions.style.cssText = 'margin-top:4px;';
          for (const correction of diagnostic.corrections) {
            const button = document.createElement('button');
            button.type = 'button';
            button.textContent = syntaxCorrectionLabel(correction);
            button.style.cssText =
              'font:inherit;padding:0.15em 0.5em;margin:0 4px 0 0;cursor:pointer;';
            if (canApplySyntax(input)) {
              button.addEventListener('click', () => {
                if (editorText(input) !== text) {
                  // The text changed after the check; the offsets are stale.
                  panel.remove();
                  return;
                }
                onApply(applySyntaxEdits(text, correction.edits));
                panel.remove();
              });
            } else {
              button.disabled = true;
            }
            actions.append(button);
          }
          item.append(actions);
        }
        list.append(item);
      }
      panel.append(list);
      if (!canApplySyntax(input)) {
        const note = document.createElement('p');
        note.textContent = 'Korjaukset ovat toistaiseksi vain tekstikentille.';
        note.style.cssText = 'color:#666;font-size:0.85em;margin:6px 0 0;';
        panel.append(note);
      }
    }
    document.body.append(panel);
  }

  // ─────────────────────────────────────────────────────────────────────
  // §7  Per-textbox toggle factory
  // ─────────────────────────────────────────────────────────────────────

  // Firefox's `contextMenus.onClicked` doesn't tell us which DOM
  // element was right-clicked; the content script captures the
  // `contextmenu` event itself, remembers the input, and round-trips
  // through the background when the menu item is toggled.
  function createTextboxToggle(opts: {
    dbg: typeof dbg;
    describeInput: typeof describeInput;
    setVisible: (el: HTMLElement, visible: boolean) => void;
    checkGrammar: (el: HTMLElement) => void;
  }): void {
    const disabledInputs = new WeakSet<HTMLElement>();
    let pendingInput: HTMLElement | null = null;

    function isTargetInput(target: EventTarget | null): boolean {
      const el = target as HTMLElement | null;
      if (!el) return false;
      if (el.isContentEditable) return true;
      const tag = el.tagName;
      if (tag === 'TEXTAREA') return true;
      if (tag === 'INPUT') {
        const inputType = (el as HTMLInputElement).type || 'text';
        return /^(text|search|email|url)?$/i.test(inputType);
      }
      return false;
    }

    document.addEventListener('contextmenu', (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (!isTargetInput(target)) return;
      pendingInput = target;
      opts.dbg('content', 'contextmenu on', describeInput(target),
          { disabled: target ? disabledInputs.has(target) : false });
      try {
        api.runtime.sendMessage({
          type: 'torille:contextmenu-fired',
          disabled: target ? disabledInputs.has(target) : false,
        });
      } catch (_) {
        // Background may have torn down; ignore.
      }
    }, true);

    api.runtime.onMessage.addListener((message: unknown) => {
      const m = message as R_ContentInboundMessage;
      if (m?.type === 'torille:check-grammar-pending-input') {
        const input = pendingInput;
        if (input) opts.checkGrammar(input);
        return Promise.resolve({ checked: !!input } as R_CheckGrammarResponse);
      }
      if (m?.type !== 'torille:toggle-pending-input') return undefined;
      const input = pendingInput;
      if (!input) return Promise.resolve({ disabled: false } as R_ToggleResponse);
      const wasDisabled = disabledInputs.has(input);
      if (wasDisabled) disabledInputs.delete(input);
      else disabledInputs.add(input);
      const nowDisabled = !wasDisabled;
      opts.setVisible(input, !nowDisabled);
      opts.dbg('content', 'toggle input', describeInput(input), { nowDisabled });
      return Promise.resolve({ disabled: nowDisabled } as R_ToggleResponse);
    });
  }

  // ─────────────────────────────────────────────────────────────────────
  // §8  Scanner + MutationObserver
  // ─────────────────────────────────────────────────────────────────────

  // Walk a root (the document or any element) and register every
  // matching text input. Records what's been attached so the
  // blacklist can hide/show them, and so we don't double-attach.
  function createScanner(opts: {
    dbg: typeof dbg;
    describeInput: typeof describeInput;
    isSiteBlacklisted: () => boolean;
    shouldAttach: (el: HTMLElement) => boolean;
    attachFn: (el: HTMLElement) => ReturnType<typeof createOverlay> | null;
    detachFn: (el: HTMLElement) => void;
  }): { attach: (el: HTMLElement) => void; detach: (el: HTMLElement) => void;
             scan: (root: ParentNode) => void; attached: Set<HTMLElement> } {
    const { attachFn } = opts;
    const attached = new Set<HTMLElement>();

    function attach(el: HTMLElement): void {
      if (!el || attached.has(el)) return;
      if (opts.isSiteBlacklisted()) {
        opts.dbg('content', 'skip (blacklisted)', describeInput(el));
        return;
      }
      if (!opts.shouldAttach(el)) {
        opts.dbg('content', 'skip (option)', describeInput(el));
        return;
      }
      const overlay = attachFn(el);
      if (!overlay) return;
      attached.add(el);
    }

    function detach(el: HTMLElement): void {
      if (!attached.delete(el)) return;
      opts.detachFn(el);
    }

    function scan(root: ParentNode): void {
      if (!root || !(root as Element).querySelectorAll) return;
      const els = (root as Element).querySelectorAll(TARGET_SELECTOR);
      for (const el of Array.from(els)) {
        attach(el as HTMLElement);
      }
    }

    // Catch text inputs that are added later (SPA, modals, comment
    // boxes mounted on click). `subtree: true` walks the whole
    // document body. `characterData: true` catches contenteditable
    // edits -- the text nodes inside the editor mutate in place
    // rather than being replaced wholesale.
    const observer = new MutationObserver((mutations: MutationRecord[]) => {
      for (const m of mutations) {
        for (const node of Array.from(m.addedNodes)) {
          if (!(node instanceof HTMLElement)) continue;
          if (node.matches && node.matches(TARGET_SELECTOR)) attach(node);
          if (node.querySelectorAll) {
            const inner = node.querySelectorAll(TARGET_SELECTOR);
            for (const el of Array.from(inner)) attach(el as HTMLElement);
          }
        }
      }
    });
    observer.observe(document.documentElement || document.body, {
      childList: true,
      subtree: true,
    });

    return { attach, detach, scan, attached };
  }

  // ─────────────────────────────────────────────────────────────────────
  // §9  Bootstrap
  // ─────────────────────────────────────────────────────────────────────

  // Per-overlay `inputId` counter. Each textarea / contenteditable
  // attached by the scanner gets the next number, which the worker
  // client bakes into its batch payloads and verifies on responses.
  // Page-scoped, monotonic across the lifetime of this content
  // script -- the scanner calls `nextInputId()` once per `attach`.
  let nextInputId = 1;

  // Opt-in: search boxes are skipped unless the options page enables
  // `torille-searchboxes`. Missing key means off.
  let checkSearchboxes = false;

  const perInput = new Map<HTMLElement, ReturnType<typeof createOverlay>>();

  // Hide or show the host element for one input. Used by the
  // per-textbox toggle and the blacklist.
  function setVisible(el: HTMLElement, visible: boolean): void {
    const o = perInput.get(el);
    if (o) o.setVisible(visible);
    else {
      // The input was skipped (e.g., blacklisted); just store the
      // intent so a later refreshOverlays can apply it.
      pendingVisibility.set(el, visible);
    }
  }
  const pendingVisibility = new WeakMap<HTMLElement, boolean>();

  const scanner = createScanner({
    dbg,
    describeInput,
    isSiteBlacklisted: () => blacklist.isSiteBlacklisted(),
    shouldAttach: (el) =>
      !isCodeEditorField(el) && (checkSearchboxes || !isSearchField(el)),
    detachFn: (el) => {
      perInput.get(el)?.destroy();
      perInput.delete(el);
      pendingVisibility.delete(el);
      dbg('input:detach', { which: describeInput(el) });
    },
    attachFn: (el) => {
      // One worker client per overlay, so each textarea's batch
      // messages carry their own `inputId`. The factory is cheap
      // (it's just a closure over `dbg` + an integer), so per-overlay
      // allocation isn't measurable.
      const inputId = nextInputId++;
      const workerClient = createWorkerClient({ dbg, inputId });
      // Allocate the per-textbox wasm state on attach. The slot
      // lives for the lifetime of this overlay (we don't yet
      // detect overlay teardown when the DOM element is removed;
      // a future patch wires the scanner's `detach` callback
      // through `workerClient.destroyStateAsync`).
      workerClient.createStateAsync().then((idx) => {
        if (idx !== null) {
          dbg('content', 'createState', { inputId, idx });
        }
      });
      const overlay = createOverlay({ el, workerClient, dbg, describeInput });
      perInput.set(el, overlay);
      dbg('input:attach', { inputId, ...{ which: describeInput(el) } });
      overlay.initialRender();
      // Apply any visibility the toggle stored before the overlay
      // existed.
      const wanted = pendingVisibility.get(el);
      if (wanted !== undefined) overlay.setVisible(wanted);
      return overlay;
    },
  });

  function refreshSearchFields(): void {
    if (checkSearchboxes) {
      scanner.scan(document);
      return;
    }
    for (const el of Array.from(perInput.keys())) {
      if (isSearchField(el)) scanner.detach(el);
    }
  }

  async function loadSearchSetting(): Promise<void> {
    const cur = await api.storage.local.get('torille-searchboxes');
    checkSearchboxes = cur['torille-searchboxes'] === true;
    dbg('content', 'searchboxes setting ->', checkSearchboxes);
    refreshSearchFields();
  }

  const blacklist = createBlacklist({
    dbg,
    refreshOverlays({ reScan }: { reScan: boolean }) {
      const blacklisted = blacklist.isSiteBlacklisted();
      dbg('content', 'refreshOverlays',
          { blacklisted, visible: !blacklisted, count: perInput.size });
      for (const [, o] of perInput) o.setVisible(!blacklisted);
      if (reScan) scanner.scan(document);
    },
  });

  createTextboxToggle({
    dbg,
    describeInput,
    setVisible,
    checkGrammar(input: HTMLElement): void {
      const overlay = perInput.get(input);
      if (!overlay) return;
      const text = editorText(input);
      overlay.workerClient.checkSyntaxAsync(text).then((diagnostics) => {
        if (diagnostics === null) return;
        showGrammarPanel(input, text, diagnostics, (newText) => {
          setEditorText(input, newText);
        });
      }).catch((e: unknown) => {
        dbg('content', 'checkGrammar failed', String(e));
      });
    },
  });

  // Listen for the options-page debug toggle and to push the flag
  // into the worker. The flag is global, so any one of the per-input
  // worker clients can carry it -- the background forwards the
  // 'torille:debug' message to the wasm worker regardless of inputId.
  // If no overlay exists yet (debug toggled before any input is
  // attached), we drop it; nothing to debug anyway.
  if (api.storage?.onChanged) {
    api.storage.onChanged.addListener((changes, area) => {
      if (area === 'local' && changes['torille-debug']) {
        const on = changes['torille-debug'].newValue === true;
        TORILLE_DEBUG = on;
        dbg('content', 'storage.onChanged torille-debug ->', on);
        const first = perInput.values().next().value;
        if (first) first.setDebug(on);
      }
      if (area === 'local' && changes['torille-searchboxes']) {
        checkSearchboxes = changes['torille-searchboxes'].newValue === true;
        dbg('content', 'storage.onChanged torille-searchboxes ->',
            checkSearchboxes);
        refreshSearchFields();
      }
      if (area === 'local'
          && Object.values(UI_SETTINGS).some((key) => key in changes)) {
        void loadUiSettings().then(() => {
          dbg('content', 'storage.onChanged ui settings', uiSettings);
          for (const [el, overlay] of perInput) {
            if (isCodeEditorField(el)) scanner.detach(el);
            else overlay.refresh();
          }
          scanner.scan(document);
        });
      }
    });
  }

  // Bootstrap. The blacklist load resolves and runs the first scan
  // (only if the host isn't blacklisted). MutationObserver covers
  // anything that mounts later.
  void (async () => {
    await loadUiSettings();
    blacklist.load();
    void loadSearchSetting();
  })();
})();