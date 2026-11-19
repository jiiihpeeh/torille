//! The exact `(stem, ending) → bases + readings` table.
//!
//! The stem maps answer "which lemmas have this stem"; the resolver then
//! *builds* each candidate's paradigm to find out whether that stem actually
//! takes the ending the surface carries, and builds it again to read the
//! matching form. This table is the missing dimension: for a stem the
//! generator saw, it lists the bases and the readings each `stem + ending`
//! spells.
//!
//! ## Contract
//!
//! **The table never rejects.** A lookup that misses — no such stem, no such
//! ending, an empty table — falls through to the engine walk. Missing data can
//! therefore only make the resolver slower, never wrong, and a partial table
//! (a frequency-bounded subset, say) is a coverage question rather than a
//! correctness one.
//!
//! ## Blob layout (`FMT1`)
//!
//! The key region matches [`crate::map_reader::FrontCodedKeys`], so the same
//! binary search reads it; the header puts this table's own sections in the
//! fields that reader does not touch.
//!
//! ```text
//!   0  u32 magic "FMT1"        4  u32 key_count
//!   8  u32 restart_count      12  u32 restarts_off (= 40)
//!  16  u32 keys_off           20  u32 group_offs_off  (u32 per key)
//!  24  u32 bases_off          28  u32 base_count
//!  32  u32 readings_off       36  u32 reading_count
//!  40  restarts: restart_count x u32, offsets into the keys region
//!      keys:        front-coded records (varint shared, varint len, suffix)
//!      group_offs:  key_count x u32, absolute offsets into the groups region
//!      groups:      varint base_groups; per group varint base_id,
//!                   varint entries; per entry u8 ending_len, ending bytes,
//!                   u8 reading_count, reading_count x varint reading_id
//!      bases:       u32 count, (count + 1) x u32 offsets, UTF-8 lemmas
//!      readings:    u32 count, count x 11 packed bytes
//! ```
//!
//! The blob is written by `tools/src/bin/gen_form_table.rs` (which computes it
//! with the engine) and committed under `maps/`. `build.rs` decompresses it
//! into `OUT_DIR` only when the `form-table` feature is on; otherwise it emits
//! an empty header, so the default build neither downloads nor embeds it.

use crate::map_reader::{FrontCodedKeys, read_u32};
use crate::tags::{
    Case, Clitic, Degree, Mood, Number, PartOfSpeech, Person, Polarity, Possessive, Tags, Tense,
    Voice,
};

const MAGIC: u32 = 0x3154_4D46;
const GROUP_OFFS_OFF: usize = 20;
const BASES_OFF: usize = 24;
const BASE_COUNT: usize = 28;
const READINGS_OFF: usize = 32;
const READING_COUNT: usize = 36;
const HEADER: usize = 40;

/// Bytes per packed reading: one index per [`Tags`] dimension.
pub const READING_BYTES: usize = 11;

/// The table compiled into the binary.
pub fn table() -> &'static FormTable {
    static TABLE: FormTable = FormTable::new(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/form_table.bin",
    )));
    &TABLE
}

/// The bases that spell `stem + ending` in the embedded table.
///
/// Empty when the table has nothing to say, which the caller must treat as
/// "walk the engine", not as "no base".
pub fn bases(stem: &str, ending: &str) -> Vec<&'static str> {
    table().bases(stem, ending)
}

/// The readings the embedded table records for `base` at `stem + ending`.
pub fn readings(stem: &str, ending: &str, base: &str) -> Option<Vec<Tags>> {
    table().readings(stem, ending, base)
}

/// Pack a reading into its 11 indices.
pub fn encode_reading(tags: &Tags) -> [u8; READING_BYTES] {
    [
        optional_index(tags.part_of_speech, &PartOfSpeech::ALL),
        optional_index(tags.case, &Case::ALL),
        optional_index(tags.number, &Number::ALL),
        optional_index(tags.person, &Person::ALL),
        optional_index(tags.tense, &Tense::ALL),
        optional_index(tags.mood, &Mood::ALL),
        optional_index(tags.voice, &Voice::ALL),
        optional_index(tags.possessive, &Possessive::ALL),
        optional_index(tags.degree, &Degree::ALL),
        optional_index(tags.clitic, &Clitic::ALL),
        index(tags.polarity, &Polarity::ALL),
    ]
}

