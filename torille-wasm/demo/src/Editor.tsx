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

export interface EditorProps {
  text: string;
  onText: (next: string) => void;
  misspellings: Misspell[];
  focused: string | null;
  stateIdx: number;
  onFocus: (word: string) => void;
  onClose: () => void;
}

export function Editor(props: EditorProps) {
  const tokens = createMemo(() => tokenize(props.text));
  const misspellSet = createMemo(
    () => new Set(props.misspellings.map((m) => m.word)),
  );

  // The underlay is built as one HTML string with a `<span>` only for
  // each misspelled word. Rendering one node per token made the
  // underlay O(document) in the DOM: a 60 000-word paste created
  // ~120 000 nodes and hundreds of MB of JS heap, which no amount of
  // wasm-side chunking could offset. Misspells are rare, so the
  // string is almost entirely text nodes and the browser parses it in
  // one go. `.misspell` keeps `pointer-events: auto` in the CSS, so
  // clicks still land on the spans and bubble to the container
  // handler below.
  const escText = (s: string): string =>
    s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const escAttr = (s: string): string => escText(s).replace(/"/g, '&quot;');
  const underlayHtml = createMemo(() => {
    const miss = misspellSet();
    const focused = props.focused;
    let out = '';
    for (const tok of tokens()) {
      if (tok.kind === 'word' && miss.has(tok.text)) {
        const cls = tok.text === focused ? 'misspell focused' : 'misspell';
        out += `<span class="${cls}" data-word="${escAttr(tok.text)}">${escText(tok.text)}</span>`;
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
    { word: string; x: number; y: number; suggestions: string[] } | null
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
    const focused = props.focused;
    if (!focused) { setPopup(null); return; }
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
      const span = document.querySelector<HTMLElement>(
        '.editor .underlay .misspell.focused',
      );
      if (!span) return;
      const editor = span.closest<HTMLElement>('.editor')!.getBoundingClientRect();
      const r = span.getBoundingClientRect();
      setPopup({
        word: focused,
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
          // word. (User may have moved focus to a different
          // misspell by the time the wasm finishes.)
          const cur = popup();
          if (cur !== null && cur.word === focused) {
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

  let textareaRef!: HTMLTextAreaElement;
  onMount(() => {
    textareaRef.value = props.text;
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

  return (
    <div class="editor">
      <div
        class="underlay"
        innerHTML={underlayHtml()}
        onMouseDown={(e: MouseEvent) => {
          const el = (e.target as HTMLElement | null)?.closest?.(
            '.misspell',
          ) as HTMLElement | null;
          if (!el) return;
          e.preventDefault();
          props.onFocus(el.dataset.word ?? '');
        }}
      />
      <textarea
        ref={textareaRef}
        spellcheck={false}
        onInput={(e: InputEvent & { currentTarget: HTMLTextAreaElement }) =>
          writeText(e.currentTarget.value)}
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
              writeText(props.text.replaceAll(p().word, replacement));
              props.onClose();
            }}
            onClose={() => { props.onClose(); }}
          />
        )}
      </Show>
    </div>
  );
}