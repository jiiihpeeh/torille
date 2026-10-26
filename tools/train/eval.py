# SPDX-License-Identifier: MIT OR Apache-2.0
"""Evaluate the trained reranker on the held-out splits.

Slice-3.5 evaluation: chars + bigrams + trigrams + 12-dim edit
features, ~38k params. The earlier ablation harness (A/B/C/D with
morphology, lemma, log_freq, ops) is preserved in the git history
of this file but not in the production path.

Reports:
  - per-split table: engine_top1 vs model_top1, with top-3/5, MRR,
    generation_recall, n_eval, n_miss
  - deterministic ranker comparison (the hand-coded NN happens to
    learn anything beyond this baseline?)
  - false-positive rate (engine + model)
  - latency breakdown: feature extraction vs MLP vs total

Usage:
    python3 eval.py \
        --weights /tmp/torille_reranker.npz \
        --data-dir data/rerank \
        --report /tmp/torille_eval_report.md
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from model import (  # noqa: E402
    QueryFeatures, RerankerWeights, edit_features_batch,
    score_batch, trigram_features_batch,
)
from deterministic_ranker import deterministic_rank  # noqa: E402


HIGH_CONFIDENCE_THRESHOLD = 0.7


def load_jsonl(path: Path) -> list[dict]:
    """Read JSONL, silently skipping lines that do not parse."""
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
    if skipped:
        print(f"  (load_jsonl: skipped {skipped} malformed lines in {path})")
    return rows


# ---------------------------------------------------------------------------
#   Top-1 + top-k + MRR.
# ---------------------------------------------------------------------------


def _score_row(weights: RerankerWeights, row: dict,
               cache: dict) -> np.ndarray:
    """Score one row's candidates. Uses a per-row QueryFeatures cache
    so the same misspelled doesn't get re-pooled multiple times."""
    q = cache.get(row["misspelled"])
    if q is None:
        q = QueryFeatures.from_word(row["misspelled"], weights)
        cache[row["misspelled"]] = q
    candidates = row["candidates"]
    return score_batch(
        weights, q,
        [c["word"] for c in candidates],
        [c.get("edit_distance", 0) for c in candidates],
    )


def model_top1(weights: RerankerWeights, rows: list[dict]) -> tuple[float, int, int]:
    n_eval = n_correct = n_miss = 0
    cache: dict[str, QueryFeatures] = {}
    for row in rows:
        ci = row.get("correct_idx", -1)
        if ci < 0:
            n_miss += 1
            continue
        candidates = row["candidates"]
        if not candidates:
            continue
        scores = _score_row(weights, row, cache)
        n_eval += 1
        if int(np.argmax(scores)) == ci:
            n_correct += 1
    return (n_correct / n_eval if n_eval else 0.0), n_eval, n_miss


def model_topk_metrics(
    weights: RerankerWeights, rows: list[dict],
    ks: tuple[int, ...] = (1, 3, 5),
) -> dict[str, float]:
    """Top-k + MRR for the model. Uses a per-misspelled QueryFeatures
    cache so query-side features are computed once per query."""
    hits = {k: 0 for k in ks}
    mrr_sum = 0.0
    n_eval = 0
    n_miss = 0
    cache: dict[str, QueryFeatures] = {}
    for row in rows:
        ci = row.get("correct_idx", -1)
        if ci < 0:
            n_miss += 1
            continue
        candidates = row["candidates"]
        if not candidates:
            continue
        scores = _score_row(weights, row, cache)
        n_eval += 1
        order = np.argsort(-scores)
        rank = int(np.where(order == ci)[0][0]) if ci in order else len(order)
        mrr_sum += 1.0 / (rank + 1)
        for k in ks:
            if rank < k:
                hits[k] += 1
    return {
        **{f"top{k}": (hits[k] / n_eval if n_eval else 0.0) for k in ks},
        "mrr": (mrr_sum / n_eval if n_eval else 0.0),
        "n_eval": n_eval,
        "n_miss": n_miss,
    }


