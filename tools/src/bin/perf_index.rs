// SPDX-License-Identifier: MIT OR Apache-2.0
//! `perf_index` — where the reverse index's time and memory go.
//!
//! `check` and `lookup` build the whole lexicon index at start-up, and that
//! build is the largest one-off cost in the tree (seconds and gigabytes before
//! the first word is answered). This tool reports the total time and the size
//! of each field, so an optimisation can be argued from numbers. Peak RSS is
//! measured from outside:
//!
//!     /usr/bin/time -v target/release/perf_index
//!
//! Usage:
//!   perf_index                  # multi-threaded, as the tools run it
//!   perf_index --single-thread  # the wasm shape (one worker)

use std::process::ExitCode;
use std::time::Instant;

use serde_json::json;

use torille_tools::{catalogue, sanalista};

fn mb(bytes: usize) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let single_thread = args.iter().any(|a| a == "--single-thread");
    let json = args.iter().any(|a| a == "--json");
    if single_thread {
        torille::expand::set_single_thread(true);
    }
    let cat = catalogue();
    let lexicon = sanalista().to_lemmas();
    let start = Instant::now();
    let builder = torille::expand::build(&lexicon, &cat);
    let elapsed = start.elapsed();

    let (words, suggestible, lemmas) = builder.stats();
    let tags = std::mem::size_of::<torille::tags::Tags>();
    let readings: usize = builder.analyses.iter().map(|r| r.len()).sum();
    let analyses_heap = readings * tags + builder.analyses.len() * std::mem::size_of::<Vec<torille::tags::Tags>>();
    let first_base = builder.first_base.len() * std::mem::size_of::<(u32, u16)>();
    let more_bases = builder.more_bases.len() * std::mem::size_of::<(u32, u32, u16)>();
    let base_words: usize = builder.base_words.iter().map(|w| w.len()).sum();
    let base_classes = builder
        .base_classes
        .iter()
        .map(|c| c.len() * std::mem::size_of::<torille::kotus::Inflection>())
        .sum::<usize>();
    let base_poses = builder
        .base_poses
        .iter()
        .map(|p| p.len() * std::mem::size_of::<torille::tags::PartOfSpeech>())
        .sum::<usize>();

    let known_bytes = builder.known.total_bytes();
    let suggestible_bytes = words.div_ceil(8);
    let lemmas_bytes = builder.lemmas.total_bytes();
    let total_bytes = known_bytes
        + suggestible_bytes
        + lemmas_bytes
        + analyses_heap
        + first_base
        + more_bases
        + base_words
        + base_classes
        + base_poses;
    if json {
        println!(
            "{}",
            json!({
                "single_thread": single_thread,
                "build_ms": elapsed.as_secs_f64() * 1000.0,
                "words": words,
                "suggestible": suggestible,
                "lemmas": lemmas,
                "known_bytes": known_bytes,
                "suggestible_bytes": suggestible_bytes,
                "lemmas_bytes": lemmas_bytes,
                "analyses_bytes": analyses_heap,
                "first_base_bytes": first_base,
                "more_bases_bytes": more_bases,
                "base_words_bytes": base_words,
                "base_classes_bytes": base_classes,
                "base_poses_bytes": base_poses,
                "total_bytes": total_bytes,
            })
        );
        return ExitCode::SUCCESS;
    }
    println!("build: {elapsed:?} (single_thread={single_thread}, {words} words)");
    println!("known       {words:>9} entries  {}", mb(builder.known.total_bytes()));
    println!(
        "suggestible {suggestible:>9} bits    {}",
        mb(words.div_ceil(8))
    );
    println!("lemmas      {lemmas:>9} entries  {}", mb(builder.lemmas.total_bytes()));
    println!(
        "analyses    {readings:>9} readings ({tags} bytes each)  {}",
        mb(analyses_heap)
    );
    println!("first_base  {first_base:>9} bytes  {}", mb(first_base));
    println!("more_bases  {more_bases:>9} bytes  {}", mb(more_bases));
    println!("base_words  {base_words:>9} bytes  {}", mb(base_words));
    println!("base_classes {base_classes:>8} bytes  {}", mb(base_classes));
    println!("base_poses  {base_poses:>9} bytes  {}", mb(base_poses));
    ExitCode::SUCCESS
}
