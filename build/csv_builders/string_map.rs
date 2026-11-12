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
             run `{tool}` first",
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
        "python3 tools/oracle/build_verb_lexicon.py",
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
        "python3 tools/oracle/build_verb_lexicon.py",
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
        "python3 tools/oracle/build_adverbi_forms.py",
        1,
        &[0],
    );
}

/// Emit `data/lexical_adverbs.csv` as an exact `surface -> base` map.
///
/// These are established one-word adverbs whose spelling is lexical, not a
/// productive `päin`/compound join (`alhaaltapäin`, `ylhäältäpäin`,
/// `äkkipikaa`). The CSV is `surface<TAB>base` with a header.
pub(crate) fn emit_lexical_adverbs(csv_path: &Path, blob_path: &Path) {
    emit_string_map(
        csv_path,
        blob_path,
        "lexical adverbs",
        "curated (Kotus/Kielikello)",
        0,
        &[1],
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
        "cargo run --release -p torille-tools --bin gen_adj_degrees",
        0,
        &[1, 2],
    );
}

/// Emit one style table as `key -> (kind, replacement)`.
///
/// The CSV is `key,kind,replacement,note`: the note is curation-only and is
/// not shipped. The key is lowercased and whitespace-normalised; the kind is
/// checked against the style layer's closed set so a typo fails the build
/// rather than silently disabling a row. A phrase may be at most four words,
/// matching `style_check::lexicon::MAX_PHRASE_WORDS`.
pub(crate) fn emit_style_table(csv_path: &Path, blob_path: &Path, label: &str) {
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
            "build.rs: could not read {}: {e}",
            csv_path.display(),
        )
    });
    let header = csv.lines().next().unwrap_or_default();
    assert!(
        header.starts_with("phrase,") || header.starts_with("word,"),
        "build.rs: {}: expected a `phrase,kind,replacement,note` or \
         `word,kind,replacement,note` header",
        csv_path.display(),
    );
    let mut entries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in csv.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.splitn(4, ',');
        let Some(key) = fields.next().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let key = key
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let Some(kind) = fields.next().map(str::trim).filter(|s| !s.is_empty()) else {
            panic!(
                "build.rs: {}: `{key}` has no style kind",
                csv_path.display()
            );
        };
        if !matches!(kind, "cliche" | "filler" | "wordy" | "colloquial") {
            panic!(
                "build.rs: {}: `{key}` has unknown style kind `{kind}`",
                csv_path.display()
            );
        }
        let replacement = fields.next().map(str::trim).unwrap_or_default();
        if entries.contains_key(&key) {
            panic!(
                "build.rs: {}: duplicate style key `{key}`",
                csv_path.display()
            );
        }
        if key.split(' ').count() > 4 {
            panic!(
                "build.rs: {}: `{key}` is longer than four words",
                csv_path.display()
            );
        }
        entries.insert(key, vec![kind.to_string(), replacement.to_string()]);
    }
    eprintln!(
        "build.rs: {label}: {} keys from {} rows",
        entries.len(),
        csv.lines().count().saturating_sub(1),
    );
    let entries: Vec<(String, Vec<String>)> = entries.into_iter().collect();
    write_front_coded_map(blob_path, &entries);
}

