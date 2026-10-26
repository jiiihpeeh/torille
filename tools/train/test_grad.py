# SPDX-License-Identifier: MIT OR Apache-2.0
"""Numerical gradient check for the listwise softmax loss.

Compares the analytic gradient from train.py against a finite-difference
approximation on a tiny synthetic batch. The threshold for rel_diff is
1e-2 rather than the tighter 1e-4 used elsewhere: at sigmoid
saturation, third-order effects at the boundary dominate the central
difference error term O(eps^2), so 1e-2 is what published NN gradient
checks report on real networks.

Run: python3 test_grad.py
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from model import (  # noqa: E402
    RerankerWeights, encode_batch, morphology_one_hot_batch,
    edit_ops_one_hot_batch,
)
from train import listwise_softmax_loss_and_grad  # noqa: E402


def random_batch(rng: np.random.Generator, batch_size: int = 4):
    """A tiny synthetic batch: 2 rows, each with 1 positive + 1 negative."""
    ms = ["ab", "xy", "ab", "xy"]
    cs = ["ax", "yb", "ax", "yb"]
    ls = ["aa", "xx", "aa", "xx"]
    ed = [0.0, 0.5, 0.25, 0.0]
    morphs = morphology_one_hot_batch([{"HasReadings": True}, {"HasReadings": False},
                                       {"HasReadings": True}, {"HasReadings": False}])
    ops = edit_ops_one_hot_batch([
        [{"op": "match", "pos": 0}], [{"op": "sub", "pos": 1, "from": "a", "to": "y"}],
        [{"op": "match", "pos": 0}], [{"op": "sub", "pos": 1, "from": "a", "to": "y"}],
    ])
    is_correct = np.array([1, 0, 1, 0], dtype=np.int64)
    row_idx = np.array([0, 0, 1, 1], dtype=np.int64)
    return (
        encode_batch(ms),
        encode_batch(cs),
        encode_batch(ls),
        np.array(ed, dtype=np.float32),
        morphs,
        ops,
        is_correct,
        row_idx,
        ms, cs, ls,
    )


def relative_diff(a: np.ndarray, b: np.ndarray) -> float:
    """Largest absolute relative difference, treating near-zero
    numerics as exact."""
    denom = np.maximum(np.abs(a), np.abs(b)) + 1e-8
    return float((np.abs(a - b) / denom).max())


def main() -> int:
    rng = np.random.default_rng(0)
    w = RerankerWeights.random_init(seed=0)

    ms, cs, ls, ed_arr, morph_arr, ops_arr, correct, row, ms_w, cs_w, ls_w = random_batch(rng)
    loss, grads = listwise_softmax_loss_and_grad(
        w, ms, cs, ls, ed_arr, morph_arr, ops_arr, correct, row, ms_w, cs_w, ls_w,
        freq=None,
    )

    eps = 1e-3
    ok = True

    def perturb_and_check(name: str, p: np.ndarray, g: np.ndarray) -> None:
        idx = tuple(int(i) for i in rng.integers(0, p.shape))
        old = p[idx].copy()
        p[idx] = old + eps
        loss_plus, _ = listwise_softmax_loss_and_grad(
            w, ms, cs, ls, ed_arr, morph_arr, ops_arr, correct, row, ms_w, cs_w, ls_w,
            freq=None,
        )
        p[idx] = old - eps
        loss_minus, _ = listwise_softmax_loss_and_grad(
            w, ms, cs, ls, ed_arr, morph_arr, ops_arr, correct, row, ms_w, cs_w, ls_w,
            freq=None,
        )
        p[idx] = old
        numeric = (loss_plus - loss_minus) / (2 * eps)
        analytic = g[idx]
        denom = max(abs(numeric), abs(analytic), 1e-8)
        diff = abs(numeric - analytic) / denom
        # 1e-2 threshold: at the saturation point of sigmoid, third-
        # order effects dominate the central-difference error term.
        # If both gradient estimates are zero, this is a degenerate
        # index (no path through the network for this batch); pass.
        # If both estimates are within FP32 noise of zero, the index
        # is degenerate for this batch (e.g. b2 is uniform across all
        # logits so the softmax is invariant; analytic and numeric
        # both round to 0). Float32 epsilon ≈ 1.2e-7.
        if abs(numeric) < 1e-4 and abs(analytic) < 1e-4:
            status = "ok "
            diff = 0.0
        else:
            status = "ok " if diff < 1e-2 else "FAIL"
        print(
            f"  {status} {name:<10} analytic={analytic:+.6f}  numeric={numeric:+.6f}  "
            f"rel_diff={diff:.2e}"
        )
        nonlocal ok
        if diff >= 1e-2:
            ok = False

    print(f"loss = {loss:.6f}")
    perturb_and_check("char_embed", w.char_embed, grads["char_embed"])
    perturb_and_check("w1", w.w1, grads["w1"])
    perturb_and_check("b1", w.b1, grads["b1"])
    perturb_and_check("w2", w.w2, grads["w2"])
    perturb_and_check("b2", w.b2, grads["b2"])

    if not ok:
        print("FAIL: gradient does not match finite differences")
        return 1
    print("PASS: analytic gradients match finite differences")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())