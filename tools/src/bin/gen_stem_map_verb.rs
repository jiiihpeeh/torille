// SPDX-License-Identifier: MIT OR Apache-2.0
//! `gen_stem_map_verb` — write `data/verb_stem_to_base_map.csv` from the engine.
//!
//! Verb forms strip differently from nouns: finite personal endings, the
//! passive impersonal endings, infinitives, and participles whose oblique
//! cases strip a nominal case ending. The output is the `(stem, base)` map
//! `build.rs` embeds; the runtime tries a verb ending and then a nominal one.
//!
//! The negation auxiliaries and the fused connectives are a closed lexical
//! table (`data/verb_connectives.csv`), not a verb paradigm; they are added
//! after the engine forms.
//!
//! Run after a verb-class change, then rebuild:
//!
//!     cargo run --release -p torille-tools --bin gen_stem_map_verb
//!     cargo build -p torille

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use torille::kotus::{Lemma, lemmas, sanalista};
use torille::spec::Catalogues;
use torille::tags::{Case, Mood, Number, Person, Polarity, Tense, Voice};

const POSSESSIVE_CSV: &str = "data/verb_connectives.csv";

fn main() {
    let mut out = PathBuf::from("data/verb_stem_to_base_map.csv");
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

    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut lines: Vec<String> = Vec::new();
    let mut bare: BTreeMap<(String, String), String> = BTreeMap::new();

    for lemma in lemmas() {
        if !is_verb(lemma) || lemma.inflections.is_empty() {
            continue;
        }
        for form in torille_tools::inflect_lemma(lemma.word, catalogue, &index) {
            let tags = form.union();
            if tags.polarity == Polarity::Negative || form.word.contains(' ') {
                continue;
            }
            let voice = tags.voice.map(Voice::name).unwrap_or("");
            if !matches!(voice, "active" | "passive" | "") {
                continue;
            }
            let infinitive = tags.mood == Some(Mood::Infinitive);
            let mood = tags
                .mood
                .map(Mood::name)
                .filter(|_| !infinitive)
                .or_else(|| {
                    if tags.person.is_some() && tags.tense.is_some() {
                        Some("indicative")
                    } else {
                        None
                    }
                })
                .unwrap_or("");
            let tense = tags.tense.map(Tense::name).unwrap_or("");
            let person = tags.person.map(person_name).unwrap_or("");
            let number = tags.number.map(Number::name).unwrap_or("");
            let case = tags.case.map(Case::name).unwrap_or("");
            let participle = if !infinitive && tags.case.is_some() && tags.mood.is_none() {
                format!(
                    "{} {}",
                    tags.tense.map(Tense::name).unwrap_or("present"),
                    tags.voice.map(Voice::name).unwrap_or("active")
                )
            } else {
                String::new()
            };

            if !participle.is_empty() || infinitive {
                if tags.possessive.is_some() {
                    continue;
                }
                if infinitive
                    && participle.is_empty()
                    && number == "singular"
                    && (case.is_empty() || case == "nominative")
                {
                    // The 1st infinitive and the 3rd-infinitive genitive share
                    // a reading; emit each bare stem.
                    emit(
                        &mut seen,
                        &mut lines,
                        strip_stem_vowels(&form.word),
                        lemma.word,
                    );
                    continue;
                }
                let kind = if participle.is_empty() {
                    format!("inf:{infinitive}:{case}")
                } else {
                    participle.clone()
                };
                let key = (lemma.word.to_string(), kind);
                let shorter = bare
                    .get(&key)
                    .is_none_or(|current| form.word.chars().count() < current.chars().count());
                if shorter {
                    bare.insert(key, form.word.clone());
                }
                if !(case.is_empty() || case == "nominative") || number != "singular" {
                    emit(
                        &mut seen,
                        &mut lines,
                        strip_stem_vowels(&case_stem(&form.word)),
                        lemma.word,
                    );
                }
                continue;
            }

            if let Some(stem) = stem_of(&form.word, mood, tense, person, number, voice) {
                emit(&mut seen, &mut lines, Some(stem), lemma.word);
            }
            for extra in stems_of(&form.word, mood, tense, person, number, voice) {
                emit(&mut seen, &mut lines, Some(extra), lemma.word);
            }
        }
    }

    for ((lemma, _kind), form) in &bare {
        emit(&mut seen, &mut lines, strip_stem_vowels(form), lemma);
    }

    // The negation auxiliaries are a closed family the engine does not emit
    // as a verb paradigm.
    for surface in ["en", "et", "ei", "emme", "ette", "eivät"] {
        emit(&mut seen, &mut lines, Some(surface.to_string()), "ei");
    }
    // The fused connectives are committed lexical data.
    for (surface, base) in connectives() {
        emit(&mut seen, &mut lines, Some(surface), &base);
    }

    fs::write(&out, lines.join("")).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    eprintln!("wrote {} ({} unique (stem, base) pairs)", out.display(), seen.len());
}

