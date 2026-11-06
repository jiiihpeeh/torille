// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_stem_map_noun` — write `data/stem_to_base_map.csv` from the engine.
//!
//! The noun counterpart of the verb map: inflect every classed nominal
//! lemma, recover the stem each form was built on, and emit one
//! `(stem, base)` row per pair. `build.rs` embeds the sorted map; the runtime
//! searches it after stripping a case ending.
//!
//! Classless lemmas are deliberately excluded: their surface must not become
//! a base, or it shadows the compound gate that names the head
//! (`raitiotiellä` -> `tie`).
//!
//! Run after a class change, then rebuild:
//!
//!     cargo run --release -p torille-tools --bin gen_stem_map_noun
//!     cargo build -p torille

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use torille::kotus::{Lemma, lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::{Confidence, Polarity};
use torille_tools::stem_key::nominal_stem_key;

const NOMINAL_POS: [&str; 4] = ["substantiivi", "adjektiivi", "numeraali", "pronomini"];

fn main() {
    let mut out = PathBuf::from("data/stem_to_base_map.csv");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                out = PathBuf::from(args.next().unwrap_or_else(|| usage()));
            }
            _ => usage(),
        }
    }

    let catalogue = Catalogues::embedded();
    let lexicon = sanalista().to_lemmas();
    let index = torille::expand::LexiconIndex::new(&lexicon);

    // Python (and the committed map) treats a lemma as classless when *any*
    // of its rows has no class: a classless homonym keeps the whole lemma out
    // of the map. `piilevä` is the example (adjective class 10 plus a
    // classless noun row).
    let classless: HashSet<&str> = lemmas()
        .iter()
        .filter(|lemma| is_nominal(lemma) && lemma.inflections.is_empty())
        .map(|lemma| lemma.word)
        .collect();

    let mut seen_lemmas = HashSet::new();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
    let mut text = String::new();
    let mut written = 0usize;

    for lemma in lemmas() {
        if !is_nominal(lemma) || lemma.inflections.is_empty() || classless.contains(lemma.word) {
            continue;
        }
        if !seen_lemmas.insert(lemma.word) {
            continue;
        }
        for form in torille_tools::inflect_lemma(lemma.word, catalogue, &index) {
            let tags = form.union();
            let (Some(case), Some(number)) = (tags.case, tags.number) else {
                continue;
            };
            if tags.polarity == Polarity::Negative || tags.clitic.is_some() {
                continue;
            }
            if form.confidence == Confidence::Guessed {
                continue;
            }
            let Some(stem) = nominal_stem_key(&form.word, case, number, tags.possessive) else {
                continue;
            };
            if stem.is_empty() {
                continue;
            }
            let key = (stem.clone(), lemma.word.to_string());
            if !seen_pairs.insert(key) {
                continue;
            }
            text.push_str(&stem);
            text.push('\t');
            text.push_str(lemma.word);
            text.push('\n');
            written += 1;
        }
    }

    fs::write(&out, text).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    eprintln!("wrote {} ({written} unique (stem, base) pairs)", out.display());
}

fn usage() -> ! {
    eprintln!("usage: gen_stem_map_noun [--out PATH]");
    std::process::exit(2);
}

fn is_nominal(lemma: &Lemma<'_>) -> bool {
    lemma
        .part_of_speech
        .split([',', '+'])
        .any(|item| NOMINAL_POS.contains(&item.trim()))
}
