# SPDX-License-Identifier: MIT OR Apache-2.0
"""Golden fixture for Python / Rust feature parity.

The Slice-3.5 reranker is trained in Python (this directory) and
inferred in Rust (`src/rerank/mlp.rs`, which reads the flat `.trm`
weight blob this script writes alongside the `.npz`). The two
implementations must produce identical feature vectors, logits and
rankings for the same (query, candidate, weights) input. If they
don't, the trained model silently misbehaves at inference time and
the bug is very hard to localise.

This module is the source of truth for what the Rust side must
reproduce. Running it dumps a JSON file with every intermediate
value for a small, hand-picked set of (q, c) pairs that exercise
each feature:

  - identity               (`kissa` -> `kissa`)
  - single substitution    (`kissa` -> `kisaa`)
  - single insertion       (`kissa` -> `kissaa`)
  - single deletion        (`kissa` -> `kisa`)
  - adjacent transposition (`jouksee` -> `juoksee`)
  - unknown character      (`juoks?ee` -> `juoksee`)
  - length mismatch        (`koira` -> `koiralle`)
  - ascii vs unicode       (`talo` -> `tälo`)

The Rust forward path (`ModelA` in `src/rerank/mlp.rs`, which loads
the `.trm` blob and produces a score) re-implements every function
in `model.py` that touches the feature vector:

  - `encode_chars`              (char embedding mean-pool)
  - `bigram_features`           (sum-pooled hashed bigrams)
  - `trigram_features`          (sum-pooled hashed trigrams)
  - `_boundary_tokens`          (the `<`, `>` boundary markers)
  - `_UNK_GLYPH` = `"~"`        (single-char placeholder for unknown)
  - `_bigram_hash`              (`ord(a) * 257 + ord(b) % dim`)
  - `_trigram_hash`             (`ord(a) * 65537 + ord(b) * 257 + ord(c) % dim`)
  - `edit_features`             (the 12-dim edit-alignment vector)
  - `_levenshtein_with_ops`     (OSA with transposition rule)
  - The x matrix layout in `score_batch` (q_char, q_bigram, q_trigram,
    c_char, c_bigram, c_trigram, edit_features)
  - The two MLP matmuls + ReLU

For each case we dump:
  - the input words
  - the per-block features (char_ids, bigram indices, trigram
    indices, edit features)
  - the concatenated feature vector x
  - the pre-activation h_pre, post-activation h, and the final logit

For each whole candidate list ("ranking") we dump the raw logits,
the full argmax order and the winning index, because ranking is the
product and a tie broken the wrong way silently flips a candidate.

`tests/rerank_parity.rs` loads this JSON plus the `.trm` weights and
asserts every one of those values, then asserts that Rust's argmax
order equals Python's.

Usage:
    # Seeded random Model A -- regenerable from any checkout, no
    # training run needed. This is what the committed fixture is.
    python3 tools/train/parity_test.py \\
        --output  tests/fixtures/rerank_parity.json

    # Or against real trained weights.
    python3 tools/train/parity_test.py \\
        --weights /tmp/torille_reranker.npz \\
        --output  /tmp/torille_parity.json

Both write `tests/fixtures/rerank_parity.trm` (or `--trm PATH`)
next to the JSON, so the fixture and the weights can never describe
different models. If the feature pipeline ever changes in Python,
regenerate the fixture and update `src/rerank/mlp.rs` to match.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Iterable

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from model import (  # noqa: E402
    PAD_IDX, UNK_IDX, FeatureConfig, QueryFeatures, RerankerWeights,
    _bigram_hash, _boundary_tokens, _levenshtein_with_ops, _trigram_hash,
    _UNK_GLYPH, bigram_features, edit_features, encode_chars,
    score_batch, trigram_features,
)


def config_for(weights: RerankerWeights) -> FeatureConfig:
    """The `FeatureConfig` a set of weights implies.

    Thin wrapper over `RerankerWeights.feature_config`, which is also
    what `score_batch` uses; going through it here is what stops the
    fixture from dumping a 12-dim edit vector for a 6-dim Model A and
    having every downstream comparison be garbage. (This is the bug
    `eval.py --benchmark` used to hit.)
    """
    return weights.feature_config()


# Hand-picked cases. Each entry is (query, candidate, edit_distance)
# -- the `edit_distance` is the engine's value as it would appear in
# the rerank JSONL (i.e. standard Levenshtein from the Rust engine,
# not OSA from this Python module). The Python test still records
# what `_levenshtein_with_ops` produces, so the two can be compared.
PARITY_CASES: list[tuple[str, str, int]] = [
    ("kissa", "kissa", 0),       # identity
    ("kissa", "kisaa", 1),       # substitution
    ("kissa", "kissaa", 1),      # insertion
    ("kissa", "kisa", 1),        # deletion
    ("jouksee", "juoksee", 1),   # adjacent transposition
    ("juoksee", "jouksee", 2),   # reverse of above (deletion of `j` + ...)
    ("juoks?ee", "juoksee", 1),  # unknown char (sub `?` -> `o`)
    ("koira", "koiralle", 3),    # length mismatch (suffix added)
    ("talo", "tälo", 1),         # ASCII vs unicode vowel
    ("aakkonen", "aakkoset", 2), # shared 5-char prefix, divergence late
    ("juoksee", "juoksee", 0),   # identity on the long word
    ("y", "y", 0),               # single character
    ("", "talo", 4),             # empty query
    ("talo", "", 4),             # empty candidate
    ("ääää", "yxyx", 4),         # non-ASCII, all slots consumed
]

# Whole-list rankings. The Rust side must reproduce not just the logits
# but the *argmax index* -- ranking is the product, and a tie broken the
# wrong way silently flips a candidate.
#
# Each entry is (query, [(candidate, edit_distance), ...]).
RANKING_CASES: list[tuple[str, list[tuple[str, int]]]] = [
    ("juoksee", [("juoksee", 0), ("juokse", 1), ("juoksen", 1),
                 ("juokseeen", 1), ("kissa", 6)]),
    ("kissa", [("kissa", 0), ("kisaa", 1), ("kisa", 1),
               ("kissaa", 1), ("koira", 4)]),
    # Two candidates that differ only in a transposition: the case
    # where a tie broken differently would flip the ranking.
    ("jouksee", [("juoksee", 1), ("jouksee", 0), ("juokseee", 1)]),
    # A single-candidate list, and one whose first element wins outright.
    ("talo", [("talo", 0)]),
    ("tälo", [("talo", 1), ("tälo", 0), ("tällöin", 4)]),
]

# N-gram hash parity.
#
# Production Model A has `trigram_dim == 0`, so the model fixture on its
# own never exercises the trigram hash at all -- a bug there would pass
# every other check. These compare the hash *functions* directly, at
# several widths, with full vectors (they are integer counts, so the two
# sides must agree exactly rather than approximately).
NGRAM_WORDS = ["", "a", "y", "kissa", "juoksee", "juoks€ee",
               "tälo", "a-b", "'-'", "ääää", "<x", "a~b"]
NGRAM_DIMS = [1, 4, 16, 64]


def _nonzero_dict(vec: np.ndarray, max_entries: int = 16) -> dict:
    """Compact representation of a sparse-ish vector.

    Returns `{i: float(v)` for the `max_entries` largest entries.
    For dense feature vectors (e.g. the (n_in,) input `x`) we
    capture every entry but as a 4-decimal float string to keep
    the JSON file readable.
    """
    flat = np.asarray(vec, dtype=np.float32).ravel()
    if flat.size > max_entries:
        idx = np.argsort(-np.abs(flat))[:max_entries]
    else:
        idx = np.arange(flat.size)
    return {int(i): round(float(flat[i]), 6) for i in idx}


def _feature_dumps(word: str, weights: RerankerWeights) -> dict:
    """Dump every per-word feature for `word` at the given weights."""
    ids = encode_chars(word)
    valid_mask = (ids != PAD_IDX).astype(np.int8).tolist()
    ids_list = ids.tolist()
    # Bigram and trigram indices with their hashes, for the Rust
    # side to reproduce.
    chars = _boundary_tokens(word)
    bigram_idx = [
        (i, _bigram_hash(chars[i], chars[i + 1], weights.bigram_dim))
        for i in range(max(0, len(chars) - 1))
    ] if weights.bigram_dim else []
    trigram_idx = [
        (i, _trigram_hash(chars[i], chars[i + 1], chars[i + 2], weights.trigram_dim))
        for i in range(max(0, len(chars) - 2))
    ] if weights.trigram_dim else []
    char_mean = np.zeros(weights.embed_dim, dtype=np.float32)
    if any(v == 1 for v in valid_mask):
        char_mean = weights.char_embed[ids][np.array(valid_mask, dtype=bool)].mean(axis=0)

    return {
        "word": word,
        "boundary_tokens": chars,
        "char_ids": ids_list,
        "char_valid_mask": valid_mask,
        "char_mean_nonzero": _nonzero_dict(char_mean, max_entries=8),
        "bigram_indices": bigram_idx,
        "trigram_indices": trigram_idx,
        "bigram_nonzero": _nonzero_dict(bigram_features(word, weights.bigram_dim)),
        "trigram_nonzero": _nonzero_dict(trigram_features(word, weights.trigram_dim)),
    }


def _case_dump(query: str, candidate: str, ed: int,
               weights: RerankerWeights) -> dict:
    """Dump everything for one (q, c) case at the given weights."""
    cfg = config_for(weights)
    qf = QueryFeatures.from_word(query, weights)
    edit_vec = edit_features(query, candidate, ed, cfg)
    python_osa = _levenshtein_with_ops(query, candidate)
    # Build the full x matrix the same way score_batch does, so the
    # Rust reference can read the slice-by-slice layout.
    n_in = weights.n_in
    e = weights.embed_dim
    b = weights.bigram_dim
    t = weights.trigram_dim
    d = weights.edit_dim
    cand_char = np.zeros((1, e), dtype=np.float32)
    cand_ids = encode_chars(candidate)
    valid = cand_ids != PAD_IDX
    if valid.any():
        cand_char[0] = weights.char_embed[cand_ids][valid].mean(axis=0)
    # NB: these take a *str*, not a list. Passing `[candidate]` made
    # `_boundary_tokens` see one element equal to the whole word, so
    # the dumped x vector hashed `['<', '~', '>']` instead of the real
    # bigrams -- silently wrong for every trigram model.
    cand_bigram = bigram_features(candidate, b)
    cand_trigram = trigram_features(candidate, t)
    x = np.zeros((1, n_in), dtype=np.float32)
    x[0, 0:e] = qf.char_mean
    x[0, e:e + b] = qf.bigram
    x[0, e + b:e + b + t] = qf.trigram
    x[0, e + b + t:e + b + t + e] = cand_char[0]
    x[0, 2 * e + b + t:2 * e + b + t + b] = cand_bigram
    x[0, 2 * e + 2 * b + t:2 * e + 2 * b + t + t] = cand_trigram
    x[0, 2 * e + 2 * b + 2 * t:2 * e + 2 * b + 2 * t + d] = edit_vec

    # MLP forward.
    h_pre = x @ weights.w1.T + weights.b1
    h = np.maximum(h_pre, 0.0)
    h2 = np.concatenate([h, np.ones((1, 1), dtype=np.float32)], axis=1)
    logit = float((h2 @ weights.w2.T + weights.b2).squeeze(-1)[0])

    # The production entry point must agree with the hand-expanded
    # forward above. If it doesn't, the fixture would be documenting a
    # computation that `score_batch` never actually performs.
    production = float(score_batch(weights, qf, [candidate], [ed])[0])
    if abs(production - logit) > 1e-4:
        raise AssertionError(
            f"score_batch({query!r},{candidate!r}) = {production}, "
            f"manual forward = {logit}"
        )

    return {
        "query": query,
        "candidate": candidate,
        "engine_edit_distance": ed,
        "python_osa": python_osa,
        "edit_features": edit_vec.tolist(),
        "query_features": _feature_dumps(query, weights),
        "candidate_features": _feature_dumps(candidate, weights),
        "x_vector": x[0].tolist(),
        "h_pre_nonzero": _nonzero_dict(h_pre[0], max_entries=8),
        "h_nonzero": _nonzero_dict(h[0], max_entries=8),
        "logit": logit,
    }


def _ranking_dump(query: str, pairs: list[tuple[str, int]],
                  weights: RerankerWeights) -> dict:
    """Score a whole candidate list and record the resulting order.

    Both sides record the same three things: the raw logits in input
    order, the candidate order from `argsort(-logit, kind="stable")`,
    and the argmax index. `kind="stable"` is what makes the Python
    tie-break "first maximum wins", which is also what Rust's
    `ModelA::argmax` does -- if either side breaks ties differently the
    rankings drift apart on exactly the rows where two candidates score
    alike.
    """
    words = [c for c, _ in pairs]
    eds = [e for _, e in pairs]
    qf = QueryFeatures.from_word(query, weights)
    logits = [float(v) for v in score_batch(weights, qf, words, eds)]
    if len(logits) != len(words):
        raise AssertionError(f"score_batch returned {len(logits)} for {len(words)}")
    order = sorted(range(len(words)), key=lambda i: (-logits[i], i))
    argmax = order[0]
    return {
        "query": query,
        "candidates": words,
        "edit_distances": eds,
        "logits": logits,
        "order": order,
        "argmax": argmax,
    }


def _edit_index_doc(weights: RerankerWeights) -> dict[str, str]:
    """The meaning of each slot of `edit_features` at this width.

    Written out rather than assumed, because the layout genuinely
    moves: with op counts off, the structural block starts at 1; with
    them on, at 5; the position block always trails the structural one.
    """
    cfg = config_for(weights)
    d = weights.edit_dim
    labels: dict[str, str] = {"0": "edit_distance / 4"}
    struct = 1 + (4 if cfg.use_edit_ops else 0)
    if cfg.use_edit_ops:
        labels["1"] = "substitutions / 4"
        labels["2"] = "insertions / 4"
        labels["3"] = "deletions / 4"
        labels["4"] = "transpositions / 2"
    for i, name in enumerate(["prefix / 10", "suffix / 10",
                              "|len(q) - len(c)| / 5",
                              "len(q) / 20", "len(c) / 20"]):
        labels[str(struct + i)] = name
    if cfg.use_edit_position:
        labels[str(struct + 5)] = "edit_start / max(1, len(q))"
        labels[str(struct + 6)] = "edit_end / max(1, len(q))"
    if len(labels) != d:
        raise AssertionError(
            f"described {len(labels)} slots but edit_dim={d}: {sorted(labels)}"
        )
    return dict(sorted(labels.items(), key=lambda kv: int(kv[0])))


def main(argv: Iterable[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        description="Dump a parity-test fixture for the Rust reranker.",
    )
    p.add_argument("--weights", type=Path, default=None,
                   help="Trained .npz weights. Omit to build a seeded "
                        "random Model A instead, which makes the fixture "
                        "regenerable without a training run.")
    p.add_argument("--seed", type=int, default=0,
                   help="Seed for the random-init fallback (default 0).")
    p.add_argument("--output", type=Path, required=True,
                   help="Where to write the JSON fixture.")
    p.add_argument("--trm", type=Path, default=None,
                   help="Where to write the .trm weights blob the Rust "
                        "test loads. Defaults to the .npz path with a "
                        ".trm suffix.")
    args = p.parse_args(argv)

    if args.weights is not None:
        weights = RerankerWeights.load(args.weights)
        origin = str(args.weights)
    else:
        # Production Model A dimensions, seeded: a fixture that any
        # checkout can rebuild with one command and that exercises the
        # real arithmetic (47k parameters, 326-wide input) rather than a
        # toy size.
        weights = RerankerWeights.from_config(
            FeatureConfig(use_trigrams=False, use_edit_ops=False,
                          use_edit_position=False),
            seed=args.seed, embed_dim=96, hidden_dim=128, bigram_dim=64,
        )
        origin = f"random_init(seed={args.seed}), production Model A dims"
    fixture = {
        "schema_version": 1,
        "architecture": {
            "embed_dim": weights.embed_dim,
            "hidden_dim": weights.hidden_dim,
            "bigram_dim": weights.bigram_dim,
            "trigram_dim": weights.trigram_dim,
            "edit_dim": weights.edit_dim,
            "n_in": weights.n_in,
            "param_count": weights.param_count(),
            "weights_origin": origin,
        },
        "constants": {
            "pad_idx": PAD_IDX,
            "unk_idx": UNK_IDX,
            "bound_left": "<",
            "bound_right": ">",
            "unk_glyph": _UNK_GLYPH,
            "bigram_hash_formula": "(ord(a) * 257 + ord(b)) % dim",
            "trigram_hash_formula": "(ord(a) * 65537 + ord(b) * 257 + ord(c)) % dim",
        },
        "feature_layout": {
            "x_slice": [
                {"name": "q_char",  "offset": 0,
                 "size": "embed_dim"},
                {"name": "q_bigram", "offset": "embed_dim",
                 "size": "bigram_dim"},
                {"name": "q_trigram", "offset": "embed_dim + bigram_dim",
                 "size": "trigram_dim"},
                {"name": "c_char",  "offset": "embed_dim + bigram_dim + trigram_dim",
                 "size": "embed_dim"},
                {"name": "c_bigram", "offset": "2*embed_dim + bigram_dim + trigram_dim",
                 "size": "bigram_dim"},
                {"name": "c_trigram", "offset": "2*embed_dim + 2*bigram_dim + trigram_dim",
                 "size": "trigram_dim"},
                {"name": "edit_features", "offset": "2*embed_dim + 2*bigram_dim + 2*trigram_dim",
                 "size": "edit_dim"},
            ],
        },
        # Derived from config_for(weights): ops start at index 1 only
        # when edit_dim >= 10, and the structural block shifts right by
        # four to make room. Position goes *after* the structural block.
        "edit_feature_indices": _edit_index_doc(weights),
        "cases": [
            _case_dump(q, c, ed, weights) for q, c, ed in PARITY_CASES
        ],
        "rankings": [
            _ranking_dump(q, pairs, weights) for q, pairs in RANKING_CASES
        ],
        "ngram_parity": [
            {
                "word": w,
                "boundary_tokens": _boundary_tokens(w),
                "dim": d,
                "bigram": bigram_features(w, d).tolist(),
                "trigram": trigram_features(w, d).tolist(),
            }
            for w in NGRAM_WORDS for d in NGRAM_DIMS
        ],
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(fixture, indent=2, ensure_ascii=False))

    # The Rust test reads this blob; writing it here means the fixture
    # JSON and the weights can never describe different models.
    if args.trm is not None:
        trm_path = args.trm
    elif args.weights is not None:
        trm_path = args.weights.with_suffix(".trm")
    else:
        trm_path = args.output.with_suffix(".trm")
    weights.save_trm(trm_path)

    print(f"parity fixture -> {args.output}")
    print(f"  weights: {origin}")
    print(f"  weights (.trm) -> {trm_path} ({trm_path.stat().st_size} bytes)")
    print(f"  cases: {len(fixture['cases'])}")
    print(f"  rankings: {len(fixture['rankings'])} "
          f"({sum(len(r['candidates']) for r in fixture['rankings'])} scored)")
    print(f"  ngram pairs: {len(fixture['ngram_parity'])}")
    print(f"  param count: {fixture['architecture']['param_count']}")
    print(f"  n_in: {fixture['architecture']['n_in']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
