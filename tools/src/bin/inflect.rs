// SPDX-License-Identifier: MIT OR Apache-2.0
//! `inflect` — inflect a lemma and show every generated form with its
//! confidence and reading.
//!
//! Usage:
//!   inflect LEMMA [CLASS] [POS] [--clitic CLITIC]
//!
//! CLASS is a Kotus class expression such as `73`, `7*D` or `9*B*A*A`.
//! POS is a part of speech (e.g. `substantiivi`, `verbi`, `adjektiivi`), or a
//! Finnish/English name. When CLASS is omitted the lemma's own class from the
//! sanalista is used; when POS is omitted the lemma's own part of speech is.
//! `--clitic` attaches one of `ko`, `kin`, `kaan`, `han` or `pa` to every
//! form.

use std::process::ExitCode;

use torille_tools::{catalogue, format_form, lookup, parse_inflection, parse_pos};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut word = None;
    let mut class = None;
    let mut pos = None;
    let mut clitic = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--clitic" => {
                i += 1;
                clitic = args.get(i).cloned();
            }
            a if a.starts_with('-') => {
                eprintln!("{a}: unknown option");
                return ExitCode::from(2);
            }
            _ => {
                if word.is_none() {
                    word = Some(args[i].clone());
                } else if class.is_none() {
                    class = Some(args[i].clone());
                } else if pos.is_none() {
                    pos = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }
    let word = word.unwrap_or_default();

    if word.is_empty() {
        eprintln!("usage: inflect LEMMA [CLASS] [POS] [--clitic CLITIC]");
        return ExitCode::from(2);
    }

    let clitic = clitic.as_deref().and_then(parse_clitic);

    let cat = catalogue();
    // A hash probe into the embedded lexicon, not a walk over 104 743 rows.
    let found = lookup(&word).next();
    if found.is_none() && class.is_none() {
        eprintln!("{word}: not in the sanalista; give a CLASS to inflect it");
        return ExitCode::from(2);
    }

    // Resolve the part of speech. Prefer the explicit one, then the sanalista.
    //
    // `inflecting_reading`, not `parse_pos`: this is a noun inflector, so a word
    // the sanalista says can be a noun is read as one. Taking the field's first
    // entry instead read `pieni` (`adjektiivi, substantiivi`) as an adjective and
    // dropped the possessive column the same word was entitled to -- which is
    // what `test_binding_agrees_with_binary.py` caught.
    let pos_of = |name: Option<String>| -> Option<torille::tags::PartOfSpeech> {
        name.and_then(|n| parse_pos(&n))
    };
    let fallback_pos =
        found.map(|l| torille::tags::PartOfSpeech::inflecting_reading(l.part_of_speech()));
    let part_of_speech = pos_of(pos.clone()).or(fallback_pos);

    // Resolve the class. Prefer the explicit one, then the sanalista's.
    //
    // `resolve_inflections`, not the first row's first class: a lemma can be
    // listed under more than one class, and each is a paradigm the dictionary
    // confirms. `ori` is `48,5`, so taking one of them threw away 178 forms --
    // the `48` grid gives `oreen, oreella, oreeseen` and the `5` grid gives
    // `oreiden, oreilla, oreihin`, and only running both gives the word's real
    // paradigm. That was the last disagreement between this binary and the
    // binding.
    let inflections: Vec<torille::kotus::Inflection> = if let Some(c) = class.clone() {
        match parse_inflection(&c) {
            Some(i) => vec![i],
            None => {
                eprintln!("{c}: not a class expression (expected e.g. 73, 7*D)");
                return ExitCode::from(2);
            }
        }
    } else {
        torille_tools::resolve_inflections(&word)
    };

    if inflections.is_empty() {
        eprintln!("{word}: no class to inflect with");
        return ExitCode::from(2);
    }

    let pos = part_of_speech.unwrap_or(torille::tags::PartOfSpeech::Unknown);
    let possessives = pos == torille::tags::PartOfSpeech::Noun;
    // The lexicon, because a class-50/51 compound is inflected part by part and
    // `inflect` hands the compound engine nothing to find the parts with.
    // Materialising all 104 743 rows to inflect one word is the price of
    // matching the binding, which has already paid it.
    let lexicon = torille::kotus::sanalista().to_lemmas();
    let index = torille::expand::LexiconIndex::new(&lexicon);
    let forms =
        torille::expand::inflect_all(&word, &inflections, &cat, Some(&index), pos, possessives);
    let forms = match clitic {
        Some(c) => torille::expand::cliticize_all(&forms, c),
        None => forms,
    };

    let classes: Vec<String> = inflections.iter().map(|i| i.to_string()).collect();
    println!("{word}  class {}  {}\n", classes.join(", "), pos.finnish());
    if forms.is_empty() {
        println!("(no forms generated)");
        return ExitCode::SUCCESS;
    }
    // The three confidence levels are kept apart: `plain` is the linguistically
    // valid paradigm, `repaired` is a typo or corruption normalised to a valid
    // form, and `guessed` is a speculative analysis. Only the plain ones are
    // offered to the checker; the rest are shown for inspection.
    use torille::tags::Confidence;
    for (level, confidence) in [
        ("plain", Confidence::Plain),
        ("repaired", Confidence::Repaired),
        ("guessed", Confidence::Guessed),
    ] {
        let group: Vec<_> = forms
            .iter()
            .filter(|f| f.confidence == confidence)
            .collect();
        if group.is_empty() {
            continue;
        }
        println!("## {level} ({})", group.len());
        for f in &group {
            println!("{}", format_form(f));
        }
        println!();
    }
    let plain = forms.iter().filter(|f| f.plain()).count();
    println!("{} forms; {plain} plain", forms.len());
    ExitCode::SUCCESS
}

/// Parses a clitic name into a [`Clitic`].
fn parse_clitic(s: &str) -> Option<torille::tags::Clitic> {
    use torille::tags::Clitic;
    match s.to_lowercase().as_str() {
        "ko" => Some(Clitic::Ko),
        "kin" => Some(Clitic::Kin),
        "kaan" => Some(Clitic::Kaan),
        "han" => Some(Clitic::Han),
        "pa" => Some(Clitic::Pa),
        _ => None,
    }
}
