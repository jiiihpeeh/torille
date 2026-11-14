import {
  OVERLAY_CSS, addTapHandler, disableNativeSpellcheck, ignoredRangesFor,
  restoreNativeSpellcheck, saveAndDisableNativeSpellcheck, uiSettings,
} from '../shared';
import { isIgnored, maskIgnored } from '../ignore-ranges';
import { partitionUnderlines, type Underline } from '../underline';
import { applySyntaxEdits } from '../grammar-panel';
import { appendSuggestionButtons } from './popup';
import type { Overlay, OverlayOptions } from './index';

// Overlay for `<textarea>` and `<input>`: shadow-DOM mirror over
// the element with a red-wavy underline on misspellings.
export function createTextareaOverlay(opts: OverlayOptions): Overlay {
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
    appendSuggestionButtons(popup, suggestions, (sug) => {
      replaceWordAt(start, end, sug);
      closePopup();
    });
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
  // Grammar and style results are opt-in: they arrive only after the
  // context-menu command, and they are cleared on the next edit so a stale
  // offset never paints over the wrong word.
  let grammarDiagnostics: R_SyntaxDiagnostic[] = [];
  let styleDiagnostics: R_SyntaxDiagnostic[] = [];

  function paint(text: string, hits: R_AnalyzeHit[]): void {
    const underlines: Underline[] = [];
    for (const hit of hits) {
      underlines.push({ start: hit.start, end: hit.end, layer: 'spelling' });
    }
    if (uiSettings.grammarUnderlines) {
      for (const diagnostic of grammarDiagnostics) {
        underlines.push({
          start: diagnostic.start,
          end: diagnostic.end,
          layer: 'grammar',
          severity: diagnostic.severity,
        });
      }
    }
    if (uiSettings.styleUnderlines) {
      for (const diagnostic of styleDiagnostics) {
        underlines.push({
          start: diagnostic.start,
          end: diagnostic.end,
          layer: 'style',
        });
      }
    }

    const regions = partitionUnderlines(text.length, underlines);
    overlay.textContent = '';
    let misspellCount = 0;
    for (const region of regions) {
      const slice = text.slice(region.start, region.end);
      const span = document.createElement('span');
      span.className = region.classes.length ? region.classes.join(' ') : 'token';
      span.textContent = slice;
      if (region.classes.includes('misspell')) {
        misspellCount++;
        span.dataset.word = slice;
        span.dataset.start = String(region.start);
        span.dataset.end = String(region.end);
        if (uiSettings.popupTrigger === 'hover') {
          span.addEventListener('mouseenter', () => {
            openPopup(span, slice, region.start, region.end);
          });
        } else {
          span.addEventListener('mousedown', (e: Event) => {
            e.preventDefault();
            openPopup(span, slice, region.start, region.end);
          });
        }
        // Hover is meaningless on touch, so a tap always opens the
        // dropdown regardless of the popup trigger setting.
        addTapHandler(span, () => {
          openPopup(span, slice, region.start, region.end);
        });
      }
      overlay.appendChild(span);
    }

    const overlayText = overlay.textContent ?? '';
    if (overlayText !== text) {
      dbg('content', 'render:drift',
          { textarea: text, overlay: overlayText,
            lenDiff: overlayText.length - text.length });
    }
    // The text may have grown taller than the textarea's viewport. Resize the
    // mirror before the next paint so scrolled-to lines keep their underlines.
    syncOverlayHeight();
    dbg('content', 'render:paint',
        { regions: regions.length, misspellings: misspellCount,
          grammar: grammarDiagnostics.length, style: styleDiagnostics.length });
  }

  function repaintCurrent(): void {
    const text = input.value;
    paint(text, lastHitsText === text ? lastHits : []);
  }

  function clearDiagnosticUnderlines(): void {
    grammarDiagnostics = [];
    styleDiagnostics = [];
    // Remove only the diagnostic classes from the existing spans. Repainting
    // here would drop the spelling underlines until the debounced render
    // catches up, which made every keystroke blink.
    for (const span of overlay.querySelectorAll(
      '.grammar-error, .grammar-warning, .grammar-suggestion, .style-suggestion',
    )) {
      span.classList.remove(
        'grammar-error', 'grammar-warning', 'grammar-suggestion', 'style-suggestion',
      );
    }
  }

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

    // Stash the latest hits at the factory scope so the popup's click
    // handler can read suggestions without a second wasm round-trip.
    lastHits = hits;
    lastHitsText = text;
    paint(text, hits);
    dbg('content', 'render:done',
        { gen: myGen, hits: hits?.length ?? 0 });
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

  function onInput(): void {
    workerClient.cancelCheckAsync();
    clearDiagnosticUnderlines();
    scheduleRender();
  }

  input.addEventListener('input', onInput);
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
    applyTextEdits(edits: R_SyntaxEdit[]): boolean {
      const current = input.value;
      const updated = applySyntaxEdits(current, edits);
      if (updated === current) return false;
      input.value = updated;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      return true;
    },
    setGrammarDiagnostics(diagnostics: R_SyntaxDiagnostic[]): void {
      grammarDiagnostics = diagnostics;
      repaintCurrent();
    },
    setStyleDiagnostics(diagnostics: R_SyntaxDiagnostic[]): void {
      styleDiagnostics = diagnostics;
      repaintCurrent();
    },
    initialRender() { void render(); },
    refresh() {
      repaintCurrent();
      scheduleRender();
    },
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
      input.removeEventListener('input', onInput);
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