def generation_recall(rows: list[dict]) -> float:
    if not rows:
        return 0.0
    return sum(1 for r in rows if r.get("correct_idx", -1) >= 0) / len(rows)


def engine_top1(rows: list[dict]) -> tuple[float, int]:
    n_correct = n_eval = 0
    for row in rows:
        ci = row.get("correct_idx", -1)
        if ci < 0:
            continue
        candidates = row["candidates"]
        if not candidates:
            continue
        indexed = sorted(
            enumerate(candidates),
            key=lambda kv: (kv[1].get("edit_distance", 99), kv[1]["word"]),
        )
        if indexed[0][0] == ci:
            n_correct += 1
        n_eval += 1
    return (n_correct / n_eval if n_eval else 0.0), n_eval


def deterministic_topk_metrics(rows: list[dict]) -> dict[str, float]:
    hits = {1: 0, 3: 0, 5: 0}
    mrr_sum = 0.0
    n_eval = 0
    for row in rows:
        ci = row.get("correct_idx", -1)
        if ci < 0:
            continue
        candidates = row["candidates"]
        if not candidates:
            continue
        scores = deterministic_rank(row["misspelled"], candidates)
        n_eval += 1
        order = np.argsort(-np.asarray(scores))
        rank = int(np.where(order == ci)[0][0]) if ci in order else len(order)
        mrr_sum += 1.0 / (rank + 1)
        for k in (1, 3, 5):
            if rank < k:
                hits[k] += 1
    return {
        "top1": (hits[1] / n_eval if n_eval else 0.0),
        "top3": (hits[3] / n_eval if n_eval else 0.0),
        "top5": (hits[5] / n_eval if n_eval else 0.0),
        "mrr": (mrr_sum / n_eval if n_eval else 0.0),
        "n_eval": n_eval,
    }


def false_positive_rate_model(
    weights: RerankerWeights, engine_miss_rows: list[dict],
) -> tuple[int, int, float]:
    """Of rows where the engine did NOT surface the correct form, how
    often does the model pick something confidently?"""
    high = 0
    total = 0
    max_probs: list[float] = []
    cache: dict[str, QueryFeatures] = {}
    for row in engine_miss_rows:
        candidates = row["candidates"]
        if not candidates:
            continue
        logits = _score_row(weights, row, cache)
        # Listwise softmax across the candidate list.
        shifted = logits - logits.max()
        exp_x = np.exp(shifted)
        probs = exp_x / exp_x.sum()
        total += 1
        max_p = float(probs.max())
        max_probs.append(max_p)
        if max_p > HIGH_CONFIDENCE_THRESHOLD:
            high += 1
    return high, total, (float(np.mean(max_probs)) if max_probs else 0.0)


def false_positive_rate_engine(engine_miss_rows: list[dict]) -> tuple[int, int]:
    total = 0
    for row in engine_miss_rows:
        if row["candidates"]:
            total += 1
    return total, total


# ---------------------------------------------------------------------------
#   Latency breakdown.
# ---------------------------------------------------------------------------


