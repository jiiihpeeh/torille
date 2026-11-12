// Shared suggestion-button list for both overlay popups.
//
// The textarea popup styles buttons through the shadow stylesheet, while the
// contenteditable popup is a light-DOM panel and needs inline styles. Both
// share the same click/tap behaviour and the same "first candidate is
// focused" convention, so only the decoration differs.

import { addTapHandler } from '../shared';

/**
 * Append one button per suggestion to `host`.
 *
 * `choose` runs at most once per button, on `mousedown` or a tap. `decorate`
 * may add inline styles for the light-DOM popup; the shadow popup leaves it
 * out and relies on `.suggestions button` in `OVERLAY_CSS`.
 */
export function appendSuggestionButtons(
  host: HTMLElement,
  suggestions: string[],
  choose: (suggestion: string) => void,
  decorate?: (button: HTMLButtonElement) => void,
): void {
  let picked = 0;
  for (const suggestion of suggestions) {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = suggestion;
    let chosen = false;
    const select = (): void => {
      if (chosen) return;
      chosen = true;
      choose(suggestion);
    };
    button.addEventListener('mousedown', (event: Event) => {
      event.preventDefault();
      select();
    });
    addTapHandler(button, select);
    if (picked === 0) button.classList.add('focused');
    decorate?.(button);
    picked++;
    host.appendChild(button);
  }
}
