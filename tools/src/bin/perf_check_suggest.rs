// SPDX-License-Identifier: MIT OR Apache-2.0
//! `perf_check_suggest` — latency of the check and suggestion paths.
//!
//! The engine has `profile_hot` for the suggestion walk; this tool covers the
//! first-pass checker and the public `check`/`suggest` entry points, on a word
//! list or on a book excerpt. It is the harness behind `PERFORMANCE_TODO.md`.
//!
//! Usage:
//!   perf_check_suggest                 # the default word list
//!   perf_check_suggest taloo koirra    # specific words
//!   perf_check_suggest --corpus torille-wasm/demo/books/7000.txt
//!   perf_check_suggest --single-thread # wasm-shaped (one worker)
//!   perf_check_suggest --json          # machine-readable output
//!
//! `--iters` controls the slow entry points (`check_fast`, `check`,
//! `suggest`); the constant-time gates run `--fast-iters` times. Timings are
//! warm: every entry point is called once before its measured loop.

use std::collections::HashSet;
use std::process::ExitCode;
use std::time::Instant;

use torille::expand::check_normal::{NormalizedOutcome, check_normal_fast};
use torille::suggest::{check, check_fast, is_known_word_fast, suggest};
use torille::tokenise::tokenise;

const DEFAULT_WORDS: &[(&str, &str)] = &[
    ("talo", "lemma"),
    ("talossa", "inflected"),
    ("puhtaan", "inflected"),
    ("korrelle", "inflected"),
    ("edistämisvaroista", "compound"),
    ("kahvikekkereitä", "compound"),
    ("henkiruumiiseenne", "possessive compound"),
    ("taloo", "typo"),
    ("koirra", "typo"),
    ("baromeetteri", "typo"),
    ("salamyhkää", "typo"),
    ("silmäränpäyksen", "transposition"),
    ("xyzzy", "nonsense"),
];

struct Args {
    words: Vec<(String, String)>,
    corpus: Option<String>,
    tokens: usize,
    iters: usize,
    fast_iters: usize,
    json: bool,
    single_thread: bool,
    top: usize,
}

fn parse_args() -> Args {
    let mut args = Args {
        words: Vec::new(),
        corpus: None,
        tokens: 2000,
        iters: 1,
        fast_iters: 50,
        json: false,
        single_thread: false,
        top: 0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--corpus" => args.corpus = it.next(),
            "--tokens" => {
                args.tokens = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(args.tokens)
            }
            "--iters" => {
                args.iters = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(args.iters)
                    .max(1)
            }
            "--fast-iters" => {
                args.fast_iters = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(args.fast_iters)
                    .max(1)
            }
            "--json" => args.json = true,
            "--single-thread" => args.single_thread = true,
            "--top" => args.top = it.next().and_then(|n| n.parse().ok()).unwrap_or(args.top),
            "-h" | "--help" => {
                println!(
                    "usage: perf_check_suggest [--corpus FILE] [--tokens N] \
                     [--iters N] [--fast-iters N] [--single-thread] [--json] \
                     [--top N] [WORD...]"
                );
                std::process::exit(0);
            }
            other if other.starts_with('-') => {
                eprintln!("perf_check_suggest: unknown option {other:?}");
                std::process::exit(2);
            }
            word => args.words.push((word.to_string(), "argument".to_string())),
        }
    }
    if args.words.is_empty() && args.corpus.is_none() {
        args.words = DEFAULT_WORDS
            .iter()
            .map(|(w, k)| ((*w).to_string(), (*k).to_string()))
            .collect();
    }
    args
}

