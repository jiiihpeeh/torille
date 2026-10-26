// Suggestion worker -- a minimal sibling of `spell-worker.ts`
// that ONLY handles wasm calls the per-textbox render needs to
// fan out in parallel after the first-pass misspell ranges
// land. Spawned by the main thread's `startSuggestionPool` (in
// `spell-client.ts`) so that several workers can run wasm calls
// concurrently, with each one memoising its own results.
//
// A separate worker file (instead of routing through the
// primary `spell-worker.ts`) is the reason `bg.check` and
// `bg.suggest_for_misspell` can run truly in parallel: each
// worker has its own wasm instance, and the demo's main thread
// picks the worker with the shortest queue for each new
// request.

import { bg, initTorilleWasm } from './wasm-loader';

const workerSelf = self as unknown as {
  postMessage(m: unknown): void;
  addEventListener(
    type: string,
    listener: (e: MessageEvent) => void,
  ): void;
};

const post = (m: unknown): void => {
  workerSelf.postMessage(m);
};

function postError(
  id: number | undefined,
  for_: string,
  err: unknown,
): void {
  post({
    type: 'error',
    id: id ?? -1,
    for: for_,
    error: String((err as Error)?.message ?? err),
  });
}

const SUG_DEBUG = true;
function log(label: string, ...args: unknown[]): void {
  if (!SUG_DEBUG) return;
  const ts = new Date().toISOString().replace('T', ' ').slice(0, 23);
  console.log(`[torille-sug] [${ts}] [${label}]`, ...args);
}

const wasmUrl = new URL('./torille_wasm_bg.wasm', import.meta.url);
log('boot:wasm-url', wasmUrl.toString());
try {
  await initTorilleWasm(wasmUrl.toString());
} catch (err) {
  log('boot:throw', String(err));
  postError(undefined, 'init', err);
  throw err;
}
try {
  bg.warm_caches();
} catch (err) {
  log('warm:throw', String(err));
}
log('boot:ready', 'wasm up, caches warm');
post({ type: 'ready' });

// ── per-worker LRU memo cache ──────────────────────────────────────
const MEMO_LIMIT = 2048;
const memo = new Map<string, string[]>();

function memoKey(
  word: string, max: number, maxDistance: number,
): string {
  return max + '\u0000' + maxDistance + '\u0000' + word.toLowerCase();
}

function memoGet(key: string): string[] | undefined {
  const v = memo.get(key);
  if (v === undefined) return undefined;
  memo.delete(key);
  memo.set(key, v); // LRU touch
  return v;
}
function memoSet(key: string, suggestions: string[]): void {
  if (memo.has(key)) {
    memo.delete(key);
  } else if (memo.size >= MEMO_LIMIT) {
    const oldest = memo.keys().next().value;
    if (oldest !== undefined) memo.delete(oldest);
  }
  memo.set(key, suggestions);
}

// ── dispatch ─────────────────────────────────────────────────────────
//
//  Two message types are handled:
//
//    * { type: 'check-known', word } -- runs `bg.check(word)`, the
//      compound/derivation-aware known-word probe. The first pass
//      already ran the cheap gate pipeline, so this only adds the
//      forms those gates miss; it does not run the full engine walk.
//      Returns true if the word is a known Finnish form. Used to
//      take inflected forms (talossa, juoksee, ...) OFF the misspell
//      list without re-blocking the first-pass render.
//
//    * { type: 'suggest-for-misspell', word, max, maxDistance }
//      -- returns the ranked candidates. Memoised so repeat asks
//      are microseconds, no wasm call.

workerSelf.addEventListener('message', (e: MessageEvent) => {
  const msg = e.data as {
    id?: number;
    type: string;
    word?: string;
    max?: number;
    maxDistance?: number;
  };
  log('in:recv', `type=${msg.type} id=${msg.id} word=${msg.word ?? ''}`);
  if (msg.type === 'check-known') {
    const id = msg.id;
    const word = (msg.word ?? '').toLowerCase();
    if (!word) {
      post({ type: 'check-known', id, known: false });
      return;
    }
    const t0 = performance.now();
    try {
      const known = !!bg.check(word);
      const dt = (performance.now() - t0).toFixed(2);
      log('wasm:check-known', `id=${id} word=${word} known=${known} (${dt}ms)`);
      post({ type: 'check-known', id, known });
    } catch (err) {
      log('wasm:check-known:throw', `id=${id} word=${word}`, String(err));
      postError(id, 'check-known', err);
    }
    return;
  }
  if (msg.type === 'suggest-for-misspell') {
    const id = msg.id;
    const word = msg.word ?? '';
    const max = msg.max ?? 8;
    const maxDistance = msg.maxDistance ?? 2;
    const key = memoKey(word, max, maxDistance);
    const cached = memoGet(key);
    if (cached !== undefined) {
      log('memo:hit',
          `id=${id} word=${word} max=${max} dist=${maxDistance}`,
          `-> ${cached.length} candidates (cached)`);
      post({
        type: 'suggest-for-misspell', id,
        suggestions: cached, cached: true,
      });
      return;
    }
    const t0 = performance.now();
    try {
      const arr = bg.suggest_for_misspell(0, word, max, maxDistance);
      const suggestions: string[] = [];
      for (let i = 0; i < arr.length; i++) suggestions.push(arr[i] as string);
      memoSet(key, suggestions);
      const dt = (performance.now() - t0).toFixed(2);
      log('wasm:suggest-for-misspell',
          `id=${id} word=${word} max=${max} dist=${maxDistance}`,
          `-> ${suggestions.length} candidates (${dt}ms)`,
          suggestions.length > 0 ? `[${suggestions.slice(0, 3).join(', ')}...]` : '');
      post({
        type: 'suggest-for-misspell', id,
        suggestions, cached: false,
      });
    } catch (err) {
      log('wasm:suggest-for-misspell:throw',
          `id=${id} word=${word}`, String(err));
      postError(id, 'suggest-for-misspell', err);
    }
    return;
  }
});
