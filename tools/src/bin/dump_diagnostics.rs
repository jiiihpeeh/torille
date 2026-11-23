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

use std::collections::BTreeMap;
use std::process::ExitCode;

use serde_json::json;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut path = "tests/fixtures/perf/page-2000.txt".to_string();
    let mut words = 0usize;
    let mut as_json = false;
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
            "--json" => as_json = true,
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
    if as_json {
        // A stable fingerprint for the suite: the ordered rule ids and spans,
        // plus the per-kind counts. FNV-1a is dependency-free and deterministic.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for diagnostic in &check.diagnostics {
            *counts.entry(diagnostic.kind.name()).or_default() += 1;
            for byte in format!(
                "{}|{}|{}|",
                diagnostic.rule_id().as_str(),
                diagnostic.span.start,
                diagnostic.span.end
            )
            .bytes()
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
            diagnostics.push(json!({
                "rule_id": diagnostic.rule_id().as_str(),
                "kind": diagnostic.kind.name(),
                "severity": diagnostic.severity.name(),
                "start": diagnostic.span.start,
                "end": diagnostic.span.end,
                "message": diagnostic.message,
            }));
        }
        println!(
            "{}",
            json!({
                "path": path,
                "words": if words == 0 { text.split_whitespace().count() } else { words },
                "bytes": text.len(),
                "total": check.diagnostics.len(),
                "hash": format!("{hash:016x}"),
                "counts": counts,
                "diagnostics": diagnostics,
            })
        );
        return ExitCode::SUCCESS;
    }
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
        let expected = diagnostic.expected.as_deref().unwrap_or("-");
        println!(
            "{} {} {}..{} {:?} -> {} [{}]",
            diagnostic.rule_id().as_str(),
            diagnostic.kind.name(),
            start,
            end,
            &text[start..end],
            correction,
            expected
        );
        println!("    {}", text[left..right].replace('\n', " "));
    }
    println!("total {} diagnostics", check.diagnostics.len());
    ExitCode::SUCCESS
}
