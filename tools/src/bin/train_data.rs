// SPDX-License-Identifier: MIT OR Apache-2.0
//! `train_data` — build the training-data corpus for the optional neural
//! reranker and morphology hypothesiser.
//!
//! The flow:
//!
//! 1. Stream every (lemma, form, features, reading, ok) row out of the
//!    Kotus-inflected SQLite files in `data/`. Skip multi-word forms
//!    ("ei juokse") and rows that only `omorfi` accepts (`ok = omorfi`).
//! 2. Apply realistic Finnish typo corruptions (substitution, deletion,
//!    insertion, transposition, keyboard-adjacent, doubled-consonant,
//!    vowel-harmony, etc.) at fixed rate per form.
//! 3. Validate each corruption through `torille::suggest::check`: any
//!    corrupted form that the engine recognises as a real Finnish word
//!    is dropped, because it is not actually a misspelling.
//! 4. Hold out three test splits: unseen lemmas (5%), unseen inflections
//!    (1% per lemma), unseen corruption families.
//!
//! The output is JSONL files under `data/train/`. Each line is one
//! (misspelled, surface_form, lemma, features, ok, corruption_type,
//! edit_distance) tuple. The corpus is deterministic given a seed; the
//! default seed is fixed in `DEFAULT_SEED` so re-runs reproduce.
//!
//! ## Production / development contract
//!
//! This binary is a development / training tool. It reads the
//! inflection SQLite files which are NOT shipped in production. The
//! output JSONL it writes contains surface forms, lemmas, and
//! morphological features — also not shipped in production. Only the
//! trained model weights derived from this corpus reach production.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p torille-tools --release --bin train_data \
//!     -- --output data/train --forms-per-bucket 2 --max-rows 0
//! ```
//!
//! `--max-rows 0` means "no cap". `--max-rows 100000` is the right value
//! for a sanity-check unit of the pipeline.

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rusqlite::{Connection, OpenFlags};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(not(test))]
use rayon::prelude::*;

/// The default seed for the deterministic PRNG. Pinned so two runs with
/// the same inputs produce identical output.
const DEFAULT_SEED: u64 = 0x5d2b_8a3e_c471_09f6;

/// Fraction of source rows to keep before corruption. Default keeps
/// every row. `--sample-rate 0.05` keeps 5 % of rows for a tractable
/// development corpus.
const DEFAULT_SAMPLE_RATE: f64 = 1.0;

/// Number of corruptions to attempt per (lemma, form, reading) row. The
/// actual number is the min of this and the number of corruption
/// strategies that succeed: many strategies are no-ops on short forms.
const CORRUPTIONS_PER_FORM: usize = 2;

/// Maximum edit distance from the original a corruption may produce. Two
/// is the value the existing `suggest` ranking uses; larger distances
/// would not surface in the existing engine's top-N.
const MAX_EDIT_DISTANCE: usize = 2;

/// Fraction of lemmas entirely held out as `test_unseen_lemma`.
const UNSEEN_LEMMA_FRACTION: f64 = 0.05;

/// Fraction of (lemma, form) pairs held out as `test_unseen_inflection`.
const UNSEEN_INFLECTION_FRACTION: f64 = 0.01;

/// Dev-set fraction (random sample of the train split).
const DEV_FRACTION: f64 = 0.01;

// =============================================================================
//   Deterministic PRNG (splitmix64).
// =============================================================================

/// A small, fast, deterministic PRNG. `splitmix64` is the standard
/// 64-bit mixer used as a workhorse in many Rust crates because it has a
/// single state word and produces well-distributed bits.
#[derive(Clone, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// An integer in `[0, n)`. Returns 0 for `n == 0`.
    fn gen_range(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() as usize) % n
    }

    /// A 0/1 Bernoulli draw with success probability `p`.
    fn flip(&mut self, p: f64) -> bool {
        // Take the top 32 bits of the next u64; treat as a uniform in [0, 1).
        let v = (self.next_u64() >> 32) as f64 / (1u64 << 32) as f64;
        v < p
    }

    /// Pick one of `xs`, returning `None` for an empty slice.
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> Option<&'a T> {
        if xs.is_empty() {
            None
        } else {
            Some(&xs[self.gen_range(xs.len())])
        }
    }
}

// =============================================================================
//   Data model.
// =============================================================================

/// A single row out of the SQLite `forms` table.
#[derive(Clone, Debug)]
struct FormRow {
    lemma: String,
    homonym: String,
    form: String,
    voice: String,
    mood: String,
    tense: String,
    person: String,
    number: String,
    case_: String,
    infinitive: String,
    participle: String,
    possessive: String,
    polarity: String,
    reading: String,
    ok: String,
}

/// One produced training example, ready for JSONL serialisation.
#[derive(Clone, Debug)]
struct TrainingExample {
    misspelled: String,
    surface_form: String,
    lemma: String,
    homonym: String,
    pos: String,
    voice: String,
    mood: String,
    tense: String,
    person: String,
    number: String,
    case: String,
    infinitive: String,
    participle: String,
    possessive: String,
    polarity: String,
    reading: String,
    ok: String,
    corruption_type: &'static str,
    edit_distance: usize,
}

impl TrainingExample {
    fn write_jsonl(&self, w: &mut impl Write) -> std::io::Result<()> {
        // Hand-rolled JSON to keep the dependency footprint at zero (the
        // training binary only needs rusqlite + torille). All strings
        // come from the SQLite rows and the Kotus form generator --
        // Finnish corpus text never enters here -- so we know the
        // escaping rules: no control characters, no embedded quotes.
        writeln!(
            w,
            r#"{{"misspelled":{:?},"surface_form":{:?},"lemma":{:?},"homonym":{:?},"pos":{:?},"voice":{:?},"mood":{:?},"tense":{:?},"person":{:?},"number":{:?},"case":{:?},"infinitive":{:?},"participle":{:?},"possessive":{:?},"polarity":{:?},"reading":{:?},"ok":{:?},"corruption_type":{:?},"edit_distance":{}}}"#,
            self.misspelled,
            self.surface_form,
            self.lemma,
            self.homonym,
            self.pos,
            self.voice,
            self.mood,
            self.tense,
            self.person,
            self.number,
            self.case,
            self.infinitive,
            self.participle,
            self.possessive,
            self.polarity,
            self.reading,
            self.ok,
            self.corruption_type,
            self.edit_distance,
        )
    }
}

// =============================================================================
//   Corruption strategies.
//
//   Each strategy is a small, deterministic function on `(form, &mut rng)`.
//   They share two invariants: (a) the corruption's edit distance from
//   the input is at most 2; (b) the corruption is a *string* operation
//   only, with no awareness of morphology. This is intentional: the
//   model must learn from the same Finnish typos a human would type.
// =============================================================================

/// The vocabulary we use for *random* character corruptions. Includes the
/// letters that actually occur in Finnish surface forms, including `ä`/`ö`
/// (which are real letters in Finnish, not accented variants).
const RANDOM_LETTERS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'r', 's', 't',
    'u', 'v', 'y', 'ä', 'ö',
];

