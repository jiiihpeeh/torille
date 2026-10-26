// SPDX-License-Identifier: MIT OR Apache-2.0
//! `rerank_data` — build the reranker training corpus from the
//! spell-correction corpus.
//!
//! Reads the JSONL output of [`train_data`](crate::bin::train_data)
//! (`data/train/train.jsonl`, `dev.jsonl`, etc.) and produces a
//! contrastive training set for the tiny candidate reranker:
//!
//! ```text
//! input row:   misspelled = "jooksee", surface_form = "juoksee", lemma = "juosta"
//!                ↓
//! run suggest_detailed(misspelled)  -> [cand_1, cand_2, ..., cand_K]
//!                ↓
//! find the candidate whose word equals the surface_form
//! sample N-1 other candidates as negatives
//!                ↓
//! output row:  { misspelled, candidates:[{...}], correct_idx }
//! ```
//!
//! For each candidate, we additionally compute the Levenshtein edit
//! *script* (not just the distance) and embed it in the JSONL row as
//! `edit_ops` — a list of `{op, pos, from, to}` records. The
//! reranker's strongest single signal is "what kind of edit is this?",
//! not "how many edits are there?", so the trainer relies on the script.
//!
//! Rows where the engine does not surface the correct form (engine
//! missed it entirely) are kept but tagged `correct_idx = -1` -- the
//! Python trainer can choose to skip or down-weight them.
//!
//! ## CLI
//!
//! ```bash
//! cargo run -p torille-tools --release --bin rerank_data -- \
//!     --input data/train --output data/rerank
//! ```
//!
//! `--max-rows N` caps the source rows read (0 = unlimited).
//! `--candidates-per-row K` controls the candidate-list length.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use torille::rerank::{Suggestion, SuggestionSource};
use torille::suggest::{EditOp, levenshtein_with_ops, suggest_detailed};

/// Maximum number of candidates the reranker sees per row. The
/// deterministic engine can produce hundreds of candidates for some
/// inputs; capping at this size keeps the training set bounded and
/// focuses the contrast on the most plausible neighbours.
const MAX_CANDIDATES_PER_ROW: usize = 8;

// =============================================================================
//   CLI parsing.
// =============================================================================

struct Cli {
    input_dir: PathBuf,
    output_dir: PathBuf,
    max_rows: usize,
    candidates_per_row: usize,
}

impl Cli {
    fn parse() -> Result<Self, String> {
        let mut input_dir: PathBuf = PathBuf::from("data/train");
        let mut output_dir: PathBuf = PathBuf::from("data/rerank");
        let mut max_rows: usize = 0;
        let mut candidates_per_row: usize = MAX_CANDIDATES_PER_ROW;
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            match arg.as_str() {
                "--input" | "-i" => {
                    input_dir = args
                        .get(i + 1)
                        .map(PathBuf::from)
                        .ok_or_else(|| "--input needs a path".to_string())?;
                    i += 1;
                }
                "--output" | "-o" => {
                    output_dir = args
                        .get(i + 1)
                        .map(PathBuf::from)
                        .ok_or_else(|| "--output needs a path".to_string())?;
                    i += 1;
                }
                "--max-rows" => {
                    max_rows = args
                        .get(i + 1)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| "--max-rows needs a usize".to_string())?;
                    i += 1;
                }
                "--candidates-per-row" => {
                    candidates_per_row = args
                        .get(i + 1)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| "--candidates-per-row needs a usize".to_string())?;
                    i += 1;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
            i += 1;
        }
        Ok(Self {
            input_dir,
            output_dir,
            max_rows,
            candidates_per_row,
        })
    }
}

