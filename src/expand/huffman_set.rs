//! A static-Huffman-coded, block-restarted set of strings.
//!
//! [`crate::expand::static_table::StaticSet`] stores the same kind of data --
//! sorted keys front-coded as `varint(shared_prefix) + varint(suffix_len) +
//! suffix` -- but its bytes are not uniformly distributed: the varint length
//! bytes and the Finnish case endings make a few byte values very common. The
//! 325 k surface forms in `proper_names.bin` spend 1.65 MB that way, and an
//! order-0 entropy code takes the same stream to ~1.0 MB.
//!
//! This is still a **set read in place**: the decoder walks the bitstream with
//! no startup pass, no heap copy and no decompressor crate. The price is that
//! a probe decodes bits instead of reading bytes; the proper-name gate is a
//! fallback after the common-word gates, so that trade is the right one.
//!
//! ## Blob layout (`HST1`)
//!
//! ```text
//!   0  u32 magic            = [`MAGIC`] ("HST1")
//!   4  u32 key_count
//!   8  u32 restart_count    = ceil(key_count / RESTART)
//!  12  u32 restarts_off     = 40
//!  16  u32 table_off        = restarts_off + restart_count * 4
//!  20  u32 bits_off         = table_off + (1 << MAX_CODE_LEN) * 2
//!  24  u32 max_code_len     = [`MAX_CODE_LEN`]
//!  28  ... reserved (three u32)
//!  40  restarts: restart_count x u32 bit offsets into `bits`
//!      table:    2^MAX_CODE_LEN x u16 `(symbol << 4) | code_len`
//!      bits:     canonical-Huffman-coded front-coded key stream
//! ```
//!
//! The front-coded stream is exactly the one `build.rs::emit_front_coded_keys`
//! writes (without the header): every [`RESTART`]-th record is a full key,
//! the rest are prefix diffs. Blocks are bit-aligned rather than byte-aligned,
//! so the restart table stores bit offsets.
//!
//! The code lengths are length-limited to [`MAX_CODE_LEN`] bits (package-merge
//! at build time) so the decode table fits in 8 KB. The table is a direct
//! lookup: peek the next 12 bits, get a symbol and its length, advance.
//!
//! Built by `build.rs::emit_huffman_set`, which round-trips every key through
//! the same decoder before writing the blob.

use crate::map_reader::{RESTART, read_u16, read_u32};

/// Set blob magic (`"HST1"`).
#[cfg(test)]
const MAGIC: u32 = 0x3154_5348;

/// Header size in bytes.
#[cfg(test)]
const HEADER: usize = 40;

/// Maximum Huffman code length, and therefore the width of the decode table.
pub const MAX_CODE_LEN: usize = 12;

/// A sorted set of strings whose front-coded stream is Huffman-coded.
pub struct HuffmanSet {
    blob: &'static [u8],
}

impl HuffmanSet {
    /// Wrap a blob built by `build.rs::emit_huffman_set`.
    pub const fn new(blob: &'static [u8]) -> Self {
        Self { blob }
    }

    fn key_count(&self) -> usize {
        read_u32(self.blob, 4) as usize
    }

    #[cfg(test)]
    fn restart_count(&self) -> usize {
        read_u32(self.blob, 8) as usize
    }

    fn restarts_off(&self) -> usize {
        read_u32(self.blob, 12) as usize
    }

    fn table_off(&self) -> usize {
        read_u32(self.blob, 16) as usize
    }

    fn bits_off(&self) -> usize {
        read_u32(self.blob, 20) as usize
    }

    /// `(symbol, code_len)` for the next code at absolute bit `bit`.
    #[inline]
    fn symbol(&self, bit: &mut usize) -> u8 {
        let byte = *bit >> 3;
        let shift = *bit & 7;
        let at = self.bits_off() + byte;
        // 12 bits starting at `shift` need three bytes (19 bits).
        let b0 = *self.blob.get(at).unwrap_or(&0) as u32;
        let b1 = *self.blob.get(at + 1).unwrap_or(&0) as u32;
        let b2 = *self.blob.get(at + 2).unwrap_or(&0) as u32;
        let window = (b0 << 16) | (b1 << 8) | b2;
        let index = ((window >> (24 - MAX_CODE_LEN - shift)) & 0xFFF) as usize;
        let entry = read_u16(self.blob, self.table_off() + index * 2);
        let len = (entry & 0xF) as usize;
        assert!(len != 0, "corrupt HST1 blob: zero-length code");
        *bit += len;
        (entry >> 4) as u8
    }

