// Content-script entry point.
//
// This file wires the modules together: it loads the settings, builds the
// scanner and the worker client, and starts the page observer. The overlay,
// worker client, grammar panel, blacklist and context-menu tracking live in
// sibling modules; esbuild bundles them into one classic content script.
//
// File layout:
//   shared.ts             debug shim, UI settings, native spellcheck, DOM helpers
//   worker-client.ts      runtime message wrappers
//   overlay/              textarea and contenteditable overlays + popup list
//   grammar-panel.ts      Finnish labels, edit application, panel
//   context-menu.ts       pending-input tracking and menu messages
//   blacklist.ts          per-host blacklist
//   scanner.ts            input discovery + MutationObserver
//   ignore-ranges.ts      code/BBCode/URL mask

import { createBlacklist } from './blacklist';
import { createTextboxToggle } from './context-menu';
import { editorText, showGrammarPanel, showStylePanel } from './grammar-panel';
import { createOverlay } from './overlay';
import { createScanner } from './scanner';
import { isIgnored } from './ignore-ranges';
import {
  UI_SETTINGS, api, dbg, describeInput, ignoredRangesFor, isCodeEditorField,
  isSearchField, loadUiSettings, setDebugFlag, uiSettings,
} from './shared';
import { createWorkerClient } from './worker-client';

// Per-overlay `inputId` counter. Each textarea / contenteditable
// attached by the scanner gets the next number, which the worker
// client bakes into its batch payloads and verifies on responses.
// Page-scoped, monotonic across the lifetime of this content
// script -- the scanner calls `nextInputId()` once per `attach`.
let nextInputId = 1;

// Opt-in: search boxes are skipped unless the options page enables
// `torille-searchboxes`. Missing key means off.
let checkSearchboxes = false;

const perInput = new Map<HTMLElement, ReturnType<typeof createOverlay>>();

// Hide or show the host element for one input. Used by the
// per-textbox toggle and the blacklist.
function setVisible(el: HTMLElement, visible: boolean): void {
  const o = perInput.get(el);
  if (o) o.setVisible(visible);
  else {
    // The input was skipped (e.g., blacklisted); just store the
    // intent so a later refreshOverlays can apply it.
    pendingVisibility.set(el, visible);
  }
}
const pendingVisibility = new WeakMap<HTMLElement, boolean>();

const scanner = createScanner({
  dbg,
  describeInput,
  isSiteBlacklisted: () => blacklist.isSiteBlacklisted(),
  shouldAttach: (el) =>
    !isCodeEditorField(el) && (checkSearchboxes || !isSearchField(el)),
  detachFn: (el) => {
    perInput.get(el)?.destroy();
    perInput.delete(el);
    pendingVisibility.delete(el);
    dbg('input:detach', { which: describeInput(el) });
  },
  attachFn: (el) => {
    // One worker client per overlay, so each textarea's batch
    // messages carry their own `inputId`. The factory is cheap
    // (it's just a closure over `dbg` + an integer), so per-overlay
    // allocation isn't measurable.
    const inputId = nextInputId++;
    const workerClient = createWorkerClient({ dbg, inputId });
    // Allocate the per-textbox wasm state on attach. The slot
    // lives for the lifetime of this overlay (we don't yet
    // detect overlay teardown when the DOM element is removed;
    // a future patch wires the scanner's `detach` callback
    // through `workerClient.destroyStateAsync`).
    workerClient.createStateAsync().then((idx) => {
      if (idx !== null) {
        dbg('content', 'createState', { inputId, idx });
      }
    });
    const overlay = createOverlay({ el, workerClient, dbg, describeInput });
    perInput.set(el, overlay);
    dbg('input:attach', { inputId, ...{ which: describeInput(el) } });
    overlay.initialRender();
    // Apply any visibility the toggle stored before the overlay
    // existed.
    const wanted = pendingVisibility.get(el);
    if (wanted !== undefined) overlay.setVisible(wanted);
    return overlay;
  },
});

function refreshSearchFields(): void {
  if (checkSearchboxes) {
    scanner.scan(document);
    return;
  }
  for (const el of Array.from(perInput.keys())) {
    if (isSearchField(el)) scanner.detach(el);
  }
}

