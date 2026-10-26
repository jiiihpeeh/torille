//! Compile-time lookup tables for the spell-check pipeline.
//!
//! The form subset (lemmas + genitive-singular forms) and the **stem maps**
//! are sorted, front-coded binary blobs, embedded with `include_bytes!` and
//! read through a block-restarted binary search. The compiler never parses a
//! string literal for them.
//!
//! ## Blob layout (MST2 / SST2)
//!
//! All integers are little-endian; `varint` is LEB128.
//!
//! ```text
//!   0  u32 magic            = [`MAGIC`] (map) / [`SET_MAGIC`] (set)
//!   4  u32 key_count
//!   8  u32 restart_count    = ceil(key_count / RESTART)
//!  12  u32 restarts_off     = 40
//!  16  u32 keys_off
//!  20  u32 ids_off          -- one u32 id per key
//!  24  u32 overflow_off     -- multi-value lists (maps only)
//!  28  u32 string_offsets_off
//!  32  u32 string_count
//!  36  u32 strings_off
//!  40  restarts: restart_count x u32   -- offsets into the keys region
//!      keys:     front-coded records (see below)
//!      ids:      key_count x u32
//!      overflow: count byte + count x u32 ids, for multi-value keys
//!      offsets:  (string_count + 1) x u32 -- byte offsets into `strings`
//!      strings:  concatenated UTF-8, no terminators
//! ```
//!
//! Keys are sorted and **front-coded**: every `RESTART`-th record stores its
//! key in full, the others store `varint(shared_prefix_len)` +
//! `varint(suffix_len)` + the suffix bytes, relative to the previous key. The
//! restart points let a lookup seek to the start of the record's block and
//! decode at most `RESTART` records, so binary search is
//! `O(log n * RESTART)` decodes.
//!
//! Each key has one `u32` in `ids`. A value below [`MULTI`] is a direct string
//! id; a value with the top bit set points into `overflow`, whose byte at that
//! offset is the count and whose following `count` u32s are string ids.
//! **99.5 % of the noun stem map's keys have exactly one base**, so the
//! direct case is what matters: it turns the old 8-byte `(start, count)` row
//! into 4 bytes. The base strings are deduplicated into one pool (83 k unique
//! strings for 687 k entries), and an offset table gives O(1) `id -> &str`.
//!
//! Built by `build.rs::write_front_coded_map` (maps) and
//! `build.rs::emit_front_coded_set` (the form set). The tests encode the same
//! format in `encode_map` / `encode_set`, so the reader is pinned against a
//! hand-built blob, independent of `build.rs`.

use crate::map_reader::{FrontCodedKeys, MAP_HEADER, read_u32};

/// Map blob magic (`"MST2"`).
///
/// Used by the test encoders below; kept at module scope so the format
/// documentation can link to it.
#[allow(dead_code)]
pub const MAGIC: u32 = 0x3254_534D;

/// Set blob magic (`"SST2"`).
#[allow(dead_code)]
pub const SET_MAGIC: u32 = 0x3254_5353;

/// High bit of an id: the low 31 bits index the overflow region.
const MULTI: u32 = 0x8000_0000;

/// Header size in bytes (kept as a local name for the test encoders).
#[allow(dead_code)]
const HEADER: usize = MAP_HEADER;

/// A `(key, values)` lookup over one embedded blob.
pub struct StemMap {
    keys: FrontCodedKeys,
}

impl StemMap {
    /// Wrap a blob built by `build.rs::write_front_coded_map`.
    pub const fn new(blob: &'static [u8]) -> Self {
        Self {
            keys: FrontCodedKeys::new(blob),
        }
    }

    fn ids_off(&self) -> usize {
        read_u32(self.keys.blob(), 20) as usize
    }

    fn overflow_off(&self) -> usize {
        read_u32(self.keys.blob(), 24) as usize
    }

    fn string_offsets_off(&self) -> usize {
        read_u32(self.keys.blob(), 28) as usize
    }

