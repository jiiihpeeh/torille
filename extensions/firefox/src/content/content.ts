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

  const api: typeof browser = (typeof browser !== 'undefined') ? browser : chrome;

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
  ].join(', ');

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
        text-decoration: underline wavy #c33;
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
      text-decoration: underline wavy #c33;
      text-decoration-skip-ink: none;
      pointer-events: auto;
      cursor: pointer;
    }
    .suggestions {
      position: absolute;
      z-index: 2147483647;
      background: white;
      border: 1px solid #ccc;
      border-radius: 4px;
      box-shadow: 0 2px 8px rgba(0,0,0,0.18);
      padding: 4px 0;
      font: 13px/1.4 -apple-system, system-ui, sans-serif;
      color: #222;
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
    }
    .suggestions button:hover,
    .suggestions button.focused {
      background: #07c;
      color: white;
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
        const idx = await stateIdx();
        if (idx === null) return null;
        // The proper-noun update is fired ahead of analyze so the
        // wasm has the latest text in hand. analyze-text awaits in
        // the same render, so the renderer's wall-clock cost is
        // still one round-trip.
        api.runtime.sendMessage({
          type: 'torille:update-state', idx, text,
        } as R_UpdateState).catch(() => {});
        const r = await send<R_AnalyzeTextResponse>({
          type: 'torille:analyze-text', inputId, gen, idx, text, max,
        });
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
         initialRender: () => void; setVisible: (visible: boolean) => void;
         setDebug: (on: boolean) => void } {
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
         initialRender: () => void; setVisible: (visible: boolean) => void;
         setDebug: (on: boolean) => void } {
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

    el.parentNode!.insertBefore(host, el);
    host.style.position = 'absolute';
    host.style.zIndex = '2147483646';
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
      overlay.style.width = (
        isTextarea
          ? (el.clientWidth || r.width - borderLeft - borderRight)
          : r.width - borderLeft - borderRight
      ) + 'px';
      overlay.style.height = (
        isTextarea
          ? (el.clientHeight || r.height - borderTop - borderBottom)
          : r.height - borderTop - borderBottom
      ) + 'px';
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

    syncBox();

    let focused: string | null = null;

    function closePopup(): void {
      popup.style.display = 'none';
      popup.textContent = '';
      focused = null;
      el.removeAttribute('data-torille-focused');
    }

    function openPopup(misspellEl: HTMLElement, word: string,
                     start: number, end: number): void {
      focused = word;
      el.setAttribute('data-torille-focused', word);
      popup.textContent = '';
      popup.style.display = 'block';

      // Position just below the focused misspelling.
      const hostRect = host.getBoundingClientRect();
      const missRect = misspellEl.getBoundingClientRect();
      let left = missRect.left - hostRect.left;
      let top = missRect.bottom - hostRect.top + 2;
      if (left + 220 > hostRect.width) {
        left = Math.max(0, missRect.right - hostRect.left - 220);
      }
      popup.style.left = left + 'px';
      popup.style.top = top + 'px';

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
        workerClient.suggestWithStateAsync(word, 8).then((fallback) => {
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
        return;
      }
      let picked = 0;
      for (const sug of suggestions) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.textContent = sug;
        btn.addEventListener('mousedown', (e: Event) => {
          e.preventDefault();
          replaceWordAt(start, end, sug);
          closePopup();
        });
        if (picked === 0) btn.classList.add('focused');
        picked++;
        popup.appendChild(btn);
      }
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

      // One wasm round-trip per render, regardless of token count.
      // The workerClient handles the wasm-side slot internally; this
      // call lazy-creates the state on first render. The hits carry
      // UTF-16 ranges into the original `text`, so the JS-side
      // tokenise step we used to do here is now redundant.
      const hits = await workerClient.analyzeTextAsync(text, 8, myGen);
      if (myGen !== renderGen) {
        dbg('content', 'render:stale', { gen: myGen, current: renderGen });
        return;
      }
      if (hits === null) {
        // The checker could not answer. Keep the underlines from the
        // previous render rather than blanking the overlay or (worse)
        // painting the whole text red.
        dbg('content', 'render:unavailable', { gen: myGen });
        return;
      }

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
          span.addEventListener('mousedown', (e: Event) => {
            e.preventDefault();
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

    input.addEventListener('input', scheduleRender);
    input.addEventListener('scroll', () => {
      // The overlay has overflow: hidden, so its own scrollTop is
      // ignored. We instead shift the overlay up by the textarea's
      // scrollTop so the visible portion of the overlay text lines
      // up with the visible portion of the textarea. The mirror's
      // overflow: hidden clips the off-screen part.
      positionOverlay();
    });
    window.addEventListener('scroll', syncBox, { passive: true });
    window.addEventListener('resize', syncBox);

    // The textarea can change size for reasons that don't fire a
    // window resize: auto-grow via JS, CSS-driven reflow, font-load,
    // etc. Watch the element directly so the host stays glued to it.
    if (typeof ResizeObserver !== 'undefined') {
      new ResizeObserver(() => syncBox()).observe(el);
    }
    // Web fonts can swap the rendered font long after our initial
    // syncBox(). Re-sync once the document reports all fonts loaded.
    if (typeof document.fonts !== 'undefined' && document.fonts.ready) {
      document.fonts.ready.then(() => {
        dbg('content', 'fonts.ready:resync');
        syncBox();
      }).catch(() => {});
    }

    // Click outside the popup closes it. Composed-path check so we
    // see through the shadow boundary.
    document.addEventListener('mousedown', (e: Event) => {
      if (focused === null) return;
      const path = e.composedPath();
      if (path.includes(popup)) return;
      if (path.some((n) => (n as HTMLElement).classList &&
                         (n as HTMLElement).classList.contains('misspell'))) return;
      closePopup();
    }, true);

    return {
      host,
      overlay,
      initialRender() { void render(); },
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
         initialRender: () => void; setVisible: (visible: boolean) => void;
         setDebug: (on: boolean) => void } {
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
      const suggestions = await workerClient.suggestWithStateAsync(info.word, 8);
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
      const hits = await workerClient.analyzeTextAsync(text, 8, ++renderGen);
      if (hits === null) {
        dbg('content', 'highlight:unavailable');
        return; // keep the previous ranges
      }
      const ranges: Range[] = [];
      misspellRegions.clear();
      for (const hit of hits) {
        const start = textNodeAt(hit.start);
        const end = textNodeAt(hit.end);
        if (!start || !end) continue;
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

    // Click hit-test: the CSS Highlight pseudo-element can't receive
    // events, so when the user clicks the contenteditable we look up
    // which misspell Region's bounding rects contain the click point.
    el.addEventListener('click', (e: MouseEvent) => {
      if (misspellRegions.size === 0) return;
      const x = e.clientX, y = e.clientY;
      for (const ms of misspellRegions.values()) {
        for (const r of Array.from(ms.range.getClientRects())) {
          if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
            dbg('content', 'click:on-misspell', { word: ms.word });
            openPopupAtWord(ms.word, ms.range);
            e.preventDefault();
            return;
          }
        }
      }
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
          btn.addEventListener('mousedown', (e: Event) => {
            e.preventDefault();
            replaceRange(range, sug);
            closePopup();
            void renderMisspellHighlights();
          });
          host.appendChild(btn);
        }
        host.style.display = 'block';
        host.style.left = (window.scrollX + rect.left) + 'px';
        host.style.top = (window.scrollY + rect.bottom + 2) + 'px';
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
    document.addEventListener('input', (e: Event) => {
      if (e.target === el || (e.target instanceof Node && el.contains(e.target))) {
        dbg('content', 'input:hit', { target: (e.target as Node).nodeName });
        scheduleCheck();
        if (HIGHLIGHTS_SUPPORTED) void renderMisspellHighlights();
      }
    }, true);

    // Close popup on click outside.
    document.addEventListener('mousedown', (e: Event) => {
      if (host.style.display === 'none') return;
      if (e.composedPath().includes(host)) return;
      dbg('content', 'click-outside:close');
      closePopup();
    }, true);

    return {
      host,
      overlay: host,
      initialRender() {
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
    attachFn: (el: HTMLElement) => ReturnType<typeof createOverlay> | null;
  }): { attach: (el: HTMLElement) => void; scan: (root: ParentNode) => void;
             attached: Set<HTMLElement> } {
    const { attachFn } = opts;
    const attached = new Set<HTMLElement>();

    function attach(el: HTMLElement): void {
      if (!el || attached.has(el)) return;
      if (opts.isSiteBlacklisted()) {
        opts.dbg('content', 'skip (blacklisted)', describeInput(el));
        return;
      }
      const overlay = attachFn(el);
      if (!overlay) return;
      attached.add(el);
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

    return { attach, scan, attached };
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
    });
  }

  // Bootstrap. The blacklist load resolves and runs the first scan
  // (only if the host isn't blacklisted). MutationObserver covers
  // anything that mounts later.
  blacklist.load();
})();