#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Prove the Rust and Python Model-A paths rank identically.

Python has no candidate generator — the engine lives in Rust — so the
only way to hand both implementations *identical* inputs is to export
the candidate lists once and feed them to each side:

```text
# 1. candidate lists out of a rerank JSONL split (Rust engine wrote them)
rank_agreement.py export data/rerank/dev.jsonl /tmp/cand.tsv

# 2. Rust scores them
cargo run -p torille-tools --release --bin rerank -- \
    --trm /tmp/modelA.trm --candidates-in /tmp/cand.tsv > /tmp/rust.tsv

# 3. Python scores the same lists and diffs the two rankings
rank_agreement.py compare /tmp/cand.tsv /tmp/modelA.trm /tmp/rust.tsv
```

`compare` asserts three things, in increasing order of strictness:

1. the same **argmax** (the brief's headline requirement),
2. the same **full ranking order**, top to bottom,
3. logits agreeing within a small tolerance.

(1) and (2) are exact: a ranking is the product, and a tie broken
toward the wrong index silently flips a candidate. (3) allows for the
two sides accumulating a float32 dot product in a different order —
numpy tiles it, Rust walks it linearly.

Weights are read from the `.trm` blob, i.e. the same bytes the Rust
binary loads, so a mismatch cannot be blamed on two different models.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from model import QueryFeatures, RerankerWeights, score_batch  # noqa: E402

# f32 dot products with different summation orders land within ~1e-6 of
# each other; 1e-4 is three orders of magnitude of headroom over that
# while still far tighter than the gaps that decide a ranking.
TOL = 1e-4


def read_candidates(path: Path) -> list[tuple[str, list[tuple[str, int]]]]:
    """Read `query<TAB>idx<TAB>word<TAB>edit_distance` rows."""
    groups: list[tuple[str, list[tuple[str, int]]]] = []
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) != 4:
            raise SystemExit(f"{path}:{lineno}: expected 4 columns, got {len(parts)}")
        q, idx_s, word, ed_s = parts
        idx = int(idx_s)
        ed = int(ed_s)
        if not groups or groups[-1][0] != q:
            if idx != 0:
                raise SystemExit(f"{path}:{lineno}: {q!r} starts at {idx}, expected 0")
            groups.append((q, [(word, ed)]))
        else:
            if idx != len(groups[-1][1]):
                raise SystemExit(
                    f"{path}:{lineno}: {q!r} index {idx}, expected {len(groups[-1][1])}"
                )
            groups[-1][1].append((word, ed))
    if not groups:
        raise SystemExit(f"{path}: no candidate rows")
    return groups


def read_rankings(path: Path) -> dict[str, list[tuple[int, str, int, float]]]:
    """Read `rerank`'s TSV into `{query: [(rank, cand, ed, logit), ...]}`."""
    out: dict[str, list[tuple[int, str, int, float]]] = {}
    for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line:
            continue
        parts = line.split("\t")
        if parts[0] == "query":
            continue  # header
        if len(parts) != 5:
            raise SystemExit(f"{path}:{lineno}: expected 5 columns, got {len(parts)}")
        q, rank_s, cand, ed_s, logit_s = parts
        out.setdefault(q, []).append((int(rank_s), cand, int(ed_s), float(logit_s)))
    if not out:
        raise SystemExit(f"{path}: no ranking rows")
    return out


def order_by_logit(logits: list[float]) -> list[int]:
    """Python's tie-break: highest logit first, ties toward the lowest index.

    Matches `sorted(range(n), key=lambda i: (-logit, i))`, which is what
    `parity_test.py` records and what Rust's `ModelA::argmax` promises.
    """
    return sorted(range(len(logits)), key=lambda i: (-logits[i], i))