fn usage() -> ! {
    eprintln!("usage: gen_stem_map_verb [--out PATH]");
    std::process::exit(2);
}

fn is_verb(lemma: &Lemma<'_>) -> bool {
    lemma
        .part_of_speech
        .split([',', '+'])
        .any(|item| item.trim() == "verbi")
}

fn person_name(person: Person) -> &'static str {
    match person {
        Person::First => "first",
        Person::Second => "second",
        Person::Third => "third",
    }
}

fn emit(
    seen: &mut HashSet<(String, String)>,
    lines: &mut Vec<String>,
    stem: Option<String>,
    base: &str,
) {
    let Some(stem) = stem.filter(|s| !s.is_empty()) else {
        return;
    };
    if base.is_empty() {
        return;
    }
    let key = (stem.clone(), base.to_string());
    if !seen.insert(key) {
        return;
    }
    lines.push(format!("{stem}\t{base}\n"));
}

fn connectives() -> Vec<(String, String)> {
    let Ok(text) = fs::read_to_string(POSSESSIVE_CSV) else {
        return Vec::new();
    };
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let surface = cols.next()?.trim();
            let base = cols.next()?.trim();
            (!surface.is_empty()).then(|| (surface.to_string(), base.to_string()))
        })
        .collect()
}

fn personal_endings<'a>(
    mood: &str,
    tense: &str,
    person: &str,
    number: &str,
) -> Option<&'a [&'a str]> {
    Some(match (mood, tense, person, number) {
        ("indicative", "present", "first", "singular") => &["n"],
        ("indicative", "present", "second", "singular") => &["t"],
        ("indicative", "present", "third", "singular") => &[""],
        ("indicative", "present", "first", "plural") => &["mme"],
        ("indicative", "present", "second", "plural") => &["tte"],
        ("indicative", "present", "third", "plural") => &["vat", "vät"],
        ("indicative", "past", "first", "singular") => &["in"],
        ("indicative", "past", "second", "singular") => &["it"],
        ("indicative", "past", "third", "singular") => &["i", ""],
        ("indicative", "past", "first", "plural") => &["imme"],
        ("indicative", "past", "second", "plural") => &["itte"],
        ("indicative", "past", "third", "plural") => &["ivat", "ivät"],
        ("conditional", "", "first", "singular") => &["isin"],
        ("conditional", "", "second", "singular") => &["isit"],
        ("conditional", "", "third", "singular") => &["isi"],
        ("conditional", "", "first", "plural") => &["isimme"],
        ("conditional", "", "second", "plural") => &["isitte"],
        ("conditional", "", "third", "plural") => &["isivät", "isivat"],
        ("potential", "", "first", "singular") => &["nen"],
        ("potential", "", "second", "singular") => &["net"],
        ("potential", "", "third", "singular") => &["nee"],
        ("potential", "", "first", "plural") => &["nemme"],
        ("potential", "", "second", "plural") => &["nette"],
        ("potential", "", "third", "plural") => &["nevät", "nevat"],
        ("imperative", "", "first", "singular") => &["koon", "köön"],
        ("imperative", "", "second", "singular") => &[""],
        ("imperative", "", "third", "singular") => &["koon", "köön"],
        ("imperative", "", "first", "plural") => &["kaamme", "käämme"],
        ("imperative", "", "second", "plural") => &["kaa", "kää"],
        ("imperative", "", "third", "plural") => &["koot", "kööt"],
        _ => return None,
    })
}

