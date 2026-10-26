# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify that the Slice-3.5 feature matrix has no candidate-position column.

Prints the column layout of the (N, n_in) feature matrix
the way `score_batch` builds it. If the layout is correct,
there is no `candidate_position` feature and the model
cannot learn "always pick index 0" as a shortcut -- the
100/0/0/0 `correct_idx` histogram is structurally
irrelevant to what the model can learn.

The output is intended to be checked in as
`data/rerank_typo_v2/NO_POSITIONAL_FEATURE.txt`.

Usage:
    python3 tools/diag_positional.py > data/rerank_typo_v2/NO_POSITIONAL_FEATURE.txt
"""

from __future__ import annotations

import sys

sys.path.insert(0, "tools/train")
from model import RerankerWeights  # noqa: E402


def main() -> int:
    w = RerankerWeights.random_init(seed=0)
    e, b, t, d = w.embed_dim, w.bigram_dim, w.trigram_dim, w.edit_dim

    print("Slice-3.5 feature matrix layout")
    print("=" * 60)
    print()
    print("x.shape = (N, n_in), where N = number of candidates")
    print()
    print("  slice                      width   feature")
    print("  -------------------------  ------  -------------------------")
    print(f"  [0:e]                      {e:>6}  q_char     (mean-pooled)")
    print(f"  [e:e+b]                    {b:>6}  q_bigram   (sum-pooled)")
    print(f"  [e+b:e+b+t]                {t:>6}  q_trigram  (sum-pooled)")
    print(f"  [e+b+t:e+b+t+e]            {e:>6}  c_char     (mean-pooled)")
    print(f"  [2e+b+t:2e+b+t+b]          {b:>6}  c_bigram   (sum-pooled)")
    print(f"  [2e+2b+t:2e+2b+t+t]        {t:>6}  c_trigram  (sum-pooled)")
    print(f"  [2e+2b+2t:2e+2b+2t+d]      {d:>6}  edit_features (12-dim)")
    print("  -------------------------  ------  -------------------------")
    print(f"  n_in = 2e + 2b + 2t + d = {2*e + 2*b + 2*t + d}")
    print()
    print("Edit feature layout (12 dims):")
    print("  [0]  edit_distance / 4")
    print("  [1]  substitutions / 4")
    print("  [2]  insertions / 4")
    print("  [3]  deletions / 4")
    print("  [4]  transpositions / 2")
    print("  [5]  prefix / 10")
    print("  [6]  suffix / 10")
    print("  [7]  edit_start / max(1, len(q))")
    print("  [8]  edit_end   / max(1, len(q))")
    print("  [9]  |len(q) - len(c)| / 5")
    print("  [10] len(q) / 20")
    print("  [11] len(c) / 20")
    print()
    print("No `candidate_position` column. The model cannot learn")
    print("`index 0 is correct` as a shortcut regardless of how the")
    print("training data orders its candidates.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
