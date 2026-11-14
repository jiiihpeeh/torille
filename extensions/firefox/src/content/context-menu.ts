import { api, describeInput, type DebugFn, type DescribeInput } from './shared';
import { updateCheckProgress } from './grammar-panel';

// ─────────────────────────────────────────────────────────────────────
// §7  Per-textbox toggle factory
// ─────────────────────────────────────────────────────────────────────

// Firefox's `contextMenus.onClicked` doesn't tell us which DOM
// element was right-clicked; the content script captures the
// `contextmenu` event itself, remembers the input, and round-trips
// through the background when the menu item is toggled.
export function createTextboxToggle(opts: {
  dbg: DebugFn;
  describeInput: DescribeInput;
  setVisible: (el: HTMLElement, visible: boolean) => void;
  checkGrammar: (el: HTMLElement) => void;
  checkStyle: (el: HTMLElement) => void;
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
    if (m?.type === 'torille:check-progress') {
      updateCheckProgress(m.inputId, m.done, m.total);
      return;
    }
    if (m?.type === 'torille:check-grammar-pending-input') {
      const input = pendingInput;
      if (input) opts.checkGrammar(input);
      return Promise.resolve({ checked: !!input } as R_CheckGrammarResponse);
    }
    if (m?.type === 'torille:check-style-pending-input') {
      const input = pendingInput;
      if (input) opts.checkStyle(input);
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
