# SPDX-License-Identifier: MIT OR Apache-2.0
"""Train the production char-MLP reranker (Slice 3.5).

Pure-numpy training (no PyTorch dependency). Production model:
chars + hashed bigrams + hashed trigrams + 12-dim edit/alignment
features, ~38k params. The edit features are the Slice-3.5
addition; they give the model an explicit view of the
transposition/substitution/insertion/deletion counts between the
misspelled and candidate words, which is what lets it learn
`jouksee -> juoksee` as a single transformation rather than as
two independent edits.

Loss: listwise softmax over each query's candidate list.

Usage:
    python3 train.py \
        --train data/rerank/train.jsonl \
        --dev data/rerank/dev.jsonl \
        --epochs 20 \
        --output /tmp/torille_reranker.npz \
        --seed 0
"""

from __future__ import annotations

import argparse
import json
import math
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from model import (  # noqa: E402
    BIGRAM_DIM, DEFAULT_CONFIG, EMBED_DIM, HIDDEN_DIM, MAX_LEN,
    PAD_IDX, FeatureConfig, RerankerWeights,
    bigram_features_batch, edit_features_batch, encode_batch, encode_chars,
    trigram_features_batch,
)


# =============================================================================
#   Data loading.
# =============================================================================


def load_jsonl(path: Path, max_rows: int = 0) -> list[dict]:
    """Read one rerank-data JSONL into a list of row dicts."""
    rows: list[dict] = []
    skipped = 0
    with path.open() as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                skipped += 1
                continue
            if max_rows > 0 and len(rows) >= max_rows:
                break
    if skipped:
        print(f"  ({skipped} malformed line(s) skipped)", flush=True)
    return rows


def rows_to_arrays(rows: list[dict], config: FeatureConfig = DEFAULT_CONFIG) -> tuple:
    """Flatten rerank-data rows into per-(row, candidate) entries.

    Returns:
      c_ids        : (N, MAX_LEN) int64 candidate char IDs
      edit_arr     : (N, config.edit_dim) float32 edit/alignment features
      is_correct   : (N,) int64, 1 iff this entry is the positive
      row_idx      : (N,) int64, source-row index
      ms_words     : list[str] misspelled words (one per entry)
      c_words      : list[str] candidate words

    Edit features are computed per (misspelled, candidate) pair on
    the fly; the JSONL only carries `edit_distance`, so the per-op
    counts and prefix/suffix/length features have to be derived.
    For the 3,000-row training set this is fast enough (O(N * L^2)
    over short strings).
    """
    flat_c: list[str] = []
    flat_ed: list[int] = []
    flat_correct: list[int] = []
    flat_row_idx: list[int] = []
    flat_ms: list[str] = []

    for src_row_idx, row in enumerate(rows):
        ci = row.get("correct_idx", -1)
        if ci < 0:
            continue
        candidates = row["candidates"]
        for k, cand in enumerate(candidates):
            flat_ms.append(row["misspelled"])
            flat_c.append(cand["word"])
            flat_ed.append(min(int(cand.get("edit_distance", 0)), 4))
            flat_correct.append(int(k == ci))
            flat_row_idx.append(src_row_idx)

    if not flat_c:
        zero = np.zeros(0, dtype=np.int64)
        zero_arr = np.zeros((0, MAX_LEN), dtype=np.int64)
        # NOTE: the actual edit_dim is determined by the ablation
        # config in the caller; we use a placeholder 0 here.
        return zero_arr, np.zeros((0, 0), dtype=np.float32), zero, zero, [], []

    # Group by source row so we can compute edit features per
    # (query, candidate_list) once instead of in a Python loop.
    c_ids = encode_batch(flat_c)
    edit_arr = np.zeros((len(flat_c), config.edit_dim), dtype=np.float32)
    row_to_word: dict[int, str] = {}
    row_to_indices: dict[int, list[int]] = {}
    for i, (src, ed) in enumerate(zip(flat_row_idx, flat_ed)):
        if int(src) not in row_to_word:
            row_to_word[int(src)] = flat_ms[i]
            row_to_indices[int(src)] = []
        row_to_indices[int(src)].append(i)
    for src, idxs in row_to_indices.items():
        q_word = row_to_word[src]
        c_words = [flat_c[i] for i in idxs]
        eds = [flat_ed[i] for i in idxs]
        edit_arr[idxs] = edit_features_batch(q_word, c_words, eds, config)

    return (
        c_ids,
        edit_arr,
        np.array(flat_correct, dtype=np.int64),
        np.array(flat_row_idx, dtype=np.int64),
        flat_ms,
        flat_c,
    )


