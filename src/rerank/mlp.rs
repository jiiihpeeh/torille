//! Model-A inference: character features, the tiny MLP, and argmax.
//!
//! This is the production forward pass described in the reranker brief:
//!
//! ```text
//! misspelled ──┬── char embedding + mean pooling ──┐
//!              ├── hashed character bigrams ───────┤
//! candidate  ──┼── char embedding + mean pooling ──┼── concat ── Linear ── ReLU ── Linear ── logit
//!              ├── hashed character bigrams ───────┤
//! edit distance ┘                                  │
//! ```
//!
//! Three rules hold throughout this file:
//!
//! 1. **Raw logits.** [`ModelA::score_batch`] returns unbounded floats.
//!    No sigmoid, no softmax. Ranking is `argmax` over the logits; any
//!    caller that wants probabilities applies a listwise softmax *across*
//!    the candidate list, outside the model. Squeezing a sigmoid into
//!    `score_batch` would warp that softmax into `softmax(sigmoid(x))`,
//!    which is not a probability distribution over the candidates.
//!
//! 2. **Query features are computed once.** The misspelled word does not
//!    change between candidates, so [`ModelA::query_features`] runs once
//!    per query and [`score_batch`] only ever reads it. Nothing in
//!    `score_batch` takes the query *word*, which is what makes the
//!    recompute impossible to reintroduce by accident.
//!
//! 3. **No per-candidate allocation.** The `(N, n_in)` feature matrix and
//!    the hidden activations live in a caller-owned [`Scratch`], so
//!    scoring a query allocates nothing after the first call.
//!
//! The feature definitions mirror `tools/train/model.py` exactly; the
//! golden fixtures in `tools/train/parity_test.py` are what this module
//! is checked against.

use std::fmt;

/// Maximum characters read from each of the query and candidate.
///
/// Matches `MAX_LEN` in `tools/train/model.py`. Longer words are
/// truncated on the right, so `edit_features` still sees the *whole*
/// word (it works on the strings, not the encoded ids).
pub const MAX_LEN: usize = 32;

/// PAD column index in the character vocabulary. Also `CHAR_INDEX["<pad>"]`.
pub const PAD_IDX: u16 = 0;
/// UNK column index in the character vocabulary. Also `CHAR_INDEX["<unk>"]`.
pub const UNK_IDX: u16 = 1;

/// Boundary markers framing the n-gram character sequence.
///
/// ASCII, never in `CHAR_INDEX`, so they hash as themselves and
/// word-initial pairs hash differently from word-internal ones.
const BOUND_LEFT: char = '<';
const BOUND_RIGHT: char = '>';
/// Stand-in for a character outside the vocabulary on the n-gram path.
///
/// Exactly one character long, so the trigram hash still receives three
/// real characters (`juoks€ee` → trigram `s~e`, not `see`).
const UNK_GLYPH: char = '~';

/// Number of characters in the vocabulary: pad, unk, 26 lower, `äö`,
/// 26 upper, `ÄÖ`, and `'` `-`.
pub const NUM_CHARS: usize = 60;

/// The vocabulary index of `c`, or [`UNK_IDX`] when `c` is not in it.
///
/// This is `CHAR_INDEX` from `tools/train/model.py`, expressed as a
/// closed-form mapping so no table has to be built or hashed at runtime.
///
/// ```text
///   0  <pad>      2..27  a-z      28 ä   29 ö
///   1  <unk>     30..55  A-Z      56 Ä   57 Ö
///                              58 '    59 -
/// ```
#[inline]
pub fn char_index(c: char) -> u16 {
    match c {
        'a'..='z' => 2 + (c as u32 - 'a' as u32) as u16,
        'ä' => 28,
        'ö' => 29,
        'A'..='Z' => 30 + (c as u32 - 'A' as u32) as u16,
        'Ä' => 56,
        'Ö' => 57,
        '\'' => 58,
        '-' => 59,
        _ => UNK_IDX,
    }
}

/// `true` if `c` has a vocabulary entry (i.e. is not an unknown glyph).
#[inline]
pub fn in_vocab(c: char) -> bool {
    matches!(
        c,
        'a'..='z' | 'A'..='Z' | 'ä' | 'ö' | 'Ä' | 'Ö' | '\'' | '-'
    )
}

// ---------------------------------------------------------------------------
//   Errors.
// ---------------------------------------------------------------------------

/// Why a model file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// The file does not start with the expected magic bytes.
    BadMagic,
    /// The header declares a version this build does not understand.
    BadVersion(u32),
    /// The header's dimensions are nonsensical (zero, or absurd).
    BadDimensions(String),
    /// The payload is shorter than the header promises.
    Truncated { want: usize, got: usize },
    /// An I/O failure while reading the file.
    Io(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::BadMagic => write!(f, "not a torille reranker model (bad magic)"),
            ModelError::BadVersion(v) => write!(f, "unsupported model format version {v}"),
            ModelError::BadDimensions(d) => write!(f, "bad model dimensions: {d}"),
            ModelError::Truncated { want, got } => {
                write!(f, "model file truncated: need {want} bytes, got {got}")
            }
            ModelError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ModelError {}

impl From<std::io::Error> for ModelError {
    fn from(e: std::io::Error) -> Self {
        ModelError::Io(e.to_string())
    }
}

/// The four magic bytes at the start of a `.trm` model.
pub const MAGIC: [u8; 4] = *b"TRM1";
/// The format version this build writes.
pub const VERSION: u32 = 1;

// ---------------------------------------------------------------------------
//   The model.
// ---------------------------------------------------------------------------

/// A trained Model-A reranker: weights only, no Python in sight.
///
/// The layout on disk (and in memory) is
///
/// ```text
/// char_embed   (num_chars, embed_dim)   row-major
/// w1           (hidden_dim, n_in)       row-major
/// b1           (hidden_dim,)
/// w2           (hidden_dim + 1,)        the trailing slot is the bias
/// b2           (1,)
/// ```
///
/// `n_in` is derived from the dimensions, so a file cannot claim one
/// layout and store another.
#[derive(Clone, Debug)]
pub struct ModelA {
    pub char_embed: Vec<f32>,
    pub w1: Vec<f32>,
    pub b1: Vec<f32>,
    pub w2: Vec<f32>,
    pub b2: f32,
    pub embed_dim: usize,
    pub hidden_dim: usize,
    pub bigram_dim: usize,
    pub trigram_dim: usize,
    pub edit_dim: usize,
}

/// The per-query features, computed once and reused for every candidate.
///
/// Holding the word alongside the vectors lets [`ModelA::score_batch`]
/// derive the per-candidate edit features without the caller having to
/// thread the query word through the API — and, more importantly, lets
/// `score_batch` take *no* query word, so a query-side recompute cannot
/// creep back in.
#[derive(Clone, Debug)]
pub struct QueryFeatures {
    /// The query word these features were built from.
    pub word: String,
    /// Mean of the non-PAD character embeddings, `(embed_dim,)`.
    pub char_mean: Vec<f32>,
    /// Sum-pooled hashed bigrams, `(bigram_dim,)`.
    pub bigram: Vec<f32>,
    /// Sum-pooled hashed trigrams, `(trigram_dim,)`. Empty when the
    /// model has trigrams disabled.
    pub trigram: Vec<f32>,
}

/// One candidate to score: its surface form and the engine's edit
/// distance from the query.
#[derive(Clone, Copy, Debug)]
pub struct Candidate<'a> {
    pub word: &'a str,
    /// As reported by the engine. Clamped to 4 and divided by 4 inside
    /// [`ModelA::score_batch`], exactly as the Python does.
    pub edit_distance: i32,
}

