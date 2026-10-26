# torille-wasm-leptos

A browser-driven Leptos demo of the torille Finnish spell-checker.

Unlike `torille-wasm/demo` and the Firefox / Chromium extensions,
this is **a single wasm** — one `wasm-pack` target that compiles
the Leptos UI and the torille engine into the same linear memory.
The render layer calls `torille_wasm::wasm_demo::*` directly. No
`postMessage`, no worker, no `runtime.sendMessage`.

## Files

```
Cargo.toml                    -- lib-only, wasm32, leptos 0.8
src/lib.rs                    -- #[wasm_bindgen(start)] mounts the App
src/app.rs                    -- the App component (signals + per-textarea state)
src/components/mod.rs         -- shared components module
src/components/editor.rs      -- textarea + underlay
src/components/popup.rs       -- suggestion popup
index.html                    -- boots the wasm and triggers mount
Cargo.lock                    -- *not* committed (single-crate workspace)
```

The `.gitignore` excludes `/pkg/` and `/target/`. Run
`wasm-pack build --release --target web` to produce the
`pkg/torille_wasm_leptos_bg.wasm` and `pkg/torille_wasm_leptos.js`
shim that `index.html` imports.

## Build

```
cd torille-wasm-leptos
wasm-pack build --release --target web
```

Output goes to `./pkg/`. The `index.html` at the crate root
fetches the `.wasm` from `pkg/torille_wasm_leptos_bg.wasm` and
runs `init(bytes)` then `start()`.

## Background

The older demo (`torille-wasm/demo`) and the browser extensions
needed a postMessage + worker boundary because their UI runs in
the browser's main thread while the engine lives in a wasm
worker (one process per browser session). The Leptos demo drops
that boundary: it compiles everything to one wasm, runs `start`
once the wasm finishes loading, and reactive state lives in
Leptos signals rather than via postMessage envelopes. Single-wasm
is browser-friendly, completely client-side, and avoids every IPC
round-trip the older demos were paying.

The architecture is driven by `torille` already being a `no_std`-
friendly Rust core. The extension code paths (`torille-firefox`,
`torille-chromium`, `torille-wasm/demo`) keep their postMessage
protocol because the browser sandbox forces them across origins;
this Leptos deployment is the shape that's idiomatic when you
control deployment.
