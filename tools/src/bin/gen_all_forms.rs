// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_all_forms` — write `data/all_forms.csv` from the engine.
//!
//! The dump is the input to `build.rs`'s fast form subset (bare lemmas and
//! genitive singulars). It is local and gitignored; the committed artifact is
//! `maps/all_forms.bin`. Keeping the dump in step with the engine is what
//! stops a fixed inflection bug from leaving its old invented surfaces in the
//! fast set.
//!
//! Run after a class change, then rebuild the blob:
//!
//!     cargo run --release -p torille-tools --bin gen_all_forms
//!     cargo build -p torille

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use torille::expand::{LexiconIndex, inflect_all};
use torille::kotus::{Lemma, lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::{Confidence, PartOfSpeech, Polarity, Possessive};

const NOMINAL_POS: [&str; 4] = ["substantiivi", "adjektiivi", "numeraali", "pronomini"];

fn main() {
    let mut out = PathBuf::from("data/all_forms.csv");
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
    let index = LexiconIndex::new(&lexicon);

    let mut seen_lemmas = HashSet::new();
    let mut seen_rows: HashSet<(String, String, String, String, String)> = HashSet::new();
    let mut text = String::new();
    let mut written = 0usize;

    for lemma in lemmas() {
        if !is_nominal(lemma) || !is_classed(lemma) {
            continue;
        }
        if !seen_lemmas.insert(lemma.word) {
            continue;
        }
        for form in inflect_word(lemma.word, catalogue, &index) {
            let tags = form.union();
            if tags.polarity == Polarity::Negative || tags.clitic.is_some() {
                continue;
            }
            if form.confidence == Confidence::Guessed {
                continue;
            }
            let case = tags.case.map(|c| c.name()).unwrap_or("");
            let number = tags.number.map(|n| n.name()).unwrap_or("");
            let possessive = tags.possessive.map(possessive_name).unwrap_or("");
            let key = (
                form.word.clone(),
                lemma.word.to_string(),
                case.to_string(),
                number.to_string(),
                possessive.to_string(),
            );
            if !seen_rows.insert(key) {
                continue;
            }
            text.push_str(&form.word);
            text.push('\t');
            text.push_str(lemma.word);
            text.push('\t');
            text.push_str(case);
            text.push('\t');
            text.push_str(number);
            text.push('\t');
            text.push_str(possessive);
            text.push('\n');
            written += 1;
        }
    }

    fs::write(&out, text).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    eprintln!("wrote {} ({written} rows) from the engine", out.display());
}

fn usage() -> ! {
    eprintln!("usage: gen_all_forms [--out PATH]");
    std::process::exit(2);
}

fn is_nominal(lemma: &Lemma<'_>) -> bool {
    lemma
        .part_of_speech
        .split([',', '+'])
        .any(|item| NOMINAL_POS.contains(&item.trim()))
}

fn is_classed(lemma: &Lemma<'_>) -> bool {
    !lemma.inflections.is_empty()
}

fn possessive_name(p: Possessive) -> &'static str {
    match p {
        Possessive::FirstSingular => "1sg",
        Possessive::SecondSingular => "2sg",
        Possessive::Third => "3",
        Possessive::FirstPlural => "1pl",
        Possessive::SecondPlural => "2pl",
    }
}

/// The binding's `Engine.inflect` decision, reproduced without PyO3.
fn inflect_word(
    word: &str,
    catalogue: &Catalogues,
    index: &LexiconIndex<'_>,
) -> Vec<torille::expand::Generated> {
    let inflections = torille_tools::resolve_inflections(word);
    let rows: Vec<_> = torille_tools::lookup(word).collect();
    let noun_capable = rows
        .iter()
        .any(|row| PartOfSpeech::is_substantival(row.part_of_speech()));
    let pos = if noun_capable {
        PartOfSpeech::Noun
    } else {
        rows.first()
            .map(|row| PartOfSpeech::inflecting_reading(row.part_of_speech()))
            .unwrap_or(PartOfSpeech::Unknown)
    };
    inflect_all(word, &inflections, catalogue, Some(index), pos, noun_capable)
}
