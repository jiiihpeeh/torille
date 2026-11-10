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
    // Keep the dropdown inside a narrow phone viewport. The position
    // comes from the misspelling's rect, so a word near the right or
    // bottom edge would otherwise push the menu off-screen.
    const parent = ref.offsetParent as HTMLElement | null;
    const origin = parent?.getBoundingClientRect() ?? { left: 0, top: 0 };
    const rect = ref.getBoundingClientRect();
    const margin = 8;
    if (rect.right > window.innerWidth - margin) {
      ref.style.left = `${Math.max(margin, window.innerWidth - margin - rect.width) - origin.left}px`;
    }
    if (rect.bottom > window.innerHeight - margin) {
      ref.style.top = `${Math.max(margin, window.innerHeight - margin - rect.height) - origin.top}px`;
    }

    function onDocClick(e: MouseEvent) {
      if (!ref.contains(e.target as Node)) props.onClose();
    }
    function onDocTouch(e: TouchEvent) {
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
    document.addEventListener('touchstart', onDocTouch, { passive: true });
    document.addEventListener('keydown', onKey);
    onCleanup(() => {
      document.removeEventListener('mousedown', onDocClick);
      document.removeEventListener('touchstart', onDocTouch);
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
          {(s, i) => {
            let chosen = false;
            const choose = () => {
              if (chosen) return;
              chosen = true;
              props.onPick(s);
            };
            return (
              <button
                class={i() === focused() ? 'focused' : ''}
                onMouseDown={(e) => { e.preventDefault(); choose(); }}
                onTouchEnd={(e) => { e.preventDefault(); choose(); }}
                onMouseEnter={() => setFocused(i())}
              >
                {s}
              </button>
            );
          }}
        </For>
      </Show>
    </div>
  );
}