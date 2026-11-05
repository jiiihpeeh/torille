//! Writers for the compact set/map blobs the engine binary-searches at
//! run time.
//!
//! * [`front_coded`] writes front-coded sets and maps (`all_forms.bin`,
//!   `stem_map_*.bin`, `proper_names.bin`, ...).
//! * [`huffman`] writes the `HST1` front-coded set used for proper names.
//!
//! All of them share the `write_varint` / `human_bytes` helpers in the
//! crate root (`build.rs`).

pub mod front_coded;
pub mod huffman;
