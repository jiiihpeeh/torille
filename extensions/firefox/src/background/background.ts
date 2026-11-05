// Background service worker for the torille Firefox extension.
//
// Hosts two context-menu checkboxes and routes them to either the
// content script (per-textbox toggle) or `browser.storage.local`
// (per-host blacklist).
//
//   "Älä tarkasta tätä tekstiä"  -> per-textbox toggle, round-trips
//                                   through the content script's
//                                   pendingInput.
//   "Älä tarkasta tällä sivulla"  -> toggles the current host in the
//                                   blacklist storage key. The
//                                   content script listens for
//                                   `storage.onChanged` and
//                                   re-attaches overlays as
//                                   needed.
//
// The settings page (`options.html`) also edits the same blacklist
// storage key.
//
// File layout:
//   §1  Debug + browser shim
//   §2  Wasm worker: hosts the spell-worker in the extension's own
//       origin so content scripts (which share the page's origin and
//       therefore cannot construct Workers against chrome-extension://
//       URLs in MV3) can ask for `check` / `suggest` over a message
//       channel.
//   §3  Message dispatch (content-script -> background)
//   §4  Menu click dispatch
//   §5  Storage helpers

(() => {
  'use strict';

// ───────────────────────────────────────────────────────────────────────
// §1  Debug + browser shim
// ───────────────────────────────────────────────────────────────────────

// Firefox's MV3 background page is an event page; it can be torn down
// and re-spawned, so the module-level flag only sticks while the page
// is alive. We seed it from the extension storage so the user can
// toggle it from the options page without reloading, and re-read on
// `storage.onChanged` so a same-page flip also takes effect.
let TORILLE_DEBUG = false;
const api: typeof browser = (typeof browser !== 'undefined') ? browser : chrome;

api.storage?.local?.get('torille-debug').then((cur) => {
  TORILLE_DEBUG = cur['torille-debug'] === true;
  dbg('background', 'init debug flag', TORILLE_DEBUG);
});
api.storage?.onChanged?.addListener((changes, area) => {
  if (area === 'local' && changes['torille-debug']) {
    TORILLE_DEBUG = changes['torille-debug'].newValue === true;
    dbg('background', 'storage.onChanged torille-debug ->', TORILLE_DEBUG);
  }
});

function dbg(label: string, ...args: unknown[]): void {
  if (!TORILLE_DEBUG) return;
  const head = label ? `[torille:${label}]` : '[torille]';
  console.log(head, ...args);
}

// ───────────────────────────────────────────────────────────────────────
// §2  Wasm worker
// ───────────────────────────────────────────────────────────────────────
//
// The wasm spell-worker lives here, in the background's own context.
// Content scripts can't do `new Worker(chrome-extension://...)` in
// Chrome MV3 because they share the page's origin; the background can,
// because the background runs in the extension's origin. One worker
// per browser session — the ~150 ms wasm boot is amortised across every
// tab that's open at the same time.
//
// Init and per-request timeouts match the reference's
// `INIT_TIMEOUT_MS` / `REQUEST_TIMEOUT_MS`. Without them, a hung worker
// (e.g. a transient wasm fetch failure) would make every content-script
// `check` / `suggest` hang forever. With them, the user gets a
// `{status: 'unavailable'}` and we tear down + retry on the next call.

const INIT_TIMEOUT_MS = 30000;
const REQUEST_TIMEOUT_MS = 8000;

type SpellCallPayload =
  | { type: 'check'; word: string }
  | { type: 'check-fast'; idx: number; word: string }
  | { type: 'suggest'; word: string; max: number }
  | { type: 'check-batch'; inputId: number; words: string[] }
  | { type: 'suggest-batch'; inputId: number; items: { word: string; max: number }[] }
  // Per-textbox state. Same wire names as the wasm worker; the
  // background is a transparent forwarder. `analyze-text` and
  // `suggest-with-state` carry the response back, the others are
  // fire-and-forget from the worker's perspective.
  | { type: 'create-state' }
  | { type: 'destroy-state'; idx: number }
  | { type: 'clear-state'; idx: number }
  | { type: 'update-state'; idx: number; text: string }
  | { type: 'analyze-text'; idx: number; text: string; max: number }
  | { type: 'suggest-with-state'; idx: number; word: string; max: number };

type SpellResponse =
  | { id: number; type: 'check'; ok: boolean }
  | { id: number; type: 'check-fast'; ok: boolean | null }
  | { id: number; type: 'suggest'; suggestions: string[] }
  | { id: number; type: 'check-batch'; inputId: number; ok: (boolean | null)[] }
  | { id: number; type: 'suggest-batch'; inputId: number; results: string[][] }
  | { id: number; type: 'create-state'; idx: number }
  | {
      id: number;
      type: 'analyze-text';
      hits: { word: string; start: number; end: number; suggestions: string[] }[];
      stateValid?: boolean;
    }
  | { id: number; type: 'suggest-with-state'; suggestions: string[] }
  // Fire-and-forget ops (destroy/clear/update-state, debug) resolve
  // with this in the direct host, where there is no message id to
  // mirror. Callers ignore the value.
  | { id: number; type: 'noop' };

type SpellHandlerEntry = {
  resolve: (value: SpellResponse) => void;
  reject: (err: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

// Shared word-result cache. Lives at the background so every content
// script (in every tab) shares the same hits. Bounded so a long-lived
// background doesn't grow without limit on pathological inputs.
//
// The values are the wasm `check()` result for the lowercased word.
// Anything else (e.g. compound analysis) is intentionally not cached
// here -- the wasm side is already the cheapest part, and the cache's
// job is to skip the IPC layer entirely on a repeat.
const wordCache: Map<string, boolean> = new Map();
const WORD_CACHE_LIMIT = 8192;

function wordCacheGet(word: string): boolean | undefined {
  const v = wordCache.get(word);
  if (v === undefined) return undefined;
  // Refresh recency: re-insert at the tail of the insertion-order map.
  wordCache.delete(word);
  wordCache.set(word, v);
  return v;
}

function wordCacheSet(word: string, ok: boolean): void {
  if (wordCache.has(word)) {
    wordCache.delete(word);
  } else if (wordCache.size >= WORD_CACHE_LIMIT) {
    // Drop the oldest entry (Map iteration order = insertion order).
    const oldest = wordCache.keys().next().value;
    if (oldest !== undefined) wordCache.delete(oldest);
  }
  wordCache.set(word, ok);
}

// Tiny LRU for suggestions. Suggestions are heavier than checks
// (edit-distance over the candidate set), so caching them is more
// valuable; the cap keeps it from growing on adversarial input.
const suggestCache: Map<string, string[]> = new Map();
const SUGGEST_CACHE_LIMIT = 1024;

function suggestCacheGet(word: string, max: number): string[] | undefined {
  const key = max + '\u0000' + word;
  const v = suggestCache.get(key);
  if (v === undefined) return undefined;
  suggestCache.delete(key);
  suggestCache.set(key, v);
  return v;
}

function suggestCacheSet(word: string, max: number, suggestions: string[]): void {
  const key = max + '\u0000' + word;
  if (suggestCache.has(key)) {
    suggestCache.delete(key);
  } else if (suggestCache.size >= SUGGEST_CACHE_LIMIT) {
    const oldest = suggestCache.keys().next().value;
    if (oldest !== undefined) suggestCache.delete(oldest);
  }
  suggestCache.set(key, suggestions);
}

let spellWorker: Worker | null = null;
let spellWorkerInit: Promise<void> | null = null;
let spellWorkerReady: Promise<void> | null = null;
let spellWorkerReadyResolve: (() => void) | null = null;
let nextSpellId = 1;
const spellHandlers = new Map<number, SpellHandlerEntry>();

// ─── direct host (Chromium MV3) ───────────────────────────────────────
//
// A Chrome extension service worker cannot create a nested Worker, so
// `new Worker(...)` throws there. In that case we import
// `spell-core.js` into the service worker itself: the wasm still runs
// off the page's main thread (and the service worker is per-extension,
// not per-tab), which is the only property the extra worker provided.
//
// The module is loaded lazily so the Firefox event page, where a real
// Worker exists, never pays for it. The type is structural so we don't
// need a cross-rootDir `import type`.
type DirectCore = {
  init(): Promise<void>;
  handle(msg: unknown): Promise<Record<string, unknown> | null>;
  setDebug?(on: boolean): void;
};

let directCore: DirectCore | null = null;
let directCoreInit: Promise<void> | null = null;

function teardownSpellWorker(reason: string): void {
  if (!spellWorker) return;
  for (const entry of spellHandlers.values()) {
    clearTimeout(entry.timer);
    entry.reject(new Error(reason));
  }
  spellHandlers.clear();
  spellWorker.terminate();
  spellWorker = null;
  spellWorkerInit = null;
  spellWorkerReady = null;
  spellWorkerReadyResolve = null;
}

function ensureSpellWorker(): Promise<void> {
  if (spellWorkerInit) return spellWorkerInit;
  const url = api.runtime.getURL('spell-worker/spell-worker.js');
  dbg('background', 'creating wasm worker', url);

  spellWorkerReady = new Promise<void>((resolve) => {
    spellWorkerReadyResolve = resolve;
  });

  const worker = new Worker(url, { type: 'module' });
  worker.addEventListener('message', (e: MessageEvent) => {
    const msg = e.data as
      | { id?: number; type?: string; ok?: boolean | boolean[];
          suggestions?: string[]; results?: string[][]; on?: boolean;
          idx?: number; hits?: unknown };
    dbg('worker', 'msg', msg);
    if (msg.type === 'ready') {
      spellWorkerReadyResolve?.();
      return;
    }
    if (msg.type === 'debug') {
      TORILLE_DEBUG = !!msg.on;
      return;
    }
    const id = msg.id;
    if (typeof id === 'number') {
      const handler = spellHandlers.get(id);
      if (handler) {
        spellHandlers.delete(id);
        clearTimeout(handler.timer);
        handler.resolve(msg as unknown as SpellResponse);
      }
    }
  });
  worker.addEventListener('error', (e: ErrorEvent) => {
    dbg('background', 'worker error', String(e.message || e));
    teardownSpellWorker('worker crashed: ' + (e.message || 'unknown'));
  });

  spellWorker = worker;

  spellWorkerInit = new Promise<void>((resolve, reject) => {
    const t = setTimeout(() => {
      dbg('background', 'worker init timeout', INIT_TIMEOUT_MS + 'ms');
      teardownSpellWorker('worker init timeout');
      reject(new Error('wasm worker init timed out'));
    }, INIT_TIMEOUT_MS);
    spellWorkerReady!.then(() => {
      clearTimeout(t);
      resolve();
    }).catch((e: unknown) => {
      clearTimeout(t);
      reject(e instanceof Error ? e : new Error(String(e)));
    });
  });

  return spellWorkerInit;
}

async function ensureSpellWorkerForRequest(): Promise<void> {
  // Chromium MV3: no nested workers from a service worker. Use the
  // in-service-worker host instead.
  if (typeof Worker === 'undefined') {
    await ensureDirectCore();
    return;
  }
  try {
    await ensureSpellWorker();
    await spellWorkerReady!;
  } catch (e) {
    // Worker creation can also fail on a hardened build (CSP on
    // extension pages, a missing packaged file). Fall back rather
    // than leaving every check unavailable.
    dbg('background', 'worker host unavailable; using direct core', String(e));
    teardownSpellWorker('worker host unavailable');
    await ensureDirectCore();
  }
}

function withTimeout<T>(p: Promise<T>, ms: number, what: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`${what} timed out after ${ms}ms`)), ms);
    p.then(
      (v) => { clearTimeout(timer); resolve(v); },
      (e) => { clearTimeout(timer); reject(e); },
    );
  });
}