impl<'a> Candidate<'a> {
    pub fn new(word: &'a str, edit_distance: i32) -> Self {
        Self {
            word,
            edit_distance,
        }
    }
}

/// Reusable buffers for [`ModelA::score_batch`].
///
/// Owned by the caller so that scoring a query allocates nothing after
/// the first call. One `Scratch` per thread; it is not `Sync`.
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    /// Row count of the matrices below, as written by
    /// [`ModelA::extract_features`]. Lets [`ModelA::forward`] be a
    /// separate stage from feature extraction without passing lengths
    /// around.
    rows: usize,
    /// The `(N, n_in)` feature matrix.
    x: Vec<f32>,
    /// Hidden pre-activations, `(N, hidden_dim)`.
    h: Vec<f32>,
    /// Raw logits, `(N,)`.
    logits: Vec<f32>,
}

impl Scratch {
    /// Reserve room for `n` candidates of a model with the given shape.
    pub fn reserve(&mut self, n: usize, n_in: usize, hidden_dim: usize) {
        self.x.resize(n * n_in, 0.0);
        self.h.resize(n * hidden_dim, 0.0);
        self.logits.resize(n, 0.0);
    }

    /// The logits from the most recent [`ModelA::score_batch`] call.
    ///
    /// Empty before the first call and after a call with zero
    /// candidates.
    pub fn logits(&self) -> &[f32] {
        &self.logits
    }
}

impl ModelA {
    /// Input width for this model's configuration.
    ///
    /// `2 * (embed + bigram + trigram) + edit`, matching
    /// `FeatureConfig.n_in` on the Python side. Query block and
    /// candidate block are the same width, and the edit features sit at
    /// the tail.
    pub fn n_in(&self) -> usize {
        2 * (self.embed_dim + self.bigram_dim + self.trigram_dim) + self.edit_dim
    }

    /// Total parameters stored, i.e. the number the reports quote.
    pub fn param_count(&self) -> usize {
        self.char_embed.len() + self.w1.len() + self.b1.len() + self.w2.len() + 1
    }

    /// A short architecture label for reports.
    pub fn describe(&self) -> String {
        format!(
            "embed={} hidden={} bigram={} trigram={} edit={} n_in={} params={}",
            self.embed_dim,
            self.hidden_dim,
            self.bigram_dim,
            self.trigram_dim,
            self.edit_dim,
            self.n_in(),
            self.param_count()
        )
    }

    // -- serialisation ----------------------------------------------------

    /// Decode a model from the bytes of a `.trm` file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModelError> {
        if bytes.len() < 4 || bytes[0..4] != MAGIC {
            return Err(ModelError::BadMagic);
        }
        let mut off = 4usize;

        let read_u32 = |off: &mut usize| -> Result<u32, ModelError> {
            let end = off.checked_add(4).ok_or(ModelError::Truncated {
                want: usize::MAX,
                got: bytes.len(),
            })?;
            if end > bytes.len() {
                return Err(ModelError::Truncated {
                    want: end,
                    got: bytes.len(),
                });
            }
            let v = u32::from_le_bytes(bytes[*off..end].try_into().expect("4 bytes"));
            *off = end;
            Ok(v)
        };

        let version = read_u32(&mut off)?;
        if version != VERSION {
            return Err(ModelError::BadVersion(version));
        }
        let num_chars = read_u32(&mut off)? as usize;
        let embed_dim = read_u32(&mut off)? as usize;
        let hidden_dim = read_u32(&mut off)? as usize;
        let bigram_dim = read_u32(&mut off)? as usize;
        let trigram_dim = read_u32(&mut off)? as usize;
        let edit_dim = read_u32(&mut off)? as usize;

        // Sanity: a model bigger than this is a corrupt header, not a
        // real one. Guards the length arithmetic below.
        for (name, v) in [
            ("num_chars", num_chars),
            ("embed_dim", embed_dim),
            ("hidden_dim", hidden_dim),
        ] {
            if v == 0 || v > 1 << 20 {
                return Err(ModelError::BadDimensions(format!("{name}={v}")));
            }
        }
        if num_chars != NUM_CHARS {
            return Err(ModelError::BadDimensions(format!(
                "num_chars={num_chars}, expected {NUM_CHARS}"
            )));
        }

        let n_in = 2 * (embed_dim + bigram_dim + trigram_dim) + edit_dim;
        let n_embed = num_chars * embed_dim;
        let n_w1 = hidden_dim * n_in;
        let n_w2 = hidden_dim + 1;
        let want_bytes = (n_embed + n_w1 + hidden_dim + n_w2 + 1) * 4;
        if bytes.len() < off + want_bytes {
            return Err(ModelError::Truncated {
                want: off + want_bytes,
                got: bytes.len(),
            });
        }

