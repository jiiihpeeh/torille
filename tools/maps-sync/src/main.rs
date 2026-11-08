// SPDX-License-Identifier: MIT OR Apache-2.0
//! `torille-maps` — inspect, fetch or publish the orphan `maps` branch.
//!
//! This crate does not depend on `torille`, so it builds on a fresh clone
//! even when the maps (and therefore `torille`) cannot compile yet.
//!
//! Usage:
//!   cargo run -p torille-maps -- status
//!   cargo run -p torille-maps -- fetch
//!   cargo run -p torille-maps -- publish

use std::process::ExitCode;

use torille_maps as maps;

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| usage());
    let root = match maps::repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("maps: {err}");
            return ExitCode::from(1);
        }
    };
    let remote = maps::remote();
    match command.as_str() {
        "status" => {
            let missing = maps::missing(&root);
            if missing.is_empty() {
                println!(
                    "maps: all {} required blobs are present",
                    maps::REQUIRED.len()
                );
                ExitCode::SUCCESS
            } else {
                println!("maps: missing {}", missing.join(", "));
                ExitCode::from(1)
            }
        }
        "fetch" => match maps::fetch(&root, &remote) {
            Ok(count) => {
                println!("maps: fetched {count} blobs from {remote}/maps");
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("maps: fetch failed: {err}");
                ExitCode::from(1)
            }
        },
        "publish" => match maps::publish(&root, &remote) {
            Ok(commit) => {
                println!("maps: published {commit} to {remote}/maps");
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("maps: publish failed: {err}");
                ExitCode::from(1)
            }
        },
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("usage: torille-maps <status|fetch|publish>");
    std::process::exit(2)
}
