#!/usr/bin/env bash
# Build the torille Firefox add-on from a source archive.
#
# Requirements and manual steps: SOURCE_BUILD.md.
set -euo pipefail

cd "$(dirname "$0")"

for cmd in cargo wasm-pack node npm zip; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "error: '$cmd' is not installed or not on PATH" >&2
    echo "see SOURCE_BUILD.md for the required versions" >&2
    exit 1
  fi
done

if ! compgen -G "maps/*.bin.gz" >/dev/null; then
  echo "error: maps/*.bin.gz are missing from this source archive" >&2
  exit 1
fi

# The binary maps are included, so the build must not try to fetch the
# orphan maps branch from the network.
export TORILLE_NO_FETCH=1

echo "==> building the Rust engine"
cargo build --release -p torille

echo "==> building wasm (wasm-pack)"
(cd torille-wasm && wasm-pack build --release)

echo "==> installing extension dependencies (npm ci)"
(cd extensions/firefox && npm ci)

echo "==> building the Firefox extension"
(cd extensions/firefox && npm run build)

if [[ ! -f extensions/firefox/torille-firefox.xpi ]]; then
  echo "error: extensions/firefox/torille-firefox.xpi was not produced" >&2
  exit 1
fi

echo
echo "==> done"
echo "unpacked: extensions/firefox/dist/"
echo "package:  extensions/firefox/torille-firefox.xpi"
sha256sum extensions/firefox/torille-firefox.xpi