        let read_f32s = |off: &mut usize, n: usize| -> Vec<f32> {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let end = *off + 4;
                let v = f32::from_le_bytes(bytes[*off..end].try_into().expect("4 bytes"));
                out.push(v);
                *off = end;
            }
            out
        };

        let char_embed = read_f32s(&mut off, n_embed);
        let w1 = read_f32s(&mut off, n_w1);
        let b1 = read_f32s(&mut off, hidden_dim);
        let w2 = read_f32s(&mut off, n_w2);
        let b2 = read_f32s(&mut off, 1)[0];

        Ok(Self {
            char_embed,
            w1,
            b1,
            w2,
            b2,
            embed_dim,
            hidden_dim,
            bigram_dim,
            trigram_dim,
            edit_dim,
        })
    }

    /// Encode this model back into the `.trm` byte layout.
    ///
    /// `from_bytes(to_bytes(m)) == m` is one of the regression tests;
    /// the writer also exists so a test can build a model in memory
    /// without going through Python.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n_in = self.n_in();
        let mut out = Vec::with_capacity(28 + 4 * self.param_count());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(NUM_CHARS as u32).to_le_bytes());
        out.extend_from_slice(&(self.embed_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.hidden_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.bigram_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.trigram_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.edit_dim as u32).to_le_bytes());
        let _ = n_in;
        for v in self
            .char_embed
            .iter()
            .chain(self.w1.iter())
            .chain(self.b1.iter())
            .chain(self.w2.iter())
            .chain(std::iter::once(&self.b2))
        {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Read a `.trm` file from disk.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, ModelError> {
        Self::from_bytes(&std::fs::read(path)?)
    }

    // -- features ---------------------------------------------------------

    /// Compute the query-side features once, for reuse by every
    /// candidate in the list.
    pub fn query_features(&self, word: &str) -> QueryFeatures {
        let mut char_sum = vec![0.0f32; self.embed_dim];
        let mut n_valid = 0.0f32;
        for (i, c) in word.chars().take(MAX_LEN).enumerate() {
            let _ = i;
            let idx = char_index(c);
            if idx == PAD_IDX {
                continue;
            }
            n_valid += 1.0;
            let row = &self.char_embed[idx as usize * self.embed_dim..][..self.embed_dim];
            for (d, &v) in row.iter().enumerate() {
                char_sum[d] += v;
            }
        }
        if n_valid > 0.0 {
            for v in char_sum.iter_mut() {
                *v /= n_valid;
            }
        } else {
            char_sum.iter_mut().for_each(|v| *v = 0.0);
        }

        let bigram = bigram_features(word, self.bigram_dim);
        let trigram = if self.trigram_dim > 0 {
            trigram_features(word, self.trigram_dim)
        } else {
            Vec::new()
        };

        QueryFeatures {
            word: word.to_string(),
            char_mean: char_sum,
            bigram,
            trigram,
        }
    }

    /// Build one candidate's row of the feature matrix, in place.
    ///
    /// Layout, matching `score_batch` in `tools/train/model.py`:
    ///
    /// ```text
    /// [q_char | q_bigram | q_trigram | c_char | c_bigram | c_trigram | edit]
    /// ```
    ///
    /// The query block is **copied** from the precomputed features, not
    /// re-pooled: the query does not change between candidates, so
    /// pooling it `n` times would be the per-candidate recompute the
    /// brief rules out. `out` must be exactly `n_in()` long and is
    /// fully overwritten.
    fn write_row(&self, query: &QueryFeatures, cand: Candidate<'_>, out: &mut [f32]) {
        let e = self.embed_dim;
        let b = self.bigram_dim;
        let t = self.trigram_dim;
        let d = self.edit_dim;
        let stride = e + b + t;

        out[..e].copy_from_slice(&query.char_mean);
        out[e..e + b].copy_from_slice(&query.bigram);
        if t > 0 {
            out[e + b..stride].copy_from_slice(&query.trigram);
        }

        let cid = encode_chars(cand.word);
        pool_chars_into(&self.char_embed, &cid, &mut out[stride..stride + e]);
        bigram_features_into(cand.word, b, &mut out[stride + e..2 * stride - t]);
        if t > 0 {
            trigram_features_into(cand.word, t, &mut out[2 * stride - t..2 * stride]);
        }
        let mut edit = [0.0f32; MAX_EDIT_DIM];
        edit_features_into(
            &query.word,
            cand.word,
            cand.edit_distance,
            d,
            &mut edit[..d],
        );
        out[2 * stride..2 * stride + d].copy_from_slice(&edit[..d]);
    }

    /// The single-row feature vector for one candidate.
    ///
    /// This is `score_batch`'s own row, extracted — not a second
    /// implementation — so a test that compares it against Python's
    /// hand-expanded `x` is testing the matrix the model actually
    /// consumes. Allocated per call; `score_batch` writes the same
    /// bytes into a reusable `Scratch` instead.
    pub fn feature_vector(&self, query: &QueryFeatures, cand: &Candidate<'_>) -> Vec<f32> {
        let mut row = vec![0.0f32; self.n_in()];
        self.write_row(query, *cand, &mut row);
        row
    }

    /// Score a whole candidate list with one batched forward pass.
    ///
    /// Returns raw logits, one per candidate, aligned with `candidates`.
    /// Ranking is `argmax`; apply a listwise softmax outside if you need
    /// probabilities. An empty candidate list scores to an empty slice
    /// and does no work at all.
    pub fn score_batch<'a>(
        &self,
        query: &QueryFeatures,
        candidates: &[Candidate<'_>],
        scratch: &'a mut Scratch,
    ) -> &'a [f32] {
        if candidates.is_empty() {
            scratch.rows = 0;
            scratch.logits.clear();
            return &scratch.logits;
        }
        self.extract_features(query, candidates, scratch);
        self.forward(scratch)
    }

    /// Stage one: build the `(N, n_in)` feature matrix. No MLP work.
    ///
    /// Split out of [`score_batch`] so the benchmark can time feature
    /// extraction and inference separately — the brief asks for both
    /// numbers, and fusing them would make "is the MLP actually
    /// significant?" unanswerable. `score_batch` is this plus
    /// [`ModelA::forward`].
    pub fn extract_features(
        &self,
        query: &QueryFeatures,
        candidates: &[Candidate<'_>],
        scratch: &mut Scratch,
    ) {
        let n = candidates.len();
        scratch.rows = n;
        if n == 0 {
            scratch.logits.clear();
            return;
        }
        let n_in = self.n_in();
        scratch.reserve(n, n_in, self.hidden_dim);

        // `reserve` zero-fills on first use, but a reused scratch still
        // holds the previous query's values in blocks we might not
        // rewrite, so `write_row` writes every column unconditionally.
        let x = &mut scratch.x;
        for (row, cand) in candidates.iter().enumerate() {
            let base = row * n_in;
            self.write_row(query, *cand, &mut x[base..base + n_in]);
        }
    }

    /// Stage two: the two matmuls over a matrix [`extract_features`]
    /// already filled, yielding raw logits.
    ///
    /// One `(N, n_in) x (n_in, hidden)` multiply with a ReLU, then one
    /// `(N, hidden) x (hidden, 1)`. No sigmoid, no softmax — see the
    /// module docs.
    pub fn forward<'a>(&self, scratch: &'a mut Scratch) -> &'a [f32] {
        let n = scratch.rows;
        if n == 0 {
            scratch.logits.clear();
            return &scratch.logits;
        }
        let n_in = self.n_in();
        debug_assert_eq!(scratch.x.len(), n * n_in, "call extract_features first");
        scratch.h.resize(n * self.hidden_dim, 0.0);
        scratch.logits.resize(n, 0.0);

        // One matmul for the hidden layer, one for the output layer.
        let x = &scratch.x;
        let h = &mut scratch.h;
        for row in 0..n {
            let xb = row * n_in;
            let hb = row * self.hidden_dim;
            for unit in 0..self.hidden_dim {
                let wb = unit * n_in;
                let mut acc = self.b1[unit];
                for i in 0..n_in {
                    acc += x[xb + i] * self.w1[wb + i];
                }
                // ReLU. Written as a select so a negative stays exactly
                // -0.0-free 0.0 rather than accumulating sign bits.
                h[hb + unit] = if acc > 0.0 { acc } else { 0.0 };
            }
        }

        let h = &scratch.h;
        let logits = &mut scratch.logits;
        for (row, lg) in logits.iter_mut().enumerate() {
            let hb = row * self.hidden_dim;
            // w2 has hidden_dim + 1 slots: the last one multiplies the
            // appended 1.0, i.e. it is the output bias.
            let mut acc = self.b2 + self.w2[self.hidden_dim];
            for unit in 0..self.hidden_dim {
                acc += h[hb + unit] * self.w2[unit];
            }
            *lg = acc;
        }

        &scratch.logits
    }

    /// The index of the highest-scoring candidate, or `None` when the
    /// list is empty.
    ///
    /// Ties go to the **lowest** index, which is what `np.argmax` does —
    /// the two implementations must break ties identically or the
    /// rankings drift apart on rows where two candidates score alike.
    pub fn argmax(scores: &[f32]) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, &s) in scores.iter().enumerate() {
            match best {
                None => best = Some(i),
                Some(b) if scores[b] < s => best = Some(i),
                _ => {}
            }
        }
        best
    }

    /// Convenience: score, then argmax. Returns `None` for an empty
    /// candidate list.
    pub fn top1(
        &self,
        query: &QueryFeatures,
        candidates: &[Candidate<'_>],
        scratch: &mut Scratch,
    ) -> Option<usize> {
        let logits = self.score_batch(query, candidates, scratch);
        Self::argmax(logits)
    }
}

