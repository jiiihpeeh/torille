// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_stem_endings` — write the per-stem ending-signature blobs.
//!
//! For every `(stem, base)` row in the committed stem-map CSV, generate the
//! base's forms and record the endings the runtime probe can strip to that
//! stem. Many stems share a signature, so the blob stores a pattern dictionary
//! plus one `u16` pattern id per stem key, in the stem map's sorted key order.
//!
//! Run after a stem-map or engine change, then rebuild and publish the maps
//! branch:
//!
//! ```bash
//! cargo run --release -p torille-tools --bin gen_stem_endings
//! cargo build -p torille
//! cargo run -p torille-maps -- publish
//! ```
//!
//! The generator checks the key count against the embedded stem map, so a
//! stale CSV fails here instead of producing a misaligned table.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use torille::expand::LexiconIndex;
use torille::expand::check_normal::{ENDINGS, STEM_BUCKETS_NOUN, STEM_BUCKETS_VERB, VERB_ENDINGS};
use torille::kotus::{lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::PartOfSpeech;

const POSSESSIVE_SUFFIXES: &[&str] = &["nsa", "nsä", "nne", "mme", "si", "ni", "an", "än", "en"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Map {
    Noun,
    Verb,
}

impl Map {
    fn label(self) -> &'static str {
        match self {
            Map::Noun => "noun",
            Map::Verb => "verb",
        }
    }

    fn csv(self) -> &'static str {
        match self {
            Map::Noun => "data/stem_to_base_map.csv",
            Map::Verb => "data/verb_stem_to_base_map.csv",
        }
    }
}

fn endings(map: Map) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = vec![""];
    match map {
        Map::Noun => out.extend(ENDINGS.iter().copied()),
        Map::Verb => {
            out.extend(VERB_ENDINGS.iter().copied());
            out.extend(ENDINGS.iter().copied());
            out.extend(POSSESSIVE_SUFFIXES.iter().copied());
        }
    }
    let mut seen: HashSet<&'static str> = HashSet::new();
    out.retain(|ending| seen.insert(ending));
    out
}

fn trim_vowels(word: &str) -> &str {
    word.trim_end_matches(torille::is_vowel)
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    let maps: Vec<Map> = match which.as_str() {
        "noun" => vec![Map::Noun],
        "verb" => vec![Map::Verb],
        "all" => vec![Map::Noun, Map::Verb],
        other => panic!("usage: gen_stem_endings [noun|verb|all], not {other:?}"),
    };
    for map in maps {
        generate(map);
    }
}