def cmd_export(src: Path, dst: Path) -> int:
    rows = 0
    skipped = 0
    out: list[str] = []
    for line in src.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            skipped += 1
            continue
        q = row["misspelled"]
        for i, c in enumerate(row["candidates"]):
            out.append(f"{q}\t{i}\t{c['word']}\t{c.get('edit_distance', 0)}")
        rows += 1
    if not rows:
        raise SystemExit(f"{src}: no usable rows")
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_text("\n".join(out) + "\n", encoding="utf-8")
    note = f", {skipped} malformed row(s) skipped" if skipped else ""
    print(f"{src} -> {dst}: {rows} queries, {len(out)} candidates{note}")
    return 0


def cmd_compare(cand_path: Path, trm_path: Path, rust_path: Path) -> int:
    weights = RerankerWeights.load_trm(trm_path)
    print(f"weights : {trm_path} ({weights.param_count()} params, "
          f"embed={weights.embed_dim} hidden={weights.hidden_dim} "
          f"bigram={weights.bigram_dim} trigram={weights.trigram_dim} "
          f"edit={weights.edit_dim})")

    groups = read_candidates(cand_path)
    rust = read_rankings(rust_path)

    n_query = 0
    n_ranked = 0
    n_cand = 0
    argmax_bad: list[str] = []
    order_bad: list[str] = []
    max_abs = 0.0
    worst = ("", -1)

    for query, cands in groups:
        # A query `rerank` was asked to rank (it may have been cut off
        # by --top).
        got = rust.get(query)
        if got is None:
            continue
        n_query += 1
        n_cand += len(cands)
        words = [w for w, _ in cands]
        eds = [e for _, e in cands]
        qf = QueryFeatures.from_word(query, weights)
        logits = [float(v) for v in score_batch(weights, qf, words, eds)]
        order = order_by_logit(logits)

        # Rust emitted (rank, word, ed, logit) in descending logit order.
        got_sorted = sorted(got)
        got_order_words = [g[1] for g in got_sorted]
        k = len(got_sorted)
        want_order_words = [words[i] for i in order[:k]]
        n_ranked += k

        if got_order_words[0] != want_order_words[0]:
            argmax_bad.append(f"{query!r}: rust {got_order_words[0]!r}, "
                              f"python {want_order_words[0]!r}")
        if got_order_words != want_order_words:
            order_bad.append(f"{query!r}: rust {got_order_words}, "
                             f"python {want_order_words}")

        for rank, word, _ed, lg in got_sorted:
            idx = words.index(word)
            d = abs(lg - logits[idx])
            if d > max_abs:
                max_abs = d
                worst = (f"{query!r}->{word!r}", d)

    print(f"queries compared : {n_query} / {len(groups)}")
    print(f"rankings compared: {n_ranked} positions over {n_cand} candidates")
    print(f"argmax agreement : {n_query - len(argmax_bad)}/{n_query}")
    print(f"full-order agree.: {n_query - len(order_bad)}/{n_query}")
    print(f"max |d logit|    : {max_abs:.3e} at {worst[0]} (tolerance {TOL:g})")

    fail = False
    for msg in argmax_bad[:10]:
        print(f"  ARGMAX MISMATCH {msg}")
    for msg in order_bad[:10]:
        print(f"  ORDER MISMATCH  {msg}")
    if argmax_bad:
        print(f"FAILED: {len(argmax_bad)} argmax mismatch(es)")
        fail = True
    if order_bad:
        print(f"FAILED: {len(order_bad)} ordering mismatch(es)")
        fail = True
    if max_abs > TOL:
        print(f"FAILED: max logit delta {max_abs:.3e} exceeds {TOL:g}")
        fail = True
    if not fail:
        print("OK: Python and Rust produce identical rankings")
    return 1 if fail else 0


def main(argv: list[str]) -> int:
    if len(argv) >= 4 and argv[1] == "export":
        return cmd_export(Path(argv[2]), Path(argv[3]))
    if len(argv) >= 5 and argv[1] == "compare":
        return cmd_compare(Path(argv[2]), Path(argv[3]), Path(argv[4]))
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