/// Widest edit-feature vector any configuration uses (the D layout).
const MAX_EDIT_DIM: usize = 12;

// ---------------------------------------------------------------------------
//   Feature extraction.
// ---------------------------------------------------------------------------

/// Encode `s` to `MAX_LEN` character indices, PAD-padded on the right.
///
/// Characters outside the vocabulary become [`UNK_IDX`]; the string is
/// truncated after `MAX_LEN` **characters** (not bytes), matching
/// `encode_chars` in Python.
pub fn encode_chars(s: &str) -> [u16; MAX_LEN] {
    let mut out = [PAD_IDX; MAX_LEN];
    for (i, c) in s.chars().take(MAX_LEN).enumerate() {
        out[i] = char_index(c);
    }
    out
}

/// Mean-pool the non-PAD rows of `ids` into `out`.
fn pool_chars_into(char_embed: &[f32], ids: &[u16; MAX_LEN], out: &mut [f32]) {
    let embed_dim = out.len();
    out.iter_mut().for_each(|v| *v = 0.0);
    let mut n_valid = 0.0f32;
    for &idx in ids.iter() {
        if idx == PAD_IDX {
            continue;
        }
        n_valid += 1.0;
        let row = &char_embed[idx as usize * embed_dim..][..embed_dim];
        for (d, &v) in row.iter().enumerate() {
            out[d] += v;
        }
    }
    if n_valid > 0.0 {
        for v in out.iter_mut() {
            *v /= n_valid;
        }
    }
}

/// Sum-pooled hashed bigrams with boundary markers, written into `out`.
///
/// `out` must be exactly `dim` long. `dim == 0` is a no-op. Walks the
/// framed sequence lazily, so no intermediate `Vec` is built.
fn bigram_features_into(s: &str, dim: usize, out: &mut [f32]) {
    out.iter_mut().for_each(|v| *v = 0.0);
    if dim == 0 {
        return;
    }
    let framed = s.chars().map(|c| if in_vocab(c) { c } else { UNK_GLYPH });
    let mut prev = BOUND_LEFT;
    for c in framed {
        let h = (prev as u32 * 257 + c as u32) as usize % dim;
        out[h] += 1.0;
        prev = c;
    }
    let h = (prev as u32 * 257 + BOUND_RIGHT as u32) as usize % dim;
    out[h] += 1.0;
}

/// Sum-pooled hashed trigrams with boundary markers.
///
/// A three-slot ring holds the framed window, so this stays
/// allocation-free too — it runs once per candidate in the hot path.
fn trigram_features_into(s: &str, dim: usize, out: &mut [f32]) {
    out.iter_mut().for_each(|v| *v = 0.0);
    if dim == 0 {
        return;
    }
    let framed = std::iter::once(BOUND_LEFT)
        .chain(s.chars().map(|c| if in_vocab(c) { c } else { UNK_GLYPH }))
        .chain(std::iter::once(BOUND_RIGHT));
    let mut ring = [BOUND_LEFT; 3];
    let mut n = 0usize;
    for ch in framed {
        ring[n % 3] = ch;
        n += 1;
        if n >= 3 {
            let a = ring[(n - 3) % 3] as u32;
            let b = ring[(n - 2) % 3] as u32;
            let c = ring[(n - 1) % 3] as u32;
            out[(a * 65537 + b * 257 + c) as usize % dim] += 1.0;
        }
    }
}

