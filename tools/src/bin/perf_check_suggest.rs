// SPDX-License-Identifier: MIT OR Apache-2.0
//! `perf_check_suggest` — latency of the check and suggestion paths.
//!
//! The engine has `profile_hot` for the suggestion walk; this tool covers the
//! first-pass checker and the public `check`/`suggest` entry points, on a word
//! list or on a book excerpt. It is the harness behind `../../../misc_md/PERFORMANCE_TODO.md`.
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

use torille::expand::check_normal::{
    NormalizedOutcome, check_normal_fast, check_normal_fast_accept,
};
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
    warm: bool,
    first_pass: bool,
    parity: bool,
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
        warm: false,
        first_pass: false,
        parity: false,
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
            "--warm" => args.warm = true,
            "--first-pass" => args.first_pass = true,
            "--parity" => args.parity = true,
            "--top" => args.top = it.next().and_then(|n| n.parse().ok()).unwrap_or(args.top),
            "-h" | "--help" => {
                println!(
                    "usage: perf_check_suggest [--corpus FILE] [--tokens N] \
                     [--iters N] [--fast-iters N] [--single-thread] [--json] \
                     [--top N] [--warm] [--first-pass] [--parity] [WORD...]"
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

/// Time the cold first pass: one `check_normal_fast` call per unique surface.
///
/// The ordinary corpus mode repeats a token, so the thread-local memo makes
/// every repeat nearly free and hides the cost of the first occurrence. This
/// mode deduplicates first, then times each surface once — the shape the
/// misspell first pass actually pays on a fresh page.
fn run_first_pass(
    path: &str,
    limit: usize,
    top: usize,
    json: bool,
) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("cannot read {path}: {err}"))?;
    let mut seen = std::collections::HashSet::new();
    let mut words: Vec<String> = Vec::new();
    for token in tokenise(&text).iter().filter(|t| t.is_word()).take(limit) {
        if let Some(lower) = token.lower_word()
            && seen.insert(lower.clone())
        {
            words.push(lower);
        }
    }

    let mut rows: Vec<(u128, &String, bool)> = Vec::with_capacity(words.len());
    let mut unknown = 0usize;
    let started = Instant::now();
    for word in &words {
        let token_start = Instant::now();
        let is_unknown = !check_normal_fast_accept(word);
        let us = token_start.elapsed().as_micros();
        if is_unknown {
            unknown += 1;
        }
        rows.push((us, word, is_unknown));
    }
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
    let known_us: u128 = rows.iter().filter(|r| !r.2).map(|r| r.0).sum();
    let unknown_us: u128 = rows.iter().filter(|r| r.2).map(|r| r.0).sum();
    let mut times: Vec<u128> = rows.iter().map(|r| r.0).collect();
    times.sort_unstable();
    let p50 = percentile(&times, 0.50);
    let p95 = percentile(&times, 0.95);
    let p99 = percentile(&times, 0.99);
    let max = *times.last().unwrap_or(&0);

    let mut out = String::new();
    if !json {
        out.push_str(&format!(
            "\n{path}: {} unique surfaces, {unknown} unknown, {total_ms:.0} ms, \
             {:.4} ms/surface\n",
            words.len(),
            total_ms / words.len().max(1) as f64
        ));
        out.push_str(&format!(
            "  p50={p50}µs p95={p95}µs p99={p99}µs max={max}µs\n  \
             known={known_us}µs unknown={unknown_us}µs\n"
        ));
        if top > 0 {
            rows.sort_by_key(|r| std::cmp::Reverse(r.0));
            for (us, word, is_unknown) in rows.iter().take(top) {
                out.push_str(&format!(
                    "  {us:>9}µs  {:<24} {}\n",
                    word,
                    if *is_unknown { "unknown" } else { "known" }
                ));
            }
        }
    }

    let row = format!(
        "{{\"path\":{},\"unique\":{},\"unknown\":{unknown},\
         \"total_ms\":{total_ms:.3},\"per_surface_ms\":{:.4},\
         \"p50_us\":{p50},\"p95_us\":{p95},\"p99_us\":{p99},\"max_us\":{max},\
         \"known_us\":{known_us},\"unknown_us\":{unknown_us}}}",
        json_string(path),
        words.len(),
        total_ms / words.len().max(1) as f64
    );
    Ok((out, row))
}

/// Compare the shipping accept gate with the naming gate on every unique
/// surface. A mismatch is either a new false accept (the signature is stale)
/// or a new false rejection (the signature misses a real form); both are
/// release blockers.
fn run_parity(path: &str, limit: usize) -> Result<(String, usize), String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("cannot read {path}: {err}"))?;
    let mut seen = std::collections::HashSet::new();
    let mut words: Vec<String> = Vec::new();
    for token in tokenise(&text).iter().filter(|t| t.is_word()).take(limit) {
        if let Some(lower) = token.lower_word()
            && seen.insert(lower.clone())
        {
            words.push(lower);
        }
    }
    let mut mismatches = Vec::new();
    for word in &words {
        let accepted = check_normal_fast_accept(word);
        let named = !matches!(check_normal_fast(word, false), NormalizedOutcome::Unknown);
        if accepted != named {
            mismatches.push((word, accepted, named));
        }
    }
    let mut out = format!(
        "\n{path}: {} unique surfaces, {} signature/naming mismatch(es)\n",
        words.len(),
        mismatches.len()
    );
    for (word, accepted, named) in mismatches.iter().take(50) {
        out.push_str(&format!(
            "  {word:<24} accept={accepted} named={named}\n"
        ));
    }
    Ok((out, mismatches.len()))
}

fn main() -> ExitCode {
    let args = parse_args();
    if args.single_thread {
        // The wasm always runs with one worker; the setter makes the native
        // probe measure that shape instead of the host's core count.
        torille::expand::set_single_thread(true);
    }

    if args.warm {
        torille::suggest::warm_caches();
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
    let mut first_pass_rows = Vec::new();
    if args.parity {
        let Some(path) = &args.corpus else {
            eprintln!("perf_check_suggest: --parity needs --corpus FILE");
            return ExitCode::from(2);
        };
        match run_parity(path, args.tokens) {
            Ok((text, mismatches)) => {
                print!("{text}");
                if mismatches > 0 {
                    return ExitCode::from(1);
                }
                return ExitCode::SUCCESS;
            }
            Err(err) => {
                eprintln!("perf_check_suggest: {err}");
                return ExitCode::from(2);
            }
        }
    }
    if let Some(path) = &args.corpus {
        if args.first_pass {
            match run_first_pass(path, args.tokens, args.top, args.json) {
                Ok((text, row)) => {
                    if !args.json {
                        print!("{text}");
                    }
                    first_pass_rows.push(row);
                }
                Err(err) => {
                    eprintln!("perf_check_suggest: {err}");
                    return ExitCode::from(2);
                }
            }
        } else {
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
    }

    if args.json {
        println!(
            "{{\"single_thread\":{},\"words\":[{}],\"corpus\":[{}],\"first_pass\":[{}]}}",
            args.single_thread,
            word_rows.join(","),
            corpus_rows.join(","),
            first_pass_rows.join(",")
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
