//! Precompute the noun lexicon at build time.
//!
//! `data/nykysuomensanalista.csv` is 2.8 MB of tab-separated rows, and parsing it
//! into `Lemma` costs ~31 ms and ~420 000 allocations on every process start.
//! That is a real cost for a command-line tool that inflects one word, so the
//! parse happens here instead and the result is linked in as data.
//!
//! What is emitted:
//!
//! * one binary blob (`data/precomputed/sanalista.bin`) holding the
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

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[path = "build/huffman.rs"]
mod huffman;
#[path = "build/sanalista.rs"]
mod sanalista;
#[path = "build/front_coded.rs"]
mod front_coded;

use front_coded::{
    emit_front_coded_set, emit_proper_name_lemmas, emit_proper_names, emit_stem_map,
    write_front_coded_map,
};
use sanalista::{generate, push_blob_string};

#[path = "src/rejected.rs"]
mod rejected;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let data = Path::new(&manifest).join("data/nykysuomensanalista.csv");
    println!("cargo:rerun-if-changed={}", data.display());

    let text = std::fs::read_to_string(&data).unwrap_or_else(|e| panic!("{}: {e}", data.display()));
    let blob = generate(&text);

    // The indexed lexicon is written to `data/precomputed/` in the crate
    // root rather than `OUT_DIR`, so a fresh `cargo build` (or one from
    // CI) just `include_bytes!`s the same file again -- no 1-2 min regen
    // unless the sanalista changes. `cargo:rerun-if-changed` keeps it in
    // step with `nykysuomensanalista.csv`.
    let precomputed = Path::new(&manifest).join("data/precomputed");
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
    // `tools/build_catalogue_csv.py` from the Omorfi/Voikko oracles, so the
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
    // `tools/build_verb_lexicon.py`.
    let conneg_csv = Path::new(&manifest).join("data/verb_connegatives.csv");
    println!("cargo:rerun-if-changed={}", conneg_csv.display());
    emit_connegatives(&conneg_csv, &precomputed.join("connegatives.bin"));

    // Negation fusions (`enkä`, `ettet`) and the finite auxiliaries:
    // form -> lemma, written by `tools/build_verb_lexicon.py`. The same
    // data the `connectives` table holds.
    let connectives_csv = Path::new(&manifest).join("data/verb_connectives.csv");
    println!("cargo:rerun-if-changed={}", connectives_csv.display());
    emit_connectives(&connectives_csv, &precomputed.join("connectives.bin"));
}

/// Emit a tab-separated CSV as a front-coded `StaticMap` blob.
///
/// `key_col` and `value_cols` are zero-based; the first CSV row is a header.
/// The first row for a key wins. A row whose requested values are all empty
/// is dropped (an adjective with no derived degree, for example).
fn emit_string_map(
    csv_path: &Path,
    blob_path: &Path,
    label: &str,
    tool: &str,
    key_col: usize,
    value_cols: &[usize],
) {
    if !csv_path.exists() && blob_path.exists() {
        eprintln!(
            "build.rs: {} missing but {} present -- reusing",
            csv_path.display(),
            blob_path.display(),
        );
        return;
    }
    let csv = fs::read_to_string(csv_path).unwrap_or_else(|e| {
        panic!(
            "build.rs: could not read {}: {e}; \
             run `python3 tools/{tool}` first",
            csv_path.display(),
        )
    });
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        let Some(key) = cols.get(key_col).map(|s| s.trim()) else {
            continue;
        };
        if key.is_empty() || key.contains('\0') || map.contains_key(key) {
            continue;
        }
        let values: Vec<String> = value_cols
            .iter()
            .map(|&c| {
                cols.get(c)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()
            })
            .collect();
        if values.iter().all(String::is_empty) {
            continue;
        }
        map.insert(key.to_string(), values);
    }
    let entries: Vec<(String, Vec<String>)> = map.into_iter().collect();
    eprintln!(
        "build.rs: {label}: {} keys from {} rows",
        entries.len(),
        csv.lines().count().saturating_sub(1),
    );
    write_front_coded_map(blob_path, &entries);
}

/// Emit `data/verb_connegatives.csv` as a `form -> lemma` map.
///
/// The CSV is `surface<TAB>base<TAB>mood<TAB>voice`, with a header.
fn emit_connegatives(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "connegatives",
        "build_verb_lexicon.py",
        0,
        &[1],
    );
}

