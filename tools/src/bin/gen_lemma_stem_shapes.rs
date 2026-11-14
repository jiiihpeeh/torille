// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_lemma_stem_shapes` — write the per-lemma stem-shape masks.
//!
//! `src/expand/stem_shape.rs` embeds `maps/lemma_stem_shape.bin.gz`: one
//! `[u64; 4]` per lexicon row. A bit is the hash of the first four characters of
//! a surface the lemma can emit, so the recognition walk can reject a
//! candidate before `emits_form` builds its paradigm.
//!
//! The mask is conservative: every form generated here sets its own bit, and
//! classless rows get an all-ones mask. A false positive costs one
//! `emits_form`; a false negative would lose a word, so the generator
//! self-checks a sample against `emits_form`.
//!
//! Run after a sanalista or class-engine change, then rebuild:
//!
//!     cargo run --release -p torille-tools --bin gen_lemma_stem_shapes
//!     cargo build -p torille

use std::path::Path;

use torille::expand::stem_shape::prefix_bits;
use torille::expand::{emits_form, inflect_all};
use torille::kotus::lemmas;
use torille::spec::Catalogues;
use torille::tags::PartOfSpeech;

const MAGIC: &[u8; 4] = b"LSS2";
const MASK_BYTES: usize = 32;

fn main() {
    let catalogue = Catalogues::embedded();
    let lexicon = lemmas();

    let mut masks: Vec<[u64; 4]> = Vec::with_capacity(lexicon.len());
    let mut classless = 0usize;
    let mut bits = 0u32;
    let mut checked = 0usize;

    for (row, lemma) in lexicon.iter().enumerate() {
        // A classless row's paradigm is assembled by the compound fallbacks.
        // Skip the filter there rather than risk a false negative.
        if lemma.inflections.is_empty() {
            masks.push([u64::MAX; 4]);
            classless += 1;
            continue;
        }

        let mut mask = prefix_bits(lemma.word);
        let forms = inflect_all(
            lemma.word,
            &lemma.inflections,
            catalogue,
            None,
            PartOfSpeech::Noun,
            true,
        );
        for form in &forms {
            if !form.word.is_empty() {
                for (slot, bit) in mask.iter_mut().zip(prefix_bits(&form.word)) {
                    *slot |= bit;
                }
            }
        }
        if mask.iter().all(|word| *word == 0) {
            // Defensive: an empty mask would reject everything.
            mask = [u64::MAX; 4];
        }
        bits += mask.iter().map(|word| word.count_ones()).sum::<u32>();
        masks.push(mask);

        // Spot-check the same probe the search uses, with the same `None`
        // lexicon. Every 97th lemma keeps the generator fast while covering
        // every class region of the lexicon.
        if row % 97 == 0 {
            for form in &forms {
                assert!(
                    emits_form(
                        lemma.word,
                        &lemma.inflections,
                        catalogue,
                        None,
                        PartOfSpeech::Noun,
                        true,
                        &form.word,
                    ),
                    "{} / {}: emits_form disagrees with inflect_all",
                    lemma.word,
                    form.word
                );
                assert!(
                    mask.iter()
                        .zip(prefix_bits(&form.word))
                        .all(|(mask, bit)| mask & bit != 0),
                    "{} / {}: generated form is not in its own mask",
                    lemma.word,
                    form.word
                );
                checked += 1;
            }
        }
    }

    let mut bytes = Vec::with_capacity(8 + masks.len() * MASK_BYTES);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(masks.len() as u32).to_le_bytes());
    for mask in &masks {
        for word in mask {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    torille_tools::gzip::write(Path::new("maps/lemma_stem_shape.bin.gz"), &bytes);

    let raw = bytes.len();
    let average = bits as f64 / masks.len().max(1) as f64;
    println!(
        "lemma stem shapes: {} rows ({} classless), {:.1} bits/row, {} bytes raw, {} forms spot-checked",
        masks.len(),
        classless,
        average,
        raw,
        checked
    );
}
