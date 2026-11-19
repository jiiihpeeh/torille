// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_lemma_stem_shapes` — write the per-lemma stem-shape masks.
//!
//! `src/expand/stem_shape.rs` embeds `maps/lemma_stem_shape.bin.gz`: one
//! 256-bit prefix mask and one 64-bit suffix mask per lexicon row. A prefix
//! bit hashes the first four characters of a surface the lemma can emit, and
//! the suffix bit hashes the last four, so the recognition walk can reject a
//! candidate before `emits_form` builds its paradigm.
//!
//! The mask is conservative: every form generated here sets its own prefix and
//! suffix bits, and classless rows get an all-ones mask. A false positive costs
//! one `emits_form`; a false negative would lose a word, so the generator
//! self-checks a sample against `emits_form`.
//!
//! Run after a sanalista or class-engine change, then rebuild:
//!
//!     cargo run --release -p torille-tools --bin gen_lemma_stem_shapes
//!     cargo build -p torille

use std::path::Path;

use torille::expand::stem_shape::{prefix_bits, suffix_bits};
use torille::expand::{emits_form, inflect_all, LexiconIndex};
use torille::kotus::lemmas;
use torille::spec::Catalogues;
use torille::tags::PartOfSpeech;

const MAGIC: &[u8; 4] = b"LSS3";
const MASK_BYTES: usize = 40;

fn main() {
    let catalogue = Catalogues::embedded();
    let lexicon = lemmas();
    let index = LexiconIndex::new(lexicon);

    let mut masks: Vec<[u64; 5]> = Vec::with_capacity(lexicon.len());
    let mut classless = 0usize;
    let mut bits = 0u32;
    let mut checked = 0usize;

    for (row, lemma) in lexicon.iter().enumerate() {
        // A classless row's paradigm is assembled by the compound fallbacks.
        // Skip the filter there rather than risk a false negative.
        if lemma.inflections.is_empty() {
            masks.push([u64::MAX; 5]);
            classless += 1;
            continue;
        }

        let mut mask = [0u64; 5];
        for (slot, bit) in mask[..4].iter_mut().zip(prefix_bits(lemma.word)) {
            *slot |= bit;
        }
        mask[4] |= suffix_bits(lemma.word);
        // The callers do not all pass the lexicon index: the stem-map gates
        // use `Some(&index)` for compound heads, while the spell path uses
        // `None`. Generate both sets so the mask is a superset of either.
        let forms_without = inflect_all(
            lemma.word,
            &lemma.inflections,
            catalogue,
            None,
            PartOfSpeech::Noun,
            true,
        );
        let forms_with = inflect_all(
            lemma.word,
            &lemma.inflections,
            catalogue,
            Some(&index),
            PartOfSpeech::Noun,
            true,
        );
        for form in forms_without.iter().chain(forms_with.iter()) {
            if !form.word.is_empty() {
                for (slot, bit) in mask[..4].iter_mut().zip(prefix_bits(&form.word)) {
                    *slot |= bit;
                }
                mask[4] |= suffix_bits(&form.word);
            }
        }
        if mask[..4].iter().all(|word| *word == 0) {
            // Defensive: an empty mask would reject everything.
            mask = [u64::MAX; 5];
        }
        bits += mask.iter().map(|word| word.count_ones()).sum::<u32>();
        masks.push(mask);

        // Spot-check the same probe the search uses, with each lexicon
        // argument. Every 97th lemma keeps the generator fast while covering
        // every class region of the lexicon.
        if row % 97 == 0 {
            for (forms, lexicon) in [
                (&forms_without, None),
                (&forms_with, Some(&index)),
            ] {
                for form in forms {
                    assert!(
                        emits_form(
                            lemma.word,
                            &lemma.inflections,
                            catalogue,
                            lexicon,
                            PartOfSpeech::Noun,
                            true,
                            &form.word,
                        ),
                        "{} / {}: emits_form disagrees with inflect_all",
                        lemma.word,
                        form.word
                    );
                    assert!(
                        mask[..4]
                            .iter()
                            .zip(prefix_bits(&form.word))
                            .all(|(mask, bit)| mask & bit != 0),
                        "{} / {}: generated form is not in its own prefix mask",
                        lemma.word,
                        form.word
                    );
                    assert!(
                        mask[4] & suffix_bits(&form.word) != 0,
                        "{} / {}: generated form is not in its own suffix mask",
                        lemma.word,
                        form.word
                    );
                    checked += 1;
                }
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
