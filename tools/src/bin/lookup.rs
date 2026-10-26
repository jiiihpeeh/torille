// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lookup` — what base word is this form?
//!
//! Usage:
//!   lookup [options] WORD
//!
//!   --case NAME   only the base forms under which WORD is in that case. The
//!                 name is a Finnish case name (`genitive`, `inessive`, ...) or
//!                 `none` for a word that is in no case at all, which is what a
//!                 verb form and an indeclinable word are. This is what makes
//!                 the answer an answer rather than a list: `voi` is a form of
//!                 `voi` and of `voida`, but only the noun is in a case, since a
//!                 verb has no case to be in.
//!   --unranked    report the sanalista's order instead of the ranked one, which
//!                 is what the sanalista reaches the words in. Comparing the two
//!                 is how the ranking is measured rather than assumed.
//!
//! Builds the index over the whole sanalista, then for WORD prints the base
//! forms it is an inflected form of, likelier first, and the readings the index
//! holds for it.
//!
//! This is the slow path and it is slow on purpose: it inflects every lemma in
//! the language, so it answers about any word at all rather than about a
//! neighbourhood of one. Thirty seconds buys a complete answer, and a caller
//! that has to ask per keystroke wants something else.

use std::process::ExitCode;

use torille::expand::ClassPrior;
use torille::tags::{Case, Tags};

use torille_tools::{catalogue, sanalista};

fn main() -> ExitCode {
    let mut case: Option<Option<Case>> = None;
    let mut ranked = true;
    let mut word = String::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--case" => match args.next().as_deref().map(parse_case) {
                Some(Some(parsed)) => case = Some(parsed),
                Some(None) => {
                    eprintln!("unknown case; the names are:");
                    eprintln!("  {}", case_names().join(", "));
                    return ExitCode::from(2);
                }
                None => {
                    eprintln!("--case needs a case name");
                    return ExitCode::from(2);
                }
            },
            "--unranked" => ranked = false,
            "--help" | "-h" => {
                eprintln!("usage: lookup [--case NAME] [--unranked] WORD");
                return ExitCode::from(2);
            }
            other if word.is_empty() => word = other.to_string(),
            other => {
                eprintln!("expected one word, got {other:?} as well");
                return ExitCode::from(2);
            }
        }
    }
    if word.is_empty() {
        eprintln!("usage: lookup [--case NAME] [--unranked] WORD");
        return ExitCode::from(2);
    }

    let cat = catalogue();
    // `expand::build` wants the rows, so this is the one tool that materialises
    // them; `contains` below is answered by the hash table instead.
    let lexicon = sanalista().to_lemmas();
    let builder = torille::expand::build(&lexicon, &cat);
    // Counted over the same lexicon the index was built from, so the prior and
    // the index cannot disagree about what the sanalista says.
    let prior = ClassPrior::of(&lexicon);

    let (words, suggestible, lemmas) = builder.stats();
    println!("index: {words} words, {suggestible} suggestible, {lemmas} lemmas\n");

    if !torille_tools::sanalista().contains(&word) {
        println!("{word}: not a lemma in the sanalista");
    }

    // The bases. Which words the word could be is the index's own question and is
    // asked of the index; which of them to try first is this tool's, and it is
    // asked of the core rather than here so that the ranking is the same one a
    // caller in any language gets.
    let bases: Vec<&str> = match (case, ranked) {
        (Some(c), true) => builder.ranked_bases_in(&word, c, &prior),
        (Some(c), false) => builder.bases_in(&word, c),
        (None, true) => builder.ranked_bases_of(&word, &prior),
        (None, false) => builder.bases_of(&word),
    };

    if bases.is_empty() {
        match case {
            Some(_) => println!("{word}: no base forms in the index for that case"),
            None => println!("{word}: no base forms in the index"),
        }
    } else {
        println!("{word} is a form of:");
        for (base, classes) in with_classes(&builder, &word, &bases) {
            println!("  {base}{}", describe_classes(&classes));
        }
        if case.is_none() && ranked {
            let plain = builder.bases_of(&word);
            if plain != bases {
                println!("\n  (the sanalista's own order is {})", plain.join(", "));
            }
        }
    }

    // The readings, if the index knows the word at all.
    match builder.readings_of(&word) {
        Some(readings) => {
            println!("\nreadings:");
            for r in readings {
                println!("  {}", describe(r));
            }
        }
        None => {
            println!("\n{word}: not in the index");
        }
    }
    ExitCode::SUCCESS
}

/// The classes and part of speech behind each reported base, in the reported
/// order.
///
/// The order comes from the index rather than from a fresh lookup, so what is
/// printed beside a base is the classes the index actually reached the word
/// under and not a second reading of the sanalista that could disagree.
fn with_classes<'a>(
    builder: &'a torille::expand::FormBuilder,
    word: &str,
    bases: &[&'a str],
) -> Vec<(&'a str, Vec<torille::kotus::Inflection>)> {
    let by_word = builder.bases_with_classes(word);
    bases
        .iter()
        .map(|base| {
            let classes = by_word
                .iter()
                .find(|(b, _)| b == base)
                .map(|(_, c)| c.to_vec())
                .unwrap_or_default();
            (*base, classes)
        })
        .collect()
}

/// How the sanalista describes a base, beside its name.
fn describe_classes(classes: &[torille::kotus::Inflection]) -> String {
    if classes.is_empty() {
        return String::new();
    }
    let spelled: Vec<String> = classes.iter().map(ToString::to_string).collect();
    format!("  [{}]", spelled.join(", "))
}

/// The case a `--case` argument names.
///
/// `None` from the outer is an error -- an unknown name -- and `Some(None)` from
/// the inner is a word that is in no case at all, which is a real answer to the
/// question and not a wildcard: `bases_in(word, None)` asks for the bases that
/// reached the word with no case, and a noun is not one of them.
fn parse_case(name: &str) -> Option<Option<Case>> {
    if name.eq_ignore_ascii_case("none") || name.eq_ignore_ascii_case("caseless") {
        return Some(None);
    }
    Case::ALL
        .into_iter()
        .find(|c| c.name().eq_ignore_ascii_case(name))
        .map(Some)
}

fn case_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Case::ALL.iter().map(|c| c.name()).collect();
    names.push("none");
    names
}

fn describe(t: &Tags) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(pos) = t.part_of_speech {
        parts.push(pos.finnish().to_string());
    }
    if let Some(case) = t.case {
        parts.push(case.name().to_string());
    }
    if let Some(n) = t.number {
        parts.push(n.name().to_string());
    }
    if let Some(tense) = t.tense {
        parts.push(tense.name().to_string());
    }
    if let Some(person) = t.person {
        parts.push(person.name().to_string());
    }
    if let Some(mood) = t.mood {
        parts.push(mood.name().to_string());
    }
    if let Some(voice) = t.voice {
        parts.push(voice.name().to_string());
    }
    if let Some(poss) = t.possessive {
        parts.push(format!("possessive {}", possessive_label(poss)));
    }
    if parts.is_empty() {
        "(no tags)".to_string()
    } else {
        parts.join(" ")
    }
}

/// A short label for a possessive, for the tools' output.
fn possessive_label(p: torille::tags::Possessive) -> &'static str {
    use torille::tags::Possessive;
    match p {
        Possessive::FirstSingular => "1sg",
        Possessive::SecondSingular => "2sg",
        Possessive::Third => "3",
        Possessive::FirstPlural => "1pl",
        Possessive::SecondPlural => "2pl",
    }
}