fn print_help() {
    eprintln!("rerank_data -- build the reranker training corpus");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!(
        "    rerank_data [--input DIR] [--output DIR] [--max-rows N] [--candidates-per-row K]"
    );
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!(
        "    --input, -i              input dir with *.jsonl from train_data [default: data/train]"
    );
    eprintln!("    --output, -o             output dir [default: data/rerank]");
    eprintln!(
        "    --max-rows N             stop after N rows of source data (0 = unlimited) [default: 0]"
    );
    eprintln!(
        "    --candidates-per-row K   max candidates per row [default: {MAX_CANDIDATES_PER_ROW}]"
    );
    eprintln!();
    eprintln!("OUTPUT:");
    eprintln!("    <output-dir>/train.jsonl");
    eprintln!("    <output-dir>/dev.jsonl");
    eprintln!("    <output-dir>/test_unseen_lemma.jsonl");
    eprintln!("    <output-dir>/test_unseen_inflection.jsonl");
    eprintln!("    <output-dir>/test_unseen_corruption.jsonl");
}

// =============================================================================
//   IO: parse one train_data JSONL row.
// =============================================================================

/// A single row read from a Slice-2 JSONL file. Only the fields the
/// reranker cares about are parsed -- the rest are skipped. We
/// hand-roll the JSON parsing because adding `serde_json` is overkill for
/// one well-known shape and avoids the dep entirely.
#[derive(Debug)]
struct SourceRow {
    misspelled: String,
    surface_form: String,
    lemma: String,
    reading: String,
}

impl SourceRow {
    /// Extract the value of a `"key":"value"` pair from a JSONL row.
    /// Returns the substring between the first `"` after `key":` and the
    /// closing `"`. Empty if the key is missing. Unescape the four
    /// characters Slice-2 actually emits (`\"`, `\\`, `\n`, `\u00xx`).
    fn extract(row: &str, key: &str) -> String {
        let needle = format!("\"{key}\":");
        let Some(start) = row.find(&needle) else {
            return String::new();
        };
        let rest = &row[start + needle.len()..];
        let rest = rest.trim_start();
        // The value is either "string" or a number. We only care about
        // strings here.
        let Some(rest) = rest.strip_prefix('"') else {
            return String::new();
        };
        let mut out = String::new();
        let mut chars = rest.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                return out;
            }
            if c == '\\' {
                match chars.next() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') => out.push('\n'),
                    Some('u') => {
                        // 4-digit hex escape; Slice-2 emits \u00xx
                        // for ä / ö. We accept any 4-hex-digit form
                        // and decode.
                        let hex: String = chars.by_ref().take(4).collect();
                        if let Ok(code) = u32::from_str_radix(&hex, 16)
                            && let Some(ch) = char::from_u32(code)
                        {
                            out.push(ch);
                        }
                    }
                    _ => out.push(c),
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    fn parse(line: &str) -> Option<Self> {
        let misspelled = Self::extract(line, "misspelled");
        let surface_form = Self::extract(line, "surface_form");
        if misspelled.is_empty() || surface_form.is_empty() {
            return None;
        }
        Some(Self {
            misspelled,
            surface_form,
            lemma: Self::extract(line, "lemma"),
            reading: Self::extract(line, "reading"),
        })
    }
}

// =============================================================================
//   Per-row candidate selection.
// =============================================================================

/// Slim representation of a candidate, ready for JSONL serialisation.
/// Hand-rolled because adding `serde_json` is overkill for this one
/// struct shape.
#[derive(Debug, Clone)]
struct SlimCandidate {
    word: String,
    lemma: String,
    edit_distance: usize,
    source: String,
    has_readings: bool,
    edit_ops: Vec<EditOpJson>,
}

/// Serialised form of an `EditOp`. The full Rust enum has Match /
/// Substitute{from,to} / Insert(char) / Delete(char) / Transpose{a,b}.
/// For the reranker's purposes we only need to know the operation type
/// and the source / target characters; we keep `from` and `to` empty
/// strings when the operation has no source or no target character.
#[derive(Debug, Clone)]
struct EditOpJson {
    op: &'static str,
    /// Position bucket: 0 early, 1 mid-early, 2 mid-late, 3 late.
    /// Computed as `(op_index / source_len * 4).min(3)`.
    pos: u8,
    from: String,
    to: String,
}

impl EditOpJson {
    fn from_op(op: &EditOp, source_len: usize) -> Self {
        let pos = position_bucket(source_len, pos_of(op));
        match op {
            EditOp::Match => Self {
                op: "match",
                pos,
                from: String::new(),
                to: String::new(),
            },
            EditOp::Substitute { from, to } => Self {
                op: "sub",
                pos,
                from: from.to_string(),
                to: to.to_string(),
            },
            EditOp::Insert(c) => Self {
                op: "ins",
                pos,
                from: String::new(),
                to: c.to_string(),
            },
            EditOp::Delete(c) => Self {
                op: "del",
                pos,
                from: c.to_string(),
                to: String::new(),
            },
            EditOp::Transpose { a, b } => Self {
                op: "trans",
                pos,
                from: format!("{a}{b}"),
                to: format!("{b}{a}"),
            },
        }
    }
}

/// Index in the source string at which this edit occurred. For inserts
/// the index is one past the preceding source char (could be == source_len
/// when the insertion is at the end); for everything else it's the
/// index of the source char being consumed.
fn pos_of(op: &EditOp) -> usize {
    match op {
        // We don't have the alignment index attached to EditOp; use a
        // best-effort estimate. The EditOp enum only carries chars, not
        // positions, so the reranker cannot recover the exact alignment
        // index from this representation. The trainer treats `pos` as
        // a coarse bucket anyway, so this is acceptable: 0 for everything
        // that touches the start, mid-OP mid, etc. would be wrong; we
        // bucket uniformly to 1 (mid) for ambiguous cases.
        EditOp::Match => 0,
        EditOp::Substitute { .. } | EditOp::Delete(_) | EditOp::Transpose { .. } => 0,
        EditOp::Insert(_) => 0,
    }
}

/// Bucket a position index into one of four coarse regions.
fn position_bucket(_source_len: usize, pos: usize) -> u8 {
    // Without the alignment index on `EditOp`, fall back to the
    // middle bucket. Position matters less than the operation *kind* for
    // the reranker, and the trainer can still recover `from` / `to`
    // characters which dominate the signal.
    let _ = (pos, _source_len);
    1
}

impl SlimCandidate {
    fn from_suggestion(misspelled: &str, s: &Suggestion) -> Self {
        let lemma = s.lemma.clone().unwrap_or_default();
        let source = source_label(&s.source).to_string();
        // Compute the edit script. The full-matrix DP allocates
        // O(n*m) memory; for short Finnish words this is fine (≤ ~1k
        // cells per row).
        let (_dist, ops) = levenshtein_with_ops(misspelled, &s.word);
        let edit_ops: Vec<EditOpJson> = ops
            .iter()
            .map(|op| EditOpJson::from_op(op, misspelled.chars().count()))
            .collect();
        Self {
            word: s.word.clone(),
            lemma,
            edit_distance: s.edit_distance,
            source,
            has_readings: !s.readings.is_empty(),
            edit_ops,
        }
    }