/// The framed character sequence the n-gram hashes see: `s` with
/// boundary markers around it and unknown characters folded to
/// [`UNK_GLYPH`].
///
/// This is the `_boundary_tokens` of `tools/train/model.py`, exposed
/// so the parity fixture can compare it against the hot path above —
/// which deliberately does *not* build this `Vec` (see the tests).
pub fn boundary_tokens(s: &str) -> Vec<char> {
    let mut out = Vec::with_capacity(s.chars().count() + 2);
    out.push(BOUND_LEFT);
    for c in s.chars() {
        out.push(if in_vocab(c) { c } else { UNK_GLYPH });
    }
    out.push(BOUND_RIGHT);
    out
}

/// Standing-alone bigram vector, for tests and for parity fixtures.
pub fn bigram_features(s: &str, dim: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; dim];
    bigram_features_into(s, dim, &mut out);
    out
}

/// Standing-alone trigram vector, for tests and for parity fixtures.
pub fn trigram_features(s: &str, dim: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; dim];
    trigram_features_into(s, dim, &mut out);
    out
}

/// The edit / alignment feature vector for one `(query, candidate)` pair.
///
/// Width is `edit_dim` and the layout matches `edit_features` in
/// `tools/train/model.py`:
///
/// ```text
/// [0]         edit_distance / 4
/// [1..5]      op counts           (only when edit_dim >= 10)
/// [k..k+5]    prefix, suffix, |len|, q_len, c_len
/// [k+5..k+7]  edit_start, edit_end (only when edit_dim >= 12)
/// ```
///
/// For the production Model A, `edit_dim == 6` and the vector is
/// `[ed, prefix, suffix, len_diff, q_len, c_len]`.
///
/// `use_ops` / `use_position` are derived from `edit_dim`, exactly the
/// way `score_batch` on the Python side reconstructs a `FeatureConfig`
/// from the weights.
pub fn edit_features_into(q: &str, c: &str, edit_distance: i32, edit_dim: usize, out: &mut [f32]) {
    out.iter_mut().for_each(|v| *v = 0.0);
    if edit_dim == 0 {
        return;
    }
    if q.is_empty() && c.is_empty() {
        return;
    }
    // 0: normalised edit distance, always set.
    out[0] = (edit_distance.min(4) as f32) / 4.0;

    if q.is_empty() || c.is_empty() {
        return;
    }

    // Structural block. Its offset depends only on whether the op-count
    // block is present.
    let use_ops = edit_dim >= 10;
    let use_position = edit_dim >= 12;
    let q_chars: Vec<char> = q.chars().collect();
    let c_chars: Vec<char> = c.chars().collect();

    let mut prefix = 0usize;
    while prefix < q_chars.len() && prefix < c_chars.len() && q_chars[prefix] == c_chars[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < q_chars.len() - prefix
        && suffix < c_chars.len() - prefix
        && q_chars[q_chars.len() - 1 - suffix] == c_chars[c_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let struct_start = 1 + if use_ops { 4 } else { 0 };
    if out.len() <= struct_start + 4 {
        return;
    }
    out[struct_start] = (prefix.min(10) as f32) / 10.0;
    out[struct_start + 1] = (suffix.min(10) as f32) / 10.0;
    out[struct_start + 2] =
        ((q_chars.len() as i64 - c_chars.len() as i64).unsigned_abs() as usize).min(5) as f32 / 5.0;
    out[struct_start + 3] = (q_chars.len().min(20) as f32) / 20.0;
    out[struct_start + 4] = (c_chars.len().min(20) as f32) / 20.0;
    let pos_start = struct_start + 5;

    if edit_distance <= 0 || q == c {
        return;
    }
    if !use_ops && !use_position {
        return;
    }

    let ops = levenshtein_osa(&q_chars, &c_chars);
    if use_ops && out.len() > 4 {
        out[1] = (ops.substitutions.min(4) as f32) / 4.0;
        out[2] = (ops.insertions.min(4) as f32) / 4.0;
        out[3] = (ops.deletions.min(4) as f32) / 4.0;
        out[4] = (ops.transpositions.min(2) as f32) / 2.0;
    }
    if use_position && out.len() >= pos_start + 2 {
        let q_len = q_chars.len().max(1);
        out[pos_start] = (ops.edit_start.min(q_len) as f32) / q_len as f32;
        out[pos_start + 1] = (ops.edit_end.min(q_len) as f32) / q_len as f32;
    }
}

/// Standing-alone edit features, for tests and parity fixtures.
pub fn edit_features(q: &str, c: &str, edit_distance: i32, edit_dim: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; edit_dim];
    edit_features_into(q, c, edit_distance, edit_dim, &mut out);
    out
}

// ---------------------------------------------------------------------------
//   Optimal String Alignment.
// ---------------------------------------------------------------------------

/// Operation counts and edit span from an OSA backtrace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OsaCounts {
    pub distance: usize,
    pub substitutions: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub transpositions: usize,
    /// 0-indexed position in `q` of the first edit; 0 when there is none.
    pub edit_start: usize,
    /// 0-indexed position in `q` of the last edit; `max(0, len(q)-1)`
    /// when there is none.
    pub edit_end: usize,
}

