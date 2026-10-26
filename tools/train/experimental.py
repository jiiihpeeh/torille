# SPDX-License-Identifier: MIT OR Apache-2.0
"""Experimental features for the Slice-3 ablation studies.

The production model (`model.py`) is the slim char+bigram+ed
configuration. These features were evaluated as additions to the
production model in the ablation harness (`eval.py`), but on the
300-row training corpus:

  ablation A (chars + ed):                   0.731 top-1, 0.839 mrr
  ablation B (+ lemma):                      0.630 top-1, 0.775 mrr
  ablation C (+ log_freq):                   0.380 top-1, 0.581 mrr
  ablation D (full: + morph + ops):          0.343 top-1, 0.557 mrr

The extra features hurt in this regime. They are kept here for
re-running the ablation studies when more training data is
available, but the production forward pass does not consume them.

This module deliberately does not depend on `model.py`'s production
forward path; it is a side-channel used only by `train.py` when the
user opts into an experimental model.
"""

from __future__ import annotations

import json
import math as _math
from pathlib import Path

import numpy as np


# ---------------------------------------------------------------------------
# Morphology one-hot.
# ---------------------------------------------------------------------------

# The morphology layout lives in `data/tags_layout.json` and is loaded
# at module init. The layout always begins with an explicit "unknown"
# slot at index 0 of every dim.

_TAGS_LAYOUT_PATH = Path(__file__).parent.parent.parent / "data" / "tags_layout.json"


def _load_tags_layout() -> tuple[list[dict], int]:
    if _TAGS_LAYOUT_PATH.exists():
        with _TAGS_LAYOUT_PATH.open() as fh:
            data = json.load(fh)
        return data["dimensions"], int(data["morph_dim"])
    fallback = [
        {"name": "POS", "size": 15, "values": ["unknown"] * 1 + ["unknown", "noun", "adjective", "pronoun", "numeral", "ordinal", "verb", "negative_verb", "adverb", "conjunction", "interjection", "preposition", "postposition", "other"]},
        {"name": "Case", "size": 17, "values": ["unknown"] + ["nominative", "accusative", "genitive", "partitive", "inessive", "elative", "illative", "adessive", "ablative", "allative", "essive", "translative", "abessive", "comitative", "prolative", "instructive"]},
        {"name": "Number", "size": 4, "values": ["unknown", "singular", "plural", "collective"]},
        {"name": "Person", "size": 4, "values": ["unknown", "first_person", "second_person", "third_person"]},
        {"name": "Tense", "size": 5, "values": ["unknown", "present", "past", "perfect", "pluperfect"]},
        {"name": "Mood", "size": 7, "values": ["unknown", "indicative", "conditional", "potential", "imperative", "instructive", "infinitive"]},
        {"name": "Voice", "size": 4, "values": ["unknown", "active", "passive", "middle"]},
        {"name": "Polarity", "size": 3, "values": ["unknown", "affirmative", "negative"]},
        {"name": "HasReadings", "size": 3, "values": ["unknown", "false", "true"]},
    ]
    return fallback, sum(d["size"] for d in fallback)


_DIMS, MORPH_DIM = _load_tags_layout()
MORPH_VALUE_INDEX: dict[str, dict[str, int]] = {
    d["name"]: {v: i for i, v in enumerate(d["values"])} for d in _DIMS
}


def _normalise(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value or "").strip().lower().replace(" ", "_")


def morphology_one_hot(features: dict[str, object] | None) -> np.ndarray:
    out = np.zeros(MORPH_DIM, dtype=np.float32)
    if features is None:
        return out
    offset = 0
    for dim in _DIMS:
        name = dim["name"]
        n = dim["size"]
        val = _normalise(features.get(name, ""))
        idx = MORPH_VALUE_INDEX[name].get(val, MORPH_VALUE_INDEX[name].get("unknown", 0))
        out[offset + idx] = 1.0
        offset += n
    return out


def morphology_one_hot_batch(features_list) -> np.ndarray:
    return np.stack([morphology_one_hot(f) for f in features_list], axis=0)


# ---------------------------------------------------------------------------
# Edit-operations one-hot (4 ops x 4 position buckets = 16).
# ---------------------------------------------------------------------------

OP_INDEX = {"sub": 0, "ins": 1, "del": 2, "trans": 3}
OPS_DIM = 16
POS_BUCKETS = 4


def edit_ops_one_hot(ops: list[dict]) -> np.ndarray:
    """Multi-hot encoding of an edit-ops list with relative position.

    Position is derived from the op's index in the ops list as a
    fraction of the list length -- this generalises to unseen word
    lengths.
    """
    out = np.zeros(OPS_DIM, dtype=np.float32)
    if not ops:
        return out
    n_ops = len(ops)
    for i, op in enumerate(ops):
        op_name = op.get("op", "")
        if op_name not in OP_INDEX:
            continue
        op_idx = OP_INDEX[op_name]
        ratio = (i + 0.5) / max(1, n_ops)
        pos_idx = min(POS_BUCKETS - 1, int(ratio * POS_BUCKETS))
        out[op_idx * POS_BUCKETS + pos_idx] = 1.0
    return np.minimum(out, 1.0)


def edit_ops_one_hot_batch(ops_list) -> np.ndarray:
    return np.stack([edit_ops_one_hot(ops) for ops in ops_list], axis=0)


# ---------------------------------------------------------------------------
# Log-frequency feature.
# ---------------------------------------------------------------------------

def log_surface_freq(word: str, freq: dict[str, int] | None = None) -> float:
    """log1p(surface-form frequency)."""
    if freq is None:
        return 0.0
    return float(_math.log1p(freq.get(word, 0)))


def log_surface_freq_batch(words: list[str], freq: dict[str, int] | None = None) -> np.ndarray:
    arr = np.zeros(len(words), dtype=np.float32)
    if freq is None:
        return arr
    for i, w in enumerate(words):
        arr[i] = float(_math.log1p(freq.get(w, 0)))
    return arr