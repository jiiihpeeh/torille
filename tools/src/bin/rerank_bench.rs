// SPDX-License-Identifier: MIT OR Apache-2.0
//! `rerank_bench` — component-level latency of the Model-A reranking path.
//!
//! The pipeline has four stages and the brief wants each timed
//! separately, at several candidate-list widths, because "is the MLP
//! actually the bottleneck?" and "does candidate generation dominate?"
//! have different answers and different fixes:
//!
//! ```text
//!   1. candidate generation   suggest_detailed(word, K, d)
//!   2. feature extraction     query_features + extract_features
//!   3. MLP inference          forward  (two matmuls, raw logits)
//!   4. total reranking        score_batch = 2 + 3, plus argmax
//! ```
//!
//! Stage 3 is timed against a matrix stage 2 already built, so the two
//! numbers are disjoint and sum to stage 4 within noise. Stage 1 is
//! timed at every width too: asking the engine for 100 candidates is
//! genuinely more work than asking for 5, and quoting a single
//! "candidate generation" number would hide that.
//!
//! Whatever the engine sets up lazily on first use is timed once and
//! printed on its own line, so a per-request number never quietly
//! includes a one-off.
//!
//! ## CLI
//!
//! ```bash
//! cargo run -p torille-tools --release --bin rerank_bench -- \
//!     --trm tests/fixtures/rerank_parity.trm \
//!     --words /tmp/queries.txt \
//!     --sizes 5,10,20,50,100
//! ```
//!
//! `--words` is one query per line (the `misspelled` field of a
//! rerank JSONL split, extracted with any JSON tool). Wall-clock timing
//! on a shared machine is noisy, so defaults use enough repetitions to
//! average it out; report medians, not single samples.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use torille::rerank::{Candidate, ModelA, Scratch};
use torille::suggest::suggest_detailed;

// =============================================================================
//   CLI parsing.
// =============================================================================

struct Cli {
    trm: PathBuf,
    words: PathBuf,
    sizes: Vec<usize>,
    reps: usize,
    gen_reps: usize,
    max_distance: usize,
}

impl Cli {
    fn parse() -> Result<Self, String> {
        let mut trm = PathBuf::from("tests/fixtures/rerank_parity.trm");
        let mut words: Option<PathBuf> = None;
        let mut sizes: Vec<usize> = vec![5, 10, 20, 50, 100];
        let mut reps = 20;
        let mut gen_reps = 5;
        // 2, not 3: `rerank_data` builds the training corpus with
        // `suggest_detailed(misspelled, k, 2)` and the engine's own
        // tests use (5, 2), so that is the width production actually
        // runs at. A looser bound prunes less and makes candidate
        // generation look far more expensive than it is.
        let mut max_distance = 2usize;

        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--trm" => {
                    i += 1;
                    trm = args.get(i).map(PathBuf::from).ok_or("--trm needs a path")?;
                }
                "--words" => {
                    i += 1;
                    words = Some(
                        args.get(i)
                            .map(PathBuf::from)
                            .ok_or("--words needs a path")?,
                    );
                }
                "--sizes" => {
                    i += 1;
                    let raw = args.get(i).ok_or("--sizes needs a list")?;
                    sizes = raw
                        .split(',')
                        .map(|s| s.trim().parse::<usize>().map_err(|e| e.to_string()))
                        .collect::<Result<_, _>>()?;
                }
                "--reps" => {
                    i += 1;
                    reps = args
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or("--reps needs an integer")?;
                }
                "--gen-reps" => {
                    i += 1;
                    gen_reps = args
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or("--gen-reps needs an integer")?;
                }
                "--max-distance" => {
                    i += 1;
                    max_distance = args
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or("--max-distance needs an integer")?;
                }
                "--help" | "-h" => {
                    return Err("rerank_bench --words <file> [--trm <file>] \
                         [--sizes 5,10,20,50,100] [--reps 20] [--gen-reps 5] \
                         [--max-distance 2]"
                        .to_string());
                }
                other => return Err(format!("unknown flag: {other}")),
            }
            i += 1;
        }

        let words = words.ok_or("--words <file> is required (one query per line)")?;
        if reps == 0 {
            return Err("--reps must be at least 1".to_string());
        }
        if sizes.is_empty() {
            return Err("--sizes must not be empty".to_string());
        }
        Ok(Self {
            trm,
            words,
            sizes,
            reps,
            gen_reps,
            max_distance,
        })
    }
}

// =============================================================================
//   Measurement.
// =============================================================================

/// One row of the report: per-query microseconds for each stage.
#[derive(Default)]
struct Row {
    size: usize,
    /// Mean candidates actually returned at this width (the engine may
    /// come up short on a hard query).
    cand_mean: f64,
    gen_us: f64,
    feat_us: f64,
    mlp_us: f64,
    total_us: f64,
    p50_us: f64,
    p95_us: f64,
    /// Queries where generation returned fewer than `size` candidates.
    short: usize,
}

/// Mean of a slice; `0.0` for an empty one.
fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Nearest-rank percentile on a sorted slice.
fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[idx.min(sorted.len()) - 1]
}

