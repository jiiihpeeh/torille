// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sorted, `include_bytes!`'d key/value tables.
//!
//! The derived and pronoun tables are computed with the engine, which a build
//! script cannot call, so they are written by the `gen_derived` tool and
//! committed under `maps/`. At run time the bytes are the embedded
//! blob and a lookup is a binary search over the key offsets; nothing is
//! parsed or allocated.
//!
//! The layout is little-endian:
//!
//! ```text
//! "DER1" | u32 count | u32 keys_len | u32 vals_len
//! | u32 key_offsets[count + 1]
//! | u32 value_offsets[count + 1]   (only when vals_len > 0)
//! | keys bytes | vals bytes
//! ```
//!
//! Offsets are relative to their own byte pool. A set has `vals_len == 0` and
//! no value table.

use std::sync::OnceLock;

/// A loaded [`Blob`], cached in a [`OnceLock`].
pub(crate) struct Blob {
    bytes: &'static [u8],
    count: usize,
    has_values: bool,
    values_at: usize,
    keys_start: usize,
    vals_start: usize,
}

impl Blob {
    pub(crate) fn load(bytes: &'static [u8]) -> Self {
        assert!(
            bytes.len() >= 16 && &bytes[..4] == b"DER1",
            "blob: bad header"
        );
        let count = le_u32(bytes, 4) as usize;
        let keys_len = le_u32(bytes, 8) as usize;
        let vals_len = le_u32(bytes, 12) as usize;
        let has_values = vals_len > 0;
        let values_at = if has_values { 16 + (count + 1) * 4 } else { 0 };
        let keys_start = 16 + (count + 1) * 4 + if has_values { (count + 1) * 4 } else { 0 };
        Blob {
            bytes,
            count,
            has_values,
            values_at,
            keys_start,
            vals_start: keys_start + keys_len,
        }
    }

    fn key(&self, i: usize) -> &'static str {
        let start = le_u32(self.bytes, 16 + i * 4) as usize;
        let end = le_u32(self.bytes, 16 + (i + 1) * 4) as usize;
        std::str::from_utf8(&self.bytes[self.keys_start + start..self.keys_start + end])
            .expect("blob: key is not utf-8")
    }

    pub(crate) fn value(&self, i: usize) -> &'static str {
        if !self.has_values {
            return "";
        }
        let at = self.values_at + i * 4;
        let start = le_u32(self.bytes, at) as usize;
        let end = le_u32(self.bytes, at + 4) as usize;
        std::str::from_utf8(&self.bytes[self.vals_start + start..self.vals_start + end])
            .expect("blob: value is not utf-8")
    }

    pub(crate) fn find(&self, word: &str) -> Option<usize> {
        let (mut lo, mut hi) = (0usize, self.count);
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.key(mid).cmp(word) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Some(mid),
            }
        }
        None
    }

    pub(crate) fn iter(&'static self) -> impl Iterator<Item = (&'static str, &'static str)> {
        (0..self.count).map(move |i| (self.key(i), self.value(i)))
    }
}

/// Loads the blob in `slot` on first use.
pub(crate) fn cell(slot: &'static OnceLock<Blob>, bytes: &'static [u8]) -> &'static Blob {
    slot.get_or_init(|| Blob::load(bytes))
}

fn le_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(
        bytes[at..at + 4]
            .try_into()
            .expect("blob: shorter than its header"),
    )
}
