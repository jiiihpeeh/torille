// Spell-check worker.
//
// The wasm lives entirely on this thread; the main thread sends
// `R_WorkerInbound` requests and gets matching `R_WorkerOutbound`
// responses back. The full protocol is supported (single + batch
// check/suggest, debug toggle) so the demo speaks the same
// postMessage contract as the browser extensions.
//
// Two worker-side caches (checkCache, suggestCache) keep repeated
// words free across renders of the same textarea. The demo only
// has one text input, so the cache is mostly a stress test for the
// extension's behaviour; both code paths share the same logic.

import { bg, initTorilleWasm } from './wasm-loader';
import type { R_WorkerInbound, R_WorkerOutbound } from '../../protocol.js';

// The wasm-bindgen 0.14 shim only exposes the static-init path
// (`import * as wasm from "./x.wasm"; __wbg_set_wasm(wasm);
// wasm.__wbindgen_start()`). Current Chrome (155+ here) rejects
// `import *` from a `.wasm` response with `Failed to load module
// script: Expected a JavaScript-or-Wasm module script but the
// server responded with a MIME type of "application/wasm"` —
// the spec change apparently flipped from "respond with text/
// javascript" to "respond with application/wasm" with the
// generic "or-wasm" wording, and the namespace form no longer
// matches. Firefox has the same kind of strict-MIME error with
// slightly different text.
//
// `wasm-loader.ts` works around this by importing the shim
// directly from `torille_wasm_bg.js` (the namespace the shim
// declares its imports under), manually fetching +
// `WebAssembly.compile` + `WebAssembly.instantiate`-ing the wasm
// with the shim's 9 import functions, then calling
// `__wbg_set_wasm` to attach the instance and
// `__wbindgen_start` to seed the externref table. After that,
// every shim export (including `bg.check`, `bg.create_text_area_
// state`, `bg.analyze_text`, ...) dispatches into the live
// instance.

// We default the in-worker debug log ON. The user asked for
// comprehensive stage-by-stage output, so every handler
// entry/exit, every wasm call, every postMessage, and every
// cache hit/miss now prints to the worker console with a
// consistent timestamp prefix. The `TORILLE_DEBUG` toggle is
// retained in case we ever need to silence it from the main
// thread (it does not have a side-channel today; the demo
// currently hard-codes the value at module load).
let TORILLE_DEBUG = true;

// Every log line gets a millisecond timestamp prefix and a
// consistent `[torille-demo:…]` tag, so a worker console dump
// reads as a single interleaved timeline against the page's
// own console. `dbgStage` is the entrance/exit marker -- pair
// them around any handler block so the timeline shows what ran
// in what order.
function ts(): string {
  return new Date().toISOString().replace('T', ' ').slice(0, 23);
}
function dbg(label: string, ...args: unknown[]): void {
  if (!TORILLE_DEBUG) return;
  const head = label ? `[torille-demo] [${ts()}] [${label}]` : `[torille-demo] [${ts()}]`;
  console.log(head, ...args);
}
function dbgStage(enter: boolean, stage: string, extra?: unknown): void {
  if (!TORILLE_DEBUG) return;
  const arrow = enter ? '\u25B6' : '\u25C0';
  const tail = extra === undefined ? '' : ` ${typeof extra === 'string' ? extra : JSON.stringify(extra)}`;
  console.log(`[torille-demo] [${ts()}] ${arrow} ${stage}${tail}`);
}

const checkCache: Map<string, boolean> = new Map();
const CHECK_CACHE_LIMIT = 16384;

// The wasm instance's exports, set once at boot by the manual
// loader. Kept so debug handlers can inspect the linear memory.
let wasmExports: WebAssembly.Exports | undefined;

function checkCacheGet(word: string): boolean | undefined {
  const v = checkCache.get(word);
  if (v === undefined) return undefined;
  // LRU touch: re-insert so the entry is fresh.
  checkCache.delete(word);
  checkCache.set(word, v);
  if (TORILLE_DEBUG) dbg('cache:check:hit', word, '->', v,
                        `size=${checkCache.size}/${CHECK_CACHE_LIMIT}`);
  return v;
}

function checkCacheSet(word: string, ok: boolean): void {
  if (checkCache.has(word)) {
    checkCache.delete(word);
  } else if (checkCache.size >= CHECK_CACHE_LIMIT) {
    const oldest = checkCache.keys().next().value;
    if (oldest !== undefined) checkCache.delete(oldest);
  }
  checkCache.set(word, ok);
  if (TORILLE_DEBUG) dbg('cache:check:set', word, '->', ok,
                        `size=${checkCache.size}/${CHECK_CACHE_LIMIT}`);
}

const suggestCache: Map<string, string[]> = new Map();
const SUGGEST_CACHE_LIMIT = 2048;

function suggestCacheGet(word: string, max: number): string[] | undefined {
  const key = max + '\u0000' + word;
  const v = suggestCache.get(key);
  if (v === undefined) return undefined;
  suggestCache.delete(key);
  suggestCache.set(key, v);
  if (TORILLE_DEBUG) dbg('cache:suggest:hit', word, `max=${max}`,
                         '->', v.length, 'candidates',
                         `size=${suggestCache.size}/${SUGGEST_CACHE_LIMIT}`);
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
  if (TORILLE_DEBUG) dbg('cache:suggest:set', word, `max=${max}`,
                         '->', suggestions.length, 'candidates',
                         `size=${suggestCache.size}/${SUGGEST_CACHE_LIMIT}`);
}

