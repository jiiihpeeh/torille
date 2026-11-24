// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cross-platform access to the orphan `maps` branch.
//!
//! The implementation lives in `build/maps_sync.rs` so the engine's build
//! script can compile the same file directly: Cargo never packages a nested
//! package, and the published `torille` crate must not depend on a path-only
//! helper. This crate is the thin CLI wrapper (`status`, `fetch`, `publish`).
//!
//! Everything is `std::process::Command` against `git`, with paths and
//! environment built in Rust. There is no shell, no `tar`, no globbing and no
//! platform-specific script.

#[path = "../../../build/maps_sync.rs"]
mod implementation;

pub use implementation::*;
