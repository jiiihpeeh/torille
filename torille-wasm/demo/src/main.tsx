// Bootstrap: render a Loading splash immediately, then swap to App
// once the wasm is ready. We keep the splash's disposer so App can
// remove it; otherwise both roots race for the same #app element and
// the user sees the splash stuck above the editor forever.

import { render } from 'solid-js/web';
import { App } from './App';
import { ready, mountReadySignal } from './spell-client';

// Mount the wasm-ready promise on `window.__torille.ready`
// before the worker boot so the probe can `await` it
// without an import.
mountReadySignal();

function Loading() {
  return (
    <>
      <h1>
        torille-wasm demo <small>loading wasm…</small>
      </h1>
      <p>
        starting spell-check worker (10.5 MB wasm) — first load may take a moment.
      </p>
    </>
  );
}

const disposeLoading = render(() => <Loading />, document.getElementById('app')!);

// Spawn the worker and wait for its wasm to be ready before
// rendering the editor. If the worker fails to start (older browser,
// CSP, etc.) we leave the error on screen.
ready()
  .then(() => {
    disposeLoading();
    render(() => <App />, document.getElementById('app')!);
  })
  .catch((e) => {
    disposeLoading();
    document.getElementById('app')!.textContent =
      'spell worker failed to start: ' + (e as Error).message;
  });