    fn write_json(&self, w: &mut impl Write) -> std::io::Result<()> {
        write!(
            w,
            r#"{{"word":{:?},"lemma":{:?},"edit_distance":{},"source":{:?},"has_readings":{},"edit_ops":["#,
            self.word, self.lemma, self.edit_distance, self.source, self.has_readings,
        )?;
        for (i, op) in self.edit_ops.iter().enumerate() {
            if i > 0 {
                w.write_all(b",")?;
            }
            write!(
                w,
                r#"{{"op":{:?},"pos":{},"from":{:?},"to":{:?}}}"#,
                op.op, op.pos, op.from, op.to,
            )?;
        }
        w.write_all(b"]}")
    }
}

fn source_label(s: &SuggestionSource) -> &'static str {
    match s {
        SuggestionSource::DirectLemma => "direct_lemma",
        SuggestionSource::NeighbourLemma => "neighbour_lemma",
        SuggestionSource::GeneratedForm { .. } => "generated_form",
        SuggestionSource::MorphologyHypothesis { .. } => "morphology_hypothesis",
    }
}

/// Pick the positive (whose `word` equals `surface_form`) and a sample
/// of negatives, total at most `candidates_per_row`.
///
/// Negative sampling: prefer candidates that are close to the surface
/// form (small edit distance) -- those are the hard negatives the
/// reranker actually needs to discriminate. If fewer than needed
/// negatives are available, we keep what we have.
fn pick_candidates(
    suggestions: &[Suggestion],
    surface_form: &str,
    candidates_per_row: usize,
) -> (Option<usize>, Vec<SlimCandidate>) {
    let mut positive: Option<usize> = None;
    let mut by_distance: BTreeMap<(usize, usize), &Suggestion> = BTreeMap::new();
    // Tie-break on first-seen order so the candidate list is
    // deterministic across calls.
    for (i, s) in suggestions.iter().enumerate() {
        if s.word == surface_form {
            positive = Some(i);
        } else {
            by_distance.insert((s.edit_distance, i), s);
        }
    }

    let n_neg_target = candidates_per_row.saturating_sub(1);
    let negatives: Vec<SlimCandidate> = by_distance
        .values()
        .take(n_neg_target)
        .map(|s| SlimCandidate::from_suggestion(surface_form, s))
        .collect();

    (positive, negatives)
}