// Per-state memo cache for `suggest_words_with_state`. The state
// pool is identified by `idx` -- a slot's `proper_nouns` can
// change between renders (the user cleared the textarea, typed a
// new proper noun, ...) so the cache key MUST include `idx`.
// When a slot is `destroy_text_area_state`-ed, the matching
// entries get cleaned up.
const SUGGEST_WITH_STATE_CACHE_LIMIT = 2048;
const suggestWithStateCache: Map<string, string[]> = new Map();
function suggestWithStateCacheKey(
  idx: number, word: string, max: number, maxDistance: number,
): string {
  return idx + '\u0001' + max + '\u0001' + maxDistance + '\u0001' + word.toLowerCase();
}
function suggestWithStateCacheGet(
  idx: number, word: string, max: number, maxDistance: number,
): string[] | undefined {
  const k = suggestWithStateCacheKey(idx, word, max, maxDistance);
  const v = suggestWithStateCache.get(k);
  if (v === undefined) return undefined;
  suggestWithStateCache.delete(k);
  suggestWithStateCache.set(k, v);
  if (TORILLE_DEBUG) dbg('cache:suggest-with-state:hit',
                         `idx=${idx} word=${word} max=${max} dist=${maxDistance}`,
                         `-> ${v.length} candidates`,
                         `size=${suggestWithStateCache.size}/${SUGGEST_WITH_STATE_CACHE_LIMIT}`);
  return v;
}
function suggestWithStateCacheSet(
  idx: number, word: string, max: number, maxDistance: number, suggestions: string[],
): void {
  const k = suggestWithStateCacheKey(idx, word, max, maxDistance);
  if (suggestWithStateCache.has(k)) {
    suggestWithStateCache.delete(k);
  } else if (suggestWithStateCache.size >= SUGGEST_WITH_STATE_CACHE_LIMIT) {
    const oldest = suggestWithStateCache.keys().next().value;
    if (oldest !== undefined) suggestWithStateCache.delete(oldest);
  }
  suggestWithStateCache.set(k, suggestions);
  if (TORILLE_DEBUG) dbg('cache:suggest-with-state:set',
                         `idx=${idx} word=${word} max=${max} dist=${maxDistance}`,
                         `-> ${suggestions.length} candidates`,
                         `size=${suggestWithStateCache.size}/${SUGGEST_WITH_STATE_CACHE_LIMIT}`);
}
function suggestWithStateCacheInvalidateIdx(idx: number): void {
  const prefix = idx + '\u0001';
  let removed = 0;
  for (const k of suggestWithStateCache.keys()) {
    if (k.startsWith(prefix)) {
      suggestWithStateCache.delete(k);
      removed++;
    }
  }
  if (removed > 0 && TORILLE_DEBUG) {
    dbg('cache:suggest-with-state:invalidate', `idx=${idx} removed=${removed}`);
  }
}

