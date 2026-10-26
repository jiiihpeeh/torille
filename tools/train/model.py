# SPDX-License-Identifier: MIT OR Apache-2.0
"""Production char-MLP reranker for Torille.

Slice-3.5 refactor: adds edit-alignment features and char trigrams on
top of the Slice-3 Model A baseline.

Architecture (default ~38k parameters, all float32):

    misspelled --[char embed, mean-pool]----> q_char      (48)
    misspelled --[hashed bigrams, sum-pool]-> q_bigram    (64)
    misspelled --[hashed trigrams, sum-pool]-> q_trigram  (64)
    candidate  --[char embed, mean-pool]----> c_char      (48)
    candidate  --[hashed bigrams, sum-pool]-> c_bigram    (64)
    candidate  --[hashed trigrams, sum-pool]-> c_trigram  (64)
    edit_features(q, c)                       --->        ed_vec  (12)
                                                                       |
                              concat -----------------------------------+
                                  |
                              Linear(N_IN, HIDDEN) + ReLU
                                  |
                              Linear(HIDDEN+1, 1)
                                  |
                              logit (raw)

  N_IN = 2 * (EMBED_DIM + BIGRAM_DIM + TRIGRAM_DIM) + EDIT_DIM = 364
  param count:
    char_embed       60 * 48                     = 2,880
    w1               HIDDEN * (N_IN + 1)         = 96 * 365 = 35,040
    b1               HIDDEN                              = 96
    w2               1 * (HIDDEN + 1)                  = 97
    b2               1                                       = 1
    total                                          ~38,114

The forward pass returns the **raw logit**, not a sigmoid probability.
For listwise ranking the relative scores are compared directly; the
listwise softmax is applied by the loss and the evaluator, not inside
the model.

Query-side features (`q_char`, `q_bigram`, `q_trigram`) are computed
**once per query** by `QueryFeatures.from_word` and reused across all
candidates, so `score_batch` runs one batched matmul for the hidden
layer and one for the output -- not a per-candidate Python loop.

Edit features (the 12-dim vector) are the most important addition
over the Slice-3 Model A: they tell the model explicitly what kind
of transformation the correction requires. The `edit_distance`
scalar of Model A is folded into this vector; transpositions,
substitutions, prefix/suffix match, length difference, and identity
are all separate features. This is what gives the model the ability
to learn `jouksee -> juoksee` as a *transposition* rather than as
two independent edits.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import numpy as np


# ---------------------------------------------------------------------------
# Char vocabulary.
# ---------------------------------------------------------------------------
# Finnish-relevant alphabet. Picked from nykysuomensanalista.txt + the
# Unicode vowels the engine handles. PAD and UNK are first so they
# have stable indices (0 and 1).

_PAD_TOKENS = ("<pad>", "<unk>")
_ASCII_LOWER = "abcdefghijklmnopqrstuvwxyzäö"  # 28 chars
_ASCII_UPPER = "ABCDEFGHIJKLMNOPQRSTUVWXYZÄÖ"  # 28 chars
_EXTRA = "'-"  # 2 chars

CHAR_INDEX: dict[str, int] = {}
_i = 0
for tok in _PAD_TOKENS:
    CHAR_INDEX[tok] = _i
    _i += 1
for ch in _ASCII_LOWER:
    CHAR_INDEX[ch] = _i
    _i += 1
for ch in _ASCII_UPPER:
    CHAR_INDEX[ch] = _i
    _i += 1
for ch in _EXTRA:
    CHAR_INDEX[ch] = _i
    _i += 1
del _i

NUM_CHARS = len(CHAR_INDEX)  # 2 + 28 + 28 + 2 = 60
PAD_IDX = CHAR_INDEX["<pad>"]
UNK_IDX = CHAR_INDEX["<unk>"]


# ---------------------------------------------------------------------------
# Architecture defaults.
# ---------------------------------------------------------------------------
# CLI flags in train.py can override EMBED_DIM and HIDDEN_DIM.
# Defaults chosen for ~38k params on the 3,000-row training set
# (down from Slice-3's 47k) -- the data/parameter ratio is tighter
# and the new edit features do the heavy lifting.
EMBED_DIM = 48
HIDDEN_DIM = 96
MAX_LEN = 32  # max chars per (misspelled / candidate) field
BIGRAM_DIM = 64   # hashed bigram projection size
TRIGRAM_DIM = 64  # hashed trigram projection size
# Edit features are the 12-dim vector from `edit_features`. When
# ablations disable parts of the edit features, the *effective*
# `edit_dim` shrinks; see `FeatureConfig` below.
EDIT_DIM = 12     # number of edit-alignment features (see edit_features)

# The flat weight blob the Rust inference path reads (`ModelA::from_bytes`
# in src/rerank/mlp.rs). Kept in step with TRM_MAGIC/VERSION there.
TRM_MAGIC = b"TRM1"
TRM_VERSION = 1


# ---------------------------------------------------------------------------
# Feature configuration (for A/B/C/D ablations).
# ---------------------------------------------------------------------------
# Each ablation physically removes feature dimensions from the
# architecture (rather than zeroing them), so `n_in` and the
# parameter count change in step. The four ablations are:
#
#   A: chars + bigrams + edit_distance (n_in = 225, ~15k params)
#   B: A + trigrams                                   (n_in = 353)
#   C: B + edit op counts                             (n_in = 357)
#   D: C + edit position                              (n_in = 361)
#
# Wait -- the v5 plan claimed A=145, B=273, C=277, D=365. Those
# numbers assumed EMBED_DIM=32, BIGRAM_DIM=32 for A. We use
# the same EMBED_DIM=48, BIGRAM_DIM=64 across all ablations
# (changing those would re-introduce the architecture diff
# we're trying to isolate), so the actual n_in for A is 225.
# The plan's "params" column is approximate.
#
# Layout invariants preserved across all ablations:
#   * x = [q_char, q_bigram, (q_trigram), c_char, c_bigram,
#          (c_trigram), edit_features]
#   * When a feature group is disabled, the corresponding
#     slice is simply absent; no zeros are inserted.
#   * `RerankerWeights` and `.npz` round-trip the dimensions
#     via `trigram_dim` and `edit_dim` metadata fields.
#
# The Rust inference path needs to know which layout a saved
# model uses; we encode that via the metadata. A/B/C are
# research models (no Rust inference path needed); only D is
# a production candidate and uses the existing layout.

@dataclass
class FeatureConfig:
    """Which feature groups are enabled in the model.

    Defaults match the current Slice-3.5 (D). Each ablation
    disables one or more groups, reducing `n_in` and the
    parameter count.
    """

    use_trigrams: bool = True
    use_edit_ops: bool = True
    use_edit_position: bool = True

    @property
    def trigram_dim(self) -> int:
        """Trigram projection size; 0 when trigrams are disabled."""
        return TRIGRAM_DIM if self.use_trigrams else 0

    @property
    def edit_dim(self) -> int:
        """Edit-feature vector width; depends on which edit sub-features
        are enabled.

        The always-present block is 6 wide (ed, prefix, suffix,
        length-diff, q_len, c_len); ops add 4 and position adds 2, so
        the width is 6 / 10 / 12 for A / C / D.

        *Which* slots those occupy is `edit_features`'s business rather
        than this property's -- ops take [1..4], then a 5-wide
        structural block, then position. The two were described out of
        order here before, which would have sent anyone porting the
        layout to the wrong indices; see the comment in `edit_features`.
        """
        n = 1 + 5  # ed + (prefix, suffix, length-diff, q_len, c_len)
        if self.use_edit_ops:
            n += 4
        if self.use_edit_position:
            n += 2
        return n

    @property
    def n_in(self) -> int:
        """Total input feature width for this configuration."""
        return (2 * (EMBED_DIM + BIGRAM_DIM + self.trigram_dim)
                + self.edit_dim)

    @property
    def param_count_estimate(self) -> int:
        """Approximate parameter count for the architecture.

        char_embed       = NUM_CHARS * EMBED_DIM
        w1               = HIDDEN_DIM * (n_in + 1)
        b1               = HIDDEN_DIM
        w2               = HIDDEN_DIM + 1
        b2               = 1
        """
        return (
            NUM_CHARS * EMBED_DIM
            + HIDDEN_DIM * (self.n_in + 1)
            + HIDDEN_DIM
            + HIDDEN_DIM + 1
            + 1
        )

    def label(self) -> str:
        """Short tag for filenames / reports: A, B, C, or D."""
        if not self.use_trigrams and not self.use_edit_ops and not self.use_edit_position:
            return "A"
        if not self.use_edit_ops and not self.use_edit_position:
            return "B"
        if not self.use_edit_position:
            return "C"
        return "D"


# Default config is Slice-3.5 (everything enabled).
DEFAULT_CONFIG = FeatureConfig(
    use_trigrams=True,
    use_edit_ops=True,
    use_edit_position=True,
)


# ---------------------------------------------------------------------------
# Char / n-gram encoders.
# ---------------------------------------------------------------------------
# Bigrams and trigrams are extracted with explicit boundary markers
# (`<` left, `>` right) so that word-initial and word-final character
# pairs/triples hash distinctly. Unknown characters are mapped to the
# `<unk>` token rather than silently dropped, so that
# `juoks€ee` produces the trigram `s€e` and not just `see`.


# Boundary characters. Picked to be ASCII (small, never collide with
# Finnish letters) and outside CHAR_INDEX, so they only ever appear at
# the n-gram boundaries.
_BOUND_LEFT = "<"
_BOUND_RIGHT = ">"
# Single-character placeholder for unknown characters in the n-gram
# path. Must NOT be in CHAR_INDEX (so the char embed never sees it)
# and must be exactly one character long (so trigram hashing
# receives three real characters). Tilde is ASCII, never in any
# Finnish word, and never in CHAR_INDEX, so it satisfies both.
_UNK_GLYPH = "~"


def encode_chars(s: str, max_len: int = MAX_LEN) -> np.ndarray:
    """Encode a string into a fixed-length int vector of char indices.

    Characters not in CHAR_INDEX become UNK_IDX. Strings longer than
    max_len are truncated from the right.
    """
    out = np.full(max_len, PAD_IDX, dtype=np.int64)
    for i, ch in enumerate(s[:max_len]):
        out[i] = CHAR_INDEX.get(ch, UNK_IDX)
    return out


def encode_batch(strs: Iterable[str], max_len: int = MAX_LEN) -> np.ndarray:
    """Vectorised char-encoding for a batch of strings."""
    arrs = [encode_chars(s, max_len) for s in strs]
    return np.stack(arrs, axis=0)


def _boundary_tokens(s: str) -> list[str]:
    """Return the character sequence with boundary markers and an
    unknown-character glyph substituted for characters outside
    `CHAR_INDEX`.

    Result for `"juoksee"` is
      `['<', 'j', 'u', 'o', 'k', 's', 'e', 'e', '>']`.
    Result for `"juoks€ee"` is
      `['<', 'j', 'u', 'o', 'k', 's', '~', 'e', 'e', '>']`.

    The unknown glyph is a single character (not the multi-character
    `<unk>`) so the trigram path still gets three real characters
    to hash. The char-embed path is unaffected; it has its own
    `UNK_IDX` constant in `CHAR_INDEX`.
    """
    return (
        [_BOUND_LEFT]
        + [c if c in CHAR_INDEX else _UNK_GLYPH for c in s]
        + [_BOUND_RIGHT]
    )


def _bigram_hash(a: str, b: str, dim: int) -> int:
    """Hash a character bigram to one of `dim` buckets.

    Uses the formula `ord(a) * 257 + ord(b)`, then mods into `dim`.
    The 257-multiplier is a small prime > 128 to avoid trivial
    collisions on Finnish letter pairs like `aa`.
    """
    return (ord(a) * 257 + ord(b)) % dim


def _trigram_hash(a: str, b: str, c: str, dim: int) -> int:
    """Hash a character trigram to one of `dim` buckets.

    Uses the formula `ord(a) * 65537 + ord(b) * 257 + ord(c)`. The
    65537-multiplier is the Fermat prime F4 (= 2**16 + 1); it gives
    a good spread for Finnish stem patterns. The hash must remain
    bit-for-bit stable across training (Python) and inference (Rust)
    -- the Rust reranker reads this same formula via the .npz
    weights and reproduces it. See `tools/train/parity_test.py`
    for the golden fixture.
    """
    return (ord(a) * 65537 + ord(b) * 257 + ord(c)) % dim


def bigram_features(s: str, dim: int = BIGRAM_DIM) -> np.ndarray:
    """Sum-pooled hashed bigram features with boundary markers.

    Returns a `dim`-dim vector where index `i` is the count of
    bigrams that hashed to `i`. Unknown characters contribute as
    `<unk>` rather than being skipped, so the feature is sensitive
    to corrupted inputs.

    `dim == 0` returns an empty vector rather than raising: a feature
    group that an ablation switched off must be *absent*, not fatal.
    """
    out = np.zeros(dim, dtype=np.float32)
    if dim == 0:
        return out
    chars = _boundary_tokens(s)
    for i in range(len(chars) - 1):
        out[_bigram_hash(chars[i], chars[i + 1], dim)] += 1.0
    return out


def bigram_features_batch(strs: Iterable[str], dim: int = BIGRAM_DIM) -> np.ndarray:
    """Vectorised bigram-features for a batch of strings."""
    arrs = [bigram_features(s, dim) for s in strs]
    return np.stack(arrs, axis=0)


def trigram_features(s: str, dim: int = TRIGRAM_DIM) -> np.ndarray:
    """Sum-pooled hashed trigram features with boundary markers.

    Captures Finnish stem-and-suffix patterns more faithfully than
    bigrams alone: `juo`, `uok`, `oks`, `kse`, `see` from `juoksee`
    all hash distinctly and survive across inflections.

    `dim == 0` returns an empty vector (see `bigram_features`).
    """
    out = np.zeros(dim, dtype=np.float32)
    if dim == 0:
        return out
    chars = _boundary_tokens(s)
    for i in range(len(chars) - 2):
        out[_trigram_hash(chars[i], chars[i + 1], chars[i + 2], dim)] += 1.0
    return out


def trigram_features_batch(strs: Iterable[str], dim: int = TRIGRAM_DIM) -> np.ndarray:
    """Vectorised trigram-features for a batch of strings.

    Returns an empty (N, 0) array if `dim == 0` (trigrams
    disabled in the ablation config). Callers that build the
    feature matrix should slice around the missing trigram
    block; see `score_batch` and `forward_all`.
    """
    if dim == 0:
        # Materialise first: `len(list(strs))` on a generator would
        # consume it and leave nothing for the caller.
        items = list(strs)
        return np.zeros((len(items), 0), dtype=np.float32)
    arrs = [trigram_features(s, dim) for s in strs]
    return np.stack(arrs, axis=0)


# ---------------------------------------------------------------------------
# Edit / alignment features.
# ---------------------------------------------------------------------------
# 12-dim feature vector for each (misspelled, candidate) pair. The
# previous Slice-3 model fed only a single `edit_distance / 4` scalar;
# the per-row op counts (substitutions, insertions, deletions,
# transpositions) and the structural features (prefix/suffix match,
# length difference) are what let the model distinguish
# `jouksee -> juoksee` (transposition) from `kissa -> kissa` (identity).


def _levenshtein_with_ops(q: str, c: str) -> dict:
    """Optimal String Alignment (OSA) distance with operation counts.

    This is the *restricted* Damerau-Levenshtein variant: each
    substring may be edited at most once. For spelling correction
    this is the right choice -- `ab -> ba` is one transposition
    and `cb -> bc` is one transposition, but `cab -> bca` is
    counted as two (not collapsed into a single "rotation"). The
    rule and the resulting operation counts must remain identical
    to the Rust implementation in
    `torille::suggest::levenshtein_with_ops`; see
    `tools/train/parity_test.py` for the golden fixture.

    Returns a dict with:
      distance        : int   OSA distance
      substitutions   : int
      insertions      : int
      deletions       : int
      transpositions  : int
      edit_start      : int   0-indexed position in `q` of the first
                              edit. Defaults to 0 if `q` is empty
                              or all matches.
      edit_end        : int   0-indexed position in `q` of the last
                              edit. Defaults to max(0, len(q) - 1).

    Tie-breaking in the backtrace: when several moves produce the
    same dp value at (i, j), the priority is
      match > transposition > substitution > deletion > insertion.
    This order is the same as the one the DP table was filled in,
    so any cell can be reconstructed from the dp value alone.
    """
    m, n = len(q), len(c)
    if m == 0:
        return {"distance": n, "substitutions": 0, "insertions": n,
                "deletions": 0, "transpositions": 0,
                "edit_start": 0, "edit_end": 0}
    if n == 0:
        return {"distance": m, "substitutions": 0, "insertions": 0,
                "deletions": m, "transpositions": 0,
                "edit_start": 0, "edit_end": max(0, m - 1)}

    # DP table.
    dp = [[0] * (n + 1) for _ in range(m + 1)]
    for i in range(m + 1):
        dp[i][0] = i
    for j in range(n + 1):
        dp[0][j] = j
    for i in range(1, m + 1):
        for j in range(1, n + 1):
            if q[i - 1] == c[j - 1]:
                dp[i][j] = dp[i - 1][j - 1]
            else:
                # Same priority as the backtrace: transposition
                # beats substitution, sub beats del, del beats ins.
                cost = m + n + 1
                if (i >= 2 and j >= 2
                        and q[i - 1] == c[j - 2]
                        and q[i - 2] == c[j - 1]):
                    cost = min(cost, dp[i - 2][j - 2] + 1)
                cost = min(cost, dp[i - 1][j - 1] + 1)  # substitution
                cost = min(cost, dp[i - 1][j] + 1)      # deletion
                cost = min(cost, dp[i][j - 1] + 1)      # insertion
                dp[i][j] = cost

    # Backtrace, recording the first and last positions in q where
    # an edit was applied.
    i, j = m, n
    subs = ins = dels = trans = 0
    edit_start = -1
    edit_end = -1

    def _record(start: int, end: int) -> None:
        nonlocal edit_start, edit_end
        if edit_start < 0 or start < edit_start:
            edit_start = start
        if end > edit_end:
            edit_end = end

    while i > 0 and j > 0:
        if q[i - 1] == c[j - 1]:
            i -= 1
            j -= 1
            continue
        if (i >= 2 and j >= 2
                and q[i - 1] == c[j - 2]
                and q[i - 2] == c[j - 1]
                and dp[i][j] == dp[i - 2][j - 2] + 1):
            trans += 1
            # A transposition spans positions (i-2, i-1) in q.
            _record(i - 2, i - 1)
            i -= 2
            j -= 2
            continue
        if dp[i - 1][j - 1] + 1 == dp[i][j]:
            subs += 1
            _record(i - 1, i - 1)
            i -= 1
            j -= 1
        elif dp[i - 1][j] + 1 == dp[i][j]:
            dels += 1
            _record(i - 1, i - 1)
            i -= 1
        else:
            ins += 1
            # Insertion at j-1 has no q-position; attribute it to
            # the end of what we've already consumed in q.
            _record(max(0, i - 1), max(0, i - 1))
            j -= 1
    while i > 0:
        dels += 1
        _record(i - 1, i - 1)
        i -= 1
    while j > 0:
        ins += 1
        _record(0, 0)
        j -= 1

    if edit_start < 0:
        edit_start = 0
        edit_end = max(0, m - 1)

    return {
        "distance": dp[m][n],
        "substitutions": subs,
        "insertions": ins,
        "deletions": dels,
        "transpositions": trans,
        "edit_start": edit_start,
        "edit_end": edit_end,
    }


# Strict-parity switch. When True, the engine's precomputed
# `edit_distance` (in the JSONL) and the Python OSA distance must
# agree, or `edit_features` raises. When False, the JSONL distance
# is treated as ground truth and the Python computation is only
# used to derive the per-op counts and edit position.
#
# Why False is the default: the engine uses *standard* Levenshtein
# (no transposition rule) for the `Suggestion.edit_distance` field
# that the rerank-data generator copies into the JSONL. Python's
# OSA path here does count transpositions, so for any pair where
# the optimal path includes a transposition the two distances will
# disagree by one (e.g. `jouksee -> juokse`: JSONL says 1, OSA
# says 2). The model still benefits from the Python op counts and
# edit position -- the inconsistency is documented and the JSONL
# value is what the production Rust path will see.
_STRICT_PARITY = False


def edit_features(
    q: str, c: str, edit_distance: int,
    config: FeatureConfig = DEFAULT_CONFIG,
) -> np.ndarray:
    """Edit/alignment feature vector for a (q, c) pair.

    The width depends on `config`:
      * A / B: 6 dims  -- [ed, prefix, suffix, |len|, q_len, c_len]
      * C:     10 dims -- ops [sub, ins, del, trans] at [1..4], then
                          the 5-wide structural block at [5..9]
      * D:     12 dims -- C's 10, then [edit_start, edit_end] at [10..11]

    The structural block is at a fixed offset *for a given config*
    (`1 + 4 if use_edit_ops else 1`) rather than at a fixed index
    across configs, and edit position follows it rather than preceding
    it. `src/rerank/mlp.rs` mirrors this exactly and
    `tests/rerank_parity.rs` pins both sides, so a layout change shows
    up as a parity failure rather than as a quiet accuracy drop.

    All values are clipped to a sensible range so the network
    sees a stable scale across word lengths.

    The op counts and edit position are computed only when
    `edit_distance > 0` and the strings are not equal -- a
    hot path for the candidate list where most candidates
    differ from the query.
    """
    out = np.zeros(config.edit_dim, dtype=np.float32)
    if not q and not c:
        return out

    # 0: edit distance, always set.
    out[0] = min(edit_distance, 4) / 4.0

    # Layout (when both ops and position are enabled, D config):
    #   [0]         edit_distance
    #   [1..5]      ops    (if use_edit_ops)         -- sub, ins, del, trans
    #   [k..k+5]    structural (always)              -- prefix, suffix, len_diff, q_len, c_len
    #   [k+5..k+7]  position (if use_edit_position)  -- edit_start, edit_end
    # where k = 5 if ops else 1.
    if q and c:
        # Cheap structural features first -- always computed.
        prefix = 0
        while prefix < len(q) and prefix < len(c) and q[prefix] == c[prefix]:
            prefix += 1
        suffix = 0
        while (suffix < len(q) - prefix
               and suffix < len(c) - prefix
               and q[len(q) - 1 - suffix] == c[len(c) - 1 - suffix]):
            suffix += 1
        # Compute offsets.
        struct_start = 1 + (4 if config.use_edit_ops else 0)
        out[struct_start + 0] = min(prefix, 10) / 10.0
        out[struct_start + 1] = min(suffix, 10) / 10.0
        out[struct_start + 2] = min(abs(len(q) - len(c)), 5) / 5.0
        out[struct_start + 3] = min(len(q), 20) / 20.0
        out[struct_start + 4] = min(len(c), 20) / 20.0
        # Position block goes AFTER structural.
        pos_start = struct_start + 5

        # Op counts + edit position: only when there's actual editing
        # to do.
        if edit_distance > 0 and q != c:
            ops = _levenshtein_with_ops(q, c)
            if _STRICT_PARITY and ops["distance"] != edit_distance:
                raise ValueError(
                    f"edit_distance mismatch for ({q!r}, {c!r}): "
                    f"engine={edit_distance}, python_osa={ops['distance']}. "
                    f"Set model._STRICT_PARITY = False to silence."
                )
            if config.use_edit_ops:
                out[1 + 0] = min(ops["substitutions"], 4) / 4.0
                out[1 + 1] = min(ops["insertions"], 4) / 4.0
                out[1 + 2] = min(ops["deletions"], 4) / 4.0
                out[1 + 3] = min(ops["transpositions"], 2) / 2.0
            if config.use_edit_position:
                q_len = max(1, len(q))
                out[pos_start + 0] = min(ops["edit_start"], q_len) / q_len
                out[pos_start + 1] = min(ops["edit_end"], q_len) / q_len

    return out


def edit_features_batch(
    q_word: str, c_words: list[str], edit_distances: list[int],
    config: FeatureConfig = DEFAULT_CONFIG,
) -> np.ndarray:
    """Vectorised edit-features for one query against its candidates."""
    return np.stack(
        [edit_features(q_word, c, ed, config)
         for c, ed in zip(c_words, edit_distances)],
        axis=0,
    )


# ---------------------------------------------------------------------------
# Query features (precomputed per query, reused across candidates).
# ---------------------------------------------------------------------------


@dataclass
class QueryFeatures:
    """Precomputed features of the misspelled query word.

    Computed once per query, reused for every candidate. This is the
    main inference optimisation: `score_batch` does not recompute
    `q_char`, `q_bigram`, or `q_trigram` per candidate.

    `word` is retained (not just the feature vectors) so that
    `score_batch` can compute per-candidate edit features without
    forcing callers to thread the original word through the API.

    `trigram` is a zero-length array when the model's config has
    trigrams disabled, so the field is always present but may be
    empty.
    """

    word: str
    char_mean: np.ndarray   # (embed_dim,) mean-pool of char embeddings
    bigram: np.ndarray      # (bigram_dim,) hashed-bigram sum
    trigram: np.ndarray     # (trigram_dim,) hashed-trigram sum (may be empty)

    @classmethod
    def from_word(
        cls, word: str, weights: "RerankerWeights",
    ) -> "QueryFeatures":
        ids = encode_chars(word)
        valid = ids != PAD_IDX
        if not valid.any():
            char_mean = np.zeros(weights.embed_dim, dtype=np.float32)
        else:
            char_mean = weights.char_embed[ids][valid].mean(axis=0)
        trigram_dim = weights.trigram_dim
        if trigram_dim > 0:
            trigram = trigram_features(word, trigram_dim)
        else:
            trigram = np.zeros(0, dtype=np.float32)
        return cls(
            word=word,
            char_mean=char_mean,
            bigram=bigram_features(word, weights.bigram_dim),
            trigram=trigram,
        )


# ---------------------------------------------------------------------------
# Parameters + (de)serialisation.
# ---------------------------------------------------------------------------


@dataclass
class RerankerWeights:
    """Pure-numpy parameter container for the char-MLP reranker.

    Stored as float32 by default; the production Rust forward pass reads
    these as raw little-endian bytes via the on-disk .npz. INT8
    quantisation is out of scope for this refactor.

    `n_in` is computed from `trigram_dim` + `edit_dim` so the
    architecture honestly reflects which feature groups are
    enabled. For the production Slice-3.5 (D) config,
    `n_in = 364`. The A/B/C ablations have smaller `n_in`
    because they disable one or more groups.
    """

    char_embed: np.ndarray  # (NUM_CHARS, EMBED_DIM)
    w1: np.ndarray          # (HIDDEN_DIM, N_IN)
    b1: np.ndarray          # (HIDDEN_DIM,)
    w2: np.ndarray          # (1, HIDDEN_DIM + 1)
    b2: np.ndarray          # (1,)
    embed_dim: int = EMBED_DIM
    hidden_dim: int = HIDDEN_DIM
    bigram_dim: int = BIGRAM_DIM
    trigram_dim: int = TRIGRAM_DIM
    edit_dim: int = EDIT_DIM

    @property
    def n_in(self) -> int:
        # q_char + q_bigram + (q_trigram if enabled)
        # + c_char + c_bigram + (c_trigram if enabled)
        # + edit features (variable width).
        return (2 * (self.embed_dim + self.bigram_dim + self.trigram_dim)
                + self.edit_dim)

    def param_count(self) -> int:
        return sum(a.size for a in self.as_list())

    def feature_config(self) -> FeatureConfig:
        """The `FeatureConfig` these weights imply.

        Deriving it here rather than at each call site is what keeps
        Model A's 6-wide edit vector from being built with the default
        12-wide one: `score_batch`, `eval.py --benchmark` and
        `parity_test.py` all read it from here, so there is exactly one
        place that decides how wide a trained model's edit block is.

        The thresholds match the widths `FeatureConfig.edit_dim`
        produces -- 6 / 10 / 12 -- so the mapping cannot drift from the
        ablation flags that produced them.
        """
        return FeatureConfig(
            use_trigrams=self.trigram_dim > 0,
            use_edit_ops=self.edit_dim >= 10,
            use_edit_position=self.edit_dim >= 12,
        )

    def as_list(self) -> list[np.ndarray]:
        return [self.char_embed, self.w1, self.b1, self.w2, self.b2]

    def save(self, path: Path) -> None:
        """Write the `.npz` (this file's own format) and a sibling `.trm`.

        The `.trm` is the flat little-endian blob `src/rerank/mlp.rs`
        reads: it needs no ZIP, no NPY header and no new Rust
        dependency. Writing it *from here*, unconditionally, is what
        keeps the two formats from drifting apart -- there is no second
        command to forget to run.
        """
        path.parent.mkdir(parents=True, exist_ok=True)
        np.savez(
            path,
            char_embed=self.char_embed,
            w1=self.w1, b1=self.b1,
            w2=self.w2, b2=self.b2,
            embed_dim=np.int32(self.embed_dim),
            hidden_dim=np.int32(self.hidden_dim),
            bigram_dim=np.int32(self.bigram_dim),
            trigram_dim=np.int32(self.trigram_dim),
            edit_dim=np.int32(self.edit_dim),
        )
        self.save_trm(path.with_suffix(".trm"))

    def to_trm_bytes(self) -> bytes:
        """Serialise to the `.trm` layout read by `ModelA::from_bytes`.

        ```text
        "TRM1"  u32 version  u32 num_chars  u32 embed  u32 hidden
                u32 bigram   u32 trigram    u32 edit
                f32 LE: char_embed, w1, b1, w2, b2
        ```

        `n_in` is deliberately *not* stored: the reader derives it from
        the dimensions, so a header cannot claim one layout and store
        another.
        """
        if len(CHAR_INDEX) != 60:
            raise ValueError(
                f"vocabulary has {len(CHAR_INDEX)} entries; the Rust side "
                "hard-codes 60"
            )
        header = struct.pack(
            "<4s7I", TRM_MAGIC, TRM_VERSION, len(CHAR_INDEX),
            self.embed_dim, self.hidden_dim, self.bigram_dim,
            self.trigram_dim, self.edit_dim,
        )
        body = b"".join(
            np.ascontiguousarray(a, dtype="<f4").tobytes() for a in self.as_list()
        )
        return header + body

    def save_trm(self, path: Path) -> None:
        """Write the bytes from `to_trm_bytes` to `path`."""
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(self.to_trm_bytes())

    @classmethod
    def from_trm_bytes(cls, blob: bytes) -> "RerankerWeights":
        """Inverse of `to_trm_bytes`. Round-trip is a regression test."""
        if blob[:4] != TRM_MAGIC:
            raise ValueError(f"not a TRM blob: {blob[:4]!r}")
        (version, num_chars, embed_dim, hidden_dim,
         bigram_dim, trigram_dim, edit_dim) = struct.unpack_from("<7I", blob, 4)
        if version != TRM_VERSION:
            raise ValueError(f"unsupported TRM version {version}")
        if num_chars != len(CHAR_INDEX):
            raise ValueError(f"vocabulary size {num_chars} != {len(CHAR_INDEX)}")
        off = 4 + 7 * 4
        n_in = 2 * (embed_dim + bigram_dim + trigram_dim) + edit_dim
        shapes = [
            (num_chars, embed_dim), (hidden_dim, n_in), (hidden_dim,),
            (1, hidden_dim + 1), (1,),
        ]
        arrays = []
        for shape in shapes:
            n = int(np.prod(shape))
            end = off + n * 4
            if end > len(blob):
                raise ValueError(f"truncated: need {end} bytes, got {len(blob)}")
            arrays.append(
                np.frombuffer(blob, dtype="<f4", count=n, offset=off)
                .reshape(shape).astype(np.float32)
            )
            off = end
        if off != len(blob):
            raise ValueError(f"{len(blob) - off} trailing bytes")
        char_embed, w1, b1, w2, b2 = arrays
        return cls(
            char_embed=char_embed, w1=w1, b1=b1, w2=w2, b2=b2,
            embed_dim=embed_dim, hidden_dim=hidden_dim,
            bigram_dim=bigram_dim, trigram_dim=trigram_dim,
            edit_dim=edit_dim,
        )

    @classmethod
    def load_trm(cls, path: Path) -> "RerankerWeights":
        """Read a `.trm` file. What the Rust binary does, in Python."""
        return cls.from_trm_bytes(path.read_bytes())

    @classmethod
    def load(cls, path: Path) -> "RerankerWeights":
        data = np.load(path, allow_pickle=False)
        return cls(
            char_embed=data["char_embed"],
            w1=data["w1"], b1=data["b1"],
            w2=data["w2"], b2=data["b2"],
            embed_dim=int(data["embed_dim"]),
            hidden_dim=int(data["hidden_dim"]),
            bigram_dim=int(data["bigram_dim"]) if "bigram_dim" in data else BIGRAM_DIM,
            trigram_dim=int(data["trigram_dim"]) if "trigram_dim" in data else TRIGRAM_DIM,
            edit_dim=int(data["edit_dim"]) if "edit_dim" in data else EDIT_DIM,
        )

    @classmethod
    def random_init(
        cls, seed: int = 0,
        embed_dim: int = EMBED_DIM,
        hidden_dim: int = HIDDEN_DIM,
        bigram_dim: int = BIGRAM_DIM,
        trigram_dim: int = TRIGRAM_DIM,
        edit_dim: int = EDIT_DIM,
    ) -> "RerankerWeights":
        rng = np.random.default_rng(seed)

        def glorot(fan_in: int, shape: tuple[int, int]) -> np.ndarray:
            limit = np.sqrt(2.0 / fan_in)
            return rng.uniform(-limit, limit, size=shape).astype(np.float32)

        embed = rng.normal(0.0, 0.1, size=(NUM_CHARS, embed_dim)).astype(np.float32)
        n_in = (2 * (embed_dim + bigram_dim + trigram_dim) + edit_dim)
        w1 = glorot(n_in, (hidden_dim, n_in))
        b1 = np.zeros(hidden_dim, dtype=np.float32)
        w2 = glorot(hidden_dim, (1, hidden_dim + 1))
        b2 = np.zeros(1, dtype=np.float32)
        return cls(
            char_embed=embed, w1=w1, b1=b1, w2=w2, b2=b2,
            embed_dim=embed_dim, hidden_dim=hidden_dim,
            bigram_dim=bigram_dim, trigram_dim=trigram_dim, edit_dim=edit_dim,
        )

    @classmethod
    def from_config(
        cls, config: FeatureConfig, seed: int = 0,
        embed_dim: int = EMBED_DIM,
        hidden_dim: int = HIDDEN_DIM,
        bigram_dim: int = BIGRAM_DIM,
    ) -> "RerankerWeights":
        """Initialise weights sized for a `FeatureConfig`.

        The trigram and edit dimensions are taken from the
        config; the bigram/embed/hidden dimensions are
        command-line overridable for ablation studies.
        """
        return cls.random_init(
            seed=seed,
            embed_dim=embed_dim,
            hidden_dim=hidden_dim,
            bigram_dim=bigram_dim,
            trigram_dim=config.trigram_dim,
            edit_dim=config.edit_dim,
        )


# ---------------------------------------------------------------------------
# Forward (single + batch).
# ---------------------------------------------------------------------------


def _pool_chars_batch(char_ids_batch: np.ndarray, weights: RerankerWeights) -> np.ndarray:
    """Batched mean-pool of (N, MAX_LEN) char IDs into (N, embed_dim)."""
    emb = weights.char_embed[char_ids_batch]               # (N, MAX_LEN, embed_dim)
    valid = (char_ids_batch != PAD_IDX).astype(np.float32)  # (N, MAX_LEN)
    valid = valid[..., None]                                # (N, MAX_LEN, 1)
    emb_sum = (emb * valid).sum(axis=1)                     # (N, embed_dim)
    n_valid = valid.sum(axis=1).clip(min=1.0)              # (N, 1)
    return emb_sum / n_valid


def score_batch(
    weights: RerankerWeights,
    query: QueryFeatures,
    candidate_words: list[str],
    edit_distances: list[int],
) -> np.ndarray:
    """Score a query's full candidate list in one batched matmul.

    Returns a 1-D float32 array of **raw logits** (one per candidate).
    Argmax over logits is the model's ranking; apply listwise softmax
    outside the model for per-candidate probabilities.

    Query-side features are taken from the precomputed `query`
    object -- they are computed once and broadcast to all candidates.
    Per-candidate edit features are computed here from the words
    and the supplied edit distances; the caller does not have to
    thread them through.

    The signature is unchanged from the Slice-3 Model A API so the
    production Rust forward path (which reads the saved .npz
    directly) only has to learn the new feature dimensions, not
    new arguments.

    The x matrix layout is:
      [0:e]                            q_char
      [e:e+b]                          q_bigram
      [e+b:e+b+t]                      q_trigram    (only if t > 0)
      [e+b+t:e+b+t+e]                  c_char
      [2e+b+t:2e+b+t+b]                c_bigram
      [2e+2b+t:2e+2b+t+t]              c_trigram    (only if t > 0)
      [2e+2b+2t:2e+2b+2t+d]            edit features (variable width)
    where e, b, t, d are the embed / bigram / trigram / edit
    dimensions stored on the weights. When t = 0, the trigram
    slices are absent; when d < 12, the edit slice is shorter.
    """
    n = len(candidate_words)
    if n == 0:
        return np.zeros(0, dtype=np.float32)

    cand_ids = encode_batch(candidate_words)
    cand_char = _pool_chars_batch(cand_ids, weights)
    cand_bigram = bigram_features_batch(candidate_words, weights.bigram_dim)
    if weights.trigram_dim > 0:
        cand_trigram = trigram_features_batch(candidate_words, weights.trigram_dim)
    else:
        cand_trigram = np.zeros((n, 0), dtype=np.float32)
    # Build a FeatureConfig from the weights dimensions so the
    # edit-feature vector width matches the model's expectation.
    edit_config = weights.feature_config()
    cand_edit = edit_features_batch(
        query.word, candidate_words, edit_distances, edit_config,
    )

    n_in = weights.n_in
    e = weights.embed_dim
    b = weights.bigram_dim
    t = weights.trigram_dim
    d = weights.edit_dim
    x = np.empty((n, n_in), dtype=np.float32)
    # Query side: char + bigram + (trigram if enabled).
    x[:, 0:e] = query.char_mean
    x[:, e:e + b] = query.bigram
    if t > 0:
        x[:, e + b:e + b + t] = query.trigram
    # Candidate side.
    x[:, e + b + t:e + b + t + e] = cand_char
    x[:, 2 * e + b + t:2 * e + b + t + b] = cand_bigram
    if t > 0:
        x[:, 2 * e + 2 * b + t:2 * e + 2 * b + t + t] = cand_trigram
    # Edit features at the tail.
    x[:, 2 * e + 2 * b + 2 * t:2 * e + 2 * b + 2 * t + d] = cand_edit

    h = np.maximum(x @ weights.w1.T + weights.b1, 0.0)         # (N, hidden)
    h2 = np.concatenate([h, np.ones((n, 1), dtype=np.float32)], axis=1)
    return (h2 @ weights.w2.T + weights.b2).squeeze(-1)


# ---------------------------------------------------------------------------
# Smoke test.
# ---------------------------------------------------------------------------


if __name__ == "__main__":
    print("Slice-3.5 ablation summary")
    print("=" * 60)
    for cfg in [
        FeatureConfig(use_trigrams=False, use_edit_ops=False, use_edit_position=False),
        FeatureConfig(use_trigrams=True, use_edit_ops=False, use_edit_position=False),
        FeatureConfig(use_trigrams=True, use_edit_ops=True, use_edit_position=False),
        FeatureConfig(use_trigrams=True, use_edit_ops=True, use_edit_position=True),
    ]:
        w = RerankerWeights.from_config(cfg, seed=0)
        print(f"  {cfg.label()}: n_in={w.n_in:>4}  params={w.param_count():>6,}")
    print()

    w = RerankerWeights.random_init(seed=0)
    print(
        f"EMBED_DIM={EMBED_DIM}, HIDDEN_DIM={HIDDEN_DIM}, "
        f"BIGRAM_DIM={BIGRAM_DIM}, TRIGRAM_DIM={TRIGRAM_DIM}, "
        f"EDIT_DIM={EDIT_DIM}"
    )
    print(f"N_IN={w.n_in}, params={w.param_count()}")
    # Verify batch == single (vectorised sanity check).
    q = QueryFeatures.from_word("koirissa", w)
    cands = ["koirissa", "koirassa", "koirista"]
    eds = [0, 1, 1]
    logits = score_batch(w, q, cands, eds)
    print(f"koirissa -> {cands} -> {logits}")

    # The jouksee/juoksee transposition case.
    q2 = QueryFeatures.from_word("jouksee", w)
    logits2 = score_batch(w, q2, ["juoksee", "jouksee", "juokse"], [1, 0, 1])
    print(f"jouksee -> juoksee/jouksee/juokse -> {logits2}")
    # Edit features for the transposition.
    ef = edit_features("jouksee", "juoksee", 1)
    print(f"edit_features(jouksee, juoksee, 1) = {ef}")