/// Emit `data/verb_connectives.csv` as a `form -> lemma` map.
///
/// The CSV is `surface<TAB>base<TAB>kind`, with a header.
fn emit_connectives(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "connectives",
        "build_verb_lexicon.py",
        0,
        &[1],
    );
}

/// Emit `data/adverbit.csv` as a `form -> lemma` map.
///
/// The CSV is `lemma<TAB>form`, so the key is the second column.
fn emit_adverbi(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "adverbit",
        "build_adverbi_forms.py",
        1,
        &[0],
    );
}

/// Emit `data/adj_degrees.csv` as `positive -> (comparative, superlative)`.
///
/// Both degree cells are stored even when one is empty, so the runtime can
/// read the pair positionally.
fn emit_adj_degrees(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "adj_degrees",
        "build_adj_degrees.py",
        0,
        &[1, 2],
    );
}

/// Emit the paradigm catalogue as a binary blob from `data/catalogue.csv`.
///
/// CSV layout (tab-separated, one row per form):
///
/// ```text
/// class  slot  form  optional
/// 9      0     kala  0
/// 9      5     kalojen 0
/// 9      5     kalain  1
/// ```
///
/// For each `(class, slot)` the first row is the model form, the rows after it
/// are alternants, and `optional` marks an alternant the oracle half-confirms.
/// The blob format is read back by [`crate::spec::Catalogues::from_bytes`]
/// (documented there).
///
/// **Reuse path**: if the CSV is missing but the blob is present, skip -- the
/// same contract as the stem maps and `all_forms`, so a clone without the
/// generator's output still builds.
fn emit_catalogue(csv_path: &Path, blob_path: &Path) {
    if !csv_path.exists() && blob_path.exists() {
        eprintln!(
            "build.rs: {} missing but precomputed blob present -- reusing",
            csv_path.display(),
        );
        return;
    }
    let csv = fs::read_to_string(csv_path).unwrap_or_else(|e| {
        panic!(
            "build.rs: could not read {}: {e}; \
             run `python3 tools/build_catalogue_csv.py` first",
            csv_path.display(),
        )
    });

    // class -> slot -> (model + alternants in order, optional set).
    let mut classes: BTreeMap<u16, BTreeMap<u16, Vec<(String, bool)>>> = BTreeMap::new();
    let mut rows = 0usize;
    for line in csv.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let mut f = line.split('\t');
        let (Some(class), Some(slot), Some(form), Some(optional)) =
            (f.next(), f.next(), f.next(), f.next())
        else {
            continue;
        };
        let Some(class) = class.trim().parse::<u16>().ok() else {
            continue;
        };
        let Some(slot) = slot.trim().parse::<u16>().ok() else {
            continue;
        };
        if form.is_empty() || form.contains('\0') {
            continue;
        }
        classes
            .entry(class)
            .or_default()
            .entry(slot)
            .or_default()
            .push((form.to_string(), optional.trim() == "1"));
        rows += 1;
    }
    assert!(!classes.is_empty(), "catalogue.csv parsed to no classes");

    // Serialise: header + body (records) + string blob.
    let mut strings: Vec<u8> = Vec::new();
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&(classes.len() as u32).to_le_bytes());
    for (class, slots) in &classes {
        body.extend_from_slice(&class.to_le_bytes());
        body.extend_from_slice(&(slots.len() as u32).to_le_bytes());
        for forms in slots.values() {
            let Some((model, rest)) = forms.split_first() else {
                continue;
            };
            push_blob_string(&mut strings, &mut body, &model.0);
            body.extend_from_slice(&(rest.len() as u32).to_le_bytes());
            for (form, _) in rest {
                push_blob_string(&mut strings, &mut body, form);
            }
            let optional = rest.iter().filter(|(_, o)| *o).count();
            body.extend_from_slice(&(optional as u32).to_le_bytes());
            for (form, o) in rest {
                if *o {
                    push_blob_string(&mut strings, &mut body, form);
                }
            }
        }
    }

    let mut out = Vec::with_capacity(16 + body.len() + strings.len());
    out.extend_from_slice(b"TCAT");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(&strings);
    fs::write(blob_path, &out).unwrap_or_else(|e| panic!("{}: {e}", blob_path.display()));
    eprintln!(
        "build.rs: catalogue: {} classes, {} CSV rows -> {} blob",
        classes.len(),
        rows,
        human_bytes(out.len()),
    );
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
