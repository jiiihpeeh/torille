// Editor: a `<textarea>` overlaid on a coloured underlay that draws
// the same text with misspellings underlined.
//
// The textarea is uncontrolled: setting `value` on every keystroke
// makes the browser re-position the caret to the end of the new text,
// which is the lag you feel while typing. We let the browser own the
// textarea's value and only mirror its content into the `text`
// signal via the `input` event. The underlay (which is the only
// consumer that needs the latest text) reads from that signal.

import { createEffect, createMemo, createSignal, onCleanup, onMount, Show } from 'solid-js';
import { tokenize } from './tokenize';
import {
  suggestWithState,
  suggestForMisspellAsync,
  getCachedSuggestions,
  getCachedSuggestionsForState,
  putCachedSuggestionsForState,
} from './spell-client';
import { Misspell } from './find-misspellings';
import { SuggestionsPopup } from './SuggestionsPopup';

/** The misspelled word the popup is open for.
 *
 *  `start` is the JS-string offset of this occurrence in the
 *  textarea, so the same surface form at two positions opens its
 *  own popup and replaces only itself. `-1` means "first
 *  occurrence", used by the debug/probe surface, which only has a
 *  word string. */
export interface FocusTarget {
  word: string;
  start: number;
}

export interface EditorProps {
  text: string;
  onText: (next: string) => void;
  misspellings: Misspell[];
  focused: FocusTarget | null;
  stateIdx: number;
  onFocus: (target: FocusTarget) => void;
  onClose: () => void;
}