async function ensureDirectCore(): Promise<DirectCore> {
  if (directCore !== null) return directCore;
  if (directCoreInit === null) {
    directCoreInit = (async (): Promise<void> => {
      const url = api.runtime.getURL('spell-worker/spell-core.js');
      dbg('background', 'importing direct spell core', url);
      const mod = await import(url) as {
        createSpellCore(): DirectCore;
      };
      const core = mod.createSpellCore();
      await withTimeout(core.init(), INIT_TIMEOUT_MS, 'direct core init');
      directCore = core;
      dbg('background', 'direct spell core ready');
    })().catch((e: unknown) => {
      directCoreInit = null; // allow the next request to retry
      directCore = null;
      throw e;
    });
  }
  await directCoreInit;
  return directCore!;
}

function spellCall(payload: SpellCallPayload): Promise<SpellResponse> {
  const id = nextSpellId++;

  if (directCore !== null) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        // A hung wasm call would otherwise wedge every later
        // request. Drop the host so the next call re-imports and
        // re-initialises it.
        directCore = null;
        directCoreInit = null;
        reject(new Error('direct spell request timed out'));
      }, REQUEST_TIMEOUT_MS);
      directCore!.handle(payload).then(
        (out) => {
          clearTimeout(timer);
          resolve(out
            ? { id, ...out } as SpellResponse
            : { id, type: 'noop' });
        },
        (e) => {
          clearTimeout(timer);
          reject(e instanceof Error ? e : new Error(String(e)));
        },
      );
    });
  }

  if (!spellWorker) {
    return Promise.reject(new Error('worker not initialised'));
  }
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      spellHandlers.delete(id);
      // A timed-out request also tears the worker down so the next
      // call gets a fresh one.
      teardownSpellWorker('request timeout');
      reject(new Error('worker request timed out'));
    }, REQUEST_TIMEOUT_MS);
    spellHandlers.set(id, { resolve, reject, timer });
    spellWorker!.postMessage({ id, ...payload });
  });
}

