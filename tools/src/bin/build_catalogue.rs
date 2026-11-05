// SPDX-License-Identifier: MIT OR Apache-2.0
//! Build `data/catalogue.csv` from the engine's own output.
//!
//! The catalogue is the eight-slot template the heuristic aligns a lemma
//! onto, so the forms that belong in it are the ones the engine emits for a
//! representative member of the class. This tool picks, per class, a
//! representative word out of the sanalista whose engine output is complete
//! and stable across every gradation the word carries, then writes that
//! output as the class template. `Catalogues::embedded()` is then consistent
//! with the engine by construction.
//!
//! This replaces the Kotus taivutustyypit document as the source of the
//! templates: the representative is an ordinary sanalista word (so nothing
//! from the taivutus catalogue is copied), and the forms come from the
//! engine, not from the document.
//!
//! ## Workflow
//!
//! `data/catalogue.csv` must already exist so the build has a blob to run
//! against. Seed it once from the Omorfi/Voikko oracle:
//!
//!     python3 tools/oracle/build_catalogue_csv.py
//!
//! Then refine, rebuilding the blob between passes, until the CSV stops
//! changing (one or two passes):
//!
//!     cargo run -p torille-tools --bin build_catalogue
//!     cargo build
//!
//! Slot 0 (the model word) is chosen here, not read from the seed; a slot
//! the engine has no form for keeps the seed's forms.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use torille::expand::{LexiconIndex, inflect_all};
use torille::grammar::cases_by_slot;
use torille::kotus::{Gradation, Inflection, lemmas, sanalista, slot_nominal};
use torille::spec::Catalogues;
use torille::tags::{Case, Confidence, Mood, Number, PartOfSpeech, Person, Tags, Tense, Voice};

/// A grammatical cell to read out of the engine's output, as a set of
/// constraints. Only the fields a slot fixes are compared.
#[derive(Clone, Copy, Default)]
struct Cell {
    case: Option<Case>,
    number: Option<Number>,
    person: Option<Person>,
    tense: Option<Tense>,
    mood: Option<Mood>,
    voice: Option<Voice>,
}

/// The cell a nominal slot stands for. `slot_nominal` is the engine's own
/// position table, so the template cannot drift from it.
fn noun_cell(i: usize) -> Cell {
    match slot_nominal(i) {
        Some((case, number)) => Cell {
            case: Some(case),
            number: Some(number),
            ..Cell::default()
        },
        None => Cell::default(),
    }
}

/// The cell a verb slot stands for, mirroring `verb_slot_tags` in
/// `src/expand/inflect.rs`.
fn verb_cell(i: usize) -> Cell {
    let mut cell = Cell::default();
    match i {
        1 => {
            cell.tense = Some(Tense::Present);
            cell.mood = Some(Mood::Indicative);
            cell.person = Some(Person::First);
            cell.number = Some(Number::Singular);
        }
        2 => {
            cell.tense = Some(Tense::Past);
            cell.mood = Some(Mood::Indicative);
            cell.person = Some(Person::Third);
            cell.number = Some(Number::Singular);
        }
        3 => {
            cell.tense = Some(Tense::Present);
            cell.mood = Some(Mood::Conditional);
            cell.person = Some(Person::Third);
            cell.number = Some(Number::Singular);
        }
        4 => {
            cell.tense = Some(Tense::Present);
            cell.mood = Some(Mood::Potential);
            cell.person = Some(Person::Third);
            cell.number = Some(Number::Singular);
        }
        5 => {
            cell.tense = Some(Tense::Present);
            cell.mood = Some(Mood::Imperative);
            cell.person = Some(Person::Third);
            cell.number = Some(Number::Singular);
        }
        6 => {
            cell.tense = Some(Tense::Present);
            cell.mood = Some(Mood::Instructive);
        }
        7 => {
            cell.tense = Some(Tense::Past);
            cell.mood = Some(Mood::Indicative);
            cell.voice = Some(Voice::Passive);
            cell.case = Some(Case::Illative);
        }
        _ => {}
    }
    cell
}

fn reading_matches(reading: &Tags, cell: &Cell) -> bool {
    (cell.case.is_none() || reading.case == cell.case)
        && (cell.number.is_none() || reading.number == cell.number)
        && (cell.person.is_none() || reading.person == cell.person)
        && (cell.tense.is_none() || reading.tense == cell.tense)
        && (cell.mood.is_none() || reading.mood == cell.mood)
        && (cell.voice.is_none() || reading.voice == cell.voice)
        && reading.possessive.is_none()
}

