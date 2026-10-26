// SPDX-License-Identifier: MIT OR Apache-2.0
//! `rerank` — the production Model-A reranking path, end to end.
//!
//! ```text
//! suggest_detailed(word, K, d)   candidate generation (deterministic engine)
//!       ↓
//! ModelA::query_features(word)   computed ONCE per query
//!       ↓
//! ModelA::score_batch(...)       one (N, n_in) matrix, two matmuls, raw logits
//!       ↓
//! ModelA::argmax / sort          ranking; softmax happens outside, if at all
//! ```
//!
//! No Python anywhere on this path: the weights come from the flat
//! `.trm` blob `tools/train/model.py` writes next to its `.npz`. The
//! two implementations are checked against each other by
//! `tests/rerank_parity.rs`, not by one calling the other.
//!
//! The output is deliberately line-oriented TSV so it can be diffed
//! against the Python side without a JSON dependency in the tool:
//!
//! ```text
//! query<TAB>rank<TAB>candidate<TAB>edit_distance<TAB>logit
//! ```
//!
//! `--candidates-in` skips the engine and scores a candidate list read
//! from a TSV (`rank_agreement.py export` writes one from a rerank
//! JSONL split). Python has no engine, so that is the only way to hand
//! the two implementations *identical* inputs and diff their rankings;
//! see `rank_agreement.py compare`.
//!
//! ## CLI
//!
//! ```bash
//! cargo run -p torille-tools --release --bin rerank -- \
//!     --trm tests/fixtures/rerank_parity.trm --top 3 jooksee koiraa
//!
//! # or a whole query list:
//! cargo run -p torille-tools --release --bin rerank -- \
//!     --trm /tmp/opencode/modelA_prod.trm --words /tmp/opencode/q_dev.txt
//! ```

use std::fs;
use std::path::PathBuf;

use torille::rerank::{Candidate, ModelA, Scratch};
use torille::suggest::suggest_detailed;

struct Cli {
    trm: PathBuf,
    words: Option<PathBuf>,
    candidates_in: Option<PathBuf>,
    positional: Vec<String>,
    top: usize,
    max_distance: usize,
    max_candidates: usize,
    header: bool,
}

impl Cli {
    fn parse() -> Result<Self, String> {
        let mut trm = PathBuf::from("tests/fixtures/rerank_parity.trm");
        let mut words: Option<PathBuf> = None;
        let mut candidates_in: Option<PathBuf> = None;
        let mut positional = Vec::new();
        let mut top = 5usize;
        // Matches `rerank_data`'s corpus-generation bound.
        let mut max_distance = 2usize;
        let mut max_candidates = 50usize;
        let mut header = true;

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
                "--candidates-in" => {
                    i += 1;
                    candidates_in = Some(
                        args.get(i)
                            .map(PathBuf::from)
                            .ok_or("--candidates-in needs a path")?,
                    );
                }
                "--top" => {
                    i += 1;
                    top = args
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or("--top needs an integer")?;
                }
                "--max-distance" => {
                    i += 1;
                    max_distance = args
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or("--max-distance needs an integer")?;
                }
                "--max-candidates" => {
                    i += 1;
                    max_candidates = args
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or("--max-candidates needs an integer")?;
                }
                "--no-header" => header = false,
                "--help" | "-h" => {
                    return Err("rerank [--trm <file>] [--words <file>] \
                         [--candidates-in <tsv>] [--top 5] [--max-candidates 50] \
                         [--max-distance 2] [--no-header] [word...]"
                        .to_string());
                }
                other if other.starts_with('-') => {
                    return Err(format!("unknown flag: {other}"));
                }
                other => positional.push(other.to_string()),
            }
            i += 1;
        }

        if words.is_none() && candidates_in.is_none() && positional.is_empty() {
            return Err(
                "give at least one word, --words <file>, or --candidates-in <tsv>".to_string(),
            );
        }
        Ok(Self {
            trm,
            words,
            candidates_in,
            positional,
            top,
            max_distance,
            max_candidates,
            header,
        })
    }
}