// Serve a `check-batch` against the shared `wordCache` first; the
// remaining words go to the wasm worker in a single `check-batch` call.
// The worker's response is zipped back into the caller's item order,
// the cache is populated with the new entries, and the ordered `ok[]`
// array is returned. Items the worker didn't see (all-cached hits) cost
// exactly one runtime.sendMessage, no wasm call, no IPC to the worker.
//
// `inputId` is the caller's per-overlay tag; we forward it to the
// worker (so a single wasm log line can be tied back to one input)
// and the response echoes it (so the caller can sanity-check
// routing).
async function spellBatchCheck(
  inputId: number, items: string[],
): Promise<(boolean | null)[]> {
  const out: (boolean | null)[] = new Array(items.length);
  const missIdx: number[] = [];
  const missWords: string[] = [];
  for (let i = 0; i < items.length; i++) {
    const cached = wordCacheGet(items[i]);
    if (cached !== undefined) {
      out[i] = cached;
    } else {
      missIdx.push(i);
      missWords.push(items[i]);
    }
  }
  if (missWords.length === 0) return out;
  try {
    await ensureSpellWorkerForRequest();
    const r = await spellCall({
      type: 'check-batch', inputId, words: missWords,
    });
    if (r.type !== 'check-batch') throw new Error('unexpected worker response');
    const ok = r.ok;
    if (ok.length !== missWords.length) {
      throw new Error('worker returned mismatched batch length');
    }
    for (let j = 0; j < missWords.length; j++) {
      const v = ok[j];
      out[missIdx[j]] = v;
      if (typeof v === 'boolean') wordCacheSet(missWords[j], v);
    }
  } catch (e) {
    dbg('background', 'spellBatchCheck failed; leaving entries unknown',
        String(e));
    // `null`, not `false`: a transport failure must not underline
    // every word in the textarea.
    for (const idx of missIdx) out[idx] = null;
  }
  return out;
}

