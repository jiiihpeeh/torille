//! Writers that turn the committed source data into the embedded binary
//! blobs. Every module here reads a file (CSV, TSV or a raw text grid) and
//! writes a compact `data/precomputed/*.bin`.
//!
//! The blob formats are read back by the engine: see `src/map_reader.rs`,
//! `src/expand/static_table.rs` and `src/spec.rs`. Keep this tree free of
//! engine dependencies -- a build script cannot link the crate it builds.
//!
//! `human_bytes` and `write_varint` live in the crate root (`build.rs`)
//! because every writer needs them; the submodules import them with
//! `crate::{human_bytes, write_varint}`.

pub mod catalogue;
pub mod sanalista;
pub mod string_map;