/// Mean warm wall time per call, in microseconds.
fn time_us<F: FnMut()>(iters: usize, mut f: F) -> u128 {
    f(); // warm lazy statics and the per-call buffers
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed().as_micros() / iters as u128
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

fn main() -> ExitCode {
    let args = parse_args();
    if args.single_thread {
        // The wasm always runs with one worker; the setter makes the native
        // probe measure that shape instead of the host's core count.
        torille::expand::set_single_thread(true);
    }

    let nouns: HashSet<String> = HashSet::new();
    let mut word_rows = Vec::new();
    for (word, kind) in &args.words {
        let check_normal = time_us(args.fast_iters, || {
            let _ = check_normal_fast(word, false);
        });
        let known_fast = time_us(args.fast_iters, || {
            let _ = is_known_word_fast(word);
        });
        let fast = time_us(args.iters, || {
            let _ = check_fast(word, &nouns);
        });
        let full = time_us(args.iters, || {
            let _ = check(word);
        });
        let sug = time_us(args.iters, || {
            let _ = suggest(word, 5, 2);
        });
        if !args.json {
            println!(
                "{word:<24} {kind:<22} normal={check_normal:>8}µs known_fast={known_fast:>8}µs \
                 check_fast={fast:>10}µs check={full:>10}µs suggest={sug:>10}µs"
            );
        }
        word_rows.push(format!(
            "{{\"word\":{},\"kind\":{},\"check_normal_fast_us\":{check_normal},\
             \"is_known_word_fast_us\":{known_fast},\"check_fast_us\":{fast},\
             \"check_us\":{full},\"suggest_us\":{sug}}}",
            json_string(word),
            json_string(kind)
        ));
    }

    let mut corpus_rows = Vec::new();
    if let Some(path) = &args.corpus {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let words: Vec<String> = tokenise(&text)
                    .iter()
                    .filter(|t| t.is_word())
                    .take(args.tokens)
                    .filter_map(|t| t.lower_word())
                    .collect();
                let mut times = Vec::with_capacity(words.len());
                let mut timed: Vec<(u128, &String)> = Vec::with_capacity(words.len());
                let mut unknown = 0usize;
                let start = Instant::now();
                for word in &words {
                    let token_start = Instant::now();
                    let outcome = check_normal_fast(word, false);
                    let us = token_start.elapsed().as_micros();
                    times.push(us);
                    timed.push((us, word));
                    if matches!(outcome, NormalizedOutcome::Unknown) {
                        unknown += 1;
                    }
                }
                let total_ms = start.elapsed().as_secs_f64() * 1000.0;
                times.sort_unstable();
                let p50 = percentile(&times, 0.50);
                let p95 = percentile(&times, 0.95);
                let p99 = percentile(&times, 0.99);
                let max = *times.last().unwrap_or(&0);
                if !args.json {
                    println!(
                        "\n{path}: {} tokens, {unknown} unknown, {total_ms:.0} ms, \
                         {:.3} ms/token",
                        words.len(),
                        total_ms / words.len().max(1) as f64
                    );
                    println!("  p50={p50}µs p95={p95}µs p99={p99}µs max={max}µs");
                    if args.top > 0 {
                        timed.sort_by_key(|(us, _)| std::cmp::Reverse(*us));
                        for (us, word) in timed.iter().take(args.top) {
                            println!("  {us:>9}µs  {word}");
                        }
                    }
                }
                corpus_rows.push(format!(
                    "{{\"path\":{},\"tokens\":{},\"unknown\":{unknown},\
                     \"total_ms\":{total_ms:.3},\"per_token_ms\":{:.4},\
                     \"p50_us\":{p50},\"p95_us\":{p95},\"p99_us\":{p99},\"max_us\":{max}}}",
                    json_string(path),
                    words.len(),
                    total_ms / words.len().max(1) as f64
                ));
            }
            Err(err) => {
                eprintln!("perf_check_suggest: cannot read {path}: {err}");
                return ExitCode::from(2);
            }
        }
    }

    if args.json {
        println!(
            "{{\"single_thread\":{},\"words\":[{}],\"corpus\":[{}]}}",
            args.single_thread,
            word_rows.join(","),
            corpus_rows.join(",")
        );
    }
    ExitCode::SUCCESS
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