async function loadSearchSetting(): Promise<void> {
  const cur = await api.storage.local.get('torille-searchboxes');
  checkSearchboxes = cur['torille-searchboxes'] === true;
  dbg('content', 'searchboxes setting ->', checkSearchboxes);
  refreshSearchFields();
}

const blacklist = createBlacklist({
  dbg,
  refreshOverlays({ reScan }: { reScan: boolean }) {
    const blacklisted = blacklist.isSiteBlacklisted();
    dbg('content', 'refreshOverlays',
        { blacklisted, visible: !blacklisted, count: perInput.size });
    for (const [, o] of perInput) o.setVisible(!blacklisted);
    if (reScan) scanner.scan(document);
  },
});

createTextboxToggle({
  dbg,
  describeInput,
  setVisible,
  checkGrammar(input: HTMLElement): void {
    const overlay = perInput.get(input);
    if (!overlay) return;
    const text = editorText(input);
    overlay.workerClient.checkSyntaxAsync(text).then((diagnostics) => {
      if (diagnostics === null) return;
      showGrammarPanel(input, diagnostics, (correction) => {
        // If the field changed after the check, the offsets are stale.
        if (editorText(input) !== text) return false;
        return overlay.applyTextEdits(correction.edits);
      });
    }).catch((e: unknown) => {
      dbg('content', 'checkGrammar failed', String(e));
    });
  },
  checkStyle(input: HTMLElement): void {
    const overlay = perInput.get(input);
    if (!overlay) return;
    const text = editorText(input);
    overlay.workerClient.checkStyleAsync(text).then((diagnostics) => {
      if (diagnostics === null) return;
      // Code blocks, URLs and BBCode are not style prose. Drop any finding
      // that intersects an ignored range, exactly as the spell overlay does.
      const ignored = ignoredRangesFor(text);
      const visible = ignored.length === 0
        ? diagnostics
        : diagnostics.filter(
            (diagnostic) => !isIgnored(
              { start: diagnostic.start, end: diagnostic.end }, ignored),
          );
      showStylePanel(input, visible, (correction) => {
        // If the field changed after the check, the offsets are stale.
        if (editorText(input) !== text) return false;
        return overlay.applyTextEdits(correction.edits);
      });
    }).catch((e: unknown) => {
      dbg('content', 'checkStyle failed', String(e));
    });
  },
});

// Listen for the options-page debug toggle and to push the flag
// into the worker. The flag is global, so any one of the per-input
// worker clients can carry it -- the background forwards the
// 'torille:debug' message to the wasm worker regardless of inputId.
// If no overlay exists yet (debug toggled before any input is
// attached), we drop it; nothing to debug anyway.
if (api.storage?.onChanged) {
  api.storage.onChanged.addListener((changes, area) => {
    if (area === 'local' && changes['torille-debug']) {
      const on = changes['torille-debug'].newValue === true;
      setDebugFlag(on);
      dbg('content', 'storage.onChanged torille-debug ->', on);
      const first = perInput.values().next().value;
      if (first) first.setDebug(on);
    }
    if (area === 'local' && changes['torille-searchboxes']) {
      checkSearchboxes = changes['torille-searchboxes'].newValue === true;
      dbg('content', 'storage.onChanged torille-searchboxes ->',
          checkSearchboxes);
      refreshSearchFields();
    }
    if (area === 'local'
        && Object.values(UI_SETTINGS).some((key) => key in changes)) {
      void loadUiSettings().then(() => {
        dbg('content', 'storage.onChanged ui settings', uiSettings);
        for (const [el, overlay] of perInput) {
          if (isCodeEditorField(el)) scanner.detach(el);
          else overlay.refresh();
        }
        scanner.scan(document);
      });
    }
  });
}

// Bootstrap. The blacklist load resolves and runs the first scan
// (only if the host isn't blacklisted). MutationObserver covers
// anything that mounts later.
void (async () => {
  await loadUiSettings();
  blacklist.load();
  void loadSearchSetting();
})();