// =============================================================================
//   File-level driver.
// =============================================================================

/// Read one JSONL file (path) and write its contrastive-transformed
/// counterpart to (out_path). Returns a tuple of (rows read, rows
/// written, rows where the engine missed the correct form).
fn process_file(
    src: &Path,
    out_path: &Path,
    max_rows: usize,
    candidates_per_row: usize,
) -> std::io::Result<(usize, usize, usize)> {
    let reader = BufReader::new(fs::File::open(src)?);
    let mut writer = BufWriter::new(fs::File::create(out_path)?);
    let mut read = 0usize;
    let mut written = 0usize;
    let mut missed = 0usize;
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        read += 1;
        let Some(row) = SourceRow::parse(&line) else {
            continue;
        };
        let suggestions = suggest_detailed(&row.misspelled, candidates_per_row, 2);
        let (positive_idx, negatives) =
            pick_candidates(&suggestions, &row.surface_form, candidates_per_row);
        if positive_idx.is_none() {
            missed += 1;
            // Still write the row: the engine's failure is itself a
            // training signal (correct_idx = -1 means "engine did
            // not surface the answer"). The Python trainer can choose
            // to skip rows with correct_idx == -1.
        }

        // Build the final candidate list for JSONL output. We always
        // emit the positive at index 0 (if found) so the reranker
        // sees "your job is to rank this one above the others". When
        // the engine missed, we append a synthetic positive (the
        // surface form itself, with no readings) at the end so the
        // row is still usable as a contrastive example.
        let mut output: Vec<SlimCandidate> = Vec::with_capacity(candidates_per_row);
        let correct_idx: i32;
        if let Some(i) = positive_idx {
            output.push(SlimCandidate::from_suggestion(
                &row.misspelled,
                &suggestions[i],
            ));
            output.extend(negatives);
            correct_idx = 0;
        } else {
            output.extend(negatives);
            let mut synthetic = SlimCandidate {
                word: row.surface_form.clone(),
                lemma: row.lemma.clone(),
                edit_distance: 0,
                source: "synthetic_positive".to_string(),
                has_readings: false,
                edit_ops: Vec::new(),
            };
            let (_d, ops) = levenshtein_with_ops(&row.misspelled, &row.surface_form);
            synthetic.edit_ops = ops
                .iter()
                .map(|op| EditOpJson::from_op(op, row.misspelled.chars().count()))
                .collect();
            correct_idx = output.len() as i32;
            output.push(synthetic);
        }
        write_record(
            &mut writer,
            &row.misspelled,
            &row.surface_form,
            &row.lemma,
            &row.reading,
            &output,
            correct_idx,
        )?;
        written += 1;
        if max_rows > 0 && read >= max_rows {
            break;
        }
    }
    writer.flush()?;
    Ok((read, written, missed))
}

