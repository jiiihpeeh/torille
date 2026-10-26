// SPDX-License-Identifier: MIT OR Apache-2.0
//! `look_base` -- print the result of `torille_tools::find_bases` for one word.
//!
//! A spell checker. There is a tool for it and a library function, and the two
//! have the same behaviour because the tool is just a printer over the
//! function. The work -- the four phases of "which lemmas is this a form of" --
//! lives in [`torille_tools::find_bases`], so any caller (a real spell checker,
//! a web service, another binary) gets the same answers without spawning a
//! process.
//!
//! Usage:
//!   look_base <word>

use std::process::ExitCode;

use torille_tools::{find_bases, gradation_letter};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(word) = args.next() else {
        eprintln!("usage: look_base <word>");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("usage: look_base <word>");
        return ExitCode::from(2);
    }

    let report = find_bases(&word);

    println!("# input: {word:?}");
    println!(
        "\n# sanalista direct: {} row(s) whose lemma is exactly {word:?}",
        report.direct.len()
    );
    for row in &report.direct {
        let classes: Vec<String> = row
            .inflections()
            .map(|i| format!("{}{}", i.class, gradation_letter(i.gradation)))
            .collect();
        println!(
            "  row={:>5}  word={:?}  homonymy={:?}  pos={:?}  classes=[{}]",
            row.row(),
            row.word(),
            row.homonymy(),
            row.part_of_speech(),
            classes.join(",")
        );
    }

    println!(
        "\n# bases: {} lemma(s) that inflect to {word:?}",
        report.bases.len()
    );
    for lemma in &report.bases {
        let classes: Vec<String> = lemma
            .inflections
            .iter()
            .map(|i| format!("{}{}", i.class, gradation_letter(i.gradation)))
            .collect();
        println!(
            "  word={:?}  homonymy={:?}  classes=[{}]",
            lemma.word,
            lemma.homonymy,
            classes.join(",")
        );
    }

    if !report.neighbours.is_empty() {
        println!(
            "\n# neighbours: {} head/tail candidate(s) that don't inflect to {word:?}",
            report.neighbours.len()
        );
        for lemma in &report.neighbours {
            println!("  word={:?}  homonymy={:?}", lemma.word, lemma.homonymy);
        }
    }

    println!(
        "\n# {} bases + {} neighbours",
        report.bases.len(),
        report.neighbours.len()
    );
    ExitCode::SUCCESS
}
