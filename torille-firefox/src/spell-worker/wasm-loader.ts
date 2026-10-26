// Manual WASM loader for the extension.
//
// The pkg in `torille-wasm/pkg` is a wasm-bindgen *bundler-target*
// build: `torille_wasm.js` imports the `.wasm` as an ES module, which
// browsers refuse unless the response has `Content-Type:
// application/wasm` (the extension's `moz-extension://` /
// `chrome-extension://` fetch does not), and which also drags in a
// sibling `torille_wasm_bg.js` the old build script never copied.
//
// The demo solved this the same way (`torille-wasm/demo/src/
// wasm-loader.ts`): import the glue directly, fetch the bytes, and
// instantiate by hand. The wasm's import section references the glue
// module by its generated filename, so the import object is that one
// key mapped to the glue namespace -- no enumerating `__wbg_*` by
// hand.
//
// After `__wbg_set_wasm` and `__wbindgen_start`, every export on the
// glue (`check`, `analyze_text`, ...) dispatches into the live
// instance.

import * as glue from './torille_wasm_bg.js';

// The module key baked into the wasm's import section.
const GLUE_MODULE = './torille_wasm_bg.js';

/**
 * Fetch, instantiate, and wire up the torille wasm.
 *
 * Safe to call from a dedicated worker or from Chromium's extension
 * service worker (which cannot create nested workers); both have
 * `fetch` and `WebAssembly`.
 */
export async function initTorilleWasm(url: string): Promise<WebAssembly.Exports> {
  const bytes = await (await fetch(url)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(
    new Uint8Array(bytes),
    { [GLUE_MODULE]: glue },
  );
  glue.__wbg_set_wasm(instance.exports);
  const start = (instance.exports as Record<string, unknown>).__wbindgen_start;
  if (typeof start === 'function') {
    (start as () => void)();
  }
  return instance.exports;
}

export { glue as bg };