fn generate(map: Map) {
    let out_path = format!("maps/stem_endings_{}.bin.gz", map.label());
    let file = File::open(map.csv()).expect("open stem-map csv");
    let mut base_to_stems: HashMap<String, Vec<String>> = HashMap::new();
    let mut stem_bases: HashMap<String, Vec<String>> = HashMap::new();
    for line in BufReader::new(file).lines() {
        let line = line.expect("read csv");
        let mut parts = line.split('\t');
        let Some(stem) = parts.next() else { continue };
        let Some(base) = parts.next() else { continue };
        base_to_stems
            .entry(base.to_string())
            .or_default()
            .push(stem.to_string());
        stem_bases
            .entry(stem.to_string())
            .or_default()
            .push(base.to_string());
    }

    let cat = Catalogues::embedded();
    let lexicon = lemmas();
    let index = LexiconIndex::new(lexicon);
    let ending_list = endings(map);
    let ending_ids: HashMap<&str, u8> = ending_list
        .iter()
        .enumerate()
        .map(|(id, ending)| (*ending, id as u8))
        .collect();

    let mut signatures: HashMap<String, HashSet<u8>> = HashMap::new();
    for stem in stem_bases.keys() {
        signatures.insert(stem.clone(), HashSet::new());
    }
    let max_ending_bytes = ending_list.iter().map(|ending| ending.len()).max().unwrap_or(0);
    for (base, stems) in &base_to_stems {
        let forms = base_forms(base, map, cat, &index);
        for form in &forms {
            let first = form
                .char_indices()
                .map(|(index, _)| index)
                .find(|&index| form.len() - index <= max_ending_bytes)
                .unwrap_or(form.len());
            for start in form[first..]
                .char_indices()
                .map(|(index, _)| first + index)
                .chain(std::iter::once(form.len()))
            {
                let Some(&ending_id) = ending_ids.get(&form[start..]) else {
                    continue;
                };
                let candidate = &form[..start];
                for stem in stems {
                    let matches = match map {
                        Map::Noun => candidate == stem,
                        Map::Verb => candidate == stem || trim_vowels(candidate) == stem,
                    };
                    if matches {
                        signatures
                            .get_mut(stem)
                            .expect("stem inserted")
                            .insert(ending_id);
                    }
                }
            }
        }
    }

    let mut patterns: HashMap<Vec<u8>, u16> = HashMap::new();
    let mut rows: Vec<(&String, u16)> = Vec::with_capacity(signatures.len());
    // Assign pattern ids in sorted stem order so the blob is reproducible.
    let mut stems: Vec<&String> = signatures.keys().collect();
    stems.sort_unstable();
    for stem in stems {
        let set = &signatures[stem];
        let mut pattern: Vec<u8> = set.iter().copied().collect();
        pattern.sort_unstable();
        let next = patterns.len() as u16;
        let id = *patterns.entry(pattern).or_insert(next);
        rows.push((stem, id));
    }

    let expected = match map {
        Map::Noun => STEM_BUCKETS_NOUN.len(),
        Map::Verb => STEM_BUCKETS_VERB.len(),
    };
    assert_eq!(
        rows.len(),
        expected,
        "{}: the stem map has {expected} keys but the CSV has {};          regenerate the stem map first",
        map.label(),
        rows.len()
    );

    let mut out: Vec<u8> = Vec::new();
    out.write_all(b"ENDP").unwrap();
    out.write_all(&[1, map as u8, 0, 0]).unwrap();
    out.write_all(&(ending_list.len() as u32).to_le_bytes()).unwrap();
    out.write_all(&(patterns.len() as u32).to_le_bytes()).unwrap();
    out.write_all(&(rows.len() as u32).to_le_bytes()).unwrap();
    for ending in &ending_list {
        out.write_all(&[ending.len() as u8]).unwrap();
        out.write_all(ending.as_bytes()).unwrap();
    }
    let mut pattern_rows: Vec<(&Vec<u8>, &u16)> = patterns.iter().collect();
    pattern_rows.sort_by_key(|(_, id)| **id);
    for (pattern, _) in &pattern_rows {
        out.write_all(&[pattern.len() as u8]).unwrap();
        out.write_all(pattern).unwrap();
    }
    // The id array is aligned with the stem map's sorted key order. The
    // prototype writes only the ids; a real integration folds this into the
    // map blob so there is no second key lookup.
    for (_, id) in &rows {
        out.write_all(&id.to_le_bytes()).unwrap();
    }
    torille_tools::gzip::write(Path::new(&out_path), &out);
    let raw = out.len();
    let mut sizes: Vec<usize> = signatures.values().map(HashSet::len).collect();
    sizes.sort_unstable();
    let avg = sizes.iter().sum::<usize>() as f64 / sizes.len().max(1) as f64;
    println!(
        "{}: stems {}, bases {}, patterns {}, endings {}, avg {:.2}",
        map.label(),
        signatures.len(),
        base_to_stems.len(),
        patterns.len(),
        ending_list.len(),
        avg
    );
    let gz = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "wrote {out_path}: raw {:.2} MB, gzip {:.0} KB; median set {}, max {}",
        raw as f64 / 1_000_000.0,
        gz as f64 / 1000.0,
        sizes.get(sizes.len() / 2).copied().unwrap_or(0),
        sizes.last().copied().unwrap_or(0)
    );
}

fn base_forms(
    base: &str,
    map: Map,
    cat: &Catalogues,
    index: &LexiconIndex,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for row in sanalista().by_word(base) {
        let inflections: Vec<_> = row.inflections().collect();
        let pos = PartOfSpeech::parse_all(row.part_of_speech())
            .first()
            .copied()
            .unwrap_or(match map {
                Map::Noun => PartOfSpeech::Noun,
                Map::Verb => PartOfSpeech::Verb,
            });
        for form in torille::expand::inflect_all(
            base,
            &inflections,
            cat,
            Some(index),
            pos,
            false,
        ) {
            out.push(form.word);
        }
    }
    out
}
