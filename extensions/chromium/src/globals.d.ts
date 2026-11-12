// globals.d.ts -- ambient types shared across every script.
//
// `@types/firefox-webext-browser` declares the `browser` global with
// promise-style APIs (the Firefox MV3 contract). Chromium uses the
// same surface but exposes it as `chrome`; we run the same source
// files in both browsers via `(typeof browser !== 'undefined') ? browser : chrome`,
// so we declare `chrome` with the same type so the content script can
// hold the runtime in a single `const api` without losing types.
//
// We also declare the runtime message types as ambient (no `import`)
// so content.ts and background.ts can use them without forcing the
// files into ES-module mode (which would emit `export {};` and break
// the classic `<script>` load). The "R_" prefix avoids name clashes
// with the runtime payload type field, which is also a string literal.
//
// The *worker* message types (`R_WorkerInbound` / `R_WorkerOutbound`)
// live in this file as ambient declarations and are also canonical in
// `torille-wasm/protocol.ts` (imported by the demo worker). The two
// copies must stay in sync; if you change R_WorkerInbound /
// R_WorkerOutbound here, mirror the change in
// `torille-wasm/protocol.ts`. The duplication exists because the
// extension's `tsconfig.json` pins `rootDir` to `./src`, so
// spell-worker.ts cannot import from outside the extension tree
// without a per-file wrapper stub, and the cleanest single-source
// option is a path mapping + rootDir relaxation that we have not
// yet done.

declare const chrome: typeof browser;

declare type R_Check = { type: 'torille:check'; word: string };
declare type R_Suggest = {
  type: 'torille:suggest';
  word: string;
  max: number;
};
// Fast per-word verdict for the contenteditable popup: runs the same
// first-pass path as `analyze-text` against one word (~1 ms) instead
// of the full `check` (which can take seconds on an unseen word).
declare type R_CheckFast = {
  type: 'torille:check-fast';
  idx: number;
  word: string;
};
// Batch variants for the render path. The single-word `R_Check` /
// `R_Suggest` are kept for the popup's per-click suggest, which only
// ever asks for one word at a time. A full re-render fires one of
// these instead of N parallel `R_Check`s.
//
// `inputId` is a small monotonic number assigned per textarea /
// contenteditable in the content script. Every batch message carries
// it; every batch response echoes it. The goal is trace continuity:
// a single log line in the wasm worker can be tied back to one
// specific input area on one page in one tab. The background uses
// the same `inputId` as a routing tag when forwarding to the wasm
// worker (so the worker's per-message log line keeps the identity).
//
// `gen` is the content script's monotonic render-generation counter.
// The response echoes it back so a slow batch can tell it has been
// superseded and discard itself. The background also uses it as a cheap
// tiebreaker when more than one batch is in flight at once.
declare type R_CheckBatch = {
  type: 'torille:check-batch';
  inputId: number;
  gen: number;
  items: string[];
};
declare type R_SuggestBatch = {
  type: 'torille:suggest-batch';
  inputId: number;
  gen: number;
  items: { word: string; max: number }[];
};
declare type R_ContextMenuFired = {
  type: 'torille:contextmenu-fired';
  disabled: boolean;
};
declare type R_SiteContextMenuFired = {
  type: 'torille:site-contextmenu-fired';
  disabled: boolean;
};
declare type R_Debug = { type: 'torille:debug'; on: boolean };

// ─── per-textbox state (the analyse_text render path) ────────────────
//
// A per-overlay wasm-side state is created on attach, taught
// proper-noun / phrase candidates from the text on every render,
// and destroyed on detach. The content script owns the lifecycle;
// the background is a transparent forwarder.
//
// `idx` is the wasm-side slot handle. The worker allocates it on
// `create-state` and returns it via `R_CreateStateResponse`. The
// content script keeps it for the lifetime of the overlay.
//
// `R_AnalyzeText` is the new render path: one worker call per
// render, regardless of how many tokens the textarea has. It runs
// the wasm-side tokeniser and first-pass misspell detection, and
// returns hits tagged with UTF-16 string offsets (converted from the
// engine's byte offsets in the worker). The content script uses
// those ranges directly for the highlight.
declare type R_CreateState = { type: 'torille:create-state' };
declare type R_DestroyState = { type: 'torille:destroy-state'; idx: number };
declare type R_ClearState = { type: 'torille:clear-state'; idx: number };
declare type R_UpdateState = {
  type: 'torille:update-state';
  idx: number;
  text: string;
};
declare type R_AnalyzeText = {
  type: 'torille:analyze-text';
  inputId: number;
  gen: number;
  idx: number;
  text: string;
  max: number;
};
declare type R_SuggestWithState = {
  type: 'torille:suggest-with-state';
  idx: number;
  word: string;
  max: number;
};
// Grammar diagnostics. Offsets are UTF-16 string offsets after the
// worker's byte-to-UTF-16 map; the content script slices JS strings.
declare type R_SyntaxEdit = {
  start: number;
  end: number;
  replacement: string;
};
declare type R_SyntaxCorrection = {
  score: number;
  note: string;
  edits: R_SyntaxEdit[];
};
declare type R_SyntaxDiagnostic = {
  kind: string;
  severity: string;
  confidence: string;
  start: number;
  end: number;
  message: string;
  expected: string | null;
  corrections: R_SyntaxCorrection[];
};
declare type R_CheckSyntax = {
  type: 'torille:check-syntax';
  idx: number;
  text: string;
};
declare type R_CheckSyntaxResponse = {
  diagnostics: R_SyntaxDiagnostic[];
};

