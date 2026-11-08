//! Precompute the noun lexicon at build time.
//!
//! `data/nykysuomensanalista.csv` is 2.8 MB of tab-separated rows, and parsing it
//! into `Lemma` costs ~31 ms and ~420 000 allocations on every process start.
//! That is a real cost for a command-line tool that inflects one word, so the
//! parse happens here instead and the result is linked in as data.
//!
//! What is emitted:
//!
//! * one binary blob (`maps/sanalista.bin.gz`) holding the
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

#[path = "build/hyphenation.rs"]
mod hyphenation;

#[path = "build/gzip.rs"]
mod gzip;

// The same std-only implementation the `torille-maps` CLI uses.
use torille_maps as maps;

use csv_builders::catalogue::emit_catalogue;
use csv_builders::sanalista::generate;
use csv_builders::string_map::{
    emit_adj_degrees, emit_adverbi, emit_connegatives, emit_connectives,
};
use map_builders::front_coded::{
    emit_empty_set, emit_front_coded_set, emit_proper_name_lemmas, emit_proper_names,
    emit_stem_map,
};

#[path = "src/rejected.rs"]
mod rejected;
#[path = "src/sanalista_overrides.rs"]
mod sanalista_overrides;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let data = Path::new(&manifest).join("data/nykysuomensanalista.csv");
    println!("cargo:rerun-if-changed={}", data.display());

    // Hyphenation patterns are vendored TeX. Parse them at build time into
    // a static trie; the binary has no pattern parser at run time.
    let tex = Path::new(&manifest).join("data/hyphenation/hyph-fi-x-school.tex");
    println!("cargo:rerun-if-changed={}", tex.display());
    let exceptions = Path::new(&manifest).join("data/hyphenation/exceptions.csv");
    println!("cargo:rerun-if-changed={}", exceptions.display());
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    hyphenation::emit(&tex, &Path::new(&out_dir).join("hyphenation_patterns.rs"));
    hyphenation::emit_exceptions(
        &exceptions,
        &Path::new(&out_dir).join("hyphenation_exceptions.rs"),
    );

    let text = std::fs::read_to_string(&data).unwrap_or_else(|e| panic!("{}: {e}", data.display()));
    let blob = generate(&text);

    // The uncompressed maps live in `OUT_DIR` and are what `include_bytes!`
    // embeds; the committed copies under `maps/` are gzip-compressed at the
    // maximum level purely to keep the repository and the generated artefacts
    // small. Compression is tooling only: the compiler never sees gzip, and
    // the runtime has no decompressor. `cargo:rerun-if-changed` keeps the
    // compressed copies in step with their CSV/engine sources.
    let out_dir = Path::new(&out_dir).to_path_buf();
    let precomputed = Path::new(&manifest).join("maps");
    std::fs::create_dir_all(&precomputed)
        .unwrap_or_else(|e| panic!("create_dir_all {}: {e}", precomputed.display()));

    // These tables are computed with the engine, so a build script cannot
    // regenerate them. They live on the orphan `maps` branch; `master` tracks
    // only `maps/README.md`. Fetch them on first build unless the user asked
    // for an offline build.
    let root = Path::new(&manifest);
    if !maps::missing(root).is_empty() {
        let offline = std::env::var_os("CARGO_NET_OFFLINE").is_some()
            || std::env::var_os("TORILLE_NO_FETCH").is_some()
            || std::env::var_os("DOCS_RS").is_some();
        if offline {
            eprintln!(
                "build.rs: binary maps are missing and fetching is disabled \
                 (CARGO_NET_OFFLINE/TORILLE_NO_FETCH/DOCS_RS)"
            );
        } else {
            let remote = maps::remote();
            match maps::fetch(root, &remote) {
                Ok(count) => {
                    eprintln!("build.rs: fetched {count} blobs from {remote}/maps")
                }
                Err(err) => eprintln!("build.rs: maps fetch failed: {err}"),
            }
        }
    }
    for required in maps::REQUIRED {
        let path = precomputed.join(required);
        assert!(
            path.exists(),
            "{} is missing: the binary maps live on the 'maps' branch; run \
             `cargo run -q -p torille-maps -- fetch` (or unset \
             TORILLE_NO_FETCH to let the build fetch them)",
            path.display(),
        );
    }

    let sanalista_raw = out_dir.join("sanalista.bin");
    std::fs::write(&sanalista_raw, &blob)
        .unwrap_or_else(|e| panic!("write {}: {e}", sanalista_raw.display()));
    gzip::write(&precomputed.join("sanalista.bin.gz"), &blob);

    // Compile-time subset of the form set: lemmas (nominative singular)
    // plus the genitive singular of every lemma, both without possessives.
    // It is emitted as a front-coded set blob, not a Rust literal -- the
    // same set as a `phf::Set` was tens of MB of source and duplicated
    // every key. The wasm's `find_misspells` binary-searches it on load;
    // anything missing falls through to the slow check
    // (`find_bases_with`).
    let forms_csv = Path::new(&manifest).join("data/all_forms.csv");
    println!("cargo:rerun-if-changed={}", forms_csv.display());
    let forms_raw = out_dir.join("all_forms.bin");
    let forms_gz = precomputed.join("all_forms.bin.gz");
    if forms_csv.exists() {
        emit_front_coded_set(&forms_csv, &forms_raw);
    } else if forms_gz.exists() {
        // A fresh clone has no `data/all_forms.csv` (it is generated), but
        // the committed set is on the `maps` branch and was just fetched.
        eprintln!(
            "build.rs: {} missing -- reusing {}",
            forms_csv.display(),
            forms_gz.display(),
        );
        gzip::decompress_file(&forms_gz, &forms_raw);
    } else {
        // Bootstrap so `gen_all_forms` can be built and run.
        eprintln!(
            "build.rs: {} missing and no committed set -- emitting an empty form set; \
             run `cargo run --release -p torille-tools --bin gen_all_forms`",
            forms_csv.display(),
        );
        emit_empty_set(&forms_raw);
    }
    gzip::compress_file(&forms_raw, &forms_gz);

    // Proper names: Wikidata supplies the lemmas, Omorfi the singular cases.
    // The fast path accepts these forms; they are never suggested.
    let proper_csv = Path::new(&manifest).join("data/proper_name_forms.csv");
    let proper_extra = Path::new(&manifest).join("data/proper_name_forms_extra.csv");
    println!("cargo:rerun-if-changed={}", proper_csv.display());
    println!("cargo:rerun-if-changed={}", proper_extra.display());
    emit_map(&out_dir, &precomputed, "proper_names.bin", |raw| {
        emit_proper_names(&proper_csv, raw)
    });
    emit_map(&out_dir, &precomputed, "proper_name_lemmas.bin", |raw| {
        emit_proper_name_lemmas(&proper_csv, &proper_extra, raw)
    });

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
    emit_map(&out_dir, &precomputed, "stem_map_noun.bin", |raw| {
        emit_stem_map(&noun_csv, raw)
    });
    emit_map(&out_dir, &precomputed, "stem_map_verb.bin", |raw| {
        emit_stem_map(&verb_csv, raw)
    });

    // Paradigm templates: `data/catalogue.csv` (class, slot, form, optional)
    // -> a binary `class -> slots` map. The CSV is generated by
    // `tools/oracle/build_catalogue_csv.py` from the Omorfi/Voikko oracles, so the
    // Kotus taivutustyypit document is never read here. The runtime reads the
    // blob through `spec::Catalogues::embedded`; nothing parses text.
    let catalogue_csv = Path::new(&manifest).join("data/catalogue.csv");
    println!("cargo:rerun-if-changed={}", catalogue_csv.display());
    emit_map(&out_dir, &precomputed, "catalogue.bin", |raw| {
        emit_catalogue(&catalogue_csv, raw)
    });

    // Adjective degrees: `positive -> (comparative, superlative)`
    // nominatives, derived by the engine. The engine emits these two
    // paradigms from the positive so the derivation is available to
    // `inflect`/`find_bases` and the stem-map verify.
    let degrees_csv = Path::new(&manifest).join("data/adj_degrees.csv");
    println!("cargo:rerun-if-changed={}", degrees_csv.display());
    emit_map(&out_dir, &precomputed, "adj_degrees.bin", |raw| {
        emit_adj_degrees(&degrees_csv, raw)
    });

    // Adverb degrees: the curated comparative/superlative of an adverb
    // (`hiljaa` -> `hiljempaa`). Same shape as the adjective table; the
    // engine cannot derive these from a positive paradigm.
    let adverb_degrees_csv = Path::new(&manifest).join("data/adverb_degrees.csv");
    println!("cargo:rerun-if-changed={}", adverb_degrees_csv.display());
    emit_map(&out_dir, &precomputed, "adverb_degrees.bin", |raw| {
        emit_adj_degrees(&adverb_degrees_csv, raw)
    });

    // Adverbial forms: the `-ksi` adverbs' possessives (`kylliksi` ->
    // `kylliksenne`), which Omorfi cannot generate. A form -> lemma map.
    let adverbit_csv = Path::new(&manifest).join("data/adverbit.csv");
    println!("cargo:rerun-if-changed={}", adverbit_csv.display());
    emit_map(&out_dir, &precomputed, "adverbit.bin", |raw| {
        emit_adverbi(&adverbit_csv, raw)
    });

    // Verb connegatives: the bare potential/imperative connegative
    // (`syöne`, `ajatelko`), which the oracle holds only inside the
    // negative phrase (`en syöne`). A form -> lemma map, written by
    // `tools/oracle/build_verb_lexicon.py`.
    let conneg_csv = Path::new(&manifest).join("data/verb_connegatives.csv");
    println!("cargo:rerun-if-changed={}", conneg_csv.display());
    emit_map(&out_dir, &precomputed, "connegatives.bin", |raw| {
        emit_connegatives(&conneg_csv, raw)
    });

    // Negation fusions (`enkä`, `ettet`) and the finite auxiliaries:
    // form -> lemma, written by `tools/oracle/build_verb_lexicon.py`. The same
    // data the `connectives` table holds.
    let connectives_csv = Path::new(&manifest).join("data/verb_connectives.csv");
    println!("cargo:rerun-if-changed={}", connectives_csv.display());
    emit_map(&out_dir, &precomputed, "connectives.bin", |raw| {
        emit_connectives(&connectives_csv, raw)
    });

    // These four tables are computed with the engine, not by this script
    // (`gen_derived` and the oracle tool). Their committed `.bin.gz` is the
    // source; unpack it into `OUT_DIR` for `include_bytes!`.
    for name in [
        "derived_by_form.bin",
        "derived_nominal.bin",
        "derived_modifiers.bin",
        "pronoun_forms.bin",
        "degree_forms.bin",
        "adverb_possessive.bin",
        "oi_plural_lemmas.bin",
    ] {
        let gz = precomputed.join(format!("{name}.gz"));
        println!("cargo:rerun-if-changed={}", gz.display());
        gzip::decompress_file(&gz, &out_dir.join(name));
    }
}

/// Emit one map uncompressed into `OUT_DIR`, then gzip the committed copy.
fn emit_map(
    out_dir: &Path,
    precomputed: &Path,
    name: &str,
    emit: impl FnOnce(&Path),
) {
    let raw = out_dir.join(name);
    emit(&raw);
    gzip::compress_file(&raw, &precomputed.join(format!("{name}.gz")));
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