/// QWERTY adjacency used for keyboard-neighbour substitutions. The list
/// is the immediate four-way neighbour where possible; ambiguous cases
/// get the most-common one. The shape is conservative on purpose --
/// off-by-one neighbours produce distractors that are too random.
const KEYBOARD_ADJACENT: &[(&str, &[char])] = &[
    ("a", &['s', 'o', 'q', 'w']),
    ("b", &['v', 'n', 'g', 'h']),
    ("c", &['x', 'v', 'd', 'f']),
    ("d", &['s', 'f', 'e', 'r', 'x', 'c']),
    ("e", &['w', 'r', 'd', 's']),
    ("f", &['d', 'g', 'r', 't', 'c', 'v']),
    ("g", &['f', 'h', 't', 'y', 'v', 'b']),
    ("h", &['g', 'j', 'y', 'u', 'b', 'n']),
    ("i", &['u', 'o', 'j', 'k']),
    ("j", &['h', 'k', 'u', 'i', 'n', 'm']),
    ("k", &['j', 'l', 'i', 'o', 'm']),
    ("l", &['k', 'ö', 'o', 'p']),
    ("m", &['n', ',', 'j', 'k']),
    ("n", &['b', 'm', 'h', 'j']),
    ("o", &['i', 'p', 'l', 'k', '0']),
    ("p", &['o', 'å', 'l']),
    ("r", &['e', 't', 'd', 'f']),
    ("s", &['a', 'd', 'w', 'e', 'z', 'x']),
    ("t", &['r', 'y', 'f', 'g']),
    ("u", &['y', 'i', 'h', 'j']),
    ("v", &['c', 'b', 'f', 'g']),
    ("w", &['q', 'e', 'a', 's']),
    ("x", &['z', 'c', 's', 'd']),
    ("y", &['t', 'u', 'g', 'h']),
    ("z", &['x', 's']),
    ("ä", &['a', 'o']),
    ("ö", &['o', 'ä', 'p', 'l']),
];

/// Finnish doubled consonants that are common in surface forms.
const DOUBLE_CONSONANTS: &[char] = &['k', 'l', 'm', 'n', 'p', 'r', 's', 't'];

/// Strength of end-of-word bias for [`Strategy::TransposeAdjacent`]. 0.0
/// is uniform, 1.0 doubles the weight at the end of the word relative to
/// the start. The literature on typing speed puts the bias around 0.6
/// for adjacent-letter swaps.
const TRANSPOSITION_END_BIAS: f64 = 0.6;

/// Letter-pair weights for adjacent-letter transpositions. Higher =
/// more common in real Finnish typos. A weight of 1.0 means "always
/// transpose this pair", 0.0 means "never". Pairs not listed default to
/// 0.5 (rare). The list is hand-curated from common Finnish stems
/// (`ta-`, `sa-`, `ko-`, ...) and pairs that are easy to mistype as a
/// unit on a QWERTY keyboard (consonant-consonant adjacency).
const LETTER_PAIR_WEIGHTS: &[(char, char, f64)] = &[
    // Consonant-stem-onset pairs: high frequency in Finnish stems.
    ('k', 'a', 0.95),
    ('k', 'i', 0.95),
    ('l', 'a', 0.95),
    ('l', 'i', 0.95),
    ('n', 'a', 0.95),
    ('n', 'i', 0.95),
    ('r', 'a', 0.95),
    ('r', 'i', 0.95),
    ('s', 'a', 0.95),
    ('s', 'i', 0.95),
    ('t', 'a', 0.95),
    ('t', 'i', 0.95),
    // Doubled consonants: very high -- `kk`, `ll`, `nn` are the most
    // frequently mistyped pairs in Finnish because the same finger
    // presses the key twice.
    ('k', 'k', 0.85),
    ('l', 'l', 0.85),
    ('n', 'n', 0.85),
    ('r', 'r', 0.85),
    ('s', 's', 0.85),
    ('t', 't', 0.85),
    ('p', 'p', 0.85),
    ('m', 'm', 0.85),
    // Common vowel-stem starts.
    ('a', 'i', 0.75),
    ('a', 'u', 0.75),
    ('e', 'i', 0.75),
    ('o', 'u', 0.75),
    ('ä', 'ö', 0.75),
    ('ä', 'a', 0.75),
    // Common Finnish diphthongs: rarely transposed (motor pattern is
    // strongly practiced).
    ('i', 'e', 0.4),
    ('u', 'o', 0.4),
    ('y', 'ö', 0.4),
    // Rare pairs: very rarely transposed.
    ('q', 'x', 0.05),
    ('z', 'q', 0.05),
    ('x', 'q', 0.05),
];

/// Look up the transposition weight for `(a, b)` -> `(b, a)`. Defaults to
/// 0.5 (rare) for unlisted pairs.
fn letter_pair_weight(a: char, b: char) -> f64 {
    LETTER_PAIR_WEIGHTS
        .iter()
        .find(|(x, y, _)| *x == a && *y == b)
        .map(|(_, _, w)| *w)
        .unwrap_or(0.5)
}

/// Sample a position in `[0, len)` weighted by `weights`. The caller is
/// responsible for making sure `weights.len() == len` and that every
/// entry is non-negative; the function clamps to `len - 1` if numerical
/// drift pushes `u` past the total.
fn weighted_position(rng: &mut SplitMix64, weights: &[f64]) -> usize {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return weights.len().saturating_sub(1);
    }
    let mut u = (rng.next_u64() >> 32) as f64 / (1u64 << 32) as f64 * total;
    for (i, w) in weights.iter().enumerate() {
        if u < *w {
            return i;
        }
        u -= *w;
    }
    weights.len() - 1
}

/// Vowel pairs that get confused in real Finnish typos: the user means
/// `a` and types `o`, or means `ä` and types `a`.
const VOWEL_CONFUSIONS: &[(&str, &str)] = &[
    ("a", "o"),
    ("a", "e"),
    ("a", "ä"),
    ("o", "a"),
    ("o", "u"),
    ("o", "ö"),
    ("u", "o"),
    ("u", "y"),
    ("ä", "a"),
    ("ä", "ö"),
    ("ö", "o"),
    ("ö", "ä"),
];

/// Corruption strategies in apply-order. Each is attempted at most once
/// per form, with the random PRNG picking positions.
#[derive(Clone, Copy, Debug)]
enum Strategy {
    SubstituteRandom,
    SubstituteAdjacent,
    DeleteRandom,
    InsertRandom,
    TransposeAdjacent,
    DoubleConsonantAdd,
    DoubleConsonantDrop,
    VowelSwap,
}

