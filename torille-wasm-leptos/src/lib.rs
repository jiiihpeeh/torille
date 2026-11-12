//! Browser-driven Leptos demo of the torille Finnish spell-checker.
//!
//! One wasm binary, no worker, no postMessage. The `torille-wasm`
//! crate is a normal Rust dependency, so the render layer calls
//! [`torille_wasm::wasm_demo`] directly. State lives at the same
//! Rust ownership level as the UI; Leptos signals coordinate the
//! reactive updates between the input event and the analysis.
//!
//! ## File layout
//!
//!   `lib.rs` -- the entry point. `#[wasm_bindgen(start)]` mounts
//!   the app once the wasm is loaded, and re-exports the
//!   `SpellState` helper so the components below can `use`
//!   everything they need.
//!
//!   `app.rs` -- the `App` component. Owns the per-textarea state,
//!   two signals (the input text + the latest analyse result),
//!   and the input/overlay rendering.
//!
//!   `components/editor.rsx` -- the textarea + mirror overlay
//!   (shared between this demo and `torille-wasm/demo` so the
//!   visual layout stays in sync).
//!
//! ## Build
//!
//!   `wasm-pack build --release --target web` from
//!   `torille-wasm-leptos/` produces `pkg/torille_wasm_leptos_bg.wasm`
//!   plus the ESM shim. A static `index.html` next to the build
//!   output (see `examples/index.html`) boots the wasm and
//!   triggers the mount via the `start` export below.

use wasm_bindgen::prelude::*;

mod app;
mod components;

pub use app::App;
pub use components::editor::Editor;
pub use components::popup::SuggestionsPopup;
pub use components::syntax::SyntaxPanel;

/// Mount the `App` component to `<body>`. The default `index.html`
/// we ship sets the wasm-bindgen `start()` export to point at
/// `start` so the moment the wasm finishes loading, the demo is
/// up.
#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(target_arch = "wasm32")]
    {
        // Make `unwrap` panics land in the browser dev console
        // instead of dying silently.
        console_error_panic_hook::set_once();

        leptos::mount::mount_to_body(App);
    }
}