// Same shape for `suggest-batch`. Suggestions are heavier than checks
// (the wasm side runs an edit-distance pass), so caching them pays off
// more. We dedupe on `(max, word)` because changing `max` invalidates
// the truncation.
async function spellBatchSuggest(
  inputId: number, items: { word: string; max: number }[],
): Promise<string[][]> {
  const out: (string[] | null)[] = new Array(items.length);
  const missIdx: number[] = [];
  const missItems: { word: string; max: number }[] = [];
  for (let i = 0; i < items.length; i++) {
    const it = items[i];
    const cached = suggestCacheGet(it.word, it.max);
    if (cached !== undefined) {
      out[i] = cached;
    } else {
      missIdx.push(i);
      missItems.push(it);
    }
  }
  if (missItems.length === 0) {
    // TS can't narrow the inner nullability of the array literal; assert.
    return out as string[][];
  }
  try {
    await ensureSpellWorkerForRequest();
    const r = await spellCall({
      type: 'suggest-batch', inputId, items: missItems,
    });
    if (r.type !== 'suggest-batch') throw new Error('unexpected worker response');
    const results = r.results;
    if (results.length !== missItems.length) {
      throw new Error('worker returned mismatched batch length');
    }
    for (let j = 0; j < missItems.length; j++) {
      out[missIdx[j]] = results[j];
      suggestCacheSet(missItems[j].word, missItems[j].max, results[j]);
    }
  } catch (e) {
    dbg('background', 'spellBatchSuggest failed; falling back to empty',
        String(e));
    for (const idx of missIdx) out[idx] = [];
  }
  return out as string[][];
}