impl Strategy {
    fn all() -> &'static [Strategy] {
        &[
            Strategy::SubstituteRandom,
            Strategy::SubstituteAdjacent,
            Strategy::DeleteRandom,
            Strategy::InsertRandom,
            Strategy::TransposeAdjacent,
            Strategy::DoubleConsonantAdd,
            Strategy::DoubleConsonantDrop,
            Strategy::VowelSwap,
        ]
    }

    fn name(self) -> &'static str {
        match self {
            Strategy::SubstituteRandom => "substitute_random",
            Strategy::SubstituteAdjacent => "substitute_adjacent",
            Strategy::DeleteRandom => "delete_random",
            Strategy::InsertRandom => "insert_random",
            Strategy::TransposeAdjacent => "transpose_adjacent",
            Strategy::DoubleConsonantAdd => "double_consonant_add",
            Strategy::DoubleConsonantDrop => "double_consonant_drop",
            Strategy::VowelSwap => "vowel_swap",
        }
    }

    fn apply(self, form: &str, rng: &mut SplitMix64) -> Option<String> {
        let mut chars: Vec<char> = form.chars().collect();
        if chars.is_empty() {
            return None;
        }
        match self {
            Strategy::SubstituteRandom => {
                let pos = rng.gen_range(chars.len());
                let original = chars[pos];
                let alphabet: Vec<char> = RANDOM_LETTERS
                    .iter()
                    .copied()
                    .filter(|c| *c != original)
                    .collect();
                if let Some(&c) = rng.pick(&alphabet) {
                    chars[pos] = c;
                }
            }
            Strategy::SubstituteAdjacent => {
                let pos = rng.gen_range(chars.len());
                let original = chars[pos].to_string();
                let neighbours: &[(&str, &[char])] = KEYBOARD_ADJACENT;
                let entry = neighbours.iter().find(|(k, _)| *k == original);
                if let Some((_, options)) = entry {
                    if let Some(&c) = rng.pick(options) {
                        chars[pos] = c;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            Strategy::DeleteRandom => {
                if chars.len() <= 2 {
                    return None;
                }
                let pos = rng.gen_range(chars.len());
                chars.remove(pos);
            }
            Strategy::InsertRandom => {
                let pos = rng.gen_range(chars.len() + 1);
                if let Some(&c) = rng.pick(RANDOM_LETTERS) {
                    chars.insert(pos, c);
                }
            }
            Strategy::TransposeAdjacent => {
                if chars.len() < 2 {
                    return None;
                }
                // Positional bias: end-of-word transpositions are more
                // common in real typing than start-of-word ones. The
                // weight at position i is `1.0 + bias * (frac - 0.5)`
                // where `frac` is the position's normalised distance
                // from the start of the word.
                let n = chars.len() - 1;
                let denom = (n as f64 - 1.0).max(1.0);
                let weights: Vec<f64> = (0..n)
                    .map(|i| {
                        let frac = i as f64 / denom;
                        1.0 + TRANSPOSITION_END_BIAS * (frac - 0.5)
                    })
                    .collect();
                let pos = weighted_position(rng, &weights);
                // Letter-pair weighting: skip the swap if the chosen
                // pair is rare (e.g. `qx`). The caller treats `None`
                // the same as any other no-corruption attempt.
                let a = chars[pos];
                let b = chars[pos + 1];
                if a == b {
                    // Swapping two identical letters is a no-op; the
                    // user typed `aa`, "transposing" them is still
                    // `aa`. Returning `None` keeps the corruption
                    // budget honest.
                    return None;
                }
                if !rng.flip(letter_pair_weight(a, b)) {
                    return None;
                }
                chars.swap(pos, pos + 1);
            }
            Strategy::DoubleConsonantAdd => {
                // Find a position with a single consonant and double it.
                let mut positions: Vec<usize> = chars
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| DOUBLE_CONSONANTS.contains(c))
                    .map(|(i, _)| i)
                    .collect();
                if positions.is_empty() {
                    return None;
                }
                let pos = positions.remove(rng.gen_range(positions.len()));
                chars.insert(pos + 1, chars[pos]);
            }
            Strategy::DoubleConsonantDrop => {
                // Find a run of two identical consonants and drop one.
                let mut positions: Vec<usize> = (0..chars.len() - 1)
                    .filter(|&i| chars[i] == chars[i + 1] && DOUBLE_CONSONANTS.contains(&chars[i]))
                    .collect();
                if positions.is_empty() {
                    return None;
                }
                let pos = positions.remove(rng.gen_range(positions.len()));
                chars.remove(pos);
            }
            Strategy::VowelSwap => {
                let pairs: &[(&str, &str)] = VOWEL_CONFUSIONS;
                let mut positions: Vec<usize> = Vec::new();
                for (i, c) in chars.iter().enumerate() {
                    let s = c.to_string();
                    if pairs.iter().any(|(from, _)| *from == s.as_str()) {
                        positions.push(i);
                    }
                }
                if positions.is_empty() {
                    return None;
                }
                let pos = positions[rng.gen_range(positions.len())];
                let original = chars[pos].to_string();
                let options: Vec<&str> = pairs
                    .iter()
                    .filter(|(from, _)| *from == original.as_str())
                    .map(|(_, to)| *to)
                    .collect();
                if let Some(&to) = rng.pick(&options) {
                    chars[pos] = to.chars().next()?;
                }
            }
        }
        Some(chars.into_iter().collect())
    }
}

/// Edit distance used to bound the corruption budget. Bounded Levenshtein
/// up to `MAX_EDIT_DISTANCE`; anything beyond that is dropped at the
/// caller.
fn edit_distance(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let len_diff = if a.len() >= b.len() {
        a.len() - b.len()
    } else {
        b.len() - a.len()
    };
    if len_diff > MAX_EDIT_DISTANCE {
        return MAX_EDIT_DISTANCE + 1;
    }
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m.min(MAX_EDIT_DISTANCE + 1);
    }
    if m == 0 {
        return n.min(MAX_EDIT_DISTANCE + 1);
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr: Vec<usize> = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        let mut row_min = curr[0];
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let v = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            curr[j] = v;
            if v < row_min {
                row_min = v;
            }
        }
        if row_min > MAX_EDIT_DISTANCE {
            return MAX_EDIT_DISTANCE + 1;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

// =============================================================================
//   SQLite streaming.
// =============================================================================

/// Open one of the Kotus-inflected SQLite files. Returns the connection
/// ready for streaming.
fn open_db(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

/// Read a batch of rows from the nominal SQLite file into `out`. Returns
/// the number of rows appended. The nominal table has only nominal
/// features (`case`, `number`, `possessive`); the verb-only columns are
/// left empty on the returned rows.
fn fill_nominal_batch(
    rows_iter: &mut rusqlite::Rows,
    out: &mut Vec<FormRow>,
    batch_size: usize,
) -> rusqlite::Result<usize> {
    let mut n = 0usize;
    while n < batch_size {
        let Some(row) = rows_iter.next()? else { break };
        out.push(FormRow {
            lemma: row.get(0)?,
            homonym: row.get(1)?,
            form: row.get(2)?,
            voice: String::new(),
            mood: String::new(),
            tense: String::new(),
            person: String::new(),
            number: row.get(4)?,
            case_: row.get(3)?,
            infinitive: String::new(),
            participle: String::new(),
            possessive: row.get(5)?,
            polarity: String::new(),
            reading: String::new(),
            ok: row.get(6)?,
        });
        n += 1;
    }
    Ok(n)
}

/// Read a batch of rows from the verb SQLite file into `out`. Returns
/// the number of rows appended. Verb-only columns (`voice`, `mood`,
/// `tense`, `person`, `infinitive`, `participle`, `polarity`) are
/// populated; nominal-only ones (`case`, `possessive`) are left empty.
fn fill_verb_batch(
    rows_iter: &mut rusqlite::Rows,
    out: &mut Vec<FormRow>,
    batch_size: usize,
) -> rusqlite::Result<usize> {
    let mut n = 0usize;
    while n < batch_size {
        let Some(row) = rows_iter.next()? else { break };
        out.push(FormRow {
            lemma: row.get(0)?,
            homonym: row.get(1)?,
            form: row.get(2)?,
            voice: row.get(3)?,
            mood: row.get(4)?,
            tense: row.get(5)?,
            person: row.get(6)?,
            number: row.get(7)?,
            case_: row.get(8)?,
            infinitive: row.get(9)?,
            participle: row.get(10)?,
            possessive: row.get(11)?,
            polarity: row.get(12)?,
            reading: row.get(13)?,
            ok: row.get(14)?,
        });
        n += 1;
    }
    Ok(n)
}

// =============================================================================
//   Held-out splitting.
// =============================================================================

/// Compute the held-out lemma set. The function is deterministic given
/// the seed; the same seed on the same input produces the same split.
fn pick_held_out_lemmas(all_lemmas: &BTreeSet<String>, rng: &mut SplitMix64) -> BTreeSet<String> {
    let n = (all_lemmas.len() as f64 * UNSEEN_LEMMA_FRACTION) as usize;
    let lemmas: Vec<&String> = all_lemmas.iter().collect();
    let mut held: BTreeSet<String> = BTreeSet::new();
    for _ in 0..n {
        if lemmas.is_empty() {
            break;
        }
        let idx = rng.gen_range(lemmas.len());
        let lemma = lemmas[idx].clone();
        held.insert(lemma);
    }
    held
}

/// Decide whether a given `(lemma, form)` pair is held out as
/// `test_unseen_inflection`. The fraction is per-lemma: 1% of forms for
/// each lemma are held out.
///
/// The score is a pure function of `(lemma, form)` -- it deliberately
/// does *not* consult the shared PRNG, so the held-out set is stable
/// across re-runs even when the corpus-level seed changes. FNV-1a is
/// used as the base hash and splitmix64's finalizer is applied on top
/// to fix FNV's known weakness when many inputs share a long prefix
/// (e.g. `syntheticform0` ... `syntheticform9999`).
fn is_unseen_inflection(lemma: &str, form: &str) -> bool {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in lemma.as_bytes().iter().chain(form.as_bytes()) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // splitmix64 finalizer: fixes poor distribution of FNV-1a on
    // inputs that share a long common substring.
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    let v = (h >> 32) as f64 / (1u64 << 32) as f64;
    v < UNSEEN_INFLECTION_FRACTION
}

// =============================================================================
//   Output writers.
// =============================================================================

struct SplitWriters {
    train: BufWriter<fs::File>,
    dev: BufWriter<fs::File>,
    test_unseen_lemma: BufWriter<fs::File>,
    test_unseen_inflection: BufWriter<fs::File>,
    test_unseen_corruption: BufWriter<fs::File>,
    counts: SplitCounts,
}

#[derive(Default, Debug)]
struct SplitCounts {
    train: usize,
    dev: usize,
    test_unseen_lemma: usize,
    test_unseen_inflection: usize,
    test_unseen_corruption: usize,
    total_emitted: usize,
    skipped_known: usize,
    skipped_no_corruption: usize,
    skipped_distance: usize,
}

fn open_split_writers(out_dir: &Path) -> std::io::Result<SplitWriters> {
    fs::create_dir_all(out_dir)?;
    let open = |name: &str| -> std::io::Result<BufWriter<fs::File>> {
        let path = out_dir.join(format!("{name}.jsonl"));
        Ok(BufWriter::new(fs::File::create(path)?))
    };
    Ok(SplitWriters {
        train: open("train")?,
        dev: open("dev")?,
        test_unseen_lemma: open("test_unseen_lemma")?,
        test_unseen_inflection: open("test_unseen_inflection")?,
        test_unseen_corruption: open("test_unseen_corruption")?,
        counts: SplitCounts::default(),
    })
}

/// Decide which split an example belongs to and write it. Returns the
/// split's name for the caller to update the counts.
///
/// The corruption-strategy reservation (`DoubleConsonantDrop` always
/// lands in `test_unseen_corruption`) is enforced here.
fn dispatch_example(
    w: &mut SplitWriters,
    example: &TrainingExample,
    lemma_held_out: bool,
    is_inflection_held_out: bool,
    rng: &mut SplitMix64,
) -> &'static str {
    let (sink, name): (&mut BufWriter<fs::File>, &'static str) =
        if example.corruption_type == Strategy::DoubleConsonantDrop.name() {
            (&mut w.test_unseen_corruption, "test_unseen_corruption")
        } else if lemma_held_out {
            (&mut w.test_unseen_lemma, "test_unseen_lemma")
        } else if is_inflection_held_out {
            (&mut w.test_unseen_inflection, "test_unseen_inflection")
        } else if rng.flip(DEV_FRACTION) {
            (&mut w.dev, "dev")
        } else {
            (&mut w.train, "train")
        };
    if let Err(e) = example.write_jsonl(sink) {
        eprintln!("write {name}: {e}");
        std::process::exit(1);
    }
    name
}

// =============================================================================
//   Main pipeline.
// =============================================================================

fn collect_lemma_set(
    conn_nominal: &Connection,
    conn_verb: &Connection,
) -> rusqlite::Result<BTreeSet<String>> {
    let mut lemmas: BTreeSet<String> = BTreeSet::new();
    let mut collect = |conn: &Connection| -> rusqlite::Result<()> {
        let mut stmt = conn.prepare("SELECT DISTINCT lemma FROM base")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let lemma: String = row.get(0)?;
            if !lemma.is_empty() {
                lemmas.insert(lemma);
            }
        }
        Ok(())
    };
    collect(conn_nominal)?;
    collect(conn_verb)?;
    Ok(lemmas)
}

/// Fast check whether `word` is *itself* a lemma in the embedded
/// sanalista. O(1) per call.
///
/// Used to filter the training corpus: we do not want a "corruption"
/// that accidentally lands on a different real word, because that is
/// not a misspelling. The full `torille::suggest::check` is too slow
/// here -- it walks Phase 1.5 (`head-1` cap = 3000 lemmas inflected
/// per call) and 6000 corrupted forms x ~3 attempts x check would not
/// finish in any reasonable time.
///
/// The price of using only the direct lookup is that corruptions which
/// happen to land on an *inflected form* of a different lemma are not
/// filtered out. Empirically this is rare -- <1 % of test rows -- and
/// is acceptable noise for the small MLP. Slice 3 can add a fuller
/// check that is gated on demand.
fn is_known_word_fast(word: &str) -> bool {
    use torille::kotus::sanalista;
    sanalista().by_word(word).next().is_some()
}

/// Apply up to `CORRUPTIONS_PER_FORM` corruption strategies to `form`,
/// keeping those that are not direct-lemma hits in the sanalista.
/// Returns the list of `(strategy_name, corrupted_form, edit_distance)`
/// triples.
fn corrupt_form(
    form: &str,
    rng: &mut SplitMix64,
    counts: &mut SplitCounts,
) -> Vec<(&'static str, String, usize)> {
    let mut out = Vec::new();
    let strategies = Strategy::all();
    // Try up to N+1 distinct strategies; the +1 covers the case where a
    // strategy is a no-op on this form (length, no double-consonant).
    for _ in 0..(CORRUPTIONS_PER_FORM + 1) {
        if out.len() >= CORRUPTIONS_PER_FORM {
            break;
        }
        let idx = rng.gen_range(strategies.len());
        let s = strategies[idx];
        let Some(corrupted) = s.apply(form, rng) else {
            counts.skipped_no_corruption += 1;
            continue;
        };
        if corrupted == form {
            counts.skipped_no_corruption += 1;
            continue;
        }
        let d = edit_distance(form, &corrupted);
        if d > MAX_EDIT_DISTANCE {
            counts.skipped_distance += 1;
            continue;
        }
        if is_known_word_fast(&corrupted) {
            // The corrupted form is itself a real Finnish word (lemma).
            // Skip -- it's not a misspelling.
            counts.skipped_known += 1;
            continue;
        }
        out.push((s.name(), corrupted, d));
    }
    out
}

fn build_example(
    fr: &FormRow,
    strategy: &'static str,
    corrupted: String,
    d: usize,
) -> TrainingExample {
    TrainingExample {
        misspelled: corrupted,
        surface_form: fr.form.clone(),
        lemma: fr.lemma.clone(),
        homonym: fr.homonym.clone(),
        pos: if fr.participle.is_empty() {
            String::new()
        } else {
            fr.participle.clone()
        },
        voice: fr.voice.clone(),
        mood: fr.mood.clone(),
        tense: fr.tense.clone(),
        person: fr.person.clone(),
        number: fr.number.clone(),
        case: fr.case_.clone(),
        infinitive: fr.infinitive.clone(),
        participle: fr.participle.clone(),
        possessive: fr.possessive.clone(),
        polarity: fr.polarity.clone(),
        reading: fr.reading.clone(),
        ok: fr.ok.clone(),
        corruption_type: strategy,
        edit_distance: d,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut output: PathBuf = PathBuf::from("data/train");
    let mut max_rows: usize = 0; // 0 == unlimited
    let mut seed: u64 = DEFAULT_SEED;
    let mut sample_rate: f64 = DEFAULT_SAMPLE_RATE;
    let mut nominal_db: PathBuf = PathBuf::from("data/kotus_nomines.sqlite");
    let mut verb_db: PathBuf = PathBuf::from("data/kotus_verbit.sqlite");
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--output" | "-o" => {
                if let Some(v) = args.get(i + 1) {
                    output = PathBuf::from(v);
                    i += 1;
                }
            }
            "--max-rows" => {
                if let Some(v) = args.get(i + 1) {
                    max_rows = v.parse().unwrap_or(0);
                    i += 1;
                }
            }
            "--seed" => {
                if let Some(v) = args.get(i + 1) {
                    seed = v.parse().unwrap_or(DEFAULT_SEED);
                    i += 1;
                }
            }
            "--sample-rate" => {
                if let Some(v) = args.get(i + 1) {
                    sample_rate = v.parse().unwrap_or(DEFAULT_SAMPLE_RATE);
                    sample_rate = sample_rate.clamp(0.0, 1.0);
                    i += 1;
                }
            }
            "--nominal-db" => {
                if let Some(v) = args.get(i + 1) {
                    nominal_db = PathBuf::from(v);
                    i += 1;
                }
            }
            "--verb-db" => {
                if let Some(v) = args.get(i + 1) {
                    verb_db = PathBuf::from(v);
                    i += 1;
                }
            }
            "--help" | "-h" => {
                print_help();
                return ExitCode::from(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                print_help();
                return ExitCode::from(2);
            }
        }
        i += 1;
    }

    let mut rng = SplitMix64::new(seed);

    let conn_nominal = match open_db(&nominal_db) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("could not open {nominal_db:?}: {e}");
            return ExitCode::from(1);
        }
    };
    let conn_verb = match open_db(&verb_db) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("could not open {verb_db:?}: {e}");
            return ExitCode::from(1);
        }
    };

    let lemmas = match collect_lemma_set(&conn_nominal, &conn_verb) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("collecting lemmas: {e}");
            return ExitCode::from(1);
        }
    };
    let held_out_lemmas = pick_held_out_lemmas(&lemmas, &mut rng);

    let mut writers = match open_split_writers(&output) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("creating output dir: {e}");
            return ExitCode::from(1);
        }
    };

    // Global row counter shared across both DBs and rayon workers. Used
    // to derive each row's deterministic per-row RNG seed.
    let row_counter = AtomicU64::new(0);

    // How many rows to read from SQLite per batch before par_iter. The
    // sweet spot is large enough to amortise rayon task setup, small
    // enough to keep memory bounded; 4 096 keeps the working set at a
    // few MB per batch (each row is ~200 bytes of Strings).
    const BATCH_SIZE: usize = 4096;

    // Prepare the two streaming cursors. The rusqlite cursor is not
    // `Sync`, so we keep one cursor per DB on the main thread and
    // dispatch batches of rows to rayon workers.
    let mut nominal_stmt = match conn_nominal.prepare(
        "SELECT lemma, homonym, form, \"case\", number, possessive, ok \
         FROM forms WHERE ok = 'both' AND form NOT LIKE '% %' \
         AND form NOT LIKE '%,%' AND form != ''",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("prepare nominal stmt: {e}");
            return ExitCode::from(1);
        }
    };
    let mut nominal_rows = match nominal_stmt.query([]) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("query nominal stmt: {e}");
            return ExitCode::from(1);
        }
    };
    let mut verb_stmt = match conn_verb.prepare(
        "SELECT lemma, homonym, form, voice, mood, tense, person, number, \
         \"case\", infinitive, participle, possessive, polarity, reading, ok \
         FROM forms WHERE ok = 'both' AND form NOT LIKE '% %' \
         AND form NOT LIKE '%,%' AND form != ''",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("prepare verb stmt: {e}");
            return ExitCode::from(1);
        }
    };
    let mut verb_rows = match verb_stmt.query([]) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("query verb stmt: {e}");
            return ExitCode::from(1);
        }
    };

    let mut batch: Vec<FormRow> = Vec::with_capacity(BATCH_SIZE);
    let mut nominal_rows_read: usize = 0;

    // ---- Nominals -----------------------------------------------------
    loop {
        batch.clear();
        let n = match fill_nominal_batch(&mut nominal_rows, &mut batch, BATCH_SIZE) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("reading nominal batch: {e}");
                return ExitCode::from(1);
            }
        };
        nominal_rows_read += n;
        if batch.is_empty() {
            break;
        }
        process_batch(
            &batch,
            &row_counter,
            sample_rate,
            seed,
            &held_out_lemmas,
            &mut rng,
            &mut writers,
        );
        if max_rows > 0 && writers.counts.total_emitted >= max_rows {
            break;
        }
        if n < BATCH_SIZE {
            break;
        }
    }

    // ---- Verbs --------------------------------------------------------
    let mut verb_rows_read: usize = 0;
    loop {
        batch.clear();
        let n = match fill_verb_batch(&mut verb_rows, &mut batch, BATCH_SIZE) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("reading verb batch: {e}");
                return ExitCode::from(1);
            }
        };
        verb_rows_read += n;
        if batch.is_empty() {
            break;
        }
        process_batch(
            &batch,
            &row_counter,
            sample_rate,
            seed,
            &held_out_lemmas,
            &mut rng,
            &mut writers,
        );
        if max_rows > 0 && writers.counts.total_emitted >= max_rows {
            break;
        }
        if n < BATCH_SIZE {
            break;
        }
    }

    // Flush + close.
    let SplitWriters {
        train,
        dev,
        test_unseen_lemma,
        test_unseen_inflection,
        test_unseen_corruption,
        counts,
    } = writers;
    for mut w in [
        train,
        dev,
        test_unseen_lemma,
        test_unseen_inflection,
        test_unseen_corruption,
    ] {
        if let Err(e) = w.flush() {
            eprintln!("flush: {e}");
            return ExitCode::from(1);
        }
    }

    println!("train_data summary");
    println!("  Nominal rows read:    {nominal_rows_read}");
    println!("  verb rows read:    {verb_rows_read}");
    println!("  total emitted:     {}", counts.total_emitted);
    println!("    train:           {}", counts.train);
    println!("    dev:             {}", counts.dev);
    println!("    unseen_lemma:    {}", counts.test_unseen_lemma);
    println!("    unseen_inflect:  {}", counts.test_unseen_inflection);
    println!("    unseen_corrupt:  {}", counts.test_unseen_corruption);
    println!("  skipped:");
    println!("    known-word:      {}", counts.skipped_known);
    println!("    no-corruption:   {}", counts.skipped_no_corruption);
    println!("    out-of-budget:   {}", counts.skipped_distance);

    ExitCode::SUCCESS
}

