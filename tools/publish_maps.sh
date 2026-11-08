#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Replace the orphan `maps` branch with the current `maps/*.bin.gz`.
#
# The branch is always exactly one commit with no parent, so the repository
# only ever holds the current map blobs: the force-push makes the previous
# commit unreachable. `master` does not track the maps at all; run
# `tools/fetch_maps.sh` after cloning to restore them.
#
# Regenerate first (`cargo build -p torille`, `gen_derived`, and — when the
# oracle changed — `tools/oracle/build_oi_plural_lemmas.py`), then:
#
#   tools/publish_maps.sh
#
# Set MAPS_REMOTE to use a remote other than `origin`.

set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

remote=${MAPS_REMOTE:-origin}
shopt -s nullglob
blobs=(maps/*.bin.gz)
if (( ${#blobs[@]} == 0 )); then
    echo "publish_maps: no maps/*.bin.gz; build the maps first" >&2
    exit 1
fi

index=$(mktemp "${TMPDIR:-/tmp}/torille-maps-index.XXXXXX")
trap 'rm -f "$index"' EXIT

# A tree containing only the current maps. `.gitignore` hides them on master,
# so `add -f` is required.
GIT_INDEX_FILE=$index git read-tree --empty
GIT_INDEX_FILE=$index git add -f -- 'maps/*.bin.gz' maps/README.md
tree=$(GIT_INDEX_FILE=$index git write-tree)
commit=$(git commit-tree "$tree" -m "Current binary maps")

git update-ref refs/heads/maps "$commit"
git push --force "$remote" refs/heads/maps:refs/heads/maps
git fetch --force "$remote" "maps:refs/remotes/$remote/maps"

echo "publish_maps: ${#blobs[@]} blobs on the '$remote/maps' branch ($commit)"
