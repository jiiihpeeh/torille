// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ud_coverage` — Phase 1 candidate-coverage harness.
//!
//! Usage:
//!   ud_coverage --file corpus.conllu [--ambiguity manual_ambiguity.json]
//!
//! Reads CoNLL-U, maps UD morphology to torille `Tags`, runs the existing
//! morphology/syntax pipeline, and reports exact/partial/ambiguous/missing/
//! unavailable candidate coverage by POS, case and morphology depth. It does
//! not train or run a model.

use std::process::ExitCode;

use torille_tools::ud::ambiguity;
use torille_tools::ud::coverage::{coverage, format_report};
use torille_tools::ud::mapping::UD_MAPPING_VERSION;
use torille_tools::ud::reader::{read_file, UdError};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("ud_coverage: {message}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut corpus = None;
    let mut ambiguity_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--file" => corpus = Some(args.next().ok_or("--file needs a path")?),
            "--ambiguity" => {
                ambiguity_path = Some(args.next().ok_or("--ambiguity needs a path")?)
            }
            "--mapping-version" => {
                println!("{UD_MAPPING_VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "usage: ud_coverage --file corpus.conllu [--ambiguity fixture.json]"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    let corpus = corpus.ok_or("missing --file corpus.conllu")?;
    let sentences = read_file(&corpus).map_err(|error: UdError| error.to_string())?;
    let report = coverage(&sentences);
    print!("{}", format_report(&report));

    if let Some(path) = ambiguity_path {
        let fixture =
            ambiguity::read_file(&path).map_err(|error| error.to_string())?;
        println!(
            "\nmanual ambiguity fixture: format={} mapping={} cases={}",
            fixture.format_version,
            fixture.mapping_version,
            fixture.cases.len()
        );
        for case in &fixture.cases {
            println!(
                "  {}: {} token {} preferred={:?} abstain_ok={}",
                case.id,
                case.surface,
                case.token_index,
                case.preferred,
                case.abstain_ok
            );
        }
    }

    Ok(())
}
