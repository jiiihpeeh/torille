#!/usr/bin/env python3
"""Combine the M0 candidate model with deterministic reading priors.

This is an offline experiment for the product question "does model +
deterministic evidence beat the model alone?". It does not train a new
model. It loads an exported M0 `.npz`, rebuilds the tags-only features, and
sweeps a deterministic prior weight on dev before reporting test.

The deterministic prior is the reading order the morphology engine already
produced. It is not a probability; the script treats it as a prior and learns
one scalar weight on dev.

Usage:
  python3 tools/neural/combine_m0.py \
    --data /tmp/m0 --model /tmp/m0-tags-29/m0-tags.npz
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np

from train_m0 import feature_keys, hash_keys


def load_rows(path: Path) -> list[dict]:
    with path.open("r", encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def build_features(rows: list[dict], dim: int) -> tuple[np.ndarray, list]:
    feature_ids: list[list[int]] = []
    groups: list[tuple[np.ndarray, int]] = []
    for row in rows:
        ids = []
        for candidate in row["candidates"]:
            ids.append(len(feature_ids))
            feature_ids.append(
                hash_keys(feature_keys(row, candidate, "tags"), dim, "fnv")
            )
        groups.append((np.asarray(ids, dtype=np.int32), int(row["label"])))
    x = np.zeros((len(feature_ids), dim), dtype=np.float32)
    for index, features in enumerate(feature_ids):
        if features:
            x[index, list(dict.fromkeys(features))] = 1.0
    return x, groups


def logits(params, x: np.ndarray) -> np.ndarray:
    hidden = np.maximum(x @ params["w1"] + params["b1"], 0.0)
    return hidden @ params["w2"] + params["b2"]


def softmax(z: np.ndarray) -> np.ndarray:
    if z.size == 0:
        return z
    shifted = z - z.max()
    exp = np.exp(shifted)
    return exp / exp.sum()


def prior_scores(rows: list[dict], kind: str) -> list[np.ndarray]:
    out = []
    for row in rows:
        n = len(row["candidates"])
        if kind == "uniform":
            prior = np.ones(n, dtype=np.float64)
        elif kind == "reciprocal":
            prior = 1.0 / (np.arange(n, dtype=np.float64) + 1.0)
        elif kind == "first":
            prior = np.full(n, 0.01, dtype=np.float64)
            prior[0] = 1.0
        else:
            raise ValueError(f"unknown prior {kind}")
        prior /= prior.sum()
        out.append(prior)
    return out


def probabilities(model, x: np.ndarray, groups: list) -> list[np.ndarray]:
    z = logits(model, x)
    return [softmax(z[ids]) for ids, _ in groups]


def top1(rows: list[dict], props: list[np.ndarray]) -> float:
    correct = sum(
        1
        for row, prob in zip(rows, props)
        if int(np.argmax(prob)) == int(row["label"])
    )
    return correct / max(len(rows), 1)


def combine_log(
    model_probs: list[np.ndarray], priors: list[np.ndarray], weight: float
) -> list[np.ndarray]:
    out = []
    for model_prob, prior in zip(model_probs, priors):
        logp = np.log(np.maximum(model_prob, 1e-12)) + weight * np.log(
            np.maximum(prior, 1e-12)
        )
        out.append(softmax(logp))
    return out


def combine_linear(
    model_probs: list[np.ndarray], priors: list[np.ndarray], weight: float
) -> list[np.ndarray]:
    out = []
    for model_prob, prior in zip(model_probs, priors):
        mixed = (1.0 - weight) * model_prob + weight * prior
        out.append(mixed / mixed.sum())
    return out


def select_best(
    rows, model_probs, priors, weight_grid, mode
) -> tuple[float, float]:
    best_weight = 0.0
    best_accuracy = -1.0
    combine = combine_log if mode == "log" else combine_linear
    for weight in weight_grid:
        accuracy = top1(rows, combine(model_probs, priors, weight))
        if accuracy > best_accuracy:
            best_accuracy = accuracy
            best_weight = weight
    return best_weight, best_accuracy


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", required=True, help="directory with dev/test.jsonl")
    parser.add_argument("--model", required=True, help="exported m0-tags .npz")
    parser.add_argument("--out", help="optional JSON report path")
    args = parser.parse_args()

    data = Path(args.data)
    dev_rows = load_rows(data / "dev.jsonl")
    test_rows = load_rows(data / "test.jsonl")

    with np.load(args.model) as stored:
        dim = int(stored["dim"])
        hidden = int(stored["hidden"])
        model = {
            "w1": np.asarray(stored["w1"], dtype=np.float32),
            "b1": np.asarray(stored["b1"], dtype=np.float32),
            "w2": np.asarray(stored["w2"], dtype=np.float32),
            "b2": np.float32(stored["b2"]),
        }
        assert model["w1"].shape == (dim, hidden)

    x_dev, groups_dev = build_features(dev_rows, dim)
    x_test, groups_test = build_features(test_rows, dim)
    dev_model = probabilities(model, x_dev, groups_dev)
    test_model = probabilities(model, x_test, groups_test)

    weight_grid = [round(0.05 * step, 2) for step in range(0, 81)]
    report: dict = {
        "model": args.model,
        "dev_tokens": len(dev_rows),
        "test_tokens": len(test_rows),
        "model_only": {
            "dev": round(top1(dev_rows, dev_model), 4),
            "test": round(top1(test_rows, test_model), 4),
        },
        "priors": {},
    }

    for prior_kind in ("uniform", "reciprocal", "first"):
        dev_prior = prior_scores(dev_rows, prior_kind)
        test_prior = prior_scores(test_rows, prior_kind)
        entries: dict = {
            "prior_only": {
                "dev": round(top1(dev_rows, dev_prior), 4),
                "test": round(top1(test_rows, test_prior), 4),
            }
        }
        for mode in ("log", "linear"):
            dev_weight, dev_acc = select_best(
                dev_rows, dev_model, dev_prior, weight_grid, mode
            )
            test_props = (
                combine_log(test_model, test_prior, dev_weight)
                if mode == "log"
                else combine_linear(test_model, test_prior, dev_weight)
            )
            entries[mode] = {
                "best_weight": dev_weight,
                "dev": round(dev_acc, 4),
                "test": round(top1(test_rows, test_props), 4),
            }
        entries["fixed_product"] = {
            "weight": 1.0,
            "test": round(
                top1(test_rows, combine_log(test_model, test_prior, 1.0)), 4
            ),
        }
        entries["fixed_sqrt"] = {
            "weight": 0.5,
            "test": round(
                top1(test_rows, combine_log(test_model, test_prior, 0.5)), 4
            ),
        }
        report["priors"][prior_kind] = entries

    print(json.dumps(report, indent=2, sort_keys=True))
    if args.out:
        Path(args.out).write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