    /// Read the LEB128 varint at absolute bit `bit`.
    #[inline]
    fn varint(&self, bit: &mut usize) -> u32 {
        let mut value = 0u32;
        let mut shift = 0;
        loop {
            let byte = self.symbol(bit);
            value |= ((byte & 0x7f) as u32) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    }

    /// Decode key `i` into `buf`; at most `RESTART` records per block.
    fn key_at<'a>(&self, i: usize, buf: &'a mut Vec<u8>) -> &'a [u8] {
        let block = i / RESTART;
        let mut bit = read_u32(self.blob, self.restarts_off() + block * 4) as usize;
        buf.clear();
        let mut j = block * RESTART;
        loop {
            let common = self.varint(&mut bit) as usize;
            let suffix = self.varint(&mut bit) as usize;
            buf.truncate(common);
            for _ in 0..suffix {
                buf.push(self.symbol(&mut bit));
            }
            if j == i {
                return buf.as_slice();
            }
            j += 1;
        }
    }

    /// True if `key` is in the set.
    pub fn contains(&self, key: &str) -> bool {
        let target = key.as_bytes();
        let mut buf = Vec::new();
        let (mut lo, mut hi) = (0usize, self.key_count());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.key_at(mid, &mut buf).cmp(target) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return true,
            }
        }
        false
    }

    /// Number of keys the set was built with.
    pub fn len(&self) -> usize {
        self.key_count()
    }

    pub fn is_empty(&self) -> bool {
        self.key_count() == 0
    }

    /// Number of front-coding restart blocks.
    #[cfg(test)]
    fn blocks(&self) -> usize {
        self.restart_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny encoder for the test blob. It uses one 8-bit code per byte
    /// value, which exercises the bit reader, the direct decode table, the
    /// varints and the block restarts without needing the package-merge
    /// builder. (The real builder is checked by `build.rs`, which decodes
    /// every key before writing the blob.)
    fn encode(keys: &[&str]) -> Vec<u8> {
        // Front-code with a restart every RESTART keys.
        let mut stream = Vec::new();
        let mut starts = Vec::new();
        let mut prev = String::new();
        for (i, key) in keys.iter().enumerate() {
            if i % RESTART == 0 {
                starts.push(stream.len());
                stream.push(0);
                stream.push(key.len() as u8);
                stream.extend_from_slice(key.as_bytes());
            } else {
                let common = prev
                    .bytes()
                    .zip(key.bytes())
                    .take_while(|(a, b)| a == b)
                    .count();
                stream.push(common as u8);
                stream.push((key.len() - common) as u8);
                stream.extend_from_slice(&key.as_bytes()[common..]);
            }
            prev = (*key).to_string();
        }

        // 8-bit code per symbol; the 12-bit table maps every index whose top
        // 8 bits are the symbol.
        let mut table = vec![0u16; 1 << MAX_CODE_LEN];
        for symbol in 0..=255u16 {
            let span = 1usize << (MAX_CODE_LEN - 8);
            let start = (symbol as usize) << (MAX_CODE_LEN - 8);
            for slot in &mut table[start..start + span] {
                *slot = (symbol << 4) | 8;
            }
        }

        let mut bits = Vec::new();
        let mut restarts = Vec::new();
        let mut next_start = 0;
        for (i, &byte) in stream.iter().enumerate() {
            while next_start < starts.len() && starts[next_start] == i {
                restarts.push(bits.len() as u32);
                next_start += 1;
            }
            for shift in (0..8).rev() {
                bits.push((byte >> shift) & 1);
            }
        }
        // The final start is `stream.len()` only for an empty stream.
        while next_start < starts.len() {
            restarts.push(bits.len() as u32);
            next_start += 1;
        }

        let mut packed = vec![0u8; bits.len().div_ceil(8)];
        for (i, bit) in bits.iter().enumerate() {
            if *bit != 0 {
                packed[i / 8] |= 1 << (7 - i % 8);
            }
        }

        let restarts_off = HEADER;
        let table_off = restarts_off + restarts.len() * 4;
        let bits_off = table_off + table.len() * 2;
        let mut blob = Vec::with_capacity(bits_off + packed.len());
        blob.extend_from_slice(&MAGIC.to_le_bytes());
        blob.extend_from_slice(&(keys.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(restarts.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(restarts_off as u32).to_le_bytes());
        blob.extend_from_slice(&(table_off as u32).to_le_bytes());
        blob.extend_from_slice(&(bits_off as u32).to_le_bytes());
        blob.extend_from_slice(&(MAX_CODE_LEN as u32).to_le_bytes());
        for _ in 0..3 {
            blob.extend_from_slice(&0u32.to_le_bytes());
        }
        for restart in &restarts {
            blob.extend_from_slice(&restart.to_le_bytes());
        }
        for entry in &table {
            blob.extend_from_slice(&entry.to_le_bytes());
        }
        blob.extend_from_slice(&packed);
        blob
    }

    /// The reader takes a `&'static [u8]`, so leak the test blob. The test
    /// process is short-lived and this keeps the API allocation-free.
    fn set(keys: &[&str]) -> &'static HuffmanSet {
        let blob = Box::leak(encode(keys).into_boxed_slice());
        Box::leak(Box::new(HuffmanSet::new(blob)))
    }

    #[test]
    fn finds_every_key_in_a_single_block() {
        let keys = ["a", "aa", "ab", "abc", "abcd", "b", "ba"];
        let set = set(&keys);
        assert_eq!(set.len(), keys.len());
        assert_eq!(set.blocks(), 1);
        for key in keys {
            assert!(set.contains(key), "{key}");
        }
        assert!(!set.contains(""));
        assert!(!set.contains("ac"));
        assert!(!set.contains("abcde"));
        assert!(!set.contains("z"));
    }

    #[test]
    fn finds_keys_across_restart_blocks() {
        let keys: Vec<String> = (0..100).map(|i| format!("key-{i:03}")).collect();
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let set = set(&refs);
        assert_eq!(set.blocks(), 7);
        for key in &refs {
            assert!(set.contains(key), "{key}");
        }
        for i in 0..100 {
            assert!(!set.contains(&format!("key-{i:03}x")));
        }
    }

    #[test]
    fn empty_set() {
        let set = set(&[]);
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert!(!set.contains("x"));
    }
}
