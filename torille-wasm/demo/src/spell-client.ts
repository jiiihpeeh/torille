// Main-thread client for the spell-check worker.
//
// The wasm itself lives in `./spell-worker.ts`. This module exposes
// `checkAsync(word)` and `suggestAsync(word, max)` which post a
// request to the worker and resolve when the matching response
// arrives. Each request carries a unique id so multiple in-flight
// calls do not collide.
//
// `ready()` returns a promise that resolves once the worker has the
// wasm instantiated, so callers can show a "loading" splash during
// the wasm download without flashing the editor before it is usable.
//
// On top of the per-word ops, the per-textbox state lifecycle and
// `analyze_text` collapse the render path from N postMessage
// crossings + N wasm-bindgen marshals to a single wasm call.

import type { DiagnosticSpan } from '../../protocol.js';

let worker: Worker | null = null;
let nextId = 1;
let readyPromise: Promise<void> | null = null;
const pending = new Map<number, (msg: Response) => void>();
// Reserved for the (currently dead) per-render async-suggestion
// channel: when an `analyzeMisspellsAsync` call supplies an
// `onSuggestionReady` callback, the matching id is registered
// here so the worker's `suggestion-ready` envelopes can route
// to it. The worker doesn't emit that envelope today (its
// `handleAnalyzeMisspells` resolves the first pass
// synchronously with the full misspell list and discards the
// per-word suggestions), so this Map stays empty. Keeping the
// declaration around so the dead-code sites compile and the
// `getCacheSizes` snapshot has a real value to report.
const liveSuggestionListeners = new Map<
  number, { listener: (word: string, s: string[], m: number, d: number) => void;
            max: number, maxDistance: number }
>();

// ── suggestion-pool ────────────────────────────────────────────────────
//
// After the primary worker's first-pass misspell ranges come
// back, the demo fires one parallel wasm call per misspell
// word: `bg.check(word)` to take inflected forms OFF the
// misspell list, plus `bg.suggest_for_misspell(word)` for the
// popup. `check` is now the engine-free gate pipeline
// (`check_normal_fast`: lemma / form-set / clitic / stem-map
// probes, single-digit milliseconds -- it no longer runs the
// head/tail/prefix engine walk). `suggest_for_misspell` still
// ranks candidates and can take appreciably longer, so the pool
// still exists to keep one slow suggestion from gating the rest.
//
// The pool has a configurable size; each member is its own
// worker with its own wasm instance plus its own LRU memo of
// completed (word, max, maxDistance) -> suggestions tuples.
// The main thread also keeps a side cache so a word that
// finished on worker A is instant on workers B/C/D.
//
// Bounded with `SUGGESTION_POOL_MEMO_LIMIT` per slot. Without
// a bound, the per-slot memo grows linearly with the number
// of unique words the demo has asked about, with no
// eviction. With 3 slots and the central cross-pool map,
// that's a 4× duplication: 1000 unique words × ~300 B/entry
// × 4 = 1.2 MB. The bound is much higher than the actual
// working set for a 1 KB textarea (~50 words) so a hard
// cap doesn't hurt the common case -- the bound exists
// purely to keep the long-running demo from OOMing after
// many chapter flips in the Gutenberg reader.
const SUGGESTION_POOL_SIZE = 3;
const SUGGESTION_POOL_MEMO_LIMIT = 4096;
const SUGGESTION_POOL_CHECK_KNOWN_LIMIT = 16384;
interface SuggestionSlot {
  worker: Worker;
  /** Number of inflight wasm calls. Picked-against for routing. */
  inflight: number;
  ready: Promise<void>;
  /** Per-worker LRU cache, key `${max}|${maxDistance}|${word.toLowerCase()}`. */
  memo: Map<string, string[]>;
  /** Pending message handlers keyed by id (per-worker). */
  inflightById: Map<number, (msg: any) => void>;
  /** Per-worker `check-known` results cached for the lifetime of this slot. */
  checkKnown: Set<string>;
}
const suggestionSlots: SuggestionSlot[] = [];

