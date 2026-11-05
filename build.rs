//! Precompute the noun lexicon at build time.
//!
//! `data/nykysuomensanalista.csv` is 2.8 MB of tab-separated rows, and parsing it
//! into `Lemma` costs ~31 ms and ~420 000 allocations on every process start.
//! That is a real cost for a command-line tool that inflects one word, so the
//! parse happens here instead and the result is linked in as data.
//!
//! What is emitted:
//!
//! * one binary blob (`maps/sanalista.bin`) holding the
//!   per-row records, the packed strings, the inflections as a flat `u32`
//!   array (class in the high 16 bits, gradation code in the low 8), and
//!   `WORD_ORDER` / `TAIL_ORDER` (row indices sorted by headword and by
//!   reversed headword);
//! * nothing else for the lexicon. `kotus.rs` binary-searches `WORD_ORDER`
//!   for `by_word`, so there is no phf literal and no second copy of the
//!   words in the binary.
//!
//! The blob is checked in and reused when the input text is unchanged, so a
//! fresh `cargo build` only `include_bytes!`s it. `cargo:rerun-if-changed`
//! keeps it in step with the data file.

use std::path::Path;

// A build script's `mod` declarations resolve next to `build.rs`, not in
// `build/`, so the two writer trees are reached with `#[path]`.
#[path = "build/map_builders/mod.rs"]
mod map_builders;
#[path = "build/csv_builders/mod.rs"]
mod csv_builders;

use csv_builders::catalogue::emit_catalogue;
use csv_builders::sanalista::generate;
use csv_builders::string_map::{
    emit_adj_degrees, emit_adverbi, emit_connegatives, emit_connectives,
};
use map_builders::front_coded::{
    emit_front_coded_set, emit_proper_name_lemmas, emit_proper_names, emit_stem_map,
};

