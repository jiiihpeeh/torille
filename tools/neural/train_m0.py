#!/usr/bin/env python3
"""Train the Phase 3 M0 candidate-reranking MLPs.

The script is intentionally dependency-light: Python 3 + numpy only. It reads
the JSONL produced by `ud_m0_features`, hashes candidate/context features into
a fixed dimension, and trains three one-hidden-layer candidate scorers:

  * morph   -- candidate morphology + surface only
  * local   -- morph + previous/next token and position
  * sentence-- local + a hashed bag of the sentence's other tokens

The output is a JSON metrics file and, for each variant, an `.npz` with the
fixed weights. No Rust production code is touched.
"""

from __future__ import annotations

import argparse
import json
import math
import zlib
from pathlib import Path

import numpy as np

DEFAULT_DIM = 512
DEFAULT_HIDDEN = 32
DEFAULT_EPOCHS = 24
DEFAULT_BATCH = 64
DEFAULT_LR = 0.01


def feature_keys(row: dict, candidate: dict, variant: str) -> list[str]:
    keys = [
        f"tok:surface={row['surface']}",
        f"tok:lower={row['lower']}",
        f"tok:depth={row['depth']}",
        f"tok:candidates={row['candidate_count']}",
    ]
    for field in (
        "pos",
        "case",
        "number",
        "person",
        "tense",
        "mood",
        "voice",
        "polarity",
        "possessive",
        "clitic",
    ):
        value = candidate.get(field)
        if value is not None:
            keys.append(f"cand:{field}={value}")
    if variant not in ("tags", "stack"):
        for lemma in candidate.get("lemmas", []):
            keys.append(f"cand:lemma={lemma}")
        if not candidate.get("provenance_known", False):
            keys.append("cand:provenance=unavailable")

    if variant in ("local", "sentence"):
        keys.append(f"ctx:prev={row.get('prev')}")
        keys.append(f"ctx:next={row.get('next')}")
        keys.append(f"ctx:position={row['position']}")
        keys.append(f"ctx:length={row['sentence_length']}")
    if variant == "sentence":
        for index, word in enumerate(row.get("sentence_tokens", [])):
            if index != row["position"]:
                keys.append(f"sent:{word}")
        keys.append(f"sent:length={row['sentence_length']}")
    if variant == "stack":
        # Deterministic morphology and context features. These are proxies for
        # the engine's own evidence; they are not model output.
        index = candidate.get("index", 0)
        keys.append(f"det:index={index}")
        keys.append(f"det:first={1 if index == 0 else 0}")
        keys.append(f"det:candidate_count={row['candidate_count']}")
        keys.append(f"det:depth={row['depth']}")
        for side in ("prev", "next"):
            for field in ("pos", "case", "number"):
                value = row.get(f"{side}_{field}")
                keys.append(f"det:{side}_{field}={value}")
                if value is not None and candidate.get(field) is not None:
                    agree = candidate[field] == value
                    keys.append(
                        f"det:agree_{side}_{field}={str(agree).lower()}"
                    )
    return keys


def fnv1a(key: str) -> int:
    value = 2166136261
    for byte in key.encode("utf-8"):
        value ^= byte
        value = (value * 16777619) & 0xFFFFFFFF
    return value


def hash_keys(keys: list[str], dim: int, hash_name: str) -> list[int]:
    if hash_name == "fnv":
        return [fnv1a(key) % dim for key in keys]
    return [zlib.crc32(key.encode("utf-8")) % dim for key in keys]


def build_dataset(rows: list[dict], variant: str, dim: int, hash_name: str):
    feature_ids: list[list[int]] = []
    groups: list[tuple[np.ndarray, int]] = []
    for row in rows:
        candidate_ids = []
        for candidate in row["candidates"]:
            candidate_ids.append(len(feature_ids))
            feature_ids.append(
                hash_keys(feature_keys(row, candidate, variant), dim, hash_name)
            )
        groups.append((np.asarray(candidate_ids, dtype=np.int32), int(row["label"])))
    x = np.zeros((len(feature_ids), dim), dtype=np.float32)
    for row_index, ids in enumerate(feature_ids):
        if ids:
            x[row_index, ids] = 1.0
    return x, groups


def init_params(rng: np.random.Generator, dim: int, hidden: int):
    return {
        "w1": rng.normal(0.0, 1.0 / math.sqrt(dim), (dim, hidden)).astype(np.float32),
        "b1": np.zeros(hidden, dtype=np.float32),
        "w2": rng.normal(0.0, 1.0 / math.sqrt(hidden), hidden).astype(np.float32),
        "b2": np.float32(0.0),
    }


def forward(params, x):
    hidden = np.maximum(x @ params["w1"] + params["b1"], 0.0)
    logits = hidden @ params["w2"] + params["b2"]
    return hidden, logits


def softmax(z):
    if z.size == 0:
        return z
    shifted = z - z.max()
    exp = np.exp(shifted)
    return exp / exp.sum()


def evaluate(x, groups, params, first_candidate_baseline: bool = False):
    if not groups:
        return 0.0, 0.0, 0
    _, logits = forward(params, x)
    correct = 0
    first_correct = 0
    for ids, label in groups:
        scores = logits[ids]
        if int(np.argmax(scores)) == label:
            correct += 1
        if first_candidate_baseline and label == 0:
            first_correct += 1
    return correct / len(groups), first_correct / len(groups), len(groups)


