// SPDX-License-Identifier: MIT OR Apache-2.0
//! `perf_grammar_style` — measure the grammar and style passes.
//!
//! Usage:
//!   perf_grammar_style [--file PATH] [--words N] [--repeat N]
//!                      [--single-thread] [--json]
//!
//! The default fixture is `tests/fixtures/perf/page-2000.txt`; `--words`
//! takes the first N word tokens of it. `--single-thread` measures the wasm
//! shape. Cold means a fresh morphology cache; the process-level engine
//! caches (the paradigm cache inside `emits_form`) are warm after the first
//! pass, which is the same shape a worker has after its first document.

use std::time::Instant;

use torille::check::check_all_with_cache;
use torille::morphology::{CacheStats, Depth, MorphologyCache, analyze_morphology};
use torille::syntax::analyze_syntax;
use torille::syntax_check::check_analysis;

const DEFAULT_FIXTURE: &str = "tests/fixtures/perf/page-2000.txt";

struct Args {
    file: String,
    words: usize,
    repeat: usize,
    slowest: usize,
    single_thread: bool,
    json: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        file: DEFAULT_FIXTURE.to_string(),
        words: 100,
        repeat: 3,
        slowest: 0,
        single_thread: false,
        json: false,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--file" => args.file = iter.next().expect("--file needs a path"),
            "--words" => {
                args.words = iter
                    .next()
                    .expect("--words needs a number")
                    .parse()
                    .expect("--words must be a number")
            }
            "--repeat" => {
                args.repeat = iter
                    .next()
                    .expect("--repeat needs a number")
                    .parse()
                    .expect("--repeat must be a number")
            }
            "--slowest" => {
                args.slowest = iter
                    .next()
                    .expect("--slowest needs a number")
                    .parse()
                    .expect("--slowest must be a number")
            }
            "--single-thread" => args.single_thread = true,
            "--json" => args.json = true,
            other => panic!("unknown argument: {other}"),
        }
    }
    args
}

fn text_of(path: &str, words: usize) -> String {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
    if words == 0 {
        return text;
    }
    let mut count = 0;
    for (index, ch) in text.char_indices() {
        if ch.is_whitespace() {
            count += 1;
            if count == words {
                return text[..index].to_string();
            }
        }
    }
    text
}

