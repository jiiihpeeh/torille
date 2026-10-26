//! Shared readers for the embedded binary maps.
//!
//! `data/precomputed/` holds two blob shapes, both embedded with
//! `include_bytes!` and read in place (no parse, no startup allocation):
//!
//! * the **sanalista** blob (`sanalista.bin`): a 48-byte header of explicit
//!   section offsets, 8-byte per-row records, and `u16`/`u32` little-endian
//!   fields;
//! * the **front-coded maps** (`stem_map_*.bin`, `all_forms.bin`,
//!   `connegatives.bin`, ...): a 40-byte header, a restart table, and sorted
//!   keys front-coded as `varint(shared_prefix) + varint(suffix_len) +
//!   suffix`.
//!
//! This module is the single place that knows the endianness helpers and the
//! front-coded binary search. The domain modules (`kotus`,
//! `expand::static_table`) keep the section semantics.

/// Records per front-coding block: one full key, then `RESTART - 1`
/// prefix-diffed ones. Smaller = faster random access, larger = better
/// compression.
pub(crate) const RESTART: usize = 16;

/// Header size of a front-coded map/set blob.
pub(crate) const MAP_HEADER: usize = 40;

/// Read a little-endian `u16` at `offset`.
#[inline]
pub(crate) fn read_u16(blob: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([blob[offset], blob[offset + 1]])
}

/// Read a little-endian `u32` at `offset`.
#[inline]
pub(crate) fn read_u32(blob: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        blob[offset],
        blob[offset + 1],
        blob[offset + 2],
        blob[offset + 3],
    ])
}

/// A section start stored as a `u32` at header byte `field`.
#[inline]
pub(crate) fn section(blob: &[u8], field: usize) -> usize {
    read_u32(blob, field) as usize
}

/// The front-coded key region shared by [`crate::expand::static_table::StemMap`]
/// and [`crate::expand::static_table::StaticSet`].
///
/// The header layout it reads:
///
/// ```text
///   4  u32 key_count
///   8  u32 restart_count
///  12  u32 restarts_off (= MAP_HEADER)
///  16  u32 keys_off
///  40  restarts: restart_count x u32 offsets into the keys region
///      keys:     front-coded records
/// ```
pub(crate) struct FrontCodedKeys {
    blob: &'static [u8],
}

impl FrontCodedKeys {
    pub(crate) const fn new(blob: &'static [u8]) -> Self {
        Self { blob }
    }

    /// The whole blob, for the owner's value/string sections.
    pub(crate) fn blob(&self) -> &'static [u8] {
        self.blob
    }

    pub(crate) fn key_count(&self) -> usize {
        read_u32(self.blob, 4) as usize
    }

    #[cfg(test)]
    pub(crate) fn restart_count(&self) -> usize {
        read_u32(self.blob, 8) as usize
    }

    fn keys_off(&self) -> usize {
        read_u32(self.blob, 16) as usize
    }

    /// Offset of front-coding block `r`'s first record inside the keys
    /// region.
    fn restart(&self, r: usize) -> usize {
        read_u32(self.blob, MAP_HEADER + r * 4) as usize
    }

    /// Read the LEB128 varint at `keys_off + off`; returns `(value,
    /// next_offset)`.
    fn varint(&self, off: usize) -> (u32, usize) {
        let keys = self.keys_off();
        let mut result = 0u32;
        let mut shift = 0;
        let mut p = keys + off;
        loop {
            let b = self.blob[p];
            p += 1;
            result |= ((b & 0x7f) as u32) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        (result, p - keys)
    }

    /// Decode key `i` into `buf` and return it as bytes (always UTF-8).
    ///
    /// Decodes from the nearest restart, at most `RESTART` records.
    fn key_at<'a>(&self, i: usize, buf: &'a mut Vec<u8>) -> &'a [u8] {
        let keys = self.keys_off();
        let block = i / RESTART;
        let mut off = self.restart(block);
        buf.clear();
        let mut j = block * RESTART;
        loop {
            let (common, p1) = self.varint(off);
            let (slen, p2) = self.varint(p1);
            let start = keys + p2;
            let end = start + slen as usize;
            buf.truncate(common as usize);
            buf.extend_from_slice(&self.blob[start..end]);
            off = p2 + slen as usize;
            if j == i {
                return buf.as_slice();
            }
            j += 1;
        }
    }

    /// The key ordinal for `key`, or `None`. Binary search over the
    /// front-coded keys.
    pub(crate) fn find(&self, key: &str) -> Option<usize> {
        let target = key.as_bytes();
        let mut buf = Vec::new();
        let (mut lo, mut hi) = (0usize, self.key_count());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.key_at(mid, &mut buf).cmp(target) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Some(mid),
            }
        }
        None
    }
}
