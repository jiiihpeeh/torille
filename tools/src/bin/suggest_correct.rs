// SPDX-License-Identifier: MIT OR Apache-2.0
//! `suggest_correct` -- given a word, suggest spell-check corrections.
//!
//! Wraps [`torille::suggest::suggest`] to surface hunspell-style "you typed X,
//! did you mean Y?" corrections on the command line. The candidate pool is
//! [`torille::expand::find_bases_with`]'s `bases` + `neighbours` + `direct`,
//! and the ranking is plain Levenshtein distance.
//!
//! Usage:
//!   suggest_correct [--max N] [--distance D] WORD [WORD...]
//!
//! Defaults: `--max 5 --distance 2`. With `--max 0` the tool still prints the
//! header for each word but emits nothing under it, which is useful for
//! counting how many neighbours exist before ranking.

use std::process::ExitCode;

use torille::suggest::suggest;

const DEFAULT_MAX_N: usize = 5;
const DEFAULT_MAX_DISTANCE: usize = 2;

fn main() -> ExitCode {
    let mut max_n: usize = DEFAULT_MAX_N;
    let mut max_distance: usize = DEFAULT_MAX_DISTANCE;
    let mut words: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--max" | "-n" => {
                let Some(v) = args.next() else {
                    eprintln!("{arg} needs a count");
                    return ExitCode::from(2);
                };
                max_n = match v.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("--max: {v:?} is not a non-negative integer");
                        return ExitCode::from(2);
                    }
                };
            }
            "--distance" | "-d" => {
                let Some(v) = args.next() else {
                    eprintln!("{arg} needs a number");
                    return ExitCode::from(2);
                };
                max_distance = match v.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("--distance: {v:?} is not a non-negative integer");
                        return ExitCode::from(2);
                    }
                };
            }
            "--help" | "-h" => {
                eprintln!("usage: suggest_correct [--max N] [--distance D] WORD [WORD...]");
                eprintln!();
                eprintln!(
                    "  --max, -N         number of suggestions to print (default {DEFAULT_MAX_N})"
                );
                eprintln!(
                    "  --distance, -d     max edit distance (default {DEFAULT_MAX_DISTANCE})"
                );
                return ExitCode::from(2);
            }
            other => words.push(other.to_string()),
        }
    }

    if words.is_empty() {
        eprintln!("usage: suggest_correct [--max N] [--distance D] WORD [WORD...]");
        return ExitCode::from(2);
    }

    for word in &words {
        let suggestions = suggest(word, max_n, max_distance);
        println!("{word}");
        if suggestions.is_empty() {
            if max_n == 0 {
                println!("  (no candidates considered)");
            } else {
                println!("  (no suggestions within distance {max_distance})");
            }
        } else {
            for s in suggestions {
                println!("  {s}");
            }
        }
    }
    ExitCode::SUCCESS
}
