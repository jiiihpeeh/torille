// SPDX-License-Identifier: MIT OR Apache-2.0
//! `dump_diagnostics` — print every grammar diagnostic with its context.
//!
//! Usage:
//!   dump_diagnostics [--file PATH] [--words N]
//!
//! The default file is `tests/fixtures/perf/page-2000.txt`. `--words` takes the
//! first N whitespace-separated words, which is the shape the performance
//! harness measures, so a count here can be compared with its `diagnostics`
//! line.
//!
//! This is the review tool for false positives: one line per diagnostic with
//! the kind, the span, the offending text, the first correction and the
//! sentence around it. `../../../misc_md/PLAN_GRAMMAR_PRECISION.md` is the work list it serves.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut path = "tests/fixtures/perf/page-2000.txt".to_string();
    let mut words = 0usize;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" => path = args.next().expect("--file needs a path"),
            "--words" => {
                words = args
                    .next()
                    .expect("--words needs a number")
                    .parse()
                    .expect("--words must be a number")
            }
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {path}: {error}");
            return ExitCode::from(2);
        }
    };
    let text = if words == 0 {
        text
    } else {
        let mut count = 0;
        let mut end = text.len();
        for (index, ch) in text.char_indices() {
            if ch.is_whitespace() {
                count += 1;
                if count == words {
                    end = index;
                    break;
                }
            }
        }
        text[..end].to_string()
    };

    let check = torille::syntax_check::check(&text);
    for diagnostic in &check.diagnostics {
        let start = diagnostic.span.start.min(text.len());
        let end = diagnostic.span.end.min(text.len());
        let left = text[..start]
            .rfind(['.', '!', '?', '\n'])
            .map_or(0, |index| index + 1);
        let right = text[end..]
            .find(['.', '!', '?', '\n'])
            .map_or(text.len(), |index| end + index + 1);
        let correction = diagnostic
            .corrections
            .first()
            .map(|correction| correction.edits[0].replacement.as_str())
            .unwrap_or("-");
        println!(
            "{} {}..{} {:?} -> {}",
            diagnostic.kind.name(),
            start,
            end,
            &text[start..end],
            correction
        );
        println!("    {}", text[left..right].replace('\n', " "));
    }
    println!("total {} diagnostics", check.diagnostics.len());
    ExitCode::SUCCESS
}
