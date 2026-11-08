#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Restore the binary maps from the orphan `maps` branch.
#
# The maps are generated artefacts. They are **not** tracked on `master`, so
# the normal history never accumulates old versions; the `maps` branch is a
# single-commit orphan that `tools/publish_maps.sh` force-pushes with the
# current blobs. This script fetches that branch and unpacks only the
# `maps/*.bin.gz` files into the working tree, where `.gitignore` keeps them
# out of `git status`.
#
# Usage:
#   tools/fetch_maps.sh              # fetch origin/maps and restore the blobs
#   tools/fetch_maps.sh --offline    # only require that the blobs are present
#
# Set MAPS_REMOTE to use a remote other than `origin`.

set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

remote=${MAPS_REMOTE:-origin}
ref="refs/remotes/$remote/maps"
offline=0
case ${1:-} in
    --offline) offline=1 ;;
    "") ;;
    *)
        echo "usage: $0 [--offline]" >&2
        exit 2
        ;;
esac

present=$(find maps -maxdepth 1 -name '*.bin.gz' 2>/dev/null | wc -l)
if (( offline )); then
    if (( present == 0 )); then
        echo "fetch_maps: no maps/*.bin.gz present (and --offline was given)" >&2
        exit 1
    fi
    echo "fetch_maps: $present blobs already present"
    exit 0
fi

git fetch --force "$remote" "maps:$ref"

if ! git rev-parse --verify --quiet "$ref" >/dev/null; then
    echo "fetch_maps: $ref is missing; the $remote remote has no 'maps' branch" >&2
    exit 1
fi

mkdir -p maps
find maps -maxdepth 1 -name '*.bin.gz' -delete
git archive "$ref" -- 'maps/*.bin.gz' | tar -x -C "$root"

restored=$(find maps -maxdepth 1 -name '*.bin.gz' | wc -l)
if (( restored == 0 )); then
    echo "fetch_maps: the $ref commit contains no maps/*.bin.gz" >&2
    exit 1
fi
echo "fetch_maps: restored $restored blobs from $ref"