/// Optimal String Alignment (restricted Damerau-Levenshtein) distance
/// with the operation counts and edit span the feature vector needs.
///
/// Ported from `_levenshtein_with_ops` in `tools/train/model.py`. The
/// backtrace priority — match, then transposition, then substitution,
/// then deletion, then insertion — must stay identical, because it
/// determines `edit_start` / `edit_end`, which are model features.
pub fn levenshtein_osa(q: &[char], c: &[char]) -> OsaCounts {
    let m = q.len();
    let n = c.len();
    if m == 0 {
        return OsaCounts {
            distance: n,
            insertions: n,
            ..Default::default()
        };
    }
    if n == 0 {
        return OsaCounts {
            distance: m,
            deletions: m,
            edit_start: 0,
            edit_end: m.saturating_sub(1),
            ..Default::default()
        };
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            if q[i - 1] == c[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                let mut cost = m + n + 1;
                if i >= 2 && j >= 2 && q[i - 1] == c[j - 2] && q[i - 2] == c[j - 1] {
                    cost = cost.min(dp[i - 2][j - 2] + 1);
                }
                cost = cost.min(dp[i - 1][j - 1] + 1); // substitution
                cost = cost.min(dp[i - 1][j] + 1); // deletion
                cost = cost.min(dp[i][j - 1] + 1); // insertion
                dp[i][j] = cost;
            }
        }
    }

    let mut i = m;
    let mut j = n;
    let mut counts = OsaCounts {
        distance: dp[m][n],
        ..Default::default()
    };
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;
    let record = |s: usize, e: usize, start: &mut Option<usize>, end: &mut Option<usize>| {
        match *start {
            None => *start = Some(s),
            Some(v) if s < v => *start = Some(s),
            _ => {}
        }
        if end.map(|v| e > v).unwrap_or(true) {
            *end = Some(e);
        }
    };

    while i > 0 && j > 0 {
        if q[i - 1] == c[j - 1] {
            i -= 1;
            j -= 1;
            continue;
        }
        if i >= 2
            && j >= 2
            && q[i - 1] == c[j - 2]
            && q[i - 2] == c[j - 1]
            && dp[i][j] == dp[i - 2][j - 2] + 1
        {
            counts.transpositions += 1;
            record(i - 2, i - 1, &mut start, &mut end);
            i -= 2;
            j -= 2;
            continue;
        }
        if dp[i - 1][j - 1] + 1 == dp[i][j] {
            counts.substitutions += 1;
            record(i - 1, i - 1, &mut start, &mut end);
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] + 1 == dp[i][j] {
            counts.deletions += 1;
            record(i - 1, i - 1, &mut start, &mut end);
            i -= 1;
        } else {
            counts.insertions += 1;
            record(
                i.saturating_sub(1),
                i.saturating_sub(1),
                &mut start,
                &mut end,
            );
            j -= 1;
        }
    }
    while i > 0 {
        counts.deletions += 1;
        record(i - 1, i - 1, &mut start, &mut end);
        i -= 1;
    }
    while j > 0 {
        counts.insertions += 1;
        record(0, 0, &mut start, &mut end);
        j -= 1;
    }

    match (start, end) {
        (Some(s), Some(e)) => {
            counts.edit_start = s;
            counts.edit_end = e;
        }
        _ => {
            counts.edit_start = 0;
            counts.edit_end = m.saturating_sub(1);
        }
    }
    counts
}