function startSuggestionPool(): void {
  if (suggestionSlots.length > 0) return;
  for (let i = 0; i < SUGGESTION_POOL_SIZE; i++) {
    // Build the slot in two halves so the `ready` promise
    // and the message-listener closure can reference `slot`
    // without tripping the temporal dead zone.
    const slot = {} as SuggestionSlot;
    const worker = new Worker(
      new URL('./suggestion-worker.js', import.meta.url),
      { type: 'module' },
    );
    slot.worker = worker;
    slot.inflight = 0;
    slot.memo = new Map();
    slot.inflightById = new Map();
    slot.checkKnown = new Set();
    slot.ready = new Promise<void>((resolve, reject) => {
      worker.addEventListener('message', (e: MessageEvent<any>) => {
        const m = e.data;
        if (m?.type === 'ready') resolve();
        if (m?.type === 'init_failed') reject(new Error(m.error));
      });
      worker.addEventListener('error', (e) => {
        reject(new Error(
          `suggestion-pool worker #${i} crashed: ${e.message || 'unknown'}`,
        ));
      });
    });
    worker.addEventListener('message', (e: MessageEvent<any>) => {
      const m = e.data;
      if (m?.type === 'suggest-for-misspell' || m?.type === 'check-known') {
        const cb = slot.inflightById.get(m.id);
        if (cb) {
          slot.inflightById.delete(m.id);
          slot.inflight = Math.max(0, slot.inflight - 1);
          cb(m);
        }
      }
    });
    suggestionSlots.push(slot);
  }
}

function pickSuggestionSlot(): SuggestionSlot {
  // Least-inflight first; ties broken by registration order so
  // we don't hammer a single worker by accident.
  let best = suggestionSlots[0];
  for (let i = 1; i < suggestionSlots.length; i++) {
    if (suggestionSlots[i].inflight < best.inflight) {
      best = suggestionSlots[i];
    }
  }
  return best;
}

const suggestionMemosAcrossPool = new Map<
  string, { word: string; max: number; maxDistance: number; suggestions: string[] }
>();
// LRU bound on the cross-pool memo. Same Map-insertion-order
// eviction as the per-slot memo.
const SUGGESTION_POOL_CROSS_MEMO_LIMIT = 8192;
function suggestionPoolMemoSet(
  key: string,
  value: { word: string; max: number; maxDistance: number; suggestions: string[] },
): void {
  if (suggestionMemosAcrossPool.has(key)) {
    suggestionMemosAcrossPool.delete(key);
  } else if (suggestionMemosAcrossPool.size >= SUGGESTION_POOL_CROSS_MEMO_LIMIT) {
    const oldest = suggestionMemosAcrossPool.keys().next().value;
    if (oldest !== undefined) suggestionMemosAcrossPool.delete(oldest);
  }
  suggestionMemosAcrossPool.set(key, value);
}
function suggestionPoolMemoKey(
  word: string, max: number, maxDistance: number,
): string {
  return `${word.toLowerCase()}|${max}|${maxDistance}`;
}

