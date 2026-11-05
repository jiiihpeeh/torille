//! The `DER1` sorted key/value table writer.
//!
//! Layout (little-endian), matching the reader in `src/expand/blob.rs`:
//!
//! ```text
//! "DER1" | u32 count | u32 keys_len | u32 vals_len
//! | u32 key_offsets[count + 1]
//! | u32 value_offsets[count + 1]   (only when vals_len > 0)
//! | keys bytes | vals bytes
//! ```
//!
//! Offsets are relative to their own byte pool. A set has `vals_len == 0` and
//! no value table. Rows must already be sorted by key; the reader binary
//! searches the offsets.

use std::path::Path;

/// Write `rows` as one `DER1` table at `path`.
///
/// `rows` must be sorted by key and keys must be unique. A `None` value makes
/// the row a set member; if every value is `None` the blob omits the value
/// table entirely.
pub fn write_table<'a>(path: &Path, rows: impl Iterator<Item = (&'a str, Option<&'a str>)>) {
    let rows: Vec<(&str, Option<&str>)> = rows.collect();
    let keys_len: usize = rows.iter().map(|(key, _)| key.len()).sum();
    let vals_len: usize = rows
        .iter()
        .map(|(_, value)| value.map_or(0, str::len))
        .sum();
    let mut out = Vec::with_capacity(16 + (rows.len() + 1) * 8 + keys_len + vals_len);
    out.extend_from_slice(b"DER1");
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&(keys_len as u32).to_le_bytes());
    out.extend_from_slice(&(vals_len as u32).to_le_bytes());

    let mut at = 0u32;
    out.extend_from_slice(&at.to_le_bytes());
    for (key, _) in &rows {
        at += key.len() as u32;
        out.extend_from_slice(&at.to_le_bytes());
    }
    if vals_len > 0 {
        let mut at = 0u32;
        out.extend_from_slice(&at.to_le_bytes());
        for (_, value) in &rows {
            at += value.map_or(0, |v| v.len()) as u32;
            out.extend_from_slice(&at.to_le_bytes());
        }
    }
    for (key, _) in &rows {
        out.extend_from_slice(key.as_bytes());
    }
    for (_, value) in &rows {
        if let Some(value) = value {
            out.extend_from_slice(value.as_bytes());
        }
    }
    std::fs::write(path, &out).unwrap_or_else(|exc| panic!("{}: {exc}", path.display()));
}