/// The words the engine emits for `cell`, best confidence first, one entry
/// per spelling.
fn words_for(
    generated: &[torille::expand::Generated],
    cell: &Cell,
    plain_only: bool,
) -> Vec<String> {
    let mut best: Vec<(String, Confidence)> = Vec::new();
    for form in generated {
        let matches = form.readings.iter().any(|r| reading_matches(r, cell));
        let keep = matches
            && (!plain_only || form.confidence == Confidence::Plain)
            && !form.word.is_empty()
            && !form.word.contains('\t');
        if !keep {
            continue;
        }
        match best.iter_mut().find(|(w, _)| *w == form.word) {
            Some((_, c)) if form.confidence > *c => *c = form.confidence,
            Some(_) => {}
            None => best.push((form.word.clone(), form.confidence)),
        }
    }
    best.sort_by_key(|a| std::cmp::Reverse(a.1));
    best.into_iter().map(|(w, _)| w).collect()
}

/// Keep the order of `prev`, restricted to words also in `next`.
fn intersect(prev: Option<Vec<String>>, next: Vec<String>) -> Vec<String> {
    match prev {
        None => next,
        Some(prev) => prev.into_iter().filter(|w| next.contains(w)).collect(),
    }
}

/// The gradations `model` carries for `class`. Empty when the word is not a
/// member of the class.
///
/// The model's *own* gradation is what the template is read under: the engine
/// reads the stem alternation back out of the template, so an ungradated run
/// of a gradating model would only deduplicate the grade away.
fn gradations_for(lex: &torille::Sanalista, model: &str, class: u16) -> Vec<Option<Gradation>> {
    let mut grads: Vec<Option<Gradation>> = Vec::new();
    for row in lex.by_word(model) {
        for inf in row.inflections() {
            if inf.class == class && !grads.contains(&inf.gradation) {
                grads.push(inf.gradation);
            }
        }
    }
    grads
}

/// The template the engine gives `model` under every gradation, as
/// `slots[0..8]`. `complete` is false when some slot has no form.
fn template_for(
    model: &str,
    class: u16,
    noun: bool,
    cat: &Catalogues,
    index: &LexiconIndex,
    lex: &torille::Sanalista,
) -> (Vec<Vec<String>>, bool) {
    let grads = gradations_for(lex, model, class);

    // Intersect across the gradations: a form that only appears under one
    // grade would be missing under the others.
    let mut plain_slot: Vec<Option<Vec<String>>> = vec![None; 8];
    let mut any_slot: Vec<Option<Vec<String>>> = vec![None; 8];
    for grad in &grads {
        let inflection = Inflection {
            class,
            gradation: *grad,
        };
        // What the consistency test passes: the lexicon, a noun reading and
        // possessives on.
        let generated = inflect_all(
            model,
            &[inflection],
            cat,
            Some(index),
            PartOfSpeech::Noun,
            true,
        );
        for slot in 1..8usize {
            let cell = if noun {
                noun_cell(slot)
            } else {
                verb_cell(slot)
            };
            let plain = words_for(&generated, &cell, /* plain_only = */ true);
            let any = words_for(&generated, &cell, /* plain_only = */ false);
            plain_slot[slot] = Some(intersect(plain_slot[slot].take(), plain));
            any_slot[slot] = Some(intersect(any_slot[slot].take(), any));
        }
    }

    let mut slots: Vec<Vec<String>> = vec![Vec::new(); 8];
    slots[0] = vec![model.to_string()];
    let mut complete = true;
    for slot in 1..8usize {
        let mut words = plain_slot[slot].take().unwrap_or_default();
        if words.is_empty() {
            words = any_slot[slot].take().unwrap_or_default();
        }
        if words.is_empty() {
            complete = false;
        }
        slots[slot] = words;
    }
    (slots, complete)
}

/// True when `slots` is what the hand-written irregular table for `class`
/// spells, for the classes that have one. The catalogue and that table are
/// checked against each other (`grammar::irregular::tests`), so a candidate
/// whose engine output the table does not spell is the wrong representative.
fn table_agrees(model: &str, class: u16, slots: &[Vec<String>]) -> bool {
    let table = cases_by_slot(model, class);
    if table.is_empty() {
        return true;
    }
    for (slot, forms) in slots.iter().enumerate().take(8) {
        if forms.is_empty() {
            continue;
        }
        let allowed: Vec<&str> = table
            .get(slot)
            .map(|v| v.iter().map(|f| f.word.as_str()).collect())
            .unwrap_or_default();
        if forms.iter().any(|w| !allowed.contains(&w.as_str())) {
            return false;
        }
    }
    true
}

