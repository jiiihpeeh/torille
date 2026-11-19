// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_form_table` — write `maps/form_table.bin.gz` from the engine.
//!
//! The table is the stem map plus the dimension it lacks: for every
//! `(stem, ending)` the generator saw, which bases spell it and with what
//! readings. It is what lets the resolver answer "does this base emit this
//! surface" without building the paradigm, and "what does it read as" without
//! walking it again. See `misc_md/PLAN_FORM_TABLE.md`.
//!
//! The split is the runtime's own: the longest prefix of the form that is a
//! `(stem, base)` pair in the committed stem maps, with the remainder in the
//! ending list that map's lookup strips.
//!
//! Run after a `RULES`, sanalista or class-engine change, then rebuild:
//!
//!     cargo run --release -p torille-tools --bin gen_form_table
//!     cargo build --features form-table

use std::collections::{BTreeMap, BTreeSet, HashMap};

use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;

use torille::expand::check_normal::{ENDINGS, POSSESSIVE_SUFFIXES, VERB_ENDINGS};
use torille::expand::{LexiconIndex, form_readings, form_table, inflect_all};
use torille::kotus::{EmbeddedLemma, Lemma, lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::{PartOfSpeech, Tags};

const NOMINAL_POS: [&str; 4] = ["substantiivi", "adjektiivi", "numeraali", "pronomini"];
const RESTART: usize = 16;

struct Pairs {
    /// stem -> base -> the pair's order in the CSV, so the encoded group keeps
    /// the order the committed map's value list has.
    by_stem: HashMap<String, HashMap<String, u32>>,
    pairs: usize,
}

impl Pairs {
    fn load(path: &str) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let mut by_stem: HashMap<String, HashMap<String, u32>> = HashMap::new();
        let mut order = 0u32;
        for line in text.lines() {
            let Some((stem, base)) = line.split_once('\t') else {
                continue;
            };
            by_stem
                .entry(stem.to_string())
                .or_default()
                .entry(base.to_string())
                .or_insert(order);
            order += 1;
        }
        Self {
            by_stem,
            pairs: order as usize,
        }
    }

    fn has(&self, stem: &str, base: &str) -> bool {
        self.by_stem
            .get(stem)
            .is_some_and(|bases| bases.contains_key(base))
    }

    fn order(&self, stem: &str, base: &str) -> Option<u32> {
        self.by_stem.get(stem).and_then(|bases| bases.get(base)).copied()
    }
}

fn is_nominal(lemma: &Lemma<'_>) -> bool {
    lemma
        .part_of_speech
        .split([',', '+'])
        .any(|item| NOMINAL_POS.contains(&item.trim()))
}

fn is_verb(lemma: &Lemma<'_>) -> bool {
    lemma
        .part_of_speech
        .split([',', '+'])
        .any(|item| item.trim() == "verbi")
}

fn pos_of(lemma: &Lemma<'_>) -> PartOfSpeech {
    PartOfSpeech::parse_all(lemma.part_of_speech)
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Noun)
}

fn pos_of_row(row: &EmbeddedLemma) -> PartOfSpeech {
    PartOfSpeech::parse_all(row.part_of_speech())
        .first()
        .copied()
        .unwrap_or(PartOfSpeech::Noun)
}

fn varint(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn gz(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(bytes).expect("gzip");
    encoder.finish().expect("gzip")
}

/// Every split of `word` that the runtime can probe: a prefix that is a
/// stem-map key of `base`, and a remainder the strip list contains.
///
/// Part 1 took only the longest prefix per form. Recording all of them is what
/// makes a miss mean "cannot emit" rather than "not recorded": the runtime
/// probes a specific `(stem, ending)` pair, so every pair it can reach must be
/// in the table for absence to be a proof.
fn splits<'a>(
    word: &'a str,
    base: &str,
    noun: &Pairs,
    verb: &Pairs,
    noun_endings: &BTreeSet<&'static str>,
    verb_endings: &BTreeSet<&'static str>,
) -> Vec<(&'a str, &'a str)> {
    let mut out = Vec::new();
    // `char_indices` never yields the end of the string, and the whole word as
    // its own stem (the empty ending: `kerro` is the imperative of `kertoa`)
    // is the probe the runtime makes first.
    let mut bounds: Vec<usize> = word.char_indices().map(|(at, _)| at).collect();
    bounds.push(word.len());
    for at in bounds.into_iter().rev() {
        let stem = &word[..at];
        let ending = &word[at..];
        if stem.is_empty() {
            continue;
        }
        let noun_ok = noun.has(stem, base) && (ending.is_empty() || noun_endings.contains(ending));
        let verb_ok = verb.has(stem, base) && (ending.is_empty() || verb_endings.contains(ending));
        if noun_ok || verb_ok {
            out.push((stem, ending));
        }
    }
    out
}

