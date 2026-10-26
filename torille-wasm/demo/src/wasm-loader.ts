// Manual WASM loader for the demo.
//
// Why not just `import * as wasm from './torille_wasm_bg.wasm'`?
// That is what the wasm-bindgen shim (`pkg/torille_wasm.js`) does
// itself, and it relies on the browser's WebAssembly ES-module
// integration -- which refuses to instantiate a module served with
// anything other than `Content-Type: application/wasm`. The demo's
// dev workflow falls back to plain `python3 -m http.server`, which
// doesn't set that MIME, so the worker dies at module load.
//
// Why not let the shim load the bytes for us? Older wasm-bindgen
// emitted a `__wbg_init` default export you could call with a
// Uint8Array (skipping the streaming path). The current shim
// (wasm-bindgen 0.2.12x) has no init function at all: importing
// `torille_wasm.js` immediately imports the .wasm as an ES module,
// wires it to the glue via `__wbg_set_wasm`, and runs the start
// function. There is nothing to call -- it has already happened
// (or failed) by the time the module is imported.
//
// So we replicate the shim's two steps by hand, from bytes:
//
//   1. `WebAssembly.instantiate(bytes, imports)` -- MIME-agnostic,
//      no streaming. The wasm's import section references the glue
//      module by its generated filename (`./torille_wasm_bg.js`,
//      the module key `strings` finds inside the .wasm), so the
//      import object is that one key mapped to the glue's exported
//      namespace -- the `__wbg_*` helpers wasm-bindgen generated.
//      We don't enumerate them by hand.
//   2. `__wbg_set_wasm(instance.exports)` hands the instance back
//      to the glue (every exported `bg.check` etc. dispatches
//      through the `wasm` variable it sets), then
//      `__wbindgen_start()` runs the pre-seed pass, exactly as
//      `pkg/torille_wasm.js` would have done after its own import.
//
// The glue module is imported from `pkg/` (bundled by esbuild);
// only the .wasm bytes are fetched at runtime.

import * as glue from '../../pkg/torille_wasm_bg.js';

// The module key baked into the wasm's import section: the wasm is
// compiled against the generated glue file's name.
const GLUE_MODULE = './torille_wasm_bg.js';

/**
 * Fetch, instantiate, and wire up the torille wasm.
 *
 * Returns the wasm exports namespace for callers that need it
 * directly. After this resolves, every other exported function
 * (e.g. `bg.check`, `bg.suggest_words`) is safe to call -- they
 * dispatch through the glue, which has the manually instantiated
 * instance attached via `__wbg_set_wasm`.
 */
export async function initTorilleWasm(url: string): Promise<WebAssembly.Exports> {
  const bytes = await (await fetch(url)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(
    new Uint8Array(bytes),
    { [GLUE_MODULE]: glue },
  );
  glue.__wbg_set_wasm(instance.exports);
  // Same order as the generated wrapper: attach first, then let
  // the start function (which pre-seeds externref slots) see the
  // instance through the glue.
  const start = (instance.exports as Record<string, unknown>).__wbindgen_start;
  if (typeof start === 'function') {
    (start as () => void)();
  }
  return instance.exports;
}

export { glue as bg };
