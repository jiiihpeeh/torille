//! Generic tab-separated CSV -> front-coded `StaticMap` writers.
//!
//! Used for the small lexical side tables (`connegatives.bin`,
//! `connectives.bin`, `adverbit.bin`, `adj_degrees.bin`). The CSV parsers
//! here are deliberately forgiving: a missing value column is empty, and a
//! row whose values are all empty is dropped.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::map_builders::front_coded::write_front_coded_map;

/// Emit a tab-separated CSV as a front-coded `StaticMap` blob.
///
/// `key_col` and `value_cols` are zero-based; the first CSV row is a header.
/// The first row for a key wins. A row whose requested values are all empty
/// is dropped (an adjective with no derived degree, for example).
pub(crate) fn emit_string_map(
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
pub(crate) fn emit_connegatives(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "connegatives",
        "build/build_verb_lexicon.py",
        0,
        &[1],
    );
}

/// Emit `data/verb_connectives.csv` as a `form -> lemma` map.
///
/// The CSV is `surface<TAB>base<TAB>kind`, with a header.
pub(crate) fn emit_connectives(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "connectives",
        "build/build_verb_lexicon.py",
        0,
        &[1],
    );
}

/// Emit `data/adverbit.csv` as a `form -> lemma` map.
///
/// The CSV is `lemma<TAB>form`, so the key is the second column.
pub(crate) fn emit_adverbi(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "adverbit",
        "build/build_adverbi_forms.py",
        1,
        &[0],
    );
}

/// Emit `data/adj_degrees.csv` as `positive -> (comparative, superlative)`.
///
/// Both degree cells are stored even when one is empty, so the runtime can
/// read the pair positionally.
pub(crate) fn emit_adj_degrees(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "adj_degrees",
        "build/build_adj_degrees.py",
        0,
        &[1, 2],
    );
}