function postError(id: number | undefined, op: string | undefined,
                   word: string | undefined, err: unknown): void {
  const out: R_WorkerOutbound = {
    type: 'error',
    id: id ?? -1,
    for: op ?? 'unknown',
    word,
    error: String(err),
  };
  if (TORILLE_DEBUG) dbg('error:post',
                         `id=${out.id} for=${out.for} word=${word ?? '(none)'} error=${out.error}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
}

async function handleCheck(msg: Extract<R_WorkerInbound, { type: 'check' }>): Promise<void> {
  dbgStage(true, 'handler:check:enter', `id=${msg.id} word=${JSON.stringify(msg.word)}`);
  // Cache hit path: same handler, but we never touch the wasm. Log
  // the cache look-up so the timeline shows the path actually taken.
  const cached = checkCacheGet(msg.word);
  let ok = cached;
  if (ok === undefined) {
    const t0 = performance.now();
    try {
      ok = bg.check(msg.word);
    } catch (err) {
      dbgStage(false, 'handler:check:throw',
               `${msg.word} -> ${String(err)}`);
      postError(msg.id, 'check', msg.word, err);
      return;
    }
    const dt = (performance.now() - t0).toFixed(2);
    if (TORILLE_DEBUG) dbg('wasm:check', `id=${msg.id}`,
                           `word=${JSON.stringify(msg.word)}`,
                           `-> ok=${ok} (${dt}ms)`);
    checkCacheSet(msg.word, ok);
  } else {
    if (TORILLE_DEBUG) dbg('wasm:check:cache', `id=${msg.id}`,
                           `word=${JSON.stringify(msg.word)}`,
                           `-> ok=${ok} (cached)`);
  }
  const out: R_WorkerOutbound = { type: 'check', id: msg.id, ok };
  if (TORILLE_DEBUG) dbg('out:post', `type=check id=${msg.id} ok=${ok}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
  dbgStage(false, 'handler:check:exit', `id=${msg.id}`);
}

async function handleSuggest(msg: Extract<R_WorkerInbound, { type: 'suggest' }>): Promise<void> {
  dbgStage(true, 'handler:suggest:enter',
           `id=${msg.id} word=${JSON.stringify(msg.word)} max=${msg.max}`);
  const cached = suggestCacheGet(msg.word, msg.max);
  let suggestions: string[];
  if (cached !== undefined) {
    suggestions = cached;
  } else {
    const t0 = performance.now();
    try {
      suggestions = Array.from(bg.suggest_words(msg.word, msg.max));
    } catch (err) {
      dbgStage(false, 'handler:suggest:throw',
               `${msg.word} -> ${String(err)}`);
      postError(msg.id, 'suggest', msg.word, err);
      return;
    }
    const dt = (performance.now() - t0).toFixed(2);
    if (TORILLE_DEBUG) dbg('wasm:suggest', `id=${msg.id}`,
                           `word=${JSON.stringify(msg.word)}`,
                           `max=${msg.max}`,
                           `-> ${suggestions.length} candidates (${dt}ms)`,
                           suggestions.length > 0
                             ? `[${suggestions.slice(0, 5).join(', ')}...]`
                             : '');
    suggestCacheSet(msg.word, msg.max, suggestions);
  }
  const out: R_WorkerOutbound = {
    type: 'suggest', id: msg.id, suggestions,
  };
  if (TORILLE_DEBUG) dbg('out:post',
                         `type=suggest id=${msg.id} n=${out.suggestions.length}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
  dbgStage(false, 'handler:suggest:exit', `id=${msg.id}`);
}

async function handleCheckBatch(
  msg: Extract<R_WorkerInbound, { type: 'check-batch' }>,
): Promise<void> {
  dbgStage(true, 'handler:check-batch:enter',
           `id=${msg.id} inputId=${msg.inputId} n=${msg.words.length}`);
  const words = msg.words;
  const ok: boolean[] = new Array(words.length);
  const missIdx: number[] = [];
  const missWords: string[] = [];
  for (let i = 0; i < words.length; i++) {
    const cached = checkCacheGet(words[i]);
    if (cached !== undefined) {
      ok[i] = cached;
    } else {
      missIdx.push(i);
      missWords.push(words[i]);
    }
  }
  if (TORILLE_DEBUG) dbg('cache:check-batch:split',
                         `n=${words.length} hits=${words.length - missWords.length} misses=${missWords.length}`);
  for (let j = 0; j < missWords.length; j++) {
    const w = missWords[j];
    let v: boolean;
    const t0 = performance.now();
    try {
      v = bg.check(w);
    } catch (err) {
      v = false;
      if (TORILLE_DEBUG) dbg('wasm:check:throw', `id=${msg.id}`,
                             `word=${w}`, String(err));
    }
    const dt = (performance.now() - t0).toFixed(2);
    if (TORILLE_DEBUG) dbg('wasm:check', `id=${msg.id}`,
                           `word=${w}`, `-> ${v} (${dt}ms)`);
    ok[missIdx[j]] = v;
    checkCacheSet(w, v);
  }
  const out: R_WorkerOutbound = {
    type: 'check-batch', id: msg.id, inputId: msg.inputId, ok,
  };
  if (TORILLE_DEBUG) dbg('out:post',
                         `type=check-batch id=${msg.id} n=${ok.length}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
  dbgStage(false, 'handler:check-batch:exit', `id=${msg.id}`);
}

async function handleSuggestBatch(
  msg: Extract<R_WorkerInbound, { type: 'suggest-batch' }>,
): Promise<void> {
  dbgStage(true, 'handler:suggest-batch:enter',
           `id=${msg.id} inputId=${msg.inputId} n=${msg.items.length}`);
  const items = msg.items;
  const results: string[][] = new Array(items.length);
  const missIdx: number[] = [];
  const missItems: { word: string; max: number }[] = [];
  for (let i = 0; i < items.length; i++) {
    const it = items[i];
    const cached = suggestCacheGet(it.word, it.max);
    if (cached !== undefined) {
      results[i] = cached;
    } else {
      missIdx.push(i);
      missItems.push(it);
    }
  }
  if (TORILLE_DEBUG) dbg('cache:suggest-batch:split',
                         `n=${items.length} hits=${items.length - missItems.length} misses=${missItems.length}`);
  for (let j = 0; j < missItems.length; j++) {
    const it = missItems[j];
    let sugg: string[];
    const t0 = performance.now();
    try {
      sugg = Array.from(bg.suggest_words(it.word, it.max));
    } catch (err) {
      sugg = [];
      if (TORILLE_DEBUG) dbg('wasm:suggest:throw', `id=${msg.id}`,
                             `word=${it.word}`, String(err));
    }
    const dt = (performance.now() - t0).toFixed(2);
    if (TORILLE_DEBUG) dbg('wasm:suggest', `id=${msg.id}`,
                           `word=${it.word}`, `max=${it.max}`,
                           `-> ${sugg.length} candidates (${dt}ms)`);
    results[missIdx[j]] = sugg;
    suggestCacheSet(it.word, it.max, sugg);
  }
  const out: R_WorkerOutbound = {
    type: 'suggest-batch', id: msg.id, inputId: msg.inputId, results,
  };
  if (TORILLE_DEBUG) dbg('out:post',
                         `type=suggest-batch id=${msg.id} n=${results.length}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
  dbgStage(false, 'handler:suggest-batch:exit', `id=${msg.id}`);
}

// Per-textbox state. `analyze_text` does the render path in one
// wasm call; `update_state_from_text` teaches the state proper
// nouns from the user's text; `clear_state` wipes the learned
// values when a textarea is reset.
function handleCreateState(
  msg: Extract<R_WorkerInbound, { type: 'create-state' }>,
): void {
  dbgStage(true, 'handler:create-state:enter', `id=${msg.id}`);
  const t0 = performance.now();
  let idx: number;
  try {
    idx = bg.create_text_area_state();
  } catch (err) {
    if (TORILLE_DEBUG) dbg('wasm:create-state:throw',
                           `id=${msg.id}`, String(err));
    postError(msg.id, 'create-state', undefined, err);
    return;
  }
  const dt = (performance.now() - t0).toFixed(2);
  if (TORILLE_DEBUG) dbg('wasm:create-state', `id=${msg.id}`,
                         `-> idx=${idx} (${dt}ms)`);
  const out: R_WorkerOutbound = {
    type: 'create-state', id: msg.id, idx,
  };
  if (TORILLE_DEBUG) dbg('out:post', `type=create-state id=${msg.id} idx=${idx}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
  dbgStage(false, 'handler:create-state:exit', `id=${msg.id} idx=${idx}`);
}

function handleDestroyState(
  msg: Extract<R_WorkerInbound, { type: 'destroy-state' }>,
): void {
  dbgStage(true, 'handler:destroy-state:enter',
           `id=${msg.id} idx=${msg.idx}`);
  const t0 = performance.now();
  try {
    bg.destroy_text_area_state(msg.idx);
  } catch (err) {
    if (TORILLE_DEBUG) dbg('wasm:destroy-state:throw',
                           `id=${msg.id}`, String(err));
    dbgStage(false, 'handler:destroy-state:exit', `id=${msg.id} threw`);
    return;
  }
  const dt = (performance.now() - t0).toFixed(2);
  if (TORILLE_DEBUG) dbg('wasm:destroy-state', `id=${msg.id}`,
                         `idx=${msg.idx} (${dt}ms)`);
  // The slot is gone -- drop every memo entry that referenced
  // its `proper_nouns`. Without this, a destroyed-and-recreated
  // slot at the same idx would reuse stale suggestions.
  suggestWithStateCacheInvalidateIdx(msg.idx);
  dbgStage(false, 'handler:destroy-state:exit', `id=${msg.id}`);
  // No outbound for this one -- the caller doesn't need a reply.
}

function handleClearState(
  msg: Extract<R_WorkerInbound, { type: 'clear-state' }>,
): void {
  dbgStage(true, 'handler:clear-state:enter',
           `id=${msg.id} idx=${msg.idx}`);
  const t0 = performance.now();
  try {
    bg.clear_state(msg.idx);
  } catch (err) {
    if (TORILLE_DEBUG) dbg('wasm:clear-state:throw',
                           `id=${msg.id}`, String(err));
    dbgStage(false, 'handler:clear-state:exit', `id=${msg.id} threw`);
    return;
  }
  const dt = (performance.now() - t0).toFixed(2);
  if (TORILLE_DEBUG) dbg('wasm:clear-state', `id=${msg.id}`,
                         `idx=${msg.idx} (${dt}ms)`);
  // `clear_state` wipes the learned values; the cache's
  // `proper_nouns`-bearing entries for this idx are stale.
  suggestWithStateCacheInvalidateIdx(msg.idx);
  dbgStage(false, 'handler:clear-state:exit', `id=${msg.id}`);
}

function handleUpdateState(
  msg: Extract<R_WorkerInbound, { type: 'update-state' }>,
): void {
  dbgStage(true, 'handler:update-state:enter',
           `id=${msg.id} idx=${msg.idx} textLen=${msg.text.length}`);
  if (TORILLE_DEBUG) dbg('handler:update-state:input',
                         `id=${msg.id} text=${JSON.stringify(msg.text.length > 64
                                                  ? msg.text.slice(0, 60) + '…'
                                                  : msg.text)}`);
  const t0 = performance.now();
  try {
    bg.update_state_from_text(msg.idx, msg.text);
  } catch (err) {
    if (TORILLE_DEBUG) dbg('wasm:update-state:throw',
                           `id=${msg.id}`, String(err));
    dbgStage(false, 'handler:update-state:exit',
             `id=${msg.id} threw after ${(performance.now() - t0).toFixed(2)}ms`);
    return;
  }
  const dt = (performance.now() - t0).toFixed(2);
  if (TORILLE_DEBUG) dbg('wasm:update-state', `id=${msg.id}`,
                         `idx=${msg.idx}`, `(${dt}ms)`);
  // `update_state_from_text` rebuilds the slot's `proper_nouns`
  // and `phrases`. Every memo entry that referenced this idx
  // is now potentially stale (a `Helsnki` -> `["helsinki"]`
  // entry becomes correct only because of the learned
  // `proper_nouns`). Drop them so the next click recomputes.
  suggestWithStateCacheInvalidateIdx(msg.idx);
  dbgStage(false, 'handler:update-state:exit', `id=${msg.id}`);
}

function handleAnalyzeText(
  msg: Extract<R_WorkerInbound, { type: 'analyze-text' }>,
): void {
  dbgStage(true, 'handler:analyze-text:enter',
           `id=${msg.id} idx=${msg.idx} textLen=${msg.text.length} max=${msg.max}`);
  if (TORILLE_DEBUG) dbg('handler:analyze-text:input',
                         `id=${msg.id} text=${JSON.stringify(msg.text.length > 64
                                                  ? msg.text.slice(0, 60) + '…'
                                                  : msg.text)}`);
  const t0 = performance.now();
  let arr: { length: number; [k: number]: unknown };
  try {
    arr = bg.analyze_text(msg.idx, msg.text, msg.max) as typeof arr;
  } catch (err) {
    if (TORILLE_DEBUG) dbg('wasm:analyze-text:throw',
                           `id=${msg.id}`, String(err));
    dbgStage(false, 'handler:analyze-text:exit',
             `id=${msg.id} threw after ${(performance.now() - t0).toFixed(2)}ms`);
    return;
  }
  const wasmDt = (performance.now() - t0).toFixed(2);
  if (TORILLE_DEBUG) dbg('wasm:analyze-text', `id=${msg.id}`,
                         `idx=${msg.idx}`, `-> ${arr.length} hits raw (${wasmDt}ms)`);
  const hits: { word: string; start: number; end: number; suggestions: string[] }[] = [];
  for (let i = 0; i < arr.length; i++) {
    const h = arr[i] as {
      word: string; start: number; end: number;
      suggestions: { length: number; [k: number]: string };
    };
    const sugg: string[] = [];
    for (let j = 0; j < h.suggestions.length; j++) {
      sugg.push(h.suggestions[j]);
    }
    hits.push({ word: h.word, start: h.start, end: h.end, suggestions: sugg });
  }
  const out: R_WorkerOutbound = { id: msg.id, type: 'analyze-text', hits };
  if (TORILLE_DEBUG) dbg('out:post',
                         `type=analyze-text id=${msg.id} hits=${out.hits.length}`,
                         hits.length > 0
                           ? `[${hits.map(h => `${h.word}(${h.start}-${h.end},${h.suggestions.length}sugg)`).join(', ')}]`
                           : '');
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
  dbgStage(false, 'handler:analyze-text:exit', `id=${msg.id}`);
}

// ── memo cache for `suggest_for_misspell` ──────────────────────
//
// Keyed by `${max}\u0000${maxDistance}\u0000${wordLower}` so the
// demo's popup at max=5,distance=2 doesn't shadow a stricter
// max=8,distance=1 lookup. LRU eviction in insertion order, like
// the other caches in this worker. The cache lives across renders
// so the second click on the same misspelling is instant.
const SUGGEST_FOR_MISSPELL_CACHE_LIMIT = 2048;
const suggestForMisspellCache: Map<string, string[]> = new Map();

function suggestForMisspellKey(
  word: string, max: number, maxDistance: number,
): string {
  return max + '\u0000' + maxDistance + '\u0000' + word.toLowerCase();
}
function suggestForMisspellCacheGet(
  word: string, max: number, maxDistance: number,
): string[] | undefined {
  const key = suggestForMisspellKey(word, max, maxDistance);
  const v = suggestForMisspellCache.get(key);
  if (v === undefined) return undefined;
  suggestForMisspellCache.delete(key);
  suggestForMisspellCache.set(key, v);
  if (TORILLE_DEBUG) dbg('cache:suggest-for-misspell:hit', word,
                         `max=${max} dist=${maxDistance}`,
                         `-> ${v.length} candidates`,
                         `size=${suggestForMisspellCache.size}/${SUGGEST_FOR_MISSPELL_CACHE_LIMIT}`);
  return v;
}
function suggestForMisspellCacheSet(
  word: string, max: number, maxDistance: number,
  suggestions: string[],
): void {
  const key = suggestForMisspellKey(word, max, maxDistance);
  if (suggestForMisspellCache.has(key)) {
    suggestForMisspellCache.delete(key);
  } else if (suggestForMisspellCache.size >= SUGGEST_FOR_MISSPELL_CACHE_LIMIT) {
    const oldest = suggestForMisspellCache.keys().next().value;
    if (oldest !== undefined) suggestForMisspellCache.delete(oldest);
  }
  suggestForMisspellCache.set(key, suggestions);
  if (TORILLE_DEBUG) dbg('cache:suggest-for-misspell:set', word,
                         `max=${max} dist=${maxDistance}`,
                         `-> ${suggestions.length} candidates`,
                         `size=${suggestForMisspellCache.size}/${SUGGEST_FOR_MISSPELL_CACHE_LIMIT}`);
}

// Track in-flight suggestion work so two `analyze-misspells`
// calls that overlap on the same word don't duplicate the wasm
// call. Map<key, Promise<string[]>>.
const suggestForMisspellInflight: Map<string, Promise<string[]>> = new Map();

function suggestForMisspellCached(
  idx: number, word: string, max: number, maxDistance: number,
): Promise<string[]> {
  const cached = suggestForMisspellCacheGet(word, max, maxDistance);
  if (cached !== undefined) return Promise.resolve(cached);
  const key = suggestForMisspellKey(word, max, maxDistance);
  const inflight = suggestForMisspellInflight.get(key);
  if (inflight !== undefined) {
    if (TORILLE_DEBUG) dbg('cache:suggest-for-misspell:coalesce',
                           word, `max=${max} dist=${maxDistance}`);
    return inflight;
  }
  if (TORILLE_DEBUG) dbg('wasm:suggest-for-misspell:start',
                         `idx=${idx} word=${word}`,
                         `max=${max} dist=${maxDistance}`);
  const t0 = performance.now();
  const p = (async (): Promise<string[]> => {
    let arr: { length: number; [k: number]: unknown };
    try {
      arr = bg.suggest_for_misspell(idx, word, max, maxDistance) as typeof arr;
    } catch (err) {
      if (TORILLE_DEBUG) dbg('wasm:suggest-for-misspell:throw',
                             `word=${word}`, String(err));
      suggestForMisspellInflight.delete(key);
      return [];
    }
    const suggestions: string[] = [];
    for (let i = 0; i < arr.length; i++) suggestions.push(arr[i] as string);
    const dt = (performance.now() - t0).toFixed(2);
    if (TORILLE_DEBUG) dbg('wasm:suggest-for-misspell',
                           `word=${word} max=${max} dist=${maxDistance}`,
                           `-> ${suggestions.length} candidates (${dt}ms)`,
                           suggestions.length > 0
                             ? `[${suggestions.slice(0, 5).join(', ')}${suggestions.length > 5 ? '…' : ''}]`
                             : '');
    suggestForMisspellCacheSet(word, max, maxDistance, suggestions);
    suggestForMisspellInflight.delete(key);
    return suggestions;
  })();
  suggestForMisspellInflight.set(key, p);
  return p;
}

/// **Two-pass render.** First-pass: get the misspelling byte
/// ranges back IMMEDIATELY so the demo shows red underlines.
/// Second-pass: for every misspell, kick off a background
/// `suggest_for_misspell` (memoised) and `postMessage` each
/// result back to the main thread as it lands. Repeat clicks
/// hit the cache.
function handleAnalyzeMisspells(
  msg: Extract<R_WorkerInbound, { type: 'analyze-misspells' }>,
): void {
  dbgStage(true, 'handler:analyze-misspells:enter',
           `id=${msg.id} idx=${msg.idx} textLen=${msg.text.length} ` +
           `max=${msg.max} maxDistance=${msg.maxDistance}`);
  if (TORILLE_DEBUG) dbg('handler:analyze-misspells:input',
                         `id=${msg.id} text=${JSON.stringify(msg.text.length > 64
                                                  ? msg.text.slice(0, 60) + '…'
                                                  : msg.text)}`);

  // Fast path: ranges only.
  const t0 = performance.now();
  let arr: { length: number; [k: number]: unknown };
  try {
    arr = bg.analyze_misspells(msg.idx, msg.text) as typeof arr;
  } catch (err) {
    dbgStage(false, 'handler:analyze-misspells:throw',
             `id=${msg.id} ${String(err)}`);
    return;
  }
  const hits: { word: string; start: number; end: number; caseOnly?: boolean }[] = [];
  for (let i = 0; i < arr.length; i++) {
    const h = arr[i] as { word: string; start: number; end: number; caseOnly?: boolean };
    hits.push({ word: h.word, start: h.start, end: h.end, caseOnly: h.caseOnly });
  }
  const dt = (performance.now() - t0).toFixed(2);
  if (TORILLE_DEBUG) dbg('wasm:analyze-misspells', `id=${msg.id}`,
                         `idx=${msg.idx}`,
                         `-> ${hits.length} hits (${dt}ms)`,
                         hits.length > 0
                           ? `[${hits.map(h => `${h.word}@${h.start}-${h.end}`).join(', ')}]`
                           : '');

  // Synchronous first-pass reply. Misspell highlights land
  // instantly.
  const outMisspells: R_WorkerOutbound = {
    id: msg.id, type: 'misspells', hits,
  };
  if (TORILLE_DEBUG) dbg('out:post',
                         `type=misspells id=${msg.id} hits=${hits.length}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(outMisspells);

  // Background second pass is no longer fired from here: the
  // main thread's suggestion pool handles `bg.check` (parallel
  // inflection-aware "is this known?") and
  // `bg.suggest_for_misspell` (parallel ranking) per misspell
  // word across 3 sibling workers. Results stream into the
  // client-side side cache as they finish; the popup reads
  // from there. See `findMisspellings` + `spell-client.ts`.
  dbgStage(false, 'handler:analyze-misspells:exit',
           `id=${msg.id} (main-thread pool handles per-word passes)`);
}

/// Direct ask for a single word's suggestions. Used when the
/// demo's popup opens on a word that never appeared in a
/// `misspells` envelope (e.g. the user clicked before the
/// background pass fired). Hits the same cache.
function handleSuggestForMisspell(
  msg: Extract<R_WorkerInbound, { type: 'suggest-for-misspell' }>,
): void {
  dbgStage(true, 'handler:suggest-for-misspell:enter',
           `id=${msg.id} idx=${msg.idx} ` +
           `word=${JSON.stringify(msg.word)} max=${msg.max} dist=${msg.maxDistance}`);
  suggestForMisspellCached(msg.idx, msg.word, msg.max, msg.maxDistance)
    .then((suggestions) => {
      const out: R_WorkerOutbound = {
        id: msg.id, type: 'suggest-for-misspell', suggestions,
      };
      if (TORILLE_DEBUG) dbg('out:post',
                             `type=suggest-for-misspell id=${msg.id} ` +
                             `word=${JSON.stringify(msg.word)} n=${suggestions.length}`);
      (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
      dbgStage(false, 'handler:suggest-for-misspell:exit', `id=${msg.id}`);
    })
    .catch((err) => {
      const msgErr = String(err);
      postError(msg.id, 'suggest-for-misspell', msg.word, msgErr);
      dbgStage(false, 'handler:suggest-for-misspell:exit',
               `id=${msg.id} threw`);
    });
}

function handleSuggestWithState(
  msg: Extract<R_WorkerInbound, { type: 'suggest-with-state' }>,
): void {
  dbgStage(true, 'handler:suggest-with-state:enter',
           `id=${msg.id} idx=${msg.idx} word=${JSON.stringify(msg.word)} max=${msg.max} dist=${msg.maxDistance}`);
  // Per-state memo cache: the popup opens the same word several
  // times during a session, and the user's `proper_nouns` set
  // for this textarea may not have changed -- keep the
  // expensive `find_bases_with` result around so the second
  // click is microseconds, not seconds.
  const cached = suggestWithStateCacheGet(msg.idx, msg.word, msg.max, msg.maxDistance);
  let suggestions: string[];
  if (cached !== undefined) {
    suggestions = cached;
  } else {
    const t0 = performance.now();
    let arr: { length: number; [k: number]: unknown };
    try {
      arr = bg.suggest_words_with_state(msg.idx, msg.word, msg.max, msg.maxDistance) as typeof arr;
    } catch (err) {
      if (TORILLE_DEBUG) dbg('wasm:suggest-with-state:throw',
                             `id=${msg.id}`, String(err));
      dbgStage(false, 'handler:suggest-with-state:exit', `id=${msg.id} threw`);
      return;
    }
    suggestions = [];
    for (let i = 0; i < arr.length; i++) suggestions.push(arr[i] as string);
    const dt = (performance.now() - t0).toFixed(2);
    if (TORILLE_DEBUG) dbg('wasm:suggest-with-state', `id=${msg.id}`,
                           `idx=${msg.idx} word=${msg.word} dist=${msg.maxDistance}`,
                           `-> ${suggestions.length} candidates (${dt}ms)`,
                           suggestions.length > 0
                             ? `[${suggestions.join(', ')}]`
                             : '');
    suggestWithStateCacheSet(msg.idx, msg.word, msg.max, msg.maxDistance, suggestions);
  }
  const out: R_WorkerOutbound = {
    id: msg.id, type: 'suggest-with-state', suggestions,
  };
  if (TORILLE_DEBUG) dbg('out:post',
                         `type=suggest-with-state id=${msg.id} n=${out.suggestions.length}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(out);
  dbgStage(false, 'handler:suggest-with-state:exit', `id=${msg.id}`);
}

function handleDebug(msg: Extract<R_WorkerInbound, { type: 'debug' }>): void {
  TORILLE_DEBUG = !!msg.on;
  dbg('worker', 'debug ->', TORILLE_DEBUG);
}

function handleMemoryInfo(
  msg: Extract<R_WorkerInbound, { type: 'memory-info' }>,
): void {
  // Snapshot the worker's own state plus the wasm linear
  // memory size. The page uses this to distinguish "JS
  // heap grew" from "wasm memory grew" when chasing
  // leaks in the Gutenberg reader.
  const out: { wasmBytes: number; pendingCount: number; } = {
    wasmBytes: 0,
    pendingCount: suggestForMisspellInflight.size,
  };
  // Reach into the wasm instance to read its memory.
  // `wasmExports` is what the manual loader instantiated --
  // the instance's exports, attached to the glue via
  // `__wbg_set_wasm` at boot.
  try {
    const mem = (wasmExports as any)?.memory ?? (wasmExports as any)?.__wbindgen_memory;
    if (mem && typeof mem.buffer?.byteLength === 'number') {
      out.wasmBytes = mem.buffer.byteLength;
    }
  } catch {
    // Defensive: the wasm shape can vary between
    // wasm-pack versions; never throw out of a debug
    // handler.
  }
  (self as unknown as { postMessage(m: unknown): void }).postMessage({
    type: 'memory-info',
    id: msg.id,
    ...out,
  });
}

// Map of message-type -> handler. Adding a new op is one line here.
const HANDLERS: {
  check: (msg: Extract<R_WorkerInbound, { type: 'check' }>) => Promise<void>;
  suggest: (msg: Extract<R_WorkerInbound, { type: 'suggest' }>) => Promise<void>;
  'check-batch': (
    msg: Extract<R_WorkerInbound, { type: 'check-batch' }>,
  ) => Promise<void>;
  'suggest-batch': (
    msg: Extract<R_WorkerInbound, { type: 'suggest-batch' }>,
  ) => Promise<void>;
  'create-state': (
    msg: Extract<R_WorkerInbound, { type: 'create-state' }>,
  ) => void;
  'destroy-state': (
    msg: Extract<R_WorkerInbound, { type: 'destroy-state' }>,
  ) => void;
  'clear-state': (
    msg: Extract<R_WorkerInbound, { type: 'clear-state' }>,
  ) => void;
  'update-state': (
    msg: Extract<R_WorkerInbound, { type: 'update-state' }>,
  ) => void;
  'analyze-text': (
    msg: Extract<R_WorkerInbound, { type: 'analyze-text' }>,
  ) => void;
  'analyze-misspells': (
    msg: Extract<R_WorkerInbound, { type: 'analyze-misspells' }>,
  ) => void;
  'suggest-for-misspell': (
    msg: Extract<R_WorkerInbound, { type: 'suggest-for-misspell' }>,
  ) => void;
  'suggest-with-state': (
    msg: Extract<R_WorkerInbound, { type: 'suggest-with-state' }>,
  ) => void;
  debug: (msg: Extract<R_WorkerInbound, { type: 'debug' }>) => void;
  'memory-info': (msg: Extract<R_WorkerInbound, { type: 'memory-info' }>) => void;
} = {
  check: handleCheck,
  suggest: handleSuggest,
  'check-batch': handleCheckBatch,
  'suggest-batch': handleSuggestBatch,
  'create-state': handleCreateState,
  'destroy-state': handleDestroyState,
  'clear-state': handleClearState,
  'update-state': handleUpdateState,
  'analyze-text': handleAnalyzeText,
  'analyze-misspells': handleAnalyzeMisspells,
  'suggest-for-misspell': handleSuggestForMisspell,
  'suggest-with-state': handleSuggestWithState,
  debug: handleDebug,
  'memory-info': handleMemoryInfo,
};

const post = (m: R_WorkerOutbound): void => {
  if (TORILLE_DEBUG) dbg('out:post', `type=${m.type} id=${m.id}`);
  (self as unknown as { postMessage(m: unknown): void }).postMessage(m);
};

// Top-level error handlers. The wasm can trap on edge cases
// (e.g. unreachable when internal state is corrupted); these
// surface to the devtools console where the cause can be
// inspected. The main thread also sees per-message errors via
// the `error` envelope on the postMessage channel.
self.addEventListener('error', (e) => {
  console.error('[torille-demo worker:error]', e.message ?? e, e);
  if (TORILLE_DEBUG) dbg('worker:error:top',
                         e.message ?? '(no message)', e.error ?? '');
});
self.addEventListener('unhandledrejection', (e) => {
  console.error('[torille-demo worker:unhandledrejection]', e.reason);
  if (TORILLE_DEBUG) dbg('worker:unhandledrejection', String(e.reason));
});

// ── module body ────────────────────────────────────────────────
// Boot order: log the start, fetch the wasm bytes, instantiate
// via the manual loader, then post `ready`. Each step gets its
// own marker so a puppeteer run that fails to come up can see
// exactly where in the sequence the worker stalled.
dbgStage(true, 'worker:boot');
if (TORILLE_DEBUG) dbg('worker:boot:info', 'import.meta.url=', String(import.meta.url));

const wasmUrl = new URL('./torille_wasm_bg.wasm', import.meta.url);
if (TORILLE_DEBUG) dbg('worker:boot:wasm-url', wasmUrl.toString());
dbgStage(true, 'worker:boot:fetch');
try {
  wasmExports = await initTorilleWasm(wasmUrl.toString());
} catch (err) {
  dbgStage(false, 'worker:boot:instantiate:throw', String(err));
  postError(undefined, 'init', undefined,
             `wasm init failed: ${(err as Error)?.message ?? err}`);
  throw err;
}
dbgStage(false, 'worker:boot:fetch',
         `instantiated ok; bg has ${Object.keys(bg).length} exports`);
dbgStage(true, 'worker:boot:warm-caches');
const warmT0 = performance.now();
try {
  // Pre-fill the lemma `HashSet` (~100k entries) and the kotus
  // paradigm catalogue. Doing it here means the first user
  // keystroke's `check` doesn't pay ~1 second of cold-start.
  bg.warm_caches();
} catch (err) {
  // The fallback path also works -- first interactive call just
  // pays the warm-up cost. Don't fail boot over this.
  if (TORILLE_DEBUG) dbg('worker:boot:warm-caches:throw', String(err));
}
const warmDt = (performance.now() - warmT0).toFixed(0);
if (TORILLE_DEBUG) dbg('worker:boot:warm-caches:done',
                       `${warmDt}ms`);
dbgStage(false, 'worker:boot:warm-caches');

// Pre-warm the inflected form set so the first-pass
// `find_misspells` can recognise every form the engine can
// produce (gradated nouns, verb persons, ...), not just the
// lemma + productive noun-case strip. The first-pass check
// falls back to the fast strip until this returns, so the
// user is never blocked -- the warm-up just makes the
// underlines more accurate once it finishes. A few seconds
// on a desktop wasm build.
dbgStage(true, 'worker:boot:warm-form-set');
const formSetT0 = performance.now();
try {
  const dt = bg.warm_form_set();
  if (TORILLE_DEBUG) dbg('worker:boot:warm-form-set:done',
                         `${dt.toFixed(0)}ms wasm-side`);
} catch (err) {
  if (TORILLE_DEBUG) dbg('worker:boot:warm-form-set:throw', String(err));
}
const formSetDt = (performance.now() - formSetT0).toFixed(0);
if (TORILLE_DEBUG) dbg('worker:boot:warm-form-set:wall',
                       `${formSetDt}ms`);
dbgStage(false, 'worker:boot:warm-form-set');
post({ type: 'ready' });
if (TORILLE_DEBUG) dbg('out:post', 'type=ready id=0');
dbgStage(false, 'worker:boot', 'ready posted, onmessage registering');

(self as unknown as { onmessage: (e: MessageEvent<R_WorkerInbound>) => void }).onmessage =
  (e: MessageEvent<R_WorkerInbound>): void => {
    const msg = e.data;
    if (TORILLE_DEBUG) dbg('in:recv',
                           `type=${msg.type} id=${msg.id}`,
                           msg.type === 'check' || msg.type === 'suggest'
                              || msg.type === 'suggest-with-state'
                             ? `word=${JSON.stringify((msg as { word: string }).word)}`
                             : msg.type === 'analyze-text' || msg.type === 'update-state'
                              || msg.type === 'analyze-misspells'
                             ? `textLen=${(msg as { text: string }).text.length}`
                             : '');
    // Fire-and-forget: do NOT await the handler here. If we did,
    // the runtime's async `onmessage` semantics would queue every
    // subsequent message behind the current handler's promise,
    // serialising what is otherwise independent work -- a batch of
    // 16 `check` requests would process one at a time, with the
    // 16th waiting on the 1st. Each handler already manages its own
    // errors and posts its own response, so there's nothing to await.
    (async () => {
      try {
        if (msg.type === 'check') {
          await HANDLERS.check(msg);
        } else if (msg.type === 'suggest') {
          await HANDLERS.suggest(msg);
        } else if (msg.type === 'check-batch') {
          await HANDLERS['check-batch'](msg);
        } else if (msg.type === 'suggest-batch') {
          await HANDLERS['suggest-batch'](msg);
        } else if (msg.type === 'create-state') {
          HANDLERS['create-state'](msg);
        } else if (msg.type === 'destroy-state') {
          HANDLERS['destroy-state'](msg);
        } else if (msg.type === 'clear-state') {
          HANDLERS['clear-state'](msg);
        } else if (msg.type === 'update-state') {
          HANDLERS['update-state'](msg);
        } else if (msg.type === 'analyze-text') {
          HANDLERS['analyze-text'](msg);
        } else if (msg.type === 'analyze-misspells') {
          HANDLERS['analyze-misspells'](msg);
        } else if (msg.type === 'suggest-for-misspell') {
          HANDLERS['suggest-for-misspell'](msg);
        } else if (msg.type === 'suggest-with-state') {
          HANDLERS['suggest-with-state'](msg);
        } else if (msg.type === 'debug') {
          HANDLERS.debug(msg);
        } else {
          if (TORILLE_DEBUG) dbg('in:unknown-type',
                                 (msg as { type?: string }).type ?? '(none)');
        }
      } catch (err) {
        if (TORILLE_DEBUG) dbg('handler:unhandled',
                               `id=${(msg as { id?: number }).id}`,
                               `type=${(msg as { type?: string }).type}`,
                               String(err));
        const m = msg as { id?: number; type?: string; word?: string };
        postError(m.id, m.type, m.word, err);
      }
    })();
  };