fn ms(elapsed: std::time::Duration) -> f64 {
    elapsed.as_secs_f64() * 1000.0
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// `direct=3 stem_map=12 ...` for the text report.
fn depths(stats: &CacheStats) -> String {
    Depth::ALL
        .iter()
        .map(|depth| format!("{}={}", depth.label(), stats.by_depth[depth.index()]))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The same counters as a JSON object.
fn depths_json(stats: &CacheStats) -> String {
    Depth::ALL
        .iter()
        .map(|depth| format!("\"{}\":{}", depth.label(), stats.by_depth[depth.index()]))
        .collect::<Vec<_>>()
        .join(",")
}

fn main() {
    let args = parse_args();
    if args.single_thread {
        torille::expand::set_single_thread(true);
    }
    let text = text_of(&args.file, args.words);
    let words = text.split_whitespace().count();

    // Tokenise once for the cold morphology pass.
    let t = Instant::now();
    let tokens = torille::tokenise::tokenise(&text);
    let tokenise_ms = ms(t.elapsed());

    // Cold: a fresh per-textarea cache. This is the first check of a page.
    let mut cache = MorphologyCache::new();
    let cold = Instant::now();
    let morphology = analyze_morphology(&tokens, &mut cache);
    let morphology_cold_ms = ms(cold.elapsed());
    let cold_stats = cache.stats();

    // Warm: the same cache after one complete pass.
    let mut warm_morphology = Vec::new();
    let mut warm_syntax = Vec::new();
    let mut warm_rules = Vec::new();
    let mut warm_style = Vec::new();
    let mut warm_all = Vec::new();
    for _ in 0..args.repeat.max(1) {
        let t = Instant::now();
        let _ = analyze_morphology(&tokens, &mut cache);
        warm_morphology.push(ms(t.elapsed()));

        let t = Instant::now();
        let analysis = analyze_syntax(&tokens, &morphology);
        warm_syntax.push(ms(t.elapsed()));

        let t = Instant::now();
        let _ = check_analysis(&tokens, &analysis);
        warm_rules.push(ms(t.elapsed()));

        let t = Instant::now();
        let _ = torille::style_check::check_with_cache(&text, &mut cache);
        warm_style.push(ms(t.elapsed()));

        let t = Instant::now();
        let _ = check_all_with_cache(&text, &mut cache);
        warm_all.push(ms(t.elapsed()));
    }

    // Fresh cache for the style-only cold shape; the engine's paradigm cache
    // is warm by now, so this is a lower bound on a truly cold process.
    let mut fresh = MorphologyCache::new();
    let t = Instant::now();
    let _ = torille::style_check::check_with_cache(&text, &mut fresh);
    let style_fresh_cache_ms = ms(t.elapsed());

    if args.slowest > 0 {
        let mut unique: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for token in &tokens {
            if token.is_word() {
                unique.insert(token.text.to_lowercase());
            }
        }
        let mut timings: Vec<(f64, String)> = Vec::new();
        for word in &unique {
            let single = torille::tokenise::tokenise(word);
            let mut cold = MorphologyCache::new();
            let t = Instant::now();
            let _ = analyze_morphology(&single, &mut cold);
            timings.push((ms(t.elapsed()), word.clone()));
        }
        timings.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("slowest unique words (fresh cache per word):");
        for (elapsed, word) in timings.iter().take(args.slowest) {
            println!("  {elapsed:9.2} ms  {word}");
        }
    }

    let stats = cache.stats();
    if args.json {
        println!(
            "{{\"file\":\"{}\",\"words\":{},\"bytes\":{},\"single_thread\":{},\
             \"tokenise_ms\":{:.2},\"morphology_cold_ms\":{:.2},\
             \"morphology_warm_ms\":{:.2},\"syntax_ms\":{:.2},\
             \"rules_ms\":{:.2},\"style_ms\":{:.2},\"all_ms\":{:.2},\
             \"style_fresh_cache_ms\":{:.2},\
             \"cold_cache\":{{\"words\":{},\"hits\":{},\"misses\":{}}},\
             \"cold_depths\":{{{}}},\"cold_deep_upgrades\":{},\"cold_proper_names\":{},\
             \"warm_cache\":{{\"words\":{},\"hits\":{},\"misses\":{}}},\
             \"warm_depths\":{{{}}},\"warm_deep_upgrades\":{},\"warm_proper_names\":{}}}",
            args.file,
            words,
            text.len(),
            args.single_thread,
            tokenise_ms,
            morphology_cold_ms,
            median(warm_morphology),
            median(warm_syntax),
            median(warm_rules),
            median(warm_style),
            median(warm_all),
            style_fresh_cache_ms,
            cold_stats.words,
            cold_stats.hits,
            cold_stats.misses,
            depths_json(&cold_stats),
            cold_stats.deep_upgrades,
            cold_stats.proper_names,
            stats.words,
            stats.hits,
            stats.misses,
            depths_json(&stats),
            stats.deep_upgrades,
            stats.proper_names,
        );
        return;
    }

    println!(
        "{}: {words} words, {} bytes, single_thread={}",
        args.file,
        text.len(),
        args.single_thread
    );
    println!("cold morphology: {morphology_cold_ms:9.2} ms");
    println!("warm morphology: {:9.2} ms", median(warm_morphology));
    println!("warm syntax:    {:9.2} ms", median(warm_syntax));
    println!("warm rules:     {:9.2} ms", median(warm_rules));
    println!("warm style:     {:9.2} ms", median(warm_style));
    println!("warm all:       {:9.2} ms", median(warm_all));
    println!("style, fresh cache: {style_fresh_cache_ms:9.2} ms");
    println!(
        "cold cache: {} words, {} hits, {} misses",
        cold_stats.words, cold_stats.hits, cold_stats.misses
    );
    println!(
        "depth counts: {} (deep upgrades {}, proper names {})",
        depths(&cold_stats),
        cold_stats.deep_upgrades,
        cold_stats.proper_names
    );
    println!(
        "warm cache: {} words, {} hits, {} misses",
        stats.words, stats.hits, stats.misses
    );
}