    fn string_count(&self) -> usize {
        read_u32(self.keys.blob(), 32) as usize
    }

    fn strings_off(&self) -> usize {
        read_u32(self.keys.blob(), 36) as usize
    }

    /// The string for id `id`.
    fn string(&self, id: u32) -> &'static str {
        debug_assert!((id as usize) < self.string_count());
        let offsets = self.string_offsets_off();
        let start = read_u32(self.keys.blob(), offsets + id as usize * 4) as usize;
        let end = read_u32(self.keys.blob(), offsets + (id as usize + 1) * 4) as usize;
        let base = self.strings_off() + start;
        std::str::from_utf8(&self.keys.blob()[base..base + (end - start)])
            .expect("blob holds UTF-8 values")
    }

    /// True if `key` is in the map (regardless of how many values it has).
    pub fn contains(&self, key: &str) -> bool {
        self.keys.find(key).is_some()
    }

    /// Iterate the values for `key`. Empty iterator if the key isn't in the
    /// map.
    pub fn bases(&self, key: &str) -> BasesIter<'_> {
        let Some(i) = self.keys.find(key) else {
            return BasesIter {
                map: self,
                single: None,
                offset: 0,
                remaining: 0,
            };
        };
        let id = read_u32(self.keys.blob(), self.ids_off() + i * 4);
        if id & MULTI == 0 {
            // 99.5 % of the noun stem map: one value, inlined.
            BasesIter {
                map: self,
                single: Some(id),
                offset: 0,
                remaining: 0,
            }
        } else {
            let at = self.overflow_off() + (id & !MULTI) as usize;
            let count = self.keys.blob()[at] as usize;
            BasesIter {
                map: self,
                single: None,
                offset: at + 1,
                remaining: count,
            }
        }
    }

    /// Number of unique keys the map was built with.
    pub fn len(&self) -> usize {
        self.keys.key_count()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.key_count() == 0
    }

    /// Number of front-coding restart blocks.
    #[cfg(test)]
    fn blocks(&self) -> usize {
        self.keys.restart_count()
    }
}

/// A sorted, front-coded set of strings: the `all_forms` subset.
///
/// Reuses the [`StemMap`] key reader. The blob has the same header and
/// restart table, with the value fields left at zero because a set has no
/// payload.
pub struct StaticSet {
    keys: FrontCodedKeys,
}

impl StaticSet {
    /// Wrap a blob built by `build.rs::emit_front_coded_set`.
    pub const fn new(blob: &'static [u8]) -> Self {
        Self {
            keys: FrontCodedKeys::new(blob),
        }
    }

    /// True if `key` is in the set.
    pub fn contains(&self, key: &str) -> bool {
        self.keys.find(key).is_some()
    }

    /// Number of keys in the set.
    pub fn len(&self) -> usize {
        self.keys.key_count()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.key_count() == 0
    }

    /// Number of front-coding restart blocks.
    #[cfg(test)]
    fn blocks(&self) -> usize {
        self.keys.restart_count()
    }
}

/// Iterator over a key's values. Yields `&str` slices into the blob.
pub struct BasesIter<'a> {
    map: &'a StemMap,
    /// The direct single-value id, if this key has exactly one value.
    single: Option<u32>,
    /// Next u32 id in the overflow region (multi-value keys).
    offset: usize,
    remaining: usize,
}