# =============================================================================
#   Forward / backward.
# =============================================================================


def _pool_chars(char_ids: np.ndarray, weights: RerankerWeights) -> np.ndarray:
    """Mean-pool of (N, MAX_LEN) char IDs into (N, embed_dim)."""
    emb = weights.char_embed[char_ids]
    valid = (char_ids != PAD_IDX).astype(np.float32)
    valid = valid[..., None]
    emb_sum = (emb * valid).sum(axis=1)
    n_valid = valid.sum(axis=1).clip(min=1.0)
    return emb_sum / n_valid


def forward_all(
    weights: RerankerWeights,
    c_ids: np.ndarray,
    edit_arr: np.ndarray,
    q_char: np.ndarray,
    q_bigram: np.ndarray,
    q_trigram: np.ndarray,
    c_words: list[str],
    row_idx: np.ndarray,
):
    """Forward pass: query features are precomputed (broadcast), only
    candidate-side features are computed per batch.

    The query-side feature arrays are indexed by `row_idx` to expand
    them from (n_rows, *) to (N, *) -- one query feature row per
    candidate entry. `edit_arr` is already (N, edit_dim) by
    construction in `rows_to_arrays`.

    Layout of the (N, n_in) feature matrix `x`:
      [0:e]                          q_char[row_idx]
      [e:e+b]                        q_bigram[row_idx]
      [e+b:e+b+t]                    q_trigram[row_idx]
      [e+b+t:e+b+t+e]                c_char
      [2e+b+t:2e+b+t+b]              c_bigram
      [2e+2b+t:2e+2b+t+t]            c_trigram
      [2e+2b+2t:2e+2b+2t+d]          edit features
    """
    n = c_ids.shape[0]
    c_char = _pool_chars(c_ids, weights)
    c_bigram = bigram_features_batch(c_words, weights.bigram_dim)
    c_trigram = trigram_features_batch(c_words, weights.trigram_dim)

    n_in = weights.n_in
    e = weights.embed_dim
    b = weights.bigram_dim
    t = weights.trigram_dim
    d = weights.edit_dim
    x = np.empty((n, n_in), dtype=np.float32)
    # Index the per-row query features by row_idx so each candidate
    # entry gets the query features of its source row. With n_rows
    # distinct queries and N = sum(candidates) candidate entries,
    # the LHS is (N, *) and the RHS is (n_rows, *); numpy's
    # advanced indexing expands to (N, *) via row_idx.
    x[:, 0:e] = q_char[row_idx]
    x[:, e:e + b] = q_bigram[row_idx]
    if t > 0:
        x[:, e + b:e + b + t] = q_trigram[row_idx]
    x[:, e + b + t:e + b + t + e] = c_char
    x[:, 2 * e + b + t:2 * e + b + t + b] = c_bigram
    if t > 0:
        x[:, 2 * e + 2 * b + t:2 * e + 2 * b + t + t] = c_trigram
    x[:, 2 * e + 2 * b + 2 * t:2 * e + 2 * b + 2 * t + d] = edit_arr

    h_pre = x @ weights.w1.T + weights.b1
    h = np.maximum(h_pre, 0.0)
    h2 = np.concatenate([h, np.ones((n, 1), dtype=np.float32)], axis=1)
    logits = (h2 @ weights.w2.T + weights.b2).squeeze(-1)
    cache = dict(x=x, h_pre=h_pre, h=h, h2=h2, logits=logits)
    return logits, cache