fn measure(model: &ModelA, words: &[String], cli: &Cli, size: usize) -> Row {
    let mut scratch = Scratch::default();

    // --- 1. candidate generation ----------------------------------------
    // Each query keeps its own candidate list so stage 2 can consume
    // the same lists that were just timed; regenerating them would
    // silently double-count.
    let mut per_query: Vec<(String, Vec<torille::rerank::Suggestion>)> = Vec::new();
    let mut gen_us = Vec::with_capacity(words.len());
    for w in words {
        let mut last = Vec::new();
        let t = Instant::now();
        for _ in 0..cli.gen_reps {
            last = suggest_detailed(w, size, cli.max_distance);
        }
        gen_us.push(t.elapsed().as_secs_f64() * 1e6 / cli.gen_reps as f64);
        per_query.push((w.clone(), last));
    }

    let short = per_query.iter().filter(|(_, s)| s.len() < size).count();
    let cand_mean = mean(
        &per_query
            .iter()
            .map(|(_, s)| s.len() as f64)
            .collect::<Vec<_>>(),
    );

    // --- 2. feature extraction ------------------------------------------
    // Reuses the candidate lists stage 1 just timed: regenerating them
    // here would charge candidate generation to feature extraction.
    let mut feat_us = Vec::with_capacity(words.len());
    for (w, sugs) in &per_query {
        let cands: Vec<Candidate> = sugs
            .iter()
            .map(|s| Candidate::new(&s.word, s.edit_distance as i32))
            .collect();
        let t = Instant::now();
        for _ in 0..cli.reps {
            let q = model.query_features(w);
            model.extract_features(&q, &cands, &mut scratch);
            std::hint::black_box(&scratch);
        }
        feat_us.push(t.elapsed().as_secs_f64() * 1e6 / cli.reps as f64);
    }

    // --- 3. MLP inference over a matrix stage 2 just built ---------------
    let mut mlp_us = Vec::with_capacity(words.len());
    for (w, sugs) in &per_query {
        let cands: Vec<Candidate> = sugs
            .iter()
            .map(|s| Candidate::new(&s.word, s.edit_distance as i32))
            .collect();
        let q = model.query_features(w);
        model.extract_features(&q, &cands, &mut scratch);
        let t = Instant::now();
        for _ in 0..cli.reps {
            std::hint::black_box(model.forward(&mut scratch));
        }
        mlp_us.push(t.elapsed().as_secs_f64() * 1e6 / cli.reps as f64);
    }

    // --- 4. total: one score_batch, then the argmax a caller does --------
    let mut total_us = Vec::with_capacity(words.len());
    for (w, sugs) in &per_query {
        let cands: Vec<Candidate> = sugs
            .iter()
            .map(|s| Candidate::new(&s.word, s.edit_distance as i32))
            .collect();
        let t = Instant::now();
        for _ in 0..cli.reps {
            let q = model.query_features(w);
            let logits = model.score_batch(&q, &cands, &mut scratch);
            std::hint::black_box(ModelA::argmax(logits));
        }
        total_us.push(t.elapsed().as_secs_f64() * 1e6 / cli.reps as f64);
    }

    let mut sorted = total_us.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    Row {
        size,
        cand_mean,
        gen_us: mean(&gen_us),
        feat_us: mean(&feat_us),
        mlp_us: mean(&mlp_us),
        total_us: mean(&total_us),
        p50_us: pct(&sorted, 50.0),
        p95_us: pct(&sorted, 95.0),
        short,
    }
}

fn main() {
    let cli = match Cli::parse() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rerank_bench: {e}");
            std::process::exit(2);
        }
    };

    let words: Vec<String> = fs::read_to_string(&cli.words)
        .unwrap_or_else(|e| {
            eprintln!("rerank_bench: reading {}: {e}", cli.words.display());
            std::process::exit(2);
        })
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    if words.is_empty() {
        eprintln!("rerank_bench: {} has no queries", cli.words.display());
        std::process::exit(2);
    }

    let model = ModelA::load(&cli.trm).unwrap_or_else(|e| {
        eprintln!("rerank_bench: loading {}: {e}", cli.trm.display());
        std::process::exit(2);
    });

    println!("model   : {}", model.describe());
    println!(
        "weights : {} ({} bytes)",
        cli.trm.display(),
        fs::metadata(&cli.trm).map(|m| m.len()).unwrap_or(0)
    );
    println!("queries : {} from {}", words.len(), cli.words.display());
    println!(
        "reps    : {} for rerank stages, {} for candidate generation",
        cli.reps, cli.gen_reps
    );
    println!("distance: {}", cli.max_distance);

    // One query's worth of setup, before any measurement: whatever the
    // engine builds lazily on first use, plus the first query itself.
    // It is charged to nothing below, so a per-request number never
    // quietly includes a one-off.
    let t = Instant::now();
    let _ = suggest_detailed(&words[0], 1, cli.max_distance);
    println!(
        "setup (first suggest call, once): {:.1} ms",
        t.elapsed().as_secs_f64() * 1e3
    );

    println!();
    println!(
        "{:>5} {:>8} | {:>10} {:>10} {:>10} | {:>10} {:>10} {:>10} | {:>7}",
        "K", "cand", "gen_us", "feat_us", "mlp_us", "total_us", "p50_us", "p95_us", "short",
    );
    println!("{}", "-".repeat(95));

    for size in &cli.sizes {
        let r = measure(&model, &words, &cli, *size);
        println!(
            "{:>5} {:>8.2} | {:>10.3} {:>10.3} {:>10.3} | {:>10.3} {:>10.3} {:>10.3} | {:>7}",
            r.size,
            r.cand_mean,
            r.gen_us,
            r.feat_us,
            r.mlp_us,
            r.total_us,
            r.p50_us,
            r.p95_us,
            r.short,
        );
    }

    println!();
    println!("`total_us` is one `score_batch` plus the argmax over its logits;");
    println!("`feat_us` + `mlp_us` should land close to it -- the difference is");
    println!("the second `query_features` call each stage pays.");
}