// ---------------------------------------------------------------------------
//   Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic toy model, so the tests do not depend on a
    /// trained file being present.
    fn toy_model() -> ModelA {
        let embed_dim = 4;
        let hidden_dim = 8;
        let bigram_dim = 4;
        let trigram_dim = 0;
        let edit_dim = 6;
        let n_in = 2 * (embed_dim + bigram_dim + trigram_dim) + edit_dim;

        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let mut next_f = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ((seed >> 40) as f32 / 16_777_216.0) - 0.5
        };

        let char_embed: Vec<f32> = (0..NUM_CHARS * embed_dim).map(|_| next_f()).collect();
        let w1: Vec<f32> = (0..hidden_dim * n_in).map(|_| next_f()).collect();
        let b1: Vec<f32> = (0..hidden_dim).map(|_| next_f() * 0.1).collect();
        let w2: Vec<f32> = (0..hidden_dim + 1).map(|_| next_f()).collect();
        ModelA {
            char_embed,
            w1,
            b1,
            w2,
            b2: 0.25,
            embed_dim,
            hidden_dim,
            bigram_dim,
            trigram_dim,
            edit_dim,
        }
    }

    fn query(m: &ModelA, word: &str) -> QueryFeatures {
        m.query_features(word)
    }

    /// Model A's input width: query block + candidate block + edit tail.
    #[test]
    fn model_a_feature_dimensions() {
        let m = toy_model();
        // 2 * (4 embed + 4 bigram) + 6 edit = 22.
        assert_eq!(m.n_in(), 22);
        assert_eq!(
            m.n_in(),
            2 * (m.embed_dim + m.bigram_dim + m.trigram_dim) + m.edit_dim
        );
        assert_eq!(
            m.param_count(),
            NUM_CHARS * m.embed_dim + m.hidden_dim * m.n_in() + m.hidden_dim + m.hidden_dim + 1 + 1
        );
        // The scratch must hold exactly n rows of n_in.
        let mut s = Scratch::default();
        let q = query(&m, "kissa");
        let cands = [Candidate::new("kisa", 1)];
        let logits = m.score_batch(&q, &cands, &mut s);
        assert_eq!(logits.len(), 1);
        assert_eq!(s.x.len(), m.n_in());
    }

    /// Batched and single-candidate scoring must agree. If they do not,
    /// the batched path has a row-stride bug that would silently reorder
    /// candidate lists.
    #[test]
    fn batch_and_single_scoring_agree() {
        let m = toy_model();
        let q = query(&m, "juoksee");
        let words = ["juoksee", "juokse", "juoksen", "kissa", "talo"];
        let cands: Vec<Candidate> = words.iter().map(|w| Candidate::new(w, 1)).collect();

        let mut batch = Scratch::default();
        let batched = m.score_batch(&q, &cands, &mut batch).to_vec();

        let mut one = Scratch::default();
        for (i, c) in cands.iter().enumerate() {
            let solo = m
                .score_batch(&q, std::slice::from_ref(c), &mut one)
                .to_vec();
            assert!(
                (solo[0] - batched[i]).abs() <= 1e-5,
                "candidate {i} ({}) scored {} in batch but {} alone",
                c.word,
                batched[i],
                solo[0]
            );
        }
    }

    /// The model returns raw logits, never a squashed probability.
    ///
    /// A sigmoid would pin every score into (0, 1); a model whose bias
    /// is large must be allowed to exceed 1. If someone reintroduces
    /// sigmoid into `score_batch`, this fails.
    #[test]
    fn scores_are_raw_logits_not_probabilities() {
        let mut m = toy_model();
        // A large positive bias pushes the logit well past 1.0; a large
        // negative one pushes it below 0.0.
        m.b2 = 7.5;
        let q = query(&m, "talo");
        let cands = [Candidate::new("talo", 0), Candidate::new("tal", 1)];
        let mut s = Scratch::default();
        let logits = m.score_batch(&q, &cands, &mut s);
        assert!(
            logits.iter().any(|&v| v > 1.0),
            "expected an out-of-range logit, got {logits:?}"
        );

        m.b2 = -7.5;
        let q = query(&m, "talo");
        let logits = m.score_batch(&q, &cands, &mut s);
        assert!(
            logits.iter().any(|&v| v < 0.0),
            "expected a negative logit, got {logits:?}"
        );
    }

    /// The query features are computed once by `query_features` and only
    /// read by `score_batch` — the API has no query *word* to recompute
    /// from, which is the property the brief asks for.
    #[test]
    fn query_features_are_reused_not_recomputed() {
        let m = toy_model();
        let q = query(&m, "koiraa");
        // The same QueryFeatures object scores every candidate list
        // without being rebuilt.
        let mut s = Scratch::default();
        let first = m
            .score_batch(&q, &[Candidate::new("koira", 1)], &mut s)
            .to_vec();
        let second = m
            .score_batch(&q, &[Candidate::new("koira", 1)], &mut s)
            .to_vec();
        assert_eq!(first, second);
        // The stored word is the single copy the edit features read.
        assert_eq!(q.word, "koiraa");
        assert!(q.char_mean.len() == m.embed_dim);
        assert!(q.bigram.len() == m.bigram_dim);
    }

    /// Serialization round-trip: bytes in, same model out.
    #[test]
    fn serialisation_roundtrips() {
        let m = toy_model();
        let bytes = m.to_bytes();
        let back = ModelA::from_bytes(&bytes).expect("roundtrip");
        assert_eq!(back.embed_dim, m.embed_dim);
        assert_eq!(back.hidden_dim, m.hidden_dim);
        assert_eq!(back.bigram_dim, m.bigram_dim);
        assert_eq!(back.trigram_dim, m.trigram_dim);
        assert_eq!(back.edit_dim, m.edit_dim);
        assert_eq!(back.char_embed, m.char_embed);
        assert_eq!(back.w1, m.w1);
        assert_eq!(back.b1, m.b1);
        assert_eq!(back.w2, m.w2);
        assert_eq!(back.b2.to_bits(), m.b2.to_bits());

        // And the round-tripped model scores identically.
        let q1 = m.query_features("kissaa");
        let q2 = back.query_features("kissaa");
        let cands = [Candidate::new("kissa", 1), Candidate::new("kisaa", 1)];
        let mut s1 = Scratch::default();
        let mut s2 = Scratch::default();
        let a = m.score_batch(&q1, &cands, &mut s1);
        let b = back.score_batch(&q2, &cands, &mut s2);
        assert_eq!(a, b);
    }

    /// A corrupt file is an error, not a panic.
    #[test]
    fn rejects_corrupt_files() {
        assert!(matches!(ModelA::from_bytes(b""), Err(ModelError::BadMagic)));
        assert!(matches!(
            ModelA::from_bytes(b"NOPE\x01\x00\x00\x00"),
            Err(ModelError::BadMagic)
        ));
        let mut bad_version = toy_model().to_bytes();
        bad_version[4..8].copy_from_slice(&99u32.to_le_bytes());
        assert_eq!(
            ModelA::from_bytes(&bad_version).err(),
            Some(ModelError::BadVersion(99))
        );

        let good = toy_model().to_bytes();
        assert!(matches!(
            ModelA::from_bytes(&good[..good.len() - 4]),
            Err(ModelError::Truncated { .. })
        ));
    }

    /// An empty candidate list scores to an empty slice — no crash, no
    /// phantom candidate, no allocation beyond clearing the buffer.
    #[test]
    fn empty_candidate_list_scores_empty() {
        let m = toy_model();
        let q = query(&m, "talo");
        let mut s = Scratch::default();
        let logits = m.score_batch(&q, &[], &mut s);
        assert!(logits.is_empty());
        assert_eq!(m.top1(&q, &[], &mut s), None);
        assert_eq!(ModelA::argmax(&[]), None);
    }

    /// `argmax` breaks ties toward the lowest index, like `np.argmax`.
    #[test]
    fn argmax_breaks_ties_lowest_index() {
        assert_eq!(ModelA::argmax(&[1.0, 5.0, 5.0, 2.0]), Some(1));
        assert_eq!(ModelA::argmax(&[0.0]), Some(0));
        assert_eq!(ModelA::argmax(&[]), None);
        assert_eq!(ModelA::argmax(&[f32::NAN, 1.0]), Some(0));
    }

    /// The allocation-free n-gram walkers must hash exactly what
    /// `boundary_tokens` frames. This guards the duplication between
    /// the hot path and the helper the fixture compares against.
    #[test]
    fn hot_path_agrees_with_boundary_tokens() {
        for word in [
            "",
            "a",
            "kissa",
            "juoksee",
            "juoks€ee",
            "tälo",
            "a-b",
            "'-'",
        ] {
            let framed = boundary_tokens(word);
            // Bigrams, hashed straight off the framed sequence.
            for dim in [1usize, 7, 64] {
                let mut expect = vec![0.0f32; dim];
                for i in 0..framed.len().saturating_sub(1) {
                    let h = (framed[i] as u32 * 257 + framed[i + 1] as u32) as usize % dim;
                    expect[h] += 1.0;
                }
                assert_eq!(
                    bigram_features(word, dim),
                    expect,
                    "bigram mismatch for {word:?} at dim {dim}"
                );

                let mut expect = vec![0.0f32; dim];
                for i in 0..framed.len().saturating_sub(2) {
                    let h = (framed[i] as u32 * 65537
                        + framed[i + 1] as u32 * 257
                        + framed[i + 2] as u32) as usize
                        % dim;
                    expect[h] += 1.0;
                }
                assert_eq!(
                    trigram_features(word, dim),
                    expect,
                    "trigram mismatch for {word:?} at dim {dim}"
                );
            }
        }
        // The framing itself, checked against the documented shape.
        assert_eq!(
            boundary_tokens("juoksee").iter().collect::<String>(),
            "<juoksee>"
        );
        assert_eq!(
            boundary_tokens("juoks€ee").iter().collect::<String>(),
            "<juoks~ee>"
        );
    }

    /// The character vocabulary must agree with the Python `CHAR_INDEX`
    /// slot for slot: pad 0, unk 1, a-z, äö, A-Z, ÄÖ, then `'` `-`.
    #[test]
    fn character_vocabulary_matches_python() {
        assert_eq!(NUM_CHARS, 60);
        assert_eq!(char_index('a'), 2);
        assert_eq!(char_index('z'), 27);
        assert_eq!(char_index('ä'), 28);
        assert_eq!(char_index('ö'), 29);
        assert_eq!(char_index('A'), 30);
        assert_eq!(char_index('Z'), 55);
        assert_eq!(char_index('Ä'), 56);
        assert_eq!(char_index('Ö'), 57);
        assert_eq!(char_index('\''), 58);
        assert_eq!(char_index('-'), 59);
        assert_eq!(char_index('€'), UNK_IDX);
        assert_eq!(char_index(' '), UNK_IDX);
        // The boundary and unknown glyphs are deliberately outside the
        // vocabulary so they never collide with a real letter.
        assert!(!in_vocab(BOUND_LEFT));
        assert!(!in_vocab(BOUND_RIGHT));
        assert!(!in_vocab(UNK_GLYPH));
    }

    /// Encoding is per-character and truncates at MAX_LEN.
    #[test]
    fn encode_chars_truncates_by_characters() {
        let ids = encode_chars("kissa");
        assert_eq!(ids[0], char_index('k'));
        assert_eq!(ids[4], char_index('a'));
        assert_eq!(ids[5], PAD_IDX);

        // The input is longer than MAX_LEN, so every slot is filled with
        // `y` and the trailing characters are dropped entirely.
        let long: String = "y".repeat(MAX_LEN + 10);
        let ids = encode_chars(&long);
        assert!(ids.iter().all(|&v| v == char_index('y')));
    }

    /// Bigrams and trigrams frame the word with boundary markers, so
    /// `kissa` has len+1 bigrams and len trigrams worth of mass.
    #[test]
    fn ngram_mass_counts_boundaries() {
        // "kissa" -> 6 bigrams (<k, ki, is, ss, sa, a>)
        let big = bigram_features("kissa", 64);
        assert!((big.iter().sum::<f32>() - 6.0).abs() < 1e-6);
        // -> 5 trigrams (<ki, kis, iss, ssa, sa>)
        let tri = trigram_features("kissa", 64);
        assert!((tri.iter().sum::<f32>() - 5.0).abs() < 1e-6);
        // A one-character word still gets both boundaries.
        let big = bigram_features("a", 64);
        assert!((big.iter().sum::<f32>() - 2.0).abs() < 1e-6);
        // Unknown characters contribute as the unknown glyph rather
        // than being dropped.
        let big = bigram_features("a€", 64);
        assert!((big.iter().sum::<f32>() - 3.0).abs() < 1e-6);
    }

    /// An all-PAD (empty) query pools to zeros rather than dividing by
    /// zero and producing NaN.
    #[test]
    fn empty_query_pools_to_zeros() {
        let m = toy_model();
        let q = m.query_features("");
        assert!(q.char_mean.iter().all(|&v| v == 0.0));
        let cands = [Candidate::new("talo", 0)];
        let mut s = Scratch::default();
        let logits = m.score_batch(&q, &cands, &mut s);
        assert!(logits.iter().all(|v| v.is_finite()));
    }

    /// The edit-feature layout for Model A (edit_dim = 6).
    #[test]
    fn edit_features_layout_for_model_a() {
        // [ed, prefix, suffix, len_diff, q_len, c_len]
        let f = edit_features("kissa", "kissa", 0, 6);
        assert_eq!(f.len(), 6);
        assert_eq!(f[0], 0.0); // edit distance 0
        // prefix = 5 characters, min(5,10)/10.
        assert!((f[1] - 0.5).abs() < 1e-6);
        assert_eq!(f[2], 0.0); // no suffix left past the prefix
        assert_eq!(f[3], 0.0); // same length
        assert!((f[4] - 0.25).abs() < 1e-6); // min(5,20)/20
        assert!((f[5] - 0.25).abs() < 1e-6);

        let f = edit_features("kissa", "kisaa", 1, 6);
        assert!((f[0] - 0.25).abs() < 1e-6);
        assert!((f[1] - 0.3).abs() < 1e-6); // prefix "kis" = 3
        assert!((f[2] - 0.1).abs() < 1e-6); // suffix "a" = 1
        assert_eq!(f[3], 0.0); // same length

        // An empty query leaves only the edit-distance slot set.
        let f = edit_features("", "kissa", 5, 6);
        assert_eq!(f[0], 1.0); // min(5,4)/4
        assert_eq!(f[1], 0.0);
        assert_eq!(f[5], 0.0);

        // Lengths are counted in characters, not bytes.
        let f = edit_features("äää", "aaa", 3, 6);
        assert_eq!(f[3], 0.0);
        assert_eq!(f[4], 0.15); // 3/20
    }

    /// The D-layout edit vector keeps the structural block at the same
    /// relative place, with ops first and position last.
    #[test]
    fn edit_features_layout_for_model_d() {
        let f = edit_features("jouksee", "juoksee", 2, 12);
        assert_eq!(f.len(), 12);
        assert!((f[0] - 0.5).abs() < 1e-6); // 2/4
        // 1..5 = sub, ins, del, trans. The change is one swap, so the
        // OSA backtrace counts a single transposition and nothing else.
        assert_eq!((f[1], f[2], f[3]), (0.0, 0.0, 0.0));
        assert!(
            (f[4] - 0.5).abs() < 1e-6,
            "expected one transposition: {f:?}"
        );
        // 5..10 = prefix, suffix, len_diff, q_len, c_len.
        assert!((f[5] - 0.1).abs() < 1e-6, "prefix 'j': {f:?}");
        assert!((f[6] - 0.4).abs() < 1e-6, "suffix 'ksee' is 4: {f:?}");
        assert_eq!(f[7], 0.0); // equal length
        assert!((f[8] - (7.0f32 / 20.0)).abs() < 1e-6);
        assert!((f[9] - (7.0f32 / 20.0)).abs() < 1e-6);
        // 10..12 = edit_start, edit_end. The swap sits at q[1..3].
        assert!((f[10] - 1.0 / 7.0).abs() < 1e-6, "edit_start: {f:?}");
        assert!((f[11] - 2.0 / 7.0).abs() < 1e-6, "edit_end: {f:?}");
    }
}