def train_variant(train_rows, dev_rows, variant, dim, hidden, epochs, batch, lr, seed=13, hash_name="zlib"):
    rng = np.random.default_rng(seed)
    x_train, groups_train = build_dataset(train_rows, variant, dim, hash_name)
    x_dev, groups_dev = build_dataset(dev_rows, variant, dim, hash_name)
    params = init_params(rng, dim, hidden)
    m = {key: np.zeros_like(value) for key, value in params.items()}
    v = {key: np.zeros_like(value) for key, value in params.items()}
    step = 0
    best = (-1.0, {key: value.copy() for key, value in params.items()}, 0)
    stale = 0

    for epoch in range(1, epochs + 1):
        order = rng.permutation(len(groups_train))
        total_loss = 0.0
        for start in range(0, len(order), batch):
            batch_groups = [groups_train[i] for i in order[start : start + batch]]
            ids = np.concatenate([group[0] for group in batch_groups])
            hidden_values, logits = forward(params, x_train[ids])
            grad_logits = np.zeros_like(logits)
            offset = 0
            for group_ids, label in batch_groups:
                n = len(group_ids)
                z = logits[offset : offset + n]
                prob = softmax(z)
                total_loss -= math.log(float(prob[label]) + 1e-9)
                grad = prob.copy()
                grad[label] -= 1.0
                grad_logits[offset : offset + n] = grad
                offset += n
            grad_logits /= max(len(batch_groups), 1)

            grad_hidden = np.outer(grad_logits, params["w2"]).astype(np.float32)
            grad_hidden[hidden_values <= 0.0] = 0.0
            grad_w1 = x_train[ids].T @ grad_hidden
            grad_b1 = grad_hidden.sum(axis=0)
            grad_w2 = hidden_values.T @ grad_logits
            grad_b2 = np.float32(grad_logits.sum())
            grads = {"w1": grad_w1, "b1": grad_b1, "w2": grad_w2, "b2": grad_b2}

            step += 1
            for key in params:
                m[key] = 0.9 * m[key] + 0.1 * grads[key]
                v[key] = 0.999 * v[key] + 0.001 * grads[key] ** 2
                m_hat = m[key] / (1.0 - 0.9**step)
                v_hat = v[key] / (1.0 - 0.999**step)
                params[key] = params[key] - lr * m_hat / (np.sqrt(v_hat) + 1e-8)

        dev_accuracy, _, dev_tokens = evaluate(x_dev, groups_dev, params)
        if dev_accuracy > best[0] + 1e-6:
            best = (
                dev_accuracy,
                {key: value.copy() for key, value in params.items()},
                epoch,
            )
            stale = 0
        else:
            stale += 1
        print(
            f"{variant} epoch {epoch:02d}: loss={total_loss / max(len(groups_train), 1):.4f} "
            f"dev={dev_accuracy:.4f} ({dev_tokens} tokens) best={best[0]:.4f}@{best[2]}"
        )
        if stale >= 6:
            break

    return best[1], best[2], best[0], len(groups_train), len(groups_dev)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", required=True, help="directory with train/dev/test.jsonl")
    parser.add_argument("--out", required=True, help="directory for metrics and weights")
    parser.add_argument("--dim", type=int, default=DEFAULT_DIM)
    parser.add_argument("--hidden", type=int, default=DEFAULT_HIDDEN)
    parser.add_argument("--epochs", type=int, default=DEFAULT_EPOCHS)
    parser.add_argument("--batch", type=int, default=DEFAULT_BATCH)
    parser.add_argument("--lr", type=float, default=DEFAULT_LR)
    parser.add_argument("--seed", type=int, default=13)
    parser.add_argument("--hash", choices=["zlib", "fnv"], default="zlib")
    parser.add_argument(
        "--variants",
        nargs="+",
        default=["morph", "local", "sentence"],
        choices=["morph", "local", "sentence", "tags", "stack"],
    )
    args = parser.parse_args()

    data = Path(args.data)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    def load(name):
        path = data / f"{name}.jsonl"
        with path.open("r", encoding="utf-8") as handle:
            return [json.loads(line) for line in handle if line.strip()]

    train_rows = load("train")
    dev_rows = load("dev")
    test_rows = load("test")
    report = {
        "dim": args.dim,
        "hidden": args.hidden,
        "epochs": args.epochs,
        "batch": args.batch,
        "learning_rate": args.lr,
        "seed": args.seed,
        "hash": args.hash,
        "train_tokens": len(train_rows),
        "dev_tokens": len(dev_rows),
        "test_tokens": len(test_rows),
        "variants": {},
    }

    for variant in args.variants:
        print(f"=== {variant} ===")
        params, best_epoch, dev_acc, train_tokens, dev_tokens = train_variant(
            train_rows,
            dev_rows,
            variant,
            args.dim,
            args.hidden,
            args.epochs,
            args.batch,
            args.lr,
            args.seed,
            args.hash,
        )
        x_test, groups_test = build_dataset(test_rows, variant, args.dim, args.hash)
        test_acc, test_first, test_tokens = evaluate(
            x_test, groups_test, params, first_candidate_baseline=True
        )
        x_dev, groups_dev = build_dataset(dev_rows, variant, args.dim, args.hash)
        dev_acc2, dev_first, _ = evaluate(
            x_dev, groups_dev, params, first_candidate_baseline=True
        )
        np.savez(
            out / f"m0-{variant}.npz",
            dim=args.dim,
            hidden=args.hidden,
            **params,
        )
        report["variants"][variant] = {
            "best_epoch": best_epoch,
            "train_tokens": train_tokens,
            "dev_tokens": dev_tokens,
            "dev_top1": round(dev_acc2, 4),
            "dev_first_candidate": round(dev_first, 4),
            "test_tokens": test_tokens,
            "test_top1": round(test_acc, 4),
            "test_first_candidate": round(test_first, 4),
        }
        print(
            f"{variant}: dev={dev_acc2:.4f} test={test_acc:.4f} "
            f"first-candidate test={test_first:.4f}"
        )

    (out / "m0_report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