// One-shot helper used by `analyzeMisspellsAsync`'s callback to
// kick off a suggestion call without waiting (fire-and-forget).
export function dispatchSuggestForMisspell(
  word: string, max: number, maxDistance: number,
): Promise<string[]> {
  const pk = suggestionPoolMemoKey(word, max, maxDistance);
  // Already cached in the cross-pool central memo AND the
  // editor-visible side cache -- resolve immediately so the
  // popup's first paint on click lands with the background
  // result without firing a slow `suggestWithState` round-trip.
  const pre = suggestionMemosAcrossPool.get(pk);
  if (pre !== undefined) {
    return Promise.resolve(pre.suggestions);
  }
  // Editor-visible lemma side cache (separate Map, different
  // key prefix to keep the encoding obvious). Same keying is
  // used by `getCachedSuggestions` -- mirror the result back so
  // the popup can read it the moment the user clicks.
  const editorCached = suggestionSideCache.get(
    lemmaSideCacheKey(word, max, maxDistance),
  );
  if (editorCached !== undefined) {
    return Promise.resolve(editorCached);
  }
  // Scan all slots' per-worker memos so a word that finished on
  // any worker is instant on every other.
  for (const slot of suggestionSlots) {
    const k = `${max}\u0000${maxDistance}\u0000${word.toLowerCase()}`;
    const v = slot.memo.get(k);
    if (v !== undefined) {
      suggestionPoolMemoSet(pk, {
        word, max, maxDistance, suggestions: v,
      });
      setWithLRU(
        suggestionSideCache,
        lemmaSideCacheKey(word, max, maxDistance),
        v,
        SUGGEST_SIDE_CACHE_LIMIT,
      );
      return Promise.resolve(v);
    }
  }
  const slot = pickSuggestionSlot();
  const id = nextId++;
  return new Promise<string[]>((resolve) => {
    slot.inflight++;
    slot.inflightById.set(id, (m) => {
      const suggestions: string[] = m.suggestions ?? [];
      suggestionPoolMemoSet(pk, { word, max, maxDistance, suggestions });
      // Mirror into the per-slot memo with an LRU bound.
      // Without the bound, the memo grows linearly with
      // the number of unique words the demo has asked
      // about, with no eviction -- the source of the
      // "memory grows dramatically on chapter flip"
      // bug. Insertion order IS the LRU order for
      // `Map`, so on overflow we drop the oldest
      // entry (the first one in iteration order).
      const memoKey = `${max}\u0000${maxDistance}\u0000${word.toLowerCase()}`;
      if (slot.memo.has(memoKey)) {
        slot.memo.delete(memoKey);
      } else if (slot.memo.size >= SUGGESTION_POOL_MEMO_LIMIT) {
        const oldest = slot.memo.keys().next().value;
        if (oldest !== undefined) slot.memo.delete(oldest);
      }
      slot.memo.set(memoKey, suggestions);
      // Mirror into the editor-visible side cache so a popup
      // click on the word reads instantly from
      // `getCachedSuggestions`. LRU-bounded so the
      // main-thread cache doesn't grow without limit
      // across many unique words.
      setWithLRU(
        suggestionSideCache,
        lemmaSideCacheKey(word, max, maxDistance),
        suggestions,
        SUGGEST_SIDE_CACHE_LIMIT,
      );
      resolve(suggestions);
    });
    slot.worker.postMessage({
      type: 'suggest-for-misspell', id, word, max, maxDistance,
    });
  });
}

// Same shape but `bg.check`-based: asks the inflection-aware
// spell-check whether the word is a known Finnish form. Returns
// true once we have a positive answer from any worker; in-flight
// de-dup so two parallel asks on the same word coalesce.
const checkKnownInflight = new Map<string, Promise<boolean>>();
export function dispatchCheckKnown(word: string): Promise<boolean> {
  const lower = word.toLowerCase();
  // A previous answer for this word:
  for (const slot of suggestionSlots) {
    if (slot.checkKnown.has(lower)) return Promise.resolve(true);
  }
  if (checkKnownInflight.has(lower)) {
    return checkKnownInflight.get(lower)!;
  }
  const slot = pickSuggestionSlot();
  const id = nextId++;
  const p = new Promise<boolean>((resolve) => {
    slot.inflight++;
    slot.inflightById.set(id, (m) => {
      const known = !!m.known;
      if (known) {
        // Per-slot `checkKnown` is bounded so the slot's
        // `Set` doesn't grow without limit across many
        // chapter flips. On overflow we clear and
        // re-insert the new entry -- the cost is one
        // re-cache per slot, but check-known is cheap
        // (O(1) hashmap probe against a string set).
        if (slot.checkKnown.size >= SUGGESTION_POOL_CHECK_KNOWN_LIMIT) {
          slot.checkKnown.clear();
        }
        slot.checkKnown.add(lower);
        // Mirror across other slots so a re-render is
        // instant. Each slot has its own bound.
        for (const s of suggestionSlots) {
          if (s === slot) continue;
          if (s.checkKnown.size >= SUGGESTION_POOL_CHECK_KNOWN_LIMIT) {
            s.checkKnown.clear();
          }
          s.checkKnown.add(lower);
        }
      }
      resolve(known);
    });
    slot.worker.postMessage({ type: 'check-known', id, word });
  });
  checkKnownInflight.set(lower, p);
  p.finally(() => checkKnownInflight.delete(lower));
  return p;
}

