// Suggestion popup that appears under a focused misspelling.
//
// Keyboard nav: ArrowDown / ArrowUp move focus, Enter picks, click on
// outside closes.

import { createSignal, For, onCleanup, onMount, Show } from 'solid-js';

export interface SuggestionsPopupProps {
  word: string;
  x: number;
  y: number;
  suggestions: string[];
  onPick: (replacement: string) => void;
  onClose: () => void;
}

export function SuggestionsPopup(props: SuggestionsPopupProps) {
  let ref!: HTMLDivElement;
  const [focused, setFocused] = createSignal(0);

  onMount(() => {
    function onDocClick(e: MouseEvent) {
      if (!ref.contains(e.target as Node)) props.onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') { props.onClose(); return; }
      if (e.key === 'ArrowDown') {
        setFocused((f) => (f + 1) % Math.max(props.suggestions.length, 1));
        e.preventDefault();
        return;
      }
      if (e.key === 'ArrowUp') {
        setFocused((f) => (f - 1 + props.suggestions.length) % Math.max(props.suggestions.length, 1));
        e.preventDefault();
        return;
      }
      if (e.key === 'Enter' && props.suggestions[focused()]) {
        props.onPick(props.suggestions[focused()]);
        e.preventDefault();
      }
    }
    document.addEventListener('mousedown', onDocClick);
    document.addEventListener('keydown', onKey);
    onCleanup(() => {
      document.removeEventListener('mousedown', onDocClick);
      document.removeEventListener('keydown', onKey);
    });
  });

  return (
    <div ref={ref} class="suggestions" style={{ left: `${props.x}px`, top: `${props.y}px` }}>
      <Show
        when={props.suggestions.length > 0}
        fallback={<div class="empty">(no suggestions)</div>}
      >
        <For each={props.suggestions}>
          {(s, i) => (
            <button
              class={i() === focused() ? 'focused' : ''}
              onMouseDown={(e) => { e.preventDefault(); props.onPick(s); }}
              onMouseEnter={() => setFocused(i())}
            >
              {s}
            </button>
          )}
        </For>
      </Show>
    </div>
  );
}