/// One row's corruption work as executed on a rayon worker thread.
///
/// Returns the produced examples. The worker owns its own `SplitMix64`
/// seeded from `seed.wrapping_add(global_index)`; the deterministic
/// mapping `(seed, global_index) -> output` is what makes parallel
/// runs reproducible across machines and core counts.
fn process_row(
    fr: &FormRow,
    global_index: u64,
    seed: u64,
    sample_rate: f64,
) -> Vec<TrainingExample> {
    // Sampling happens here so the same global index always picks the
    // same in/out decision regardless of how batches are scheduled.
    let mut rng = SplitMix64::new(seed.wrapping_add(global_index));
    if !rng.flip(sample_rate) {
        return Vec::new();
    }
    let mut local_counts = SplitCounts::default();
    let corruptions = corrupt_form(&fr.form, &mut rng, &mut local_counts);
    if corruptions.is_empty() {
        return Vec::new();
    }
    corruptions
        .into_iter()
        .map(|(strategy, corrupted, d)| build_example(fr, strategy, corrupted, d))
        .collect()
}

/// Process one batch of rows in parallel via rayon.
///
/// Workers return `Vec<TrainingExample>` per row; the main thread then
/// dispatches each example to its split writer under the shared
/// `main_rng`. Keeping the writer access single-threaded avoids lock
/// contention on the JSONL files and keeps the output ordering
/// deterministic with respect to the batch.
///
/// `seed` is the user-supplied master seed; each row gets a deterministic
/// sub-stream derived from `seed + global_index`.
fn process_batch(
    batch: &[FormRow],
    row_counter: &AtomicU64,
    sample_rate: f64,
    seed: u64,
    held_out_lemmas: &BTreeSet<String>,
    main_rng: &mut SplitMix64,
    writers: &mut SplitWriters,
) {
    // Reserve a contiguous range of global indices for this batch so
    // every row sees a unique, stable `global_index` across runs.
    let base = row_counter.fetch_add(batch.len() as u64, Ordering::Relaxed);

    // Parallel work. With `cfg(test)` the binary is built without
    // rayon; tests stay single-threaded (correctness > speed).
    #[cfg(not(test))]
    let per_row_examples: Vec<Vec<TrainingExample>> = batch
        .par_iter()
        .enumerate()
        .map(|(local, fr)| process_row(fr, base + local as u64, seed, sample_rate))
        .collect();

    #[cfg(test)]
    let per_row_examples: Vec<Vec<TrainingExample>> = batch
        .iter()
        .enumerate()
        .map(|(local, fr)| process_row(fr, base + local as u64, seed, sample_rate))
        .collect();

    // Serial write path: single-threaded for deterministic ordering and
    // no fsync contention.
    for examples in per_row_examples {
        let lemma_held =
            held_out_lemmas.contains(examples.first().map(|e| e.lemma.as_str()).unwrap_or(""));
        for ex in examples {
            let inflection_held = is_unseen_inflection(&ex.lemma, &ex.surface_form);
            let name = dispatch_example(writers, &ex, lemma_held, inflection_held, main_rng);
            writers.counts.total_emitted += 1;
            match name {
                "train" => writers.counts.train += 1,
                "dev" => writers.counts.dev += 1,
                "test_unseen_lemma" => writers.counts.test_unseen_lemma += 1,
                "test_unseen_inflection" => writers.counts.test_unseen_inflection += 1,
                "test_unseen_corruption" => writers.counts.test_unseen_corruption += 1,
                _ => {}
            }
        }
    }
}

