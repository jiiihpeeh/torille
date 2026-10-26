// Demo-side debug surface for the Puppeteer probes.
//
// The probes used to coordinate with the demo's async machinery
// (wasm boot, first-pass analyzer, slow-check pool, popup
// stabilisation) through fixed `setTimeout` budgets -- 3 s for
// wasm boot, 1.5 s for the slow check, 6 s for the popup to
// stabilise. Each budget was a guess; each was wrong for some
// text length / machine load / wasm build. This module replaces
// those guesses with explicit completion signals the probe can
// `await` on:
//
//   `__torille.ready`               -- Promise<void>  wasm up
//   `__torille.panels[i].setText`   -- programmatic text setter
//                                       (goes through the same
//                                       input-event path as
//                                       user typing, so Solid
//                                       state + wasm state stay
//                                       in sync)
//   `__torille.panels[i].onRenderDone(cb)` -- fires when a render's
//                                       first-pass + slow-check
//                                       pool has settled (or
//                                       been cancelled by a
//                                       newer render)
//   `__torille.popups[i].stableSince` -- Date, the last time the
//                                       popup's `suggestions`
//                                       array's reference
//                                       changed; probes can
//                                       poll for a stable
//                                       reference (no fixed
//                                       grace period)
//
// All four are mounted on the global `window.__torille` so the
// probe can `page.evaluate(() => window.__torille.ready)` without
// imports. `panels` is keyed by mount order; `popups` likewise.

import { createSignal } from 'solid-js';

interface PanelEntry {
  heading: () => string;
  candidates: () => unknown[];
  demoted: () => Set<string>;
  misspellings: () => unknown[];
  setText: (text: string) => void;
  /**
   * Subscribe to render completion. The callback fires when a
   * render's spell-check round is *fully settled* -- the
   * first-pass is in, the slow-check pool has drained, and
   * the candidate list is stable. A new render that cancels
   * the in-flight round (via the `isCancelled` closure
   * added in `find-misspellings.ts`) does NOT fire this
   * callback for the old render; only the latest active
   * round gets the signal. Returns an unsubscribe fn.
   */
  onRenderDone: (cb: () => void) => () => void;
}

interface PopupEntry {
  /** Reactive getter for the current suggestions array. The
   *  reference changes on every popup update, so a probe can
   *  poll for `Object.is(prev, current) === true` to detect
   *  "no update since last check" without a fixed grace
   *  period. */
  suggestions: () => string[];
  /** `Date.now()` of the last `suggestions` reference
   *  change. Polled by the probe to wait for stability. */
  stableSince: () => number;
}

declare global {
  interface Window {
    __torille?: {
      ready: Promise<void>;
      panels: PanelEntry[];
      popups: PopupEntry[];
    };
  }
}

const POPUP_STABLE_GRACE_MS = 200;

/**
 * Install the global `window.__torille` object. Idempotent --
 * the first call sets up `ready` and the `panels` /
 * `popups` arrays; subsequent calls only ensure the
 * arrays exist. Call from each panel/popup owner.
 */
export function ensureDebugRoot(): NonNullable<Window['__torille']> {
  if (typeof window === 'undefined') {
    // Non-browser (SSR, tests); the surface is meaningless.
    return {
      ready: Promise.resolve(),
      panels: [],
      popups: [],
    };
  }
  const w = window as Window;
  if (!w.__torille) {
    w.__torille = {
      ready: Promise.resolve(),
      panels: [],
      popups: [],
    };
  }
  return w.__torille;
}

/**
 * Register a wasm-ready promise. Called once from
 * `spell-client.ts` so the test can `await
 * window.__torille.ready` to know the wasm has booted.
 */
export function setReadyPromise(p: Promise<void>): void {
  if (typeof window === 'undefined') return;
  const w = ensureDebugRoot();
  w.ready = p;
}

/**
 * Register a panel. Called from TextPanel / GutenbergReader
 * on mount. The returned entry should be the same object
 * stored in `__torille.panels` so the test can read its
 * signals directly.
 */
export function registerPanel(
  signals: Omit<PanelEntry, 'onRenderDone'>,
  onRenderDone: (cb: () => void) => () => void,
): PanelEntry {
  const root = ensureDebugRoot();
  const entry: PanelEntry = { ...signals, onRenderDone };
  root.panels.push(entry);
  return entry;
}

/**
 * Remove a panel on unmount. Matches `registerPanel`'s
 * entry by reference and splices it out of the array.
 */
export function unregisterPanel(entry: PanelEntry): void {
  const root = ensureDebugRoot();
  const i = root.panels.indexOf(entry);
  if (i >= 0) root.panels.splice(i, 1);
}

/**
 * Create a popup entry. Each Editor that opens a popup
 * registers its own entry. Returns the entry plus an
 * `update(suggestions)` function the popup calls on every
 * render to bump `stableSince`.
 */
export function registerPopup(): {
  entry: PopupEntry;
  update: (suggestions: string[]) => void;
} {
  const root = ensureDebugRoot();
  const [suggestions, setSuggestions] = createSignal<string[]>(
    [],
    { equals: false },
  );
  const [stableSince, setStableSince] = createSignal(Date.now());
  const entry: PopupEntry = {
    suggestions,
    stableSince,
  };
  root.popups.push(entry);
  let last: string[] = [];
  return {
    entry,
    update(next: string[]) {
      // Only bump `stableSince` when the popup's content
      // reference actually changes -- this is what lets a
      // probe do `Object.is(prev, current)` checks without
      // a fixed grace period.
      if (next !== last) {
        last = next;
        setSuggestions(next);
        setStableSince(Date.now());
      }
    },
  };
}

export function unregisterPopup(entry: PopupEntry): void {
  const root = ensureDebugRoot();
  const i = root.popups.indexOf(entry);
  if (i >= 0) root.popups.splice(i, 1);
}

/** Convenience: the grace period the probe should wait past
 *  `stableSince` to be sure no further popup update is
 *  coming. Documented here so the probe and the popup
 *  agree on the same number. */
export const POPUP_STABLE_GRACE = POPUP_STABLE_GRACE_MS;
