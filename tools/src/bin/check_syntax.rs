// SPDX-License-Identifier: MIT OR Apache-2.0
//! `check_syntax` — print grammar diagnostics and their corrections.
//!
//! Usage:
//!   check_syntax "Koira nukkuvat."
//!
//! The exit code is 1 when at least one `Error` diagnostic is found, 0
//! otherwise. Warnings and suggestions do not fail the command.

use std::process::ExitCode;

use torille::syntax_check::{self, Severity};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: check_syntax \"TEXT\"");
        return ExitCode::from(2);
    }
    let text = args.join(" ");
    let check = syntax_check::check(&text);

    println!("text: {text}");
    println!(
        "clauses: {}  tokens: {}  diagnostics: {}",
        check.analysis.clauses.len(),
        check.analysis.tokens.len(),
        check.diagnostics.len()
    );
    for diagnostic in &check.diagnostics {
        println!(
            "\n{}  {}  {}  {}..{}",
            diagnostic.kind.name(),
            diagnostic.severity.name(),
            diagnostic.confidence.name(),
            diagnostic.span.start,
            diagnostic.span.end,
        );
        println!("  message: {}", diagnostic.message);
        if let Some(expected) = &diagnostic.expected {
            println!("  expected: {expected}");
        }
        for correction in &diagnostic.corrections {
            let edits: Vec<String> = correction
                .edits
                .iter()
                .map(|edit| {
                    format!(
                        "{}..{} -> {:?}",
                        edit.start, edit.end, edit.replacement
                    )
                })
                .collect();
            println!(
                "  correction: {}  score={:.2}  note={}",
                edits.join(", "),
                correction.score,
                correction.note
            );
        }
    }

    if check
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