export function Editor(props: EditorProps) {
  let textareaRef!: HTMLTextAreaElement;
  let editorRef!: HTMLDivElement;
  // Normalise CRLF before tokenising. A `<textarea>` normalises its
  // value to LF, and the HTML parser normalises CRLF in the underlay
  // markup to LF, but the offsets in `props.text` would still count the
  // CRs. That mismatch made clicking a word in the Gutenberg reader
  // resolve to an earlier token (its chapters use CRLF), so the popup
  // never matched the clicked misspelling. Tokenising the normalised
  // text keeps token offsets, textarea caret offsets and underlay
  // offsets in the same coordinate space.
  const tokens = createMemo(() =>
    tokenize(props.text.replace(/\r\n?/g, '\n')),
  );
  const misspellSet = createMemo(
    () => new Set(props.misspellings.map((m) => m.word)),
  );

  // Resolve a word-only focus (the probe API) to its first
  // occurrence. The underlay and the popup both use this, so the
  // `.focused` outline and the popup still agree when the caller
  // did not name an occurrence.
  const resolvedFocus = createMemo<FocusTarget | null>(() => {
    const f = props.focused;
    if (!f) return null;
    if (f.start >= 0) return f;
    const tok = tokens().find(
      (t) => t.kind === 'word' && t.text === f.word,
    );
    return tok && tok.kind === 'word'
      ? { word: f.word, start: tok.start }
      : f;
  });

  // Starts of the misspelled occurrences currently touched by the
  // selection. Only the *set* matters for the render, so a drag that
  // stays inside one word does not rebuild the underlay on every
  // `selectionchange`. Used to drop the tint while a word is selected:
  // the native selection highlight lives in the textarea *below* the
  // underlay, so an opaque tint on the span would paint over it and
  // make the selection look different from a correctly spelled word.
  // The squiggle (a text-decoration) stays.
  const [selectedSpans, setSelectedSpans] = createSignal<Set<number>>(
    new Set(),
  );
  let lastSelectionKey = '';
  const syncSelection = (): void => {
    if (!textareaRef) return;
    const active = document.activeElement === textareaRef;
    const start = active ? textareaRef.selectionStart : 0;
    const end = active ? textareaRef.selectionEnd : 0;
    let key = '';
    if (start !== end) {
      const starts: number[] = [];
      for (const tok of tokens()) {
        if (
          tok.kind === 'word' &&
          misspellSet().has(tok.text) &&
          tok.start < end &&
          tok.end > start
        ) {
          starts.push(tok.start);
        }
      }
      key = starts.join(',');
    }
    if (key === lastSelectionKey) return;
    lastSelectionKey = key;
    setSelectedSpans(new Set(key === '' ? [] : key.split(',').map(Number)));
  };
  // A misspell list or text update can change which spans the current
  // selection touches (e.g. the slow check demotes a word while it is
  // selected). Recompute the set when those dependencies change.
  createEffect(() => {
    tokens();
    misspellSet();
    syncSelection();
  });

  // Hover affordance: after the pointer rests on an underlined word
  // for a moment, switch the textarea's cursor to `pointer`. The
  // underlay is `pointer-events: none` (so selection works normally),
  // which means `:hover` cannot see the spans; we hit-test the pointer
  // against the spans' client rects instead. The small delay keeps a
  // word that merely passes under the cursor while typing/moving from
  // flashing into a clickable style.
  const HOVER_DELAY_MS = 350;
  const [pointerCursor, setPointerCursor] = createSignal(false);
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  let hoverStart: number | null = null;
  let hoverFrame = 0;
  let hoverPoint: { x: number; y: number } | null = null;

  const clearHover = (): void => {
    if (hoverTimer !== undefined) {
      clearTimeout(hoverTimer);
      hoverTimer = undefined;
    }
    hoverStart = null;
    setPointerCursor(false);
  };

  const misspellAtPoint = (x: number, y: number): number | null => {
    const root = editorRef;
    if (!root) return null;
    const spans = root.querySelectorAll<HTMLElement>(
      '.underlay .misspell',
    );
    for (const span of spans) {
      for (const r of span.getClientRects()) {
        if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) {
          return Number(span.dataset.start ?? '-1');
        }
      }
    }
    return null;
  };

  const onHoverMove = (e: MouseEvent): void => {
    hoverPoint = { x: e.clientX, y: e.clientY };
    if (hoverFrame !== 0) return;
    // Coalesce mousemove storms to one hit-test per frame; a reader
    // page can hold hundreds of underlined spans.
    hoverFrame = requestAnimationFrame(() => {
      hoverFrame = 0;
      const p = hoverPoint;
      if (!p) return;
      const hit = misspellAtPoint(p.x, p.y);
      if (hit === null) {
        clearHover();
        return;
      }
      if (hit === hoverStart) return;
      if (hoverTimer !== undefined) clearTimeout(hoverTimer);
      hoverStart = hit;
      setPointerCursor(false);
      hoverTimer = setTimeout(() => {
        hoverTimer = undefined;
        if (hoverStart === hit) setPointerCursor(true);
      }, HOVER_DELAY_MS);
    });
  };

  const leaveHover = (): void => {
    if (hoverFrame !== 0) {
      cancelAnimationFrame(hoverFrame);
      hoverFrame = 0;
    }
    hoverPoint = null;
    clearHover();
  };

  // The underlay is built as one HTML string with a `<span>` only for
  // each misspelled word. Rendering one node per token made the
  // underlay O(document) in the DOM: a 60 000-word paste created
  // ~120 000 nodes and hundreds of MB of JS heap, which no amount of
  // wasm-side chunking could offset. Misspells are rare, so the
  // string is almost entirely text nodes and the browser parses it in
  // one go. The underlay is `pointer-events: none`, so the textarea
  // owns every mouse interaction (caret, selection, dragging); the
  // container handler below only serves synthetic span events from
  // the probes.
  const escText = (s: string): string =>
    s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const escAttr = (s: string): string => escText(s).replace(/"/g, '&quot;');
  const underlayHtml = createMemo(() => {
    const miss = misspellSet();
    const focus = resolvedFocus();
    const selected = selectedSpans();
    let out = '';
    for (const tok of tokens()) {
      if (tok.kind === 'word' && miss.has(tok.text)) {
        const classes = ['misspell'];
        if (
          focus !== null &&
          tok.text === focus.word &&
          tok.start === focus.start
        ) {
          classes.push('focused');
        }
        if (selected.has(tok.start)) {
          // Selected: keep the underline, hide the tint so the
          // textarea's native selection highlight shows through.
          classes.push('selected');
        }
        out += `<span class="${classes.join(' ')}" data-word="${escAttr(tok.text)}" data-start="${tok.start}">${escText(tok.text)}</span>`;
      } else {
        out += escText(tok.text);
      }
    }
    return out;
  });

  // After each focused-word change, find the on-screen span and
  // open the popup. Two source-of-truth options for suggestions:
  //   * the side cache (filled by the background second pass,
  //     instant read),
  //   * `suggestForMisspellAsync` (asks the worker, hits the
  //     worker's own LRU first, may fall to the wasm).
  // We try the side cache first; the popup opens immediately if
  // it lands; if it doesn't, fall back to a worker fetch and
  // show what comes back.
  const [popup, setPopup] = createSignal<
    {
      word: string;
      start: number;
      x: number;
      y: number;
      suggestions: string[];
    } | null
  >(null);

  // Popup debug entry. Each Editor registers its own popup
  // with the global `__torille.popups` so the puppeteer probe
  // can `await` stability (the popup's `suggestions` array
  // stops changing) without polling the DOM or guessing a
  // fixed grace period.
  let popupUpdate: ((suggestions: string[]) => void) | null = null;
  let popupUnregister: (() => void) | null = null;
  onMount(() => {
    import('./debug').then(({ registerPopup, unregisterPopup }) => {
      const { entry, update } = registerPopup();
      popupUpdate = update;
      popupUnregister = () => unregisterPopup(entry);
    });
  });
  onCleanup(() => {
    popupUnregister?.();
  });

  // Mirror the popup's suggestions into the debug surface
  // whenever the popup signal changes. The reference check
  // in `registerPopup`'s `update` keeps the timestamp
  // monotonic with actual content changes, so the probe
  // can compare `Object.is(prev, current)` to detect
  // stability without a fixed grace period.
  createEffect(() => {
    const p = popup();
    if (!popupUpdate) return;
    popupUpdate(p?.suggestions ?? []);
  });
  createEffect(() => {
    const target = resolvedFocus();
    if (!target) { setPopup(null); return; }
    const focused = target.word;
    const idx = props.stateIdx;
    if (idx < 0) return;
    // Two caches: lemma-only (background pass) and
    // state-aware (proper nouns). State-aware is preferred --
    // it's the actual inflection-aware set the user is most
    // likely to pick.
    const cachedState = getCachedSuggestionsForState(
      idx, focused, /* max */ 8, /* dist */ 3,
    );
    const cached = cachedState
      ?? getCachedSuggestions(focused, /* max */ 8, /* dist */ 3);
    requestAnimationFrame(() => {
      // Scope the lookup to THIS editor: two panels can each show
      // the same misspelling. Within the panel, prefer the span
      // carrying this occurrence's offset, so the popup opens
      // under the word the user clicked rather than the first
      // copy of it.
      const root = editorRef;
      if (!root) return;
      const span = root.querySelector<HTMLElement>(
        target.start >= 0
          ? `.underlay .misspell[data-start="${target.start}"]`
          : '.underlay .misspell.focused',
      );
      if (!span) return;
      const editor = root.getBoundingClientRect();
      const r = span.getBoundingClientRect();
      setPopup({
        word: focused,
        start: target.start,
        x: r.left - editor.left,
        y: r.bottom - editor.top + 4,
        suggestions: cached ?? [],
      });
    });
    if (cachedState === undefined) {
      // Slow path: prefer the per-textbox proper-noun-aware
      // suggester so `talossa` -> `talo` (edit distance 3,
      // recovered only by the inflection-aware ranker) is
      // included. Memoised at the worker through the LRU it
      // sets up internally; first click pays the cost
      // (sub-second typically), repeat clicks hit cache --
      // including those that come from another text input
      // that finished this same word's background pass.
      suggestWithState(idx, focused, /* max */ 8, /* dist */ 3)
        .then((suggestions) => {
          // Only update if the popup is still showing this
          // occurrence. (User may have moved focus to a
          // different misspell by the time the wasm finishes.)
          const cur = popup();
          if (cur !== null && cur.word === focused && cur.start === target.start) {
            setPopup({ ...cur, suggestions });
          }
          // Also seed the client-side side cache under the
          // (idx-aware) key the popup read from, so the next
          // open lands in the cache rather than firing
          // another worker fetch.
          putCachedSuggestionsForState(
            idx, focused, /* max */ 8, /* dist */ 3, suggestions,
          );
        });
      // Also kick the lemma-only path in the background so
      // subsequent renders of the same word already have *some*
      // answer in the side cache (visible as the popup's first
      // paint for ~10ms before the proper-noun-aware list
      // arrives).
      void suggestForMisspellAsync(idx, focused, /* max */ 8, /* dist */ 3);
    }
  });

  onMount(() => {
    textareaRef.value = props.text;
    // Selection may be dragged, shifted with the keyboard, or set
    // programmatically. `selectionchange` on the document covers all
    // of them; the textarea's own events are a belt-and-braces path
    // for browsers that fire it late.
    document.addEventListener('selectionchange', syncSelection);
    textareaRef.addEventListener('select', syncSelection);
    textareaRef.addEventListener('mouseup', syncSelection);
    textareaRef.addEventListener('keyup', syncSelection);
  });
  onCleanup(() => {
    document.removeEventListener('selectionchange', syncSelection);
    textareaRef?.removeEventListener('select', syncSelection);
    textareaRef?.removeEventListener('mouseup', syncSelection);
    textareaRef?.removeEventListener('keyup', syncSelection);
    leaveHover();
  });

  // External text sync. The Editor is conceptually a controlled
  // component but uses an uncontrolled `<textarea>` for caret /
  // IME stability; when `props.text` changes from outside the
  // user (e.g. the Gutenberg reader's chapter flip), mirror the
  // new value into the textarea. The no-op guard (`!==`) keeps
  // user-typed keystrokes from racing the effect: the user
  // keystroke already updated `textareaRef.value` before the
  // effect re-ran, so the equality check makes the effect a
  // pass-through.
  createEffect(() => {
    const next = props.text;
    if (textareaRef && textareaRef.value !== next) {
      textareaRef.value = next;
    }
  });

  // The single place that writes to the textarea. Skips when the
  // value already matches (no-op that doesn't move the caret); only
  // writes when an external actor -- a picked suggestion -- asks the
  // textarea to take a new value while the user hasn't typed.
  const writeText = (next: string): void => {
    if (textareaRef && textareaRef.value !== next) {
      textareaRef.value = next;
    }
    props.onText(next);
  };

  // Which word the caret sits in. Used when the user clicks the
  // textarea: the underlay is `pointer-events: none`, so selection,
  // caret movement and dragging are all handled by the textarea
  // itself, and a plain click (no drag) opens the popup for the
  // occurrence under the caret.
  const wordAtCaret = (): FocusTarget | null => {
    if (!textareaRef) return null;
    if (textareaRef.selectionStart !== textareaRef.selectionEnd) {
      return null;
    }
    const pos = textareaRef.selectionStart;
    let atBoundary: FocusTarget | null = null;
    for (const tok of tokens()) {
      if (tok.kind !== 'word') continue;
      if (pos >= tok.start && pos < tok.end) {
        return { word: tok.text, start: tok.start };
      }
      if (pos === tok.end) {
        atBoundary = { word: tok.text, start: tok.start };
      }
    }
    return atBoundary;
  };

  const openAtCaret = (): void => {
    const target = wordAtCaret();
    if (target && misspellSet().has(target.word)) {
      props.onFocus(target);
    } else {
      props.onClose();
    }
    syncSelection();
  };

  return (
    <div class="editor" ref={editorRef}>
      <div
        class="underlay"
        innerHTML={underlayHtml()}
        onMouseDown={(e: MouseEvent) => {
          // Real clicks land on the textarea (the underlay is
          // pointer-events: none); this path exists for the
          // synthetic events the probes dispatch on a span, and
          // would also keep a span click working if the CSS ever
          // re-enables pointer events for them.
          const el = (e.target as HTMLElement | null)?.closest?.(
            '.misspell',
          ) as HTMLElement | null;
          if (!el) return;
          e.preventDefault();
          const word = el.dataset.word ?? '';
          const start = Number(el.dataset.start ?? '-1');
          props.onFocus({ word, start: Number.isFinite(start) ? start : -1 });
        }}
      />
      <textarea
        ref={textareaRef}
        spellcheck={false}
        classList={{ 'over-misspell': pointerCursor() }}
        onInput={(e: InputEvent & { currentTarget: HTMLTextAreaElement }) =>
          writeText(e.currentTarget.value)}
        onClick={openAtCaret}
        onMouseMove={onHoverMove}
        onMouseLeave={leaveHover}
        placeholder="Type some Finnish prose…"
      />
      <Show when={popup()}>
        {(p) => (
          <SuggestionsPopup
            word={p().word}
            x={p().x}
            y={p().y}
            suggestions={p().suggestions}
            onPick={(replacement) => {
              // Replace only the occurrence the popup belongs to.
              // `replaceAll` would silently rewrite every copy of the
              // same misspelling, including ones the user never
              // clicked.
              const base = textareaRef.value;
              let start = p().start;
              const word = p().word;
              if (
                start < 0 ||
                base.slice(start, start + word.length) !== word
              ) {
                start = base.indexOf(word);
              }
              if (start < 0) {
                props.onClose();
                return;
              }
              writeText(
                base.slice(0, start) +
                  replacement +
                  base.slice(start + word.length),
              );
              props.onClose();
            }}
            onClose={() => { props.onClose(); }}
          />
        )}
      </Show>
    </div>
  );
}