def listwise_softmax_loss_and_grad(
    weights: RerankerWeights,
    c_ids: np.ndarray,
    edit_arr: np.ndarray,
    is_correct: np.ndarray,
    row_idx: np.ndarray,
    q_char: np.ndarray,
    q_bigram: np.ndarray,
    q_trigram: np.ndarray,
    c_words: list[str],
) -> tuple[float, dict[str, np.ndarray]]:
    """Listwise softmax loss across each query's candidate list.

    Each query contributes exactly one term, regardless of how many
    candidates it has.
    """
    logits, cache = forward_all(
        weights, c_ids, edit_arr, q_char, q_bigram, q_trigram, c_words,
        row_idx,
    )
    n_rows = int(row_idx.max()) + 1 if row_idx.size else 0
    loss_sum = 0.0
    d_logits = np.zeros_like(logits)

    for r in range(n_rows):
        idxs = np.where(row_idx == r)[0]
        if not idxs.size:
            continue
        is_corr = is_correct[idxs]
        if is_corr.sum() != 1:
            continue
        correct_local = int(np.argmax(is_corr))
        row_logits = logits[idxs]
        row_logits_shift = row_logits - row_logits.max()
        exp_x = np.exp(row_logits_shift)
        softmax = exp_x / exp_x.sum()
        loss_sum += -math.log(max(softmax[correct_local], 1e-12))
        d_row = softmax.copy()
        d_row[correct_local] -= 1.0
        d_logits[idxs] = d_row

    mean_loss = loss_sum / max(1, n_rows)
    d_logits = d_logits / max(1, n_rows)

    grads = _backward_from_dlogits(weights, cache, d_logits)
    return mean_loss, grads


def _backward_from_dlogits(weights: RerankerWeights, cache: dict,
                           d_logits: np.ndarray) -> dict[str, np.ndarray]:
    """Backward pass: gradients of the loss w.r.t. parameters."""
    h2 = cache["h2"]
    d_w2 = (d_logits[:, None] * h2).sum(axis=0, keepdims=True)
    d_b2 = d_logits.sum(axis=0, keepdims=True)
    d_h2 = d_logits[:, None] * weights.w2

    d_h = d_h2[:, :weights.hidden_dim]
    h_pre = cache["h_pre"]
    relu_mask = (h_pre > 0).astype(np.float32)
    d_h_pre = d_h * relu_mask
    x = cache["x"]
    d_w1 = d_h_pre.T @ x
    d_b1 = d_h_pre.sum(axis=0)
    d_x = d_h_pre @ weights.w1

    # Split d_x back into the seven feature blocks; only c_char flows
    # to char_embed (the other six are inputs, not parameters).
    e = weights.embed_dim
    b = weights.bigram_dim
    t = weights.trigram_dim
    d_c_char = d_x[:, e + b + t:e + b + t + e]

    # Mean-pool backward: same logic as the production forward.
    def grad_embed(char_ids: np.ndarray, d_pooled: np.ndarray) -> np.ndarray:
        embed_dim = weights.embed_dim
        valid = (char_ids != PAD_IDX).astype(np.float32)
        count = valid.sum(axis=1).clip(min=1.0)
        d_embed = np.zeros_like(weights.char_embed)
        d_per_sample = d_pooled / count[:, None]
        flat_ids = char_ids.ravel()
        flat_grads = (d_per_sample[:, None, :] * valid[..., None]).reshape(-1, embed_dim)
        np.add.at(d_embed, flat_ids, flat_grads)
        return d_embed

    # `c_ids` is the candidate char_ids passed to forward_all; the
    # production QueryFeatures API lets us reuse the same backward
    # by passing c_ids here.
    d_embed = grad_embed(_c_ids_for_backward[0], d_c_char)

    return dict(
        char_embed=d_embed,
        w1=d_w1, b1=d_b1,
        w2=d_w2, b2=d_b2,
    )


# Cache the latest candidate char_ids so the backward pass can find
# them. The forward pass writes this; the backward reads it. A
# little ugly but keeps `listwise_softmax_loss_and_grad` stateless
# across calls without forcing a closure.
_c_ids_for_backward: list[np.ndarray] = [None]  # type: ignore[list-item]


# =============================================================================
#   Adam optimizer.
# =============================================================================