#[path = "src/rejected.rs"]
mod rejected;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let data = Path::new(&manifest).join("data/nykysuomensanalista.csv");
    println!("cargo:rerun-if-changed={}", data.display());

    let text = std::fs::read_to_string(&data).unwrap_or_else(|e| panic!("{}: {e}", data.display()));
    let blob = generate(&text);

    // The indexed lexicon is written to `maps/` in the crate
    // root rather than `OUT_DIR`, so a fresh `cargo build` (or one from
    // CI) just `include_bytes!`s the same file again -- no 1-2 min regen
    // unless the sanalista changes. `cargo:rerun-if-changed` keeps it in
    // step with `nykysuomensanalista.csv`.
    let precomputed = Path::new(&manifest).join("maps");
    std::fs::create_dir_all(&precomputed)
        .unwrap_or_else(|e| panic!("create_dir_all {}: {e}", precomputed.display()));
    let blob_path = precomputed.join("sanalista.bin");
    std::fs::write(&blob_path, &blob)
        .unwrap_or_else(|e| panic!("write {}: {e}", blob_path.display()));
    eprintln!(
        "build.rs: sanalista blob: {} bytes -> {}",
        blob.len(),
        blob_path.display(),
    );

    // Compile-time subset of the form set: lemmas (nominative singular)
    // plus the genitive singular of every lemma, both without possessives.
    // It is emitted as a front-coded set blob, not a Rust literal -- the
    // same set as a `phf::Set` was tens of MB of source and duplicated
    // every key. The wasm's `find_misspells` binary-searches it on load;
    // anything missing falls through to the slow check
    // (`find_bases_with`).
    let forms_csv = Path::new(&manifest).join("data/all_forms.csv");
    println!("cargo:rerun-if-changed={}", forms_csv.display());
    emit_front_coded_set(&forms_csv, &precomputed.join("all_forms.bin"));

    // Proper names: Wikidata supplies the lemmas, Omorfi the singular cases.
    // The fast path accepts these forms; they are never suggested.
    let proper_csv = Path::new(&manifest).join("data/proper_name_forms.csv");
    let proper_extra = Path::new(&manifest).join("data/proper_name_forms_extra.csv");
    println!("cargo:rerun-if-changed={}", proper_csv.display());
    println!("cargo:rerun-if-changed={}", proper_extra.display());
    emit_proper_names(&proper_csv, &precomputed.join("proper_names.bin"));
    emit_proper_name_lemmas(
        &proper_csv,
        &proper_extra,
        &precomputed.join("proper_name_lemmas.bin"),
    );

    // Stem maps: `(stem, bases)` pairs from
    // `data/stem_to_base_map.csv` (nouns) and
    // `data/verb_stem_to_base_map.csv` (verbs). Each is emitted as one
    // binary blob: a sorted, front-coded key region, a `(start, count)`
    // value table, and a NUL-separated bases region. See
    // `src/expand/static_table.rs` for the format. Emitting the keys as
    // a blob (not a rust `phf` literal) is what keeps the build fast --
    // the previous noun literal alone was ~31 MB / 900 k lines of source
    // and took tens of minutes for rustc to parse. Runtime probe: a
    // binary search over the front-coded keys, then a walk over the
    // `(start, count)` base range.
    let noun_csv = Path::new(&manifest).join("data/stem_to_base_map.csv");
    let verb_csv = Path::new(&manifest).join("data/verb_stem_to_base_map.csv");
    println!("cargo:rerun-if-changed={}", noun_csv.display());
    println!("cargo:rerun-if-changed={}", verb_csv.display());
    emit_stem_map(&noun_csv, &precomputed.join("stem_map_noun.bin"));
    emit_stem_map(&verb_csv, &precomputed.join("stem_map_verb.bin"));

    // Paradigm templates: `data/catalogue.csv` (class, slot, form, optional)
    // -> a binary `class -> slots` map. The CSV is generated by
    // `tools/build/build_catalogue_csv.py` from the Omorfi/Voikko oracles, so the
    // Kotus taivutustyypit document is never read here. The runtime reads the
    // blob through `spec::Catalogues::embedded`; nothing parses text.
    let catalogue_csv = Path::new(&manifest).join("data/catalogue.csv");
    println!("cargo:rerun-if-changed={}", catalogue_csv.display());
    emit_catalogue(&catalogue_csv, &precomputed.join("catalogue.bin"));

    // Adjective degrees: `positive -> (comparative, superlative)`
    // nominatives, derived by Omorfi. The engine emits these two
    // paradigms from the positive so the derivation is available to
    // `inflect`/`find_bases` and the stem-map verify.
    let degrees_csv = Path::new(&manifest).join("data/adj_degrees.csv");
    println!("cargo:rerun-if-changed={}", degrees_csv.display());
    emit_adj_degrees(&degrees_csv, &precomputed.join("adj_degrees.bin"));

    // Adverbial forms: the `-ksi` adverbs' possessives (`kylliksi` ->
    // `kylliksenne`), which Omorfi cannot generate. A form -> lemma map.
    let adverbit_csv = Path::new(&manifest).join("data/adverbit.csv");
    println!("cargo:rerun-if-changed={}", adverbit_csv.display());
    emit_adverbi(&adverbit_csv, &precomputed.join("adverbit.bin"));

    // Verb connegatives: the bare potential/imperative connegative
    // (`syöne`, `ajatelko`), which the oracle holds only inside the
    // negative phrase (`en syöne`). A form -> lemma map, written by
    // `tools/build/build_verb_lexicon.py`.
    let conneg_csv = Path::new(&manifest).join("data/verb_connegatives.csv");
    println!("cargo:rerun-if-changed={}", conneg_csv.display());
    emit_connegatives(&conneg_csv, &precomputed.join("connegatives.bin"));

    // Negation fusions (`enkä`, `ettet`) and the finite auxiliaries:
    // form -> lemma, written by `tools/build/build_verb_lexicon.py`. The same
    // data the `connectives` table holds.
    let connectives_csv = Path::new(&manifest).join("data/verb_connectives.csv");
    println!("cargo:rerun-if-changed={}", connectives_csv.display());
    emit_connectives(&connectives_csv, &precomputed.join("connectives.bin"));
}


/// Append `v` as LEB128 to `out`.
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

/// Format a byte count for `eprintln!` output. `human_bytes(1234)`
/// → `"1.2 KiB"`. Doesn't pretend to be more precise than it is.
fn human_bytes(n: usize) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} {}", UNITS[0])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}
