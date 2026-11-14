import {
  addTapHandler, disableNativeSpellcheck, ensureHighlightStyle, ignoredRangesFor,
  restoreNativeSpellcheck, saveAndDisableNativeSpellcheck, uiSettings,
} from '../shared';
import { isIgnored, isInsideCodeDom, maskIgnored } from '../ignore-ranges';
import { appendSuggestionButtons } from './popup';
import type { Overlay, OverlayOptions } from './index';

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
export function createContentEditableOverlay(opts: OverlayOptions): Overlay {
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
  // Grammar and style results arrive only from the opt-in panel commands.
  // They are cleared on the next edit, so their offsets never go stale.
  let grammarDiagnostics: R_SyntaxDiagnostic[] = [];
  let styleDiagnostics: R_SyntaxDiagnostic[] = [];

  function closePopup(): void {
    host.style.display = 'none';
    host.textContent = '';
  }

  /** The contenteditable popup is light DOM, so its buttons need inline styles. */
  function styleSuggestionButton(button: HTMLButtonElement): void {
    button.style.display = 'block';
    button.style.width = '100%';
    button.style.textAlign = 'left';
    button.style.background = 'none';
    button.style.border = '0';
    button.style.padding = '5px 12px';
    button.style.cursor = 'pointer';
    button.style.font = 'inherit';
    button.style.color = 'inherit';
    button.style.touchAction = 'manipulation';
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
    appendSuggestionButtons(host, suggestions, (sug) => {
      const range = document.createRange();
      range.setStart(info.node, info.start);
      range.setEnd(info.node, info.end);
      replaceRange(range, sug);
    }, styleSuggestionButton);
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

  const DIAGNOSTIC_HIGHLIGHTS = [
    'torille-grammar-error',
    'torille-grammar-warning',
    'torille-grammar-suggestion',
    'torille-style',
  ] as const;

  function highlightStore(): { set: (k: string, h: unknown) => void;
                               delete: (k: string) => void } {
    return (CSS as unknown as {
      highlights: { set: (k: string, h: unknown) => void;
                    delete: (k: string) => void };
    }).highlights;
  }

  function setDiagnosticHighlight(name: string, ranges: Range[]): void {
    const highlights = highlightStore();
    if (ranges.length === 0) {
      highlights.delete(name);
      return;
    }
    const highlightCtor = (globalThis as unknown as {
      Highlight: new (...ranges: AbstractRange[]) => unknown;
    }).Highlight;
    highlights.set(name, new highlightCtor(...ranges));
  }

  function diagnosticRange(start: number, end: number): Range | null {
    const from = textNodeAt(start);
    const to = textNodeAt(end);
    if (!from || !to) return null;
    const range = document.createRange();
    try {
      range.setStart(from.node, from.offset);
      range.setEnd(to.node, to.offset);
    } catch (e) {
      dbg('content', 'diagnostic-highlight:range-failed', { err: String(e) });
      return null;
    }
    return range;
  }

  function renderDiagnosticHighlights(): void {
    if (!HIGHLIGHTS_SUPPORTED) return;
    const grammar: Record<string, Range[]> = {
      'torille-grammar-error': [],
      'torille-grammar-warning': [],
      'torille-grammar-suggestion': [],
    };
    if (uiSettings.grammarUnderlines) {
      for (const diagnostic of grammarDiagnostics) {
        const range = diagnosticRange(diagnostic.start, diagnostic.end);
        if (!range) continue;
        const name = diagnostic.severity === 'error'
          ? 'torille-grammar-error'
          : diagnostic.severity === 'warning'
            ? 'torille-grammar-warning'
            : 'torille-grammar-suggestion';
        grammar[name].push(range);
      }
    }
    for (const [name, ranges] of Object.entries(grammar)) {
      setDiagnosticHighlight(name, ranges);
    }

    const style: Range[] = [];
    if (uiSettings.styleUnderlines) {
      for (const diagnostic of styleDiagnostics) {
        const range = diagnosticRange(diagnostic.start, diagnostic.end);
        if (range) style.push(range);
      }
    }
    setDiagnosticHighlight('torille-style', style);
    dbg('content', 'diagnostic-highlights',
        { grammar: grammarDiagnostics.length, style: styleDiagnostics.length });
  }

  function clearDiagnosticHighlights(): void {
    grammarDiagnostics = [];
    styleDiagnostics = [];
    if (!HIGHLIGHTS_SUPPORTED) return;
    const highlights = highlightStore();
    for (const name of DIAGNOSTIC_HIGHLIGHTS) highlights.delete(name);
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
      appendSuggestionButtons(host, suggestions, (sug) => {
        replaceRange(range, sug);
        closePopup();
        void renderMisspellHighlights();
      }, styleSuggestionButton);
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
      workerClient.cancelCheckAsync();
      clearDiagnosticHighlights();
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
    applyTextEdits(edits: R_SyntaxEdit[]): boolean {
      const ordered = [...edits].sort(
        (a, b) => (b.start - a.start) || (b.end - a.end));
      let changed = false;
      for (const edit of ordered) {
        const start = textNodeAt(edit.start);
        const end = textNodeAt(edit.end);
        if (!start || !end) continue;
        const range = document.createRange();
        range.setStart(start.node, start.offset);
        range.setEnd(end.node, end.offset);
        range.deleteContents();
        if (edit.replacement !== '') {
          range.insertNode(document.createTextNode(edit.replacement));
        }
        changed = true;
      }
      if (changed) {
        el.dispatchEvent(new Event('input', { bubbles: true }));
      }
      return changed;
    },
    setGrammarDiagnostics(diagnostics: R_SyntaxDiagnostic[]): void {
      grammarDiagnostics = diagnostics;
      renderDiagnosticHighlights();
    },
    setStyleDiagnostics(diagnostics: R_SyntaxDiagnostic[]): void {
      styleDiagnostics = diagnostics;
      renderDiagnosticHighlights();
    },
    initialRender() {
      if (HIGHLIGHTS_SUPPORTED) void renderMisspellHighlights();
    },
    refresh() {
      renderDiagnosticHighlights();
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
      clearDiagnosticHighlights();
      restoreNativeSpellcheck(el, nativeAttrs);
      el.removeEventListener('click', onEditorClick);
      document.removeEventListener('input', onDocumentInput, true);
      document.removeEventListener('mousedown', onDocumentMouseDown, true);
      workerClient.tearDownState();
      host.remove();
    },
  };
}
