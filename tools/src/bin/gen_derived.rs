// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_derived` — write the derived-form tables the engine embeds.
//!
//! `src/expand/derived.rs` embeds three sorted blobs:
//! `derived_by_form.bin` (surface -> derived lemma), `derived_nominal.bin` and
//! `derived_modifiers.bin`. They are built here once and committed, like the
//! other `data/precomputed/*.bin` files, because computing them at start-up
//! costs ~85 ms: the 55 flagged sanalista rows inflect to 42 532 surfaces.
//!
//! Run after a sanalista or `RULES` change:
//!
//!     cargo run --release -p torille-tools --bin gen_derived
//!
//! The format is described on `Blob` in `src/expand/derived.rs`.

use std::path::Path;

fn main() {
    let computed = torille::expand::derived::compute();

    let mut by_form: Vec<(String, &'static str)> = computed.by_form.into_iter().collect();
    by_form.sort_by(|a, b| a.0.cmp(&b.0));
    write(
        Path::new("data/precomputed/derived_by_form.bin"),
        by_form.iter().map(|(surface, lemma)| (surface.as_str(), Some(*lemma))),
    );

    let mut nominal: Vec<&str> = computed.nominal.iter().map(String::as_str).collect();
    nominal.sort_unstable();
    write(
        Path::new("data/precomputed/derived_nominal.bin"),
        nominal.iter().map(|word| (*word, None)),
    );

    let mut modifiers: Vec<&str> = computed.modifiers.iter().map(String::as_str).collect();
    modifiers.sort_unstable();
    write(
        Path::new("data/precomputed/derived_modifiers.bin"),
        modifiers.iter().map(|word| (*word, None)),
    );

    // The closed pronoun class (Kotus 101): scanning the 104k lemma rows to
    // find its 32 lemmas costs ~20 ms on the first unknown word of a process.
    let mut pronouns: Vec<(String, &'static str)> =
        torille::expand::check_normal::compute_pronoun_forms()
            .into_iter()
            .collect();
    pronouns.sort_by(|a, b| a.0.cmp(&b.0));
    write(
        Path::new("data/precomputed/pronoun_forms.bin"),
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

/// Writes one sorted table.
///
/// Layout (little-endian): `"DER1"`, `count`, `keys_len`, `vals_len`, the
/// `count + 1` key offsets, the `count + 1` value offsets when the table has
/// values, the key bytes, then the value bytes. The runtime reads it with
/// `Blob::load`.
fn write<'a>(path: &Path, rows: impl Iterator<Item = (&'a str, Option<&'a str>)>) {
    let rows: Vec<(&str, Option<&str>)> = rows.collect();
    let keys_len: usize = rows.iter().map(|(key, _)| key.len()).sum();
    let vals_len: usize = rows
        .iter()
        .map(|(_, value)| value.map_or(0, str::len))
        .sum();
    let mut out = Vec::with_capacity(16 + (rows.len() + 1) * 8 + keys_len + vals_len);
    out.extend_from_slice(b"DER1");
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&(keys_len as u32).to_le_bytes());
    out.extend_from_slice(&(vals_len as u32).to_le_bytes());

    let mut at = 0u32;
    out.extend_from_slice(&at.to_le_bytes());
    for (key, _) in &rows {
        at += key.len() as u32;
        out.extend_from_slice(&at.to_le_bytes());
    }
    if vals_len > 0 {
        let mut at = 0u32;
        out.extend_from_slice(&at.to_le_bytes());
        for (_, value) in &rows {
            at += value.map_or(0, |v| v.len()) as u32;
            out.extend_from_slice(&at.to_le_bytes());
        }
    }
    for (key, _) in &rows {
        out.extend_from_slice(key.as_bytes());
    }
    for (_, value) in &rows {
        if let Some(value) = value {
            out.extend_from_slice(value.as_bytes());
        }
    }
    std::fs::write(path, &out).unwrap_or_else(|exc| panic!("{}: {exc}", path.display()));
}
