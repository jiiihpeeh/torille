//! Build-time stem-shape masks for the recognition walks.
//!
//! `emits_form` answers "does this lemma's paradigm contain the surface?" by
//! building (or probing) the whole paradigm. The walks try thousands of
//! candidate lemmas, and most of them cannot emit the surface at all. This
//! module is the cheap pre-filter: one 64-bit mask per lexicon row, holding
//! the first three characters of every surface the row can generate.
//!
//! The mask is conservative by construction. The generator walks the same
//! `inflect_all` sources as the runtime and sets a bit for every emitted form,
//! so a real form can never be rejected. A hash collision only turns a
//! rejection into a false positive, which takes the old `emits_form` path.
//!
//! Classless rows (compounds, proper-name-like entries) get an all-ones mask:
//! their paradigm is assembled by the compound fallbacks, and the filter is
//! not worth the risk there.
//!
//! The blob is written by `gen_lemma_stem_shapes` and committed under `maps/`.
//! `build.rs` decompresses it into `OUT_DIR`; the runtime reads the plain
//! bytes with `include_bytes!`.

use std::sync::OnceLock;

use crate::kotus::Lemma;

/// Blob magic and header: `LSS2` + the row count as a little-endian `u32`.
const MAGIC: &[u8; 4] = b"LSS2";
const HEADER: usize = 8;
const MASK_BYTES: usize = 32;

/// The embedded mask blob. `build.rs` writes an empty header (count 0) when
/// the committed blob is absent, which disables the filter.
const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lemma_stem_shape.bin"));

/// The mask bytes, or `None` when the blob is absent or does not match the
/// embedded lexicon.
static SHAPE: OnceLock<Option<&'static [u8]>> = OnceLock::new();

fn masks() -> Option<&'static [u8]> {
    *SHAPE.get_or_init(|| {
        if BYTES.len() < HEADER || &BYTES[..4] != MAGIC {
            return None;
        }
        let count = u32::from_le_bytes(BYTES[4..HEADER].try_into().ok()?) as usize;
        if count != crate::kotus::sanalista().len() || BYTES.len() != HEADER + count * MASK_BYTES {
            return None;
        }
        Some(&BYTES[HEADER..])
    })
}

/// The four hash bits for a surface's first four characters (or the whole
/// surface when it is shorter).
///
/// Four is more selective than three and still conservative: the generator
/// stores the same prefix of every form, and a form shorter than four can
/// only be that shorter surface. Four independent hashes turn the 64-bit mask
/// into a 256-bit Bloom mask: a form sets one bit in each word, and a
/// candidate must have all four bits set. The generator and the runtime must
/// use the same function; the tool calls it through this module rather than
/// duplicating it. Lowercasing keeps a sentence-initial token and its
/// lowercase lemma on the same bit.
pub fn prefix_bits(surface: &str) -> [u64; 4] {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    const SEEDS: [u64; 4] = [
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
        0x1656_67b1_9e37_79f9,
        0x27d4_eb2f_1656_67c5,
    ];
    let mut out = [0u64; 4];
    for (slot, seed) in out.iter_mut().zip(SEEDS) {
        let mut hash = FNV_OFFSET ^ seed;
        for ch in surface.chars().take(4) {
            for lower in ch.to_lowercase() {
                let mut buf = [0u8; 4];
                for &byte in lower.encode_utf8(&mut buf).as_bytes() {
                    hash ^= byte as u64;
                    hash = hash.wrapping_mul(FNV_PRIME);
                }
            }
        }
        *slot = 1u64 << (hash & 63);
    }
    out
}

/// The first hash bit, kept for callers that only need one.
pub fn prefix_bit(surface: &str) -> u64 {
    prefix_bits(surface)[0]
}

/// The row index of `lemma` in the embedded lexicon, if it is one of its rows.
///
/// The walks return `&Lemma` borrowed from the lexicon, so the row is the
/// pointer offset. A caller passing a different slice (tests, a caller-owned
/// lexicon) gets `None`, and the filter stays out of the way.
fn row_of(lemma: &Lemma<'_>) -> Option<usize> {
    let lexicon = crate::kotus::lemmas();
    let start = lexicon.as_ptr() as usize;
    let size = std::mem::size_of::<Lemma<'_>>();
    let end = start.checked_add(size.checked_mul(lexicon.len())?)?;
    let at = lemma as *const Lemma<'_> as usize;
    if at < start || at >= end {
        return None;
    }
    Some((at - start) / size)
}

