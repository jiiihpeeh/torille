// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ud_m0_features` — Phase 3 M0 feature extraction.
//!
//! Usage:
//!   ud_m0_features --train train.conllu --dev dev.conllu --test test.conllu --out DIR
//!
//! Writes `train.jsonl`, `dev.jsonl` and `test.jsonl`. Each row is an
//! ambiguous UD token whose gold reading is a real torille candidate. No model
//! is trained and no production code is exercised by the model.

use std::fs::{File, create_dir_all};
use std::io::{BufWriter, Write};
use std::process::ExitCode;

use torille_tools::ud::m0::{M0Example, examples};
use torille_tools::ud::reader::read_file;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("ud_m0_features: {message}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut train = None;
    let mut dev = None;
    let mut test = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--train" => train = Some(args.next().ok_or("--train needs a path")?),
            "--dev" => dev = Some(args.next().ok_or("--dev needs a path")?),
            "--test" => test = Some(args.next().ok_or("--test needs a path")?),
            "--out" => out = Some(args.next().ok_or("--out needs a directory")?),
            "--help" | "-h" => {
                println!(
                    "usage: ud_m0_features --train train.conllu --dev dev.conllu --test test.conllu --out DIR"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let out = out.ok_or("missing --out DIR")?;
    create_dir_all(&out).map_err(|error| error.to_string())?;

    for (name, path) in [
        ("train", train.as_deref()),
        ("dev", dev.as_deref()),
        ("test", test.as_deref()),
    ] {
        let Some(path) = path else {
            continue;
        };
        let sentences = read_file(path).map_err(|error| error.to_string())?;
        let rows = examples(&sentences);
        write_jsonl(&out, name, &rows)?;
        let ambiguous = rows.len();
        let candidates: usize = rows.iter().map(|row: &M0Example| row.candidate_count).sum();
        println!(
            "{name}: {ambiguous} ambiguous tokens, {candidates} candidates, {} sentences",
            sentences.len()
        );
    }
    Ok(())
}

fn write_jsonl(dir: &str, name: &str, rows: &[M0Example]) -> Result<(), String> {
    let path = std::path::Path::new(dir).join(format!("{name}.jsonl"));
    let file = File::create(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, row)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        writer
            .write_all(b"\n")
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    writer
        .flush()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(())
}