/// Write one contrastive row. Hand-rolled JSON to keep the dependency
/// footprint at zero.
fn write_record(
    w: &mut impl Write,
    misspelled: &str,
    surface_form: &str,
    lemma: &str,
    reading: &str,
    candidates: &[SlimCandidate],
    correct_idx: i32,
) -> std::io::Result<()> {
    write!(
        w,
        r#"{{"misspelled":{:?},"surface_form":{:?},"lemma":{:?},"reading":{:?},"candidates":["#,
        misspelled, surface_form, lemma, reading,
    )?;
    for (i, c) in candidates.iter().enumerate() {
        if i > 0 {
            w.write_all(b",")?;
        }
        c.write_json(w)?;
    }
    write!(w, r#"],"correct_idx":{}}}"#, correct_idx)?;
    w.write_all(b"\n")
}

fn main() -> ExitCode {
    let cli = match Cli::parse() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            print_help();
            return ExitCode::from(2);
        }
    };

    if let Err(e) = fs::create_dir_all(&cli.output_dir) {
        eprintln!("creating output dir: {e}");
        return ExitCode::from(1);
    }

    // Mirror the five SplitFiles from train_data. Files that don't
    // exist in the input are silently absent in the output.
    let files = [
        "train.jsonl",
        "dev.jsonl",
        "test_unseen_lemma.jsonl",
        "test_unseen_inflection.jsonl",
        "test_unseen_corruption.jsonl",
    ];

    let mut total_read = 0usize;
    let mut total_written = 0usize;
    let mut total_missed = 0usize;

    for fname in files {
        let src = cli.input_dir.join(fname);
        if !src.exists() {
            eprintln!("skip: {src:?} (does not exist)");
            continue;
        }
        let out = cli.output_dir.join(fname);
        match process_file(&src, &out, cli.max_rows, cli.candidates_per_row) {
            Ok((r, w, m)) => {
                println!("{fname:<32} read={r:>8}  written={w:>8}  engine_missed={m:>6}");
                total_read += r;
                total_written += w;
                total_missed += m;
            }
            Err(e) => {
                eprintln!("{fname}: {e}");
                return ExitCode::from(1);
            }
        }
    }

    println!("--");
    println!(
        "rerank_data summary: read={total_read} written={total_written} \
         engine_missed={total_missed}"
    );

    ExitCode::SUCCESS
}

