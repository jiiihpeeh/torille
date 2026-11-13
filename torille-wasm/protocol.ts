// Spell-check worker message protocol.
//
// The wasm lives in a module worker; the main thread (the demo's
// Solid app, or an extension's background page) talks to it via
// postMessage. These unions are the contract. Both ends of the
// channel import from this file so a message-shape change touches
// exactly one place.
//
// This is the *worker-level* protocol. The extension's content
// script and background script have their own protocol on top of
// `runtime.sendMessage` (the `torille:check` / `torille:suggest` /
// `torille:check-batch` / etc. family in `globals.d.ts`); the
// background translates between the two. The demo skips the
// intermediate hop and talks to the worker directly, so it only
// uses these types.
//
// `id` is a monotonic per-worker counter that lets the caller
// correlate responses to in-flight requests without a separate
// lock. `inputId` (on batch ops) is a routing tag the extension
// uses to tie a worker log line back to a specific text input
// on a specific page; the demo passes a constant 0.

/** One grammar or style diagnostic. The worker only forwards the
 *  fields the demo needs; the extension has its own richer ambient
 *  type in `globals.d.ts`. */
export interface DiagnosticSpan {
  kind: string;
  severity: string;
  confidence: string;
  start: number;
  end: number;
  message: string;
  expected?: string | null;
  corrections?: unknown[];
}

/** Inbound: caller -> worker. */
export type R_WorkerInbound =
  | { id: number; type: 'check'; word: string }
  | { id: number; type: 'suggest'; word: string; max: number }
  | { id: number; type: 'check-batch'; inputId: number; words: string[] }
  | {
      id: number;
      type: 'suggest-batch';
      inputId: number;
      items: { word: string; max: number }[];
    }
  // Per-textbox state lifecycle. The demo and the browser
  // extensions both speak these; the worker maintains a u32
  // index per text input and processes the whole textarea in
  // one wasm call so the render path collapses N postMessage /
  // wasm-bindgen crossings into a single one.
  | { id: number; type: 'create-state' }
  | { id: number; type: 'destroy-state'; idx: number }
  | { id: number; type: 'clear-state'; idx: number }
  | { id: number; type: 'update-state'; idx: number; text: string }
  | {
      id: number;
      type: 'analyze-text';
      idx: number;
      text: string;
      max: number;
    }
  | {
      id: number;
      type: 'suggest-with-state';
      idx: number;
      word: string;
      max: number;
      maxDistance: number;
    }
  // **Two-pass render**: the demo asks for misspell ranges, the
  // worker answers IMMEDIATELY (fast lemma-HashSet check), then
  // background wasm calls compute suggestions per word. Each
  // suggestion posts back as a separate `suggestion-ready`
  // message. The memo cache makes repeat clicks on the same word
  // instant.
  | {
      id: number;
      type: 'analyze-misspells';
      idx: number;
      text: string;
      max: number;            // suggested count sent in follow-ups
      maxDistance: number;    // edit distance cap for follow-ups
    }
  | {
      id: number;
      type: 'suggest-for-misspell';
      idx: number;
      word: string;
      max: number;
      maxDistance: number;
    }
  | { id: number; type: 'check-syntax'; idx: number; text: string }
  | { id: number; type: 'check-style'; text: string }
  | {
      id: number;
      type: 'check-grammar-style';
      idx: number;
      text: string;
      syntax: boolean;
      style: boolean;
    }
  | { type: 'debug'; on: boolean }
  | { id: number; type: 'memory-info' };

/** Outbound: worker -> caller. The `error` envelope covers every
 *  per-message throw; the `ready` / `init_failed` envelopes cover
 *  the startup handshake. */
export type R_WorkerOutbound =
  | { type: 'ready' }
  | { type: 'init_failed'; error: string }
  | { id: number; type: 'check'; ok: boolean }
  | { id: number; type: 'suggest'; suggestions: string[] }
  | { id: number; type: 'check-batch'; inputId: number; ok: boolean[] }
  | {
      id: number;
      type: 'suggest-batch';
      inputId: number;
      results: string[][];
    }
  | { id: number; type: 'create-state'; idx: number }
  | {
      id: number;
      type: 'analyze-text';
      hits: { word: string; start: number; end: number; suggestions: string[] }[];
    }
  | { id: number; type: 'suggest-with-state'; suggestions: string[] }
  // Two-pass render envelopes. `misspells` is the synchronous
  // first-pass reply; `suggestion-ready` posts one per misspell
  // word as the background wasm calls finish, with the cache
  // filling as we go so the click-then-click-again UX is instant.
  | {
      id: number;
      type: 'misspells';
      hits: { word: string; start: number; end: number }[];
    }
  | {
      id: number;          // correlates with the original analyze-misspells id
      type: 'suggestion-ready';
      word: string;
      suggestions: string[];
    }
  | { id: number; type: 'suggest-for-misspell'; suggestions: string[] }
  | { id: number; type: 'check-syntax'; diagnostics: DiagnosticSpan[] }
  | { id: number; type: 'check-style'; diagnostics: DiagnosticSpan[] }
  | { id: number; type: 'check-progress'; done: number; total: number }
  | {
      id: number;
      type: 'check-grammar-style';
      diagnostics: DiagnosticSpan[];
      style: DiagnosticSpan[];
    }
  | { id: number; type: 'memory-info'; wasmBytes: number; pendingCount: number }
  | { id: number; type: 'error'; for: string; word?: string; error: string };