export function suggestionPoolReady(): Promise<void[]> {
  return Promise.all(suggestionSlots.map(s => s.ready));
}

// Debug: ask the main spell-worker for its wasm memory
// size and pending-callback count. Used by the memory
// probe to distinguish "JS heap grew" from "wasm linear
// memory grew" when chasing leaks.
export function getWorkerMemoryInfo(): Promise<{ wasmBytes: number; pendingCount: number }> {
  if (!worker) return Promise.resolve({ wasmBytes: 0, pendingCount: 0 });
  return new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'memory-info') {
        resolve({ wasmBytes: msg.wasmBytes, pendingCount: msg.pendingCount });
      } else {
        resolve({ wasmBytes: 0, pendingCount: 0 });
      }
    });
    worker!.postMessage({ type: 'memory-info', id });
  });
}

// Debug: snapshot every bounded cache on the main thread.
// Used by the memory probe to attribute growth to the
// right structure (per-slot memo, central memo, side cache).
// Main-thread only: workers' JS heaps live in separate
// frames and aren't visible from `performance.memory`.
export interface CacheSizes {
  liveSuggestionListeners: number;
  pending: number;
  checkKnownInflight: number;
  suggestionMemosAcrossPool: number;
  suggestionSideCache: number;
  suggestionSideCacheForState: number;
  perSlotMemo: number[];
  perSlotCheckKnown: number[];
}
export function getCacheSizes(): CacheSizes {
  return {
    liveSuggestionListeners: liveSuggestionListeners.size,
    pending: pending.size,
    checkKnownInflight: checkKnownInflight.size,
    suggestionMemosAcrossPool: suggestionMemosAcrossPool.size,
    suggestionSideCache: suggestionSideCache.size,
    suggestionSideCacheForState: suggestionSideCacheForState.size,
    perSlotMemo: suggestionSlots.map((s) => s.memo.size),
    perSlotCheckKnown: suggestionSlots.map((s) => s.checkKnown.size),
  };
}

// Side cache: per-word suggestions the worker finishes
// computing in the background. The popup reads from this so a
// fresh render that lands on a word whose background pass is
// already done shows the candidates without firing another
// wasm call.
//
// Two keying schemes coexist:
//   * `lemma-only` path -- keyed by word + max + maxDistance.
//     Used by the background pass for fast-but-imperfect
//     candidates. Returned by `getCachedSuggestions`.
//   * `state-aware` path -- keyed by idx + word + max +
//     maxDistance. Used by the popup when it fires
//     `suggestWithState`. Returned by
//     `getCachedSuggestionsForState`. The idx tag matters:
//     the same word in two different text inputs (each with
//     its own `proper_nouns`) can have different best
//     candidates.
const suggestionSideCache: Map<string, string[]> = new Map();
const suggestionSideCacheForState: Map<string, string[]> = new Map();
// LRU bounds. Without these, every unique (word, max,
// maxDistance) tuple the demo asks about adds an entry
// that never gets evicted -- the source of the per-chapter
// JS heap growth in the Gutenberg reader (50 chapter flips
// × ~100 unique words/chapter = 5000 entries, each ~270 B,
// totals ~1.3 MB; with `suggestionSideCacheForState` 4x
// the size, this is the ~5 MB-plus leak the memory probe
// was tracking). The bound is high enough that the
// working set for any single chapter is well within it.
const SUGGEST_SIDE_CACHE_LIMIT = 2048;
const SUGGEST_WITH_STATE_SIDE_CACHE_LIMIT = 4096;
function setWithLRU<K, V>(map: Map<K, V>, key: K, value: V, limit: number): void {
  if (map.has(key)) {
    map.delete(key);
  } else if (map.size >= limit) {
    const oldest = map.keys().next().value;
    if (oldest !== undefined) map.delete(oldest);
  }
  map.set(key, value);
}

