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
import {
  editorText, hideCheckProgress, showCheckProgress, showCombinedPanel,
  showGrammarPanel, showStylePanel,
} from './grammar-panel';
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

// One token per context-menu check. A stale check's `finally` must not hide
// the progress toast of a newer check for the same input.
const checkTokens = new Map<number, number>();
function nextCheckToken(inputId: number): number {
  const next = (checkTokens.get(inputId) ?? 0) + 1;
  checkTokens.set(inputId, next);
  return next;
}

// Rule-level dismissals. The stable rule id is the wasm's `rule_id`, not a
// span: ignoring `subject_predicate_agreement` keeps it hidden after the
// sentence is edited. The set is persisted in extension storage so the
// choices survive a reload; the options page can clear it.
const IGNORED_RULES_KEY = 'torille-ignored-rules';
const ignoredRules = new Set<string>();
const ignoredRulesReady = (async (): Promise<void> => {
  try {
    const raw = await api.storage.local.get(IGNORED_RULES_KEY);
    const stored = raw[IGNORED_RULES_KEY];
    if (Array.isArray(stored)) {
      for (const rule of stored) {
        if (typeof rule === 'string') ignoredRules.add(rule);
      }
    }
  } catch (_) { /* storage unavailable: per-page ignores only */ }
})();

function ruleOf(diagnostic: R_SyntaxDiagnostic): string {
  return diagnostic.rule_id || diagnostic.kind;
}

function filterIgnored(
  diagnostics: R_SyntaxDiagnostic[],
): R_SyntaxDiagnostic[] {
  return diagnostics.filter((diagnostic) => !ignoredRules.has(ruleOf(diagnostic)));
}

function ignoreRule(ruleId: string): void {
  ignoredRules.add(ruleId);
  api.storage.local.set({
    [IGNORED_RULES_KEY]: [...ignoredRules],
  }).catch(() => { /* storage unavailable */ });
}

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
  async checkGrammar(input: HTMLElement): Promise<void> {
    await ignoredRulesReady;
    const overlay = perInput.get(input);
    if (!overlay) return;
    const text = editorText(input);
    const inputId = overlay.workerClient.inputId;
    const token = nextCheckToken(inputId);
    const apply = (correction: R_SyntaxCorrection): boolean => {
      if (editorText(input) !== text) return false;
      return overlay.applyTextEdits(correction.edits);
    };
    showCheckProgress(inputId, 'Tarkistetaan kielioppia…');
    overlay.workerClient.checkSyntaxAsync(text).then((diagnostics) => {
      if (diagnostics === null) return;
      // The field changed while the check ran; the offsets are stale.
      if (editorText(input) !== text) return;
      const render = (list: R_SyntaxDiagnostic[]): void => {
        overlay.setGrammarDiagnostics(list);
        showGrammarPanel(input, list, apply, (ruleId) => {
          ignoreRule(ruleId);
          render(list.filter((d) => ruleOf(d) !== ruleId));
        });
      };
      render(filterIgnored(diagnostics));
    }).catch((e: unknown) => {
      dbg('content', 'checkGrammar failed', String(e));
    }).finally(() => {
      if (checkTokens.get(inputId) === token) hideCheckProgress(inputId);
    });
  },
  async checkStyle(input: HTMLElement): Promise<void> {
    await ignoredRulesReady;
    const overlay = perInput.get(input);
    if (!overlay) return;
    const text = editorText(input);
    const inputId = overlay.workerClient.inputId;
    const token = nextCheckToken(inputId);
    const apply = (correction: R_SyntaxCorrection): boolean => {
      if (editorText(input) !== text) return false;
      return overlay.applyTextEdits(correction.edits);
    };
    showCheckProgress(inputId, 'Tarkistetaan tyyliä…');
    overlay.workerClient.checkStyleAsync(text).then((diagnostics) => {
      if (diagnostics === null) return;
      // The field changed while the check ran; the offsets are stale.
      if (editorText(input) !== text) return;
      // Code blocks, URLs and BBCode are not style prose. Drop any finding
      // that intersects an ignored range, exactly as the spell overlay does.
      const ignored = ignoredRangesFor(text);
      const visible = filterIgnored(diagnostics).filter(
        (diagnostic) => !isIgnored(
          { start: diagnostic.start, end: diagnostic.end }, ignored),
      );
      const render = (list: R_SyntaxDiagnostic[]): void => {
        overlay.setStyleDiagnostics(list);
        showStylePanel(input, list, apply, (ruleId) => {
          ignoreRule(ruleId);
          render(list.filter((d) => ruleOf(d) !== ruleId));
        });
      };
      render(visible);
    }).catch((e: unknown) => {
      dbg('content', 'checkStyle failed', String(e));
    }).finally(() => {
      if (checkTokens.get(inputId) === token) hideCheckProgress(inputId);
    });
  },
  async checkAll(input: HTMLElement): Promise<void> {
    await ignoredRulesReady;
    const overlay = perInput.get(input);
    if (!overlay) return;
    const text = editorText(input);
    const inputId = overlay.workerClient.inputId;
    const token = nextCheckToken(inputId);
    const apply = (correction: R_SyntaxCorrection): boolean => {
      if (editorText(input) !== text) return false;
      return overlay.applyTextEdits(correction.edits);
    };
    showCheckProgress(inputId, 'Tarkistetaan kielioppia ja tyyliä…');
    overlay.workerClient.checkGrammarStyleAsync(text).then((result) => {
      if (result === null) return;
      // The field changed while the check ran; the offsets are stale.
      if (editorText(input) !== text) return;
      const ignored = ignoredRangesFor(text);
      const syntax = filterIgnored(result.diagnostics);
      const style = filterIgnored(result.style).filter(
        (diagnostic) => !isIgnored(
          { start: diagnostic.start, end: diagnostic.end }, ignored),
      );
      const render = (
        nextSyntax: R_SyntaxDiagnostic[],
        nextStyle: R_SyntaxDiagnostic[],
      ): void => {
        overlay.setGrammarDiagnostics(nextSyntax);
        overlay.setStyleDiagnostics(nextStyle);
        showCombinedPanel(input, nextSyntax, nextStyle, apply, (ruleId) => {
          ignoreRule(ruleId);
          render(
            nextSyntax.filter((d) => ruleOf(d) !== ruleId),
            nextStyle.filter((d) => ruleOf(d) !== ruleId),
          );
        });
      };
      render(syntax, style);
    }).catch((e: unknown) => {
      dbg('content', 'checkAll failed', String(e));
    }).finally(() => {
      if (checkTokens.get(inputId) === token) hideCheckProgress(inputId);
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
    if (area === 'local' && changes[IGNORED_RULES_KEY]) {
      const value = changes[IGNORED_RULES_KEY].newValue;
      ignoredRules.clear();
      if (Array.isArray(value)) {
        for (const rule of value) {
          if (typeof rule === 'string') ignoredRules.add(rule);
        }
      }
      dbg('content', 'storage.onChanged ignored rules',
          [...ignoredRules]);
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