declare type R_Message =
  | R_Check
  | R_CheckFast
  | R_Suggest
  | R_CheckBatch
  | R_SuggestBatch
  | R_ContextMenuFired
  | R_SiteContextMenuFired
  | R_Debug
  | R_CreateState
  | R_DestroyState
  | R_ClearState
  | R_UpdateState
  | R_AnalyzeText
  | R_SuggestWithState
  | R_CheckSyntax;
// `null` means "the checker could not answer" (worker down, wasm
// failure, timeout). It is deliberately distinct from `false`
// ("misspelled"): only a `false` should produce a red underline.
declare type R_CheckResponse = { ok: boolean | null };
declare type R_CheckFastResponse = { ok: boolean | null };
declare type R_SuggestResponse = { suggestions: string[] };
// Batch responses preserve the order of the caller's `items` array so
// the caller can zip results back without a key map. `gen` lets the
// caller discard stale responses without inspecting them. `inputId`
// is echoed back so the caller can sanity-check that a response
// actually came from one of its own inputs.
declare type R_CheckBatchResponse = {
  inputId: number;
  gen: number;
  ok: (boolean | null)[];
};
declare type R_SuggestBatchResponse = {
  inputId: number;
  gen: number;
  results: string[][];
};
// `analyze-text` returns a `hits` array tagged with UTF-16 string
// offsets so the caller can highlight directly without re-tokenising
// on the JS side. Each hit has the misspelled word's text plus an
// optional suggestion list. The render path leaves the list empty
// and fills it from `suggest-with-state` on click.
declare type R_AnalyzeHit = {
  word: string;
  start: number;
  end: number;
  suggestions: string[];
};
declare type R_CreateStateResponse = { idx: number };
declare type R_AnalyzeTextResponse = {
  inputId: number;
  gen: number;
  hits: R_AnalyzeHit[];
  /** False when the wasm worker no longer has this textarea's slot. */
  stateValid?: boolean;
};
declare type R_SuggestWithStateResponse = { suggestions: string[] };
declare type R_ContentMessageResponse =
  | R_CheckResponse
  | R_CheckFastResponse
  | R_SuggestResponse
  | R_CheckBatchResponse
  | R_SuggestBatchResponse
  | R_CreateStateResponse
  | R_AnalyzeTextResponse
  | R_SuggestWithStateResponse;
declare type R_TogglePendingInputMessage = {
  type: 'torille:toggle-pending-input';
};
declare type R_CheckGrammarPendingInputMessage = {
  type: 'torille:check-grammar-pending-input';
};
declare type R_ContentInboundMessage =
  | R_TogglePendingInputMessage
  | R_CheckGrammarPendingInputMessage;
declare type R_ToggleResponse = { disabled: boolean };
declare type R_CheckGrammarResponse = { checked: boolean };
// Worker-level message protocol. The wasm worker is loaded once per
// browser session in the background; the same wire is shared by
// every overlay's content script. `R_CreateState` /
// `R_UpdateState` / `R_AnalyzeText` / `R_SuggestWithState` are
// the per-textbox render path; the others remain for the popup's
// per-click suggest and any callers that haven't migrated yet.
declare type R_WorkerInbound =
  | { id: number; type: 'check'; word: string }
  | { id: number; type: 'check-fast'; idx: number; word: string }
  | { id: number; type: 'suggest'; word: string; max: number }
  | { id: number; type: 'check-batch'; inputId: number; words: string[] }
  | { id: number; type: 'suggest-batch';
      inputId: number; items: { word: string; max: number }[] }
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
    }
  | { id: number; type: 'check-syntax'; idx: number; text: string }
  | { type: 'debug'; on: boolean };
declare type R_WorkerOutbound =
  | { type: 'ready' }
  | { type: 'init_failed'; error: string }
  | { id: number; type: 'check'; ok: boolean }
  | { id: number; type: 'check-fast'; ok: boolean | null }
  | { id: number; type: 'suggest'; suggestions: string[] }
  | { id: number; type: 'check-batch'; inputId: number; ok: (boolean | null)[] }
  | { id: number; type: 'suggest-batch';
      inputId: number; results: string[][] }
  | { id: number; type: 'create-state'; idx: number }
  | {
      id: number;
      type: 'analyze-text';
      hits: { word: string; start: number; end: number; suggestions: string[] }[];
    }
  | { id: number; type: 'suggest-with-state'; suggestions: string[] }
  | { id: number; type: 'check-syntax'; diagnostics: R_SyntaxDiagnostic[] }
  | { id: number; type: 'error'; for: string; word?: string; error: string };
