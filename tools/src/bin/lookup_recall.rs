// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lookup_recall` -- how good is the reverse lookup, measured.
//!
//! Reads `(form, lemma)` pairs on stdin, one `TAB`-separated pair per line, and
//! for each asks the index which base forms the word has. Writes one TSV line
//! per pair to stdout:
//!
//! ```text
//! form  lemma  found  nbases  plain_top1  ranked_top1  len_ok  first_base
//! ```
//!
//! - `found` is 1 when `lemma` is among the word's base forms. This is the
//!   question the reverse lookup is asked -- which words *could* this be -- and
//!   the reference has exactly one answer to it per pair.
//! - `nbases` is how many base forms the word has, so a caller can see how much
//!   of the answer is ambiguity rather than the engine.
//! - `plain_top1` and `ranked_top1` are 1 when the word's first base is `lemma`
//!   under the sanalista's order and under [`ClassPrior`]. Both are reported
//!   because the ranking is a guess and a guess has to be measured against the
//!   alternative rather than argued for; the gate reads both.
//! - `len_ok` is 1 when the two orders hold the same bases. The ranking
//!   reorders and must never filter, so a word that comes back with fewer bases
//!   than it has is the one failure this could hide.
//! - `first_base` is the ranked first base itself, empty when there is none.
//!   `ranked_top1` only says whether it is the reference's lemma; the name is
//!   what lets a report ask whether it is *a* confirmed base of the surface,
//!   which is the right question when the word genuinely has several.
//!
//! The thresholds live in `tools/test_reverse_lookup_recall.py`, not here. A
//! measurement that grades itself cannot be disagreed with.
//!
//! Usage:
//!   lookup_recall < pairs.tsv > results.tsv
//!
//! This builds the whole index, so it is the slow path: see `lookup`.

use std::io::{BufRead, BufWriter, Write};
use std::process::ExitCode;

use torille::expand::{ClassPrior, FormBuilder};
use torille_tools::{catalogue, sanalista};

/// How many pairs are read before they are handed to the threads.
///
/// The batches are answered in order and the answer for a pair does not depend
/// on any other pair, so a batch is the unit that has to be kept whole for the
/// output to stay in input order -- and that is all the ordering the caller
/// needs. A batch is not a whole run: a run is 626 201 pairs over about four
/// minutes, and holding the input and the output of the whole of it to save that
/// ordering would cost more memory than the index already costs. Sixteen
/// thousand pairs is about a second's worth per thread, so the spawn cost is
/// invisible, and at that size the batch is still small enough that the slowest
/// thread does not hold up the write.
const BATCH: usize = 1 << 14;

/// The counts, kept apart from the lines so that answering a batch does not have
/// to walk it twice.
#[derive(Default)]
struct Tally {
    n: u64,
    found: u64,
    plain: u64,
    ranked: u64,
    by_size: [u64; 8],
    misses: Vec<String>,
}

impl Tally {
    /// How many of the first misses are still wanted, which is the number of
    /// report lines still owed. Each batch is answered in input order and its
    /// misses appended in that order, so "the first 200 overall" falls out of
    /// taking the first 200 of each batch until the tally is full.
    fn room(&self) -> usize {
        200usize.saturating_sub(self.misses.len())
    }
}