function lemmaSideCacheKey(word: string, max: number, maxDistance: number): string {
  return 'l' + '|' + max + '|' + maxDistance + '|' + word.toLowerCase();
}
function stateSideCacheKey(
  idx: number, word: string, max: number, maxDistance: number,
): string {
  return 's' + '|' + idx + '|' + max + '|' + maxDistance + '|' + word.toLowerCase();
}

export function getCachedSuggestions(
  word: string, max: number, maxDistance: number,
): string[] | undefined {
  return suggestionSideCache.get(lemmaSideCacheKey(word, max, maxDistance));
}
export function getCachedSuggestionsForState(
  idx: number, word: string, max: number, maxDistance: number,
): string[] | undefined {
  return suggestionSideCacheForState.get(stateSideCacheKey(idx, word, max, maxDistance));
}
export function putCachedSuggestionsForState(
  idx: number, word: string, max: number, maxDistance: number,
  suggestions: string[],
): void {
  const k = stateSideCacheKey(idx, word, max, maxDistance);
  setWithLRU(suggestionSideCacheForState, k, suggestions,
             SUGGEST_WITH_STATE_SIDE_CACHE_LIMIT);
}

type Response =
  | { type: 'check'; id: number; ok: boolean }
  | { type: 'suggest'; id: number; suggestions: string[] }
  | { type: 'ready' }
  | { type: 'init_failed'; error: string }
  | { type: 'create-state'; id: number; idx: number }
  | {
      type: 'analyze-text';
      id: number;
      hits: { word: string; start: number; end: number; suggestions: string[] }[];
    }
  | { type: 'suggest-with-state'; id: number; suggestions: string[] }
  // Two-pass render envelopes. `misspells` resolves the
  // first-pass promise synchronously; `suggestion-ready`
  // updates a side-cache without resolving any promise.
  | {
      type: 'misspells';
      id: number;
      hits: { word: string; start: number; end: number }[];
    }
  | {
      type: 'suggestion-ready';
      id: number;
      word: string;
      suggestions: string[];
    }
  | { type: 'suggest-for-misspell'; id: number; suggestions: string[] }
  | {
      type: 'check-syntax';
      id: number;
      diagnostics: DiagnosticSpan[];
    }
  | {
      type: 'check-style';
      id: number;
      diagnostics: DiagnosticSpan[];
    }
  | { type: 'memory-info'; id: number; wasmBytes: number; pendingCount: number }
  | { type: 'error'; id: number; for: string; word?: string; error: string };