#[derive(Clone)]
struct Row {
    stem: String,
    ending: String,
    base: u32,
    readings: Vec<[u8; form_table::READING_BYTES]>,
}

#[derive(Default, Clone, Copy)]
struct Stats {
    rows: usize,
    forms: usize,
    split: usize,
    unsplit: usize,
    readings: usize,
}

fn main() {
    let catalogue = Catalogues::embedded();
    let lexicon = sanalista().to_lemmas();
    let index = LexiconIndex::new(&lexicon);
    let noun = Pairs::load("data/stem_to_base_map.csv");
    let verb = Pairs::load("data/verb_stem_to_base_map.csv");
    eprintln!(
        "stem maps: {} noun pairs, {} verb pairs",
        noun.pairs, verb.pairs
    );

    let noun_endings: BTreeSet<&'static str> = ENDINGS.iter().copied().collect();
    let mut verb_endings: BTreeSet<&'static str> = VERB_ENDINGS.iter().copied().collect();
    verb_endings.extend(ENDINGS.iter().copied());
    // The verb ladder chains the possessive suffixes too (`voisi` is `voida`
    // reached through `voi` + `si`), so a split it can probe must be recorded.
    verb_endings.extend(POSSESSIVE_SUFFIXES.iter().copied());

    let mut base_ids: HashMap<String, u32> = HashMap::new();
    let mut base_names: Vec<String> = Vec::new();
    let mut rows: Vec<Row> = Vec::new();
    let mut stats = Stats::default();

    for lemma in lemmas() {
        if lemma.inflections.is_empty() {
            continue;
        }
        if !is_nominal(lemma) && !is_verb(lemma) {
            continue;
        }
        let base = match base_ids.get(lemma.word) {
            Some(id) => *id,
            None => {
                let id = base_names.len() as u32;
                base_ids.insert(lemma.word.to_string(), id);
                base_names.push(lemma.word.to_string());
                id
            }
        };
        let pos = pos_of(lemma);
        for form in inflect_all(
            lemma.word,
            &lemma.inflections,
            catalogue,
            Some(&index),
            pos,
            false,
        ) {
            if form.word.contains(' ') {
                continue;
            }
            stats.forms += 1;
            // Every form is recorded, possessive and clitic ones included:
            // `inflect_all(possessives = false)` still emits them, so the
            // engine can accept those surfaces, and a table that omitted them
            // could not be used to reject anything.
            let splits = splits(
                &form.word,
                lemma.word,
                &noun,
                &verb,
                &noun_endings,
                &verb_endings,
            );
            if splits.is_empty() {
                stats.unsplit += 1;
                continue;
            }
            stats.split += 1;
            let readings: Vec<_> = form
                .readings
                .iter()
                .map(form_table::encode_reading)
                .collect();
            for (stem, ending) in splits {
                rows.push(Row {
                    stem: stem.to_string(),
                    ending: ending.to_string(),
                    base,
                    readings: readings.clone(),
                });
            }
        }
    }

    // Merge duplicate `(stem, ending, base)` rows — the same surface from
    // several slots or several homonym rows — into one entry with the union of
    // its readings.
    rows.sort_by(|a, b| {
        (a.stem.as_str(), a.ending.as_str(), a.base).cmp(&(
            b.stem.as_str(),
            b.ending.as_str(),
            b.base,
        ))
    });
    let mut merged: Vec<Row> = Vec::with_capacity(rows.len());
    for row in rows {
        match merged.last_mut() {
            Some(last)
                if last.stem == row.stem && last.ending == row.ending && last.base == row.base =>
            {
                for reading in row.readings {
                    if !last.readings.contains(&reading) {
                        last.readings.push(reading);
                    }
                }
            }
            _ => merged.push(row),
        }
    }
    stats.rows = merged.len();

    // `add_reading` keeps the fullest of any readings that nest, and its
    // result depends on the order for readings that do not nest. The walk
    // feeds it in form order, so replay the recorded order rather than a
    // sorted one: the table must return the same `Vec`, not only the same set.
    for row in &mut merged {
        let mut tags: Vec<Tags> = Vec::new();
        for reading in &row.readings {
            torille::tags::add_reading(&mut tags, form_table::decode_reading(reading));
        }
        row.readings = tags.iter().map(form_table::encode_reading).collect();
    }

    let with_readings = build(&merged, &base_names, &noun, &verb, true);
    let without_readings = build(&merged, &base_names, &noun, &verb, false);
    stats.readings = merged.iter().map(|row| row.readings.len()).sum();

    println!(
        "forms {}, split {}, unsplit {} ({:.2}%), merged rows {}, readings {}",
        stats.forms,
        stats.split,
        stats.unsplit,
        100.0 * stats.unsplit as f64 / stats.forms.max(1) as f64,
        stats.rows,
        stats.readings
    );
    println!(
        "table with readings: {} raw, {} gz",
        with_readings.len(),
        gz(&with_readings).len()
    );
    println!(
        "table ending-only:  {} raw, {} gz",
        without_readings.len(),
        gz(&without_readings).len()
    );

    let path = std::path::Path::new("maps/form_table.bin.gz");
    let gzipped = gz(&with_readings);
    std::fs::write(path, &gzipped).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    eprintln!(
        "wrote {} ({} bytes from {} raw)",
        path.display(),
        gzipped.len(),
        with_readings.len()
    );

    spot_check(&with_readings, &noun, &verb, &noun_endings, &verb_endings, catalogue, &index);
}

