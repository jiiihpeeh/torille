// Morphology pool worker for the Firefox extension.
//
// The background creates up to four of these when grammar/style runs. Each
// owns a wasm instance and one textarea state; the state's morphology cache
// carries across the surface groups it resolves. The background collects the
// encoded rows and sends them to the primary spell worker, which runs the
// full-document syntax/style pass once.

import { bg, initTorilleWasm } from './wasm-loader.js';

type MorphologyInbound = {
  id: number;
  type: 'morphology-chunk';
  text: string;
};

type MorphologyOutbound =
  | { type: 'ready' }
  | { type: 'init_failed'; error: string }
  | { id: number; type: 'morphology-chunk'; payload: Uint8Array }
  | { id: number; type: 'error'; for: string; error: string };

const workerSelf = self as unknown as {
  postMessage(message: MorphologyOutbound, transfer?: Transferable[]): void;
  addEventListener(type: string, listener: (event: MessageEvent) => void): void;
};

function post(message: MorphologyOutbound, transfer?: Transferable[]): void {
  workerSelf.postMessage(message, transfer);
}

const wasmUrl = new URL('./torille_wasm_bg.wasm', import.meta.url);
try {
  await initTorilleWasm(wasmUrl.toString());
} catch (error) {
  post({
    type: 'init_failed',
    error: String((error as Error)?.message ?? error),
  });
  throw error;
}

const state = bg.create_text_area_state();
post({ type: 'ready' });

workerSelf.addEventListener('message', (event: MessageEvent) => {
  const message = event.data as MorphologyInbound;
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