/// Read `query<TAB>idx<TAB>word<TAB>edit_distance` rows, grouped by
/// query in file order.
///
/// This is how the Python/Rust agreement check feeds *identical*
/// candidate lists to both sides: Python has no engine, so the
/// candidates have to come from somewhere both implementations agree
/// on. `rank_agreement.py export` writes this file from a rerank
/// JSONL split.
fn read_candidates(path: &std::path::Path) -> Vec<(String, Vec<(String, i32)>)> {
    let raw = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("rerank: reading {}: {e}", path.display());
        std::process::exit(2);
    });
    let mut out: Vec<(String, Vec<(String, i32)>)> = Vec::new();
    for (lineno, line) in raw.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split('\t');
        let (q, idx, word, ed) = (
            it.next().unwrap_or_default(),
            it.next().unwrap_or_default(),
            it.next().unwrap_or_default(),
            it.next().unwrap_or_default(),
        );
        let idx: usize = idx.parse().unwrap_or_else(|_| {
            eprintln!(
                "rerank: {}:{}: bad index {idx:?}",
                path.display(),
                lineno + 1
            );
            std::process::exit(2);
        });
        let ed: i32 = ed.parse().unwrap_or_else(|_| {
            eprintln!(
                "rerank: {}:{}: bad edit distance {ed:?}",
                path.display(),
                lineno + 1
            );
            std::process::exit(2);
        });
        match out.last_mut() {
            Some((last_q, list)) if *last_q == q => {
                if idx != list.len() {
                    eprintln!(
                        "rerank: {}:{}: index {idx} for {q:?} but {} already listed",
                        path.display(),
                        lineno + 1,
                        list.len()
                    );
                    std::process::exit(2);
                }
                list.push((word.to_string(), ed));
            }
            _ => {
                if idx != 0 {
                    eprintln!(
                        "rerank: {}:{}: query {q:?} starts at index {idx}, expected 0",
                        path.display(),
                        lineno + 1
                    );
                    std::process::exit(2);
                }
                out.push((q.to_string(), vec![(word.to_string(), ed)]));
            }
        }
    }
    if out.is_empty() {
        eprintln!("rerank: {} contains no candidates", path.display());
        std::process::exit(2);
    }
    out
}

/// The ranking order: highest logit first, ties toward the lowest index.
///
/// Same promise as `ModelA::argmax` and as Python's
/// `sorted(range(n), key=lambda i: (-logit, i))`.
fn order_by_logit(logits: &[f32]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..logits.len()).collect();
    order.sort_by(|&a, &b| {
        let ord = logits[b]
            .partial_cmp(&logits[a])
            .expect("logits must not be NaN");
        ord.then(a.cmp(&b))
    });
    order
}

fn main() {
    let cli = match Cli::parse() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rerank: {e}");
            std::process::exit(2);
        }
    };

    // Normalise every input mode to (query, [(word, edit_distance)]) in
    // file / argument order, so the scoring loop below never branches.
    let work: Vec<(String, Vec<(String, i32)>)> = if let Some(path) = &cli.candidates_in {
        read_candidates(path)
    } else {
        let queries: Vec<String> = if let Some(path) = &cli.words {
            fs::read_to_string(path)
                .unwrap_or_else(|e| {
                    eprintln!("rerank: reading {}: {e}", path.display());
                    std::process::exit(2);
                })
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        } else {
            cli.positional.clone()
        };
        if queries.is_empty() {
            eprintln!("rerank: no queries");
            std::process::exit(2);
        }
        queries
            .iter()
            .map(|w| {
                let sugs = suggest_detailed(w, cli.max_candidates, cli.max_distance);
                let list = sugs
                    .iter()
                    .map(|s| (s.word.clone(), s.edit_distance as i32))
                    .collect();
                (w.clone(), list)
            })
            .collect()
    };

    let model = ModelA::load(&cli.trm).unwrap_or_else(|e| {
        eprintln!("rerank: loading {}: {e}", cli.trm.display());
        std::process::exit(2);
    });
    let mut scratch = Scratch::default();

    if cli.header {
        println!("query\trank\tcandidate\tedit_distance\tlogit");
    }
    for (word, sugs) in &work {
        let cands: Vec<Candidate> = sugs.iter().map(|(w, e)| Candidate::new(w, *e)).collect();

        // Query features are computed once here and copied into every
        // candidate row by `score_batch`.
        let query = model.query_features(word);
        let logits = model.score_batch(&query, &cands, &mut scratch).to_vec();

        for (rank, &idx) in order_by_logit(&logits).iter().take(cli.top).enumerate() {
            // `{:e}` is Rust's shortest round-tripping form for f32;
            // `rank_agreement.py` parses both sides as floats and
            // compares within a tolerance, so the two formattings do
            // not have to agree -- only the values do.
            println!(
                "{}\t{}\t{}\t{}\t{:e}",
                word, rank, sugs[idx].0, sugs[idx].1, logits[idx],
            );
        }
    }
}
