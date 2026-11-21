// SPDX-License-Identifier: MIT OR Apache-2.0
//! Prototype: measure a per-stem ending signature for the negative-evidence
//! filter. Not part of the engine.
//!
//! Reads the TSV written by
//! `perf_grammar_style --probe-events PATH` and, for every probed stem,
//! computes the set of endings any base in that stem bucket can produce.
//! The signature is a necessary condition: a bit clear means no base at the
//! stem can spell `stem + ending`, so the verifier can skip the paradigm.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};

use torille::expand::LexiconIndex;
use torille::expand::check_normal::{ENDINGS, STEM_BUCKETS_NOUN, STEM_BUCKETS_VERB, VERB_ENDINGS};
use torille::kotus::{lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::PartOfSpeech;

const POSSESSIVE_SUFFIXES: &[&str] = &["nsa", "nsä", "nne", "mme", "si", "ni", "an", "än", "en"];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
}

struct Row {
    map: Map,
    surface: String,
    stem: String,
    ending: String,
    base: String,
    hit: bool,
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
    let path = std::env::args().nth(1).expect("events path");
    let file = std::fs::File::open(&path).expect("open events");
    let mut rows: Vec<Row> = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.expect("read event");
        let mut parts = line.split('\t');
        let map = match parts.next().expect("map") {
            "noun" => Map::Noun,
            "verb" => Map::Verb,
            other => panic!("unknown map {other}"),
        };
        let surface = parts.next().expect("surface").to_string();
        let stem = parts.next().expect("stem").to_string();
        let ending = parts.next().expect("ending").to_string();
        let base = parts.next().expect("base").to_string();
        let hit = parts.next().expect("hit") == "1";
        rows.push(Row {
            map,
            surface,
            stem,
            ending,
            base,
            hit,
        });
    }

    let cat = Catalogues::embedded();
    let lexicon = lemmas();
    let index = LexiconIndex::new(lexicon);
    let noun_endings = endings(Map::Noun);
    let verb_endings = endings(Map::Verb);

    let mut form_cache: HashMap<(String, Map), Vec<String>> = HashMap::new();
    let mut signature_cache: HashMap<(Map, String), HashSet<usize>> = HashMap::new();
    let mut base_signature_cache: HashMap<(Map, String, String), HashSet<usize>> = HashMap::new();

    for map in [Map::Noun, Map::Verb] {
        let mut events = 0usize;
        let mut hits = 0usize;
        let mut failures = 0usize;
        let mut rejected = 0usize;
        let mut rejected_hits = 0usize;
        let mut missing_ending = 0usize;
        let mut not_prefix = 0usize;
        let mut stems: HashSet<String> = HashSet::new();
        let mut base_pairs: HashSet<(String, String)> = HashSet::new();
        let mut per_stem: HashMap<String, (usize, usize, usize)> = HashMap::new();
        let mut endings_total = 0usize;
        let mut failed_forms_total = 0usize;
        let mut failed_forms_matching = 0usize;
        let mut base_endings_total = 0usize;
        let mut pairs = 0usize;
        let list = match map {
            Map::Noun => &noun_endings,
            Map::Verb => &verb_endings,
        };
        let index_of: HashMap<&str, usize> = list
            .iter()
            .enumerate()
            .map(|(index, ending)| (*ending, index))
            .collect();

        for row in rows.iter().filter(|row| row.map == map) {
            events += 1;
            if row.hit {
                hits += 1;
            } else {
                failures += 1;
            }
            if !row.surface.starts_with(&row.stem) {
                not_prefix += 1;
            }
            let Some(&ending_index) = index_of.get(row.ending.as_str()) else {
                missing_ending += 1;
                continue;
            };
            stems.insert(row.stem.clone());
            base_pairs.insert((row.stem.clone(), row.base.clone()));

            let signature = signature_cache
                .entry((map, row.stem.clone()))
                .or_insert_with(|| {
                    let mut set = HashSet::new();
                    for base in match map {
                        Map::Noun => STEM_BUCKETS_NOUN.bases(&row.stem).collect::<Vec<_>>(),
                        Map::Verb => STEM_BUCKETS_VERB.bases(&row.stem).collect::<Vec<_>>(),
                    } {
                        add_base_endings(
                            map,
                            &row.stem,
                            base,
                            list,
                            cat,
                            &index,
                            &mut form_cache,
                            &mut set,
                        );
                    }
                    set
                });
            base_signature_cache
                .entry((map, row.stem.clone(), row.base.clone()))
                .or_insert_with(|| {
                    let mut set = HashSet::new();
                    add_base_endings(
                        map,
                        &row.stem,
                        &row.base,
                        list,
                        cat,
                        &index,
                        &mut form_cache,
                        &mut set,
                    );
                    set
                });
            let counter = per_stem.entry(row.stem.clone()).or_default();
            counter.2 += 1;
            if !row.hit {
                counter.0 += 1;
            }
            if !row.hit
                && let Some(forms) = form_cache.get(&(row.base.clone(), map))
            {
                failed_forms_total += forms.len();
                failed_forms_matching += forms
                    .iter()
                    .filter(|form| form.ends_with(row.ending.as_str()))
                    .count();
            }
            if !signature.contains(&ending_index) {
                rejected += 1;
                counter.1 += 1;
                if row.hit {
                    rejected_hits += 1;
                }
            }
        }

        for ((_, stem), set) in signature_cache.iter().filter(|((m, _), _)| *m == map) {
            if stems.contains(stem) {
                endings_total += set.len();
                pairs += 1;
            }
        }
        for ((_, stem, base), set) in base_signature_cache
            .iter()
            .filter(|((m, _, _), _)| *m == map)
        {
            if base_pairs.contains(&(stem.clone(), base.clone())) {
                base_endings_total += set.len();
            }
        }

        let map_len = match map {
            Map::Noun => STEM_BUCKETS_NOUN.len(),
            Map::Verb => STEM_BUCKETS_VERB.len(),
        };
        let avg = if pairs == 0 {
            0.0
        } else {
            endings_total as f64 / pairs as f64
        };
        let base_avg = if base_pairs.is_empty() {
            0.0
        } else {
            base_endings_total as f64 / base_pairs.len() as f64
        };
        let raw = avg * map_len as f64;
        println!(
            "{}: events {events} (hit {hits}, fail {failures}), stems {} ({} pairs)",
            map.label(),
            stems.len(),
            base_pairs.len()
        );
        println!(
            "  rejected {rejected} ({:.1}% of failures), false rejects {rejected_hits}, \
             missing ending {missing_ending}, not prefix {not_prefix}",
            100.0 * rejected as f64 / failures.max(1) as f64
        );
        println!(
            "  per-stem avg endings {avg:.2}, per-(stem,base) {base_avg:.2}; \
             map len {map_len}; sidecar raw ~{:.2} MB",
            raw / 1_000_000.0
        );
        println!(
            "  failed verify forms: {failed_forms_total} generated, \
             {failed_forms_matching} end with the target ending ({:.1}%); \
             avg {:.1} -> {:.1}",
            100.0 * failed_forms_matching as f64 / failed_forms_total.max(1) as f64,
            failed_forms_total as f64 / failures.max(1) as f64,
            failed_forms_matching as f64 / failures.max(1) as f64
        );
        let mut patterns: HashMap<Vec<usize>, usize> = HashMap::new();
        for ((_, stem), set) in signature_cache.iter().filter(|((m, _), _)| *m == map) {
            if !stems.contains(stem) {
                continue;
            }
            let mut pattern: Vec<usize> = set.iter().copied().collect();
            pattern.sort_unstable();
            *patterns.entry(pattern).or_insert(0) += 1;
        }
        let dictionary_raw: usize = patterns.keys().map(Vec::len).sum();
        println!(
            "  distinct ending-set patterns {} (sample), dictionary raw {} bytes, \
             per-stem id ~{} bytes for the full map",
            patterns.len(),
            dictionary_raw,
            map_len * 2
        );

        let mut ranked: Vec<(&String, &(usize, usize, usize))> = per_stem.iter().collect();
        ranked.sort_by_key(|(_, (_, _, events))| std::cmp::Reverse(*events));
        for limit in [10_000usize, 25_000, 50_000, 100_000] {
            let slice = &ranked[..ranked.len().min(limit)];
            if slice.is_empty() {
                continue;
            }
            let failures: usize = slice.iter().map(|(_, (f, _, _))| f).sum();
            let caught: usize = slice.iter().map(|(_, (_, r, _))| r).sum();
            let raw_bytes: usize = slice
                .iter()
                .map(|(stem, _)| {
                    let set = &signature_cache[&(map, (*stem).clone())];
                    1 + set.len() + stem.len()
                })
                .sum();
            println!(
                "  top {:>6} stems: {:>5} failures, {:>5} rejected ({:.1}%), raw ~{:.2} MB",
                slice.len(),
                failures,
                caught,
                100.0 * caught as f64 / failures.max(1) as f64,
                raw_bytes as f64 / 1_000_000.0
            );
        }
    }
}

#[allow(clippy::too_many_arguments)] // prototype: the context is explicit
fn add_base_endings(
    map: Map,
    stem: &str,
    base: &str,
    endings: &[&str],
    cat: &Catalogues,
    index: &LexiconIndex,
    form_cache: &mut HashMap<(String, Map), Vec<String>>,
    out: &mut HashSet<usize>,
) {
    let forms = form_cache
        .entry((base.to_string(), map))
        .or_insert_with(|| base_forms(base, map, cat, index));
    for form in forms {
        for (ending_index, ending) in endings.iter().enumerate() {
            if !form.ends_with(ending) || form.len() < ending.len() {
                continue;
            }
            let candidate = &form[..form.len() - ending.len()];
            let matches = match map {
                Map::Noun => candidate == stem,
                Map::Verb => candidate == stem || trim_vowels(candidate) == stem,
            };
            if matches {
                out.insert(ending_index);
            }
        }
    }
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
