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
        let known = !builder.bases_of(word).is_empty() || builder.readings_of(word).is_some();
        if known {
            println!("{word}: known word");
        } else if let Some(base) = torille::expand::derived::base(word) {
            // The sanalista omits productive derived verbs, so the reverse
            // index cannot see them; report the class-engine base directly.
            println!("{word}: known word ({base})");
        } else {
            match &compound {
                torille::compound::Analysis::Simple(_) => {
                    println!("{word}: known word (in the sanalista)");
                }
                torille::compound::Analysis::Compound(c) => {
                    let parts: Vec<&str> = c.parts.iter().map(|p| p.lemma.as_str()).collect();
                    println!("{word}: compound {}", parts.join(" + "));
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