function startWorker(): Promise<void> {
  if (readyPromise) return readyPromise;
  // Spawn the parallel suggestion pool alongside the primary
  // worker. The pool's workers download the wasm in parallel
  // to the primary; by the time the demo's first `analyze-
  // misspells` lands, they're all ready to take incoming
  // suggests and check-known asks without first-call latency.
  startSuggestionPool();
  readyPromise = new Promise<void>((resolve, reject) => {
    worker = new Worker(new URL('./spell-worker.js', import.meta.url), {
      type: 'module',
    });
    worker.addEventListener('message', (e: MessageEvent<Response>) => {
      const msg = e.data;
      if (msg.type === 'ready') {
        resolve();
        return;
      }
      if (msg.type === 'init_failed') {
        reject(new Error(msg.error));
        return;
      }
      // Side-cache channel for the two-pass render: `misspells`
      // resolves the originating promise; `suggestion-ready`
      // fires the registered live listener (which also updates
      // the side cache).
      if (msg.type === 'suggestion-ready') {
        const live = liveSuggestionListeners.get(msg.id);
        if (live !== undefined) {
          // Background-pass results land in the lemma-only
          // side cache. The popup reads this before doing
          // anything else. LRU-bounded so the cache
          // doesn't grow without limit across many
          // unique words.
          setWithLRU(
            suggestionSideCache,
            lemmaSideCacheKey(msg.word, live.max, live.maxDistance),
            msg.suggestions,
            SUGGEST_SIDE_CACHE_LIMIT,
          );
          live.listener(msg.word, msg.suggestions,
                        live.max, live.maxDistance);
        }
        return;
      }
      if (msg.type === 'misspells') {
        const handler = pending.get(msg.id);
        if (handler) {
          pending.delete(msg.id);
          handler(msg);
        }
        // The live listener (if any) stays registered: the
        // worker still has `suggestion-ready` envelopes in
        // flight for this id, and the listener will fire for
        // each one as it lands.
        return;
      }
      const handler = pending.get(msg.id);
      if (!handler) return;
      pending.delete(msg.id);
      handler(msg);
    });
    worker.addEventListener('error', (e) => {
      // Module-load errors (e.g. a failed wasm ESM import) fire
      // here with `e.message` typically the literal string
      // "undefined" and the real error in `e.error`. Surface the
      // whole event so devtools / puppeteer can see what actually
      // went wrong, and make the reject message useful for
      // callers that don't have access to the console.
      console.error('[torille-demo] worker error event', e);
      reject(new Error(`spell worker crashed: ${e.message || 'unknown'}`));
    });
  });
  return readyPromise;
}

export function ready(): Promise<void> {
  return startWorker();
}

// Mount the ready promise on `window.__torille.ready` so the
// Puppeteer probe can `await window.__torille.ready` instead
// of guessing a fixed boot-time budget. `setReadyPromise` is
// idempotent -- repeated calls overwrite the previous promise
// with the latest one.
import { setReadyPromise } from './debug';
let readyMounted = false;
export function mountReadySignal(): void {
  if (readyMounted) return;
  readyMounted = true;
  setReadyPromise(startWorker());
}

export function checkAsync(word: string): Promise<boolean> {
  return new Promise<boolean>((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'check') resolve(msg.ok);
      else resolve(false);
    });
    worker!.postMessage({ type: 'check', id, word });
  });
}

export function suggestAsync(word: string, max: number): Promise<string[]> {
  return new Promise<string[]>((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'suggest') resolve(msg.suggestions);
      else resolve([]);
    });
    worker!.postMessage({ type: 'suggest', id, word, max });
  });
}

// ─── per-textbox state ────────────────────────────────────────────────
//
// The state lifecycle mirrors the wasm-side pool: `createState()`
// returns a u32 index the caller remembers; every `updateState` /
// `analyzeText` / `suggestWithState` call needs the same idx;
// `destroyState` returns the slot to the pool.
//
// The render path is `updateState` + `analyzeText`: one
// postMessage each, instead of `Promise.all(words.map(checkAsync))`
// which used to fire 16+ crossings per textarea render.

export interface Hit {
  word: string;
  start: number;
  end: number;
  suggestions: string[];
}

export function createState(): Promise<number> {
  return new Promise<number>((resolve, reject) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'create-state' && typeof msg.idx === 'number') {
        resolve(msg.idx);
      } else {
        reject(new Error('create-state: unexpected response'));
      }
    });
    worker!.postMessage({ type: 'create-state', id });
  });
}

export function destroyState(idx: number): void {
  // Fire-and-forget -- there's no `destroy-state` reply envelope.
  const id = nextId++;
  worker!.postMessage({ type: 'destroy-state', id, idx });
}

export function clearState(idx: number): void {
  worker!.postMessage({ type: 'clear-state', id: nextId++, idx });
}

