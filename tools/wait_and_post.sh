#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Wait for all 4 rerank_data shards to finish, then concatenate
# and run the post-rerank pipeline.
#
# Idempotent: re-running will skip the concat if train.jsonl already
# exists in the output dir, and the post_rerank.py output is also
# resumable.
#
# Usage: tools/wait_and_post.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUT=data/rerank_v2
N=4
LOG="$OUT/POST_PIPELINE.log"

echo "=== WAITING FOR ALL $N SHARDS ===" | tee "$LOG"
date | tee -a "$LOG"

# Poll for completion.
while true; do
  done_count=0
  for i in $(seq 0 $((N-1))); do
    log_path="$OUT/shard$i/log.txt"
    if [[ -f "$log_path" ]] && grep -q "rerank_data summary:" "$log_path"; then
      done_count=$((done_count + 1))
    fi
  done
  echo "  $(date +%H:%M:%S): $done_count/$N shards done" | tee -a "$LOG"
  if [[ "$done_count" -eq "$N" ]]; then
    break
  fi
  sleep 30
done

echo "" | tee -a "$LOG"
echo "=== ALL SHARDS DONE; CONCATENATING ===" | tee -a "$LOG"

# Concatenate per-shard train.jsonl into one.
if [[ ! -f "$OUT/train.jsonl" ]] || [[ "$OUT/shard0/train.jsonl" -nt "$OUT/train.jsonl" ]]; then
  : > "$OUT/train.jsonl"
  for i in $(seq 0 $((N-1))); do
    cat "$OUT/shard$i/train.jsonl" >> "$OUT/train.jsonl"
  done
  # Copy dev/test from shard 0 (identical across shards).
  for f in dev.jsonl test_unseen_lemma.jsonl test_unseen_inflection.jsonl test_unseen_corruption.jsonl; do
    if [[ -f "$OUT/shard0/$f" ]]; then
      cp "$OUT/shard0/$f" "$OUT/$f"
    fi
  done
  echo "  concatenated $(wc -l < $OUT/train.jsonl) train rows" | tee -a "$LOG"
else
  echo "  train.jsonl already up to date" | tee -a "$LOG"
fi

echo "" | tee -a "$LOG"
echo "=== RUNNING POST_RERANK PIPELINE ===" | tee -a "$LOG"
date | tee -a "$LOG"

python3 tools/post_rerank.py \
  --data-dir "$OUT" \
  --cross-data-dir data/rerank_scale \
  --output "$OUT/ABLATION_REPORT.md" \
  --epochs 20 \
  --seed 0 \
  2>&1 | tee -a "$LOG"

echo "" | tee -a "$LOG"
echo "=== POST PIPELINE DONE ===" | tee -a "$LOG"
date | tee -a "$LOG"