fn passive_endings(mood: &str, tense: &str) -> Option<&'static [&'static str]> {
    Some(match (mood, tense) {
        ("indicative", "present") => &["taan", "tään", "aan", "ään"],
        ("indicative", "past") => &["ttiin", "tiin", "iin"],
        ("conditional", "") => &["ttaisiin", "täisiin", "taisiin", "isiin"],
        ("potential", "") => &["ttaneen", "täneen", "taneen", "neen"],
        ("imperative", "") => &[
            "ttäköön", "täköön", "ttakoon", "takoon", "köön", "koon",
        ],
        _ => return None,
    })
}


fn stem_of(
    form: &str,
    mood: &str,
    tense: &str,
    person: &str,
    number: &str,
    voice: &str,
) -> Option<String> {
    if !voice.is_empty() && !matches!(voice, "active" | "passive") {
        return None;
    }
    if voice == "passive" {
        if !mood.is_empty() && person.is_empty() && number.is_empty() {
            let endings = passive_endings(mood, tense)?;
            for ending in endings {
                if ending.is_empty() {
                    return Some(form.to_string());
                }
                if form.chars().count() > ending.chars().count() && form.ends_with(ending) {
                    return Some(form[..form.len() - ending.len()].to_string());
                }
            }
            return Some(form.to_string());
        }
        return None;
    }
    if !mood.is_empty() && !person.is_empty() && !number.is_empty() {
        let endings = personal_endings(mood, tense, person, number)?;
        for ending in endings {
            if ending.is_empty() {
                return Some(form.to_string());
            }
            if form.chars().count() > ending.chars().count() && form.ends_with(ending) {
                return Some(form[..form.len() - ending.len()].to_string());
            }
        }
    }
    None
}

fn stems_of(
    form: &str,
    mood: &str,
    tense: &str,
    person: &str,
    number: &str,
    voice: &str,
) -> Vec<String> {
    if voice != "active" || mood != "indicative" || tense != "past" || person != "third" || number != "singular" {
        return Vec::new();
    }
    personal_endings(mood, tense, person, number)
        .map(|endings| {
            endings
                .iter()
                .filter_map(|ending| {
                    if ending.is_empty() {
                        Some(form.to_string())
                    } else if form.chars().count() > ending.chars().count() && form.ends_with(*ending) {
                        Some(form[..form.len() - ending.len()].to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn case_stem(form: &str) -> String {
    let longest = CASE_ENDINGS
        .iter()
        .filter(|ending| form.chars().count() > ending.chars().count() && form.ends_with(*ending))
        .max_by_key(|ending| ending.len());
    match longest {
        Some(ending) => form[..form.len() - ending.len()].to_string(),
        None => form.to_string(),
    }
}

fn strip_stem_vowels(stem: &str) -> Option<String> {
    let trimmed = stem.trim_end_matches(['a', 'e', 'i', 'o', 'u', 'y', 'ä', 'ö']);
    if trimmed.chars().count() >= 2 {
        Some(trimmed.to_string())
    } else {
        Some(stem.to_string())
    }
}

/// Nominal case endings used by the oblique non-finite forms. Must stay in
/// sync with `ENDINGS` in `src/expand/check_normal.rs`; the longest match wins.
const CASE_ENDINGS: &[&str] = &[
    "iltä", "ilta", "istä", "ista", "issä", "inä", "ina", "jen", "ten", "den", "siin", "isiin",
    "kaan", "kään", "seen", "hon", "hön", "jaa", "jää", "ihin", "issa", "illä", "ille", "iksi",
    "tta", "ttä", "ssa", "ssä", "sta", "stä", "lla", "llä", "lle", "lta", "ltä", "ksi", "han",
    "hän", "kin", "isi", "na", "nä", "ta", "tä", "ja", "jä", "en", "an", "än", "in", "on", "ön",
    "un", "yn", "a", "ä", "n", "t",
];
