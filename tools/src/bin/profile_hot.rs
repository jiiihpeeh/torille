// SPDX-License-Identifier: MIT OR Apache-2.0
//! `profile_hot` -- phase-by-phase timing of the per-keystroke spell-check
//! hot path. Output is a text "flame": top-of-stack at "main", leaves at the
//! innermost named function, with the percent of wall-clock time spent in
//! each frame.
//!
//! Why a custom one instead of `cargo flamegraph` / `perf record`?
//!   * `cargo flamegraph` requires a third-party install and the box this
//!     runs on can't reach crates.io.
//!   * `perf_event_paranoid=4` blocks `perf` for unprivileged users.
//!
//! Instead we instrument the hot path manually: time `find_bases`,
//! `levenshtein`, `inflect` separately on a representative workload, and
//! print a phase report. The numbers come from the same release profile
//! the criterion bench uses, so they tell us where to look next.
//!
//! Run:
//!
//! ```bash
//! cargo build --release --bin profile_hot -p torille-tools
//! ./target/release/profile_hot --iters=500
//! ```

use std::process::ExitCode;
use std::time::Instant;

use torille::expand::find_bases_with;
use torille::kotus::Lemma as KotusLemma;
use torille::spec::Catalogues;
use torille::suggest::{levenshtein, suggest};

const DEFAULT_ITERS: usize = 500;

fn main() -> ExitCode {
    let mut iters: usize = DEFAULT_ITERS;
    for arg in std::env::args().skip(1) {
        if let Some(n) = arg.strip_prefix("--iters=") {
            iters = n.parse().unwrap_or(DEFAULT_ITERS);
        } else if let Some(n) = arg.strip_prefix("-n=") {
            iters = n.parse().unwrap_or(DEFAULT_ITERS);
        }
    }

    let cat = Catalogues::embedded().clone();
    let lexicon: Vec<KotusLemma<'static>> = torille::kotus::parse_sanalista(torille::SANALISTA);
    let lexicon: &'static [KotusLemma<'static>] = Box::leak(lexicon.into_boxed_slice());

    let words = ["taloo", "koirra", "puhtaan", "öihin", "xyzzy"];

    // Warmup.
    for w in words.iter().cycle().take(50) {
        let _ = suggest(w, 5, 2);
    }

    // ---- Phase 1: full `suggest` ------------------------------------------
    let t_full = Instant::now();
    for w in words.iter().cycle().take(iters) {
        let _ = suggest(w, 5, 2);
    }
    let full = t_full.elapsed();

    // ---- Phase 2: `find_bases_with` only (no edit-distance ranking) --------
    let t_find = Instant::now();
    let mut bases_total = 0usize;
    let mut neigh_total = 0usize;
    let mut direct_total = 0usize;
    for w in words.iter().cycle().take(iters) {
        let r = find_bases_with(lexicon, &cat, w);
        bases_total += r.bases.len();
        neigh_total += r.neighbours.len();
        direct_total += r.direct.len();
    }
    let find = t_find.elapsed();

    // ---- Phase 3: `levenshtein` only --------------------------------------
    // The full `suggest` is dominated by `levenshtein` over every
    // candidate. We can simulate that by running find_bases once per
    // word and timing the edit-distance calls that follow.
    let mut lev_total = 0u128;
    let mut lev_calls = 0usize;
    for w in words.iter().cycle().take(iters) {
        let r = find_bases_with(lexicon, &cat, w);
        for lemma in &r.bases {
            let t = Instant::now();
            let _ = levenshtein(w, lemma.word, 2);
            lev_total += t.elapsed().as_nanos();
            lev_calls += 1;
        }
        for lemma in &r.neighbours {
            let t = Instant::now();
            let _ = levenshtein(w, lemma.word, 2);
            lev_total += t.elapsed().as_nanos();
            lev_calls += 1;
        }
        for direct in &r.direct {
            let t = Instant::now();
            let _ = levenshtein(w, direct.word(), 2);
            lev_total += t.elapsed().as_nanos();
            lev_calls += 1;
        }
    }
    let lev_secs = lev_total as f64 / 1e9;

    // ---- Phase 4: ranking / sorting overhead --------------------------------
    // Time spent after the levenshtein loop: dedup + sort + truncate + map.
    // Approximated as `full - find - lev` -- not exact, but close enough
    // to size the candidate.

    // Every timed phase runs `words.iter().cycle().take(iters)`, so each
    // one makes exactly `iters` calls -- the cycle only chooses *which*
    // word, not how many. This used to be `iters * words.len()`, which
    // divided µs/call and candidates/call by 5 (the array length) and
    // claimed five times as many calls as were made. That is why
    // `profile_hot` reported ~130 ms/query for a workload the models
    // measured at ~578 ms: same work, divisor five times too large.
    let n = iters;
    let full_secs = full.as_secs_f64();
    let find_secs = find.as_secs_f64();
    let rank_secs = full_secs - find_secs - lev_secs;
    let find_pct = 100.0 * find_secs / full_secs;
    let lev_pct = 100.0 * lev_secs / full_secs;
    let rank_pct = 100.0 * rank_secs / full_secs;

    println!(
        "{n} `suggest(\"{{taloo,koirra,puhtaan,öihin,xyzzy}}, 5, 2)` calls in {:.3?} ms ({:.1} µs/call)",
        full.as_millis(),
        full.as_micros() as f64 / n as f64,
    );
    println!();
    println!("# Phase breakdown (wall clock, release LTO profile):");
    println!("  {:<28}  {:>10}  {:>6}", "phase", "seconds", "%");
    println!(
        "  {:<28}  {:>10.3}  {:>5.1}%",
        "find_bases (phases 1-4)", find_secs, find_pct
    );
    println!(
        "  {:<28}  {:>10.3}  {:>5.1}%",
        "levenshtein (per candidate)", lev_secs, lev_pct
    );
    println!(
        "  {:<28}  {:>10.3}  {:>5.1}%",
        "rank / dedup / map",
        rank_secs.max(0.0),
        rank_pct
    );
    println!("  {:<28}  {:>10.3}  {:>5.1}%", "TOTAL", full_secs, 100.0);
    println!();
    println!("# Candidates seen during the work:");
    println!(
        "  find_bases bases/direct/neighbours total: {} / {} / {}",
        bases_total, direct_total, neigh_total
    );
    println!("  levenshtein calls during the work: {}", lev_calls);
    println!();
    println!("# Where to look next:");
    println!(
        "  {:.0} candidates per call -> {:.0} levenshtein calls per call",
        lev_calls as f64 / n as f64,
        lev_calls as f64 / n as f64,
    );

    ExitCode::SUCCESS
}