/// Whether `lemma` might emit `surface` under the recognition walk.
///
/// `false` is a proof that it cannot; `true` is "not ruled out". The caller
/// still runs [`crate::expand::emits_form`].
pub(crate) fn might_emit(lemma: &Lemma<'_>, surface: &str) -> bool {
    let Some(row) = row_of(lemma) else {
        return true;
    };
    might_emit_row(row, surface)
}

/// [`might_emit`] for a row already named by index.
///
/// The shallow stem-map gate walks [`crate::kotus::EmbeddedLemma`] rows, not
/// the borrowed lexicon slice, so it cannot form a `&Lemma`. The row index is
/// the same index the mask blob is built from.
pub(crate) fn might_emit_row(row: usize, surface: &str) -> bool {
    let Some(masks) = masks() else {
        return true;
    };
    let bits = prefix_bits(surface);
    for (index, bit) in bits.into_iter().enumerate() {
        let at = row * MASK_BYTES + index * 8;
        let Some(bytes) = masks.get(at..at + 8) else {
            return true;
        };
        let mask = u64::from_le_bytes(bytes.try_into().expect("eight mask bytes"));
        if mask != u64::MAX && mask & bit == 0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prefix_bit_is_case_insensitive() {
        assert_eq!(prefix_bit("talo"), prefix_bit("TALO"));
        assert_eq!(prefix_bit("äiti"), prefix_bit("ÄITI"));
    }

    #[test]
    fn the_four_hashes_are_not_the_same_bit() {
        let bits = prefix_bits("talo");
        assert_eq!(bits[0], prefix_bit("talo"));
        assert!(bits.iter().any(|bit| *bit != bits[0]));
    }

    #[test]
    fn the_row_offset_round_trips() {
        let lexicon = crate::kotus::lemmas();
        let target = lexicon
            .iter()
            .position(|lemma| lemma.word == "talo")
            .expect("talo is in the lexicon");
        assert_eq!(row_of(&lexicon[target]), Some(target));
        let other = crate::kotus::Lemma {
            word: "talo",
            homonymy: "",
            part_of_speech: "substantiivi",
            inflections: Vec::new(),
        };
        assert_eq!(row_of(&other), None);
    }

    /// If the blob is present, every catalogue form of a sample of lemmas must
    /// pass. The generator performs the exhaustive check at build time; this
    /// keeps the runtime hash and the row lookup honest.
    #[test]
    fn the_mask_accepts_real_forms() {
        if masks().is_none() {
            eprintln!("lemma_stem_shape blob absent; filter disabled");
            return;
        }
        let cat = crate::spec::Catalogues::embedded();
        let lexicon = crate::kotus::lemmas();
        let index = crate::expand::LexiconIndex::new(lexicon);
        for lemma in lexicon.iter().step_by(97) {
            if lemma.inflections.is_empty() {
                continue;
            }
            for form in crate::expand::inflect_all(
                lemma.word,
                &lemma.inflections,
                cat,
                Some(&index),
                crate::tags::PartOfSpeech::Noun,
                true,
            ) {
                assert!(
                    might_emit(lemma, &form.word),
                    "{} rejected by its own mask for {}",
                    lemma.word,
                    form.word
                );
            }
        }
    }

    /// The noun fixture is the same set of lemmas `tests/catalogue.rs` runs
    /// every catalogue form of. Check that the mask accepts all of them.
    #[test]
    fn the_mask_accepts_the_noun_fixture() {
        if masks().is_none() {
            return;
        }
        let cat = crate::spec::Catalogues::embedded();
        let lexicon = crate::kotus::lemmas();
        for line in include_str!("../../tests/fixtures/noun_classes.txt").lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some(word) = line.split('\t').nth(1) else {
                continue;
            };
            let Some(row) = crate::kotus::sanalista().by_word(word).next() else {
                continue;
            };
            let lemma = &lexicon[row.row() as usize];
            if lemma.inflections.is_empty() {
                continue;
            }
            for form in crate::expand::inflect_all(
                word,
                &lemma.inflections,
                cat,
                None,
                crate::tags::PartOfSpeech::Noun,
                true,
            ) {
                assert!(
                    might_emit(lemma, &form.word),
                    "{word} rejected by its own mask for {}",
                    form.word
                );
            }
        }
    }
}