// ───────────────────────────────────────────────────────────────────────
// §2  Context-menu definitions + setup
// ───────────────────────────────────────────────────────────────────────

const MENU_DISABLE_INPUT = 'torille-disable-input';
const MENU_DISABLE_SITE = 'torille-disable-site';

const MENU_ITEMS: browser.contextMenus._CreateCreateProperties[] = [
  {
    id: MENU_DISABLE_INPUT,
    title: 'Älä tarkasta tätä tekstiä',
    contexts: ['editable'],
  },
  {
    id: MENU_DISABLE_SITE,
    title: 'Älä tarkasta tällä sivulla',
    contexts: ['all'],
  },
];

function setupContextMenus(): void {
  dbg('background', 'onInstalled: creating context menu items');
  for (const item of MENU_ITEMS) {
    try {
      api.contextMenus.create({ type: 'checkbox', ...item });
      // duplicate on hot-reload is fine; contextMenus.create throws
      // if the id already exists.
    } catch (_) { /* already created */ }
  }
  // Boot the wasm worker as part of install too, so the first keystroke
  // in any tab doesn't pay the wasm boot latency.
  ensureSpellWorkerForRequest().catch((e: unknown) =>
    dbg('background', 'wasm host boot failed', String(e)));
}

// ───────────────────────────────────────────────────────────────────────
// §3  Message dispatch (content-script -> background)
// ───────────────────────────────────────────────────────────────────────

