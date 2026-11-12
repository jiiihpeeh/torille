import {
  TARGET_SELECTOR, describeInput, type DebugFn, type DescribeInput,
} from './shared';
import type { Overlay } from './overlay';

// ─────────────────────────────────────────────────────────────────────
// §8  Scanner + MutationObserver
// ─────────────────────────────────────────────────────────────────────

// Walk a root (the document or any element) and register every
// matching text input. Records what's been attached so the
// blacklist can hide/show them, and so we don't double-attach.
export function createScanner(opts: {
  dbg: DebugFn;
  describeInput: DescribeInput;
  isSiteBlacklisted: () => boolean;
  shouldAttach: (el: HTMLElement) => boolean;
  attachFn: (el: HTMLElement) => Overlay | null;
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