def latency_breakdown(
    weights: RerankerWeights, rows: list[dict],
    max_candidates: int = 0,
) -> dict[str, float]:
    """Measure feature-extraction vs MLP vs total reranking time.

    Returns a dict with mean per-query timings in microseconds.
    `feature_extract_us` is the time to build QueryFeatures +
    per-candidate char-pool / bigram-pool / trigram-pool / edit
    features. `mlp_us` is the time to run the two matmuls.
    `total_us` is feature_extract + mlp.

    `max_candidates` truncates each row's candidate list to that many
    entries, so the sweep at 5/10/20/50/100 candidates in the report
    measures the same widths the Rust `rerank_bench` does. Zero means
    "everything the row carries".

    Candidate *generation* is not measured here: it happens in the
    Rust engine, which this process does not link. `rerank_bench`
    reports that stage.
    """
    cache: dict[str, QueryFeatures] = {}
    feat_total = 0.0
    mlp_total = 0.0
    n_calls = 0
    n_cands = 0

    for row in rows:
        candidates = row["candidates"]
        if max_candidates > 0:
            candidates = candidates[:max_candidates]
        if not candidates:
            continue
        n = len(candidates)
        t0 = time.perf_counter()

        # Feature extraction.
        q = cache.get(row["misspelled"])
        if q is None:
            q = QueryFeatures.from_word(row["misspelled"], weights)
            cache[row["misspelled"]] = q
        from model import encode_batch, bigram_features_batch
        cand_words = [c["word"] for c in candidates]
        cand_ids = encode_batch(cand_words)
        cand_char = _pool_chars(cand_ids, weights)
        cand_bigram = bigram_features_batch(cand_words, weights.bigram_dim)
        cand_trigram = trigram_features_batch(cand_words, weights.trigram_dim)
        # The width must come from the weights, not the module default:
        # Model A has edit_dim 6, and building the default 12-wide
        # vector here is what made `--benchmark` raise a shape error
        # for exactly the model the brief asks about.
        cand_edit = edit_features_batch(
            q.word, cand_words,
            [c.get("edit_distance", 0) for c in candidates],
            weights.feature_config(),
        )

        # Build the (N, n_in) matrix. Layout must match model.score_batch.
        n_in = weights.n_in
        e = weights.embed_dim
        b = weights.bigram_dim
        t = weights.trigram_dim
        d = weights.edit_dim
        x = np.empty((n, n_in), dtype=np.float32)
        x[:, 0:e] = q.char_mean
        x[:, e:e + b] = q.bigram
        x[:, e + b:e + b + t] = q.trigram
        x[:, e + b + t:e + b + t + e] = cand_char
        x[:, 2 * e + b + t:2 * e + b + t + b] = cand_bigram
        x[:, 2 * e + 2 * b + t:2 * e + 2 * b + t + t] = cand_trigram
        x[:, 2 * e + 2 * b + 2 * t:2 * e + 2 * b + 2 * t + d] = cand_edit

        t1 = time.perf_counter()

        # MLP forward.
        h_pre = x @ weights.w1.T + weights.b1
        h = np.maximum(h_pre, 0.0)
        h2 = np.concatenate([h, np.ones((n, 1), dtype=np.float32)], axis=1)
        _ = (h2 @ weights.w2.T + weights.b2).squeeze(-1)

        t2 = time.perf_counter()

        feat_total += (t1 - t0) * 1e6
        mlp_total += (t2 - t1) * 1e6
        n_calls += 1
        n_cands += n

    return {
        "n_calls": n_calls,
        "max_candidates": max_candidates,
        "candidates_per_call": (n_cands / max(1, n_calls)),
        "feature_extract_us": (feat_total / max(1, n_calls)),
        "mlp_us": (mlp_total / max(1, n_calls)),
        "total_us": ((feat_total + mlp_total) / max(1, n_calls)),
    }


def _pool_chars(char_ids: np.ndarray, weights: RerankerWeights) -> np.ndarray:
    """Mean-pool of (N, MAX_LEN) char IDs into (N, embed_dim)."""
    from model import PAD_IDX
    emb = weights.char_embed[char_ids]
    valid = (char_ids != PAD_IDX).astype(np.float32)
    valid = valid[..., None]
    emb_sum = (emb * valid).sum(axis=1)
    n_valid = valid.sum(axis=1).clip(min=1.0)
    return emb_sum / n_valid


# ---------------------------------------------------------------------------
#   Main.
# ---------------------------------------------------------------------------


