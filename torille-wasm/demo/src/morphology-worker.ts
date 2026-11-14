// Morphology pool worker.
//
// One wasm instance and one textarea state per worker. The state's
// `MorphologyCache` persists across groups, so a lemma resolved for one group
// is a hit for the next. The worker only resolves surfaces; the owning
// spell worker runs the full-document syntax/style pass after it collects the
// encoded rows.

import { bg, initTorilleWasm } from './wasm-loader';
import type { R_MorphologyInbound, R_MorphologyOutbound } from '../../protocol.js';

const workerSelf = self as unknown as {
  postMessage(m: unknown, transfer?: Transferable[]): void;
  addEventListener(type: string, listener: (e: MessageEvent) => void): void;
};

const post = (message: R_MorphologyOutbound, transfer?: Transferable[]): void => {
  workerSelf.postMessage(message, transfer);
};

const wasmUrl = new URL('./torille_wasm_bg.wasm', import.meta.url);
try {
  await initTorilleWasm(wasmUrl.toString());
} catch (error) {
  post({ type: 'init_failed', error: String((error as Error)?.message ?? error) });
  throw error;
}

const state = bg.create_text_area_state();
post({ type: 'ready' });

workerSelf.addEventListener('message', (event: MessageEvent) => {
  const message = event.data as R_MorphologyInbound;
  if (message.type !== 'morphology-chunk') return;
  try {
    const payload = bg.morphology_chunk(state, message.text);
    post({ id: message.id, type: 'morphology-chunk', payload }, [payload.buffer]);
  } catch (error) {
    post({
      id: message.id,
      type: 'error',
      for: 'morphology-chunk',
      error: String((error as Error)?.message ?? error),
    });
  }
});