export function updateState(idx: number, text: string): void {
  worker!.postMessage({ type: 'update-state', id: nextId++, idx, text });
}

/** Grammar diagnostics for one textarea, through its per-textbox cache. */
export function checkSyntax(
  idx: number,
  text: string,
): Promise<DiagnosticSpan[]> {
  return new Promise<DiagnosticSpan[]>((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'check-syntax') resolve(msg.diagnostics);
      else resolve([]);
    });
    worker!.postMessage({ type: 'check-syntax', id, idx, text });
  });
}

/** Style diagnostics. The style pass is stateless. */
export function checkStyle(text: string): Promise<DiagnosticSpan[]> {
  return new Promise<DiagnosticSpan[]>((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'check-style') resolve(msg.diagnostics);
      else resolve([]);
    });
    worker!.postMessage({ type: 'check-style', id, text });
  });
}

export function analyzeText(
  idx: number,
  text: string,
  max: number,
): Promise<Hit[]> {
  return new Promise<Hit[]>((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'analyze-text') resolve(msg.hits);
      else resolve([]);
    });
    worker!.postMessage({ type: 'analyze-text', id, idx, text, max });
  });
}

// Two-pass render: returns the misspelling ranges instantly
// (fast lemma-HashSet check), and `onSuggestionReady` streams
// per-word suggestion-ready envelopes as the background wasm
// calls finish (each memoised on both sides). Repeat clicks on
// a word whose suggestions already finished in a prior render
// hit the side cache and skip the wasm call entirely.
//
// If `onSuggestionReady` is omitted, the function returns only
// the first-pass ranges; the worker still computes the
// suggestions in the background but the client throws them
// away (they remain in the worker's LRU for later reads).
export function analyzeMisspellsAsync(
  idx: number,
  text: string,
  max: number,
  maxDistance: number,
  onSuggestionReady?: (
    word: string, suggestions: string[],
    max: number, maxDistance: number,
  ) => void,
): Promise<{ word: string; start: number; end: number; caseOnly?: boolean }[]> {
  return new Promise((resolve, reject) => {
    const id = nextId++;
    if (onSuggestionReady) {
      liveSuggestionListeners.set(id, {
        listener: onSuggestionReady, max, maxDistance,
      });
    }
    pending.set(id, (msg) => {
      if (msg.type === 'misspells') resolve(msg.hits);
      else reject(new Error('analyze-misspells: unexpected response'));
    });
    worker!.postMessage({
      type: 'analyze-misspells', id, idx, text, max, maxDistance,
    });
  });
}

// One-shot suggestion for a single word. Hits the worker's LRU
// cache and -- if the worker already finished computing it --
// returns synchronously (within microseconds, no wasm call).
// Useful when the popup opens on a word whose background pass
// hasn't landed yet.
export function suggestForMisspellAsync(
  idx: number,
  word: string,
  max: number,
  maxDistance: number,
): Promise<string[]> {
  return new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'suggest-for-misspell') resolve(msg.suggestions);
      else resolve([]);
    });
    worker!.postMessage({
      type: 'suggest-for-misspell', id, idx, word, max, maxDistance,
    });
  });
}

// Detach the live listener for an in-flight render. The
// background pass may still finish, but the popup won't update
// until you re-render. Usually you don't need this -- the
// listener is cheap and stays valid for the lifetime of the
// page -- but it's handy when you know a fresh render is
// coming and don't want stale suggestions to flicker in.
export function removeMisspellListener(id: number): void {
  liveSuggestionListeners.delete(id);
}

export function suggestWithState(
  idx: number,
  word: string,
  max: number,
  maxDistance: number,
): Promise<string[]> {
  return new Promise<string[]>((resolve) => {
    const id = nextId++;
    pending.set(id, (msg) => {
      if (msg.type === 'suggest-with-state') resolve(msg.suggestions);
      else resolve([]);
    });
    worker!.postMessage({
      type: 'suggest-with-state', id, idx, word, max, maxDistance,
    });
  });
}
