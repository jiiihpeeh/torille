import { api, type DebugFn } from './shared';

// One module worker per page. It hosts the wasm and answers
// check/suggest requests tagged with a numeric id so we can match
// responses to callers.
// MV3 worker plumbing: the wasm lives in the background script (which
// is in the extension's own origin and *can* construct Workers against
// `chrome-extension://` URLs -- content scripts can't, because they
// share the page's origin). The content script asks the background for
// `check` / `suggest` via `runtime.sendMessage`. The wasm worker is
// shared across all tabs, so the ~150 ms boot cost is paid once.
//
// `inputId` is a small monotonic number assigned per textarea /
// contenteditable on this page. It is auto-injected into every
// batch payload (and verified on the response) so the renderer
// never accidentally accepts a response meant for a different
// input. Single-word ops don't carry the id -- they're used by the
// per-click popup flow, which only asks for one word and never
// races with another overlay.
export function createWorkerClient(opts: { dbg: DebugFn; inputId: number }) {
  const o = opts;
  const { inputId } = o;

  function send<R>(payload: unknown): Promise<R | null> {
    return new Promise((resolve) => {
      try {
        const p = api.runtime.sendMessage(payload);
        if (p && typeof p.then === 'function') {
          p.then(resolve, (e: unknown) => {
            o.dbg('content', 'sendMessage rejected',
                  { inputId, err: String(e) });
            resolve(null);
          });
        } else {
          // Older Firefox (MV2) returns `undefined` from sendMessage
          // when the response isn't a promise; fall back to a default.
          resolve(null);
        }
      } catch (e) {
        o.dbg('content', 'sendMessage threw', { inputId, err: String(e) });
        resolve(null);
      }
    });
  }

  // One wasm-side slot for this overlay's lifetime. We
  // self-manage the index here so renderers and the popup's
  // suggest path share the same proper-noun set without having
  // to thread the idx through their signatures.
  //
  // `stateIdxP` is a memoised Promise so concurrent callers
  // awaiting the same idx don't fire two `create-state` round
  // trips. On failure (worker down) the cached promise stays
  // resolved to `null` and subsequent calls keep that null.
  let stateIdxP: Promise<number | null> | null = null;
  function stateIdx(): Promise<number | null> {
    if (stateIdxP === null) {
      stateIdxP = (async (): Promise<number | null> => {
        const r = await send<R_CreateStateResponse>({
          type: 'torille:create-state',
        });
        return typeof r?.idx === 'number' ? r.idx : null;
      })();
    }
    return stateIdxP;
  }

  // Fire-and-forget: when the overlay dies, the slot is freed.
  // The scanner doesn't expose a detach hook today; the slot
  // therefore stays alive until the worker is terminated (tab
  // close), bounded by the number of textareas seen in a
  // session.
  function tearDownStateFn(): void {
    stateIdx().then((idx) => {
      if (idx !== null) {
        api.runtime.sendMessage({
          type: 'torille:destroy-state', idx,
        } as R_DestroyState).catch(() => {});
      }
    }).catch(() => {});
  }

  return {
    inputId,
    async checkAsync(word: string): Promise<boolean | null> {
      const r = await send<R_CheckResponse>({ type: 'torille:check', word });
      // `null` = unavailable. Do not turn a dead worker into a
      // spelling verdict.
      return r && typeof r.ok === 'boolean' ? r.ok : null;
    },
    async checkFastAsync(word: string): Promise<boolean | null> {
      // First-pass verdict against this overlay's state (~1 ms).
      // Used by the contenteditable popup instead of `checkAsync`,
      // whose engine fallback can block the worker for seconds.
      const idx = await stateIdx();
      if (idx === null) return null;
      const r = await send<R_CheckFastResponse>({
        type: 'torille:check-fast', idx, word,
      });
      return r && typeof r.ok === 'boolean' ? r.ok : null;
    },
    // Batch counterpart of `checkAsync`. One runtime.sendMessage per
    // render instead of one per word. The result array is parallel
    // to the input; the caller zips it back to its own state. `gen`
    // is the caller's render generation; the response echoes it so
    // a slow batch can be discarded by the caller without using the
    // values.
    //
    // The `inputId` is injected here so every overlay's worker
    // client stamps its own identity on outgoing batch traffic
    // without the renderer having to remember. A response that
    // comes back with a different `inputId` is treated as a routing
    // error and discarded.
    async checkBatchAsync(
      words: string[], gen: number,
    ): Promise<(boolean | null)[] | null> {
      if (words.length === 0) return [];
      const r = await send<R_CheckBatchResponse>({
        type: 'torille:check-batch', inputId, gen, items: words,
      });
      if (!r || !Array.isArray(r.ok)) return null;
      if (r.inputId !== inputId) {
        o.dbg('content', 'checkBatchAsync:inputId-mismatch',
              { want: inputId, got: r.inputId });
        return null;
      }
      return r.ok;
    },
    async suggestAsync(word: string, max: number): Promise<string[]> {
      const r = await send<R_SuggestResponse>({ type: 'torille:suggest', word, max });
      return Array.isArray(r?.suggestions) ? r.suggestions : [];
    },
    // Batch counterpart of `suggestAsync`. Used by the render layer
    // if it wants to fetch suggestions for several misspellings in
    // one go (e.g. on a contenteditable refresh). The popup's per-
    // click flow still uses `suggestAsync` since it only ever asks
    // for one word.
    async suggestBatchAsync(
      items: { word: string; max: number }[], gen: number,
    ): Promise<string[][] | null> {
      if (items.length === 0) return [];
      const r = await send<R_SuggestBatchResponse>({
        type: 'torille:suggest-batch', inputId, gen, items,
      });
      if (!r || !Array.isArray(r.results)) return null;
      if (r.inputId !== inputId) {
        o.dbg('content', 'suggestBatchAsync:inputId-mismatch',
              { want: inputId, got: r.inputId });
        return null;
      }
      return r.results;
    },
    // ─── per-textbox state lifecycle ────────────────────────────
    //
    // The render path. `createStateAsync` returns the wasm-side
    // slot index the content script holds for the lifetime of
    // this overlay. `destroyStateAsync` on detach returns the
    // slot to the pool. Every render fires `updateState` then
    // `analyzeText` -- one wasm round-trip per render regardless
    // of token count.
    async createStateAsync(): Promise<number | null> {
      return stateIdx();
    },
    destroyStateAsync(idx: number): void {
      // Fire-and-forget; there's no reply envelope.
      api.runtime.sendMessage({
        type: 'torille:destroy-state', idx,
      } as R_DestroyState).catch(() => { /* background down -- ignore */ });
    },
    clearStateAsync(idx: number): void {
      api.runtime.sendMessage({
        type: 'torille:clear-state', idx,
      } as R_ClearState).catch(() => {});
    },
    updateStateAsync(idx: number, text: string): void {
      api.runtime.sendMessage({
        type: 'torille:update-state', idx, text,
      } as R_UpdateState).catch(() => {});
    },
    async analyzeTextAsync(
      text: string, max: number, gen: number,
    ): Promise<R_AnalyzeHit[] | null> {
      const ask = async (idx: number): Promise<R_AnalyzeTextResponse | null> => {
        // The proper-noun update is fired ahead of analyze so the
        // wasm has the latest text in hand. analyze-text awaits in
        // the same render, so the renderer's wall-clock cost is
        // still one round-trip.
        api.runtime.sendMessage({
          type: 'torille:update-state', idx, text,
        } as R_UpdateState).catch(() => {});
        return send<R_AnalyzeTextResponse>({
          type: 'torille:analyze-text', inputId, gen, idx, text, max,
        });
      };

      let idx = await stateIdx();
      if (idx === null) return null;
      let r = await ask(idx);
      if (r && r.inputId === inputId && r.stateValid === false) {
        // The wasm worker restarted and lost every per-textbox slot.
        // Allocate a fresh one and retry once; otherwise the render
        // sees an empty hit list and the underlines silently vanish.
        o.dbg('content', 'state stale; recreating', { inputId, idx });
        stateIdxP = null;
        idx = await stateIdx();
        if (idx === null) return null;
        r = await ask(idx);
      }
      if (!r || !Array.isArray(r.hits)) return null;
      if (r.inputId !== inputId) return null;
      return r.hits;
    },
    // Popup's per-click suggest. Uses the per-textbox proper-noun
    // set so words the user has typed before surface as
    // candidates when the popup shows up.
    async suggestWithStateAsync(
      word: string, max: number,
    ): Promise<string[]> {
      const idx = await stateIdx();
      if (idx === null) return [];
      const r = await send<R_SuggestWithStateResponse>({
        type: 'torille:suggest-with-state', idx, word, max,
      });
      return Array.isArray(r?.suggestions) ? r.suggestions : [];
    },
    // On-demand grammar check for the context menu. The worker
    // converts byte offsets to UTF-16 before returning.
    async checkSyntaxAsync(text: string): Promise<R_SyntaxDiagnostic[] | null> {
      const idx = await stateIdx();
      if (idx === null) return null;
      const r = await send<R_CheckSyntaxResponse>({
        type: 'torille:check-syntax', idx, inputId, text,
      });
      return Array.isArray(r?.diagnostics) ? r.diagnostics : null;
    },
    // On-demand style check for the separate Tyyli command. It uses the
    // overlay's per-textarea state so the combined export can share the
    // morphology cache with the grammar pass.
    async checkStyleAsync(text: string): Promise<R_SyntaxDiagnostic[] | null> {
      const idx = await stateIdx();
      if (idx === null) return null;
      const r = await send<R_CheckStyleResponse>({
        type: 'torille:check-style', idx, inputId, text,
      });
      return Array.isArray(r?.diagnostics) ? r.diagnostics : null;
    },
    // Stop the background's batch loop for this input. Called on every
    // edit so a long check does not keep the worker busy after its
    // offsets have gone stale.
    cancelCheckAsync(): void {
      api.runtime.sendMessage({
        type: 'torille:cancel-check', inputId,
      } as R_CancelCheck).catch(() => { /* background down */ });
    },
    // Free the wasm-side slot. Called from the scanner's
    // detach path so a per-overlay slot doesn't outlive the
    // overlay.
    tearDownState(): void { tearDownStateFn(); },
    setDebug(on: boolean): void {
      // Fire-and-forget; the background's worker picks up the flag
      // immediately.
      const m: R_Debug = { type: 'torille:debug', on };
      api.runtime.sendMessage(m);
    },
  };
}

export type WorkerClient = ReturnType<typeof createWorkerClient>;
