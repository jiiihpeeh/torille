// SPDX-License-Identifier: MIT OR Apache-2.0
//! Write a `maps/*.bin.gz` blob at maximum gzip compression.
//!
//! The engine decompresses these once on first access (`src/map_blob.rs`), so
//! the tools that compute a map with the engine (`gen_derived`) must write the
//! same container as the build script. `map_gz` can decompress or recompress
//! one for inspection.

use std::io::Write;
use std::path::Path;

/// Gzip `data` into `path` at the maximum level.
pub fn write(path: &Path, data: &[u8]) {
    let file = std::fs::File::create(path)
        .unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    encoder
        .write_all(data)
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    encoder
        .finish()
        .unwrap_or_else(|e| panic!("finish {}: {e}", path.display()));
    eprintln!(
        "gzip: {} bytes -> {} ({} bytes)",
        data.len(),
        path.display(),
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    );
}

/// Read and decompress a gzip file back to memory.
pub fn read(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(&bytes[..])
        .read_to_end(&mut out)
        .unwrap_or_else(|e| panic!("gunzip {}: {e}", path.display()));
    out
}