class Adam:
    def __init__(self, params, lr=1e-3, beta1=0.9, beta2=0.999, eps=1e-8):
        self.lr = lr
        self.beta1, self.beta2, self.eps = beta1, beta2, eps
        self.m = [np.zeros_like(p) for p in params]
        self.v = [np.zeros_like(p) for p in params]
        self.t = 0

    def step(self, params, grads):
        self.t += 1
        for i, (p, g) in enumerate(zip(params, grads)):
            self.m[i] = self.beta1 * self.m[i] + (1 - self.beta1) * g
            self.v[i] = self.beta2 * self.v[i] + (1 - self.beta2) * (g * g)
            m_hat = self.m[i] / (1 - self.beta1 ** self.t)
            v_hat = self.v[i] / (1 - self.beta2 ** self.t)
            p -= self.lr * m_hat / (np.sqrt(v_hat) + self.eps)


# =============================================================================
#   Metrics.
# =============================================================================


def top1_accuracy(weights: RerankerWeights, rows: list[dict]) -> tuple[float, int]:
    """Top-1 accuracy: how often does the model rank the engine's
    `correct_idx` candidate first?
    """
    from model import score_batch, QueryFeatures
    correct = 0
    total = 0
    for row in rows:
        ci = row.get("correct_idx", -1)
        if ci < 0:
            continue
        candidates = row["candidates"]
        if not candidates:
            continue
        q = QueryFeatures.from_word(row["misspelled"], weights)
        scores = score_batch(
            weights, q,
            [c["word"] for c in candidates],
            [c.get("edit_distance", 0) for c in candidates],
        )
        if int(np.argmax(scores)) == ci:
            correct += 1
        total += 1
    return (correct / total if total else 0.0), total


# =============================================================================
#   Training loop.
# =============================================================================


