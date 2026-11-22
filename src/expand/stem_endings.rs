//! Per-stem ending signatures for the negative-evidence filter.
//!
//! The stem maps answer "which lemmas have this stem". The verifier then
//! builds each candidate's paradigm to find out whether the stem can actually
//! take the ending the surface carries. Most of those builds fail. This table
//! records, for every stem key, the endings that *some* generated form can
//! strip to that stem:
//!
//! ```text
//! stem -> sorted ending ids
//! ```
//!
//! A missing id means no base at that stem can spell `stem + ending`, so the
//! verifier can skip the paradigm. It is a necessary condition only, derived
//! from the same engine forms the verifier uses; it never rejects a form the
//! engine would accept.
//!
//! ## Blob layout (`ENDP`)
//!
//! ```text
//!   0  "ENDP" / "END0" (empty table)
//!   4  u8 version
//!   5  u8 map kind (0 noun, 1 verb)
//!   6  u16 reserved
//!   8  u32 ending_count
//!  12  u32 pattern_count
//!  16  u32 key_count            -- must equal the stem map's key count
//!  20  endings: ending_count x (u8 len + UTF-8 bytes)
//!      patterns: pattern_count x (u8 len + ending ids, sorted)
//!      ids:      key_count x u16  -- one pattern id per stem key, in the
//!                                    stem map's sorted key order
//! ```
//!
//! The id array is aligned with the stem map's key order, so the runtime pays
//! no second key search: it reuses the ordinal `StemMap::find_index` returned.

use std::sync::OnceLock;

use crate::fast_hash::FxHashMap;
use crate::map_reader::{read_u16, read_u32};

/// The noun stem-map signature.
pub static NOUN: StemEndings =
    StemEndings::new(include_bytes!(concat!(env!("OUT_DIR"), "/stem_endings_noun.bin")));

/// The verb stem-map signature.
pub static VERB: StemEndings =
    StemEndings::new(include_bytes!(concat!(env!("OUT_DIR"), "/stem_endings_verb.bin")));

pub struct StemEndings {
    blob: &'static [u8],
    table: OnceLock<Table>,
}

struct Table {
    ending_to_id: FxHashMap<&'static str, u8>,
    patterns: Vec<&'static [u8]>,
    ids_off: usize,
    key_count: usize,
}

impl StemEndings {
    pub const fn new(blob: &'static [u8]) -> Self {
        Self {
            blob,
            table: OnceLock::new(),
        }
    }

    /// The number of stem keys the table covers. Zero for an empty table.
    #[allow(dead_code)] // used by the alignment test and the diagnostic report
    pub fn key_count(&self) -> usize {
        self.table().key_count
    }

    /// Whether the stem at `key_index` can take `ending`.
    ///
    /// A stem outside the table, an ending outside the table, or an empty
    /// table all answer `true`: the caller then runs the normal verifier.
    /// Only a known stem and a known ending can answer `false`.
    pub fn allows(&self, key_index: usize, ending: &str) -> bool {
        self.allows_known(key_index, ending).unwrap_or(true)
    }

    /// [`allows`] with the “table has no answer” case made explicit.
    ///
    /// `Some(true)` means the signature records the ending, `Some(false)`
    /// means it does not, and `None` means the table cannot answer (empty,
    /// a key outside the table, or an ending outside the dictionary). A
    /// caller that wants to skip the verifier must fall back to it on
    /// `None`; an empty table must never turn into “every form is allowed”.
    pub fn allows_known(&self, key_index: usize, ending: &str) -> Option<bool> {
        let table = self.table();
        if table.key_count == 0 || key_index >= table.key_count {
            return None;
        }
        let &ending_id = table.ending_to_id.get(ending)?;
        let pattern_id = read_u16(self.blob, table.ids_off + key_index * 2) as usize;
        Some(
            table
                .patterns
                .get(pattern_id)
                .is_some_and(|pattern| pattern.binary_search(&ending_id).is_ok()),
        )
    }

    fn table(&self) -> &Table {
        self.table.get_or_init(|| Table::parse(self.blob))
    }
}

impl Table {
    fn parse(blob: &'static [u8]) -> Self {
        let empty = Self {
            ending_to_id: FxHashMap::default(),
            patterns: Vec::new(),
            ids_off: 0,
            key_count: 0,
        };
        if blob.len() < 20 || &blob[..4] != b"ENDP" {
            return empty;
        }
        let ending_count = read_u32(blob, 8) as usize;
        let pattern_count = read_u32(blob, 12) as usize;
        let key_count = read_u32(blob, 16) as usize;
        let mut offset = 20;
        let mut ending_to_id = FxHashMap::default();
        for id in 0..ending_count {
            let len = blob[offset] as usize;
            offset += 1;
            let ending = std::str::from_utf8(&blob[offset..offset + len])
                .expect("the ending table holds UTF-8");
            offset += len;
            ending_to_id.insert(ending, id as u8);
        }
        let mut patterns = Vec::with_capacity(pattern_count);
        for _ in 0..pattern_count {
            let len = blob[offset] as usize;
            offset += 1;
            patterns.push(&blob[offset..offset + len]);
            offset += len;
        }
        let ids_off = offset;
        assert!(
            ids_off + key_count * 2 <= blob.len(),
            "the stem-ending signature blob is truncated"
        );
        Self {
            ending_to_id,
            patterns,
            ids_off,
            key_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ending_tables_align_with_the_stem_maps() {
        // The id array is indexed by the stem map's ordinal. A count mismatch
        // means the blobs and the maps were generated from different key sets.
        assert_eq!(
            NOUN.key_count(),
            crate::expand::check_normal::STEM_BUCKETS_NOUN.len(),
            "noun ending signatures and noun stem map disagree"
        );
        assert_eq!(
            VERB.key_count(),
            crate::expand::check_normal::STEM_BUCKETS_VERB.len(),
            "verb ending signatures and verb stem map disagree"
        );
    }

    #[test]
    fn an_empty_table_never_rejects() {
        let empty = StemEndings::new(b"END0");
        assert_eq!(empty.key_count(), 0);
        assert!(empty.allows(0, "ssa"));
        assert!(empty.allows(999_999, ""));
        assert_eq!(empty.allows_known(0, "ssa"), None);
        assert_eq!(empty.allows_known(999_999, ""), None);
    }
}
