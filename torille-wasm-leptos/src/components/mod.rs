//! Shared Leptos components.
//!
//! The editor and popup mirror what the SolidJS demo renders
//! -- red wavy underlines for misspellings + a click-to-replace
//! suggestion popup -- but in Leptos idioms (`view!`, signals,
//! event handlers). The wasm side has the byte ranges, so we
//! use those directly to position the underlay spans without
//! needing a JS-side tokenise.

pub mod editor;
pub mod popup;
pub mod syntax;
