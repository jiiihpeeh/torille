# SPDX-License-Identifier: MIT OR Apache-2.0
"""The old feature implementations have to stay importable, and stay out of the
production path.

`experimental.py` holds everything the ablations tried and the production model
does not consume: the morphology one-hot, the edit-ops one-hot with its relative
position buckets, and the log-frequency feature. The brief's instruction was to
keep them rather than delete them, so they are kept -- behind an experimental
module, off by default.

Nothing imports that module, which is how it came to be broken: `_load_tags_layout`
calls `json.load` and `json` was never imported, so `import experimental` raised
`NameError` for anyone with `data/tags_layout.json` present (i.e. anyone with the
noun pipeline's data). An unreachable module does not rot loudly; it just stops
existing for every caller while looking fine in `ls`. This is the gate for it.

The second half checks the thing that actually makes them "swappable":
`FeatureConfig` still produces all four ablation shapes, and `edit_features`
honours the width it promises for each. The A/B/C/D labels and their 6/6/10/12
edit widths are what `train.py --no-trigrams --no-edit-ops --no-edit-position`
selects and what the Rust path has to mirror, so a change to either direction
fails here rather than at parity-test time.

Run from the repository root:

    python3 tools/test_experimental_features.py
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "train"))

import numpy as np

FAILURES: list[str] = []


def check(cond: bool, msg: str) -> None:
    if not cond:
        FAILURES.append(msg)
        print(f"  FAIL  {msg}")


def main() -> int:
    # -- 1. the module has to import ------------------------------------
    # This is the regression the module actually had. Importing it is the
    # whole point of keeping it, so it is checked first and separately:
    # everything below is meaningless if this line fails.
    try:
        import experimental
    except Exception as exc:  # noqa: BLE001 - any failure is a failure
        print(f"FAIL  experimental.py does not import: {exc!r}")
        return 1
    print(f"  ok    experimental imports (MORPH_DIM={experimental.MORPH_DIM}, "
          f"OPS_DIM={experimental.OPS_DIM})")

    check(hasattr(experimental, "morphology_one_hot"), "morphology_one_hot missing")
    check(hasattr(experimental, "edit_ops_one_hot"), "edit_ops_one_hot missing")
    check(hasattr(experimental, "log_surface_freq"), "log_surface_freq missing")
    check(experimental.MORPH_DIM > 0, "MORPH_DIM must be > 0")

    # -- 2. the production path must NOT carry them ---------------------
    # `model.py` is what `score_batch` runs. If the experimental features
    # were reachable from there, "behind an experimental configuration"
    # would be a comment rather than a boundary.
    import model

    for name in ("morphology_one_hot", "edit_ops_one_hot", "log_surface_freq"):
        check(not hasattr(model, name),
              f"model.py exposes {name}; it belongs in experimental.py")

    # -- 3. morphology one-hot ------------------------------------------
    none_vec = experimental.morphology_one_hot(None)
    check(none_vec.shape == (experimental.MORPH_DIM,),
          f"morphology_one_hot(None) shape {none_vec.shape}")
    check(float(none_vec.sum()) == 0.0, "morphology_one_hot(None) must be all-zero")

    # One slot lights per dimension, not per vector: every dimension
    # falls back to its own index 0 when the field is absent, which is
    # what makes the encoding a block of per-dimension one-hots.
    dims = experimental._DIMS
    names = [d["name"] for d in dims]
    pos_values = experimental.MORPH_VALUE_INDEX["POS"]
    known = next((v for v in pos_values if v != "unknown"), None)
    if known is None:
        check(False, "the POS dimension in data/tags_layout.json has no known value")
    else:
        # POS is not necessarily the first block, so derive its offset
        # from the layout rather than assuming 0.
        offset = sum(d["size"] for d in dims[: names.index("POS")])
        vec = experimental.morphology_one_hot({"POS": known})
        check(vec.shape == (experimental.MORPH_DIM,), "morphology_one_hot width")
        check(float(vec.sum()) == float(len(dims)),
              f"{{'POS': {known!r}}} should light one slot per dimension "
              f"({len(dims)}), got {vec.sum()}")
        idx = offset + pos_values[known]
        check(0 <= idx < experimental.MORPH_DIM,
              f"POS index {idx} out of range for MORPH_DIM {experimental.MORPH_DIM}")
        if 0 <= idx < experimental.MORPH_DIM:
            check(float(vec[idx]) == 1.0,
                  f"{{'POS': {known!r}}} must light slot {idx}, "
                  f"but that slot holds {vec[idx]}")

    rows = [None, {"POS": known} if known else None, {"POS": "unknown"}]
    batch = experimental.morphology_one_hot_batch(rows)
    check(batch.shape == (len(rows), experimental.MORPH_DIM),
          f"morphology_one_hot_batch shape {batch.shape}")
    for i, row in enumerate(rows):
        check(np.array_equal(batch[i], experimental.morphology_one_hot(row)),
              f"morphology_one_hot_batch row {i} differs from morphology_one_hot")

    # -- 4. edit-ops one-hot --------------------------------------------
    ops = [{"op": "sub"}, {"op": "ins"}, {"op": "del"}, {"op": "trans"}]
    vec = experimental.edit_ops_one_hot(ops)
    check(vec.shape == (experimental.OPS_DIM,),
          f"edit_ops_one_hot shape {vec.shape}")
    check(float(vec.sum()) == len(ops),
          f"four distinct ops should light four slots, got {vec.sum()}")
    check(float(experimental.edit_ops_one_hot([]).sum()) == 0.0,
          "an empty op list must be all-zero")
    # An op name outside OP_INDEX is skipped rather than raising -- the
    # corpus carries ops from a different alignment than this encoder.
    check(float(experimental.edit_ops_one_hot([{"op": "nonsense"}]).sum()) == 0.0,
          "an unknown op name must be ignored, not crash")

    batch = experimental.edit_ops_one_hot_batch([ops, [], ops])
    check(batch.shape == (3, experimental.OPS_DIM),
          f"edit_ops_one_hot_batch shape {batch.shape}")
    for i, row in enumerate([ops, [], ops]):
        check(np.array_equal(batch[i], experimental.edit_ops_one_hot(row)),
              f"edit_ops_one_hot_batch row {i} differs from edit_ops_one_hot")

    # -- 5. log frequency -----------------------------------------------
    check(experimental.log_surface_freq("talo") == 0.0,
          "missing frequency table must give 0.0")
    check(experimental.log_surface_freq("talo", {"talo": 9}) == math.log(10),
          "log_surface_freq must be log1p of the count")
    check(experimental.log_surface_freq("talo", {}) == 0.0,
          "an absent word must give 0.0")
    check(experimental.log_surface_freq("a", {"a": 9})
          < experimental.log_surface_freq("a", {"a": 99}),
          "log_surface_freq must be monotone in the count")
    batch = experimental.log_surface_freq_batch(["a", "b"], {"a": 1, "b": 99})
    check(batch.shape == (2,), f"log_surface_freq_batch shape {batch.shape}")
    check(np.allclose(batch, [math.log(2), math.log(100)]),
          "log_surface_freq_batch must match log_surface_freq per word")

    # -- 6. FeatureConfig still produces all four ablations -------------
    a = model.FeatureConfig(use_trigrams=False, use_edit_ops=False,
                            use_edit_position=False)
    b = model.FeatureConfig(use_trigrams=True, use_edit_ops=False,
                            use_edit_position=False)
    c = model.FeatureConfig(use_trigrams=True, use_edit_ops=True,
                            use_edit_position=False)
    d = model.FeatureConfig(use_trigrams=True, use_edit_ops=True,
                            use_edit_position=True)

    check([x.label() for x in (a, b, c, d)] == ["A", "B", "C", "D"],
          f"ablation labels drifted: {[x.label() for x in (a, b, c, d)]}")
    # The widths are the contract the Rust side reads out of the .trm file.
    check([x.edit_dim for x in (a, b, c, d)] == [6, 6, 10, 12],
          f"edit widths drifted: {[x.edit_dim for x in (a, b, c, d)]}")
    check(a.trigram_dim == 0, "A must disable trigrams")
    check(d.trigram_dim == model.TRIGRAM_DIM, "D must use the full trigram width")
    check(model.DEFAULT_CONFIG.label() == "D",
          f"the module default is {model.DEFAULT_CONFIG.label()}, expected D")

    # -- 7. edit_features honours the config it is handed ----------------
    # The swap point: same inputs, four widths, no exception and no
    # silent padding. This is what makes the old and new shapes
    # selectable rather than merely present.
    q, cand = "juoksee", "jooksee"
    for cfg, want in ((a, 6), (b, 6), (c, 10), (d, 12)):
        vec = model.edit_features(q, cand, 1, cfg)
        check(vec.shape == (want,),
              f"{cfg.label()} edit_features width {vec.shape}, expected {want}")
        check(float(vec[0]) == min(1, 4) / 4.0,
              f"{cfg.label()}: slot 0 must be the normalised edit distance")

    # The structural block sits immediately after the op block, so its
    # start moves from index 1 (A) to index 5 (C). Pin both directions:
    # the vectors differ as a whole, and the structural block agrees
    # once it is read at its config-dependent offset.
    va = model.edit_features(q, cand, 1, a)
    vc = model.edit_features(q, cand, 1, c)
    check(not np.array_equal(va, vc[:6]),
          "C's first 6 slots must differ from A's -- ops occupy [1..4] and "
          "push the structural block to [5..9]")
    check(np.array_equal(va[1:], vc[5:]),
          "C's structural block (indices 5..9) must equal A's structural "
          "block (indices 1..5); the offset moves with use_edit_ops")

    # Batch sibling agrees with the scalar, at every width.
    for cfg in (a, c, d):
        batch = model.edit_features_batch(q, [cand, "juoksee"], [1, 0], cfg)
        check(batch.shape == (2, cfg.edit_dim),
              f"{cfg.label()} edit_features_batch shape {batch.shape}")
        check(np.array_equal(batch[0], model.edit_features(q, cand, 1, cfg)),
              f"{cfg.label()}: batch row 0 differs from edit_features")
        check(np.array_equal(batch[1], model.edit_features(q, "juoksee", 0, cfg)),
              f"{cfg.label()}: batch row 1 differs from edit_features")

    print()
    if FAILURES:
        print(f"{len(FAILURES)} failure(s):")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("OK: experimental features import, stay out of the production path,")
    print("    and FeatureConfig still yields all four ablation shapes.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