impl<'a> Iterator for BasesIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let id = match self.single.take() {
            Some(id) => id,
            None => {
                if self.remaining == 0 {
                    return None;
                }
                self.remaining -= 1;
                let id = read_u32(self.map.keys.blob(), self.offset);
                self.offset += 4;
                id
            }
        };
        Some(self.map.string(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_reader::RESTART;

    fn write_varint(mut v: u32, out: &mut Vec<u8>) {
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
    }

    /// Front-code `keys` (already sorted) into `(restarts, key_bytes)`.
    fn encode_keys(keys: &[&str]) -> (Vec<u32>, Vec<u8>) {
        let mut encoded = Vec::new();
        let mut restarts = Vec::new();
        let mut prev = String::new();
        for (i, key) in keys.iter().enumerate() {
            if i % RESTART == 0 {
                restarts.push(encoded.len() as u32);
                write_varint(0, &mut encoded);
                write_varint(key.len() as u32, &mut encoded);
                encoded.extend_from_slice(key.as_bytes());
            } else {
                let common = prev
                    .bytes()
                    .zip(key.bytes())
                    .take_while(|(a, b)| a == b)
                    .count();
                write_varint(common as u32, &mut encoded);
                write_varint((key.len() - common) as u32, &mut encoded);
                encoded.extend_from_slice(&key.as_bytes()[common..]);
            }
            prev = (*key).to_string();
        }
        (restarts, encoded)
    }

    /// Encode `entries` (already sorted by key) in the on-disk format.
    fn encode_map(entries: &[(&str, &[&str])]) -> Vec<u8> {
        let keys: Vec<&str> = entries.iter().map(|(key, _)| *key).collect();
        let (restarts, encoded_keys) = encode_keys(&keys);

        // Deduplicate the values; ids are the sorted pool position.
        let mut pool: Vec<&str> = entries
            .iter()
            .flat_map(|(_, vs)| vs.iter().copied())
            .collect();
        pool.sort_unstable();
        pool.dedup();
        let id_of = |v: &str| pool.binary_search(&v).unwrap() as u32;

        let mut ids: Vec<u32> = Vec::with_capacity(entries.len());
        let mut overflow: Vec<u8> = Vec::new();
        for (_, vs) in entries {
            if vs.len() == 1 {
                ids.push(id_of(vs[0]));
            } else {
                let off = overflow.len() as u32;
                overflow.push(vs.len() as u8);
                for v in *vs {
                    overflow.extend_from_slice(&id_of(v).to_le_bytes());
                }
                ids.push(MULTI | off);
            }
        }

        let mut string_offsets = Vec::with_capacity((pool.len() + 1) * 4);
        let mut strings = Vec::new();
        for s in &pool {
            string_offsets.extend_from_slice(&(strings.len() as u32).to_le_bytes());
            strings.extend_from_slice(s.as_bytes());
        }
        string_offsets.extend_from_slice(&(strings.len() as u32).to_le_bytes());

        let restarts_off = HEADER;
        let keys_off = restarts_off + restarts.len() * 4;
        let ids_off = keys_off + encoded_keys.len();
        let overflow_off = ids_off + ids.len() * 4;
        let string_offsets_off = overflow_off + overflow.len();
        let strings_off = string_offsets_off + string_offsets.len();

        let mut blob = Vec::new();
        blob.extend_from_slice(&MAGIC.to_le_bytes());
        blob.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(restarts.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(restarts_off as u32).to_le_bytes());
        blob.extend_from_slice(&(keys_off as u32).to_le_bytes());
        blob.extend_from_slice(&(ids_off as u32).to_le_bytes());
        blob.extend_from_slice(&(overflow_off as u32).to_le_bytes());
        blob.extend_from_slice(&(string_offsets_off as u32).to_le_bytes());
        blob.extend_from_slice(&(pool.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(strings_off as u32).to_le_bytes());
        for r in restarts {
            blob.extend_from_slice(&r.to_le_bytes());
        }
        blob.extend_from_slice(&encoded_keys);
        for id in ids {
            blob.extend_from_slice(&id.to_le_bytes());
        }
        blob.extend_from_slice(&overflow);
        blob.extend_from_slice(&string_offsets);
        blob.extend_from_slice(&strings);
        blob
    }

    /// Encode a set (already sorted keys) in the same header layout, with
    /// the value fields zero.
    fn encode_set(keys: &[&str]) -> Vec<u8> {
        let (restarts, encoded) = encode_keys(keys);
        let restarts_off = HEADER;
        let keys_off = restarts_off + restarts.len() * 4;

        let mut blob = Vec::new();
        blob.extend_from_slice(&SET_MAGIC.to_le_bytes());
        blob.extend_from_slice(&(keys.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(restarts.len() as u32).to_le_bytes());
        blob.extend_from_slice(&(restarts_off as u32).to_le_bytes());
        blob.extend_from_slice(&(keys_off as u32).to_le_bytes());
        for _ in 0..5 {
            blob.extend_from_slice(&0u32.to_le_bytes());
        }
        for r in restarts {
            blob.extend_from_slice(&r.to_le_bytes());
        }
        blob.extend_from_slice(&encoded);
        blob
    }

    fn sample() -> StemMap {
        let entries: Vec<(String, Vec<&str>)> = (0..40)
            .map(|i| (format!("stem{i:02}"), vec!["base"]))
            .collect();
        let refs: Vec<(&str, &[&str])> = entries
            .iter()
            .map(|(s, b)| (s.as_str(), b.as_slice()))
            .collect();
        StemMap::new(Box::leak(encode_map(&refs).into_boxed_slice()))
    }

    #[test]
    fn finds_across_and_inside_blocks() {
        let m = sample();
        assert_eq!(m.blocks(), 3); // 40 keys / 16
        for i in [0usize, 15, 16, 31, 32, 39] {
            let key = format!("stem{i:02}");
            assert!(m.contains(&key), "{key} not found");
            assert_eq!(m.bases(&key).collect::<Vec<_>>(), vec!["base"]);
        }
        assert!(!m.contains("nope"));
        assert_eq!(m.bases("nope").count(), 0);
    }

    #[test]
    fn iterates_multiple_bases() {
        let blob = encode_map(&[("ves", &["vesi", "vesa"]), ("vesi", &["vesi"])]);
        let m = StemMap::new(Box::leak(blob.into_boxed_slice()));
        let mut bases: Vec<&str> = m.bases("ves").collect();
        bases.sort();
        assert_eq!(bases, vec!["vesa", "vesi"]);
        assert_eq!(m.bases("vesi").collect::<Vec<_>>(), vec!["vesi"]);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn deduplicates_strings_across_keys() {
        // `talo` is shared by many keys; the pool should hold it once and
        // both keys should still return it.
        let blob = encode_map(&[("talo", &["talo"]), ("talon", &["talo"])]);
        let m = StemMap::new(Box::leak(blob.into_boxed_slice()));
        assert_eq!(m.bases("talo").collect::<Vec<_>>(), vec!["talo"]);
        assert_eq!(m.bases("talon").collect::<Vec<_>>(), vec!["talo"]);
        // One unique string + its offset sentinel.
        assert_eq!(m.string_count(), 1);
    }

    #[test]
    fn empty_map() {
        let m = StemMap::new(Box::leak(encode_map(&[]).into_boxed_slice()));
        assert!(m.is_empty());
        assert!(!m.contains("x"));
        assert_eq!(m.bases("x").count(), 0);
    }

    #[test]
    fn set_finds_across_and_inside_blocks() {
        let keys: Vec<String> = (0..40).map(|i| format!("form{i:02}")).collect();
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let s = StaticSet::new(Box::leak(encode_set(&refs).into_boxed_slice()));
        assert_eq!(s.blocks(), 3);
        for i in [0usize, 15, 16, 31, 32, 39] {
            let key = format!("form{i:02}");
            assert!(s.contains(&key), "{key} not found");
        }
        assert!(!s.contains("form40"));
        assert!(!s.contains(""));
        assert_eq!(s.len(), 40);
    }

    #[test]
    fn set_front_coding_shares_prefixes() {
        let keys = ["kissa", "kissaa", "kissan", "talo", "talon", "vesi"];
        let s = StaticSet::new(Box::leak(encode_set(&keys).into_boxed_slice()));
        for key in keys {
            assert!(s.contains(key), "{key} not found");
        }
        assert!(!s.contains("kisso"));
        assert!(!s.contains("talo "));
    }

    #[test]
    fn empty_set() {
        let s = StaticSet::new(Box::leak(encode_set(&[]).into_boxed_slice()));
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert!(!s.contains("x"));
    }
}
