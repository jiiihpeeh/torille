// SPDX-License-Identifier: MIT OR Apache-2.0
//! `check` — is a word in the dictionary, and what should it be?
//!
//! Usage:
//!   check WORD [WORD...]
//!
//! Builds the full index, then for each WORD reports whether it is a known
//! word, and if not whether it looks like a misspelling of something. A word is
//! a known word when the index has a plain reading for it. Otherwise the
//! phonotactic filter is the only gate, and the output lists the base forms the
//! word is closest to.

use std::process::ExitCode;

use torille_tools::{catalogue, sanalista};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: check WORD [WORD...]");
        return ExitCode::from(2);
    }

    let cat = catalogue();
    let lexicon = sanalista().to_lemmas();
    let builder = torille::expand::build(&lexicon, &cat);

    let (words, suggestible, lemmas) = builder.stats();
    println!("index: {words} words, {suggestible} suggestible, {lemmas} lemmas\n");

    for word in &args {
        // A compound is a valid word even if it is not in the index on its own:
        // its components are, and the compound is their concatenation.
        let compound = torille::compound::analyze(word, &builder);
        let explanation = torille::compound::explain(word, &builder);
        let known = !builder.bases_of(word).is_empty() || builder.readings_of(word).is_some();
        if known {
            // A dictionary compound still has an analysis worth printing: the
            // parts, which of them inflect, and which are frozen.
            match &explanation {
                Some(e) => println!("{word}: known word; {}", describe(e)),
                None => println!("{word}: known word"),
            }
        } else if let Some(base) = torille::expand::derived::base(word) {
            // The sanalista omits productive derived verbs, so the reverse
            // index cannot see them; report the class-engine base directly.
            println!("{word}: known word ({base})");
        } else {
            match &compound {
                torille::compound::Analysis::Simple(_) => {
                    println!("{word}: known word (in the sanalista)");
                }
                torille::compound::Analysis::Compound(_) => {
                    let e = explanation.expect("a compound has an explanation");
                    println!("{word}: compound {}", describe(&e));
                }
                torille::compound::Analysis::Unknown => {
                    // The engine's fast path knows derived verbs and
                    // comitative forms the compound splitter does not.
                    if let Some(base) = torille::suggest::is_known_word(word) {
                        println!("{word}: known word ({base})");
                        continue;
                    }
                    println!("{word}: not in the dictionary");
                    // Suggestions are the useful answer for a misspelling:
                    // `ennenkuin` -> `ennen kuin`, not a bare lemma list.
                    let suggestions = torille::suggest::suggest(word, 5, 2);
                    if !suggestions.is_empty() {
                        println!("  suggestions: {}", suggestions.join(", "));
                    }
                    let similar = builder.bases_matching(word, &torille::tags::Tags::UNKNOWN);
                    if !similar.is_empty() {
                        println!("  closest base forms:");
                        for b in similar {
                            println!("    {b}");
                        }
                    }
                }
            }
        }
    }
    ExitCode::SUCCESS
}

/// A compound in one line: the parts, what each does, and the shape.
///
/// `talonmiehen` is `talo` (frozen genitive) + `mies` (inflected), and the
/// shape is genitive-initial. That is the answer `check` could not give when it
/// only printed the lemmas.
fn describe(e: &torille::compound::Explanation) -> String {
    use torille::compound::{Action, CompoundKind};
    let kind = match e.kind {
        CompoundKind::Ordinary => "ordinary",
        CompoundKind::GenitiveInitial => "genitive-initial",
        CompoundKind::BothParts => "both parts inflect",
        CompoundKind::Lexicalised => "lexicalised",
    };
    let parts: Vec<String> = e
        .components
        .iter()
        .map(|c| {
            let what = match c.action {
                Action::Unchanged => "unchanged",
                Action::Inflected => "inflected",
                Action::Frozen => "frozen",
            };
            match c.case {
                Some(case) => format!("{} ({what} {})", c.lemma, case.name()),
                None => format!("{} ({what})", c.lemma),
            }
        })
        .collect();
    format!("{} [{kind}]", parts.join(" + "))
}
