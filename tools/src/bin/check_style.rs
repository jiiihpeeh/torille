// SPDX-License-Identifier: MIT OR Apache-2.0
//! `check_style` — print optional style suggestions.
//!
//! Usage:
//!   check_style "Tällä hetkellä tilanne on rauhallinen."
//!
//! The command always exits 0: a style finding is a suggestion, never an
//! error. Use it to inspect the layer, not as a CI gate.

use std::process::ExitCode;

use torille::style_check;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: check_style \"TEXT\"");
        return ExitCode::from(2);
    }
    let text = args.join(" ");
    let check = style_check::check(&text);

    println!("text: {text}");
    println!("diagnostics: {}", check.diagnostics.len());
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
                .map(|edit| format!("{}..{} -> {:?}", edit.start, edit.end, edit.replacement))
                .collect();
            println!(
                "  correction: {}  score={:.2}  note={}",
                edits.join(", "),
                correction.score,
                correction.note
            );
        }
    }

    ExitCode::SUCCESS
}