/// Answer every pair in one batch, in input order.
///
/// The work is split into more shares than there are threads and taken in order,
/// for the reason [`torille::expand::build`] gives: a pair that draws a long
/// word with twenty base forms takes much longer than one that draws a rare
/// word with none, and without more shares than threads the whole batch waits on
/// the unluckiest draw. `ranked_bases_of` reads the index and nothing else, so
/// the shares need no locking and the only thing they share is the index itself.
fn answer(
    batch: &[String],
    builder: &FormBuilder,
    prior: &ClassPrior,
    threads: usize,
) -> (Vec<String>, Tally) {
    let share = (batch.len() / (threads * 8)).max(64);
    let chunks: Vec<&[String]> = batch.chunks(share).collect();
    let answered: Vec<(Vec<String>, Tally)> = std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                scope.spawn(move || {
                    let mut lines = Vec::with_capacity(chunk.len());
                    let mut tally = Tally::default();
                    for line in chunk.iter() {
                        let Some((form, lemma)) = line.split_once('\t') else {
                            continue;
                        };
                        let bases = builder.bases_of(form);
                        let ranked = builder.ranked_bases_of(form, prior);
                        tally.n += 1;
                        let found = bases.contains(&lemma);
                        let plain_top1 = bases.first() == Some(&lemma);
                        let ranked_top1 = ranked.first() == Some(&lemma);
                        if found {
                            tally.found += 1;
                        } else if tally.misses.len() < 200 {
                            tally
                                .misses
                                .push(format!("{form}\t{lemma}\t{}", bases.join(",")));
                        }
                        if plain_top1 {
                            tally.plain += 1;
                        }
                        if ranked_top1 {
                            tally.ranked += 1;
                        }
                        tally.by_size[bases.len().min(7)] += 1;
                        lines.push(format!(
                            "{form}\t{lemma}\t{}\t{}\t{}\t{}\t{}\t{}",
                            found as u8,
                            bases.len(),
                            plain_top1 as u8,
                            ranked_top1 as u8,
                            (bases.len() == ranked.len()) as u8,
                            ranked.first().copied().unwrap_or(""),
                        ));
                    }
                    (lines, tally)
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    // The chunks are answered out of order and written back in order, because
    // the caller reads the TSV as a stream and a row that moved would be a row
    // that no longer says which pair it answers.
    let mut lines = Vec::with_capacity(batch.len());
    let mut total = Tally::default();
    for (mut chunk_lines, chunk_tally) in answered {
        lines.append(&mut chunk_lines);
        total.n += chunk_tally.n;
        total.found += chunk_tally.found;
        total.plain += chunk_tally.plain;
        total.ranked += chunk_tally.ranked;
        for (b, c) in total.by_size.iter_mut().zip(chunk_tally.by_size) {
            *b += c;
        }
        let room = total.room();
        total
            .misses
            .extend(chunk_tally.misses.into_iter().take(room));
    }
    (lines, total)
}

fn main() -> ExitCode {
    let cat = catalogue();
    // `expand::build` wants the rows, so this tool materialises them.
    let lexicon = sanalista().to_lemmas();
    let builder = torille::expand::build(&lexicon, &cat);
    // Counted over the same lexicon the index was built from, so the prior and
    // the index cannot disagree about what the sanalista says.
    let prior = ClassPrior::of(&lexicon);
    let (words, _, lemmas) = builder.stats();
    eprintln!("index: {words} words, {lemmas} lemmas");

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1);
    eprintln!("looking up on {threads} threads");

    let stdin = std::io::stdin();
    let mut out = BufWriter::new(std::io::stdout());
    let mut total = Tally::default();
    let mut batch: Vec<String> = Vec::with_capacity(BATCH);
    let run = |batch: &mut Vec<String>, out: &mut BufWriter<std::io::Stdout>, total: &mut Tally| {
        let (lines, tally) = answer(batch, &builder, &prior, threads);
        for line in &lines {
            let _ = out.write_all(line.as_bytes());
            let _ = out.write_all(b"\n");
        }
        total.n += tally.n;
        total.found += tally.found;
        total.plain += tally.plain;
        total.ranked += tally.ranked;
        for (b, c) in total.by_size.iter_mut().zip(tally.by_size) {
            *b += c;
        }
        let room = total.room();
        total.misses.extend(tally.misses.into_iter().take(room));
        batch.clear();
    };
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        batch.push(line);
        if batch.len() == BATCH {
            run(&mut batch, &mut out, &mut total);
        }
    }
    if !batch.is_empty() {
        run(&mut batch, &mut out, &mut total);
    }
    let _ = out.flush();

    let (n, found_n, plain_n, ranked_n) = (total.n, total.found, total.plain, total.ranked);
    let by_size = total.by_size;
    let misses = total.misses;
    let pct = |v: u64| v as f64 / n.max(1) as f64 * 100.0;
    eprintln!("\npairs {n}");
    for (label, count) in [
        ("the right base is among the answers", found_n),
        ("first base right, sanalista order", plain_n),
        ("first base right, ranked", ranked_n),
    ] {
        eprintln!("  {label:>38}  {count:>8}  {:>6.2}%", pct(count));
    }
    eprintln!("\nhow many base forms each word has:");
    for (i, c) in by_size.iter().enumerate() {
        let label = if i == 7 { "7 or more" } else { &i.to_string() };
        eprintln!("  {label:>9}: {c:>9}  {:>5.2}%", pct(*c));
    }
    if !misses.is_empty() {
        eprintln!("\nfirst misses (form, expected base, bases found):");
        for m in &misses {
            eprintln!("  {m}");
        }
    }
    ExitCode::SUCCESS
}