def main(argv=None) -> int:
    p = argparse.ArgumentParser(description="Evaluate the trained reranker (Model A).")
    p.add_argument("--weights", type=Path, default=None)
    p.add_argument("--data-dir", type=Path, required=True)
    p.add_argument("--report", type=Path, default=None)
    p.add_argument("--benchmark", action="store_true",
                   help="also print feature-extraction vs MLP latency breakdown")
    p.add_argument("--bench-sizes", default="5,10,20,50,100",
                   help="comma-separated candidate-list widths for "
                        "--benchmark; matches rerank_bench --sizes")
    args = p.parse_args(argv)

    weights: RerankerWeights | None = None
    if args.weights is not None:
        weights = RerankerWeights.load(args.weights)
        print(f"weights: {args.weights} ({args.weights.stat().st_size} bytes)")
    else:
        print("(no --weights given; running baseline-only mode)")
    print(f"data dir: {args.data_dir}")
    print()

    splits_paths = [
        ("dev", args.data_dir / "dev.jsonl"),
        ("test_unseen_lemma", args.data_dir / "test_unseen_lemma.jsonl"),
        ("test_unseen_inflection", args.data_dir / "test_unseen_inflection.jsonl"),
        ("test_unseen_corruption", args.data_dir / "test_unseen_corruption.jsonl"),
    ]

    md_lines: list[str] = []
    md_lines.append("# Reranker evaluation report")
    md_lines.append("")
    if weights is not None:
        md_lines.append(
            f"- weights: `{args.weights}` "
            f"({args.weights.stat().st_size} bytes, {weights.param_count()} params)"
        )
    md_lines.append(f"- data dir: `{args.data_dir}`")
    md_lines.append("")

    all_rows: list[tuple[str, list[dict]]] = []
    for split_name, path in splits_paths:
        if not path.exists():
            print(f"  {split_name:<28} MISSING ({path})")
            continue
        rows = load_jsonl(path)
        all_rows.append((split_name, rows))

    if not all_rows:
        print("no data found; aborting")
        return 1

    print("Per-split table (engine, deterministic, full model):")
    print(f"  {'split':<28} {'gen_rec':>7} {'engine':>7} {'det':>7} {'model':>7} "
          f"{'det_mrr':>7} {'mrr':>6} {'n_eval':>7} {'n_miss':>7}")
    print("  " + "-" * 110)
    md_lines.append("| split | gen_recall | engine | det | model | det_mrr | mrr | n_eval | n_miss |")
    md_lines.append("|---|---|---|---|---|---|---|---|---|")

    for split_name, rows in all_rows:
        engine_acc, n_eng_eval = engine_top1(rows)
        det = deterministic_topk_metrics(rows)
        gen_rec = generation_recall(rows)
        n_miss = sum(1 for r in rows if r.get("correct_idx", -1) < 0)
        if weights is not None:
            metrics = model_topk_metrics(weights, rows)
            model_acc = metrics["top1"]
            mrr = metrics["mrr"]
            n_eval = metrics["n_eval"]
            model_str = f"{model_acc:.3f}"
            mrr_str = f"{mrr:.3f}"
        else:
            n_eval = n_eng_eval
            model_str = "    -   "
            mrr_str = "    -"
        engine_str = f"{engine_acc:.3f}"
        det_str = f"{det['top1']:.3f}"
        det_mrr_str = f"{det['mrr']:.3f}"
        gen_rec_str = f"{gen_rec:.2f}"
        print(
            f"  {split_name:<28} {gen_rec_str:>7} {engine_str:>7} {det_str:>7} {model_str:>7} "
            f"{det_mrr_str:>7} {mrr_str:>6} {n_eval:>7} {n_miss:>7}"
        )
        md_lines.append(
            f"| {split_name} | {gen_rec_str} | {engine_str} | {det_str} | {model_str} | "
            f"{det_mrr_str} | {mrr_str} | {n_eval} | {n_miss} |"
        )

    # False-positive rate.
    miss_rows: list[dict] = []
    for _, rows in all_rows:
        for row in rows:
            if row.get("correct_idx", -1) < 0:
                miss_rows.append(row)
    eng_fp, eng_fp_total = false_positive_rate_engine(miss_rows)
    print()
    print("False-positive rate (rows where engine missed the correct form):")
    print(f"  engine-miss rows in eval data:                 {len(miss_rows)}")
    print(
        f"  engine picks something (no abstention):      "
        f"{eng_fp}/{eng_fp_total} ({eng_fp/max(1, eng_fp_total):.1%})"
    )
    md_lines.append("")
    md_lines.append("## False-positive estimate")
    md_lines.append("")
    md_lines.append(
        "Rows where the engine did not surface the correct form. The "
        "engine cannot abstain (it always returns a top-1 candidate), "
        "so its baseline false-positive rate is 100 % by construction. "
        "The reranker's value here is in *withholding confidence*, "
        "not in abstention."
    )
    md_lines.append("")
    md_lines.append(
        f"- engine-miss rows in evaluation splits: {len(miss_rows)}"
    )
    md_lines.append(
        f"- engine top-1 (no abstention possible): "
        f"{eng_fp}/{eng_fp_total} ({eng_fp/max(1, eng_fp_total):.1%})"
    )
    if weights is not None and miss_rows:
        mdl_fp, mdl_total, mean_max = false_positive_rate_model(weights, miss_rows)
        print(
            f"  model high-confidence (score > {HIGH_CONFIDENCE_THRESHOLD:.2f}):"
            f"        {mdl_fp}/{mdl_total} ({mdl_fp/max(1, mdl_total):.1%})"
        )
        print(f"  model mean max score on engine-miss rows:  {mean_max:.3f}")
        md_lines.append(
            f"- model high-confidence (score > {HIGH_CONFIDENCE_THRESHOLD:.2f}): "
            f"{mdl_fp}/{mdl_total} ({mdl_fp/max(1, mdl_total):.1%})"
        )
        md_lines.append(f"- model mean max score on engine-miss rows: {mean_max:.3f}")

    # Latency breakdown.
    if args.benchmark and weights is not None and all_rows:
        print()
        print("Latency breakdown (microseconds per query, dev split):")
        print("  candidate generation runs in the Rust engine and is not")
        print("  measured here; `rerank_bench` reports that stage.")
        dev_rows = next((r for n, r in all_rows if n == "dev"), all_rows[0][1])
        sizes = [int(s) for s in args.bench_sizes.split(",") if s.strip()]
        # Warm-up: the first call fills the query cache and allocates
        # the numpy temporaries, which would otherwise be charged to
        # the narrowest width and skew the sweep.
        if dev_rows:
            _ = latency_breakdown(weights, dev_rows[:5])
        hdr = (f"  {'K':>5} {'cand':>7} {'feature_us':>11} {'mlp_us':>11} "
               f"{'total_us':>11} {'n':>5}")
        print(hdr)
        print("  " + "-" * (len(hdr) - 2))
        bench_rows: list[str] = []
        for k in sizes:
            bd = latency_breakdown(weights, dev_rows, max_candidates=k)
            if not bd["n_calls"]:
                continue
            print(f"  {k:>5} {bd['candidates_per_call']:>7.2f} "
                  f"{bd['feature_extract_us']:>11.3f} {bd['mlp_us']:>11.3f} "
                  f"{bd['total_us']:>11.3f} {int(bd['n_calls']):>5}")
            bench_rows.append(
                f"| {k} | {bd['candidates_per_call']:.2f} | "
                f"{bd['feature_extract_us']:.3f} | {bd['mlp_us']:.3f} | "
                f"{bd['total_us']:.3f} |"
            )
        md_lines.append("")
        md_lines.append("## Latency breakdown (dev split, µs/query)")
        md_lines.append("")
        md_lines.append("Candidate generation runs in the Rust engine and is")
        md_lines.append("not measured here; `rerank_bench` reports that stage.")
        md_lines.append("")
        md_lines.append("| K | candidates | feature | mlp | total |")
        md_lines.append("|---|---|---|---|---|")
        md_lines.extend(bench_rows)

    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text("\n".join(md_lines) + "\n")
        print(f"\nreport -> {args.report}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())