fn read_csv(path: &PathBuf) -> BTreeMap<u16, BTreeMap<u16, Vec<String>>> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut out: BTreeMap<u16, BTreeMap<u16, Vec<String>>> = BTreeMap::new();
    for line in text.lines().skip(1) {
        let mut f = line.split('\t');
        let (Some(class), Some(slot), Some(form)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let (Ok(class), Ok(slot)) = (class.parse::<u16>(), slot.parse::<u16>()) else {
            continue;
        };
        if form.is_empty() {
            continue;
        }
        out.entry(class)
            .or_default()
            .entry(slot)
            .or_default()
            .push(form.to_string());
    }
    out
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tools has a parent")
        .to_path_buf();
    let path = root.join("data/catalogue.csv");
    let seed = read_csv(&path);
    assert!(!seed.is_empty(), "{} is empty or missing", path.display());
    let cat = Catalogues::embedded();
    let lex = sanalista();
    let index = LexiconIndex::new(lemmas());

    // Class -> member words, deduplicated and ordered shortest-first so a
    // simple representative is tried before an ornate one.
    let mut members: BTreeMap<u16, Vec<String>> = BTreeMap::new();
    for row in lex.iter() {
        let word = row.word().to_string();
        for inf in row.inflections() {
            let list = members.entry(inf.class).or_default();
            if !list.contains(&word) {
                list.push(word.clone());
            }
        }
    }
    for list in members.values_mut() {
        list.sort_by(|a, b| (a.chars().count(), a).cmp(&(b.chars().count(), b)));
    }

    let mut out = String::from("class\tslot\tform\toptional\n");
    let mut changed = 0usize;
    for class in 1..=75u16 {
        let noun = class <= 51;
        // Try the seed's model first (fewer changes if it is already good),
        // then the class's members.
        let mut candidates: Vec<String> = Vec::new();
        if let Some(model) = seed
            .get(&class)
            .and_then(|slots| slots.get(&0))
            .and_then(|v| v.first())
        {
            candidates.push(model.clone());
        }
        if let Some(list) = members.get(&class) {
            for word in list {
                if !candidates.contains(word) {
                    candidates.push(word.clone());
                }
            }
        }

        let mut chosen: Option<(String, Vec<Vec<String>>)> = None;
        let mut best: Option<(String, Vec<Vec<String>>, usize)> = None;
        for candidate in &candidates {
            // A candidate that is not a member of the class has no gradations
            // to intersect over; skip it rather than template it under a
            // made-up grade.
            if gradations_for(lex, candidate, class).is_empty() {
                continue;
            }
            let (slots, complete) = template_for(candidate, class, noun, cat, &index, lex);
            if !table_agrees(candidate, class, &slots) {
                continue;
            }
            let filled = slots.iter().filter(|s| !s.is_empty()).count();
            if best.as_ref().is_none_or(|(_, _, b)| filled > *b) {
                best = Some((candidate.clone(), slots.clone(), filled));
            }
            if complete {
                chosen = Some((candidate.clone(), slots));
                break;
            }
        }
        let (model, slots) = chosen
            .or_else(|| best.map(|(m, s, _)| (m, s)))
            .unwrap_or_else(|| {
                let m = candidates.first().cloned().unwrap_or_default();
                (m, vec![Vec::new(); 8])
            });

        for slot in 0..8u16 {
            let previous = seed
                .get(&class)
                .and_then(|s| s.get(&slot))
                .cloned()
                .unwrap_or_default();
            let words = if slot == 0 {
                vec![model.clone()]
            } else if slots[slot as usize].is_empty() {
                // The engine says nothing for this cell; keep the seed's forms
                // rather than drop the slot.
                previous.clone()
            } else {
                slots[slot as usize].clone()
            };
            if words != previous {
                changed += 1;
            }
            for word in &words {
                let _ = writeln!(out, "{class}\t{slot}\t{word}\t0");
            }
        }
        eprintln!("  class {class:2}: {model}");
    }

    fs::write(&path, &out).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    eprintln!("build_catalogue: 75 classes, {changed} slot(s) changed");
}
