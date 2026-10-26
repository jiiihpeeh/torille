// SPDX-License-Identifier: MIT OR Apache-2.0
//! `derive` — show the derivational forms of a base word.
//!
//! Usage:
//!   derive BASE
//!
//! Lists the words the productive derivational rules make from BASE, each with
//! the rule and its semantic class, and the reverse derivations of BASE.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let word = args.first().cloned().unwrap_or_default();
    if word.is_empty() {
        eprintln!("usage: derive BASE");
        return ExitCode::from(2);
    }

    // The base's part of speech, from the sanalista, so only the rules whose
    // input matches are applied.
    let pos = torille_tools::lookup(&word)
        .next()
        .and_then(|l| torille_tools::parse_pos(l.part_of_speech()));

    // Forward: every productive rule whose input part of speech matches.
    println!("derivations of {word}:");
    for (rule, derived) in torille::derivation::RULES
        .iter()
        .filter(|r| r.productivity != torille::derivation::Productivity::Lexicalized)
        .filter(|r| pos.is_none() || r.input_pos == pos.unwrap())
        .filter_map(|r| torille::derivation::derive(&word, r).map(|d| (r, d)))
    {
        println!("  {:<28} [{}] {:?}", derived, rule.id, rule.semantic);
    }

    // Reverse: what the base could itself be derived from.
    let rev = torille::derivation::reverse(&word);
    if !rev.is_empty() {
        println!("\n{word} could itself be derived from:");
        for r in rev {
            println!("  {} <- {} [{}]", word, r.source, r.rule);
        }
    }
    ExitCode::SUCCESS
}