/// Unpack a reading.
pub fn decode_reading(bytes: &[u8]) -> Tags {
    debug_assert_eq!(bytes.len(), READING_BYTES);
    Tags {
        part_of_speech: optional_value(bytes[0], &PartOfSpeech::ALL),
        case: optional_value(bytes[1], &Case::ALL),
        number: optional_value(bytes[2], &Number::ALL),
        person: optional_value(bytes[3], &Person::ALL),
        tense: optional_value(bytes[4], &Tense::ALL),
        mood: optional_value(bytes[5], &Mood::ALL),
        voice: optional_value(bytes[6], &Voice::ALL),
        possessive: optional_value(bytes[7], &Possessive::ALL),
        degree: optional_value(bytes[8], &Degree::ALL),
        clitic: optional_value(bytes[9], &Clitic::ALL),
        polarity: value(bytes[10], &Polarity::ALL),
    }
}

fn index<T: Copy + PartialEq>(value: T, all: &[T]) -> u8 {
    all.iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(0) as u8
}

fn optional_index<T: Copy + PartialEq>(value: Option<T>, all: &[T]) -> u8 {
    value.map_or(0, |value| index(value, all) + 1)
}

/// A mandatory dimension is a plain 0-based index, unlike `optional_value`.
fn value<T: Copy>(index: u8, all: &[T]) -> T {
    all.get(index as usize).copied().unwrap_or(all[0])
}

fn optional_value<T: Copy>(index: u8, all: &[T]) -> Option<T> {
    if index == 0 {
        None
    } else {
        all.get(index as usize - 1).copied()
    }
}

fn varint(blob: &[u8], at: usize) -> (u32, usize) {
    let mut result = 0u32;
    let mut shift = 0;
    let mut p = at;
    loop {
        let byte = blob[p];
        p += 1;
        result |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    (result, p)
}

/// A cursor over the variable-length group region.
struct Cursor<'a> {
    blob: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn varint(&mut self) -> u32 {
        let (value, next) = varint(self.blob, self.at);
        self.at = next;
        value
    }

    fn byte(&mut self) -> u8 {
        let value = self.blob[self.at];
        self.at += 1;
        value
    }
}

/// One embedded table.
pub struct FormTable {
    blob: &'static [u8],
    keys: FrontCodedKeys,
}

impl FormTable {
    pub const fn new(blob: &'static [u8]) -> Self {
        Self {
            blob,
            keys: FrontCodedKeys::new(blob),
        }
    }

    /// Number of stems the table knows.
    pub fn len(&self) -> usize {
        self.keys.key_count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the header looks like a table this reader understands.
    pub fn is_valid(&self) -> bool {
        self.blob.len() >= HEADER && read_u32(self.blob, 0) == MAGIC
    }

    fn base(&self, id: u32) -> &'static str {
        let count = read_u32(self.blob, BASE_COUNT) as usize;
        let offsets = read_u32(self.blob, BASES_OFF) as usize + 4;
        let strings = offsets + (count + 1) * 4;
        let start = read_u32(self.blob, offsets + id as usize * 4) as usize;
        let end = read_u32(self.blob, offsets + (id as usize + 1) * 4) as usize;
        std::str::from_utf8(&self.blob[strings + start..strings + end])
            .expect("the base pool is UTF-8")
    }

    fn reading(&self, id: u32) -> Tags {
        let count = read_u32(self.blob, READING_COUNT) as usize;
        debug_assert!((id as usize) < count);
        let at = read_u32(self.blob, READINGS_OFF) as usize + 4 + id as usize * READING_BYTES;
        decode_reading(&self.blob[at..at + READING_BYTES])
    }