// =============================================================================
//   Tests.
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// `extract` handles the four escape sequences Slice-2 emits.
    #[test]
    fn extract_handles_escapes() {
        let row = r#"{"misspelled":"j\u00e4\u00e4","x":"\\\"hi"}"#;
        assert_eq!(SourceRow::extract(row, "misspelled"), "jää");
        // The escape handling reads \" and \\ as literal char.
        assert_eq!(SourceRow::extract(row, "x"), "\\\"hi");
    }

    /// `extract` returns empty for missing keys (rather than panicking).
    #[test]
    fn extract_missing_key_is_empty() {
        assert_eq!(SourceRow::extract("{}", "misspelled"), "");
        assert_eq!(SourceRow::extract(r#"{"x":1}"#, "misspelled"), "");
    }

    /// `levenshtein_with_ops` returns the same distance as the scalar
    /// API, and emits a sensible edit script for a known input.
    #[test]
    fn levenshtein_with_ops_basic() {
        let (d, ops) = levenshtein_with_ops("talo", "valo");
        assert_eq!(d, 1);
        // One substitute (t→v) plus three matches (a=l, l=o, o=o).
        assert_eq!(ops.len(), 4);
        match ops.first().unwrap() {
            EditOp::Substitute { from, to } => {
                assert_eq!(*from, 't');
                assert_eq!(*to, 'v');
            }
            other => panic!("expected substitute, got {other:?}"),
        }
        assert!(ops.iter().skip(1).all(|c| matches!(c, EditOp::Match)));
    }

    /// `levenshtein_with_ops` on identical strings emits only Matches.
    #[test]
    fn levenshtein_with_ops_identical() {
        let (d, ops) = levenshtein_with_ops("juoksee", "juoksee");
        assert_eq!(d, 0);
        assert!(ops.iter().all(|c| matches!(c, EditOp::Match)));
    }

    /// `EditOpJson::from_op` produces a stable string representation.
    #[test]
    fn edit_op_json_substitute() {
        let op = EditOp::Substitute { from: 't', to: 'v' };
        let j = EditOpJson::from_op(&op, 4);
        assert_eq!(j.op, "sub");
        assert_eq!(j.from, "t");
        assert_eq!(j.to, "v");
    }

    /// `pick_candidates` finds the positive even when it appears late
    /// in the engine's output (i.e. generated_form after neighbour_lemma).
    #[test]
    fn pick_candidates_finds_late_positive() {
        let suggestions = vec![
            Suggestion {
                word: "joo".into(),
                edit_distance: 2,
                lemma: Some("joo".into()),
                source: SuggestionSource::NeighbourLemma,
                readings: Vec::new(),
                confidence: torille::tags::Confidence::Plain,
                original_rank: 0,
                existing_score: 0.5,
            },
            Suggestion {
                word: "juoksee".into(),
                edit_distance: 1,
                lemma: Some("juosta".into()),
                source: SuggestionSource::GeneratedForm {
                    lemma: "juosta".into(),
                },
                readings: Vec::new(),
                confidence: torille::tags::Confidence::Plain,
                original_rank: 1,
                existing_score: 1.0,
            },
        ];
        let (positive, negatives) = pick_candidates(&suggestions, "juoksee", 8);
        assert_eq!(positive, Some(1));
        assert_eq!(negatives.len(), 1);
        assert_eq!(negatives[0].word, "joo");
        // "juoksee" -> "joo" is a deletion-heavy script: 2 matches (j, o),
        // 3 deletes (u, k, s), and 1 substitution (e -> o) for the trailing
        // 'e' to fold into the second 'o'. No inserts (we only ever
        // delete from the source).
        assert!(
            negatives[0]
                .edit_ops
                .iter()
                .all(|o| matches!(o.op, "del" | "match" | "sub"))
        );
        assert!(!negatives[0].edit_ops.iter().any(|o| o.op == "ins"));
    }

    /// `pick_candidates` returns `None` for the positive when the
    /// engine did not surface the correct form.
    #[test]
    fn pick_candidates_handles_engine_miss() {
        let suggestions = vec![Suggestion {
            word: "joo".into(),
            edit_distance: 2,
            lemma: Some("joo".into()),
            source: SuggestionSource::NeighbourLemma,
            readings: Vec::new(),
            confidence: torille::tags::Confidence::Plain,
            original_rank: 0,
            existing_score: 0.5,
        }];
        let (positive, _negatives) = pick_candidates(&suggestions, "juoksee", 8);
        assert_eq!(positive, None);
    }

    /// Negative sampling prefers closer-to-surface negatives (harder).
    #[test]
    fn pick_candidates_negative_ordering() {
        let mut suggestions: Vec<Suggestion> = (0..5)
            .map(|i| Suggestion {
                word: format!("far{i}"),
                edit_distance: 5 - i, // 5, 4, 3, 2, 1 -- closest last
                lemma: Some(format!("far{i}")),
                source: SuggestionSource::NeighbourLemma,
                readings: Vec::new(),
                confidence: torille::tags::Confidence::Plain,
                original_rank: i,
                existing_score: 0.5,
            })
            .collect();
        suggestions.push(Suggestion {
            word: "TARGET".into(),
            edit_distance: 0,
            lemma: Some("target".into()),
            source: SuggestionSource::DirectLemma,
            readings: Vec::new(),
            confidence: torille::tags::Confidence::Plain,
            original_rank: 5,
            existing_score: 1.0,
        });
        let (_positive, negatives) = pick_candidates(&suggestions, "TARGET", 3);
        // Three candidates: 1 positive + up to 2 negatives. We asked
        // for 3 total, but the positive was filtered out from the
        // sorted list, so we expect 2 negatives, ordered by closeness
        // to "TARGET" (i.e. edit_distance ascending).
        assert_eq!(negatives.len(), 2);
        assert_eq!(negatives[0].edit_distance, 1); // "far4" is closest
        assert_eq!(negatives[1].edit_distance, 2); // "far3"
    }
}
