//! Compress committed `maps/*.bin.gz` blobs at the maximum gzip level.
//!
//! This is **tooling-only** compression: the repository and the generated
//! maps stay small, but the compiler still embeds plain bytes. Each writer
//! puts the uncompressed blob in `OUT_DIR` (what `include_bytes!` reads) and
//! this module gzips the committed copy under `maps/`. For the maps computed
//! by `torille-tools` rather than by this script, the direction is reversed:
//! the committed `.bin.gz` is decompressed into `OUT_DIR` before compiling.

use std::io::{Read, Write};
use std::path::Path;

/// Gzip `data` into `path` at the maximum level.
pub(crate) fn write(path: &Path, data: &[u8]) {
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
        "gzip: {} -> {} ({} bytes)",
        path.display(),
        data.len(),
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    );
}

/// Compress the plain file at `raw` into the committed `gz`.
pub(crate) fn compress_file(raw: &Path, gz: &Path) {
    let data = std::fs::read(raw).unwrap_or_else(|e| panic!("read {}: {e}", raw.display()));
    write(gz, &data);
}

/// Decompress the committed `gz` into the plain file at `raw` (in `OUT_DIR`).
///
/// Used for the maps that `gen_derived` and the oracle script compute with the
/// engine: the build script cannot recompute them, so it unpacks the committed
/// compressed copy for `include_bytes!`.
pub(crate) fn decompress_file(gz: &Path, raw: &Path) {
    let compressed =
        std::fs::read(gz).unwrap_or_else(|e| panic!("read {}: {e}", gz.display()));
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(&compressed[..])
        .read_to_end(&mut out)
        .unwrap_or_else(|e| panic!("gunzip {}: {e}", gz.display()));
    std::fs::write(raw, &out).unwrap_or_else(|e| panic!("write {}: {e}", raw.display()));
    eprintln!(
        "gunzip: {} -> {} ({} bytes)",
        gz.display(),
        raw.display(),
        out.len(),
    );
}