def main(argv=None) -> int:
    p = argparse.ArgumentParser(description="Train the char-MLP reranker.")
    p.add_argument("--train", type=Path, required=True)
    p.add_argument("--dev", type=Path, default=None)
    p.add_argument("--epochs", type=int, default=20)
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--max-train-rows", type=int, default=0)
    p.add_argument("--embed-dim", type=int, default=None)
    p.add_argument("--hidden-dim", type=int, default=None)
    p.add_argument("--trigram-dim", type=int, default=None)
    # Ablation flags: each removes a feature group from the
    # architecture (n_in shrinks, params drop). D is the default
    # (everything on); --no-trigrams makes it B; adding
    # --no-edit-ops makes it A; etc.
    p.add_argument("--no-trigrams", action="store_true",
                   help="Disable trigram features (B->A, C->B, D->C).")
    p.add_argument("--no-edit-ops", action="store_true",
                   help="Disable edit operation counts (C->B, D->C).")
    p.add_argument("--no-edit-position", action="store_true",
                   help="Disable edit position features (D->C).")
    args = p.parse_args(argv)

    config = FeatureConfig(
        use_trigrams=not args.no_trigrams,
        use_edit_ops=not args.no_edit_ops,
        use_edit_position=not args.no_edit_position,
    )
    print(
        f"ablation: {config.label()}  "
        f"(trigrams={'on' if config.use_trigrams else 'off'}, "
        f"edit_ops={'on' if config.use_edit_ops else 'off'}, "
        f"edit_position={'on' if config.use_edit_position else 'off'})",
        flush=True,
    )

    rng = np.random.default_rng(args.seed)
    e = args.embed_dim if args.embed_dim is not None else EMBED_DIM
    h = args.hidden_dim if args.hidden_dim is not None else HIDDEN_DIM
    weights = RerankerWeights.from_config(
        config=config, seed=args.seed,
        embed_dim=e, hidden_dim=h, bigram_dim=BIGRAM_DIM,
    )
    print(
        f"architecture: embed_dim={e}, hidden_dim={h}, "
        f"bigram_dim={weights.bigram_dim}, trigram_dim={weights.trigram_dim}, "
        f"edit_dim={weights.edit_dim}, n_in={weights.n_in}, "
        f"params={weights.param_count()}",
        flush=True,
    )

    print(f"reading {args.train} ...", flush=True)
    train_rows = load_jsonl(args.train, max_rows=args.max_train_rows)
    print(f"  {len(train_rows)} rows", flush=True)

    dev_rows = load_jsonl(args.dev) if args.dev else []
    print(f"  dev: {len(dev_rows)} rows", flush=True)

    if not train_rows:
        print("no training rows; aborting")
        return 1

    best_dev = -1.0
    best_weights = None
    train_top1, train_total = top1_accuracy(weights, train_rows[:200])
    print(f"epoch 0: train_top1={train_top1:.3f} (n={train_total})", flush=True)

    optimizer = Adam(weights.as_list(), lr=args.lr)

    for epoch in range(1, args.epochs + 1):
        t0 = time.time()
        order = rng.permutation(len(train_rows))
        shuffled = [train_rows[i] for i in order]
        c_ids, edit_arr, is_corr, row_idx, ms_w, c_w = rows_to_arrays(
            shuffled, config,
        )
        if c_ids.shape[0] == 0:
            print(f"epoch {epoch}: no usable rows; skipping")
            continue

        # Precompute query features per unique source-row index. The
        # same source-row's misspelled word is identical for all its
        # candidates; this avoids recomputing char-pool / bigram-pool /
        # trigram-pool per candidate.
        unique_rows = {}
        for src, w in zip(row_idx, ms_w):
            unique_rows.setdefault(int(src), w)
        q_char = np.zeros((len(unique_rows), weights.embed_dim), dtype=np.float32)
        q_bigram = np.zeros((len(unique_rows), weights.bigram_dim), dtype=np.float32)
        if weights.trigram_dim > 0:
            q_trigram = np.zeros(
                (len(unique_rows), weights.trigram_dim), dtype=np.float32,
            )
        else:
            q_trigram = np.zeros((len(unique_rows), 0), dtype=np.float32)
        for r, word in unique_rows.items():
            ids = encode_chars(word)
            valid = ids != PAD_IDX
            if valid.any():
                q_char[r] = weights.char_embed[ids][valid].mean(axis=0)
            q_bigram[r] = bigram_features_batch([word], weights.bigram_dim)[0]
            if weights.trigram_dim > 0:
                q_trigram[r] = trigram_features_batch(
                    [word], weights.trigram_dim,
                )[0]

        # Cache the candidate char IDs for the backward pass.
        _c_ids_for_backward[0] = c_ids

        loss, grads = listwise_softmax_loss_and_grad(
            weights, c_ids, edit_arr, is_corr, row_idx,
            q_char, q_bigram, q_trigram, c_w,
        )
        batch_size = max(1, c_ids.shape[0])
        scaled = {k: v / batch_size for k, v in grads.items()}
        optimizer.step(weights.as_list(), [
            scaled["char_embed"], scaled["w1"], scaled["b1"], scaled["w2"], scaled["b2"],
        ])

        train_top1, train_total = top1_accuracy(weights, train_rows[:500])
        dev_top1, dev_total = ("N/A", 0)
        if dev_rows:
            dev_top1, dev_total = top1_accuracy(weights, dev_rows)
        elapsed = time.time() - t0
        print(
            f"epoch {epoch}: loss={loss:.4f}, "
            f"train_top1={train_top1:.3f} (n={train_total}), "
            f"dev_top1={dev_top1} (n={dev_total}), {elapsed:.1f}s",
            flush=True,
        )
        if dev_rows and isinstance(dev_top1, float) and dev_top1 > best_dev:
            best_dev = dev_top1
            best_weights = RerankerWeights(
                char_embed=weights.char_embed.copy(),
                w1=weights.w1.copy(), b1=weights.b1.copy(),
                w2=weights.w2.copy(), b2=weights.b2.copy(),
                embed_dim=weights.embed_dim,
                hidden_dim=weights.hidden_dim,
                bigram_dim=weights.bigram_dim,
                trigram_dim=weights.trigram_dim,
                edit_dim=weights.edit_dim,
            )
            print(f"  ** new best dev: {best_dev:.4f}; checkpoint saved", flush=True)

    if best_weights is None:
        best_weights = weights
    args.output.parent.mkdir(parents=True, exist_ok=True)
    best_weights.save(args.output)
    print(f"weights -> {args.output} ({args.output.stat().st_size} bytes)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())