/// Encode the table. `with_readings = false` is the size-report variant; the
/// committed blob always carries readings.
fn build(
    rows: &[Row],
    base_names: &[String],
    noun: &Pairs,
    verb: &Pairs,
    with_readings: bool,
) -> Vec<u8> {
    // Stems in key order, groups in base order, endings sorted.
    let mut by_stem: BTreeMap<&str, BTreeMap<u32, Vec<&Row>>> = BTreeMap::new();
    for row in rows {
        by_stem
            .entry(row.stem.as_str())
            .or_default()
            .entry(row.base)
            .or_default()
            .push(row);
    }

    // Reading pool: the distinct readings the table stores, most common first
    // so the common ids stay one varint byte.
    let mut reading_ids: HashMap<[u8; form_table::READING_BYTES], u32> = HashMap::new();
    let mut reading_pool: Vec<[u8; form_table::READING_BYTES]> = Vec::new();
    if with_readings {
        let mut counts: HashMap<[u8; form_table::READING_BYTES], u32> = HashMap::new();
        for row in rows {
            for reading in &row.readings {
                *counts.entry(*reading).or_default() += 1;
            }
        }
        let mut ordered: Vec<_> = counts.into_iter().collect();
        ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (reading, _) in ordered {
            let id = reading_pool.len() as u32;
            reading_ids.insert(reading, id);
            reading_pool.push(reading);
        }
    }

    // Keys region.
    let mut keys = Vec::new();
    let mut restarts = Vec::new();
    let mut previous = String::new();
    for (index, stem) in by_stem.keys().enumerate() {
        if index % RESTART == 0 {
            restarts.push(keys.len() as u32);
            varint(&mut keys, 0);
            varint(&mut keys, stem.len() as u32);
            keys.extend_from_slice(stem.as_bytes());
        } else {
            let shared = previous
                .bytes()
                .zip(stem.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            varint(&mut keys, shared as u32);
            varint(&mut keys, (stem.len() - shared) as u32);
            keys.extend_from_slice(&stem.as_bytes()[shared..]);
        }
        previous = (*stem).to_string();
    }

    // Groups region, in key order.
    let mut groups = Vec::new();
    let mut group_offs = Vec::with_capacity(by_stem.len());
    for (stem, bases) in &by_stem {
        group_offs.push(groups.len() as u32);
        varint(&mut groups, bases.len() as u32);
        let mut bases: Vec<_> = bases.iter().collect();
        bases.sort_by_key(|(base, _)| {
            let name = &base_names[**base as usize];
            noun.order(stem, name)
                .or_else(|| verb.order(stem, name))
                .unwrap_or(u32::MAX)
        });
        for (base, entries) in bases {
            varint(&mut groups, *base);
            let mut entries: Vec<_> = entries.clone();
            entries.sort_by(|a, b| a.ending.cmp(&b.ending));
            varint(&mut groups, entries.len() as u32);
            for row in entries {
                groups.push(u8::try_from(row.ending.len()).expect("an ending is short"));
                groups.extend_from_slice(row.ending.as_bytes());
                if with_readings {
                    groups.push(u8::try_from(row.readings.len()).expect("few readings"));
                    for reading in &row.readings {
                        varint(&mut groups, reading_ids[reading]);
                    }
                } else {
                    groups.push(0);
                }
            }
        }
    }

    // Base pool: count, offsets, strings.
    let mut base_pool = Vec::new();
    base_pool.extend_from_slice(&(base_names.len() as u32).to_le_bytes());
    let mut offset = 0u32;
    for base in base_names {
        base_pool.extend_from_slice(&offset.to_le_bytes());
        offset += base.len() as u32;
    }
    base_pool.extend_from_slice(&offset.to_le_bytes());
    for base in base_names {
        base_pool.extend_from_slice(base.as_bytes());
    }

    let mut reading_pool_bytes = Vec::new();
    reading_pool_bytes.extend_from_slice(&(reading_pool.len() as u32).to_le_bytes());
    for reading in &reading_pool {
        reading_pool_bytes.extend_from_slice(reading);
    }

    let keys_off = 40 + restarts.len() * 4;
    let group_offs_off = keys_off + keys.len();
    let groups_off = group_offs_off + group_offs.len() * 4;
    let bases_off = groups_off + groups.len();
    let readings_off = bases_off + base_pool.len();

    let mut blob = vec![0u8; 40];
    blob[0..4].copy_from_slice(b"FMT1");
    blob[4..8].copy_from_slice(&(by_stem.len() as u32).to_le_bytes());
    blob[8..12].copy_from_slice(&(restarts.len() as u32).to_le_bytes());
    blob[12..16].copy_from_slice(&40u32.to_le_bytes());
    blob[16..20].copy_from_slice(&(keys_off as u32).to_le_bytes());
    blob[20..24].copy_from_slice(&(group_offs_off as u32).to_le_bytes());
    blob[24..28].copy_from_slice(&(bases_off as u32).to_le_bytes());
    blob[28..32].copy_from_slice(&(base_names.len() as u32).to_le_bytes());
    blob[32..36].copy_from_slice(&(readings_off as u32).to_le_bytes());
    blob[36..40].copy_from_slice(&(reading_pool.len() as u32).to_le_bytes());

    for restart in restarts {
        blob.extend_from_slice(&restart.to_le_bytes());
    }
    blob.extend_from_slice(&keys);
    for offset in group_offs {
        blob.extend_from_slice(&(groups_off as u32 + offset).to_le_bytes());
    }
    blob.extend_from_slice(&groups);
    blob.extend_from_slice(&base_pool);
    blob.extend_from_slice(&reading_pool_bytes);
    blob
}

/// Every 97th lemma, compare the table's reading for a form with the walk's.
fn spot_check(
    blob: &[u8],
    noun: &Pairs,
    verb: &Pairs,
    noun_endings: &BTreeSet<&'static str>,
    verb_endings: &BTreeSet<&'static str>,
    catalogue: &Catalogues,
    index: &LexiconIndex<'_>,
) {
    let leaked: &'static [u8] = Box::leak(blob.to_vec().into_boxed_slice());
    let table = form_table::FormTable::new(leaked);
    assert!(table.is_valid());
    let mut checked = 0usize;
    let mut skipped = 0usize;
    for (row, lemma) in lemmas().iter().enumerate() {
        if row % 97 != 0 || lemma.inflections.is_empty() {
            continue;
        }
        if !is_nominal(lemma) && !is_verb(lemma) {
            continue;
        }
        let rows: Vec<_> = sanalista().by_word(lemma.word).collect();
        let first_pos = rows.first().map(pos_of_row);
        if rows.iter().any(|row| Some(pos_of_row(row)) != first_pos) {
            skipped += 1;
            continue;
        }
        let pos = first_pos.unwrap_or(PartOfSpeech::Noun);
        let inflections: Vec<_> = rows.iter().flat_map(|row| row.inflections()).collect();
        let forms = inflect_all(lemma.word, &inflections, catalogue, Some(index), pos, false);
        for form in &forms {
            if form.word.contains(' ') {
                continue;
            }
            let splits = splits(
                &form.word,
                lemma.word,
                noun,
                verb,
                noun_endings,
                verb_endings,
            );
            for (stem, ending) in splits {
                let from_table = table.readings(stem, ending, lemma.word);
                let from_walk = form_readings(
                    lemma.word,
                    &inflections,
                    catalogue,
                    Some(index),
                    pos,
                    false,
                    &form.word,
                );
                match (from_table, from_walk) {
                    (Some(table_readings), Some(walk_readings)) => {
                        assert_eq!(
                            table_readings, walk_readings,
                            "{} / {}: table and walk disagree",
                            lemma.word, form.word
                        );
                        checked += 1;
                    }
                    (None, Some(_)) => panic!(
                        "{} / {} (stem {stem:?}, ending {ending:?}, pair {}): \
                         the walk has readings but the table has no entry",
                        lemma.word,
                        form.word,
                        noun.has(stem, lemma.word) || verb.has(stem, lemma.word),
                    ),
                    (Some(_), None) => panic!(
                        "{} / {}: the table has readings but the walk does not",
                        lemma.word, form.word
                    ),
                    (None, None) => {}
                }
            }
        }
    }
    println!("spot check: {checked} forms agree, {skipped} multi-POS lemmas skipped");
}
