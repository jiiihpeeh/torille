// Module worker entry point.
//
// The actual spell API lives in `spell-core.ts` so the same code can
// run here (Firefox, whose MV3 background is an event page and can
// create workers) and directly inside Chromium's MV3 service worker
// (which cannot create nested workers). This file is only the
// postMessage host: it adds the request id to each response and turns
// a thrown error into an `error` frame.

import { createSpellCore } from './spell-core.js';

const core = createSpellCore();

core.init().then(
  () => { self.postMessage({ type: 'ready' }); },
  (err: unknown) => {
    self.postMessage({ type: 'init_failed', error: String(err) });
  },
);

self.onmessage = async (e: MessageEvent<unknown>): Promise<void> => {
  const msg = e.data as R_WorkerInbound;
  const id = (msg as { id?: number }).id ?? -1;
  const op = (msg as { type?: string }).type ?? 'unknown';
  const word = (msg as { word?: string }).word;
  try {
    const out = await core.handle(msg);
    if (out !== null) self.postMessage({ id, ...out });
  } catch (err) {
    // A thrown check/suggest is a transport failure, not a spelling
    // verdict. The background turns this into `ok: null` /
    // `suggestions: []` and the content script leaves the word
    // unmarked rather than underlining it as wrong.
    self.postMessage({
      type: 'error', id, for: op, word, error: String(err),
    });
  }
};