// The content script captures the `contextmenu` event itself (because
// `contextMenus.onClicked` doesn't tell us which DOM element was
// right-clicked), then pushes the current input's disabled state
// here so the menu's `checked` flag matches reality when the user
// opens it. The site-level message does the same for the
// "Älä tarkasta tällä sivulla" item when the user right-clicks
// somewhere that isn't a textbox.
api.runtime.onMessage.addListener((message: unknown) => {
  const m = message as
    | R_Check
    | R_CheckFast
    | R_Suggest
    | R_CheckBatch
    | R_SuggestBatch
    | R_CreateState
    | R_DestroyState
    | R_ClearState
    | R_UpdateState
    | R_AnalyzeText
    | R_SuggestWithState
    | R_ContextMenuFired
    | R_SiteContextMenuFired
    | R_Debug;

  // Wasm spell-check: the content script's worker client sends a
  // `{type: 'torille:check' | 'torille:suggest'}` message; we route
  // it to the wasm worker and return the answer. The batched
  // `{type: 'torille:check-batch' | 'torille:suggest-batch'}` route
  // is the same idea but for a whole tokenised line -- the legacy
  // render path uses it. The per-textbox state ops are the new
  // render path and the popup's suggest-with-state.
  if (m?.type === 'torille:check' || m?.type === 'torille:suggest') {
    return (async (): Promise<{ ok: boolean | null } | { suggestions: string[] }> => {
      try {
        await ensureSpellWorkerForRequest();
        if (m.type === 'torille:check') {
          const r = await spellCall({ type: 'check', word: m.word });
          return { ok: r.type === 'check' ? r.ok : null };
        }
        const r = await spellCall({ type: 'suggest', word: m.word, max: m.max });
        return { suggestions: r.type === 'suggest' ? (r.suggestions ?? []) : [] };
      } catch (e) {
        dbg('background', 'spell route error', String(e));
        return m.type === 'torille:check' ? { ok: null } : { suggestions: [] };
      }
    })();
  }

  if (m?.type === 'torille:check-fast') {
    return (async (): Promise<R_CheckFastResponse> => {
      try {
        await ensureSpellWorkerForRequest();
        const r = await spellCall({
          type: 'check-fast', idx: m.idx, word: m.word,
        });
        return { ok: r.type === 'check-fast' ? r.ok : null };
      } catch (e) {
        dbg('background', 'check-fast route error', String(e));
        return { ok: null };
      }
    })();
  }

  if (m?.type === 'torille:check-batch') {
    return (async (): Promise<R_CheckBatchResponse> => {
      const ok = await spellBatchCheck(m.inputId, m.items);
      return { inputId: m.inputId, gen: m.gen, ok };
    })();
  }

  if (m?.type === 'torille:suggest-batch') {
    return (async (): Promise<R_SuggestBatchResponse> => {
      const results = await spellBatchSuggest(m.inputId, m.items);
      return { inputId: m.inputId, gen: m.gen, results };
    })();
  }

  // ─── per-textbox state ────────────────────────────────────────
  // `create-state` returns the wasm-side slot index; every later
  // op in this overlay's lifetime needs it.
  if (m?.type === 'torille:create-state') {
    return (async (): Promise<R_CreateStateResponse> => {
      try {
        await ensureSpellWorkerForRequest();
        const r = await spellCall({ type: 'create-state' });
        if (r.type !== 'create-state') {
          throw new Error('worker returned unexpected response');
        }
        return { idx: r.idx };
      } catch (e) {
        dbg('background', 'create-state route error', String(e));
        throw new Error('create-state unavailable');
      }
    })();
  }

  // Fire-and-forget from the content-script's perspective -- the
  // wasm worker doesn't reply to destroy-state or clear-state.
  if (m?.type === 'torille:destroy-state') {
    (async (): Promise<void> => {
      try {
        await ensureSpellWorkerForRequest();
        spellCall({ type: 'destroy-state', idx: m.idx }).catch(() => {});
      } catch (e) {
        dbg('background', 'destroy-state route error', String(e));
      }
    })();
    return;
  }

  if (m?.type === 'torille:clear-state') {
    (async (): Promise<void> => {
      try {
        await ensureSpellWorkerForRequest();
        spellCall({ type: 'clear-state', idx: m.idx }).catch(() => {});
      } catch (e) {
        dbg('background', 'clear-state route error', String(e));
      }
    })();
    return;
  }

  if (m?.type === 'torille:update-state') {
    (async (): Promise<void> => {
      try {
        await ensureSpellWorkerForRequest();
        spellCall({ type: 'update-state', idx: m.idx, text: m.text }).catch(() => {});
      } catch (e) {
        dbg('background', 'update-state route error', String(e));
      }
    })();
    return;
  }

  // `analyze-text` is the new render path: one wasm call per
  // render, regardless of token count. Returns hits tagged with
  // byte ranges so the content script can highlight without
  // re-tokenising.
  if (m?.type === 'torille:analyze-text') {
    return (async (): Promise<R_AnalyzeTextResponse> => {
      try {
        await ensureSpellWorkerForRequest();
        const r = await spellCall({
          type: 'analyze-text', idx: m.idx, text: m.text, max: m.max,
        });
        if (r.type !== 'analyze-text') {
          throw new Error('worker returned unexpected response');
        }
        return {
          inputId: m.inputId,
          gen: m.gen,
          hits: r.hits,
          stateValid: r.stateValid,
        };
      } catch (e) {
        dbg('background', 'analyze-text route error', String(e));
        throw new Error('analyze-text unavailable');
      }
    })();
  }

  // `suggest-with-state` is the popup's per-click suggest path so
  // proper nouns the user typed surface as candidates.
  if (m?.type === 'torille:suggest-with-state') {
    return (async (): Promise<R_SuggestWithStateResponse> => {
      try {
        await ensureSpellWorkerForRequest();
        const r = await spellCall({
          type: 'suggest-with-state', idx: m.idx, word: m.word, max: m.max,
        });
        if (r.type !== 'suggest-with-state') {
          throw new Error('worker returned unexpected response');
        }
        return { suggestions: r.suggestions };
      } catch (e) {
        dbg('background', 'suggest-with-state route error', String(e));
        return { suggestions: [] };
      }
    })();
  }

  // Debug flag from the content script. Route it to whichever host
  // is live; the worker has its own `debug` frame, the direct core
  // exposes `setDebug`.
  if (m?.type === 'torille:debug') {
    if (directCore) directCore.setDebug?.(!!m.on);
    if (spellWorker) spellWorker.postMessage({ type: 'debug', on: !!m.on });
    return;
  }

  if (m?.type === 'torille:contextmenu-fired') {
    dbg('background', 'contextmenu-fired', { disabled: !!m.disabled });
    try {
      api.contextMenus.update(MENU_DISABLE_INPUT, { checked: !!m.disabled });
    } catch (_) { /* menu torn down */ }
    return;
  }
  if (m?.type === 'torille:site-contextmenu-fired') {
    dbg('background', 'site-contextmenu-fired', { disabled: !!m.disabled });
    try {
      api.contextMenus.update(MENU_DISABLE_SITE, { checked: !!m.disabled });
    } catch (_) { /* menu torn down */ }
    return;
  }
});