    /// The bases that spell `stem + ending`, in stem-map order.
    pub fn bases(&self, stem: &str, ending: &str) -> Vec<&'static str> {
        let mut out = Vec::new();
        self.visit(stem, ending, |base, _| out.push(base));
        out
    }

    /// The readings recorded for `base` at `stem + ending`.
    pub fn readings(&self, stem: &str, ending: &str, base: &str) -> Option<Vec<Tags>> {
        let mut found = None;
        self.visit(stem, ending, |candidate, ids| {
            if candidate == base {
                found = Some(ids.iter().map(|id| self.reading(*id)).collect());
            }
        });
        found
    }

    /// Call `visit` for every base that spells `stem + ending`.
    fn visit(&self, stem: &str, ending: &str, mut visit: impl FnMut(&'static str, &[u32])) {
        if self.is_empty() || !self.is_valid() {
            return;
        }
        let Some(index) = self.keys.find(stem) else {
            return;
        };
        let group_offs = read_u32(self.blob, GROUP_OFFS_OFF) as usize;
        let at = read_u32(self.blob, group_offs + index * 4) as usize;
        let mut cursor = Cursor {
            blob: self.blob,
            at,
        };
        let groups = cursor.varint();
        let mut ids: Vec<u32> = Vec::new();
        for _ in 0..groups {
            let base_id = cursor.varint();
            let entries = cursor.varint();
            let mut matched = false;
            ids.clear();
            for _ in 0..entries {
                let len = cursor.byte() as usize;
                let start = cursor.at;
                let candidate = &self.blob[start..start + len];
                cursor.at += len;
                let count = cursor.byte();
                let mut entry_ids = Vec::new();
                for _ in 0..count {
                    entry_ids.push(cursor.varint());
                }
                if candidate == ending.as_bytes() {
                    matched = true;
                    ids = entry_ids;
                }
            }
            if matched {
                visit(self.base(base_id), &ids);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn varint_out(out: &mut Vec<u8>, mut value: u32) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    /// Build the smallest table the reader accepts: two stems, one base each,
    /// with one and two endings.
    fn fixture() -> Vec<u8> {
        let stems = ["aave", "talo"];
        let groups = [
            ("aave", vec![("tta", 0u8), ("ssa", 0u8)]),
            ("talo", vec![("ssa", 1u8)]),
        ];
        let readings: [[u8; READING_BYTES]; 2] = [
            encode_reading(&Tags {
                case: Some(Case::Abessive),
                number: Some(Number::Singular),
                part_of_speech: Some(PartOfSpeech::Noun),
                ..Tags::default()
            }),
            encode_reading(&Tags {
                case: Some(Case::Inessive),
                number: Some(Number::Singular),
                part_of_speech: Some(PartOfSpeech::Noun),
                ..Tags::default()
            }),
        ];

        // Keys region: every stem is a restart, so no prefix diffing.
        let mut keys = Vec::new();
        let mut restarts = Vec::new();
        for stem in stems {
            restarts.push(keys.len() as u32);
            varint_out(&mut keys, 0);
            varint_out(&mut keys, stem.len() as u32);
            keys.extend_from_slice(stem.as_bytes());
        }

        let mut group_region = Vec::new();
        let mut group_offs = Vec::new();
        for (base, group) in groups.iter().enumerate() {
            let entries = &group.1;
            group_offs.push(group_region.len() as u32);
            varint_out(&mut group_region, 1); // one base group
            varint_out(&mut group_region, base as u32);
            varint_out(&mut group_region, entries.len() as u32);
            for (ending, reading) in entries {
                group_region.push(ending.len() as u8);
                group_region.extend_from_slice(ending.as_bytes());
                group_region.push(1);
                varint_out(&mut group_region, u32::from(*reading));
            }
        }

        let bases = ["aave", "talo"];
        let mut base_pool = Vec::new();
        base_pool.extend_from_slice(&(bases.len() as u32).to_le_bytes());
        let mut offset = 0u32;
        for base in bases {
            base_pool.extend_from_slice(&offset.to_le_bytes());
            offset += base.len() as u32;
        }
        base_pool.extend_from_slice(&offset.to_le_bytes());
        for base in bases {
            base_pool.extend_from_slice(base.as_bytes());
        }

        let mut reading_pool = Vec::new();
        reading_pool.extend_from_slice(&(readings.len() as u32).to_le_bytes());
        for reading in readings {
            reading_pool.extend_from_slice(&reading);
        }

        let keys_off = HEADER + restarts.len() * 4;
        let group_offs_off = keys_off + keys.len();
        let groups_off = group_offs_off + group_offs.len() * 4;
        let bases_off = groups_off + group_region.len();
        let readings_off = bases_off + base_pool.len();

        let mut blob = vec![0u8; HEADER];
        blob[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        blob[4..8].copy_from_slice(&(stems.len() as u32).to_le_bytes());
        blob[8..12].copy_from_slice(&(restarts.len() as u32).to_le_bytes());
        blob[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
        blob[16..20].copy_from_slice(&(keys_off as u32).to_le_bytes());
        blob[GROUP_OFFS_OFF..GROUP_OFFS_OFF + 4]
            .copy_from_slice(&(group_offs_off as u32).to_le_bytes());
        blob[BASES_OFF..BASES_OFF + 4].copy_from_slice(&(bases_off as u32).to_le_bytes());
        blob[BASE_COUNT..BASE_COUNT + 4].copy_from_slice(&(bases.len() as u32).to_le_bytes());
        blob[READINGS_OFF..READINGS_OFF + 4].copy_from_slice(&(readings_off as u32).to_le_bytes());
        blob[READING_COUNT..READING_COUNT + 4]
            .copy_from_slice(&(readings.len() as u32).to_le_bytes());

        for restart in restarts {
            blob.extend_from_slice(&restart.to_le_bytes());
        }
        blob.extend_from_slice(&keys);
        for offset in group_offs {
            blob.extend_from_slice(&(groups_off as u32 + offset).to_le_bytes());
        }
        blob.extend_from_slice(&group_region);
        blob.extend_from_slice(&base_pool);
        blob.extend_from_slice(&reading_pool);
        blob
    }

    #[test]
    fn a_lookup_returns_the_bases_and_readings() {
        let blob: &'static [u8] = Box::leak(fixture().into_boxed_slice());
        let table = FormTable::new(blob);
        assert!(table.is_valid());
        assert_eq!(table.len(), 2);

        let mut bases = Vec::new();
        table.visit("aave", "tta", |base, ids| bases.push((base, ids.to_vec())));
        assert_eq!(bases.len(), 1);
        assert_eq!(bases[0].0, "aave");

        let mut found = Vec::new();
        table.visit("talo", "ssa", |base, _| found.push(base));
        assert_eq!(found, ["talo"]);

        let mut none = Vec::new();
        table.visit("talo", "tta", |base, _| none.push(base));
        assert!(none.is_empty(), "the ending belongs to the other stem");
        table.visit("kissa", "ssa", |base, _| none.push(base));
        assert!(none.is_empty(), "unknown stem");
    }

    /// When the build embedded the generated table, a hit must return exactly
    /// what the paradigm walk returns. This is the sampling half of the parity
    /// gate; `gen_form_table`'s spot check covers every 97th lemma at
    /// generation time.
    #[test]
    fn a_generated_table_agrees_with_the_walk() {
        if table().is_empty() {
            return;
        }
        let cat = crate::spec::Catalogues::embedded();
        let lexicon = crate::kotus::lemmas();
        let index = crate::expand::LexiconIndex::new(lexicon);
        for (surface, lemma) in [
            ("talossa", "talo"),
            ("aavetta", "aave"),
            ("uupuneet", "uupua"),
        ] {
            let rows: Vec<_> = crate::kotus::sanalista().by_word(lemma).collect();
            let inflections: Vec<_> = rows.iter().flat_map(|row| row.inflections()).collect();
            let pos = rows
                .first()
                .and_then(|row| PartOfSpeech::parse_all(row.part_of_speech()).first().copied())
                .unwrap_or(PartOfSpeech::Noun);
            let walk = crate::expand::form_readings(
                lemma,
                &inflections,
                cat,
                Some(&index),
                pos,
                false,
                surface,
            );
            let from_table = crate::expand::check_normal::table_readings(surface, lemma);
            assert_eq!(from_table, walk, "{surface} / {lemma}");
        }
    }

    #[test]
    fn the_embedded_table_misses_without_panicking() {
        // Whether the build embedded the real table depends on the feature; a
        // miss must never panic, which is the property this pins.
        let _ = bases("talossa", "ssa");
        let _ = readings("talossa", "ssa", "talo");
    }

    #[test]
    fn a_reading_round_trips() {
        for tags in [
            Tags::default(),
            Tags {
                case: Some(Case::Partitive),
                number: Some(Number::Plural),
                part_of_speech: Some(PartOfSpeech::Noun),
                ..Tags::default()
            },
            Tags {
                polarity: Polarity::Negative,
                ..Tags::default()
            },
            Tags {
                mood: Some(Mood::Conditional),
                tense: Some(Tense::Present),
                person: Some(Person::Third),
                number: Some(Number::Singular),
                voice: Some(Voice::Active),
                polarity: Polarity::Affirmative,
                ..Tags::default()
            },
        ] {
            assert_eq!(decode_reading(&encode_reading(&tags)), tags);
        }
    }
}
