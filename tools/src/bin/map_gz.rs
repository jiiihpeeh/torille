// SPDX-License-Identifier: MIT OR Apache-2.0
//! `map_gz` — compress or decompress a `maps/*.bin[.gz]` blob.
//!
//! The maps are committed gzip-compressed at the maximum level and the engine
//! decompresses them lazily (`src/map_blob.rs`). This tool is the manual
//! counterpart for inspecting one, or for re-compressing a blob produced
//! outside the normal build.
//!
//! Usage:
//!   map_gz compress   IN [OUT]     # default OUT = IN.gz, level 9
//!   map_gz decompress IN [OUT]     # default OUT = IN without .gz
//!   map_gz test       FILE         # round-trip and report sizes

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return usage();
    };
    match command.as_str() {
        "compress" => {
            let Some(input) = args.next() else {
                return usage();
            };
            let input = PathBuf::from(input);
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| with_suffix(&input, ".gz"));
            let bytes = std::fs::read(&input)
                .unwrap_or_else(|e| panic!("read {}: {e}", input.display()));
            torille_tools::gzip::write(&output, &bytes);
            ExitCode::SUCCESS
        }
        "decompress" => {
            let Some(input) = args.next() else {
                return usage();
            };
            let input = PathBuf::from(input);
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| without_gz(&input));
            let bytes = torille_tools::gzip::read(&input);
            std::fs::write(&output, &bytes)
                .unwrap_or_else(|e| panic!("write {}: {e}", output.display()));
            eprintln!(
                "gunzip: {} bytes -> {} ({} bytes)",
                std::fs::metadata(&input).map(|m| m.len()).unwrap_or(0),
                output.display(),
                bytes.len(),
            );
            ExitCode::SUCCESS
        }
        "test" => {
            let Some(input) = args.next() else {
                return usage();
            };
            let input = PathBuf::from(input);
            let raw = torille_tools::gzip::read(&input);
            let compressed = std::fs::metadata(&input).map(|m| m.len()).unwrap_or(0);
            if compressed == 0 {
                return usage();
            }
            println!(
                "{}: {} compressed, {} raw ({:.1}%)",
                input.display(),
                compressed,
                raw.len(),
                compressed as f64 * 100.0 / raw.len() as f64,
            );
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: map_gz compress IN [OUT] | decompress IN [OUT] | test FILE");
    ExitCode::from(2)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", path.display()))
}

fn without_gz(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    PathBuf::from(text.strip_suffix(".gz").unwrap_or(&text).to_string())
}
