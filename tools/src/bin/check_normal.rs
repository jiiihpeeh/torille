// SPDX-License-Identifier: MIT OR Apache-2.0
//! `check_normal` -- the per-keystroke "is this a known Finnish
//! word?" gate the web demo uses.
//!
//! Thin CLI wrapper around the library's six-gate pipeline:
//! `torille::expand::check_normal::check_normal(word, verbose) ->
//! NormalizedOutcome`. The gate logic -- lemma probe, form
//! subset, clitic strip, noun stem hashmap, verb stem hashmap,
//! engine fallback -- lives in the library, so the wasm, the cli,
//! and any future binding share the same behaviour.
//!
//! Usage:
//!   check_normal [-v|--verbose] [--fast] WORD [WORD...]
//!
//! `--fast` runs `check_normal_fast`, the variant the web demo's
//! per-textarea path uses (the cheap gates without the engine gate).

use std::process::ExitCode;

use torille::expand::check_normal::{NormalizedOutcome, check_normal, check_normal_fast};

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut verbose = false;
    let mut fast = false;
    let mut words = Vec::new();
    for arg in raw {
        match arg.as_str() {
            "-v" | "--verbose" => verbose = true,
            "--fast" => fast = true,
            _ => words.push(arg),
        }
    }
    if words.is_empty() {
        eprintln!("usage: check_normal [-v|--verbose] [--fast] WORD [WORD...]");
        return ExitCode::from(2);
    }

    for word in &words {
        let outcome = if fast {
            check_normal_fast(word, verbose)
        } else {
            check_normal(word, verbose)
        };
        match outcome {
            NormalizedOutcome::Lemma => println!("{word}: lemma (in the sanalista)"),
            NormalizedOutcome::Inflected { base, .. } => {
                println!("{word}: inflected -> {base}")
            }
            NormalizedOutcome::Unknown => println!("{word}: unknown"),
        }
    }
    ExitCode::SUCCESS
}