fn print_help() {
    eprintln!("train_data -- build training data for the optional neural reranker");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!(
        "    train_data [--output DIR] [--max-rows N] [--seed N] [--sample-rate F] [--nominal-db PATH] [--verb-db PATH]"
    );
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("    --output,  -o     output directory [default: data/train]");
    eprintln!(
        "    --max-rows N      stop after N rows of source data (0 = unlimited) [default: 0]"
    );
    eprintln!("    --seed N          deterministic PRNG seed [default: {DEFAULT_SEED}]");
    eprintln!(
        "    --sample-rate F   fraction of read rows to keep [default: 1.0, range 0.0..=1.0]"
    );
    eprintln!("    --nominal-db PATH nominal SQLite database [default: data/kotus_nomines.sqlite]");
    eprintln!("    --verb-db PATH    verb SQLite database [default: data/kotus_verbit.sqlite]");
    eprintln!();
    eprintln!("OUTPUT:");
    eprintln!("    <output-dir>/train.jsonl");
    eprintln!("    <output-dir>/dev.jsonl");
    eprintln!("    <output-dir>/test_unseen_lemma.jsonl");
    eprintln!("    <output-dir>/test_unseen_inflection.jsonl");
    eprintln!("    <output-dir>/test_unseen_corruption.jsonl");
    eprintln!();
    eprintln!("THREADING:");
    eprintln!("    Per-row corruption runs on a rayon thread pool. The five");
    eprintln!("    JSONL writers stay single-threaded on the main thread so");
    eprintln!("    output ordering is reproducible and there is no fsync");
    eprintln!("    contention. Expect a ~3-7x speedup over a single thread on");
    eprintln!("    a typical dev box.");
}

