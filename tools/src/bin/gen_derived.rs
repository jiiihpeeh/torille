// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_derived` — write the derived-form tables the engine embeds.
//!
//! `src/expand/derived.rs` embeds three sorted blobs:
//! `derived_by_form.bin` (surface -> derived lemma), `derived_nominal.bin` and
//! `derived_modifiers.bin`. They are built here once and committed under
//! `maps/` because computing them at start-up
//! costs ~85 ms: the 55 flagged sanalista rows inflect to 42 532 surfaces.
//!
//! Run after a sanalista or `RULES` change:
//!
//!     cargo run --release -p torille-tools --bin gen_derived
//!
//! The format is described on `Blob` in `src/expand/blob.rs`; the writer is
//! `maps/table.rs`, shared with the other blob writers.

use std::path::Path;

use torille_tools::der1::write_table;

fn main() {
    let computed = torille::expand::derived::compute();

    let mut by_form: Vec<(String, &'static str)> = computed.by_form.into_iter().collect();
    by_form.sort_by(|a, b| a.0.cmp(&b.0));
    write_table(
        Path::new("maps/derived_by_form.bin.gz"),
        by_form.iter().map(|(surface, lemma)| (surface.as_str(), Some(*lemma))),
    );

    let mut nominal: Vec<&str> = computed.nominal.iter().map(String::as_str).collect();
    nominal.sort_unstable();
    write_table(
        Path::new("maps/derived_nominal.bin.gz"),
        nominal.iter().map(|word| (*word, None)),
    );

    let mut modifiers: Vec<&str> = computed.modifiers.iter().map(String::as_str).collect();
    modifiers.sort_unstable();
    write_table(
        Path::new("maps/derived_modifiers.bin.gz"),
        modifiers.iter().map(|word| (*word, None)),
    );

    // The closed pronoun class (Kotus 101): scanning the 104k lemma rows to
    // find its 32 lemmas costs ~20 ms on the first unknown word of a process.
    let mut pronouns: Vec<(String, &'static str)> =
        torille::expand::check_normal::compute_pronoun_forms()
            .into_iter()
            .collect();
    pronouns.sort_by(|a, b| a.0.cmp(&b.0));
    write_table(
        Path::new("maps/pronoun_forms.bin.gz"),
        pronouns
            .iter()
            .map(|(surface, lemma)| (surface.as_str(), Some(*lemma))),
    );

    println!(
        "derived: {} forms, {} nominal, {} modifiers; {} pronouns",
        by_form.len(),
        nominal.len(),
        modifiers.len(),
        pronouns.len()
    );
}