// ───────────────────────────────────────────────────────────────────────
// §4  Menu click dispatch
// ───────────────────────────────────────────────────────────────────────

api.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === MENU_DISABLE_INPUT) {
    await handleDisableInputClick(tab);
    return;
  }
  if (info.menuItemId === MENU_DISABLE_SITE) {
    await handleDisableSiteClick(tab);
    return;
  }
});

async function handleDisableInputClick(tab: browser.tabs.Tab | undefined): Promise<void> {
  if (!tab || tab.id == null) return;
  let result: { disabled?: boolean } | undefined;
  try {
    result = await api.tabs.sendMessage(tab.id, { type: 'torille:toggle-pending-input' });
  } catch (_) { return; }
  dbg('background', 'toggle input', { disabled: result?.disabled });
  if (result && typeof result.disabled === 'boolean') {
    try {
      await api.contextMenus.update(MENU_DISABLE_INPUT, { checked: result.disabled });
    } catch (_) { /* ignore */ }
  }
}

async function handleDisableSiteClick(tab: browser.tabs.Tab | undefined): Promise<void> {
  const hostname = await hostnameOfTab(tab);
  if (!hostname) return;
  const list = await readBlacklist();
  const idx = list.indexOf(hostname);
  const nowBlacklisted = idx < 0;
  if (nowBlacklisted) list.push(hostname);
  else list.splice(idx, 1);
  await writeBlacklist(list);
  dbg('background', 'toggle site',
      { hostname, nowBlacklisted, size: list.length });
  try {
    await api.contextMenus.update(MENU_DISABLE_SITE, { checked: nowBlacklisted });
  } catch (_) { /* ignore */ }
  // The content script listens to storage.onChanged and re-attaches.
}

// ───────────────────────────────────────────────────────────────────────
// §5  Storage helpers
// ───────────────────────────────────────────────────────────────────────

async function readBlacklist(): Promise<string[]> {
  const cur = await api.storage.local.get('blacklist');
  return Array.isArray(cur.blacklist) ? cur.blacklist : [];
}

async function writeBlacklist(list: string[]): Promise<void> {
  await api.storage.local.set({ blacklist: list });
}

async function hostnameOfTab(tab: browser.tabs.Tab | undefined): Promise<string | null> {
  if (!tab || !tab.url) return null;
  try { return new URL(tab.url).hostname; }
  catch (_) { return null; }
}

api.runtime.onInstalled.addListener(setupContextMenus);
})();