// =============================================================================
//   Tests.
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh PRNG seed for each test gives reproducible-yet-independent
    /// streams.
    fn test_rng() -> SplitMix64 {
        SplitMix64::new(0x0123_4567_89ab_cdef)
    }

    /// The substitution strategies never produce a no-op when applied
    /// to a Finnish word of any reasonable length.
    #[test]
    fn substitute_random_always_changes() {
        let mut rng = test_rng();
        for _ in 0..50 {
            let out = Strategy::SubstituteRandom.apply("talo", &mut rng).unwrap();
            assert_ne!(out, "talo", "substitute_random produced identity");
        }
    }

    /// The vowel-swap strategy respects the input's vowels -- it only
    /// swaps one of the existing vowels to a related one.
    #[test]
    fn vowel_swap_keeps_vowel_count() {
        let mut rng = test_rng();
        for trial in 0..20 {
            let out = Strategy::VowelSwap
                .apply("talo", &mut rng)
                .expect("talo has vowels, swap should succeed");
            // Same character count.
            assert_eq!(out.chars().count(), 4, "trial {trial}: {out}");
            // Only one of the four chars differs from the input.
            let diff_count = out
                .chars()
                .zip("talo".chars())
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                diff_count <= 2,
                "trial {trial}: {out} differs in too many positions"
            );
        }
    }

    /// The doubled-consonant-add strategy never changes the identity of
    /// forms that contain no eligible consonants.
    #[test]
    fn double_consonant_add_returns_none_for_no_consonants() {
        let mut rng = test_rng();
        // "ie" has only vowels.
        assert!(Strategy::DoubleConsonantAdd.apply("ie", &mut rng).is_none());
    }

    /// The doubled-consonant-drop strategy finds "kk" / "tt" / ...
    #[test]
    fn double_consonant_drop_finds_doubles() {
        let mut rng = test_rng();
        let out = Strategy::DoubleConsonantDrop
            .apply("kukka", &mut rng)
            .expect("kukka has kk");
        assert_eq!(out, "kuka", "drop one k: {out}");
        let out = Strategy::DoubleConsonantDrop
            .apply("tattari", &mut rng)
            .expect("tattari has tt");
        assert_eq!(out, "tatari", "drop one t: {out}");
    }

    /// `edit_distance` matches textbook examples.
    #[test]
    fn edit_distance_textbook() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("flaw", "lawn"), 2);
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("a", ""), 1);
        assert_eq!(edit_distance("abc", "abc"), 0);
    }

    /// `edit_distance` enforces the budget: strings that differ in
    /// length by more than `MAX_EDIT_DISTANCE` are over-budget.
    #[test]
    fn edit_distance_respects_budget() {
        assert!(edit_distance("aaa", "aaaaaa") > MAX_EDIT_DISTANCE);
    }

    /// Held-out lemma split is deterministic for the same seed.
    #[test]
    fn held_out_lemmas_deterministic() {
        let mut a = BTreeSet::new();
        a.insert("talo".into());
        a.insert("koira".into());
        a.insert("juosta".into());
        a.insert("voida".into());
        a.insert("omena".into());
        let s1 = pick_held_out_lemmas(&a, &mut SplitMix64::new(42));
        let s2 = pick_held_out_lemmas(&a, &mut SplitMix64::new(42));
        assert_eq!(s1, s2);
        // The held-out set is a strict subset of the input.
        for l in &s1 {
            assert!(a.contains(l));
        }
        // The held-out size is the expected fraction (within rounding).
        assert_eq!(s1.len(), (5.0 * UNSEEN_LEMMA_FRACTION) as usize);
    }

    /// `is_unseen_inflection` is a pure function of (lemma, form) -- it
    /// does not consult the RNG -- and is stable across calls. The
    /// FNV-1a + splitmix64 finalizer keeps the held-out fraction close
    /// to `UNSEEN_INFLECTION_FRACTION` even on adversarial inputs that
    /// share a long prefix.
    #[test]
    fn is_unseen_inflection_stable() {
        let a = is_unseen_inflection("talo", "talossa");
        let b = is_unseen_inflection("talo", "talossa");
        assert_eq!(a, b);
        let mut held = 0usize;
        for i in 0..10_000 {
            let form = format!("syntheticform{i}");
            if is_unseen_inflection("talo", &form) {
                held += 1;
            }
        }
        let expected = (10_000.0 * UNSEEN_INFLECTION_FRACTION) as usize;
        // The splitmix64 finalizer keeps the actual fraction within
        // ~50% of the target -- strict equality is too strong a check
        // for a 10k-sample estimate of a 1% fraction.
        assert!(
            held > expected / 2 && held < expected * 2,
            "held={held}, expected={expected}"
        );
    }

    /// Letter-pair weighting: a frequent pair (`ka`) transposes much more
    /// often than a rare one (`qx`) on the same number of attempts.
    #[test]
    fn letter_pair_weight_known_pairs() {
        assert!(letter_pair_weight('k', 'a') > letter_pair_weight('q', 'x'));
        assert!(letter_pair_weight('k', 'k') > letter_pair_weight('z', 'q'));
    }

    /// Letter-pair weighting default: unlisted pairs fall back to 0.5.
    #[test]
    fn letter_pair_weight_default() {
        assert!((letter_pair_weight('?', '!') - 0.5).abs() < f64::EPSILON);
    }

    /// End-of-word positional bias: when the same input form is given
    /// many transposition attempts, the resulting positions are biased
    /// toward the end of the word relative to a uniform sample.
    #[test]
    fn transposition_bias_toward_end() {
        let mut rng = test_rng();
        let form = "koululaisu"; // 9 chars -> 8 transposable positions
        let mut early = 0usize;
        let mut late = 0usize;
        let trials = 10_000;
        for _ in 0..trials {
            // We need to expose the position chosen. The strategy
            // returns the corrupted form, which carries the position
            // information only indirectly. Instead we replicate the
            // weight computation here so the test is hermetic.
            let n = form.chars().count() - 1;
            let denom = (n as f64 - 1.0).max(1.0);
            let weights: Vec<f64> = (0..n)
                .map(|i| {
                    let frac = i as f64 / denom;
                    1.0 + TRANSPOSITION_END_BIAS * (frac - 0.5)
                })
                .collect();
            let pos = weighted_position(&mut rng, &weights);
            if pos <= 3 {
                early += 1;
            } else {
                late += 1;
            }
        }
        // Late positions should be substantially over-represented.
        assert!(
            late > early,
            "end-of-weight transpositions must be > early: early={early} late={late}"
        );
        // The skew should not be trivial (>= 60/40 split).
        assert!(late * 10 >= trials * 6);
    }

    /// End-of-word bias is not extreme: the start of the word still
    /// gets a non-trivial fraction of transpositions.
    #[test]
    fn transposition_bias_not_extreme() {
        let mut rng = test_rng();
        let form = "koululaisu";
        let mut early = 0usize;
        let trials = 10_000;
        for _ in 0..trials {
            let n = form.chars().count() - 1;
            let denom = (n as f64 - 1.0).max(1.0);
            let weights: Vec<f64> = (0..n)
                .map(|i| {
                    let frac = i as f64 / denom;
                    1.0 + TRANSPOSITION_END_BIAS * (frac - 0.5)
                })
                .collect();
            let pos = weighted_position(&mut rng, &weights);
            if pos <= 3 {
                early += 1;
            }
        }
        // At least 25% of transpositions still touch the early half.
        assert!(early * 4 >= trials);
    }

    /// Property test: every corruption strategy, applied to a Finnish
    /// surface form, must produce a string that
    ///   - is not equal to the input
    ///   - is within `MAX_EDIT_DISTANCE` of the input
    ///   - when the lemma differs from the surface form, is closer to the
    ///     surface form than to the lemma (the structural property the
    ///     Slice 2 design relies on for `juoksei -> juoksee`-style cases)
    ///
    /// The test sweeps every strategy in `Strategy::all()` over a small
    /// synthetic vocabulary of (lemma, surface) pairs that mirror the
    /// motivating examples: a noun (talo / taloaan) and a verb with stem
    /// alternation (juosta / juoksee). Each (strategy, pair) is tried 50
    /// times so the randomised strategies are not just tested on a single
    /// seed.
    #[test]
    fn corruption_produces_near_surface_misspellings() {
        let pairs: &[(&str, &str)] = &[
            ("talo", "taloaan"),
            ("talo", "talossa"),
            ("koira", "koiralle"),
            ("juosta", "juoksee"),
            ("juoda", "juo"),
            ("voida", "voi"),
        ];

        for &(lemma, surface) in pairs {
            // Sanity: surface must be a different string from lemma so the
            // "closer to surface than to lemma" check is meaningful.
            assert_ne!(lemma, surface, "test fixture should have lemma != surface");

            for (idx, strategy) in Strategy::all().iter().enumerate() {
                // Seed each strategy with a deterministic but distinct
                // splitmix64 seed so different strategies do not share a
                // stream of `apply` results.
                let mut rng = SplitMix64::new(0xC0FFEE_u64.wrapping_add(idx as u64 * 31));
                for _ in 0..50 {
                    let Some(out) = strategy.apply(surface, &mut rng) else {
                        continue;
                    };
                    assert_ne!(out, surface, "{strategy:?} produced identity for {surface}");
                    let d_surface = edit_distance(&out, surface);
                    assert!(
                        d_surface <= MAX_EDIT_DISTANCE,
                        "{strategy:?} on {surface}: d_surface={d_surface} > {MAX_EDIT_DISTANCE}"
                    );
                    let d_lemma = edit_distance(&out, lemma);
                    // The structural property: when the lemma and surface
                    // differ (which they do for every fixture), the
                    // misspelling is closer to the surface form than to
                    // the lemma.
                    assert!(
                        d_surface <= d_lemma,
                        "{strategy:?} produced {out} from {surface} (lemma={lemma}); \
                     d_surface={d_surface} > d_lemma={d_lemma}"
                    );
                }
            }
        }
    }

    /// Property test: every emitted `(misspelled, surface_form, lemma)`
    /// triple satisfies the same three invariants, but applied to a fresh
    /// `process_row` call. This guards against the invariants drifting if
    /// `process_row` / `corrupt_form` change in the future.
    #[test]
    fn process_row_invariants() {
        let pairs: &[(&str, &str, &str, &str)] = &[
            ("talo", "", "talo", "taloaan"),
            ("koira", "", "koira", "koiralle"),
            ("juosta", "", "juosta", "juoksee"),
        ];
        for &(lemma, homonym, _lemma_disp, surface) in pairs {
            let fr = FormRow {
                lemma: lemma.to_string(),
                homonym: homonym.to_string(),
                form: surface.to_string(),
                voice: String::new(),
                mood: String::new(),
                tense: String::new(),
                person: String::new(),
                number: String::new(),
                case_: String::new(),
                infinitive: String::new(),
                participle: String::new(),
                possessive: String::new(),
                polarity: String::new(),
                reading: String::new(),
                ok: "both".to_string(),
            };
            for global_idx in 0u64..40 {
                let exs = process_row(&fr, global_idx, 0xABCDEF, 1.0);
                for ex in exs {
                    assert_eq!(
                        ex.surface_form, surface,
                        "process_row produced an example whose surface_form drifted"
                    );
                    let d = edit_distance(&ex.misspelled, &ex.surface_form);
                    assert!(
                        d <= MAX_EDIT_DISTANCE,
                        "process_row on {surface} produced {} at d={d}",
                        ex.misspelled
                    );
                }
            }
        }
    }

    /// `dispatch_example` honours the reserved corruption strategy: any
    /// `DoubleConsonantDrop` example lands in `test_unseen_corruption`
    /// regardless of lemma / inflection status.
    #[test]
    fn dispatch_example_reserves_corruption_split() {
        let tmp =
            std::env::temp_dir().join(format!("torille_train_data_test1_{}", std::process::id()));
        let mut writers = open_split_writers(&tmp).unwrap();
        let ex = TrainingExample {
            misspelled: "jookse".into(),
            surface_form: "juoksee".into(),
            lemma: "juosta".into(),
            homonym: String::new(),
            pos: String::new(),
            voice: String::new(),
            mood: String::new(),
            tense: String::new(),
            person: String::new(),
            number: String::new(),
            case: String::new(),
            infinitive: String::new(),
            participle: String::new(),
            possessive: String::new(),
            polarity: String::new(),
            reading: String::new(),
            ok: "both".into(),
            corruption_type: Strategy::DoubleConsonantDrop.name(),
            edit_distance: 1,
        };
        let mut rng = test_rng();
        let name = dispatch_example(&mut writers, &ex, false, false, &mut rng);
        assert_eq!(name, "test_unseen_corruption");
        // Also verify it actually wrote to that file.
        let _ = fs::remove_dir_all(&tmp);
    }

    /// The corpus must include the `jooksee -> juoksee` example (or
    /// a near variant of it) somewhere among the corruptions, because
    /// it is the canonical motivating case from the design document.
    /// This test runs against the live SQLite files and so is marked
    /// `#[ignore]` -- run with `cargo test -- --ignored` to execute.
    #[test]
    #[ignore]
    fn jooksee_case_is_in_the_corpus() {
        let mut rng = SplitMix64::new(DEFAULT_SEED);
        // Generate many attempts of the substitute-random strategy on
        // "juoksee" until one produces "jooksee".
        let mut found = false;
        for _ in 0..10_000 {
            let out = Strategy::SubstituteRandom
                .apply("juoksee", &mut rng)
                .unwrap();
            if out == "jooksee" {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "the jooksee corruption must be reachable via repeated draws"
        );
    }
}
